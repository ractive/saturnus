//! The Lewis chip's display and register block of the HP 42S: 1024 nibbles
//! at #40000-#403FF (wiki: hardware/lewis).
//!
//! Offsets in this module are relative to #40000. The block holds the
//! display RAM (#000-#2FF) and the control registers (#300-#3FF). The
//! register numbering follows the 48's HDW window where the two chips share
//! a function: contrast at #301, CRC at #304-#307, power status at
//! #308-#309, the timer controls at #30E/#30F and the timers at #3F7 and
//! #3F8-#3FF, the 48's #12E/#12F/#137/#138 moved (wiki: hardware/lewis
//! "Registers"; the timer-control roles are inferred from how the 42S ROM
//! uses them). The timers and the CRC generator are the ones
//! in [`IoRegisters`], so the machine's time keeping serves both chips.

use super::IoRegisters;

/// Nibbles in the block.
pub const SIZE: usize = 0x400;
/// First register offset; below it is display RAM. #300 is RATE, the CPU
/// speed register, kept as storage:
/// the clock is fixed at [`crate::Model::clock_hz`] (wiki: questions/
/// lewis-clock-and-rate).
const REGS: usize = 0x300;
/// Contrast bits 0-3.
const CONTRAST: usize = 0x301;
/// DSPCTL: bit 1 contrast bit 4, bit 3 DON.
const DSPCTL: usize = 0x303;
/// DON, display on (DSPCTL bit 3).
const DON_BIT: u8 = 0x8;
/// CRC accumulator, low nibble first.
const CRC_BASE: usize = 0x304;
/// LPD, low-power detection: reads 0 (batteries good).
const LPD: usize = 0x308;
/// TIMER1 control (the 48's #12E).
const T1_CTRL: usize = 0x30E;
/// TIMER2 control (the 48's #12F).
const T2_CTRL: usize = 0x30F;
/// TIMER1, 4 bits (the 48's #137).
const TIMER1: usize = 0x3F7;
/// TIMER2, 8 nibbles, low first (the 48's #138-#13F).
const TIMER2: usize = 0x3F8;

/// Display RAM and plain register storage of the Lewis block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LewisIo {
    /// Every nibble of the block that has no special behaviour.
    pub(crate) mem: Vec<u8>,
}

impl Default for LewisIo {
    fn default() -> Self {
        Self::new()
    }
}

impl LewisIo {
    /// Power-on state: every nibble 0.
    pub fn new() -> Self {
        Self { mem: vec![0; SIZE] }
    }

    /// The nibble a CPU read at `offset` returns, without side effects;
    /// `io` supplies the timers and the CRC.
    pub fn peek(&self, io: &IoRegisters, offset: u32) -> u8 {
        let off = offset as usize % SIZE;
        match off {
            CRC_BASE..=0x307 => ((io.crc() >> ((off - CRC_BASE) * 4)) & 0xF) as u8,
            LPD => 0,
            T1_CTRL => io.timers.read_t1_ctrl(),
            T2_CTRL => io.timers.read_t2_ctrl(),
            TIMER1 => io.timers.t1,
            TIMER2..=0x3FF => io.timers.read_t2_nibble((off - TIMER2) as u8),
            _ => self.mem[off],
        }
    }

    /// CPU write of `nibble` at `offset`.
    pub fn write(&mut self, io: &mut IoRegisters, offset: u32, nibble: u8) {
        let off = offset as usize % SIZE;
        let v = nibble & 0xF;
        match off {
            CRC_BASE..=0x307 => io.set_crc_nibble(off - CRC_BASE, v),
            LPD => {}
            T1_CTRL => io.timers.write_t1_ctrl(v),
            T2_CTRL => io.timers.write_t2_ctrl(v),
            TIMER1 => io.timers.write_t1(v),
            TIMER2..=0x3FF => io.timers.write_t2_nibble((off - TIMER2) as u8, v),
            _ => self.mem[off] = v,
        }
    }

    /// Display on (DSPCTL bit 3).
    pub fn display_on(&self) -> bool {
        self.mem[DSPCTL] & DON_BIT != 0
    }

    /// 5-bit contrast: #301 bits 0-3 and DSPCTL bit 1 as bit 4.
    pub fn contrast(&self) -> u8 {
        self.mem[CONTRAST] | ((self.mem[DSPCTL] >> 1) & 1) << 4
    }

    /// The display RAM nibble at `off` (0-#2FF; larger offsets read 0).
    pub fn ram_nibble(&self, off: usize) -> u8 {
        if off < REGS { self.mem[off] } else { 0 }
    }

    /// Hardware reset: the registers are cleared, the display RAM is
    /// kept (it is RAM; the ROM keeps a restart code at #4025F across
    /// resets).
    pub fn reset(&mut self) {
        self.mem[REGS..].fill(0);
    }
}
