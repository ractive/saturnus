//! Disassembler: renders an [`Instruction`] in HP SASM syntax.
//!
//! Layout follows the SASM recommended columns (src: SASM manual 6.1): the
//! opcode field is 8 characters wide and the modifier starts at column 9,
//! e.g. `A=A+B   W`, `LC(5)   #12345`, `GOTO    #0A3F2`. Instructions
//! without a modifier are the bare mnemonic. Tests carry their second half
//! on the same line after two spaces: `?A=B    A  GOYES #01234` or
//! `?ST=1   3  RTNYES`.
//!
//! Number formats: branch targets are absolute 5-digit hex (`#0A3F2`);
//! LC/LA and D0=/D1= immediates are hex with as many digits as the opcode
//! has nibbles; small counts and bit numbers (P=, ST=0, D0=D0+, CON, DAT
//! nibble counts, ...) are decimal, as SASM evaluates a bare number as
//! decimal (src: SASM manual 6.1.3). Loads longer than 8 nibbles use
//! LCHEX/LAHEX because LC(m)/LA(m) only go to m=8 (src: SASM manual 9).
//! Undefined encodings render as `NIBHEX` of the nibbles consumed, which
//! reassembles to the same bytes.
//!
//! Where SASM accepts several spellings for one opcode (?A=B and ?B=A,
//! A=A+B and A=B+A, ABEX and BAEX, C+P+1 and C=C+P+1, ?ST=0 and ?ST#1,
//! D0=(2) and D0=HEX ...), the first spelling of Nickel's opcode-ordered
//! list is used.

use core::fmt;

use super::instr::{DatSize, Field, Instruction, OnTrue, Reg};

/// Disassemble one instruction. Branch targets are already absolute in
/// `Instruction`, so no address is needed.
pub fn disassemble(instr: &Instruction) -> String {
    instr.to_string()
}

/// The opcode-field text (the SASM mnemonic), e.g. `A=A+B`, `LC(5)`,
/// `?ST=1`, `NIBHEX`.
pub fn mnemonic(instr: &Instruction) -> String {
    use Instruction as I;
    let r = Reg::name;
    let s = |t: &str| t.to_string();
    match *instr {
        I::RtnSxm => s("RTNSXM"),
        I::Rtn => s("RTN"),
        I::RtnSc => s("RTNSC"),
        I::RtnCc => s("RTNCC"),
        I::SetHex => s("SETHEX"),
        I::SetDec => s("SETDEC"),
        I::RstkEqC => s("RSTK=C"),
        I::CEqRstk => s("C=RSTK"),
        I::ClrSt => s("CLRST"),
        I::CEqSt => s("C=ST"),
        I::StEqC => s("ST=C"),
        I::CStEx => s("CSTEX"),
        I::PInc => s("P=P+1"),
        I::PDec => s("P=P-1"),
        I::Rti => s("RTI"),
        I::PSet { .. } => s("P="),
        I::CPlusPPlus1 => s("C+P+1"),
        I::CEqP { .. } => s("C=P"),
        I::PEqC { .. } => s("P=C"),
        I::CpEx { .. } => s("CPEX"),
        I::RtnC => s("RTNC"),
        I::RtnNc => s("RTNNC"),
        I::Goc { .. } => s("GOC"),
        I::Gonc { .. } => s("GONC"),
        I::Goto { .. } => s("GOTO"),
        I::Gosub { .. } => s("GOSUB"),
        I::GoLong { .. } => s("GOLONG"),
        I::GoVLong { .. } => s("GOVLNG"),
        I::GosubL { .. } => s("GOSUBL"),
        I::GosbVL { .. } => s("GOSBVL"),
        I::Nop3 => s("NOP3"),
        I::Nop4 => s("NOP4"),
        I::Nop5 => s("NOP5"),
        I::OutCs => s("OUT=CS"),
        I::OutC => s("OUT=C"),
        I::Uncnfg => s("UNCNFG"),
        I::Config => s("CONFIG"),
        I::CId => s("C=ID"),
        I::Shutdn => s("SHUTDN"),
        I::IntOn => s("INTON"),
        I::Rsi => s("RSI"),
        I::BusCb => s("BUSCB"),
        I::IntOff => s("INTOFF"),
        I::BusCd => s("BUSCD"),
        I::Reset => s("RESET"),
        I::BusCc => s("BUSCC"),
        I::Sreq => s("SREQ?"),
        I::HsClear { mask } => s(match mask {
            0x1 => "XM=0",
            0x2 => "SB=0",
            0x4 => "SR=0",
            0x8 => "MP=0",
            0xF => "CLRHST",
            _ => "HS=0",
        }),
        I::StClear { .. } => s("ST=0"),
        I::StSet { .. } => s("ST=1"),
        I::TestHs { mask, .. } => s(match mask {
            0x1 => "?XM=0",
            0x2 => "?SB=0",
            0x4 => "?SR=0",
            0x8 => "?MP=0",
            _ => "?HS=0",
        }),
        I::TestSt { set, .. } => s(if set { "?ST=1" } else { "?ST=0" }),
        I::TestP { eq, .. } => s(if eq { "?P=" } else { "?P#" }),
        I::Invalid { .. } => s("NIBHEX"),
        I::And { dst, src, .. } => format!("{0}={0}&{1}", r(dst), r(src)),
        I::Or { dst, src, .. } => format!("{0}={0}!{1}", r(dst), r(src)),
        I::ScratchFromReg { ss, src, field } => {
            format!("{}={}{}", ss.name(), r(src), dot_f(field))
        }
        I::RegFromScratch { dst, ss, field } => {
            format!("{}={}{}", r(dst), ss.name(), dot_f(field))
        }
        I::RegScratchEx { reg, ss, field } => {
            format!("{}{}EX{}", r(reg), ss.name(), dot_f(field))
        }
        I::PtrFromReg { ptr, src, short } => {
            format!("{}={}{}", ptr.name(), r(src), if short { "S" } else { "" })
        }
        I::PtrRegEx { ptr, reg, short } => {
            format!(
                "{}{}{}",
                r(reg),
                ptr.name(),
                if short { "XS" } else { "EX" }
            )
        }
        I::PtrAdd { ptr, .. } => format!("{0}={0}+", ptr.name()),
        I::PtrSub { ptr, .. } => format!("{0}={0}-", ptr.name()),
        I::PtrLoad { ptr, nibbles, .. } => format!("{}=({})", ptr.name(), nibbles),
        I::DatWrite { ptr, src, .. } => format!("{}={}", ptr.dat(), r(src)),
        I::DatRead { dst, ptr, .. } => format!("{}={}", r(dst), ptr.dat()),
        I::LoadConst { reg, nibbles, .. } => {
            let l = if reg == Reg::A { "LA" } else { "LC" };
            if nibbles <= 8 {
                format!("{l}({nibbles})")
            } else {
                format!("{l}HEX")
            }
        }
        I::PcEqReg { reg } => format!("PC={}", r(reg)),
        I::PcEqInd { reg } => format!("PC=({})", r(reg)),
        I::RegPcEx { reg } => format!("{}PCEX", r(reg)),
        I::RegEqPc { reg } => format!("{}=PC", r(reg)),
        I::In { dst } => format!("{}=IN", r(dst)),
        I::BitClear { reg, .. } => format!("{}BIT=0", r(reg)),
        I::BitSet { reg, .. } => format!("{}BIT=1", r(reg)),
        I::TestBit { reg, set, .. } => format!("?{}BIT={}", r(reg), u8::from(set)),
        I::Slc { reg } => format!("{}SLC", r(reg)),
        I::Src { reg } => format!("{}SRC", r(reg)),
        I::Srb { reg, field } => format!("{}SRB{}", r(reg), dot_f(field)),
        I::Sl { reg, .. } => format!("{}SL", r(reg)),
        I::Sr { reg, .. } => format!("{}SR", r(reg)),
        I::Add { dst, src, .. } => format!("{0}={0}+{1}", r(dst), r(src)),
        I::Sub { dst, src, .. } => format!("{0}={0}-{1}", r(dst), r(src)),
        I::SubRev { dst, src, .. } => format!("{0}={1}-{0}", r(dst), r(src)),
        I::Inc { reg, .. } => format!("{0}={0}+1", r(reg)),
        I::Dec { reg, .. } => format!("{0}={0}-1", r(reg)),
        I::AddConst { reg, .. } => format!("{0}={0}+CON", r(reg)),
        I::SubConst { reg, .. } => format!("{0}={0}-CON", r(reg)),
        I::Zero { reg, .. } => format!("{}=0", r(reg)),
        I::Copy { dst, src, .. } => format!("{}={}", r(dst), r(src)),
        I::Exch { a, b, .. } => format!("{}{}EX", r(a), r(b)),
        I::Neg { reg, .. } => format!("{0}=-{0}", r(reg)),
        I::Not { reg, .. } => format!("{0}=-{0}-1", r(reg)),
        I::TestCmp { op, lhs, rhs, .. } => format!("?{}{}{}", r(lhs), op.symbol(), r(rhs)),
        I::TestZero { op, reg, .. } => format!("?{}{}0", r(reg), op.symbol()),
    }
}

/// The modifier-field text (operand), empty if the instruction has none.
/// For tests this is only the test's own operand, without GOYES/RTNYES.
pub fn modifier(instr: &Instruction) -> String {
    use Instruction as I;
    match *instr {
        I::And { field, .. }
        | I::Or { field, .. }
        | I::ScratchFromReg {
            field: Some(field), ..
        }
        | I::RegFromScratch {
            field: Some(field), ..
        }
        | I::RegScratchEx {
            field: Some(field), ..
        }
        | I::Srb {
            field: Some(field), ..
        }
        | I::Sl { field, .. }
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
        | I::Not { field, .. }
        | I::TestCmp { field, .. }
        | I::TestZero { field, .. } => field.name().to_string(),
        I::AddConst { field, n, .. } | I::SubConst { field, n, .. } => {
            format!("{},{}", field.name(), n)
        }
        I::DatWrite { size, .. } | I::DatRead { size, .. } => match size {
            DatSize::Field(f) => f.name().to_string(),
            DatSize::Nibbles(n) => n.to_string(),
        },
        I::PSet { n }
        | I::CEqP { n }
        | I::PEqC { n }
        | I::CpEx { n }
        | I::PtrAdd { n, .. }
        | I::PtrSub { n, .. }
        | I::StClear { bit: n }
        | I::StSet { bit: n }
        | I::BitClear { bit: n, .. }
        | I::BitSet { bit: n, .. }
        | I::TestSt { bit: n, .. }
        | I::TestP { n, .. }
        | I::TestBit { bit: n, .. } => n.to_string(),
        I::HsClear { mask } => match mask {
            0x1 | 0x2 | 0x4 | 0x8 | 0xF => String::new(),
            _ => mask.to_string(),
        },
        I::TestHs { mask, .. } => match mask {
            0x1 | 0x2 | 0x4 | 0x8 => String::new(),
            _ => mask.to_string(),
        },
        I::PtrLoad { nibbles, value, .. } => hex_digits(u64::from(value), nibbles, true),
        I::LoadConst { nibbles, value, .. } => hex_digits(value, nibbles, nibbles <= 8),
        I::Goc { target }
        | I::Gonc { target }
        | I::Goto { target }
        | I::Gosub { target }
        | I::GoLong { target }
        | I::GoVLong { target }
        | I::GosubL { target }
        | I::GosbVL { target } => addr(target),
        I::Invalid { nibbles, len } => nibbles
            .iter()
            .take(usize::from(len))
            .map(|n| char::from_digit(u32::from(*n), 16).unwrap_or('?'))
            .collect::<String>()
            .to_uppercase(),
        _ => String::new(),
    }
}

/// `.F` suffix for scratch register and SRB forms with a field.
fn dot_f(field: Option<Field>) -> &'static str {
    if field.is_some() { ".F" } else { "" }
}

/// Absolute 20-bit address as `#XXXXX`.
fn addr(a: u32) -> String {
    format!("#{:05X}", a & 0xF_FFFF)
}

/// `count` hex digits of `value`, most significant first, optionally with
/// a leading `#`.
fn hex_digits(value: u64, count: u8, hash: bool) -> String {
    let width = usize::from(count);
    let masked = if count >= 16 {
        value
    } else {
        value & ((1u64 << (4 * u32::from(count))) - 1)
    };
    if hash {
        format!("#{masked:0width$X}")
    } else {
        format!("{masked:0width$X}")
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mn = mnemonic(self);
        let md = modifier(self);
        if md.is_empty() {
            f.write_str(&mn)?;
        } else {
            write!(f, "{mn:<7} {md}")?;
        }
        match self.on_true() {
            Some(OnTrue::RtnYes) => f.write_str("  RTNYES"),
            Some(OnTrue::GoYes(t)) => write!(f, "  GOYES {}", addr(t)),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "disasm_oracle_tests.rs"]
mod oracle_tests;
