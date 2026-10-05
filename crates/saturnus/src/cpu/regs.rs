//! Saturn CPU register file and nibble helpers.
//!
//! wiki: hardware/saturn-cpu (Registers); src: SASM manual 2.1-2.7.
//!
//! Nibble numbering: nibble 0 is the least significant four bits of a
//! register, nibble 15 the most significant (src: SASM manual 2.3 "Register
//! Nibbles"). Field ranges are given as inclusive `(lo, hi)` nibble indices,
//! `0 <= lo <= hi <= 15`; e.g. the A field is `(0, 4)`, XS is `(2, 2)`, S is
//! `(15, 15)`, WP is `(0, P)` (src: SASM manual 2.3).

use super::instr::Reg;

/// Mask for 20-bit addresses (D0, D1, PC, RSTK entries).
/// wiki: hardware/saturn-cpu (20-bit address space).
pub const ADDR_MASK: u32 = 0xF_FFFF;

/// Mask for the 12-bit OUT register. src: SASM manual 2.5.
pub const OUT_MASK: u16 = 0x0FFF;

/// Number of return-stack levels. src: SASM manual 2.1.
pub const RSTK_DEPTH: usize = 8;

/// HST bit 0: external Module Missing (set by RTNSXM). src: SASM manual 2.6.
pub const HST_XM: u8 = 0b0001;
/// HST bit 1: Sticky Bit, set when a non-zero bit is shifted off the right
/// end of a field. src: SASM manual 2.6.
pub const HST_SB: u8 = 0b0010;
/// HST bit 2: Service Request (set by SREQ?). src: SASM manual 2.6.
pub const HST_SR: u8 = 0b0100;
/// HST bit 3: Module Pulled (*NINTX pulled low). src: SASM manual 2.6.
pub const HST_MP: u8 = 0b1000;

/// Arithmetic mode selected by SETHEX / SETDEC. src: SASM manual 2.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Hexadecimal (binary) arithmetic.
    #[default]
    Hex,
    /// Decimal (BCD) arithmetic.
    Dec,
}

/// Eight-level hardware return stack of 20-bit addresses.
///
/// Push drops the oldest entry when all eight levels are in use; pop returns
/// the top and inserts a zero address at the bottom, so after eight pops
/// every level reads 0 (src: SASM manual 8 (C=RSTK); wiki: hardware/saturn-cpu
/// "RSTK behaviour", tutorial p. 41: push 1..9, then read 9..2, 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReturnStack {
    /// `levels[0]` is the top of the stack, `levels[7]` the bottom.
    levels: [u32; RSTK_DEPTH],
}

impl ReturnStack {
    /// Push a 20-bit address (masked); the bottom level is lost.
    pub fn push(&mut self, addr: u32) {
        self.levels.copy_within(0..RSTK_DEPTH - 1, 1);
        self.levels[0] = addr & ADDR_MASK;
    }

    /// Pop the top address; a zero is inserted at the bottom.
    pub fn pop(&mut self) -> u32 {
        let top = self.levels[0];
        self.levels.copy_within(1..RSTK_DEPTH, 0);
        self.levels[RSTK_DEPTH - 1] = 0;
        top
    }

    /// All levels, top first. For debuggers and state inspection.
    pub fn levels(&self) -> &[u32; RSTK_DEPTH] {
        &self.levels
    }

    /// A stack holding `levels`, top first (each masked to 20 bits).
    pub fn from_levels(levels: [u32; RSTK_DEPTH]) -> Self {
        Self {
            levels: levels.map(|l| l & ADDR_MASK),
        }
    }

    /// Clear every level to zero.
    pub fn clear(&mut self) {
        self.levels = [0; RSTK_DEPTH];
    }
}

/// The complete Saturn CPU register file. wiki: hardware/saturn-cpu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    /// Working registers, 16 nibbles each.
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
    /// Scratch registers R0..R4.
    pub r: [u64; 5],
    /// Data pointers, 20 bits.
    pub d0: u32,
    pub d1: u32,
    /// Program counter, 20 bits.
    pub pc: u32,
    /// Pointer register, 0..=15.
    pub p: u8,
    /// Program status bits ST0..ST15. Bits 0-11 are the "ST register"
    /// accessed by C=ST/ST=C/CSTEX/CLRST; 12-15 belong to the OS.
    /// src: SASM manual 2.6; wiki: hardware/saturn-cpu.
    pub st: u16,
    /// Hardware status bits: see `HST_XM`, `HST_SB`, `HST_SR`, `HST_MP`.
    pub hst: u8,
    /// Carry flag. src: SASM manual 2.6.
    pub carry: bool,
    /// Arithmetic mode. src: SASM manual 2.7.
    pub mode: Mode,
    /// Output register, 12 bits (write-only for software).
    pub out: u16,
    /// Input register, 16 bits.
    pub inp: u16,
    /// Hardware return stack.
    pub rstk: ReturnStack,
    /// Maskable interrupts enabled (INTON sets, INTOFF clears).
    /// wiki: hardware/interrupts.
    pub interrupts_enabled: bool,
    /// Set on interrupt entry (jump to #0000F), cleared by RTI; while set,
    /// further interrupts are blocked. wiki: hardware/interrupts.
    pub in_interrupt: bool,
    /// An interrupt arrived while `in_interrupt` was set; the handler runs
    /// again before returning. wiki: hardware/interrupts.
    pub interrupt_pending: bool,
}

impl Default for Registers {
    fn default() -> Self {
        let mut r = Self {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            r: [0; 5],
            d0: 0,
            d1: 0,
            pc: 0,
            p: 0,
            st: 0,
            hst: 0,
            carry: false,
            mode: Mode::Hex,
            out: 0,
            inp: 0,
            rstk: ReturnStack::default(),
            interrupts_enabled: true,
            in_interrupt: false,
            interrupt_pending: false,
        };
        r.reset();
        r
    }
}

impl Registers {
    /// Power-on / reset state.
    ///
    /// Sources only fix part of this: programs start in HEX mode (tutorial
    /// p. 65 note) and all interrupts vector to #0000F (SASM manual 2.10).
    /// PC = 0, P = 0, carry clear, ST/HST zero, RSTK zero and interrupts
    /// enabled are assumptions (the ROM initialises everything it relies on);
    /// working, scratch and pointer registers are cleared for determinism.
    pub fn reset(&mut self) {
        self.a = 0;
        self.b = 0;
        self.c = 0;
        self.d = 0;
        self.r = [0; 5];
        self.d0 = 0;
        self.d1 = 0;
        self.pc = 0;
        self.p = 0;
        self.st = 0;
        self.hst = 0;
        self.carry = false;
        self.mode = Mode::Hex;
        self.out = 0;
        self.inp = 0;
        self.rstk.clear();
        self.interrupts_enabled = true;
        self.in_interrupt = false;
        self.interrupt_pending = false;
    }

    /// Read a working register.
    pub fn get(&self, reg: Reg) -> u64 {
        match reg {
            Reg::A => self.a,
            Reg::B => self.b,
            Reg::C => self.c,
            Reg::D => self.d,
        }
    }

    /// Write a working register.
    pub fn set(&mut self, reg: Reg, value: u64) {
        match reg {
            Reg::A => self.a = value,
            Reg::B => self.b = value,
            Reg::C => self.c = value,
            Reg::D => self.d = value,
        }
    }

    /// Push onto RSTK (20-bit masked).
    pub fn push(&mut self, addr: u32) {
        self.rstk.push(addr);
    }

    /// Pop from RSTK.
    pub fn pop(&mut self) -> u32 {
        self.rstk.pop()
    }

    /// Set D0, masking to 20 bits.
    pub fn set_d0(&mut self, v: u32) {
        self.d0 = v & ADDR_MASK;
    }

    /// Set D1, masking to 20 bits.
    pub fn set_d1(&mut self, v: u32) {
        self.d1 = v & ADDR_MASK;
    }

    /// Set PC, masking to 20 bits.
    pub fn set_pc(&mut self, v: u32) {
        self.pc = v & ADDR_MASK;
    }

    /// Set P, masking to 4 bits.
    pub fn set_p(&mut self, v: u8) {
        self.p = v & 0xF;
    }

    /// Set OUT, masking to 12 bits.
    pub fn set_out(&mut self, v: u16) {
        self.out = v & OUT_MASK;
    }
}

/// Nibble `idx` (0..=15, masked) of `reg`.
pub fn get_nibble(reg: u64, idx: u8) -> u8 {
    ((reg >> (4 * u32::from(idx & 0xF))) & 0xF) as u8
}

/// `reg` with nibble `idx` (0..=15, masked) replaced by `val & 0xF`.
pub fn set_nibble(reg: u64, idx: u8, val: u8) -> u64 {
    let shift = 4 * u32::from(idx & 0xF);
    (reg & !(0xF << shift)) | (u64::from(val & 0xF) << shift)
}

/// Bit mask covering nibbles `lo..=hi` in place. Indices are masked to
/// 0..=15; `lo > hi` yields an empty mask.
pub fn field_mask(lo: u8, hi: u8) -> u64 {
    let (lo, hi) = (lo & 0xF, hi & 0xF);
    if lo > hi {
        return 0;
    }
    let width_bits = 4 * u32::from(hi - lo + 1);
    let low = if width_bits >= 64 {
        u64::MAX
    } else {
        (1u64 << width_bits) - 1
    };
    low << (4 * u32::from(lo))
}

/// Number of nibbles in the field `lo..=hi` (0 if `lo > hi`).
pub fn field_width(lo: u8, hi: u8) -> u32 {
    let (lo, hi) = (lo & 0xF, hi & 0xF);
    if lo > hi { 0 } else { u32::from(hi - lo + 1) }
}

/// Value of nibbles `lo..=hi`, shifted down so nibble `lo` becomes nibble 0.
pub fn field_get(reg: u64, lo: u8, hi: u8) -> u64 {
    (reg & field_mask(lo, hi)) >> (4 * u32::from(lo & 0xF))
}

/// `reg` with nibbles `lo..=hi` replaced by the low nibbles of `value`
/// (excess high bits of `value` are discarded).
pub fn field_set(reg: u64, lo: u8, hi: u8, value: u64) -> u64 {
    let mask = field_mask(lo, hi);
    let shifted = value.checked_shl(4 * u32::from(lo & 0xF)).unwrap_or(0);
    (reg & !mask) | (shifted & mask)
}

#[cfg(test)]
// Literal groupings mark field boundaries (e.g. 0xAB_1234_5 = A field 12345).
#[allow(clippy::unusual_byte_groupings)]
mod tests {
    use super::*;

    #[test]
    fn nibble_helpers() {
        let r = 0xFEDC_BA98_7654_3210u64;
        for i in 0..16u8 {
            assert_eq!(get_nibble(r, i), i);
        }
        assert_eq!(set_nibble(r, 0, 0xA), 0xFEDC_BA98_7654_321A);
        assert_eq!(set_nibble(r, 15, 0x0), 0x0EDC_BA98_7654_3210);
        assert_eq!(set_nibble(0, 3, 0x1F), 0x0000_0000_0000_F000);
    }

    #[test]
    fn field_helpers() {
        let r = 0xFEDC_BA98_7654_3210u64;
        assert_eq!(field_mask(0, 15), u64::MAX);
        assert_eq!(field_mask(0, 4), 0xF_FFFF);
        assert_eq!(field_mask(2, 2), 0xF00);
        assert_eq!(field_mask(3, 2), 0);
        assert_eq!(field_width(0, 15), 16);
        assert_eq!(field_get(r, 0, 4), 0x43210); // A
        assert_eq!(field_get(r, 3, 14), 0xEDC_BA98_7654_3); // M
        assert_eq!(field_get(r, 15, 15), 0xF); // S
        assert_eq!(field_get(r, 0, 15), r); // W
        assert_eq!(field_set(r, 0, 1, 0xABC), 0xFEDC_BA98_7654_32BC); // B
        assert_eq!(field_set(r, 0, 15, 5), 5);
        assert_eq!(field_set(r, 15, 15, 0), 0x0EDC_BA98_7654_3210);
    }

    #[test]
    fn work_reg_get_set() {
        let mut regs = Registers::default();
        regs.set(Reg::A, 1);
        regs.set(Reg::B, 2);
        regs.set(Reg::C, 3);
        regs.set(Reg::D, 4);
        assert_eq!(
            [Reg::A, Reg::B, Reg::C, Reg::D].map(|r| regs.get(r)),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn rstk_circular() {
        // Tutorial p. 41: push 1..9, then read 9, 8, ..., 2, 0.
        let mut regs = Registers::default();
        for v in 1..=9 {
            regs.push(v);
        }
        let popped: Vec<u32> = (0..9).map(|_| regs.pop()).collect();
        assert_eq!(popped, vec![9, 8, 7, 6, 5, 4, 3, 2, 0]);
        // Everything reads zero after emptying.
        assert!(regs.rstk.levels().iter().all(|&l| l == 0));
        // 20-bit masking.
        regs.push(0x1_2345_6);
        assert_eq!(regs.pop(), 0x2_3456);
    }

    #[test]
    fn reset_state() {
        let mut regs = Registers {
            pc: 0x1234,
            p: 7,
            carry: true,
            mode: Mode::Dec,
            st: 0xFFFF,
            hst: 0xF,
            interrupts_enabled: false,
            ..Registers::default()
        };
        regs.push(5);
        regs.reset();
        assert_eq!(regs, Registers::default());
        assert_eq!(regs.pc, 0);
        assert_eq!(regs.mode, Mode::Hex);
        assert!(regs.interrupts_enabled);
        assert!(!regs.carry);
        assert_eq!(regs.rstk.levels(), &[0; RSTK_DEPTH]);
    }

    #[test]
    fn masked_setters() {
        let mut regs = Registers::default();
        regs.set_d0(0xFFF_FFFF);
        regs.set_d1(0x10_0001);
        regs.set_pc(0x12_3456);
        regs.set_p(0x1F);
        regs.set_out(0xFFFF);
        assert_eq!(
            (regs.d0, regs.d1, regs.pc, regs.p, regs.out),
            (0xF_FFFF, 0x1, 0x2_3456, 0xF, 0xFFF)
        );
    }
}
