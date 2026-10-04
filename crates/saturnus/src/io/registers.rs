//! The HDW register window: 64 nibbles at #100-#13F (wiki: hardware/io-ram).
//!
//! Offsets in this module are relative to the window base (address − #100).

use super::timers::Timers;

/// Number of nibbles in the window.
const SIZE: usize = 64;

const DISPLAY_CTRL: usize = 0x00;
const CRC_BASE: usize = 0x04;
const BATTERY: usize = 0x08;
const ANNUNC_LO: usize = 0x0B;
const ANNUNC_HI: usize = 0x0C;
const CARD_STATUS: usize = 0x0F;
const UART_TX_STATUS: usize = 0x12;
const UART_RX_LO: usize = 0x14;
const UART_RX_HI: usize = 0x15;
const SERVICE_REQ: usize = 0x18;
const KDN_REG: usize = 0x19;
const DISPLAY_START: usize = 0x20;
const LINE_OFFSET: usize = 0x25;
const LINE_COUNT_LO: usize = 0x28;
const LINE_COUNT_HI: usize = 0x29;
const T1_CTRL: usize = 0x2E;
const T2_CTRL: usize = 0x2F;
const MENU_START: usize = 0x30;
const TIMER1: usize = 0x37;
const TIMER2: usize = 0x38;

/// UART transmit status bits 0-1 (buffer full, transmitting).
const UART_TX_BUSY_BITS: u8 = 0x3;
/// KDN (key down) bit in #119.
const KDN_BIT: u8 = 0x8;

/// State behind the HDW register window, including the timers and the
/// CRC generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IoRegisters {
    /// Plain storage for registers without special behaviour.
    regs: [u8; SIZE],
    /// TIMER1 and TIMER2 (#12E/#12F control, #137 and #138-#13F values).
    pub timers: Timers,
    /// CRC accumulator (#104-#107).
    crc: u16,
    /// Free-running 8192-Hz tick count, drives the display row counter.
    ticks: u64,
    /// Last written 6-bit line count (#128-#129).
    line_count: u8,
    /// Key-down flag (KDN, #119 bit 3).
    kdn: bool,
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
            crc: 0,
            ticks: 0,
            line_count: 0,
            kdn: false,
        }
    }

    /// CPU read of the nibble at `offset` (masked to 0x3F).
    pub fn read(&mut self, offset: u32) -> u8 {
        self.peek(offset)
    }

    /// The nibble a CPU read at `offset` would return, without side effects.
    pub fn peek(&self, offset: u32) -> u8 {
        let off = (offset & 0x3F) as usize;
        match off {
            CRC_BASE..=0x07 => ((self.crc >> ((off - CRC_BASE) * 4)) & 0xF) as u8,
            // Batteries good, no cards fitted, receive buffer empty, no
            // service requests modelled yet.
            BATTERY | CARD_STATUS | UART_RX_LO | UART_RX_HI | SERVICE_REQ => 0,
            // Transmit is instantaneous until the UART is modelled (inferred).
            UART_TX_STATUS => self.regs[off] & !UART_TX_BUSY_BITS,
            KDN_REG => {
                if self.kdn {
                    KDN_BIT
                } else {
                    0
                }
            }
            LINE_COUNT_LO => self.current_row() & 0xF,
            LINE_COUNT_HI => ((self.current_row() >> 4) & 0x3) | (self.regs[off] & 0xC),
            T1_CTRL => self.timers.read_t1_ctrl(),
            T2_CTRL => self.timers.read_t2_ctrl(),
            TIMER1 => self.timers.t1,
            TIMER2..=0x3F => ((self.timers.t2 >> ((off - TIMER2) * 4)) & 0xF) as u8,
            _ => self.regs[off],
        }
    }

    /// CPU write of `nibble` at `offset` (masked to 0x3F).
    pub fn write(&mut self, offset: u32, nibble: u8) {
        let off = (offset & 0x3F) as usize;
        let v = nibble & 0xF;
        match off {
            CRC_BASE..=0x07 => {
                let shift = (off - CRC_BASE) * 4;
                self.crc = (self.crc & !(0xF << shift)) | (u16::from(v) << shift);
            }
            BATTERY | CARD_STATUS | UART_RX_LO | UART_RX_HI | SERVICE_REQ | KDN_REG => {}
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

    /// Advance the timers and the free-running tick count by `ticks`
    /// 8192-Hz ticks.
    pub fn tick(&mut self, ticks: u32) {
        self.timers.tick(ticks);
        self.ticks = self.ticks.wrapping_add(u64::from(ticks));
    }

    /// Display row currently being refreshed: one row per 4096-Hz tick,
    /// counting down from 63 (wiki: hardware/display).
    fn current_row(&self) -> u8 {
        63 - ((self.ticks / 2) % 64) as u8
    }

    /// Current CRC accumulator.
    pub fn crc(&self) -> u16 {
        self.crc
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
        self.regs[DISPLAY_CTRL] & 0x8 != 0
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

    /// Start address of the menu bitmap (#130-#134, bit 0 cleared).
    pub fn menu_start(&self) -> u32 {
        self.nibbles_le(MENU_START, 5) & !1
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
        assert_eq!(io.read(0x12), 0xC);
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
