//! Executor tests: one per instruction family, with hand-computed results.
//! Opcodes from the SASM manual section 8; the program text is hex nibbles
//! in opcode order (spaces ignored).

// Literal groupings mark field boundaries (e.g. 0xAB_1234_5 = A field 12345).
#![allow(clippy::unusual_byte_groupings)]

use super::*;
use crate::cpu::bus::FlatMemory;
use crate::cpu::regs::{HST_MP, RSTK_DEPTH};

fn nibs(hex: &str) -> Vec<u8> {
    hex.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_digit(16).expect("hex digit") as u8)
        .collect()
}

/// A CPU plus bus with a program loaded at `pc`.
struct T<B: Bus = FlatMemory> {
    cpu: Cpu,
    bus: B,
}

impl T<FlatMemory> {
    fn at(pc: u32, code: &str) -> Self {
        let mut bus = FlatMemory::new();
        bus.load(pc, &nibs(code));
        let mut cpu = Cpu::new();
        cpu.regs.pc = pc;
        T { cpu, bus }
    }
}

impl<B: Bus> T<B> {
    fn step(&mut self) -> Step {
        self.cpu.step(&mut self.bus)
    }

    fn run(&mut self, n: usize) -> Vec<Step> {
        (0..n).map(|_| self.step()).collect()
    }

    fn r(&mut self) -> &mut Registers {
        &mut self.cpu.regs
    }
}

const PC: u32 = 0x2000;

// ---------------------------------------------------------------- loads

#[test]
fn lc_wraps_from_p_and_keeps_carry() {
    // P= 14; LC(3) 321 (digits stored low first: 1, 2, 3).
    let mut t = T::at(PC, "2E 32 123");
    t.r().c = 0x0FFF_FFFF_FFFF_FFF0;
    t.r().carry = true;
    t.run(2);
    // nibble 14 = 1, 15 = 2, 0 = 3.
    assert_eq!(t.cpu.regs.c, 0x21FF_FFFF_FFFF_FFF3);
    assert!(t.cpu.regs.carry);
    assert_eq!(t.cpu.regs.pc, PC + 7);
}

#[test]
fn la_loads_a_at_p() {
    // P= 3; LA(2) AB -> A nibble 3 = A, nibble 4 = B.
    let mut t = T::at(PC, "23 8082 1 AB");
    let s = t.run(2);
    assert_eq!(t.cpu.regs.a, 0xBA000);
    assert_eq!(t.cpu.regs.pc, PC + 2 + 7);
    assert_eq!(s[1].cycles, 7 + 2);
}

// ---------------------------------------------------------------- P

#[test]
fn p_ops() {
    // P= F; P=P+1; P=P-1; P=P-1
    let mut t = T::at(PC, "2F 0C 0D 0D");
    t.step();
    assert_eq!(t.cpu.regs.p, 0xF);
    t.step();
    assert_eq!((t.cpu.regs.p, t.cpu.regs.carry), (0, true));
    t.step();
    assert_eq!((t.cpu.regs.p, t.cpu.regs.carry), (0xF, true));
    t.step();
    assert_eq!((t.cpu.regs.p, t.cpu.regs.carry), (0xE, false));
}

#[test]
fn c_p_transfers() {
    // P= 9; C=P 3; P=C 1; CPEX 0
    let mut t = T::at(PC, "29 80C3 80D1 80F0");
    t.r().c = 0x0000_0000_0000_0057;
    t.step();
    t.step();
    assert_eq!(t.cpu.regs.c, 0x9057);
    t.step();
    assert_eq!(t.cpu.regs.p, 5);
    t.step();
    // nibble 0 (7) <-> P (5)
    assert_eq!((t.cpu.regs.c, t.cpu.regs.p), (0x9055, 7));
}

#[test]
fn c_plus_p_plus_1_hex_with_carry() {
    // SETDEC; P= 2; C+P+1 (always hex)
    let mut t = T::at(PC, "05 22 809 809");
    t.r().c = 0xAB_FFFF_C;
    t.run(3);
    assert_eq!(t.cpu.regs.c, 0xAB_FFFF_F);
    assert!(!t.cpu.regs.carry);
    t.r().c = 0xAB_FFFF_D;
    t.step();
    assert_eq!(t.cpu.regs.c, 0xAB_0000_0);
    assert!(t.cpu.regs.carry);
}

// ---------------------------------------------------------------- D0/D1

#[test]
fn ptr_loads_2_4_5() {
    // D0=(2) 43; D0=(4) 4321; D1=(5) 12345; D1=(2) FF
    let mut t = T::at(PC, "19 34  1A 1234  1F 54321  1D FF");
    t.r().d0 = 0xA_BCDE;
    t.step();
    assert_eq!(t.cpu.regs.d0, 0xA_BC43);
    t.step();
    assert_eq!(t.cpu.regs.d0, 0xA_4321);
    t.step();
    assert_eq!(t.cpu.regs.d1, 0x1_2345);
    t.step();
    assert_eq!(t.cpu.regs.d1, 0x1_23FF);
    assert_eq!(t.cpu.regs.pc, PC + 4 + 6 + 7 + 4);
}

#[test]
fn ptr_from_and_exchange_with_regs() {
    // D0=A; D1=CS; AD0EX; CD1XS
    let mut t = T::at(PC, "130 13D 132 13F");
    t.r().a = 0xFFFF_FFFF_FFF1_2345;
    t.r().c = 0x1111_1111_111A_BCDE;
    t.r().d1 = 0x9_0000;
    t.r().carry = true;
    t.step();
    assert_eq!(t.cpu.regs.d0, 0x1_2345);
    t.step();
    // only nibbles 0-3 copied, nibble 4 of D1 kept
    assert_eq!(t.cpu.regs.d1, 0x9_BCDE);
    t.r().d0 = 0x5_4321;
    t.step();
    assert_eq!(t.cpu.regs.d0, 0x1_2345);
    assert_eq!(t.cpu.regs.a, 0xFFFF_FFFF_FFF5_4321);
    t.r().d1 = 0x7_6543;
    t.step();
    assert_eq!(t.cpu.regs.d1, 0x7_BCDE);
    assert_eq!(t.cpu.regs.c, 0x1111_1111_111A_6543);
    assert!(t.cpu.regs.carry, "pointer copies leave carry alone");
}

#[test]
fn ptr_arith_wraps_with_carry() {
    // D0=D0+ 3; D0=D0- 3; D1=D1+ 16; D1=D1- 1
    let mut t = T::at(PC, "162 182 17F 1C0");
    t.r().d0 = 0xF_FFFE;
    t.r().d1 = 0x0_0010;
    t.step();
    assert_eq!((t.cpu.regs.d0, t.cpu.regs.carry), (0x0_0001, true));
    t.step();
    assert_eq!((t.cpu.regs.d0, t.cpu.regs.carry), (0xF_FFFE, true));
    t.step();
    assert_eq!((t.cpu.regs.d1, t.cpu.regs.carry), (0x0_0020, false));
    t.step();
    assert_eq!((t.cpu.regs.d1, t.cpu.regs.carry), (0x0_001F, false));
}

// ---------------------------------------------------------------- DAT

#[test]
fn dat_read_field_and_count_forms() {
    // A=DAT0 A; A=DAT0 4; P= 3; A=DAT0 P; C=DAT1 X; A=DAT0 B
    let mut t = T::at(PC, "142 15A3 23 1520 157 3 14A");
    t.bus.load(0x8_0000, &nibs("123456789"));
    t.r().d0 = 0x8_0000;
    t.r().d1 = 0x8_0002;
    t.r().a = 0xFFFF_FFFF_FFFF_FFFF;
    t.step();
    assert_eq!(t.cpu.regs.a, 0xFFFF_FFFF_FFF5_4321);
    t.r().a = 0xFFFF_FFFF_FFFF_FFFF;
    t.step();
    assert_eq!(t.cpu.regs.a, 0xFFFF_FFFF_FFFF_4321);
    t.run(2);
    // P field, P = 3: nibble 3 of A = mem[D0] = 1
    assert_eq!(t.cpu.regs.a, 0xFFFF_FFFF_FFFF_1321);
    t.step();
    assert_eq!(t.cpu.regs.c & 0xFFF, 0x543);
    t.step();
    assert_eq!(t.cpu.regs.a & 0xFF, 0x21);
}

#[test]
fn dat_write_little_endian_and_wrap() {
    // DAT1=C B; DAT0=A 16; P= 2; DAT0=A WP
    let mut t = T::at(PC, "14D 158F 22 1501");
    t.r().c = 0xAB;
    t.r().d1 = 0xF_FFFF;
    t.r().a = 0xFEDC_BA98_7654_3210;
    t.r().d0 = 0x4_0000;
    t.step();
    assert_eq!(t.bus.read_nibble(0xF_FFFF), 0xB);
    assert_eq!(t.bus.read_nibble(0x0_0000), 0xA);
    t.step();
    let got: Vec<u8> = (0..16).map(|i| t.bus.read_nibble(0x4_0000 + i)).collect();
    assert_eq!(got, (0..16).collect::<Vec<u8>>());
    t.r().d0 = 0x5_0000;
    t.run(2);
    let got: Vec<u8> = (0..4).map(|i| t.bus.read_nibble(0x5_0000 + i)).collect();
    assert_eq!(got, vec![0, 1, 2, 0], "WP with P=2 writes nibbles 0..=2");
}

#[test]
fn dat_field_starting_above_zero() {
    // DAT0=A M: register nibble 3 goes to D0, nibble 14 to D0+11.
    let mut t = T::at(PC, "1505 1525");
    t.r().a = 0xFEDC_BA98_7654_3210;
    t.r().d0 = 0x6_0000;
    t.step();
    let got: Vec<u8> = (0..13).map(|i| t.bus.read_nibble(0x6_0000 + i)).collect();
    assert_eq!(got, vec![3, 4, 5, 6, 7, 8, 9, 0xA, 0xB, 0xC, 0xD, 0xE, 0]);
    t.r().a = 0;
    t.step();
    assert_eq!(t.cpu.regs.a, 0x0EDC_BA98_7654_3000);
}

// ---------------------------------------------------------------- RSTK

#[test]
fn gosub_rtn_round_trip() {
    // 2000: GOSUB 2010 (offset from 2004 = C); 2010: RTN
    let mut t = T::at(PC, "7C00");
    t.bus.load(0x2010, &nibs("01"));
    let s = t.step();
    assert_eq!(t.cpu.regs.pc, 0x2010);
    assert_eq!(t.cpu.regs.rstk.levels()[0], 0x2004);
    assert_eq!(s.cycles, 12);
    t.step();
    assert_eq!(t.cpu.regs.pc, 0x2004);
}

#[test]
fn rstk_eight_levels_circular() {
    // LC(1) n; RSTK=C for n = 1..=9, then C=RSTK nine times.
    let code: String = (1..=9).map(|n| format!("30{n:X}06")).collect::<String>() + &"07".repeat(9);
    let mut t = T::at(PC, &code);
    t.r().c = 0xABC0_0000_0000_0000;
    t.run(18);
    assert_eq!(t.cpu.regs.rstk.levels()[0], 9);
    let mut popped = Vec::new();
    for _ in 0..9 {
        t.step();
        popped.push(t.cpu.regs.c & 0xF_FFFF);
    }
    assert_eq!(popped, vec![9, 8, 7, 6, 5, 4, 3, 2, 0]);
    assert_eq!(t.cpu.regs.c >> 20, 0xABC0_0000_000, "only C(A) changes");
    assert_eq!(t.cpu.regs.rstk.levels(), &[0; RSTK_DEPTH]);
}

#[test]
fn return_variants() {
    // RTNSXM, RTNSC, RTNCC, RTNC (taken / not), RTNNC
    for (code, carry_in, pc, carry, hst) in [
        ("00", false, 0x3000, false, HST_XM),
        ("02", false, 0x3000, true, 0),
        ("03", true, 0x3000, false, 0),
        ("400", true, 0x3000, true, 0),
        ("400", false, PC + 3, false, 0),
        ("500", false, 0x3000, false, 0),
        ("500", true, PC + 3, true, 0),
    ] {
        let mut t = T::at(PC, code);
        t.r().push(0x3000);
        t.r().carry = carry_in;
        t.step();
        assert_eq!(
            (t.cpu.regs.pc, t.cpu.regs.carry, t.cpu.regs.hst),
            (pc, carry, hst),
            "{code} carry_in={carry_in}"
        );
    }
}

// ---------------------------------------------------------------- branches

#[test]
fn goc_gonc() {
    // GOC +0x10 from 2001
    let mut t = T::at(PC, "401");
    t.r().carry = true;
    let s = t.step();
    assert_eq!((t.cpu.regs.pc, s.cycles), (0x2011, 10));
    let mut t = T::at(PC, "401");
    let s = t.step();
    assert_eq!((t.cpu.regs.pc, s.cycles), (0x2003, 3));
    // GONC -0x10 (F0) from 2001
    let mut t = T::at(PC, "50F");
    t.step();
    assert_eq!(t.cpu.regs.pc, 0x1FF1);
    let mut t = T::at(PC, "50F");
    t.r().carry = true;
    t.step();
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x2003, true));
}

#[test]
fn unconditional_jumps() {
    let cases = [
        // GOTO +0x123 from 2001
        ("6321", 0x2124),
        // GOTO -0x10 from 2001
        ("60FF", 0x1FF1),
        // GOLONG +0x1234 from 2002
        ("8C4321", 0x3236),
        // GOVLNG 12345
        ("8D54321", 0x1_2345),
    ];
    for (code, target) in cases {
        let mut t = T::at(PC, code);
        t.step();
        assert_eq!(t.cpu.regs.pc, target, "{code}");
        assert_eq!(t.cpu.regs.rstk.levels()[0], 0);
    }
}

#[test]
fn subroutine_calls() {
    // GOSUBL +0x10 from 2006; GOSBVL 12345
    let mut t = T::at(PC, "8E0100");
    t.step();
    assert_eq!(t.cpu.regs.pc, 0x2016);
    assert_eq!(t.cpu.regs.rstk.levels()[0], 0x2006);
    let mut t = T::at(PC, "8F54321");
    t.step();
    assert_eq!(t.cpu.regs.pc, 0x1_2345);
    assert_eq!(t.cpu.regs.rstk.levels()[0], 0x2007);
}

#[test]
fn branch_wraps_at_20_bits() {
    // GOTO +0x10 at FFFF0: FFFF1 + 10 = 00001
    let mut t = T::at(0xF_FFF0, "6010");
    t.step();
    assert_eq!(t.cpu.regs.pc, 0x0_0001);
    // PC wraps past FFFFF on a plain instruction: P= 1 at FFFFF
    let mut t = T::at(0xF_FFFF, "21");
    t.step();
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.p), (0x0_0001, 1));
}

#[test]
fn pc_register_ops() {
    // PC=A
    let mut t = T::at(PC, "81B2");
    t.r().a = 0xFFFF_FFFF_FFF1_2345;
    t.step();
    assert_eq!(t.cpu.regs.pc, 0x1_2345);
    // PC=(C): five nibbles at C(A), little-endian
    let mut t = T::at(PC, "808E");
    t.bus.load(0x3_0000, &nibs("54321"));
    t.r().c = 0x3_0000;
    t.step();
    assert_eq!(t.cpu.regs.pc, 0x1_2345);
    // APCEX: A(A) gets the address of the next instruction
    let mut t = T::at(PC, "81B6");
    t.r().a = 0xFFFF_FFFF_FFF1_2345;
    t.step();
    assert_eq!(t.cpu.regs.pc, 0x1_2345);
    assert_eq!(t.cpu.regs.a, 0xFFFF_FFFF_FFF0_2004);
    // A=PC, C=PC
    let mut t = T::at(PC, "81B4 81B5");
    t.r().a = 0xFFFF_FFFF_FFFF_FFFF;
    t.run(2);
    assert_eq!(t.cpu.regs.a, 0xFFFF_FFFF_FFF0_2004);
    assert_eq!(t.cpu.regs.c, 0x2008);
    assert_eq!(t.cpu.regs.pc, 0x2008);
}

#[test]
fn nops() {
    let mut t = T::at(PC, "820 6300 64000");
    t.r().hst = 0xF;
    t.run(3);
    assert_eq!(t.cpu.regs.pc, PC + 12);
    assert_eq!(t.cpu.regs.hst, 0xF);
}

// ---------------------------------------------------------------- tests

#[test]
fn test_goyes_taken_and_not() {
    // ?A=B A GOYES +0x10 (from 2003)
    let mut t = T::at(PC, "8A0 01");
    t.r().a = 0x77_12345;
    t.r().b = 0x88_12345;
    let s = t.step();
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x2013, true));
    assert_eq!(s.cycles, 13 + 5);
    let mut t = T::at(PC, "8A0 01");
    t.r().b = 1;
    t.r().carry = true;
    let s = t.step();
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x2005, false));
    assert_eq!(s.cycles, 6 + 5);
}

#[test]
fn test_rtnyes() {
    // ?C#0 A RTNYES
    let mut t = T::at(PC, "8AE 00");
    t.r().push(0x4000);
    t.r().c = 1;
    t.step();
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x4000, true));
    let mut t = T::at(PC, "8AE 00");
    t.r().push(0x4000);
    t.step();
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x2005, false));
    assert_eq!(t.cpu.regs.rstk.levels()[0], 0x4000);
}

#[test]
fn test_field_comparisons() {
    // ?A<B X RTNYES (unsigned, only nibbles 0-2 compared); ?A=0 P RTNYES
    for (code, a, b, expect) in [
        ("9B4 00", 0xF123u64, 0x0200u64, true),
        ("9B4 00", 0x0200, 0xF123, false),
        ("9B4 00", 0x0123, 0x0123, false),
        // ?A=0 P with P = 0
        ("908 00", 0x10, 0x0, true),
        ("908 00", 0x11, 0x0, false),
    ] {
        let mut t = T::at(PC, code);
        t.r().a = a;
        t.r().b = b;
        t.r().push(0x4000);
        t.step();
        assert_eq!(t.cpu.regs.carry, expect, "{code} a={a:X} b={b:X}");
        assert_eq!(t.cpu.regs.pc, if expect { 0x4000 } else { PC + 5 });
    }
}

#[test]
fn test_p_and_bits() {
    // P= 5; ?P= 5 GOYES; ?P# 5 GOYES
    let mut t = T::at(PC, "25 895 01");
    t.run(2);
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x2015, true));
    let mut t = T::at(PC, "25 885 01");
    t.run(2);
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x2007, false));
    // ABIT=1 7; ?ABIT=1 7 GOYES +0x10 (from 2005+5=200A); CBIT=0 0
    let mut t = T::at(PC, "80857 80877 01");
    t.step();
    assert_eq!(t.cpu.regs.a, 0x80);
    let s = t.step();
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x200A + 0x10, true));
    assert_eq!(s.cycles, 20);
    let mut t = T::at(PC, "80880");
    t.r().c = 0xF;
    t.step();
    assert_eq!(t.cpu.regs.c, 0xE);
}

// ---------------------------------------------------------------- arithmetic

#[test]
fn hex_arithmetic_fields_and_carry() {
    // A=A+B A; A=A+B B; P= 5; A=A+B P; A=A-B X; A=B-A A
    let mut t = T::at(PC, "C0 A60 25 A00 B30 EC");
    t.r().a = 0xFFFFF;
    t.r().b = 0x00001;
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0, true));
    t.r().a = 0x1_23FF;
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0x1_2300, true));
    t.r().a = 0xF0_0000;
    t.r().b = 0x10_0000;
    t.run(2);
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0, true));
    t.r().a = 0x7_0100;
    t.r().b = 0x0_0200;
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0x7_0F00, true));
    t.r().a = 0x5;
    t.r().b = 0x3;
    t.step();
    // A = B - A = 3 - 5 = FFFFE, borrow
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0xF_FFFE, true));
}

#[test]
fn dec_arithmetic() {
    // SETDEC; A=A+B B; A=A+1 A; C=C-1 A; SETHEX; A=A+1 A
    let mut t = T::at(PC, "05 A60 E4 CE 04 E4");
    t.r().a = 0x99;
    t.r().b = 0x01;
    t.run(2);
    assert_eq!(t.cpu.regs.mode, Mode::Dec);
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0x00, true));
    t.r().a = 0x0_9999;
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0x1_0000, false));
    t.r().c = 0;
    t.step();
    assert_eq!((t.cpu.regs.c, t.cpu.regs.carry), (0x9_9999, true));
    t.r().a = 0x9;
    t.run(2);
    assert_eq!(t.cpu.regs.mode, Mode::Hex);
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0xA, false));
}

#[test]
fn constants_neg_not_zero_copy_exchange() {
    // A=A+CON A,3; A=A-CON A,5; A=-A A; A=-A-1 A; B=0 A; A=B A; ABEX A
    let mut t = T::at(PC, "818F02 818F84 F8 FC D1 D4 DC");
    t.r().a = 0xF_FFFE;
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0x0_0001, true));
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0xF_FFFC, true));
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0x0_0004, true));
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.carry), (0xF_FFFB, false));
    t.r().b = 0xAA_12345;
    t.r().carry = true;
    t.step();
    assert_eq!(t.cpu.regs.b, 0xAA_00000);
    t.r().b = 0xAA_54321;
    t.step();
    assert_eq!(t.cpu.regs.a, 0x5_4321);
    t.r().a = 0x1_1111;
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.b), (0x5_4321, 0xAA_11111));
    assert!(t.cpu.regs.carry, "zero/copy/exchange leave carry alone");
}

#[test]
fn logic_ops() {
    // A=A&B A; C=C!D X
    let mut t = T::at(PC, "0EF0 0E3F");
    t.r().a = 0xF0_FF0F0;
    t.r().b = 0x00_0FF00;
    t.r().c = 0x1_000;
    t.r().d = 0xF_ABC;
    t.r().carry = true;
    t.run(2);
    assert_eq!(t.cpu.regs.a, 0xF0_0F000);
    assert_eq!(t.cpu.regs.c, 0x1_ABC);
    assert!(t.cpu.regs.carry);
}

// ---------------------------------------------------------------- shifts

#[test]
fn shifts_set_sticky_bit() {
    // ASR A: lost nibble F -> SB
    let mut t = T::at(PC, "F4");
    t.r().a = 0xAB_0001F;
    t.step();
    assert_eq!(t.cpu.regs.a, 0xAB_00001);
    assert_eq!(t.cpu.regs.hst, HST_SB);
    // ASL A: SB not affected; existing SB stays
    let mut t = T::at(PC, "F0 F4");
    t.r().a = 0xF_0001;
    t.r().hst = HST_SB | HST_MP;
    t.step();
    assert_eq!(t.cpu.regs.a, 0x0_0010);
    t.step();
    assert_eq!(t.cpu.regs.hst, HST_SB | HST_MP, "lost zero nibble keeps SB");
    // CSRB: lost bit 1 -> SB; BSRC rotates nibble to the top; DSLC
    let mut t = T::at(PC, "81E 815 813");
    t.r().c = 0x3;
    t.r().b = 0x5;
    t.r().d = 0xA000_0000_0000_0001;
    t.step();
    assert_eq!((t.cpu.regs.c, t.cpu.regs.hst), (0x1, HST_SB));
    t.r().hst = 0;
    t.step();
    assert_eq!(
        (t.cpu.regs.b, t.cpu.regs.hst),
        (0x5000_0000_0000_0000, HST_SB)
    );
    t.r().hst = 0;
    t.step();
    assert_eq!((t.cpu.regs.d, t.cpu.regs.hst), (0x0000_0000_0000_001A, 0));
    // ASRB.F B: bit shift only within nibbles 0-1
    let mut t = T::at(PC, "81960");
    t.r().a = 0x1_03;
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.hst), (0x1_01, HST_SB));
}

// ---------------------------------------------------------------- status

#[test]
fn st_ops() {
    // ST=1 5; ST=1 F; ?ST=1 5 RTNYES; ...
    let mut t = T::at(PC, "855 85F 875 00");
    t.r().push(0x4000);
    t.run(3);
    assert_eq!(t.cpu.regs.st, 0x8020);
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x4000, true));
    // ST=0 5; ?ST=0 5 GOYES +0x10 (from 2006)
    let mut t = T::at(PC, "845 865 01");
    t.r().st = 0x20;
    let s = t.run(2);
    assert_eq!(t.cpu.regs.st, 0);
    assert_eq!(t.cpu.regs.pc, 0x2016);
    assert_eq!(s[1].cycles, 14);
    // CLRST keeps 12-15; C=ST; ST=C; CSTEX
    let mut t = T::at(PC, "09 08 0A 0B");
    t.r().st = 0xFABC;
    t.r().c = 0xFFFF_F000;
    t.step();
    assert_eq!(t.cpu.regs.c, 0xFFFF_FABC);
    t.step();
    assert_eq!(t.cpu.regs.st, 0xF000);
    t.r().c = 0x9_123;
    t.step();
    assert_eq!(t.cpu.regs.st, 0xF123);
    t.r().c = 0x9_456;
    t.step();
    assert_eq!((t.cpu.regs.st, t.cpu.regs.c), (0xF456, 0x9_123));
}

#[test]
fn hst_ops() {
    // SB=0; ?HS=0 3 (XM|SB) GOYES; CLRHST; ?MP=0 GOYES
    let mut t = T::at(PC, "822 833 01");
    t.r().hst = HST_SB | HST_XM | HST_MP;
    t.step();
    assert_eq!(t.cpu.regs.hst, HST_XM | HST_MP);
    t.step();
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x2008, false));
    let mut t = T::at(PC, "82F 838 01");
    t.r().hst = 0xF;
    t.run(2);
    assert_eq!(t.cpu.regs.hst, 0);
    assert_eq!((t.cpu.regs.pc, t.cpu.regs.carry), (0x2016, true));
}

// ---------------------------------------------------------------- scratch

#[test]
fn scratch_whole_register() {
    // R0=A; C=R0; AR4EX; R3=C
    let mut t = T::at(PC, "100 118 124 10B");
    t.r().a = 0x1234_5678_9ABC_DEF0;
    t.r().r[4] = 7;
    t.r().carry = true;
    t.run(2);
    assert_eq!(t.cpu.regs.r[0], 0x1234_5678_9ABC_DEF0);
    assert_eq!(t.cpu.regs.c, 0x1234_5678_9ABC_DEF0);
    t.step();
    assert_eq!((t.cpu.regs.a, t.cpu.regs.r[4]), (7, 0x1234_5678_9ABC_DEF0));
    t.step();
    assert_eq!(t.cpu.regs.r[3], t.cpu.regs.c);
    assert!(t.cpu.regs.carry);
}

#[test]
fn scratch_with_field() {
    // R1=A.F X; A=R2.F B; CR2EX.F XS
    let mut t = T::at(PC, "81A301 81A612 81A22A");
    t.r().a = 0xFFFF_FFFF_FFFF_F987;
    t.r().r[1] = 0x1111;
    t.step();
    assert_eq!(t.cpu.regs.r[1], 0x1987);
    t.r().r[2] = 0xAB_CD;
    t.step();
    assert_eq!(t.cpu.regs.a, 0xFFFF_FFFF_FFFF_F9CD);
    t.r().c = 0x500;
    t.r().r[2] = 0x7FF;
    t.step();
    assert_eq!((t.cpu.regs.c, t.cpu.regs.r[2]), (0x700, 0x5FF));
}

// ---------------------------------------------------------------- chip interface

#[derive(Default)]
struct RecBus {
    mem: Option<FlatMemory>,
    log: Vec<String>,
    inp: u16,
    id: u32,
    sreq: u8,
    irq: bool,
}

impl Bus for RecBus {
    fn read_nibble(&mut self, addr: u32) -> u8 {
        self.mem.as_mut().map_or(0, |m| m.read_nibble(addr))
    }
    fn write_nibble(&mut self, addr: u32, nibble: u8) {
        if let Some(m) = self.mem.as_mut() {
            m.write_nibble(addr, nibble);
        }
    }
    fn read_in(&mut self) -> u16 {
        self.log.push("in".into());
        self.inp
    }
    fn write_out(&mut self, out: u16) {
        self.log.push(format!("out {out:03X}"));
    }
    fn config(&mut self, addr: u32) {
        self.log.push(format!("config {addr:05X}"));
    }
    fn unconfig(&mut self, addr: u32) {
        self.log.push(format!("unconfig {addr:05X}"));
    }
    fn read_id(&mut self) -> u32 {
        self.log.push("id".into());
        self.id
    }
    fn reset(&mut self) {
        self.log.push("reset".into());
    }
    fn shutdown(&mut self) {
        self.log.push("shutdown".into());
    }
    fn service_request(&mut self) -> u8 {
        self.log.push("sreq".into());
        self.sreq
    }
    fn bus_command(&mut self, cmd: BusCommand) {
        self.log.push(format!("bus {cmd:?}"));
    }
    fn interrupt_pending(&mut self) -> bool {
        self.log.push("irq?".into());
        self.irq
    }
}

fn rec(pc: u32, code: &str) -> T<RecBus> {
    let mut mem = FlatMemory::new();
    mem.load(pc, &nibs(code));
    let mut cpu = Cpu::new();
    cpu.regs.pc = pc;
    T {
        cpu,
        bus: RecBus {
            mem: Some(mem),
            ..RecBus::default()
        },
    }
}

#[test]
fn chip_interface() {
    // OUT=C; OUT=CS; A=IN; C=IN; UNCNFG; CONFIG; C=ID; SREQ?; RESET;
    // BUSCB; BUSCC; BUSCD
    let mut t = rec(PC, "801 800 802 803 804 805 806 80E 80A 8083 80B 808D");
    t.bus.inp = 0x8421;
    t.bus.id = 0x1_2345;
    t.bus.sreq = 0x2;
    t.r().c = 0xFFFF_FFFF_FFFF_FABC;
    t.r().a = 0xFFFF_FFFF_FFFF_FFFF;
    t.step();
    assert_eq!(t.cpu.regs.out, 0xABC);
    t.r().c = 0xFFFF_FFFF_FFFF_FFF5;
    t.step();
    assert_eq!(t.cpu.regs.out, 0xAB5);
    t.step();
    assert_eq!(t.cpu.regs.a, 0xFFFF_FFFF_FFFF_8421);
    assert_eq!(t.cpu.regs.inp, 0x8421);
    t.step();
    assert_eq!(t.cpu.regs.c, 0xFFFF_FFFF_FFFF_8421);
    t.r().c = 0xFF_0_6000_0;
    t.run(2);
    t.step();
    assert_eq!(t.cpu.regs.c, 0xFF_0_1234_5);
    t.step();
    assert_eq!(t.cpu.regs.c, 0xFF_0_1234_2);
    assert_eq!(t.cpu.regs.hst, HST_SR);
    t.run(4);
    assert_eq!(
        t.bus.log,
        vec![
            "out ABC",
            "out AB5",
            "in",
            "in",
            "unconfig 60000",
            "config 60000",
            "id",
            "sreq",
            "reset",
            "bus B",
            "bus C",
            "bus D",
        ]
    );
    assert_eq!(t.cpu.regs.pc, PC + 3 * 9 + 4 * 2 + 3);
}

#[test]
fn sreq_without_request_leaves_sr() {
    let mut t = rec(PC, "80E");
    t.r().c = 0xF;
    t.step();
    assert_eq!((t.cpu.regs.c, t.cpu.regs.hst), (0, 0));
}

#[test]
fn shutdown_event() {
    let mut t = rec(PC, "807");
    let s = t.step();
    assert_eq!(s.event, Some(Event::Shutdown));
    assert_eq!(t.bus.log, vec!["shutdown"]);
    assert_eq!(t.cpu.regs.pc, PC + 3);
}

// ---------------------------------------------------------------- interrupts

#[test]
fn interrupt_entry_and_pending_reentry() {
    // Handler at #0000F is a bare RTI.
    let mut t = T::at(PC, "20");
    t.bus.load(INTERRUPT_VECTOR, &nibs("0F"));
    t.cpu.interrupt();
    assert_eq!(t.cpu.regs.pc, INTERRUPT_VECTOR);
    assert!(t.cpu.regs.in_interrupt);
    assert_eq!(t.cpu.regs.rstk.levels()[0], PC);
    // A second interrupt while in service is latched.
    t.cpu.interrupt();
    assert!(t.cpu.regs.interrupt_pending);
    assert_eq!(t.cpu.regs.pc, INTERRUPT_VECTOR);
    assert_eq!(t.cpu.regs.rstk.levels()[1], 0);
    // RTI re-enters at once.
    let s = t.step();
    assert_eq!(s.event, Some(Event::Rti));
    assert_eq!(t.cpu.regs.pc, INTERRUPT_VECTOR);
    assert!(t.cpu.regs.in_interrupt);
    assert!(!t.cpu.regs.interrupt_pending);
    assert_eq!(t.cpu.regs.rstk.levels()[..2], [PC, 0]);
    // Second RTI returns for real.
    t.step();
    assert_eq!(t.cpu.regs.pc, PC);
    assert!(!t.cpu.regs.in_interrupt);
}

#[test]
fn inton_intoff() {
    let mut t = T::at(PC, "808F 8080");
    t.step();
    assert!(!t.cpu.regs.interrupts_enabled);
    t.step();
    assert!(t.cpu.regs.interrupts_enabled);
}

#[test]
fn rsi_rearms_interrupt_detection() {
    // RSI with a request active: vectors right after the RSI.
    let mut t = rec(PC, "80810");
    t.bus.irq = true;
    t.step();
    assert_eq!(t.cpu.regs.pc, INTERRUPT_VECTOR);
    assert!(t.cpu.regs.in_interrupt);
    assert_eq!(t.cpu.regs.rstk.levels()[0], PC + 5);
    assert_eq!(t.bus.log, vec!["irq?"]);
    // In service: only latched, taken after RTI.
    let mut t = rec(PC, "80810");
    t.bus.irq = true;
    t.r().in_interrupt = true;
    t.step();
    assert_eq!(t.cpu.regs.pc, PC + 5);
    assert!(t.cpu.regs.interrupt_pending);
    // No request: nothing.
    let mut t = rec(PC, "80810");
    t.step();
    assert_eq!(t.cpu.regs.pc, PC + 5);
    assert!(!t.cpu.regs.interrupt_pending && !t.cpu.regs.in_interrupt);
}

// ---------------------------------------------------------------- misc

#[test]
fn invalid_opcode_event() {
    let mut t = T::at(PC, "105");
    t.r().a = 42;
    let s = t.step();
    match s.event {
        Some(Event::InvalidOpcode { pc, nibbles, len }) => {
            assert_eq!(pc, PC);
            assert_eq!(len, 3);
            assert_eq!(&nibbles[..3], &[1, 0, 5]);
            assert_eq!(t.cpu.regs.pc, PC + u32::from(len));
        }
        other => panic!("expected InvalidOpcode, got {other:?}"),
    }
    assert_eq!(t.cpu.regs.a, 42);
}

#[test]
fn reset_restores_power_on_state() {
    let mut t = T::at(PC, "2F");
    t.step();
    t.cpu.reset();
    assert_eq!(t.cpu, Cpu::new());
}

#[test]
fn smoke_sum_loop() {
    // 0100: A=0 A          D0
    // 0102: LC(5) 00005    34 50000
    // 0109: loop: A=A+C A  CA
    // 010B: C=C-1 A        CE
    // 010D: GONC loop      5 FB (from 010E: -5)
    // 0110: end
    let mut t = T::at(0x100, "D0 3450000 CA CE 5BF");
    let mut steps = 0;
    while t.cpu.regs.pc != 0x110 {
        t.step();
        steps += 1;
        assert!(steps < 100, "runaway loop");
    }
    assert_eq!(t.cpu.regs.a & 0xF_FFFF, 15);
    assert_eq!(t.cpu.regs.c & 0xF_FFFF, 0xF_FFFF);
    assert!(t.cpu.regs.carry);
    assert_eq!(steps, 2 + 3 * 6);
}
