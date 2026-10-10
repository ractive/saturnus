//! The HDW register window: 64 nibbles at #100-#13F (wiki: hardware/io-ram).
//!
//! Offsets in this module are relative to the window base (address − #100).

use super::timers::Timers;
use super::uart::{self, Uart};

/// Number of nibbles in the window.
const SIZE: usize = 64;

const DISPLAY_CTRL: usize = 0x00;
const CONTRAST_LO: usize = 0x01;
const CONTRAST_HI: usize = 0x02;
const CRC_BASE: usize = 0x04;
const BATTERY: usize = 0x08;
const ANNUNC_LO: usize = 0x0B;
const ANNUNC_HI: usize = 0x0C;
const CARD_CTRL: usize = 0x0E;
const CARD_STATUS: usize = 0x0F;
const KDN_REG: usize = 0x19;
/// LCR, IR/LED control (#11C).
pub(crate) const LCR: usize = 0x1C;
const DISPLAY_START: usize = 0x20;
const LINE_OFFSET: usize = 0x25;
const LINE_COUNT_LO: usize = 0x28;
const LINE_COUNT_HI: usize = 0x29;
const T1_CTRL: usize = 0x2E;
const T2_CTRL: usize = 0x2F;
const MENU_START: usize = 0x30;
const TIMER1: usize = 0x37;
const TIMER2: usize = 0x38;

/// KDN (key down) bit in #119.
const KDN_BIT: u8 = 0x8;
/// DON, display enable (#100 bit 3).
const DON_BIT: u8 = 0x8;
/// DA19, #129 bit 3 (wiki: hardware/io-ram, questions/da19-polarity).
const DA19_BIT: u8 = 0x8;
/// Rows of the display, refreshed bottom-up by the row counter.
const ROWS: u8 = 64;
/// Timer ticks (8192 Hz) per display row (4096 Hz).
const TICKS_PER_ROW: u8 = 2;
/// CARDCTL (#10E) bit 1: SMP, "set module pulled". While set the card
/// detect logic holds NINT low (wiki: hardware/card-ports,
/// questions/register-10e-role).
pub const CARD_SMP: u8 = 0x2;
/// CARDCTL (#10E) bit 3: ECDT, enable card detect.
pub const CARD_ECDT: u8 = 0x8;
// CARDSTAT (#10F) pairs its bits with the chip selects: bits 1 and 3
// belong to the card on CE2, bits 0 and 2 to the other slot (CE1 on the
// 48SX, NCE3 on the 48GX). The 48SX ROM J uses them this way (code at
// #09A18-#09A63, traced on saturnng and saturnus), which makes Mastracci
// 4.3's port names right for the GX, where CE2 is port 1, and wrong for
// the SX (wiki: hardware/card-ports).
/// CARDSTAT (#10F) bit 0: card present in the non-CE2 slot (48SX CE1
/// port 1, 48GX NCE3 port 2).
pub const CARD_OTHER_PRESENT: u8 = 0x1;
/// CARDSTAT (#10F) bit 1: card present on CE2 (48SX port 2, 48GX port 1).
pub const CARD_CE2_PRESENT: u8 = 0x2;
/// CARDSTAT (#10F) bit 2: writes allowed in the non-CE2 slot.
pub const CARD_OTHER_WRITE: u8 = 0x4;
/// CARDSTAT (#10F) bit 3: writes allowed on CE2.
pub const CARD_CE2_WRITE: u8 = 0x8;

/// Whether window offset `off` belongs to the UART: BAU #10D and
/// IOC-SRQ1 #110-#118. #119 (KDN) and the IR registers #11A-#11D are not
/// UART registers here; the IR ones are plain storage (wiki:
/// hardware/uart "IR": IR is not modelled).
fn is_uart(off: usize) -> bool {
    off == uart::BAU || (uart::IOC..=uart::SRQ1).contains(&off)
}

/// State behind the HDW register window, including the timers and the
/// CRC generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IoRegisters {
    /// Plain storage for registers without special behaviour.
    pub(crate) regs: [u8; SIZE],
    /// TIMER1 and TIMER2 (#12E/#12F control, #137 and #138-#13F values).
    pub timers: Timers,
    /// The UART (#10D, #110-#118) and the serial wire.
    pub uart: Uart,
    /// CRC accumulator (#104-#107).
    pub(crate) crc: u16,
    /// Display row being refreshed, 63 (top) down to 0 (bottom).
    pub(crate) row: u8,
    /// 8192-Hz ticks into the current row (0 or 1).
    pub(crate) row_phase: u8,
    /// Last written 6-bit line count (#128-#129).
    pub(crate) line_count: u8,
    /// Key-down flag (KDN, #119 bit 3).
    pub(crate) kdn: bool,
    /// Card status as the card-detect pins report it, in the #10F bit
    /// layout; kept up to date by the machine when cards change.
    pub(crate) card_status: u8,
    /// A rising edge of SMP (#10E bit 1) waits for delivery as an
    /// interrupt.
    pub(crate) card_edge: bool,
}

impl Default for IoRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl IoRegisters {
    /// Power-on state: every register 0.
    pub fn new() -> Self {
        Self {
            regs: [0; SIZE],
            timers: Timers::new(),
            uart: Uart::new(),
            crc: 0,
            row: ROWS - 1,
            row_phase: 0,
            line_count: 0,
            kdn: false,
            card_status: 0,
            card_edge: false,
        }
    }

    /// CPU read of the nibble at `offset` (masked to 0x3F).
    pub fn read(&mut self, offset: u32) -> u8 {
        let off = (offset & 0x3F) as usize;
        if is_uart(off) {
            return self.uart.read(off);
        }
        self.peek(offset)
    }

    /// The nibble a CPU read at `offset` would return, without side effects.
    pub fn peek(&self, offset: u32) -> u8 {
        let off = (offset & 0x3F) as usize;
        match off {
            CRC_BASE..=0x07 => ((self.crc >> ((off - CRC_BASE) * 4)) & 0xF) as u8,
            // Batteries good.
            BATTERY => 0,
            off if is_uart(off) => self.uart.peek(off),
            // CARDSTAT reads 0 while card detection is disabled (wiki:
            // emulators/emu48 SP16/SP19, hardware/card-ports).
            CARD_STATUS => {
                if self.regs[CARD_CTRL] & CARD_ECDT != 0 {
                    self.card_status
                } else {
                    0
                }
            }
            KDN_REG => {
                if self.kdn {
                    KDN_BIT
                } else {
                    0
                }
            }
            LINE_COUNT_LO => self.row & 0xF,
            LINE_COUNT_HI => ((self.row >> 4) & 0x3) | (self.regs[off] & 0xC),
            T1_CTRL => self.timers.read_t1_ctrl(),
            T2_CTRL => self.timers.read_t2_ctrl(),
            TIMER1 => self.timers.t1,
            TIMER2..=0x3F => self.timers.read_t2_nibble((off - TIMER2) as u8),
            _ => self.regs[off],
        }
    }

    /// CPU write of `nibble` at `offset` (masked to 0x3F).
    pub fn write(&mut self, offset: u32, nibble: u8) {
        let off = (offset & 0x3F) as usize;
        let v = nibble & 0xF;
        match off {
            CRC_BASE..=0x07 => self.set_crc_nibble(off - CRC_BASE, v),
            BATTERY | CARD_STATUS | KDN_REG => {}
            off if is_uart(off) => self.uart.write(off, v),
            DISPLAY_CTRL => {
                // Switching the display on restarts the row counter from
                // the LINECOUNT value (wiki: hardware/display "Emu48
                // findings", emulators/emu48 SP30).
                if self.regs[off] & DON_BIT == 0 && v & DON_BIT != 0 {
                    self.row = self.line_count;
                    self.row_phase = 0;
                }
                self.regs[off] = v;
            }
            CARD_CTRL => {
                if self.regs[off] & CARD_SMP == 0 && v & CARD_SMP != 0 {
                    self.card_edge = true;
                }
                self.regs[off] = v;
            }
            LINE_COUNT_LO | LINE_COUNT_HI => {
                self.regs[off] = v;
                self.line_count =
                    (self.regs[LINE_COUNT_LO] | (self.regs[LINE_COUNT_HI] << 4)) & 0x3F;
            }
            T1_CTRL => self.timers.write_t1_ctrl(v),
            T2_CTRL => self.timers.write_t2_ctrl(v),
            TIMER1 => self.timers.write_t1(v),
            TIMER2..=0x3F => self.timers.write_t2_nibble((off - TIMER2) as u8, v),
            _ => self.regs[off] = v,
        }
    }

    /// Advance the timers and the display row counter by `ticks` 8192-Hz
    /// ticks.
    ///
    /// The row counter moves one row per 4096-Hz tick, counting down from
    /// 63 to 0 and wrapping to 63: 64 rows, 64 frames per second (wiki:
    /// hardware/display). It keeps counting while DON is clear; the
    /// sources only say that nothing is drawn then, and a free-running
    /// counter cannot hang a ROM loop that waits for a row (inferred).
    pub fn tick(&mut self, ticks: u32) {
        self.timers.tick(ticks);
        let total = u64::from(self.row_phase) + u64::from(ticks);
        self.row_phase = (total % u64::from(TICKS_PER_ROW)) as u8;
        let rows = (total / u64::from(TICKS_PER_ROW)) % u64::from(ROWS);
        self.row = ((u64::from(self.row) + u64::from(ROWS) - rows) % u64::from(ROWS)) as u8;
    }

    /// Display row currently being refreshed, 63 (top) down to 0.
    #[cfg(test)]
    pub fn current_row(&self) -> u8 {
        self.row
    }

    /// Current CRC accumulator.
    pub fn crc(&self) -> u16 {
        self.crc
    }

    /// Write nibble `i` (0-3, low first) of the CRC accumulator.
    pub fn set_crc_nibble(&mut self, i: usize, v: u8) {
        let shift = (i & 3) * 4;
        self.crc = (self.crc & !(0xF << shift)) | (u16::from(v & 0xF) << shift);
    }

    /// Feed one nibble read from memory into the CRC (wiki: hardware/crc).
    pub fn crc_update(&mut self, nibble: u8) {
        let n = u16::from(nibble & 0xF);
        self.crc = (self.crc >> 4) ^ (((self.crc ^ n) & 0xF) * 0x1081);
    }

    /// Set the KDN flag (#119 bit 3).
    pub fn set_key_down(&mut self, down: bool) {
        self.kdn = down;
    }

    fn nibbles_le(&self, base: usize, count: usize) -> u32 {
        (0..count).fold(0, |acc, i| {
            acc | (u32::from(self.regs[base + i]) << (4 * i))
        })
    }

    /// Display enabled (#100 bit 3).
    pub fn display_on(&self) -> bool {
        self.regs[DISPLAY_CTRL] & DON_BIT != 0
    }

    /// 5-bit LCD contrast: #101 bits 0-3 and #102 bit 0 as bit 4; higher is
    /// darker (wiki: hardware/display "Bit names").
    pub fn contrast(&self) -> u8 {
        self.regs[CONTRAST_LO] | ((self.regs[CONTRAST_HI] & 1) << 4)
    }

    /// Set the card-detect pin state (#10F bit layout) after a card was
    /// inserted or removed. With card detection enabled (#10E bit 3) a
    /// change sets SMP (#10E bit 1), which pulls NINT low and so sets
    /// HST.MP and interrupts until the ROM clears SMP (wiki:
    /// emulators/emu48 SP16/SP19 "a card change sets MP and pulls NINT
    /// low"; questions/register-10e-role, Duchesne: the ROM writes #C to
    /// #10E until MP stays clear). Latching SMP is inferred.
    pub fn set_card_status(&mut self, status: u8) {
        let status = status & 0xF;
        if status != self.card_status && self.regs[CARD_CTRL] & CARD_ECDT != 0 {
            if self.regs[CARD_CTRL] & CARD_SMP == 0 {
                self.card_edge = true;
            }
            self.regs[CARD_CTRL] |= CARD_SMP;
        }
        self.card_status = status;
    }

    /// Whether the card-detect logic holds NINT low (SMP, #10E bit 1).
    pub fn module_pulled(&self) -> bool {
        self.regs[CARD_CTRL] & CARD_SMP != 0
    }

    /// Return and clear a pending card-detect interrupt edge.
    pub fn take_card_interrupt(&mut self) -> bool {
        std::mem::take(&mut self.card_edge)
    }

    /// Horizontal pixel offset of the display (#100 bits 0-2).
    pub fn bit_offset(&self) -> u8 {
        self.regs[DISPLAY_CTRL] & 0x7
    }

    /// Start address of the main display bitmap (#120-#124, bit 0 cleared).
    pub fn display_start(&self) -> u32 {
        self.nibbles_le(DISPLAY_START, 5) & !1
    }

    /// Nibbles added after each display row (#125-#127, 12-bit signed,
    /// bit 0 cleared).
    pub fn line_offset(&self) -> i32 {
        let raw = self.nibbles_le(LINE_OFFSET, 3) & !1;
        if raw & 0x800 != 0 {
            raw as i32 - 0x1000
        } else {
            raw as i32
        }
    }

    /// Last written 6-bit line count (#128-#129).
    pub fn line_count(&self) -> u8 {
        self.line_count
    }

    /// DA19, #129 bit 3 as last written (reads of #129 return it too).
    /// On the 48GX, 1 gives ROM address line A19 to the ROM and 0 hands
    /// the shared pin to NCE3 (wiki: questions/da19-polarity). Storage
    /// only on the other models.
    pub fn da19(&self) -> bool {
        self.regs[LINE_COUNT_HI] & DA19_BIT != 0
    }

    /// Start address of the menu bitmap (#130-#134, bit 0 cleared).
    pub fn menu_start(&self) -> u32 {
        self.nibbles_le(MENU_START, 5) & !1
    }

    /// LCR (#11C): IR/LED control as last written. Bit 3 is the LED
    /// enable on the 48 and the flash write enable on the 49G (wiki:
    /// hardware/uart, hardware/hp49g).
    pub fn lcr(&self) -> u8 {
        self.regs[LCR]
    }

    /// Annunciator byte: #10B in bits 0-3, #10C in bits 4-7.
    pub fn annunciators(&self) -> u8 {
        self.regs[ANNUNC_LO] | (self.regs[ANNUNC_HI] << 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::timers::{CTRL_INT, CTRL_XTRA_OR_RUN};

    #[test]
    fn crc_formula() {
        let mut io = IoRegisters::new();
        for n in [1u8, 2, 3] {
            io.crc_update(n);
        }
        // By hand: 0 -> 1*0x1081 = 0x1081
        // 0x1081: (0x108) ^ ((1^2)&F = 3)*0x1081 = 0x108 ^ 0x3183 = 0x308B
        // 0x308B: (0x308) ^ ((B^3)&F = 8)*0x1081 = 0x308 ^ 0x8408 = 0x8700
        assert_eq!(io.crc(), 0x8700);
        assert_eq!(io.read(0x04), 0x0);
        assert_eq!(io.read(0x05), 0x0);
        assert_eq!(io.read(0x06), 0x7);
        assert_eq!(io.read(0x07), 0x8);
        io.write(0x05, 0xA);
        assert_eq!(io.crc(), 0x87A0);
    }

    #[test]
    fn row_counter() {
        let mut io = IoRegisters::new();
        assert_eq!(io.read(0x28), 0xF);
        assert_eq!(io.read(0x29) & 0x3, 0x3);
        io.tick(1);
        assert_eq!(io.read(0x28), 0xF);
        io.tick(1);
        assert_eq!(io.read(0x28), 0xE);
        // 63 - 32 = 31 after 64 ticks.
        io.tick(62);
        assert_eq!(io.read(0x28), 0xF);
        assert_eq!(io.read(0x29) & 0x3, 0x1);
        // Written bits 2-3 of #129 read back; line count latched.
        io.write(0x28, 0x5);
        io.write(0x29, 0xE);
        assert_eq!(io.read(0x29) & 0xC, 0xC);
        assert_eq!(io.line_count(), 0x25);
    }

    #[test]
    fn row_counter_wraps_and_restarts_on_display_on() {
        let mut io = IoRegisters::new();
        // 63 -> 0 takes 63 rows, one more wraps to 63.
        io.tick(2 * 63);
        assert_eq!(io.current_row(), 0);
        io.tick(2);
        assert_eq!(io.current_row(), 63);
        // A huge batch: 2^32-1 ticks = 2^31-1 rows (+1 phase) = 63 rows mod 64.
        io.tick(u32::MAX);
        assert_eq!(io.current_row(), 0);
        assert_eq!(io.row_phase, 1);
        // LINECOUNT #37, then DON: the counter restarts at 55.
        io.write(0x28, 0x7);
        io.write(0x29, 0xB);
        io.write(0x00, 0x8);
        assert_eq!(io.current_row(), 0x37);
        assert_eq!(io.read(0x28), 0x7);
        // M32/DA19 (#129 bits 2-3) read back with the row's top bits.
        assert_eq!(io.read(0x29), 0x8 | 0x3);
        assert!(io.da19());
        io.write(0x29, 0x3);
        assert!(!io.da19());
        io.write(0x29, 0xB);
        io.tick(2);
        assert_eq!(io.current_row(), 0x36);
        // Writing #100 again with DON already set does not restart.
        io.write(0x00, 0x9);
        assert_eq!(io.current_row(), 0x36);
    }

    #[test]
    fn contrast_five_bits() {
        let mut io = IoRegisters::new();
        assert_eq!(io.contrast(), 0);
        io.write(0x01, 0xB);
        assert_eq!(io.contrast(), 11);
        io.write(0x02, 0x3);
        assert_eq!(io.contrast(), 0x1B);
        assert_eq!(io.read(0x02), 0x3);
    }

    #[test]
    fn card_status_needs_card_detect_enabled() {
        let mut io = IoRegisters::new();
        io.set_card_status(CARD_OTHER_PRESENT | CARD_OTHER_WRITE);
        assert_eq!(io.read(0x0F), 0, "detection disabled reads 0");
        assert!(!io.module_pulled());
        assert!(!io.take_card_interrupt());
        io.write(0x0E, 0xC);
        assert_eq!(io.read(0x0F), CARD_OTHER_PRESENT | CARD_OTHER_WRITE);
        // A change with detection on latches SMP and one edge.
        io.set_card_status(0);
        assert!(io.module_pulled());
        assert_eq!(io.read(0x0E), 0xE);
        assert!(io.take_card_interrupt());
        assert!(!io.take_card_interrupt());
        io.set_card_status(CARD_CE2_PRESENT);
        assert!(!io.take_card_interrupt(), "SMP already set: no new edge");
        // The ROM writes #C to clear SMP.
        io.write(0x0E, 0xC);
        assert!(!io.module_pulled());
        assert_eq!(io.read(0x0F), CARD_CE2_PRESENT);
        // Software can set SMP too.
        io.write(0x0E, 0xE);
        assert!(io.take_card_interrupt());
    }

    #[test]
    fn timer_registers() {
        let mut io = IoRegisters::new();
        io.write(0x37, 0x9);
        assert_eq!(io.read(0x37), 0x9);
        io.write(0x38, 0x2);
        io.write(0x3F, 0x1);
        assert_eq!(io.timers.t2, 0x1000_0002);
        io.write(0x2F, CTRL_XTRA_OR_RUN | CTRL_INT | 0x8);
        assert_eq!(io.read(0x2F), CTRL_XTRA_OR_RUN | CTRL_INT);
        io.tick(3);
        assert_eq!(io.read(0x38), 0xF);
        assert_eq!(io.read(0x2F), CTRL_XTRA_OR_RUN | CTRL_INT);
    }

    #[test]
    fn fixed_status_registers() {
        let mut io = IoRegisters::new();
        io.write(0x08, 0xF);
        io.write(0x0F, 0xF);
        io.write(0x12, 0xF);
        assert_eq!(io.read(0x08), 0);
        assert_eq!(io.read(0x0F), 0);
        assert_eq!(io.read(0x12), 0, "TCS writes need SON");
        assert_eq!(io.read(0x19), 0);
        io.set_key_down(true);
        assert_eq!(io.read(0x19), 0x8);
    }

    #[test]
    fn display_accessors() {
        let mut io = IoRegisters::new();
        io.write(0x00, 0xB);
        for (i, n) in [0x1, 0x2, 0x3, 0x4, 0x5].iter().enumerate() {
            io.write(0x20 + i as u32, *n);
        }
        io.write(0x25, 0xF);
        io.write(0x26, 0xF);
        io.write(0x27, 0xF);
        io.write(0x0B, 0x3);
        io.write(0x0C, 0x8);
        assert!(io.display_on());
        assert_eq!(io.bit_offset(), 3);
        assert_eq!(io.display_start(), 0x54320);
        assert_eq!(io.line_offset(), -2);
        assert_eq!(io.annunciators(), 0x83);
    }
}
