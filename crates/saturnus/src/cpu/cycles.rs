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
}
