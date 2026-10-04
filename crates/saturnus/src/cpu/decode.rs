//! Saturn instruction decoder.
//!
//! Pure function from a nibble stream to an [`Instruction`]. No allocation,
//! no dependency on the bus: the caller supplies a `fetch` closure.
//!
//! # Encoding tree
//!
//! The first nibble picks a group; most groups refine on the next one or two
//! nibbles. Immediates are little-endian (first nibble = least significant).
//! Field codes: "a" = 0-7 (P WP XS X S M B W), "b" = a+8, "f" = a or F for
//! the A field. Sources: SASM manual sections 6 and 9 (authoritative),
//! Nickel's opcode list, the Fernandes/Rechlin tutorial ch. 33-52, Gariepy's
//! processor notes; wiki: hardware/saturn-cpu.
//!
//! ```text
//! 0 x      x=0..D, F: returns, SETHEX/DEC, RSTK, ST ops, P=P+-1
//!   E f x  AND (x 0-7) / OR (x 8-F) on field f; f 8-E undefined
//! 1 0 x    R=A (x 0-4) / R=C (x 8-C); other x undefined
//! 1 1 x    A=R / C=R            1 2 x  ARnEX / CRnEX
//! 1 3 x    D0/D1 <-> A/C: =A, EX, =AS, XS (by x, see `decode_13`)
//! 1 4 x    DAT transfer, A field (x 0-7) or B field (x 8-F)
//! 1 5 x y  DAT transfer: x 0-7 with field code y (0-7 only),
//!          x 8-F with y+1 nibbles
//! 1 6/7/8/C x   D0+ / D1+ / D0- / D1- by x+1
//! 1 9/A/B  D0= 2/4/5 nibbles;  1 D/E/F  D1= 2/4/5 nibbles
//! 2 n      P= n
//! 3 x ..   LC with x+1 nibbles
//! 4 yy     GOC (yy=00: RTNC);  5 yy  GONC (yy=00: RTNNC)
//! 6 yyy    GOTO (6300: NOP4, 64000: NOP5)
//! 7 yyy    GOSUB, relative to the end of the instruction
//! 8 0 x    chip interface; 808x is a second sub-tree (INTON, RSI, LA,
//!          BUSCB, ABIT/CBIT ops and tests, PC=(A)/(C), BUSCD, INTOFF)
//! 8 1 x    rotates (0-7), 818 add/sub constant, 819 SRB.F, 81A R<->A/C
//!          with field, 81B PC ops (2-7 only), right bit shifts (C-F)
//! 8 2 n    HS=0 n (820: NOP3)       8 3 n yy  ?HS=0 n
//! 8 4/5 n  ST=0/1 n                 8 6/7 n yy  ?ST=0/1 n
//! 8 8/9 n yy  ?P# / ?P= n           8 A/B x yy  register tests, A field
//! 8 C/E    GOLONG / GOSUBL (4-nibble relative)
//! 8 D/F    GOVLNG / GOSBVL (5-nibble absolute)
//! 9 f x yy register tests: f 0-7 equality group, f 8-F order group
//! A f x    f 0-7: add/dec group; f 8-F: zero/copy/exchange group
//! B f x    f 0-7: add/inc/sub group; f 8-F: shift/negate group
//! C/D/E/F x  the same four groups, A field
//! ```
//!
//! Branch bases (target = base + signed offset): GOC/GONC/GOTO/GOLONG count
//! from the first offset nibble (pc+1, pc+1, pc+1, pc+2); GOSUB/GOSUBL from
//! the end of the instruction (pc+4, pc+6), being the return address; test
//! GOYES from the offset field (pc+3, or pc+5 for the 808x bit tests).
//! (src: SASM manual 6.5-6.6 ranges; Gariepy 8086xyy "PC+5+yy").
//!
//! Source disagreements, SASM followed:
//! - NOP3 is `820` in SASM (an HS=0 with an empty mask) but `420` in
//!   Gariepy, Mastracci and the tutorial. `420` is GOC to the next
//!   instruction and behaves the same; we decode it as that GOC.
//! - `80B` is BUSCC on the HP48 (SASM). The ARM-based Saturn+ uses `80Bxx`
//!   as a prefix for its extensions (tutorial ch. 53-54); those are not
//!   decoded, `80B` stays a 3-nibble BUSCC.
//! - Undefined slots of the real Saturn are decoded as `Invalid` with the
//!   nibbles read so far: `105`-`107`, `10D`-`10F` (and the same in 11x,
//!   12x), `15xy` with x 0-7 and y 8-F, f codes 8-E in `0E`, `818`,
//!   `819`, `81A` (Saturn+ user fields F1-F7), `818f` op 4-7/C-F, `819f`
//!   op 4-F, `81Af` sub-op 3-F or register 5-7/D-F, `81B0`, `81B1`,
//!   `81B8`-`81BF` (Saturn+ uses some of these), and `8081x` with x != 0.
//!   What real hardware does with them is not documented in our sources.

use super::instr::{Cmp, DatSize, Field, Instruction, OnTrue, Ptr, Reg, Scratch};
use super::regs::ADDR_MASK;

/// A decoded instruction and its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoded {
    pub instr: Instruction,
    /// Length in nibbles, 1..=21.
    pub len: u8,
}

/// Decode the instruction at nibble address `pc`.
///
/// `fetch` returns the nibble at a 20-bit address (only the low 4 bits of
/// the result are used). Addresses wrap at 20 bits.
pub fn decode(fetch: impl FnMut(u32) -> u8, pc: u32) -> Decoded {
    let mut r = Reader {
        fetch,
        pc: pc & ADDR_MASK,
        len: 0,
        seen: [0; 8],
    };
    let instr = decode_inner(&mut r);
    Decoded { instr, len: r.len }
}

struct Reader<F> {
    fetch: F,
    pc: u32,
    len: u8,
    /// First nibbles read, for `Invalid`.
    seen: [u8; 8],
}

impl<F: FnMut(u32) -> u8> Reader<F> {
    fn nib(&mut self) -> u8 {
        let addr = self.pc.wrapping_add(u32::from(self.len)) & ADDR_MASK;
        let n = (self.fetch)(addr) & 0xF;
        if let Some(slot) = self.seen.get_mut(usize::from(self.len)) {
            *slot = n;
        }
        self.len += 1;
        n
    }

    /// Little-endian immediate of `count` nibbles (count <= 16).
    fn imm(&mut self, count: u8) -> u64 {
        let mut v = 0u64;
        for i in 0..count {
            v |= u64::from(self.nib()) << (4 * u32::from(i));
        }
        v
    }

    /// Address of the next nibble to be read.
    fn here(&self) -> u32 {
        self.pc.wrapping_add(u32::from(self.len)) & ADDR_MASK
    }

    /// Read a signed relative offset of `count` nibbles and resolve it
    /// against `base`. Returns `(offset, target)`.
    fn rel(&mut self, count: u8, base: u32) -> (i32, u32) {
        let raw = self.imm(count);
        let bits = 4 * u32::from(count);
        let shift = 64 - bits;
        // Sign-extend from `bits`; the cast reinterprets the bit pattern.
        let off = ((raw << shift) as i64 >> shift) as i32;
        (off, base.wrapping_add_signed(off) & ADDR_MASK)
    }

    fn invalid(&self) -> Instruction {
        Instruction::Invalid {
            nibbles: self.seen,
            len: self.len,
        }
    }

    /// GOYES/RTNYES half of a test: 2-nibble offset relative to its own
    /// address; 00 means RTNYES.
    fn on_true(&mut self) -> OnTrue {
        let base = self.here();
        let (off, target) = self.rel(2, base);
        if off == 0 {
            OnTrue::RtnYes
        } else {
            OnTrue::GoYes(target)
        }
    }
}

fn decode_inner<F: FnMut(u32) -> u8>(r: &mut Reader<F>) -> Instruction {
    let first = r.nib();
    match first {
        0x0 => decode_0(r),
        0x1 => decode_1(r),
        0x2 => Instruction::PSet { n: r.nib() },
        0x3 => {
            let n = r.nib() + 1;
            Instruction::LoadConst {
                reg: Reg::C,
                nibbles: n,
                value: r.imm(n),
            }
        }
        0x4 | 0x5 => {
            let carry = first == 4;
            let base = r.here();
            let (off, target) = r.rel(2, base);
            match (carry, off) {
                (true, 0) => Instruction::RtnC,
                (false, 0) => Instruction::RtnNc,
                (true, _) => Instruction::Goc { target },
                (false, _) => Instruction::Gonc { target },
            }
        }
        0x6 => {
            let base = r.here();
            let (off, target) = r.rel(3, base);
            match off {
                3 => Instruction::Nop4,
                // 64000: GOTO over one padding nibble, which SASM emits as 0.
                4 => {
                    let save = (r.len, r.seen);
                    if r.nib() == 0 {
                        Instruction::Nop5
                    } else {
                        (r.len, r.seen) = save;
                        Instruction::Goto { target }
                    }
                }
                _ => Instruction::Goto { target },
            }
        }
        0x7 => {
            let (off, _) = r.rel(3, 0);
            let target = r.here().wrapping_add_signed(off) & ADDR_MASK;
            Instruction::Gosub { target }
        }
        0x8 => decode_8(r),
        0x9 => {
            let f = r.nib();
            let x = r.nib();
            let field = field_a_or_b(f);
            let yes = r.on_true();
            if f < 8 {
                test_eq_group(x, field, yes)
            } else {
                test_order_group(x, field, yes)
            }
        }
        0xA => {
            let f = r.nib();
            let x = r.nib();
            let field = field_a_or_b(f);
            if f < 8 {
                arith_a_group(x, field)
            } else {
                arith_b_group(x, field)
            }
        }
        0xB => {
            let f = r.nib();
            let x = r.nib();
            let field = field_a_or_b(f);
            if f < 8 {
                arith_c_group(x, field)
            } else {
                arith_d_group(x, field)
            }
        }
        0xC => arith_a_group(r.nib(), Field::A),
        0xD => arith_b_group(r.nib(), Field::A),
        0xE => arith_c_group(r.nib(), Field::A),
        _ => arith_d_group(r.nib(), Field::A),
    }
}

/// Field from an "a" or "b" code (the low three bits select the field).
fn field_a_or_b(code: u8) -> Field {
    match code & 7 {
        0 => Field::P,
        1 => Field::Wp,
        2 => Field::Xs,
        3 => Field::X,
        4 => Field::S,
        5 => Field::M,
        6 => Field::B,
        _ => Field::W,
    }
}

/// Register pair for x in 0..=3 of the add, AND/OR, equality and copy
/// tables: (A,B) (B,C) (C,A) (D,C) as `(dst, src)`.
const fn pair_x(x: u8) -> (Reg, Reg) {
    match x & 3 {
        0 => (Reg::A, Reg::B),
        1 => (Reg::B, Reg::C),
        2 => (Reg::C, Reg::A),
        _ => (Reg::D, Reg::C),
    }
}

/// The swapped pairs used by x in 4..=7 of the AND/OR table and 8..=B of
/// the add/copy/subtract tables: (B,A) (C,B) (A,C) (C,D) as `(dst, src)`.
const fn pair_y(x: u8) -> (Reg, Reg) {
    match x & 3 {
        0 => (Reg::B, Reg::A),
        1 => (Reg::C, Reg::B),
        2 => (Reg::A, Reg::C),
        _ => (Reg::C, Reg::D),
    }
}

fn decode_0<F: FnMut(u32) -> u8>(r: &mut Reader<F>) -> Instruction {
    match r.nib() {
        0x0 => Instruction::RtnSxm,
        0x1 => Instruction::Rtn,
        0x2 => Instruction::RtnSc,
        0x3 => Instruction::RtnCc,
        0x4 => Instruction::SetHex,
        0x5 => Instruction::SetDec,
        0x6 => Instruction::RstkEqC,
        0x7 => Instruction::CEqRstk,
        0x8 => Instruction::ClrSt,
        0x9 => Instruction::CEqSt,
        0xA => Instruction::StEqC,
        0xB => Instruction::CStEx,
        0xC => Instruction::PInc,
        0xD => Instruction::PDec,
        0xE => {
            let Some(field) = Field::from_f_code(r.nib()) else {
                return r.invalid();
            };
            let x = r.nib();
            let (dst, src) = if x & 4 == 0 { pair_x(x) } else { pair_y(x) };
            if x < 8 {
                Instruction::And { dst, src, field }
            } else {
                Instruction::Or { dst, src, field }
            }
        }
        _ => Instruction::Rti,
    }
}

/// Register (A or C) and scratch register from the low nibble of 10x/11x/
/// 12x and 81Af.x: 0-4 = A with R0-R4, 8-C = C with R0-R4.
fn scratch_operand(x: u8) -> Option<(Reg, Scratch)> {
    let reg = if x & 8 == 0 { Reg::A } else { Reg::C };
    Scratch::from_index(x & 7).map(|ss| (reg, ss))
}

fn decode_1<F: FnMut(u32) -> u8>(r: &mut Reader<F>) -> Instruction {
    let op = r.nib();
    match op {
        0x0..=0x2 => {
            let Some((reg, ss)) = scratch_operand(r.nib()) else {
                return r.invalid();
            };
            scratch_op(op, reg, ss, None)
        }
        0x3 => decode_13(r.nib()),
        0x4 => {
            let x = r.nib();
            let field = if x < 8 { Field::A } else { Field::B };
            dat_op(x, DatSize::Field(field))
        }
        0x5 => {
            let x = r.nib();
            let y = r.nib();
            if x < 8 {
                match Field::from_a_code(y) {
                    Some(field) => dat_op(x, DatSize::Field(field)),
                    None => r.invalid(),
                }
            } else {
                dat_op(x, DatSize::Nibbles(y + 1))
            }
        }
        0x6 => Instruction::PtrAdd {
            ptr: Ptr::D0,
            n: r.nib() + 1,
        },
        0x7 => Instruction::PtrAdd {
            ptr: Ptr::D1,
            n: r.nib() + 1,
        },
        0x8 => Instruction::PtrSub {
            ptr: Ptr::D0,
            n: r.nib() + 1,
        },
        0xC => Instruction::PtrSub {
            ptr: Ptr::D1,
            n: r.nib() + 1,
        },
        _ => {
            // 9/A/B: D0, D/E/F: D1; low two bits 1/2/3 -> 2/4/5 nibbles.
            let ptr = if op < 0xC { Ptr::D0 } else { Ptr::D1 };
            let nibbles = match op & 3 {
                1 => 2,
                2 => 4,
                _ => 5,
            };
            Instruction::PtrLoad {
                ptr,
                nibbles,
                value: r.imm(nibbles) as u32,
            }
        }
    }
}

/// 10x/11x/12x and 81Af{0,1,2}x: 0 = ss=reg, 1 = reg=ss, 2 = exchange.
fn scratch_op(op: u8, reg: Reg, ss: Scratch, field: Option<Field>) -> Instruction {
    match op {
        0 => Instruction::ScratchFromReg {
            ss,
            src: reg,
            field,
        },
        1 => Instruction::RegFromScratch {
            dst: reg,
            ss,
            field,
        },
        _ => Instruction::RegScratchEx { reg, ss, field },
    }
}

/// 13x: bit 0 = D0/D1, bit 1 = copy/exchange, bit 2 = A/C, bit 3 = short
/// (4-nibble) form.
fn decode_13(x: u8) -> Instruction {
    let ptr = if x & 1 == 0 { Ptr::D0 } else { Ptr::D1 };
    let reg = if x & 4 == 0 { Reg::A } else { Reg::C };
    let short = x & 8 != 0;
    if x & 2 == 0 {
        Instruction::PtrFromReg {
            ptr,
            src: reg,
            short,
        }
    } else {
        Instruction::PtrRegEx { ptr, reg, short }
    }
}

/// 14x/15x transfers: bit 0 = D0/D1, bit 1 = write/read, bit 2 = A/C.
fn dat_op(x: u8, size: DatSize) -> Instruction {
    let ptr = if x & 1 == 0 { Ptr::D0 } else { Ptr::D1 };
    let reg = if x & 4 == 0 { Reg::A } else { Reg::C };
    if x & 2 == 0 {
        Instruction::DatWrite {
            ptr,
            src: reg,
            size,
        }
    } else {
        Instruction::DatRead {
            dst: reg,
            ptr,
            size,
        }
    }
}

fn decode_8<F: FnMut(u32) -> u8>(r: &mut Reader<F>) -> Instruction {
    let op = r.nib();
    match op {
        0x0 => decode_80(r),
        0x1 => decode_81(r),
        0x2 => match r.nib() {
            0 => Instruction::Nop3,
            mask => Instruction::HsClear { mask },
        },
        0x3 => {
            let mask = r.nib();
            let yes = r.on_true();
            Instruction::TestHs { mask, yes }
        }
        0x4 => Instruction::StClear { bit: r.nib() },
        0x5 => Instruction::StSet { bit: r.nib() },
        0x6 | 0x7 => {
            let set = op == 0x7;
            let bit = r.nib();
            let yes = r.on_true();
            Instruction::TestSt { bit, set, yes }
        }
        0x8 | 0x9 => {
            let eq = op == 0x9;
            let n = r.nib();
            let yes = r.on_true();
            Instruction::TestP { n, eq, yes }
        }
        0xA => {
            let x = r.nib();
            let yes = r.on_true();
            test_eq_group(x, Field::A, yes)
        }
        0xB => {
            let x = r.nib();
            let yes = r.on_true();
            test_order_group(x, Field::A, yes)
        }
        0xC => {
            let base = r.here();
            let (_, target) = r.rel(4, base);
            Instruction::GoLong { target }
        }
        0xE => {
            let (off, _) = r.rel(4, 0);
            let target = r.here().wrapping_add_signed(off) & ADDR_MASK;
            Instruction::GosubL { target }
        }
        _ => {
            // 8D GOVLNG, 8F GOSBVL: absolute.
            let target = r.imm(5) as u32;
            if op == 0xD {
                Instruction::GoVLong { target }
            } else {
                Instruction::GosbVL { target }
            }
        }
    }
}

fn decode_80<F: FnMut(u32) -> u8>(r: &mut Reader<F>) -> Instruction {
    match r.nib() {
        0x0 => Instruction::OutCs,
        0x1 => Instruction::OutC,
        0x2 => Instruction::In { dst: Reg::A },
        0x3 => Instruction::In { dst: Reg::C },
        0x4 => Instruction::Uncnfg,
        0x5 => Instruction::Config,
        0x6 => Instruction::CId,
        0x7 => Instruction::Shutdn,
        0x8 => decode_808(r),
        0x9 => Instruction::CPlusPPlus1,
        0xA => Instruction::Reset,
        0xB => Instruction::BusCc,
        0xC => Instruction::CEqP { n: r.nib() },
        0xD => Instruction::PEqC { n: r.nib() },
        0xE => Instruction::Sreq,
        _ => Instruction::CpEx { n: r.nib() },
    }
}

fn decode_808<F: FnMut(u32) -> u8>(r: &mut Reader<F>) -> Instruction {
    let op = r.nib();
    let reg = if op < 8 { Reg::A } else { Reg::C };
    match op {
        0x0 => Instruction::IntOn,
        0x1 => match r.nib() {
            0 => Instruction::Rsi,
            _ => r.invalid(),
        },
        0x2 => {
            let n = r.nib() + 1;
            Instruction::LoadConst {
                reg: Reg::A,
                nibbles: n,
                value: r.imm(n),
            }
        }
        0x3 => Instruction::BusCb,
        0x4 | 0x8 => Instruction::BitClear { reg, bit: r.nib() },
        0x5 | 0x9 => Instruction::BitSet { reg, bit: r.nib() },
        0x6 | 0x7 | 0xA | 0xB => {
            let set = op & 1 == 1;
            let bit = r.nib();
            let yes = r.on_true();
            Instruction::TestBit { reg, bit, set, yes }
        }
        0xC => Instruction::PcEqInd { reg: Reg::A },
        0xD => Instruction::BusCd,
        0xE => Instruction::PcEqInd { reg: Reg::C },
        _ => Instruction::IntOff,
    }
}

fn decode_81<F: FnMut(u32) -> u8>(r: &mut Reader<F>) -> Instruction {
    let op = r.nib();
    match op {
        0x0..=0x3 => Instruction::Slc {
            reg: Reg::from_index(op),
        },
        0x4..=0x7 => Instruction::Src {
            reg: Reg::from_index(op),
        },
        0x8 => {
            let Some(field) = Field::from_f_code(r.nib()) else {
                return r.invalid();
            };
            let x = r.nib();
            if x & 4 != 0 {
                return r.invalid();
            }
            let reg = Reg::from_index(x);
            let n = r.nib() + 1;
            if x < 8 {
                Instruction::AddConst { reg, field, n }
            } else {
                Instruction::SubConst { reg, field, n }
            }
        }
        0x9 => {
            let Some(field) = Field::from_f_code(r.nib()) else {
                return r.invalid();
            };
            let x = r.nib();
            if x > 3 {
                return r.invalid();
            }
            Instruction::Srb {
                reg: Reg::from_index(x),
                field: Some(field),
            }
        }
        0xA => {
            let Some(field) = Field::from_f_code(r.nib()) else {
                return r.invalid();
            };
            let sub = r.nib();
            if sub > 2 {
                return r.invalid();
            }
            match scratch_operand(r.nib()) {
                Some((reg, ss)) => scratch_op(sub, reg, ss, Some(field)),
                None => r.invalid(),
            }
        }
        0xB => {
            let x = r.nib();
            let reg = if x & 1 == 0 { Reg::A } else { Reg::C };
            match x {
                0x2 | 0x3 => Instruction::PcEqReg { reg },
                0x4 | 0x5 => Instruction::RegEqPc { reg },
                0x6 | 0x7 => Instruction::RegPcEx { reg },
                _ => r.invalid(),
            }
        }
        _ => Instruction::Srb {
            reg: Reg::from_index(op),
            field: None,
        },
    }
}

/// 8Ax / 9ax tests: equality (0-7, pairs), zero tests (8-F).
fn test_eq_group(x: u8, field: Field, yes: OnTrue) -> Instruction {
    let op = if x & 4 == 0 { Cmp::Eq } else { Cmp::Ne };
    if x < 8 {
        // Equality pairs: (A,B) (B,C) (A,C) (C,D).
        let (lhs, rhs) = match x & 3 {
            0 => (Reg::A, Reg::B),
            1 => (Reg::B, Reg::C),
            2 => (Reg::A, Reg::C),
            _ => (Reg::C, Reg::D),
        };
        Instruction::TestCmp {
            op,
            lhs,
            rhs,
            field,
            yes,
        }
    } else {
        Instruction::TestZero {
            op,
            reg: Reg::from_index(x),
            field,
            yes,
        }
    }
}

/// 8Bx / 9bx tests: > (0-3), < (4-7), >= (8-B), <= (C-F) on the pairs
/// (A,B) (B,C) (C,A) (D,C).
fn test_order_group(x: u8, field: Field, yes: OnTrue) -> Instruction {
    let op = match x >> 2 {
        0 => Cmp::Gt,
        1 => Cmp::Lt,
        2 => Cmp::Ge,
        _ => Cmp::Le,
    };
    let (lhs, rhs) = pair_x(x);
    Instruction::TestCmp {
        op,
        lhs,
        rhs,
        field,
        yes,
    }
}

/// Aax / Cx: add (0-B) and decrement (C-F).
fn arith_a_group(x: u8, field: Field) -> Instruction {
    match x {
        0x0..=0x3 => {
            let (dst, src) = pair_x(x);
            Instruction::Add { dst, src, field }
        }
        0x4..=0x7 => {
            let reg = Reg::from_index(x);
            Instruction::Add {
                dst: reg,
                src: reg,
                field,
            }
        }
        0x8..=0xB => {
            let (dst, src) = pair_y(x);
            Instruction::Add { dst, src, field }
        }
        _ => Instruction::Dec {
            reg: Reg::from_index(x),
            field,
        },
    }
}

/// Abx / Dx: zero (0-3), copy (4-B), exchange (C-F).
fn arith_b_group(x: u8, field: Field) -> Instruction {
    match x {
        0x0..=0x3 => Instruction::Zero {
            reg: Reg::from_index(x),
            field,
        },
        0x4..=0x7 => {
            let (dst, src) = pair_x(x);
            Instruction::Copy { dst, src, field }
        }
        0x8..=0xB => {
            let (dst, src) = pair_y(x);
            Instruction::Copy { dst, src, field }
        }
        _ => {
            // ABEX BCEX ACEX CDEX
            let (a, b) = match x & 3 {
                0 => (Reg::A, Reg::B),
                1 => (Reg::B, Reg::C),
                2 => (Reg::A, Reg::C),
                _ => (Reg::C, Reg::D),
            };
            Instruction::Exch { a, b, field }
        }
    }
}

/// Bax / Ex: subtract (0-3, 8-B), increment (4-7), reverse subtract (C-F).
fn arith_c_group(x: u8, field: Field) -> Instruction {
    match x {
        0x0..=0x3 => {
            let (dst, src) = pair_x(x);
            Instruction::Sub { dst, src, field }
        }
        0x4..=0x7 => Instruction::Inc {
            reg: Reg::from_index(x),
            field,
        },
        0x8..=0xB => {
            let (dst, src) = pair_y(x);
            Instruction::Sub { dst, src, field }
        }
        _ => {
            // A=B-A B=C-B C=A-C D=C-D
            let (dst, src) = pair_x(x);
            Instruction::SubRev { dst, src, field }
        }
    }
}

/// Bbx / Fx: nibble shift left (0-3), right (4-7), negate (8-B),
/// complement (C-F).
fn arith_d_group(x: u8, field: Field) -> Instruction {
    let reg = Reg::from_index(x);
    match x >> 2 {
        0 => Instruction::Sl { reg, field },
        1 => Instruction::Sr { reg, field },
        2 => Instruction::Neg { reg, field },
        _ => Instruction::Not { reg, field },
    }
}

#[cfg(test)]
#[path = "decode_tests.rs"]
mod tests;
