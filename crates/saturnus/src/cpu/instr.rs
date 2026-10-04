//! Decoded Saturn instruction representation.
//!
//! The decoder (`decode.rs`) turns a nibble stream into an [`Instruction`];
//! the disassembler (`disasm.rs`) renders it in HP SASM syntax; the
//! executor interprets it. Branch targets are stored as absolute 20-bit
//! addresses, already resolved against the address of the instruction.
//!
//! wiki: hardware/saturn-cpu; src: SASM manual 6.2-6.3, 9

/// Field selector of a register operation.
///
/// Encoded in opcodes with either the "a" code (P=0 .. W=7) or the "b" code
/// (a + 8); the A field has no a/b code and is either implied by a short
/// opcode or encoded as F where the opcode allows it (src: SASM manual 6.3,
/// "f" codes in section 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Nibble selected by the P register.
    P,
    /// Nibbles P down to 0.
    Wp,
    /// Exponent sign, nibble 2.
    Xs,
    /// Exponent, nibbles 2..0.
    X,
    /// Mantissa sign, nibble 15.
    S,
    /// Mantissa, nibbles 14..3.
    M,
    /// Byte, nibbles 1..0.
    B,
    /// Whole register, nibbles 15..0.
    W,
    /// Address, nibbles 4..0.
    A,
}

impl Field {
    /// Field for an "a" code (0..=7). Codes 8..=15 return `None`.
    pub const fn from_a_code(code: u8) -> Option<Field> {
        match code {
            0 => Some(Field::P),
            1 => Some(Field::Wp),
            2 => Some(Field::Xs),
            3 => Some(Field::X),
            4 => Some(Field::S),
            5 => Some(Field::M),
            6 => Some(Field::B),
            7 => Some(Field::W),
            _ => None,
        }
    }

    /// Field for an "f" code: an "a" code (0..=7) or F for the A field.
    /// Codes 8..=14 are the user-defined fields F1-F7 of the ARM-based
    /// Saturn+ and are undefined on the real Saturn (wiki: hardware/saturn-cpu).
    pub const fn from_f_code(code: u8) -> Option<Field> {
        match code {
            0xF => Some(Field::A),
            c => Field::from_a_code(c),
        }
    }

    /// The inclusive nibble range `(lo, hi)` the field covers, given the
    /// current value of P (only P and WP depend on it). `p` is masked to
    /// 0..=15.
    pub const fn range(self, p: u8) -> (u8, u8) {
        let p = p & 0xF;
        match self {
            Field::P => (p, p),
            Field::Wp => (0, p),
            Field::Xs => (2, 2),
            Field::X => (0, 2),
            Field::S => (15, 15),
            Field::M => (3, 14),
            Field::B => (0, 1),
            Field::W => (0, 15),
            Field::A => (0, 4),
        }
    }

    /// SASM spelling of the field selector.
    pub const fn name(self) -> &'static str {
        match self {
            Field::P => "P",
            Field::Wp => "WP",
            Field::Xs => "XS",
            Field::X => "X",
            Field::S => "S",
            Field::M => "M",
            Field::B => "B",
            Field::W => "W",
            Field::A => "A",
        }
    }
}

/// Working register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reg {
    A,
    B,
    C,
    D,
}

impl Reg {
    /// Register for index 0..=3 (A..D); higher bits are ignored.
    pub const fn from_index(i: u8) -> Reg {
        match i & 3 {
            0 => Reg::A,
            1 => Reg::B,
            2 => Reg::C,
            _ => Reg::D,
        }
    }

    /// Single-letter name.
    pub const fn name(self) -> &'static str {
        match self {
            Reg::A => "A",
            Reg::B => "B",
            Reg::C => "C",
            Reg::D => "D",
        }
    }
}

/// Data pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ptr {
    D0,
    D1,
}

impl Ptr {
    /// SASM name.
    pub const fn name(self) -> &'static str {
        match self {
            Ptr::D0 => "D0",
            Ptr::D1 => "D1",
        }
    }

    /// Name of the memory operand addressed by this pointer.
    pub const fn dat(self) -> &'static str {
        match self {
            Ptr::D0 => "DAT0",
            Ptr::D1 => "DAT1",
        }
    }
}

/// Scratch register R0..R4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scratch {
    R0,
    R1,
    R2,
    R3,
    R4,
}

impl Scratch {
    /// Scratch register for index 0..=4; other values give `None`.
    pub const fn from_index(i: u8) -> Option<Scratch> {
        match i {
            0 => Some(Scratch::R0),
            1 => Some(Scratch::R1),
            2 => Some(Scratch::R2),
            3 => Some(Scratch::R3),
            4 => Some(Scratch::R4),
            _ => None,
        }
    }

    /// Index 0..=4.
    pub const fn index(self) -> usize {
        match self {
            Scratch::R0 => 0,
            Scratch::R1 => 1,
            Scratch::R2 => 2,
            Scratch::R3 => 3,
            Scratch::R4 => 4,
        }
    }

    /// SASM name.
    pub const fn name(self) -> &'static str {
        match self {
            Scratch::R0 => "R0",
            Scratch::R1 => "R1",
            Scratch::R2 => "R2",
            Scratch::R3 => "R3",
            Scratch::R4 => "R4",
        }
    }
}

/// Size operand of a memory transfer (DAT0/DAT1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatSize {
    /// A field selector.
    Field(Field),
    /// Nibbles 0..n-1 of the register, n in 1..=16.
    Nibbles(u8),
}

/// Comparison of a register test. All comparisons are unsigned
/// (src: SASM manual 6.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cmp {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

impl Cmp {
    /// SASM operator spelling.
    pub const fn symbol(self) -> &'static str {
        match self {
            Cmp::Eq => "=",
            Cmp::Ne => "#",
            Cmp::Gt => ">",
            Cmp::Lt => "<",
            Cmp::Ge => ">=",
            Cmp::Le => "<=",
        }
    }
}

/// What a test does when it is true: the GOYES/RTNYES half of the test
/// opcode. An offset of 00 means RTNYES (wiki: hardware/saturn-cpu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OnTrue {
    /// Return from subroutine (offset 00).
    RtnYes,
    /// Jump to the absolute address.
    GoYes(u32),
}

/// A decoded Saturn instruction (HP 48/49G instruction set: SASM levels 0-2,
/// no ARM-only Saturn+ extensions).
///
/// Comments give the opcode pattern in SASM notation: `a` field code 0-7,
/// `b` field code 8-F, `f` field code 0-7 or F (A field), `n` an immediate
/// nibble, `x`/`yy` an operand chosen by the decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Instruction {
    // ----- returns and simple 2-nibble ops (0x) -----
    /// RTNSXM `00`: return and set XM.
    RtnSxm,
    /// RTN `01`.
    Rtn,
    /// RTNSC `02`: return, set carry.
    RtnSc,
    /// RTNCC `03`: return, clear carry.
    RtnCc,
    /// SETHEX `04`.
    SetHex,
    /// SETDEC `05`.
    SetDec,
    /// RSTK=C `06`: push C(A).
    RstkEqC,
    /// C=RSTK `07`: pop into C(A).
    CEqRstk,
    /// CLRST `08`: clear ST bits 11-0.
    ClrSt,
    /// C=ST `09`.
    CEqSt,
    /// ST=C `0A`.
    StEqC,
    /// CSTEX `0B`.
    CStEx,
    /// P=P+1 `0C`.
    PInc,
    /// P=P-1 `0D`.
    PDec,
    /// RTI `0F`.
    Rti,
    /// r=r&s `0Efx` (x 0-7).
    And { dst: Reg, src: Reg, field: Field },
    /// r=r!s `0Efx` (x 8-F).
    Or { dst: Reg, src: Reg, field: Field },

    // ----- scratch registers (10x, 11x, 12x, 81Af..) -----
    /// ss=A / ss=C `10x`; ss=A.F fs `81Af0x`. `field` is `None` for the
    /// whole-register short form.
    ScratchFromReg {
        ss: Scratch,
        src: Reg,
        field: Option<Field>,
    },
    /// A=ss / C=ss `11x`; A=ss.F fs `81Af1x`.
    RegFromScratch {
        dst: Reg,
        ss: Scratch,
        field: Option<Field>,
    },
    /// AssEX / CssEX `12x`; AssEX.F fs `81Af2x`.
    RegScratchEx {
        reg: Reg,
        ss: Scratch,
        field: Option<Field>,
    },

    // ----- data pointers (13x, 16x-1Fx) -----
    /// D0=A, D1=C .. `13x` (A field); D0=AS .. (`short`, nibbles 3-0).
    PtrFromReg { ptr: Ptr, src: Reg, short: bool },
    /// AD0EX .. `13x`; AD0XS .. (`short`, nibbles 3-0).
    PtrRegEx { ptr: Ptr, reg: Reg, short: bool },
    /// D0=D0+ n `16x`, D1=D1+ n `17x` (n = x+1).
    PtrAdd { ptr: Ptr, n: u8 },
    /// D0=D0- n `18x`, D1=D1- n `1Cx` (n = x+1).
    PtrSub { ptr: Ptr, n: u8 },
    /// D0=(2) `19nn`, D0=(4) `1Annnn`, D0=(5) `1Bnnnnn`, D1 `1D/1E/1F`:
    /// load the low `nibbles` nibbles of the pointer with `value`.
    PtrLoad { ptr: Ptr, nibbles: u8, value: u32 },

    // ----- memory transfers (14x, 15xy) -----
    /// DAT0=A .. `140`/`148`/`150a`/`158x`.
    DatWrite { ptr: Ptr, src: Reg, size: DatSize },
    /// A=DAT0 .. `142`/`14A`/`152a`/`15Ax`.
    DatRead { dst: Reg, ptr: Ptr, size: DatSize },

    // ----- P register and constants (2n, 3n, 8082n, 80Cn ..) -----
    /// P= n `2n`.
    PSet { n: u8 },
    /// LC(m) `3x n..n` (C) and LA(m) `8082x n..n` (A): load `nibbles`
    /// nibbles starting at nibble P, wrapping 15 -> 0. `value` holds the
    /// constant, nibble 0 = first opcode nibble.
    LoadConst { reg: Reg, nibbles: u8, value: u64 },
    /// C+P+1 `809`.
    CPlusPPlus1,
    /// C=P n `80Cn`.
    CEqP { n: u8 },
    /// P=C n `80Dn`.
    PEqC { n: u8 },
    /// CPEX n `80Fn`.
    CpEx { n: u8 },

    // ----- jumps, calls, conditional returns -----
    /// RTNC `400`.
    RtnC,
    /// RTNNC `500`.
    RtnNc,
    /// GOC `4yy`.
    Goc { target: u32 },
    /// GONC `5yy`.
    Gonc { target: u32 },
    /// GOTO `6yyy`.
    Goto { target: u32 },
    /// GOSUB `7yyy`.
    Gosub { target: u32 },
    /// GOLONG `8Cyyyy`.
    GoLong { target: u32 },
    /// GOVLNG `8Dyyyyy` (absolute).
    GoVLong { target: u32 },
    /// GOSUBL `8Eyyyy`.
    GosubL { target: u32 },
    /// GOSBVL `8Fyyyyy` (absolute).
    GosbVL { target: u32 },
    /// PC=A `81B2`, PC=C `81B3`.
    PcEqReg { reg: Reg },
    /// PC=(A) `808C`, PC=(C) `808E`.
    PcEqInd { reg: Reg },
    /// APCEX `81B6`, CPCEX `81B7`.
    RegPcEx { reg: Reg },
    /// A=PC `81B4`, C=PC `81B5`.
    RegEqPc { reg: Reg },
    /// NOP3 `820` (SASM encoding; see decode.rs on `420`).
    Nop3,
    /// NOP4 `6300`.
    Nop4,
    /// NOP5 `64000`.
    Nop5,

    // ----- chip interface (80x, 808x) -----
    /// OUT=CS `800`.
    OutCs,
    /// OUT=C `801`.
    OutC,
    /// A=IN `802`, C=IN `803`.
    In { dst: Reg },
    /// UNCNFG `804`.
    Uncnfg,
    /// CONFIG `805`.
    Config,
    /// C=ID `806`.
    CId,
    /// SHUTDN `807`.
    Shutdn,
    /// INTON `8080`.
    IntOn,
    /// RSI `80810`.
    Rsi,
    /// BUSCB `8083`.
    BusCb,
    /// INTOFF `808F`.
    IntOff,
    /// BUSCD `808D`.
    BusCd,
    /// RESET `80A`.
    Reset,
    /// BUSCC `80B`.
    BusCc,
    /// SREQ? `80E`.
    Sreq,

    // ----- bits and status -----
    /// ABIT=0 n `8084n`, CBIT=0 n `8088n`.
    BitClear { reg: Reg, bit: u8 },
    /// ABIT=1 n `8085n`, CBIT=1 n `8089n`.
    BitSet { reg: Reg, bit: u8 },
    /// HS=0 n `82n` (n != 0; `820` is NOP3). Masks 1/2/4/8/F have the
    /// names XM=0, SB=0, SR=0, MP=0, CLRHST.
    HsClear { mask: u8 },
    /// ST=0 n `84n`.
    StClear { bit: u8 },
    /// ST=1 n `85n`.
    StSet { bit: u8 },

    // ----- shifts and rotates (81x, 819f) -----
    /// rSLC `81x` (x 0-3): rotate whole register left one nibble.
    Slc { reg: Reg },
    /// rSRC `81x` (x 4-7): rotate whole register right one nibble.
    Src { reg: Reg },
    /// rSRB `81x` (x C-F), or rSRB.F fs `819fx`: shift right one bit.
    Srb { reg: Reg, field: Option<Field> },
    /// rSL fs: shift left one nibble (`Bb0`..`Bb3`, `F0`..`F3`).
    Sl { reg: Reg, field: Field },
    /// rSR fs: shift right one nibble (`Bb4`..`Bb7`, `F4`..`F7`).
    Sr { reg: Reg, field: Field },

    // ----- arithmetic with field (Aax, Abx, Bax, Bbx, Cx-Fx, 818f) -----
    /// dst=dst+src (src may equal dst: A=A+A).
    Add { dst: Reg, src: Reg, field: Field },
    /// dst=dst-src.
    Sub { dst: Reg, src: Reg, field: Field },
    /// dst=src-dst.
    SubRev { dst: Reg, src: Reg, field: Field },
    /// r=r+1.
    Inc { reg: Reg, field: Field },
    /// r=r-1.
    Dec { reg: Reg, field: Field },
    /// r=r+CON fs,n `818f0x` with n in 1..=16.
    AddConst { reg: Reg, field: Field, n: u8 },
    /// r=r-CON fs,n `818f8x` with n in 1..=16.
    SubConst { reg: Reg, field: Field, n: u8 },
    /// r=0.
    Zero { reg: Reg, field: Field },
    /// dst=src.
    Copy { dst: Reg, src: Reg, field: Field },
    /// rsEX: exchange.
    Exch { a: Reg, b: Reg, field: Field },
    /// r=-r: two's (ten's) complement.
    Neg { reg: Reg, field: Field },
    /// r=-r-1: one's (nine's) complement.
    Not { reg: Reg, field: Field },

    // ----- tests (each with its GOYES/RTNYES half) -----
    /// ?r=s .. ?r<=s fs: `8Axyy`/`8Bxyy` (A field), `9axyy`/`9bxyy`.
    TestCmp {
        op: Cmp,
        lhs: Reg,
        rhs: Reg,
        field: Field,
        yes: OnTrue,
    },
    /// ?r=0 / ?r#0 fs (`op` is `Eq` or `Ne`).
    TestZero {
        op: Cmp,
        reg: Reg,
        field: Field,
        yes: OnTrue,
    },
    /// ?HS=0 n `83nyy` (names ?XM=0, ?SB=0, ?SR=0, ?MP=0 for single bits).
    TestHs { mask: u8, yes: OnTrue },
    /// ?ST=0 n `86nyy` (`set` false), ?ST=1 n `87nyy`.
    TestSt { bit: u8, set: bool, yes: OnTrue },
    /// ?P# n `88nyy` (`eq` false), ?P= n `89nyy`.
    TestP { n: u8, eq: bool, yes: OnTrue },
    /// ?ABIT=0/1 n `8086nyy`/`8087nyy`, ?CBIT=0/1 n `808Anyy`/`808Bnyy`.
    TestBit {
        reg: Reg,
        bit: u8,
        set: bool,
        yes: OnTrue,
    },

    /// Undefined encoding: the nibbles read until the decoder knew the
    /// sequence is not an instruction (`len` of them are valid). Covers
    /// unassigned slots and the ARM-only Saturn+ extensions.
    Invalid { nibbles: [u8; 8], len: u8 },
}

impl Instruction {
    /// The test half of a test instruction, if this is one.
    pub const fn on_true(&self) -> Option<OnTrue> {
        match *self {
            Instruction::TestCmp { yes, .. }
            | Instruction::TestZero { yes, .. }
            | Instruction::TestHs { yes, .. }
            | Instruction::TestSt { yes, .. }
            | Instruction::TestP { yes, .. }
            | Instruction::TestBit { yes, .. } => Some(yes),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_ranges() {
        assert_eq!(Field::P.range(7), (7, 7));
        assert_eq!(Field::Wp.range(7), (0, 7));
        assert_eq!(Field::Wp.range(0x17), (0, 7));
        assert_eq!(Field::Xs.range(0), (2, 2));
        assert_eq!(Field::X.range(0), (0, 2));
        assert_eq!(Field::S.range(0), (15, 15));
        assert_eq!(Field::M.range(0), (3, 14));
        assert_eq!(Field::B.range(0), (0, 1));
        assert_eq!(Field::W.range(0), (0, 15));
        assert_eq!(Field::A.range(9), (0, 4));
    }

    #[test]
    fn field_codes() {
        assert_eq!(Field::from_a_code(0), Some(Field::P));
        assert_eq!(Field::from_a_code(7), Some(Field::W));
        assert_eq!(Field::from_a_code(8), None);
        assert_eq!(Field::from_f_code(0xF), Some(Field::A));
        assert_eq!(Field::from_f_code(0x8), None);
        assert_eq!(Field::from_f_code(0xE), None);
    }
}
