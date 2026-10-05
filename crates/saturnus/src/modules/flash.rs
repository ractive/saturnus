//! The HP 49G flash: one Intel 28F160S5, 2 MB, byte-wide (x8), on NCE1
//! for reads and on NCE3 for writes (wiki: hardware/hp49g).
//!
//! Chip facts come from the Intel datasheet "5 Volt FlashFile Memory
//! 28F160S5 and 28F320S5 (x8/x16)", order number 290609-004, December 1998
//! (wiki: sources/intel-28f160s5):
//!
//! - 2 MB in 32 erase blocks of 64 KB; erase sets a block to #FF, programming
//!   can only turn 1 bits into 0 bits (sections 4.6, 4.9, Figure 4).
//! - A command user interface (CUI) takes one-byte commands (Table 3): #FF
//!   read array, #90 read identifier, #98 read query, #70 read status, #50
//!   clear status, #40/#10 program, #20 + #D0 block erase, #30 + #D0 full
//!   chip erase, #E8 write to buffer, #B0 suspend, #D0 resume, #60 + #01 set
//!   block lock-bit, #60 + #D0 clear block lock-bits, #B8 STS configuration.
//! - After program, erase or lock commands the chip outputs the status
//!   register on every read until another read-mode command (4.4, 4.6, 4.9).
//! - Status register (Table 15): bit 7 ready, 6 erase suspended, 5 erase /
//!   clear-lock error, 4 program / set-lock error, 3 VPP low, 2 program
//!   suspended, 1 device protected. Bits 5, 4, 3 and 1 stay set until
//!   clear status (4.5). Reset value #80 (3.4).
//! - Identifier codes (Table 12): manufacturer #B0 at word 0, device #D0 at
//!   word 1, block lock and erase status at word 2 of each block. In x8
//!   mode the byte address bit A0 is ignored for identifier and query data.
//!
//! Emulation choices, none of them observed on hardware:
//!
//! - Every operation completes at once: status reads always show ready
//!   (bit 7 set) and suspend has nothing to suspend.
//! - The Saturn writes nibbles to a byte-wide chip. A chip byte is the two
//!   nibbles at nibble addresses `2b` (low half) and `2b + 1` (high half),
//!   the packed-image convention (wiki: emulators/emu48, "packed" images).
//!   A write to the even nibble is held; the following write to the odd
//!   nibble of the same byte completes one byte write cycle on the chip.
//!   An odd-nibble write without its even half is dropped. So `DAT1=C B`
//!   at an even address is one command or data byte, low nibble first.
//!   How the 49G glue logic really turns nibble writes into byte cycles is
//!   undocumented (wiki: questions/hp49g-flash-write).
//! - Writes only reach the chip while [`Flash::write_enabled`] is set; the
//!   machine drives it from #11C bit 3 (wiki: hardware/hp49g "Controllers").
//! - VPP is always at programming level. WP# defaults to high, so lock-bits
//!   never block anything; the hardware protection of the boot sector
//!   (tutorial p. 161) has an unknown mechanism. [`Flash::set_wp_low`] and
//!   the lock-bit accessors let the machine model it once it is known.
//! - Unknown command bytes are ignored. Clear status leaves the read mode
//!   alone.
//!
//! Banking (wiki: hardware/hp49g, questions/hp49g-bank-latch-bits): the
//! CE1 latch holds nibble address bits A1-A6 of the latching access. A1-A4
//! pick the 128 KB bank (0-15) seen at #40000-#7FFFF, A5-A6 the bank (0-3)
//! seen at #00000-#3FFFF: Sousa's assignment (`base + 2*n` for the high
//! view, `base + #20*n` for the low one). Giesselink's opposite assignment
//! (A1-A2 low, A3-A6 high) does not boot: the ROM 2.10 and 1.19-6 boot
//! sectors then report "No System", and 2.15 runs into data. The flash
//! ignores A19, so both views mirror at #80000-#FFFFF.

use crate::Error;

/// Size of the chip in bytes.
pub const FLASH_BYTES: usize = 2 * 1024 * 1024;
/// Size of the chip in nibbles.
pub const FLASH_NIBBLES: usize = 2 * FLASH_BYTES;
/// One 49G bank: 128 KB, the #40000-nibble window one view shows.
pub const BANK_NIBBLES: usize = 0x4_0000;
/// Number of 49G banks.
pub const BANKS: usize = FLASH_NIBBLES / BANK_NIBBLES;
/// Size of one erase block in bytes (datasheet Figure 4).
pub const BLOCK_BYTES: usize = 64 * 1024;
/// Number of erase blocks.
pub const BLOCKS: usize = FLASH_BYTES / BLOCK_BYTES;

/// Manufacturer code (datasheet Table 12).
pub const MANUFACTURER_CODE: u8 = 0xB0;
/// Device code of the 16 Mbit part (datasheet Table 12).
pub const DEVICE_CODE: u8 = 0xD0;

/// Status register bits (datasheet Table 15).
pub mod sr {
    /// Write state machine ready.
    pub const READY: u8 = 0x80;
    /// Block erase suspended.
    pub const ERASE_SUSPENDED: u8 = 0x40;
    /// Error in block erase or clear lock-bits.
    pub const ERASE_ERROR: u8 = 0x20;
    /// Error in program or set lock-bit.
    pub const PROGRAM_ERROR: u8 = 0x10;
    /// VPP low, operation aborted.
    pub const VPP_LOW: u8 = 0x08;
    /// Program suspended.
    pub const PROGRAM_SUSPENDED: u8 = 0x04;
    /// Lock-bit or WP# protection detected, operation aborted.
    pub const PROTECTED: u8 = 0x02;
    /// Bits only clear status resets (datasheet 4.5).
    pub const STICKY: u8 = ERASE_ERROR | PROGRAM_ERROR | VPP_LOW | PROTECTED;
}

/// The two banks the CE1 latch selects, from the latch value (nibble
/// address bits A1-A6, bit 0 = A1). Returns `(low, high)`: the bank at
/// #00000-#3FFFF (0-3, from A5-A6) and the bank at #40000-#7FFFF (0-15,
/// from A1-A4) (wiki: questions/hp49g-bank-latch-bits, Sousa's
/// assignment, confirmed by booting the ROM).
pub fn latch_banks(latch: u8) -> (usize, usize) {
    (usize::from((latch >> 4) & 0x3), usize::from(latch & 0xF))
}

/// The latch value a latching read at nibble address `addr` stores: its
/// bits A1-A6.
pub fn latch_from_address(addr: u32) -> u8 {
    ((addr >> 1) & 0x3F) as u8
}

/// What reads return (datasheet 3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadMode {
    /// Array contents.
    Array,
    /// Identifier codes (#90).
    Identifier,
    /// CFI query data (#98).
    Query,
    /// The status register (#70 and after every write operation).
    Status,
    /// The extended status register (#E8 setup).
    ExtendedStatus,
}

/// What the next byte cycle means to the CUI.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pending {
    /// A command.
    Command,
    /// Program data for the address of this cycle (#40/#10).
    Program,
    /// Confirm of block erase (#20).
    EraseConfirm,
    /// Confirm of full chip erase (#30).
    ChipEraseConfirm,
    /// Lock-bit confirm (#60): #01 sets, #D0 clears.
    LockConfirm,
    /// STS configuration code (#B8).
    StsConfig,
    /// Write to buffer count (#E8 at a block address).
    BufferCount { block: usize },
    /// Write to buffer data: remaining count and the bytes so far.
    BufferData {
        block: usize,
        left: usize,
        data: Vec<(usize, u8)>,
    },
    /// Write to buffer confirm.
    BufferConfirm {
        block: usize,
        data: Vec<(usize, u8)>,
    },
}

/// Size of a write buffer in bytes (datasheet Table 10, offset #2A).
const WRITE_BUFFER_BYTES: usize = 32;

/// The 28F160S5 flash chip. Addresses are chip nibble addresses
/// (0..[`FLASH_NIBBLES`]), masked to the chip size.
#[derive(Clone, PartialEq, Eq)]
pub struct Flash {
    /// Array contents, one nibble per element.
    nibbles: Vec<u8>,
    mode: ReadMode,
    pending: Pending,
    status: u8,
    /// Block lock-bits, bit n = block n (non-volatile on the chip).
    lock_bits: u32,
    /// WP# driven low: lock-bits are enforced.
    wp_low: bool,
    /// The machine's write gate (#11C bit 3).
    write_enabled: bool,
    /// Even nibble of a byte write: (byte address, nibble).
    held: Option<(usize, u8)>,
}

impl std::fmt::Debug for Flash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Flash")
            .field("mode", &self.mode)
            .field("pending", &self.pending)
            .field("status", &format_args!("{:#04X}", self.status))
            .field("lock_bits", &format_args!("{:#010X}", self.lock_bits))
            .field("wp_low", &self.wp_low)
            .field("write_enabled", &self.write_enabled)
            .finish_non_exhaustive()
    }
}

impl Flash {
    fn from_nibble_vec(nibbles: Vec<u8>) -> Self {
        Self {
            nibbles,
            mode: ReadMode::Array,
            pending: Pending::Command,
            status: sr::READY,
            lock_bits: 0,
            wp_low: false,
            write_enabled: false,
            held: None,
        }
    }

    /// A chip from a packed image: 2097152 bytes, two nibbles per byte,
    /// the even nibble in the low half (as `Rom::from_packed`). This is
    /// also the chip's own byte layout.
    pub fn from_packed(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != FLASH_BYTES {
            return Err(Error::RomSize {
                expected: FLASH_BYTES,
                actual: bytes.len(),
            });
        }
        let nibbles = bytes.iter().flat_map(|&b| [b & 0xF, b >> 4]).collect();
        Ok(Self::from_nibble_vec(nibbles))
    }

    /// A chip from an unpacked image: 4194304 bytes, one nibble per byte
    /// (the 1.19-6 emulator image is in this form); each value is masked
    /// to 4 bits.
    pub fn from_unpacked(nibbles: &[u8]) -> Result<Self, Error> {
        if nibbles.len() != FLASH_NIBBLES {
            return Err(Error::RomSize {
                expected: FLASH_NIBBLES,
                actual: nibbles.len(),
            });
        }
        Ok(Self::from_nibble_vec(
            nibbles.iter().map(|n| n & 0xF).collect(),
        ))
    }

    /// An erased chip (all #FF).
    pub fn erased() -> Self {
        Self::from_nibble_vec(vec![0xF; FLASH_NIBBLES])
    }

    /// The array as a packed image, for saving; the inverse of
    /// [`Flash::from_packed`].
    pub fn to_packed(&self) -> Vec<u8> {
        self.nibbles
            .chunks_exact(2)
            .map(|p| p[0] | (p[1] << 4))
            .collect()
    }

    /// The array contents, one nibble per element, whatever the read mode.
    pub fn nibbles(&self) -> &[u8] {
        &self.nibbles
    }

    /// Bank `n` (0-15) of the array, one nibble per element. `n` wraps
    /// modulo 16.
    pub fn bank(&self, n: usize) -> &[u8] {
        let start = (n % BANKS) * BANK_NIBBLES;
        &self.nibbles[start..start + BANK_NIBBLES]
    }

    /// Chip nibble address that CPU address `addr` reaches through NCE1
    /// with `latch` in the bank latch: bit 18 picks the view (low bank at
    /// #00000-#3FFFF, high bank at #40000-#7FFFF), bit 19 is ignored.
    pub fn banked_address(addr: u32, latch: u8) -> usize {
        let (low, high) = latch_banks(latch);
        let bank = if addr & 0x4_0000 == 0 { low } else { high };
        bank * BANK_NIBBLES + (addr as usize & (BANK_NIBBLES - 1))
    }

    /// Chip nibble address that CPU address `addr` reaches through NCE3,
    /// the write path: always the bank of the #40000 view (wiki:
    /// hardware/hp49g "Controllers"; NCE3 sits at #40000 while writing).
    pub fn write_address(addr: u32, latch: u8) -> usize {
        let (_, high) = latch_banks(latch);
        high * BANK_NIBBLES + (addr as usize & (BANK_NIBBLES - 1))
    }

    /// Read through NCE1: [`Flash::read`] at [`Flash::banked_address`].
    pub fn read_banked(&self, addr: u32, latch: u8) -> u8 {
        self.read(Self::banked_address(addr, latch))
    }

    /// Write through NCE3: [`Flash::write`] at [`Flash::write_address`].
    pub fn write_banked(&mut self, addr: u32, latch: u8, nibble: u8) {
        self.write(Self::write_address(addr, latch), nibble);
    }

    /// Current read mode.
    pub fn read_mode(&self) -> ReadMode {
        self.mode
    }

    /// The status register.
    pub fn status(&self) -> u8 {
        self.status
    }

    /// Whether writes reach the chip (#11C bit 3 on the 49G).
    pub fn write_enabled(&self) -> bool {
        self.write_enabled
    }

    /// Opens or closes the write gate. Closing it drops a held nibble.
    pub fn set_write_enabled(&mut self, enabled: bool) {
        self.write_enabled = enabled;
        if !enabled {
            self.held = None;
        }
    }

    /// Drives WP#: low enforces the block lock-bits (datasheet Table 13).
    pub fn set_wp_low(&mut self, low: bool) {
        self.wp_low = low;
    }

    /// Block lock-bits, bit n = erase block n.
    pub fn lock_bits(&self) -> u32 {
        self.lock_bits
    }

    /// Sets the block lock-bits (restoring a saved state, or modelling a
    /// factory lock).
    pub fn set_lock_bits(&mut self, bits: u32) {
        self.lock_bits = bits;
    }

    /// WP# state (true = driven low).
    pub fn wp_low(&self) -> bool {
        self.wp_low
    }

    /// Restores the status register and read mode from a saved state; the
    /// command sequence in progress, if any, is dropped. The caller checks
    /// that `status` is ready and not suspended (`Nce1::load_state_blob`
    /// refuses other values).
    pub fn restore_cui(&mut self, status: u8, mode: ReadMode) {
        self.status = status;
        self.mode = mode;
        self.pending = Pending::Command;
        self.held = None;
    }

    /// Power-on or RP# reset: read array mode, status #80, no pending
    /// command (datasheet 3.4). Array and lock-bits are kept.
    pub fn reset(&mut self) {
        self.mode = ReadMode::Array;
        self.pending = Pending::Command;
        self.status = sr::READY;
        self.held = None;
    }

    /// The nibble at chip nibble address `addr` in the current read mode.
    pub fn read(&self, addr: usize) -> u8 {
        let addr = addr % FLASH_NIBBLES;
        let byte = match self.mode {
            ReadMode::Array => return self.nibbles[addr],
            ReadMode::Status => self.status,
            ReadMode::ExtendedStatus => sr::READY, // a write buffer is free
            ReadMode::Identifier => self.identifier_byte(addr / 2),
            ReadMode::Query => self.query_byte(addr / 2),
        };
        if addr & 1 == 0 { byte & 0xF } else { byte >> 4 }
    }

    /// Writes nibble `nibble` at chip nibble address `addr`: holds an even
    /// nibble, completes a byte cycle on the odd one (see the module docs).
    pub fn write(&mut self, addr: usize, nibble: u8) {
        if !self.write_enabled {
            return;
        }
        let addr = addr % FLASH_NIBBLES;
        let byte_addr = addr / 2;
        let nibble = nibble & 0xF;
        if addr & 1 == 0 {
            self.held = Some((byte_addr, nibble));
            return;
        }
        if let Some((held_addr, low)) = self.held.take()
            && held_addr == byte_addr
        {
            self.write_byte(byte_addr, low | (nibble << 4));
        }
    }

    /// One byte write cycle at chip byte address `addr` (datasheet 3.7).
    pub fn write_byte(&mut self, addr: usize, value: u8) {
        let addr = addr % FLASH_BYTES;
        const BAD_SEQUENCE: u8 = sr::ERASE_ERROR | sr::PROGRAM_ERROR;
        let pending = std::mem::replace(&mut self.pending, Pending::Command);
        match pending {
            Pending::Command => self.command(addr, value),
            Pending::Program => {
                if self.blocked(addr / BLOCK_BYTES) {
                    self.status |= sr::PROTECTED | sr::PROGRAM_ERROR;
                } else {
                    self.program_byte(addr, value);
                }
                self.mode = ReadMode::Status;
            }
            Pending::EraseConfirm => {
                let block = addr / BLOCK_BYTES;
                if value != 0xD0 {
                    self.status |= BAD_SEQUENCE;
                } else if self.blocked(block) {
                    self.status |= sr::PROTECTED | sr::ERASE_ERROR;
                } else {
                    self.erase_block(block);
                }
                self.mode = ReadMode::Status;
            }
            Pending::ChipEraseConfirm => {
                if value == 0xD0 {
                    for block in 0..BLOCKS {
                        if !self.blocked(block) {
                            self.erase_block(block);
                        }
                    }
                } else {
                    self.status |= BAD_SEQUENCE;
                }
                self.mode = ReadMode::Status;
            }
            Pending::LockConfirm => {
                match value {
                    0x01 if self.wp_low => self.status |= sr::PROTECTED | sr::PROGRAM_ERROR,
                    0x01 => self.lock_bits |= 1 << (addr / BLOCK_BYTES),
                    0xD0 if self.wp_low => self.status |= sr::PROTECTED | sr::ERASE_ERROR,
                    0xD0 => self.lock_bits = 0,
                    _ => self.status |= BAD_SEQUENCE,
                }
                self.mode = ReadMode::Status;
            }
            Pending::StsConfig => {
                if value > 3 {
                    self.status |= BAD_SEQUENCE;
                }
                self.mode = ReadMode::Status;
            }
            Pending::BufferCount { block } => {
                let count = usize::from(value) + 1;
                if count > WRITE_BUFFER_BYTES {
                    self.status |= BAD_SEQUENCE;
                    self.mode = ReadMode::Status;
                } else {
                    self.mode = ReadMode::Status;
                    self.pending = Pending::BufferData {
                        block,
                        left: count,
                        data: Vec::with_capacity(count),
                    };
                }
            }
            Pending::BufferData {
                block,
                left,
                mut data,
            } => {
                data.push((addr, value));
                self.pending = if left > 1 {
                    Pending::BufferData {
                        block,
                        left: left - 1,
                        data,
                    }
                } else {
                    Pending::BufferConfirm { block, data }
                };
            }
            Pending::BufferConfirm { block, data } => {
                // Writing past the block aborts with an invalid sequence
                // (datasheet 4.8).
                let in_block = data.iter().all(|(a, _)| a / BLOCK_BYTES == block);
                if value != 0xD0 || !in_block {
                    self.status |= BAD_SEQUENCE;
                } else if self.blocked(block) {
                    self.status |= sr::PROTECTED | sr::PROGRAM_ERROR;
                } else {
                    for (a, d) in data {
                        self.program_byte(a, d);
                    }
                }
                self.mode = ReadMode::Status;
            }
        }
    }

    /// A byte cycle while the CUI expects a command (datasheet Table 3).
    fn command(&mut self, addr: usize, value: u8) {
        match value {
            0xFF => self.mode = ReadMode::Array,
            0x90 => self.mode = ReadMode::Identifier,
            0x98 => self.mode = ReadMode::Query,
            0x70 => self.mode = ReadMode::Status,
            0x50 => self.status &= !sr::STICKY,
            0x40 | 0x10 => self.pending = Pending::Program,
            0x20 => self.pending = Pending::EraseConfirm,
            0x30 => self.pending = Pending::ChipEraseConfirm,
            0x60 => self.pending = Pending::LockConfirm,
            0xB8 => self.pending = Pending::StsConfig,
            0xE8 => {
                // Once an error is set the chip refuses buffered writes
                // (datasheet 4.8); XSR.7 still reads "available" here
                // because nothing is ever busy.
                self.mode = ReadMode::ExtendedStatus;
                if self.status & (sr::ERASE_ERROR | sr::PROGRAM_ERROR) == 0 {
                    self.pending = Pending::BufferCount {
                        block: addr / BLOCK_BYTES,
                    };
                }
            }
            // Suspend and resume: nothing is ever running, so both only
            // switch to status output (datasheet 4.11, 4.12).
            0xB0 | 0xD0 => self.mode = ReadMode::Status,
            _ => {}
        }
    }

    /// Whether lock-bits stop program and erase on `block` (datasheet
    /// Table 13: locked and WP# low).
    fn blocked(&self, block: usize) -> bool {
        self.wp_low && self.lock_bits & (1 << block) != 0
    }

    /// Programs one byte: bits can only go from 1 to 0 (datasheet 4.9).
    fn program_byte(&mut self, addr: usize, value: u8) {
        let n = 2 * addr;
        self.nibbles[n] &= value & 0xF;
        self.nibbles[n + 1] &= value >> 4;
    }

    /// Sets every byte of erase block `block` to #FF (datasheet 4.6).
    fn erase_block(&mut self, block: usize) {
        let start = 2 * block * BLOCK_BYTES;
        self.nibbles[start..start + 2 * BLOCK_BYTES].fill(0xF);
    }

    /// Block status byte (identifier and query word 2 of each block):
    /// bit 0 lock-bit, bit 1 last erase failed (never, here).
    fn block_status(&self, byte_addr: usize) -> u8 {
        u8::from(self.lock_bits & (1 << (byte_addr / BLOCK_BYTES)) != 0)
    }

    /// Identifier data at chip byte address `byte_addr` (datasheet Table
    /// 12, Figure 5). A0 is ignored; the codes repeat in every block and
    /// the reserved words read 0.
    fn identifier_byte(&self, byte_addr: usize) -> u8 {
        match (byte_addr % BLOCK_BYTES) >> 1 {
            0 => MANUFACTURER_CODE,
            1 => DEVICE_CODE,
            2 => self.block_status(byte_addr),
            _ => 0,
        }
    }

    /// CFI query data at chip byte address `byte_addr` (datasheet Tables 6
    /// to 11, values for the 28F160S5). A0 is ignored, so x8 reads see
    /// every byte twice ("Q", "Q", "R", "R", ...). Like the identifier
    /// codes, the table repeats in every block.
    fn query_byte(&self, byte_addr: usize) -> u8 {
        let word = (byte_addr % BLOCK_BYTES) >> 1;
        match word {
            0..=2 => self.identifier_byte(byte_addr),
            0x10..=0x3E => QUERY_TABLE[word - 0x10],
            _ => 0,
        }
    }
}

/// CFI query words #10-#3E of the 28F160S5 (datasheet Tables 8-11).
const QUERY_TABLE: [u8; 0x2F] = [
    // #10: "QRY", primary command set 1, extended table at #31, no
    // alternate command set.
    0x51, 0x52, 0x59, 0x01, 0x00, 0x31, 0x00, 0x00, 0x00, 0x00, 0x00,
    // #1B: VCC and VPP 2.7-5.5 V, timeouts.
    0x27, 0x55, 0x27, 0x55, 0x03, 0x06, 0x0A, 0x0F, 0x04, 0x04, 0x04, 0x04,
    // #27: 2^21 bytes, x8/x16, 32-byte buffer, one region of 32 blocks
    // of 256 x 256 bytes.
    0x15, 0x02, 0x00, 0x05, 0x00, 0x01, 0x1F, 0x00, 0x00, 0x01,
    // #31: "PRI" 1.0, features #0F, program after erase suspend,
    // block status mask 3, optimum VCC and VPP 5.0 V.
    0x50, 0x52, 0x49, 0x31, 0x30, 0x0F, 0x00, 0x00, 0x00, 0x01, 0x03, 0x00, 0x50, 0x50,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A writable erased chip.
    fn chip() -> Flash {
        let mut f = Flash::erased();
        f.set_write_enabled(true);
        f
    }

    /// Writes byte `value` at chip byte address `byte` as the Saturn
    /// would: two nibble writes, low nibble first.
    fn put(f: &mut Flash, byte: usize, value: u8) {
        f.write(2 * byte, value & 0xF);
        f.write(2 * byte + 1, value >> 4);
    }

    /// Reads the byte at chip byte address `byte` in the current mode.
    fn get(f: &Flash, byte: usize) -> u8 {
        f.read(2 * byte) | (f.read(2 * byte + 1) << 4)
    }

    #[test]
    fn packed_round_trip_and_sizes() {
        let mut img = vec![0u8; FLASH_BYTES];
        img[0] = 0x21;
        img[FLASH_BYTES - 1] = 0x9A;
        let f = Flash::from_packed(&img).unwrap();
        assert_eq!(f.read(0), 1);
        assert_eq!(f.read(1), 2);
        assert_eq!(f.read(FLASH_NIBBLES - 1), 9);
        assert_eq!(f.to_packed(), img);
        assert_eq!(
            Flash::from_packed(&[0; 16]),
            Err(Error::RomSize {
                expected: FLASH_BYTES,
                actual: 16
            })
        );
        let unpacked: Vec<u8> = img.iter().flat_map(|&b| [b & 0xF, b >> 4]).collect();
        assert_eq!(Flash::from_unpacked(&unpacked).unwrap().to_packed(), img);
        assert!(Flash::from_unpacked(&img).is_err());
    }

    #[test]
    fn writes_need_the_gate() {
        let mut f = Flash::erased();
        put(&mut f, 0x10, 0x40);
        put(&mut f, 0x10, 0x00);
        assert_eq!(get(&f, 0x10), 0xFF);
        assert_eq!(f.read_mode(), ReadMode::Array);
        // A held low nibble dies with the gate.
        f.set_write_enabled(true);
        f.write(0x20, 0x0);
        f.set_write_enabled(false);
        f.set_write_enabled(true);
        f.write(0x21, 0x7);
        assert_eq!(f.read_mode(), ReadMode::Array, "no #70 reached the chip");
    }

    #[test]
    fn odd_nibble_alone_is_dropped() {
        let mut f = chip();
        f.write(0x21, 0x7); // high half of #70 without its low half
        assert_eq!(f.read_mode(), ReadMode::Array);
        f.write(0x20, 0x0);
        f.write(0x23, 0x7); // high half of another byte
        assert_eq!(f.read_mode(), ReadMode::Array);
    }

    #[test]
    fn program_only_clears_bits() {
        let mut f = chip();
        put(&mut f, 0x1234, 0x40);
        put(&mut f, 0x1234, 0x5A);
        assert_eq!(f.read_mode(), ReadMode::Status);
        assert_eq!(get(&f, 0x1234), sr::READY);
        assert_eq!(get(&f, 0), sr::READY, "status at any address");
        put(&mut f, 0, 0xFF);
        assert_eq!(get(&f, 0x1234), 0x5A);
        // Alternate setup code #10; 1 bits cannot come back.
        put(&mut f, 0, 0x10);
        put(&mut f, 0x1234, 0xF0);
        put(&mut f, 0, 0xFF);
        assert_eq!(get(&f, 0x1234), 0x50);
        assert_eq!(f.status(), sr::READY, "no error for 0 -> 1 attempts");
    }

    #[test]
    fn block_erase() {
        let mut f = chip();
        for addr in [
            BLOCK_BYTES - 1,
            BLOCK_BYTES,
            2 * BLOCK_BYTES - 1,
            2 * BLOCK_BYTES,
        ] {
            put(&mut f, 0, 0x40);
            put(&mut f, addr, 0x00);
        }
        put(&mut f, BLOCK_BYTES + 0x100, 0x20);
        put(&mut f, BLOCK_BYTES + 0x200, 0xD0);
        assert_eq!(f.read_mode(), ReadMode::Status);
        assert_eq!(f.status(), sr::READY);
        put(&mut f, 0, 0xFF);
        assert_eq!(get(&f, BLOCK_BYTES - 1), 0x00, "block 0 untouched");
        assert_eq!(get(&f, BLOCK_BYTES), 0xFF);
        assert_eq!(get(&f, 2 * BLOCK_BYTES - 1), 0xFF);
        assert_eq!(get(&f, 2 * BLOCK_BYTES), 0x00, "block 2 untouched");
    }

    #[test]
    fn bad_confirm_sets_both_error_bits_and_clear_status_resets() {
        let mut f = chip();
        put(&mut f, 0, 0x40);
        put(&mut f, 0, 0x00);
        put(&mut f, 0, 0x20);
        put(&mut f, 0, 0xFF); // not D0
        assert_eq!(f.status(), sr::READY | sr::ERASE_ERROR | sr::PROGRAM_ERROR);
        assert_eq!(get(&f, 0), f.status(), "status output");
        put(&mut f, 0, 0xFF);
        assert_eq!(get(&f, 0), 0x00, "the erase did not happen");
        put(&mut f, 0, 0x50);
        assert_eq!(f.status(), sr::READY);
        assert_eq!(
            f.read_mode(),
            ReadMode::Array,
            "clear status keeps the mode"
        );
    }

    #[test]
    fn status_and_read_array_commands() {
        let mut f = chip();
        put(&mut f, 0x55, 0x70);
        assert_eq!(f.read_mode(), ReadMode::Status);
        assert_eq!(f.read(0x1001), 0x8);
        assert_eq!(f.read(0x1000), 0x0);
        put(&mut f, 0x55, 0xFF);
        assert_eq!(get(&f, 0x55), 0xFF);
        // Suspend and resume only switch to status output.
        put(&mut f, 0, 0xB0);
        assert_eq!(f.read_mode(), ReadMode::Status);
        put(&mut f, 0, 0xFF);
        put(&mut f, 0, 0xD0);
        assert_eq!(f.read_mode(), ReadMode::Status);
        // Unknown commands are ignored.
        put(&mut f, 0, 0xFF);
        put(&mut f, 0, 0x42);
        assert_eq!(f.read_mode(), ReadMode::Array);
    }

    #[test]
    fn identifier_codes() {
        let mut f = chip();
        put(&mut f, 0, 0x90);
        // x8: A0 ignored, so each code shows on two byte addresses.
        assert_eq!(get(&f, 0), MANUFACTURER_CODE);
        assert_eq!(get(&f, 1), MANUFACTURER_CODE);
        assert_eq!(get(&f, 2), DEVICE_CODE);
        assert_eq!(get(&f, 3), DEVICE_CODE);
        assert_eq!(get(&f, 4), 0, "block 0 unlocked");
        assert_eq!(get(&f, 6), 0);
        f.set_lock_bits(1 << 3);
        assert_eq!(get(&f, 3 * BLOCK_BYTES + 4), 1, "block 3 locked");
        assert_eq!(get(&f, 2 * BLOCK_BYTES + 4), 0);
        put(&mut f, 0, 0xFF);
        assert_eq!(get(&f, 0), 0xFF);
    }

    #[test]
    fn query_table() {
        let mut f = chip();
        put(&mut f, 0, 0x98);
        let q: Vec<u8> = (0x20..0x26).map(|b| get(&f, b)).collect();
        assert_eq!(q, b"QQRRYY");
        assert_eq!(get(&f, 2 * 0x27), 0x15, "2^21 bytes");
        assert_eq!(get(&f, 2 * 0x2D), 0x1F, "32 erase blocks");
        assert_eq!(get(&f, 2 * 0x30), 0x01, "of 64 KB");
        let pri: Vec<u8> = (0x31..0x34).map(|w| get(&f, 2 * w)).collect();
        assert_eq!(pri, b"PRI");
        assert_eq!(get(&f, 2 * 0x3E), 0x50);
        assert_eq!(get(&f, 2 * 0x3F), 0);
        assert_eq!(get(&f, 0), MANUFACTURER_CODE);
    }

    #[test]
    fn full_chip_erase() {
        let mut f = chip();
        for b in [0, FLASH_BYTES / 2, FLASH_BYTES - 1] {
            put(&mut f, 0, 0x40);
            put(&mut f, b, 0x12);
        }
        put(&mut f, 0, 0x30);
        put(&mut f, 0, 0xD0);
        assert_eq!(f.status(), sr::READY);
        assert!(f.nibbles().iter().all(|&n| n == 0xF));
    }

    #[test]
    fn lock_bits_with_wp_low() {
        let mut f = chip();
        // WP# high: set and clear work, and locks do not block.
        put(&mut f, 0, 0x60);
        put(&mut f, BLOCK_BYTES, 0x01);
        assert_eq!(f.lock_bits(), 0b10);
        put(&mut f, 0, 0x40);
        put(&mut f, BLOCK_BYTES, 0x00);
        assert_eq!(f.status(), sr::READY);
        // WP# low: locked block refuses program and erase.
        f.set_wp_low(true);
        put(&mut f, 0, 0x40);
        put(&mut f, BLOCK_BYTES + 1, 0x00);
        assert_eq!(f.status(), sr::READY | sr::PROTECTED | sr::PROGRAM_ERROR);
        put(&mut f, 0, 0x50);
        put(&mut f, 0, 0x20);
        put(&mut f, BLOCK_BYTES, 0xD0);
        assert_eq!(f.status(), sr::READY | sr::PROTECTED | sr::ERASE_ERROR);
        put(&mut f, 0, 0x50);
        // Full chip erase skips it.
        put(&mut f, 0, 0x30);
        put(&mut f, 0, 0xD0);
        put(&mut f, 0, 0xFF);
        assert_eq!(get(&f, BLOCK_BYTES), 0x00);
        assert_eq!(get(&f, BLOCK_BYTES + 1), 0xFF);
        // Clearing lock-bits needs WP# high.
        put(&mut f, 0, 0x60);
        put(&mut f, 0, 0xD0);
        assert_eq!(f.status(), sr::READY | sr::PROTECTED | sr::ERASE_ERROR);
        assert_eq!(f.lock_bits(), 0b10);
        put(&mut f, 0, 0x50);
        f.set_wp_low(false);
        put(&mut f, 0, 0x60);
        put(&mut f, 0, 0xD0);
        assert_eq!(f.lock_bits(), 0);
        assert_eq!(f.status(), sr::READY);
    }

    #[test]
    fn write_to_buffer() {
        let mut f = chip();
        let base = 5 * BLOCK_BYTES + 0x40;
        put(&mut f, base, 0xE8);
        assert_eq!(f.read_mode(), ReadMode::ExtendedStatus);
        assert_eq!(get(&f, base), 0x80, "XSR.7: buffer available");
        put(&mut f, base, 2); // three bytes
        for (i, v) in [0x11u8, 0x22, 0x33].into_iter().enumerate() {
            put(&mut f, base + i, v);
        }
        put(&mut f, base, 0xD0);
        assert_eq!(f.status(), sr::READY);
        put(&mut f, 0, 0xFF);
        assert_eq!(
            [get(&f, base), get(&f, base + 1), get(&f, base + 2)],
            [0x11, 0x22, 0x33]
        );
        assert_eq!(get(&f, base + 3), 0xFF);
        // A wrong confirm aborts with both error bits and programs nothing.
        put(&mut f, base, 0xE8);
        put(&mut f, base, 0);
        put(&mut f, base + 8, 0x00);
        put(&mut f, base, 0xFF);
        assert_eq!(f.status(), sr::READY | sr::ERASE_ERROR | sr::PROGRAM_ERROR);
        put(&mut f, 0, 0x50);
        put(&mut f, 0, 0xFF);
        assert_eq!(get(&f, base + 8), 0xFF);
        // Crossing a block boundary aborts too.
        put(&mut f, base, 0xE8);
        put(&mut f, base, 0);
        put(&mut f, 6 * BLOCK_BYTES, 0x00);
        put(&mut f, base, 0xD0);
        assert_eq!(f.status(), sr::READY | sr::ERASE_ERROR | sr::PROGRAM_ERROR);
        // Count over 32 bytes is an invalid sequence.
        put(&mut f, 0, 0x50);
        put(&mut f, base, 0xE8);
        put(&mut f, base, 32);
        assert_eq!(f.status(), sr::READY | sr::ERASE_ERROR | sr::PROGRAM_ERROR);
    }

    #[test]
    fn sts_config() {
        let mut f = chip();
        put(&mut f, 0, 0xB8);
        put(&mut f, 0, 0x03);
        assert_eq!(f.status(), sr::READY);
        put(&mut f, 0, 0xB8);
        put(&mut f, 0, 0x04);
        assert_eq!(f.status(), sr::READY | sr::ERASE_ERROR | sr::PROGRAM_ERROR);
    }

    #[test]
    fn reset_returns_to_read_array() {
        let mut f = chip();
        put(&mut f, 0, 0x20);
        put(&mut f, 0, 0x00);
        put(&mut f, 0, 0x40);
        f.reset();
        assert_eq!(f.read_mode(), ReadMode::Array);
        assert_eq!(f.status(), sr::READY);
        put(&mut f, 0, 0x00); // a command again, not program data
        assert_eq!(get(&f, 0), 0xFF);
    }

    #[test]
    fn latch_bits_follow_sousa() {
        // base + 2*n selects high bank n, base + #20*n low bank n.
        for n in 0..16u32 {
            assert_eq!(
                latch_banks(latch_from_address(0x3F000 + 2 * n)),
                (0, n as usize)
            );
        }
        for n in 0..4u32 {
            assert_eq!(
                latch_banks(latch_from_address(0x3F000 + 0x20 * n)),
                (n as usize, 0)
            );
        }
        // Both at once: #64 = A6..A1 110010 -> low 3, high 2.
        assert_eq!(latch_from_address(0x64), 0x32);
        assert_eq!(latch_banks(0x32), (3, 2));
        assert_eq!(latch_banks(latch_from_address(0x7F000 + 0x7E)), (3, 15));
    }

    #[test]
    fn banked_views() {
        let mut img = vec![0u8; FLASH_BYTES];
        for bank in 0..BANKS {
            let start = bank * BANK_NIBBLES / 2;
            img[start] = bank as u8; // first nibble of each bank = its number
            img[start + BANK_NIBBLES / 2 - 1] = 0xA0; // last nibble = #A
        }
        let f = Flash::from_packed(&img).unwrap();
        for bank in 0..BANKS {
            assert_eq!(f.bank(bank)[0], bank as u8);
            assert_eq!(f.bank(bank).len(), BANK_NIBBLES);
        }
        let latch = latch_from_address(0x40 + 0x18); // low 2, high 12
        assert_eq!(latch_banks(latch), (2, 12));
        assert_eq!(f.read_banked(0x00000, latch), 2);
        assert_eq!(f.read_banked(0x3FFFF, latch), 0xA);
        assert_eq!(f.read_banked(0x40000, latch), 12);
        assert_eq!(f.read_banked(0x7FFFF, latch), 0xA);
        // A19 is ignored: the views mirror at #80000.
        assert_eq!(f.read_banked(0x80000, latch), 2);
        assert_eq!(f.read_banked(0xC0000, latch), 12);
        assert_eq!(
            Flash::banked_address(0x40010, latch),
            12 * BANK_NIBBLES + 0x10
        );
        // The NCE3 write path always uses the high bank.
        assert_eq!(
            Flash::write_address(0x40010, latch),
            12 * BANK_NIBBLES + 0x10
        );
        assert_eq!(
            Flash::write_address(0x00010, latch),
            12 * BANK_NIBBLES + 0x10
        );
    }

    #[test]
    fn program_through_the_high_view() {
        let mut f = Flash::erased();
        f.set_write_enabled(true);
        // High bank 9: A1-A4 = 1001, so the latching access is at #12.
        let latch = latch_from_address(0x12);
        let (_, high) = latch_banks(latch);
        assert_eq!(high, 9);
        let cmd = |f: &mut Flash, addr: u32, v: u8| {
            f.write_banked(addr, latch, v & 0xF);
            f.write_banked(addr + 1, latch, v >> 4);
        };
        cmd(&mut f, 0x40000, 0x40);
        cmd(&mut f, 0x40100, 0x3C);
        cmd(&mut f, 0x40000, 0xFF);
        assert_eq!(f.bank(high)[0x100], 0xC);
        assert_eq!(f.bank(high)[0x101], 0x3);
        assert_eq!(f.read_banked(0x40100, latch), 0xC);
    }
}
