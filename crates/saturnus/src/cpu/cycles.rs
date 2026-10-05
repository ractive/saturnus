//! Approximate CPU cycle counts per instruction.
//!
//! **Approximate.** Counts come from the "cycles:" lines of the SASM manual
//! section 8 (Mnemonic Dictionary). Level-2 instructions (marked `**` in
//! SASM: A=PC, PC=A, APCEX, ABIT ops, `.F` scratch forms, 818 constants,
//! SRB.F, LA, BUSCB/BUSCD, PC=(C)) have no SASM count; for those we take the
//! tutorial tables (Fernandes/Rechlin ch. 33-52, Meta Kernel figures) rounded
//! down, which are on a slightly different scale. The tutorial itself says
//! its counts are approximations and that some depend on odd/even
//! addresses; that effect is ignored here (wiki: hardware/saturn-cpu
//! "Timing"). Exactness is not a goal of iteration 1.
//!
//! `d` is the field width in nibbles (`field.range(p)`), as in SASM's
//! "3 + d" notation.

use super::instr::{DatSize, Field, Instruction, Reg};

fn width(field: Field, p: u8) -> u32 {
    let (lo, hi) = field.range(p);
    u32::from(hi - lo) + 1
}

/// Register-field operations: 7 for the short A-field opcode forms
/// (Cx-Fx), `3 + d` for the field forms (src: SASM manual 8, e.g. A=A+B).
fn field_op(field: Field, p: u8) -> u32 {
    if field == Field::A {
        7
    } else {
        3 + width(field, p)
    }
}

/// Approximate cycles taken by `instr` with pointer register `p` (for P/WP
/// field widths). `taken` selects the GO/RTNYES count of tests and
/// conditional jumps/returns, as opposed to the NO count.
pub fn cycles(instr: &Instruction, p: u8, taken: bool) -> u32 {
    use Instruction as I;
    let branch = |yes: u32, no: u32| if taken { yes } else { no };
    match *instr {
        I::RtnSxm | I::Rtn | I::RtnSc | I::RtnCc | I::Rti => 9,
        I::SetHex | I::SetDec | I::PInc | I::PDec => 3,
        I::RstkEqC | I::CEqRstk => 8,
        I::ClrSt | I::CEqSt | I::StEqC | I::CStEx => 6,
        I::And { field, .. } | I::Or { field, .. } => 4 + width(field, p),

        I::ScratchFromReg { field, .. }
        | I::RegFromScratch { field, .. }
        | I::RegScratchEx { field, .. } => match field {
            None => 19,
            // Tutorial p. 72: 9+n (A field 14).
            Some(f) => 9 + width(f, p),
        },

        I::PtrFromReg { short, .. } | I::PtrRegEx { short, .. } => {
            if short {
                7
            } else {
                8
            }
        }
        I::PtrAdd { .. } | I::PtrSub { .. } => 7,
        I::PtrLoad { nibbles, .. } => 2 + u32::from(nibbles),

        I::DatWrite { size, .. } => match size {
            DatSize::Field(Field::A) => 17,
            DatSize::Field(Field::B) => 14,
            DatSize::Field(f) => 16 + width(f, p),
            DatSize::Nibbles(n) => 15 + u32::from(n),
        },
        I::DatRead { size, .. } => match size {
            DatSize::Field(Field::A) => 18,
            DatSize::Field(Field::B) => 15,
            DatSize::Field(f) => 17 + width(f, p),
            DatSize::Nibbles(n) => 16 + u32::from(n),
        },

        I::PSet { .. } => 2,
        I::LoadConst { reg, nibbles, .. } => {
            let m = u32::from(nibbles);
            match reg {
                // LC: 3+m (SASM). LA has a 3-nibble longer opcode; tutorial
                // p. 48 gives LA 4.5 cycles more than LC.
                Reg::C => 3 + m,
                _ => 7 + m,
            }
        }
        I::CPlusPPlus1 => 8,
        I::CEqP { .. } | I::PEqC { .. } | I::CpEx { .. } => 6,

        I::RtnC | I::RtnNc | I::Goc { .. } | I::Gonc { .. } => branch(10, 3),
        I::Goto { .. } => 11,
        I::Gosub { .. } => 12,
        I::GoLong { .. } | I::GoVLong { .. } => 14,
        I::GosubL { .. } | I::GosbVL { .. } => 15,
        // Tutorial p. 87 (no SASM count).
        I::PcEqReg { .. } => 26,
        I::PcEqInd { .. } => 23,
        I::RegPcEx { .. } => 19,
        I::RegEqPc { .. } => 11,
        // 820 is HS=0 with an empty mask: 3 cycles.
        I::Nop3 => 3,
        I::Nop4 | I::Nop5 => 11,

        I::OutCs => 4,
        I::OutC => 6,
        I::In { .. } => 7,
        I::Uncnfg => 12,
        I::Config | I::CId => 11,
        I::Shutdn | I::IntOn | I::IntOff => 5,
        I::Rsi | I::Reset | I::BusCc => 6,
        // Tutorial bus-command table.
        I::BusCb | I::BusCd => 10,
        I::Sreq => 7,

        // Tutorial ABIT table: 7.5.
        I::BitClear { .. } | I::BitSet { .. } => 7,
        I::HsClear { .. } => 3,
        I::StClear { .. } | I::StSet { .. } => 4,

        I::Slc { .. } | I::Src { .. } => 21,
        I::Srb { field, .. } => match field {
            None => 20,
            // Tutorial ASRB table: A field 13.5.
            Some(f) => 8 + width(f, p),
        },
        I::Sl { field, .. }
        | I::Sr { field, .. }
        | I::Add { field, .. }
        | I::Sub { field, .. }
        | I::SubRev { field, .. }
        | I::Inc { field, .. }
        | I::Dec { field, .. }
        | I::Zero { field, .. }
        | I::Copy { field, .. }
        | I::Exch { field, .. }
        | I::Neg { field, .. }
        | I::Not { field, .. } => field_op(field, p),
        // Tutorial p. 60: 8+n.
        I::AddConst { field, .. } | I::SubConst { field, .. } => 8 + width(field, p),

        I::TestCmp { field, .. } | I::TestZero { field, .. } => {
            let d = width(field, p);
            branch(13 + d, 6 + d)
        }
        I::TestHs { .. } | I::TestP { .. } => branch(13, 6),
        I::TestSt { .. } => branch(14, 7),
        // Tutorial ?ABIT table: 12.5 / 20.5.
        I::TestBit { .. } => branch(20, 12),

        // No source; one cycle per nibble fetched as a placeholder.
        I::Invalid { len, .. } => u32::from(len.max(1)),
    }
}

/// Which cycle table times the CPU.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CycleTable {
    /// The SASM manual's counts ([`cycles`]): the 48SX (Clarke).
    #[default]
    Sasm,
    /// The Meta Kernel counts quoted by the Saturn tutorial ([`cycles_g`]):
    /// the G-series Yorke (48GX, 49G, 38G, 39G, 40G).
    MetaKernel,
}

/// Half-cycle counts of the G-series table: `(main, extra)`. `main` is
/// rounded by the instruction's address parity, `extra` (the tutorial's
/// "second number after a comma") by the parity of the address read.
fn half_cycles_g(instr: &Instruction, p: u8, taken: bool) -> (u32, u32) {
    use Instruction as I;
    let branch = |yes: u32, no: u32| if taken { yes } else { no };
    let n = |field: Field| 2 * width(field, p);
    // Field ops: 8 for the short A-field forms, 4.5 + n otherwise.
    let field_op = |field: Field| if field == Field::A { 16 } else { 9 + n(field) };
    let main = match *instr {
        I::RtnSxm | I::Rtn | I::RtnSc | I::RtnCc | I::Rti => 22,
        I::SetHex | I::SetDec | I::PInc | I::PDec => 8,
        I::RstkEqC | I::CEqRstk => 18,
        I::ClrSt | I::CEqSt | I::StEqC | I::CStEx => 14,
        // 11 for the A field, 6 + n otherwise.
        I::And { field, .. } | I::Or { field, .. } => 12 + n(field),

        I::ScratchFromReg { field, .. }
        | I::RegFromScratch { field, .. }
        | I::RegScratchEx { field, .. } => match field {
            None => 41,
            // A field 14; the field forms follow 9 + n.
            Some(f) => 18 + n(f),
        },

        I::PtrFromReg { short, .. } | I::PtrRegEx { short, .. } => {
            if short {
                17
            } else {
                19
            }
        }
        I::PtrAdd { .. } | I::PtrSub { .. } => 17,
        // D0=(2) 6, (4) 9, (5) 10.5: 3 + 1.5 n.
        I::PtrLoad { nibbles, .. } => 6 + 3 * u32::from(nibbles),

        I::DatWrite { size, .. } => match size {
            DatSize::Field(Field::A) => 39,
            DatSize::Field(Field::B) => 33,
            DatSize::Field(f) => 38 + n(f),
            DatSize::Nibbles(k) => 36 + 2 * u32::from(k),
        },
        I::DatRead { size, .. } => match size {
            DatSize::Field(Field::A) => 47,
            DatSize::Field(Field::B) => 39,
            DatSize::Field(f) => 40 + n(f),
            DatSize::Nibbles(k) => 38 + 2 * u32::from(k),
        },

        I::PSet { .. } => 6,
        I::LoadConst { reg, nibbles, .. } => {
            let m = 3 * u32::from(nibbles);
            match reg {
                // LC 3 + 1.5 n, LA 7.5 + 1.5 n.
                Reg::C => 6 + m,
                _ => 15 + m,
            }
        }
        I::CPlusPPlus1 => 19,
        I::CEqP { .. } | I::PEqC { .. } | I::CpEx { .. } => 16,

        I::RtnC | I::RtnNc | I::Goc { .. } | I::Gonc { .. } => branch(25, 9),
        I::Goto { .. } => 28,
        I::Gosub { .. } => 30,
        I::GoLong { .. } => 34,
        I::GoVLong { .. } => 37,
        I::GosubL { .. } => 36,
        I::GosbVL { .. } => 39,
        I::PcEqReg { .. } | I::PcEqInd { .. } => 52,
        I::RegPcEx { .. } => 38,
        I::RegEqPc { .. } => 22,
        // No G-series figure for 820; CLRHST-like 4.5.
        I::Nop3 => 9,
        I::Nop4 | I::Nop5 => 22,

        I::OutCs => 11,
        I::OutC => 15,
        I::In { .. } => 17,
        I::Uncnfg => 29,
        I::Config | I::CId => 27,
        I::Shutdn => 13,
        I::IntOn | I::IntOff => 14,
        I::Rsi | I::BusCc => 17,
        I::Reset => 15,
        I::BusCb | I::BusCd => 20,
        I::Sreq => 19,

        I::BitClear { .. } | I::BitSet { .. } => 15,
        I::HsClear { .. } => 9,
        I::StClear { .. } | I::StSet { .. } => 11,

        I::Slc { .. } | I::Src { .. } => 45,
        I::Srb { field, .. } => match field {
            None => 43,
            // A field 13.5: 8.5 + n.
            Some(f) => 17 + n(f),
        },
        I::Sl { field, .. }
        | I::Sr { field, .. }
        | I::Add { field, .. }
        | I::Sub { field, .. }
        | I::SubRev { field, .. }
        | I::Inc { field, .. }
        | I::Dec { field, .. }
        | I::Zero { field, .. }
        | I::Copy { field, .. }
        | I::Exch { field, .. }
        | I::Neg { field, .. }
        | I::Not { field, .. } => field_op(field),
        I::AddConst { field, .. } | I::SubConst { field, .. } => 16 + n(field),

        // A field 13.5 / 21.5; other fields 8.5 + n / 16.5 + n.
        I::TestCmp { field, .. } | I::TestZero { field, .. } => {
            branch(33 + n(field), 17 + n(field))
        }
        I::TestHs { .. } | I::TestP { .. } => branch(31, 15),
        I::TestSt { .. } => branch(33, 17),
        I::TestBit { .. } => branch(41, 25),

        I::Invalid { len, .. } => 2 * u32::from(len.max(1)),
    };
    let extra = match *instr {
        // A=DAT0 A: 23.5,3.5; A=DAT0 f: 20+n,1+n/2.
        I::DatRead {
            size: DatSize::Field(Field::A),
            ..
        } => 7,
        I::DatRead {
            size: DatSize::Field(f),
            ..
        } if f != Field::B => 2 + width(f, p),
        // PC=A, PC=(A): 26,3.5.
        I::PcEqReg { .. } | I::PcEqInd { .. } => 7,
        _ => 0,
    };
    (main, extra)
}

/// Round a half-cycle count the tutorial's way: down at an even address,
/// up at an odd one.
fn round_half(h: u32, odd: bool) -> u32 {
    if odd { h.div_ceil(2) } else { h / 2 }
}

/// Cycles taken by `instr` on the G series (Yorke): the Meta Kernel counts
/// quoted by the Saturn tutorial (Fernandes/Rechlin, ch. 33-52; p. 44:
/// "These cycle counts are from the Meta Kernel documentation", for "the
/// HP 48G, whose processor runs at about 4 MHz"). The tutorial's rule for
/// fractional counts is applied: a `.5` rounds down when the instruction
/// sits at an even address (`pc_odd` false) and up at an odd one; the
/// second count after a comma rounds by the parity of the address read
/// (`data_odd`: the DAT pointer, or the target of PC=A / PC=(A)). wiki:
/// hardware/saturn-cpu "Timing"; Emu48's change log says the G series
/// counts differ from the S series' (wiki: emulators/emu48 SP1).
pub fn cycles_g(instr: &Instruction, p: u8, taken: bool, pc_odd: bool, data_odd: bool) -> u32 {
    let (main, extra) = half_cycles_g(instr, p, taken);
    round_half(main, pc_odd) + round_half(extra, data_odd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_counts() {
        let add_a = Instruction::Add {
            dst: Reg::A,
            src: Reg::B,
            field: Field::A,
        };
        let add_wp = Instruction::Add {
            dst: Reg::A,
            src: Reg::B,
            field: Field::Wp,
        };
        assert_eq!(cycles(&add_a, 0, false), 7);
        assert_eq!(cycles(&add_wp, 3, false), 7);
        assert_eq!(cycles(&add_wp, 15, false), 19);
        let goc = Instruction::Goc { target: 0 };
        assert_eq!((cycles(&goc, 0, true), cycles(&goc, 0, false)), (10, 3));
        let read = Instruction::DatRead {
            dst: Reg::A,
            ptr: crate::cpu::instr::Ptr::D0,
            size: DatSize::Nibbles(4),
        };
        assert_eq!(cycles(&read, 0, false), 20);
    }

    #[test]
    fn g_series_counts_follow_the_tutorial() {
        let add_a = Instruction::Add {
            dst: Reg::A,
            src: Reg::B,
            field: Field::A,
        };
        let sr_w = Instruction::Sr {
            reg: Reg::A,
            field: Field::W,
        };
        // A=A+B A: 8; ASR W: 4.5 + 16, rounded by address parity.
        assert_eq!(cycles_g(&add_a, 0, false, false, false), 8);
        assert_eq!(cycles_g(&sr_w, 0, false, false, false), 20);
        assert_eq!(cycles_g(&sr_w, 0, false, true, false), 21);
        // GONC: 4.5 or 12.5.
        let gonc = Instruction::Gonc { target: 0 };
        assert_eq!(cycles_g(&gonc, 0, true, false, false), 12);
        assert_eq!(cycles_g(&gonc, 0, false, true, false), 5);
        // C=DAT0 A: 23.5,3.5 -> 23 + 3 (both even) .. 24 + 4 (both odd).
        let read_a = Instruction::DatRead {
            dst: Reg::C,
            ptr: crate::cpu::instr::Ptr::D0,
            size: DatSize::Field(Field::A),
        };
        assert_eq!(cycles_g(&read_a, 0, false, false, false), 26);
        assert_eq!(cycles_g(&read_a, 0, false, true, true), 28);
        // LC 5 nibbles: 3 + 1.5 * 5 = 10.5.
        let lc = Instruction::LoadConst {
            reg: Reg::C,
            nibbles: 5,
            value: 0,
        };
        assert_eq!(cycles_g(&lc, 0, false, false, false), 10);
    }
}
