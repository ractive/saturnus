//! The device on NCE1, the chip select with no controller: it answers
//! every address no configured chip claims (wiki:
//! hardware/memory-controller "Controller model (Giesselink)").
//!
//! Extension point: the 48SX and 48GX have mask ROM here; the 49G has a
//! 2 MB flash whose upper address lines come from the CE1 bank latch
//! (wiki: hardware/hp49g). A new device becomes a new variant; `read`
//! receives the latch so a banked device can pick its bank, and
//! [`Nce1::nibbles`] feeds the checksum that binds saved states to the
//! system image.

use super::Rom;
use super::flash::{Flash, ReadMode};

/// What NCE1 drives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nce1 {
    /// Mask ROM read straight through the address lines. The caller has
    /// already applied model wiring such as the 48GX's DA19 (see
    /// `Hardware`); reads mirror modulo the ROM length.
    Rom(Rom),
    /// The 49G's 2 MB flash, read through the CE1 bank latch: the low
    /// view at #00000-#3FFFF, the high view at #40000-#7FFFF, both
    /// mirrored at #80000 (wiki: hardware/hp49g; see
    /// [`Flash::read_banked`]).
    Flash(Box<Flash>),
}

/// Size of the fixed part of a flash state blob: lock-bits u32, status
/// u8, read mode u8, WP# u8.
const FLASH_BLOB_HEADER: usize = 7;

impl Nce1 {
    /// The nibble at `addr` (20 bits, after any DA19 masking). `latch` is
    /// the CE1 bank latch (nibble address bits A1-A6 of the last latching
    /// read), unused by plain ROM.
    pub fn read(&self, addr: u32, latch: u8) -> u8 {
        let _ = latch;
        match self {
            Nce1::Rom(rom) => rom.read(addr),
            Nce1::Flash(f) => f.read_banked(addr, latch),
        }
    }

    /// The device contents, one nibble per element. `Machine` checksums
    /// them once, at construction, so a saved state is bound to the image
    /// that was loaded, not to contents a flash later programs.
    pub fn nibbles(&self) -> &[u8] {
        match self {
            Nce1::Rom(rom) => rom.as_slice(),
            Nce1::Flash(f) => f.nibbles(),
        }
    }

    /// Power-on / CPU reset of the device: the flash's command interface
    /// returns to read array with its write gate closed (datasheet 3.4;
    /// #11C is cleared by the reset too). ROM has nothing to reset.
    pub fn reset(&mut self) {
        if let Nce1::Flash(f) = self {
            f.reset();
            f.set_write_enabled(false);
        }
    }

    /// The flash chip, on the 49G.
    pub fn flash(&self) -> Option<&Flash> {
        match self {
            Nce1::Flash(f) => Some(f),
            Nce1::Rom(_) => None,
        }
    }

    /// The write-enable line driven by #11C bit 3 (the 49G's flash write
    /// enable; wiki: hardware/hp49g "Controllers"). `Hardware` forwards
    /// every #11C write here. ROM ignores it.
    pub fn set_write_enabled(&mut self, enabled: bool) {
        match self {
            Nce1::Rom(_) => {
                let _ = enabled;
            }
            Nce1::Flash(f) => f.set_write_enabled(enabled),
        }
    }

    /// A read that the model routes from NCE3 to this device (the 49G's
    /// flash write path while write-enabled), or `None` when the device
    /// does not take it and NCE3's own memory answers.
    pub fn nce3_read(&self, addr: u32, latch: u8) -> Option<u8> {
        match self {
            Nce1::Rom(_) => {
                let _ = (addr, latch);
                None
            }
            Nce1::Flash(f) => Some(f.read(Flash::write_address(addr, latch))),
        }
    }

    /// A write that the model routes from NCE3 to this device; returns
    /// whether the device took it (otherwise NCE3's own memory does).
    pub fn nce3_write(&mut self, addr: u32, latch: u8, nibble: u8) -> bool {
        match self {
            Nce1::Rom(_) => {
                let _ = (addr, latch, nibble);
                false
            }
            Nce1::Flash(f) => {
                f.write_banked(addr, latch, nibble);
                true
            }
        }
    }

    /// Mutable device state for a saved state (e.g. programmed flash and
    /// lock bits). Empty for ROM.
    pub fn state_blob(&self) -> Vec<u8> {
        match self {
            Nce1::Rom(_) => Vec::new(),
            Nce1::Flash(f) => {
                let mut b = Vec::with_capacity(FLASH_BLOB_HEADER + super::flash::FLASH_BYTES);
                b.extend_from_slice(&f.lock_bits().to_le_bytes());
                b.push(f.status());
                b.push(mode_code(f.read_mode()));
                b.push(u8::from(f.wp_low()));
                b.extend(f.to_packed());
                b
            }
        }
    }

    /// Restore [`Nce1::state_blob`] output; an error names what is wrong.
    pub fn load_state_blob(&mut self, blob: &[u8]) -> Result<(), &'static str> {
        match self {
            Nce1::Rom(_) if blob.is_empty() => Ok(()),
            Nce1::Rom(_) => Err("ROM takes no NCE1 state"),
            Nce1::Flash(f) => {
                if blob.len() != FLASH_BLOB_HEADER + super::flash::FLASH_BYTES {
                    return Err("flash state has the wrong size");
                }
                let (head, image) = blob.split_at(FLASH_BLOB_HEADER);
                let mode = mode_from_code(head[5]).ok_or("unknown flash read mode")?;
                let wp_low = match head[6] {
                    0 => false,
                    1 => true,
                    _ => return Err("flash WP# not 0 or 1"),
                };
                let mut chip = Flash::from_packed(image).map_err(|_| "flash image size")?;
                chip.set_lock_bits(u32::from_le_bytes([head[0], head[1], head[2], head[3]]));
                chip.restore_cui(head[4], mode);
                chip.set_wp_low(wp_low);
                chip.set_write_enabled(f.write_enabled());
                **f = chip;
                Ok(())
            }
        }
    }
}

fn mode_code(m: ReadMode) -> u8 {
    match m {
        ReadMode::Array => 0,
        ReadMode::Identifier => 1,
        ReadMode::Query => 2,
        ReadMode::Status => 3,
        ReadMode::ExtendedStatus => 4,
    }
}

fn mode_from_code(c: u8) -> Option<ReadMode> {
    Some(match c {
        0 => ReadMode::Array,
        1 => ReadMode::Identifier,
        2 => ReadMode::Query,
        3 => ReadMode::Status,
        4 => ReadMode::ExtendedStatus,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_reads_through() {
        let n = Nce1::Rom(Rom::from_packed(&[0x21, 0x43]));
        assert_eq!(n.read(2, 0x3F), 3);
        assert_eq!(n.read(5, 0), 2);
        assert_eq!(n.nibbles(), &[1, 2, 3, 4]);
        let mut n = n;
        n.set_write_enabled(true);
        assert_eq!(n.nce3_read(0, 0), None);
        assert!(!n.nce3_write(0, 0, 5));
        assert!(n.state_blob().is_empty());
        assert!(n.load_state_blob(&[]).is_ok());
        assert!(n.load_state_blob(&[1]).is_err());
    }

    #[test]
    fn flash_reads_banked_and_round_trips_state() {
        let mut img = vec![0u8; super::super::flash::FLASH_BYTES];
        img[3 * 0x2_0000] = 0x0C; // bank 3 starts with nibble C
        let mut n = Nce1::Flash(Box::new(Flash::from_packed(&img).unwrap()));
        // Latch bits A5-A6 = 3: low view shows bank 3.
        assert_eq!(n.read(0, 0x30), 0xC);
        assert_eq!(n.read(0x4_0000, 0x30), 0);
        // Latch bits A1-A4 = 3: high view shows bank 3.
        assert_eq!(n.read(0x4_0000, 0x03), 0xC);
        // Writes need the gate; NCE3 writes reach the high bank.
        assert!(n.nce3_write(0, 0, 0x0));
        n.set_write_enabled(true);
        for (a, v) in [(0, 0x0), (1, 0x4), (0x10, 0x0), (0x11, 0x0)] {
            assert!(n.nce3_write(a, 0, v)); // #40 then #00 at byte 8 of bank 0
        }
        assert_eq!(
            n.nce3_read(0x10, 0),
            Some(0x0),
            "status mode reads #80 low nibble"
        );
        assert_eq!(n.nce3_read(0x11, 0), Some(0x8));
        let blob = n.state_blob();
        let mut m = Nce1::Flash(Box::new(Flash::erased()));
        m.load_state_blob(&blob).unwrap();
        assert_eq!(m.flash().unwrap().nibbles()[0x10], 0);
        assert_eq!(m.flash().unwrap().read_mode(), ReadMode::Status);
        assert!(m.load_state_blob(&blob[1..]).is_err());
        n.reset();
        assert_eq!(n.flash().unwrap().read_mode(), ReadMode::Array);
        assert!(!n.flash().unwrap().write_enabled());
    }
}
