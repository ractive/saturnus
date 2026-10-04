//! Saturn ALU: pure functions over 64-bit register values.
//!
//! wiki: hardware/saturn-cpu; src: SASM manual 2.6, 2.7 and 8 (Mnemonic
//! Dictionary).
//!
//! Every field operation takes an inclusive nibble range `(lo, hi)` with
//! `0 <= lo <= hi <= 15` (see `regs::field_mask`). Nibbles outside the range
//! are never modified, except by the add/subtract-constant overrun described
//! at [`add_const`]. Functions return the new register value plus a carry
//! (or Sticky Bit) result; the executor decides which flag to update, as
//! documented on each function.
//!
//! Mode dependence (src: SASM manual 2.7): only [`add`], [`sub`], [`inc`],
//! [`dec`], [`neg`] and [`not`] honour HEX/DEC mode. The pointer and P
//! arithmetic ([`ptr_add`], [`ptr_sub`], [`p_inc`], [`p_dec`],
//! [`c_plus_p_plus_1`]) and the constant forms ([`add_const`],
//! [`sub_const`]) are always hexadecimal. Logic, shifts and comparisons have
//! no notion of mode.
//!
//! BCD with invalid digits (assumption, no source describes it): the ALU is
//! modelled nibble-serially. In DEC mode each digit is added in binary
//! (`s = a + b + carry_in`); if `s > 9` the digit becomes `(s + 6) & 0xF` and
//! a carry goes to the next digit. Subtraction computes `d = a - b - borrow`;
//! if `d < 0` the digit becomes `(d + 10) & 0xF` (equivalently a -6
//! correction modulo 16) and a borrow goes to the next digit. For valid
//! digits (0-9) this is exact BCD; for A-F it is deterministic.

use super::regs::{ADDR_MASK, Mode, field_get, field_mask, field_set, field_width, get_nibble};

/// Digit-serial add of `b` to `a` over `lo..=hi` with carry-in.
fn add_serial(a: u64, b: u64, lo: u8, hi: u8, mode: Mode, carry_in: bool) -> (u64, bool) {
    let (lo, hi) = (lo & 0xF, hi & 0xF);
    let mut result = a;
    let mut carry = carry_in;
    for i in lo..=hi {
        let s = get_nibble(a, i) + get_nibble(b, i) + u8::from(carry);
        let (digit, c) = match mode {
            Mode::Hex => (s & 0xF, s > 0xF),
            Mode::Dec if s > 9 => ((s + 6) & 0xF, true),
            Mode::Dec => (s, false),
        };
        carry = c;
        result = super::regs::set_nibble(result, i, digit);
    }
    (result, carry)
}

/// Digit-serial subtract of `b` from `a` over `lo..=hi` with borrow-in.
fn sub_serial(a: u64, b: u64, lo: u8, hi: u8, mode: Mode, borrow_in: bool) -> (u64, bool) {
    let (lo, hi) = (lo & 0xF, hi & 0xF);
    let mut result = a;
    let mut borrow = borrow_in;
    for i in lo..=hi {
        let d = i16::from(get_nibble(a, i)) - i16::from(get_nibble(b, i)) - i16::from(borrow);
        let (digit, br) = if d < 0 {
            let radix = match mode {
                Mode::Hex => 16,
                Mode::Dec => 10,
            };
            (((d + radix) & 0xF) as u8, true)
        } else {
            ((d & 0xF) as u8, false)
        };
        borrow = br;
        result = super::regs::set_nibble(result, i, digit);
    }
    (result, borrow)
}

/// `a(fs) = a(fs) + b(fs)`. Returns the new `a` and the carry: set when the
/// sum overflows the field (hex: wraps modulo 16^n; dec: BCD carry out of the
/// top digit of the field). Adjusts Carry.
/// src: SASM manual 8 (A=A+B), 2.6.
pub fn add(a: u64, b: u64, lo: u8, hi: u8, mode: Mode) -> (u64, bool) {
    add_serial(a, b, lo, hi, mode, false)
}

/// `a(fs) = a(fs) - b(fs)`. Carry is set on borrow out of the field.
/// Adjusts Carry. src: SASM manual 8 (A=A-B), 2.6.
pub fn sub(a: u64, b: u64, lo: u8, hi: u8, mode: Mode) -> (u64, bool) {
    sub_serial(a, b, lo, hi, mode, false)
}

/// `a(fs) = a(fs) + 1`. Adjusts Carry. src: SASM manual 8 (A=A+1).
pub fn inc(a: u64, lo: u8, hi: u8, mode: Mode) -> (u64, bool) {
    add_serial(a, 0, lo, hi, mode, true)
}

/// `a(fs) = a(fs) - 1`. Adjusts Carry (set on borrow).
/// src: SASM manual 8 (A=A-1).
pub fn dec(a: u64, lo: u8, hi: u8, mode: Mode) -> (u64, bool) {
    sub_serial(a, 0, lo, hi, mode, true)
}

/// `r = r + CON fs, c` (818 group), `c` in 1..=16. Always hexadecimal,
/// whatever SETDEC says (src: SASM manual 2.7; Emu48 change log SP10 via
/// wiki: questions/dec-mode-constant-bug). Returns the value and the carry.
///
/// Multi-nibble fields: plain hex add within the field, carry on wrap.
///
/// Single-nibble fields (`lo == hi`: P, XS, S, and WP when P = 0) overrun:
/// the add runs over all 16 nibbles of the register, starting at nibble `lo`
/// and wrapping from nibble 15 round to nibble 0, as a nibble-serial ALU
/// with a 16-nibble count would. The carry is the carry out of that whole
/// 16-nibble chain (i.e. out of nibble `lo - 1` mod 16). This reproduces
/// both worked examples in the tutorial (p. 59: A=FFFFFFFFFFFFFEF2,
/// A=A+4 XS gives 00000000000002F3, the carry wrapping into nibble 0).
/// SASM itself refuses these fields for the constant forms ("rfs":
/// S, P, WP and XS not allowed), so HP ROM code should never execute them.
// TODO(questions/dec-mode-constant-bug): behaviour chosen from Emu48 change log + tutorial agreement on overrun; verify by ROM trace in iteration 2
pub fn add_const(a: u64, c: u8, lo: u8, hi: u8) -> (u64, bool) {
    let c = u64::from(c);
    let (lo, hi) = (lo & 0xF, hi & 0xF);
    if lo == hi {
        let rot = 4 * u32::from(lo);
        let (sum, carry) = a.rotate_right(rot).overflowing_add(c);
        return (sum.rotate_left(rot), carry);
    }
    let v = u128::from(field_get(a, lo, hi)) + u128::from(c);
    let bits = 4 * field_width(lo, hi);
    let carry = (v >> bits) != 0;
    (field_set(a, lo, hi, v as u64), carry)
}

/// `r = r - CON fs, c` (818 group), `c` in 1..=16. Always hexadecimal;
/// carry on borrow. Single-nibble fields overrun exactly like [`add_const`]
/// (the borrow ripples through all 16 nibbles circularly from `lo`).
// TODO(questions/dec-mode-constant-bug): behaviour chosen from Emu48 change log + tutorial agreement on overrun; verify by ROM trace in iteration 2
pub fn sub_const(a: u64, c: u8, lo: u8, hi: u8) -> (u64, bool) {
    let c = u64::from(c);
    let (lo, hi) = (lo & 0xF, hi & 0xF);
    if lo == hi {
        let rot = 4 * u32::from(lo);
        let (diff, borrow) = a.rotate_right(rot).overflowing_sub(c);
        return (diff.rotate_left(rot), borrow);
    }
    let v = field_get(a, lo, hi);
    let borrow = v < c;
    (field_set(a, lo, hi, v.wrapping_sub(c)), borrow)
}

/// `a(fs) = -a(fs)`: two's complement in HEX, ten's complement in DEC.
/// Carry is set if the field was non-zero, else cleared.
/// src: SASM manual 8 (A=-A), 6.11.10 (r=-r).
pub fn neg(a: u64, lo: u8, hi: u8, mode: Mode) -> (u64, bool) {
    let (v, _) = sub_serial(a & !field_mask(lo, hi), a, lo, hi, mode, false);
    (v, nonzero(a, lo, hi))
}

/// `a(fs) = -a(fs) - 1`: one's complement in HEX, nine's complement in DEC
/// (each digit `d` becomes `(9 - d) & 0xF`, so invalid digits A-F map to
/// F-A; assumption). Carry is always cleared, so the returned flag is always
/// `false`. src: SASM manual 8 (A=-A-1) "Carry is always cleared".
/// The tutorial (p. 65) instead says carry is set when the field is zero;
/// we follow the SASM manual.
pub fn not(a: u64, lo: u8, hi: u8, mode: Mode) -> (u64, bool) {
    let v = match mode {
        Mode::Hex => a ^ field_mask(lo, hi),
        Mode::Dec => {
            let (lo, hi) = (lo & 0xF, hi & 0xF);
            let mut r = a;
            for i in lo..=hi {
                r = super::regs::set_nibble(r, i, 9u8.wrapping_sub(get_nibble(a, i)) & 0xF);
            }
            r
        }
    };
    (v, false)
}

/// `a(fs) = a(fs) & b(fs)`. Carry is not affected. src: SASM manual 8 (A=A&B).
pub fn and(a: u64, b: u64, lo: u8, hi: u8) -> u64 {
    a & (b | !field_mask(lo, hi))
}

/// `a(fs) = a(fs) ! b(fs)` (OR). Carry is not affected.
/// src: SASM manual 8 (A=A!B).
pub fn or(a: u64, b: u64, lo: u8, hi: u8) -> u64 {
    a | (b & field_mask(lo, hi))
}

/// Exchange field `lo..=hi` of `a` and `b`; returns `(new_a, new_b)`.
/// Carry is not affected. src: SASM manual 8 (ABEX).
pub fn exchange(a: u64, b: u64, lo: u8, hi: u8) -> (u64, u64) {
    let m = field_mask(lo, hi);
    ((a & !m) | (b & m), (b & !m) | (a & m))
}

/// `dst(fs) = src(fs)`. Carry is not affected. src: SASM manual 8 (A=B).
pub fn copy(dst: u64, src: u64, lo: u8, hi: u8) -> u64 {
    let m = field_mask(lo, hi);
    (dst & !m) | (src & m)
}

/// `a(fs) = 0`. Carry is not affected. src: SASM manual 8 (A=0).
pub fn clear(a: u64, lo: u8, hi: u8) -> u64 {
    a & !field_mask(lo, hi)
}

/// Nibble shift left within the field (ASL fs). The top nibble of the field
/// is lost, a zero enters at the bottom. Returns `(value, sb_set)`;
/// `sb_set` is always `false`: "The Sticky Bit (SB) is not affected"
/// (src: SASM manual 8 (ASL); Gariepy, SHIFT: SB on SRN/SRB only). The
/// tutorial (p. 69) says SB is set in either direction; we follow SASM.
pub fn shl_nibble(a: u64, lo: u8, hi: u8) -> (u64, bool) {
    let v = field_get(a, lo, hi);
    (field_set(a, lo, hi, v << 4), false)
}

/// Nibble shift right within the field (ASR fs). The bottom nibble of the
/// field is lost, a zero enters at the top. `sb_set` is true if the lost
/// nibble was non-zero. Carry is not affected. src: SASM manual 8 (ASR), 2.6.
pub fn shr_nibble(a: u64, lo: u8, hi: u8) -> (u64, bool) {
    let v = field_get(a, lo, hi);
    (field_set(a, lo, hi, v >> 4), v & 0xF != 0)
}

/// Circular nibble shift left over the field (ASLC uses `(0, 15)`).
/// `sb_set` is always `false`: "The Sticky Bit (SB) is not affected"
/// (src: SASM manual 8 (ASLC); Gariepy, ROTATE: "RRN only"). The wiki notes
/// an Emu48 change saying rotates update SB (SP35) without naming the
/// direction; we follow SASM.
pub fn rol_nibble(a: u64, lo: u8, hi: u8) -> (u64, bool) {
    let n = field_width(lo, hi);
    if n == 0 {
        return (a, false);
    }
    let v = field_get(a, lo, hi);
    let top = v >> (4 * (n - 1));
    (field_set(a, lo, hi, (v << 4) | top), false)
}

/// Circular nibble shift right over the field (ASRC uses `(0, 15)`): the
/// bottom nibble moves to the top. `sb_set` is true if that nibble was
/// non-zero. src: SASM manual 8 (ASRC), 6.11.9 (circular within the selected
/// field). The tutorial (p. 70) says "only if null", which contradicts SASM
/// and Gariepy; we follow SASM.
pub fn ror_nibble(a: u64, lo: u8, hi: u8) -> (u64, bool) {
    let n = field_width(lo, hi);
    if n == 0 {
        return (a, false);
    }
    let v = field_get(a, lo, hi);
    let low = v & 0xF;
    (
        field_set(a, lo, hi, (v >> 4) | (low << (4 * (n - 1)))),
        low != 0,
    )
}

/// One-bit shift right over the field (ASRB uses `(0, 15)`, ASRB.F fs a
/// field). A zero enters at the top; `sb_set` is true if the lost bit was 1.
/// Carry is not affected. src: SASM manual 8 (ASRB), 9 (ASRB.F); tutorial
/// p. 70.
pub fn shr_bit(a: u64, lo: u8, hi: u8) -> (u64, bool) {
    let v = field_get(a, lo, hi);
    (field_set(a, lo, hi, v >> 1), v & 1 != 0)
}

/// `?a=b fs`. Unsigned field comparisons; the executor sets carry to the
/// result. src: SASM manual 2.6, 8 (?A=B etc.).
pub fn eq(a: u64, b: u64, lo: u8, hi: u8) -> bool {
    field_get(a, lo, hi) == field_get(b, lo, hi)
}

/// `?a#b fs`.
pub fn ne(a: u64, b: u64, lo: u8, hi: u8) -> bool {
    !eq(a, b, lo, hi)
}

/// `?a=0 fs`.
pub fn zero(a: u64, lo: u8, hi: u8) -> bool {
    field_get(a, lo, hi) == 0
}

/// `?a#0 fs`.
pub fn nonzero(a: u64, lo: u8, hi: u8) -> bool {
    !zero(a, lo, hi)
}

/// `?a>b fs` (unsigned).
pub fn gt(a: u64, b: u64, lo: u8, hi: u8) -> bool {
    field_get(a, lo, hi) > field_get(b, lo, hi)
}

/// `?a>=b fs` (unsigned).
pub fn ge(a: u64, b: u64, lo: u8, hi: u8) -> bool {
    field_get(a, lo, hi) >= field_get(b, lo, hi)
}

/// `?a<b fs` (unsigned).
pub fn lt(a: u64, b: u64, lo: u8, hi: u8) -> bool {
    field_get(a, lo, hi) < field_get(b, lo, hi)
}

/// `?a<=b fs` (unsigned).
pub fn le(a: u64, b: u64, lo: u8, hi: u8) -> bool {
    field_get(a, lo, hi) <= field_get(b, lo, hi)
}

/// Bit `n` (0..=15, masked) of the register: `?ABIT=1 n` / `?CBIT=1 n`.
/// src: SASM manual 6.8, 9 (?ABIT=0 d, 8086n).
pub fn bit_get(a: u64, n: u8) -> bool {
    (a >> (n & 0xF)) & 1 != 0
}

/// Set (`value = true`) or clear bit `n` (0..=15, masked): `ABIT=1 n` /
/// `ABIT=0 n`. Carry is not affected. src: SASM manual 9 (ABIT=0 d, 8084n).
pub fn bit_set(a: u64, n: u8, value: bool) -> u64 {
    let m = 1u64 << (n & 0xF);
    if value { a | m } else { a & !m }
}

/// `D0=D0+ n` / `D1=D1+ n`, `n` in 1..=16. Always hex; 20-bit wrap sets
/// carry. src: SASM manual 8 (D0=D0+), 2.7.
pub fn ptr_add(d: u32, n: u8) -> (u32, bool) {
    let s = (d & ADDR_MASK) + u32::from(n);
    (s & ADDR_MASK, s > ADDR_MASK)
}

/// `D0=D0- n` / `D1=D1- n`, `n` in 1..=16. Always hex; borrow below zero
/// sets carry and wraps to 20 bits. src: SASM manual 8 (D0=D0-), 2.7.
pub fn ptr_sub(d: u32, n: u8) -> (u32, bool) {
    let d = d & ADDR_MASK;
    let n = u32::from(n);
    (d.wrapping_sub(n) & ADDR_MASK, d < n)
}

/// `P=P+1`: wraps F to 0, carry set on wrap. Always hex.
/// src: SASM manual 8 (P=P+1).
pub fn p_inc(p: u8) -> (u8, bool) {
    let p = p & 0xF;
    ((p + 1) & 0xF, p == 0xF)
}

/// `P=P-1`: wraps 0 to F, carry set on wrap. Always hex.
/// src: SASM manual 8 (P=P-1).
pub fn p_dec(p: u8) -> (u8, bool) {
    let p = p & 0xF;
    (p.wrapping_sub(1) & 0xF, p == 0)
}

/// `C+P+1`: C(A) = C(A) + P + 1, always hex, carry on 20-bit wrap; the rest
/// of C is unchanged. src: SASM manual 8 (C+P+1).
pub fn c_plus_p_plus_1(c: u64, p: u8) -> (u64, bool) {
    let v = field_get(c, 0, 4) + u64::from(p & 0xF) + 1;
    (field_set(c, 0, 4, v), v > u64::from(ADDR_MASK))
}

#[cfg(test)]
// Literal groupings mark field boundaries (e.g. 0xAB_1234_5 = A field 12345).
#[allow(clippy::unusual_byte_groupings)]
mod tests {
    use super::*;

    // Field ranges used in tests (src: SASM manual 2.3).
    const W: (u8, u8) = (0, 15);
    const A: (u8, u8) = (0, 4);
    const B: (u8, u8) = (0, 1);
    const X: (u8, u8) = (0, 2);
    const XS: (u8, u8) = (2, 2);
    const S: (u8, u8) = (15, 15);
    const M: (u8, u8) = (3, 14);

    use Mode::{Dec, Hex};

    #[test]
    fn hex_add_fields() {
        // W
        assert_eq!(add(1, 2, W.0, W.1, Hex), (3, false));
        assert_eq!(add(u64::MAX, 1, W.0, W.1, Hex), (0, true));
        // A: high nibbles of a untouched, high nibbles of b ignored.
        let a = 0x1234_5678_9AB0_0001;
        let b = 0xFFFF_FFFF_FFFF_FFFF;
        assert_eq!(add(a, b, A.0, A.1, Hex), (0x1234_5678_9AB0_0000, true));
        assert_eq!(
            add(a, 0x0000_0000_0001_0000, A.0, A.1, Hex),
            (0x1234_5678_9AB1_0001, false)
        );
        // B
        assert_eq!(add(0x1FF, 0x01, B.0, B.1, Hex), (0x100, true));
        assert_eq!(add(0x12E, 0x01, B.0, B.1, Hex), (0x12F, false));
        // X
        assert_eq!(add(0xA_FFF, 0x001, X.0, X.1, Hex), (0xA_000, true));
        assert_eq!(add(0x123, 0x111, X.0, X.1, Hex), (0x234, false));
        // XS: only nibble 2
        assert_eq!(add(0xF_FFF, 0x100, XS.0, XS.1, Hex), (0xF_0FF, true));
        assert_eq!(add(0x0_5FF, 0x2FF, XS.0, XS.1, Hex), (0x0_7FF, false));
        // P field with P = 5
        assert_eq!(add(0xF0_0000, 0x10_0000, 5, 5, Hex), (0x00_0000, true));
        assert_eq!(add(0x3F_FFFF, 0x10_0000, 5, 5, Hex), (0x4F_FFFF, false));
        // WP with P = 3: nibbles 0..=3
        assert_eq!(add(0x5_FFFF, 0x1, 0, 3, Hex), (0x5_0000, true));
        assert_eq!(add(0x5_0FFF, 0x1, 0, 3, Hex), (0x5_1000, false));
        // S, M
        assert_eq!(add(0xF << 60, 1 << 60, S.0, S.1, Hex), (0, true));
        assert_eq!(
            add(0xF_FFFF_FFFF_FFFF, 0x8008, M.0, M.1, Hex),
            (0xF_FFFF_FFFF_FFFF + 0x8000, false)
        );
    }

    #[test]
    fn dec_add() {
        // 9 + 1 on the P field (P = 0): SASM 2.7 mode probe sets carry.
        assert_eq!(add(0x9, 0x1, 0, 0, Dec), (0x0, true));
        assert_eq!(inc(0x9, 0, 0, Dec), (0x0, true));
        assert_eq!(inc(0x9, 0, 0, Hex), (0xA, false));
        // 99 + 1 on B
        assert_eq!(add(0xA99, 0x1, B.0, B.1, Dec), (0xA00, true));
        // 0999 + 1 on WP (P = 3): digit carries, no carry out.
        assert_eq!(add(0x7_0999, 0x1, 0, 3, Dec), (0x7_1000, false));
        // 9999 + 1 on (0, 3): carry out, nibble 4 untouched.
        assert_eq!(add(0x7_9999, 0x1, 0, 3, Dec), (0x7_0000, true));
        // 45 + 67 = 112 on B: 12, carry.
        assert_eq!(add(0x45, 0x67, B.0, B.1, Dec), (0x12, true));
        // Invalid digits: F + F = 30 -> (30 + 6) & F = 4, carry.
        assert_eq!(add(0xF, 0xF, 0, 0, Dec), (0x4, true));
        // A + 0 = 10 > 9 -> 0, carry (assumption, see module docs).
        assert_eq!(add(0xA, 0x0, 0, 0, Dec), (0x0, true));
    }

    #[test]
    fn sub_and_dec() {
        assert_eq!(sub(0x1000, 0x1, 0, 3, Dec), (0x0999, false));
        assert_eq!(sub(0x5_0000, 0x1, 0, 3, Dec), (0x5_9999, true));
        assert_eq!(sub(0x12, 0x34, B.0, B.1, Dec), (0x78, true));
        assert_eq!(sub(0x5_0000, 0x1, 0, 3, Hex), (0x5_FFFF, true));
        assert_eq!(sub(0x34, 0x12, B.0, B.1, Hex), (0x22, false));
        assert_eq!(dec(0x0, A.0, A.1, Hex), (0xF_FFFF, true));
        assert_eq!(dec(0x0, A.0, A.1, Dec), (0x9_9999, true));
        assert_eq!(dec(0x10, B.0, B.1, Dec), (0x09, false));
        // Invalid digit: 0 - F = -15 -> (-15 + 10) & F = B, borrow.
        assert_eq!(sub(0x0, 0xF, 0, 0, Dec), (0xB, true));
    }

    #[test]
    fn neg_not() {
        // Hex two's complement, carry iff non-zero.
        assert_eq!(neg(0x9_0000_1, A.0, A.1, Hex), (0x9_FFFF_F, true));
        assert_eq!(neg(0xAB_0000_0, A.0, A.1, Hex), (0xAB_0000_0, false));
        assert_eq!(neg(1, W.0, W.1, Hex), (u64::MAX, true));
        // Dec ten's complement.
        assert_eq!(neg(0x0123, 0, 3, Dec), (0x9877, true));
        assert_eq!(neg(1, W.0, W.1, Dec), (0x9999_9999_9999_9999, true));
        assert_eq!(neg(0x5_0000, 0, 3, Dec), (0x5_0000, false));
        // One's / nine's complement, carry always cleared.
        assert_eq!(not(0x0, A.0, A.1, Hex), (0xF_FFFF, false));
        assert_eq!(not(0x7_12, B.0, B.1, Hex), (0x7_ED, false));
        assert_eq!(not(0x7_12, B.0, B.1, Dec), (0x7_87, false));
        assert_eq!(not(0x0, B.0, B.1, Dec), (0x99, false));
        // Invalid digit: 9 - B = -2 -> E.
        assert_eq!(not(0xB, 0, 0, Dec), (0xE, false));
    }

    #[test]
    fn logic_copy_exchange() {
        let a = 0xFFFF_0000_FFFF_00F0;
        let b = 0x0F0F_0F0F_0F0F_0F0F;
        assert_eq!(and(a, b, B.0, B.1), 0xFFFF_0000_FFFF_0000);
        assert_eq!(and(a, b, W.0, W.1), a & b);
        assert_eq!(or(a, b, B.0, B.1), 0xFFFF_0000_FFFF_00FF);
        assert_eq!(or(a, b, W.0, W.1), a | b);
        assert_eq!(copy(a, b, A.0, A.1), 0xFFFF_0000_FFFF_0F0F);
        assert_eq!(
            exchange(a, b, XS.0, XS.1),
            (0xFFFF_0000_FFFF_0FF0, 0x0F0F_0F0F_0F0F_000F)
        );
        assert_eq!(clear(a, S.0, S.1), 0x0FFF_0000_FFFF_00F0);
    }

    #[test]
    fn shifts_and_sb() {
        let r = 0xAB_1234_5;
        // ASL A: top nibble of field lost, no SB.
        assert_eq!(shl_nibble(r, A.0, A.1), (0xAB_2345_0, false));
        assert_eq!(shl_nibble(0xF, W.0, W.1), (0xF0, false));
        assert_eq!(shl_nibble(0xF << 60, W.0, W.1), (0, false));
        // ASR A: bottom nibble lost, SB iff non-zero.
        assert_eq!(shr_nibble(r, A.0, A.1), (0xAB_0123_4, true));
        assert_eq!(shr_nibble(0xAB_1234_0, A.0, A.1), (0xAB_0123_4, false));
        // ASR on P field (single nibble): nibble becomes 0.
        assert_eq!(shr_nibble(0x350, 1, 1), (0x300, true));
        // ASRB W: SB iff bit 0 was 1.
        assert_eq!(shr_bit(0x3, W.0, W.1), (0x1, true));
        assert_eq!(shr_bit(0x2, W.0, W.1), (0x1, false));
        assert_eq!(shr_bit(u64::MAX, W.0, W.1), (u64::MAX >> 1, true));
        // ASRB.F XS: within nibble 2 only.
        assert_eq!(shr_bit(0xF_9FF, XS.0, XS.1), (0xF_4FF, true));
        assert_eq!(shr_bit(0xF_8FF, XS.0, XS.1), (0xF_4FF, false));
    }

    #[test]
    fn rotates() {
        // Tutorial p. 70 example for ASRC.
        assert_eq!(
            ror_nibble(0x3A8B_B382_000A_F0FC, W.0, W.1),
            (0xC3A8_BB38_2000_AF0F, true)
        );
        assert_eq!(ror_nibble(0x10, W.0, W.1), (0x1, false));
        assert_eq!(ror_nibble(0x1, W.0, W.1), (0x1 << 60, true));
        // ASLC: circular left, no SB.
        assert_eq!(rol_nibble(0xF << 60, W.0, W.1), (0xF, false));
        assert_eq!(
            rol_nibble(0x0123_4567_89AB_CDEF, W.0, W.1),
            (0x1234_5678_9ABC_DEF0, false)
        );
        // Field-restricted rotate (A field).
        assert_eq!(rol_nibble(0x9_1234_5, A.0, A.1), (0x9_2345_1, false));
        assert_eq!(ror_nibble(0x9_1234_5, A.0, A.1), (0x9_5123_4, true));
    }

    #[test]
    fn comparisons() {
        let a = 0xF000_0000_0000_0001;
        let b = 0x0000_0000_0000_0002;
        // Unsigned: on W, a > b despite top bit.
        assert!(gt(a, b, W.0, W.1));
        assert!(ge(a, b, W.0, W.1));
        assert!(!lt(a, b, W.0, W.1));
        assert!(!le(a, b, W.0, W.1));
        // On A only the low field counts.
        assert!(lt(a, b, A.0, A.1));
        assert!(le(a, b, A.0, A.1));
        assert!(!gt(a, b, A.0, A.1));
        assert!(eq(0xA_0005, 0xB_0005, 0, 3));
        assert!(ne(0xA_0005, 0xB_0005, A.0, A.1));
        assert!(ge(0x5, 0x5, B.0, B.1));
        assert!(le(0x5, 0x5, B.0, B.1));
        assert!(zero(0xF_0000, 0, 3));
        assert!(nonzero(0xF_0000, A.0, A.1));
        assert!(!nonzero(0, W.0, W.1));
    }

    #[test]
    fn bits() {
        assert!(bit_get(0x8000, 15));
        assert!(!bit_get(0x8000, 14));
        assert_eq!(bit_set(0, 15, true), 0x8000);
        assert_eq!(bit_set(u64::MAX, 0, false), u64::MAX - 1);
        // Index masked to 0..=15.
        assert_eq!(bit_set(0, 16, true), 1);
    }

    #[test]
    fn pointer_and_p_arithmetic() {
        assert_eq!(ptr_add(0xF_FFFF, 1), (0, true));
        assert_eq!(ptr_add(0xF_FFF0, 16), (0, true));
        assert_eq!(ptr_add(0x1_0000, 16), (0x1_0010, false));
        assert_eq!(ptr_sub(0x0, 1), (0xF_FFFF, true));
        assert_eq!(ptr_sub(0x5, 5), (0x0, false));
        assert_eq!(ptr_sub(0x5, 16), (0xF_FFF5, true));
        assert_eq!(p_inc(0xF), (0, true));
        assert_eq!(p_inc(0x7), (8, false));
        assert_eq!(p_dec(0), (0xF, true));
        assert_eq!(p_dec(1), (0, false));
        assert_eq!(c_plus_p_plus_1(0xAB_F_FFFE, 0), (0xAB_F_FFFF, false));
        assert_eq!(c_plus_p_plus_1(0xAB_F_FFFE, 1), (0xAB_0_0000, true));
        assert_eq!(c_plus_p_plus_1(0x0, 0xF), (0x10, false));
    }

    #[test]
    fn add_sub_const_multi_nibble() {
        // A field, always hex even for DEC-looking values.
        assert_eq!(add_const(0x7_0009, 1, A.0, A.1), (0x7_000A, false));
        assert_eq!(add_const(0x7_FFFF_F, 16, A.0, A.1), (0x7_0000_F, true));
        assert_eq!(add_const(0xFF, 2, B.0, B.1), (0x01, true));
        assert_eq!(sub_const(0x7_0000_1, 2, A.0, A.1), (0x7_FFFF_F, true));
        assert_eq!(sub_const(0x7_0001_0, 16, A.0, A.1), (0x7_0000_0, false));
        // W: overflow out of nibble 15.
        assert_eq!(add_const(u64::MAX, 1, W.0, W.1), (0, true));
    }

    /// Pins the chosen behaviour for questions/dec-mode-constant-bug so any
    /// change is deliberate. Examples from tutorial p. 59.
    #[test]
    fn const_single_nibble_overrun_pinned() {
        // A=A+4 XS on FFFFFFFFFFFFFEF2 -> 00000000000002F3 (carry wraps
        // round from nibble 15 into nibble 0).
        assert_eq!(
            add_const(0xFFFF_FFFF_FFFF_FEF2, 4, XS.0, XS.1),
            (0x0000_0000_0000_02F3, false)
        );
        // A=A+4 XS on FFFFFFFFFFF1FEFF -> FFFFFFFFFFF202FF.
        assert_eq!(
            add_const(0xFFFF_FFFF_FFF1_FEFF, 4, XS.0, XS.1),
            (0xFFFF_FFFF_FFF2_02FF, false)
        );
        // No overflow out of the nibble: behaves like a field add.
        assert_eq!(add_const(0x123, 2, XS.0, XS.1), (0x323, false));
        // S field: overflow continues into nibble 0.
        assert_eq!(add_const(0xF << 60, 2, S.0, S.1), (0x1 << 60 | 0x1, false));
        // Whole chain overflows: carry set.
        assert_eq!(add_const(u64::MAX, 1, 7, 7), (0, true));
        // Subtract: borrow ripples circularly.
        assert_eq!(sub_const(0x5, 1, 0, 0), (0x4, false));
        // 1 - 2 in nibble 2 borrows through nibbles 3..15 and round into
        // nibble 0 (5 -> 4), where the borrow stops.
        assert_eq!(
            sub_const(0x0000_0000_0000_0105, 2, XS.0, XS.1),
            (0xFFFF_FFFF_FFFF_FF04, false)
        );
        // Borrow through all 16 nibbles: carry set.
        assert_eq!(sub_const(0x0, 2, XS.0, XS.1), (0xFFFF_FFFF_FFFF_FEFF, true));
    }
}
