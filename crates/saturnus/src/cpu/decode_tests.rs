//! Decoder and disassembler tests: a hand-written opcode table covering
//! every instruction family, wrap-around, and an exhaustive no-panic sweep.
//! Expected opcodes and spellings: SASM manual sections 8-9, cross-checked
//! with Nickel's list and Gariepy's notes.

use super::*;
use crate::cpu::disasm::disassemble;
use crate::cpu::instr::{Field, Instruction, OnTrue, Ptr, Reg};

/// Decode a hex nibble string placed at `pc`; nibbles past its end read 0.
fn dec_at(hex: &str, pc: u32) -> Decoded {
    let nibs: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map_or(0xFF, |d| d as u8))
        .collect();
    assert!(nibs.iter().all(|&n| n < 16), "bad hex in test: {hex}");
    decode(
        |addr| {
            let i = addr.wrapping_sub(pc) & ADDR_MASK;
            nibs.get(i as usize).copied().unwrap_or(0)
        },
        pc,
    )
}

const PC: u32 = 0x10000;

/// (opcode nibbles, length, disassembly) with the instruction at `PC`.
const TABLE: &[(&str, u8, &str)] = &[
    // 0x: returns, modes, RSTK, status, P
    ("00", 2, "RTNSXM"),
    ("01", 2, "RTN"),
    ("02", 2, "RTNSC"),
    ("03", 2, "RTNCC"),
    ("04", 2, "SETHEX"),
    ("05", 2, "SETDEC"),
    ("06", 2, "RSTK=C"),
    ("07", 2, "C=RSTK"),
    ("08", 2, "CLRST"),
    ("09", 2, "C=ST"),
    ("0A", 2, "ST=C"),
    ("0B", 2, "CSTEX"),
    ("0C", 2, "P=P+1"),
    ("0D", 2, "P=P-1"),
    ("0F", 2, "RTI"),
    // 0E: AND / OR
    ("0E00", 4, "A=A&B   P"),
    ("0E37", 4, "C=C&D   X"),
    ("0EF4", 4, "B=B&A   A"),
    ("0E6C", 4, "B=B!A   B"),
    ("0E7A", 4, "C=C!A   W"),
    ("0E8", 3, "NIBHEX  0E8"),
    // 10x-12x: scratch registers
    ("100", 3, "R0=A"),
    ("104", 3, "R4=A"),
    ("10C", 3, "R4=C"),
    ("105", 3, "NIBHEX  105"),
    ("10F", 3, "NIBHEX  10F"),
    ("111", 3, "A=R1"),
    ("11A", 3, "C=R2"),
    ("124", 3, "AR4EX"),
    ("128", 3, "CR0EX"),
    // 13x: pointer <-> register
    ("130", 3, "D0=A"),
    ("131", 3, "D1=A"),
    ("132", 3, "AD0EX"),
    ("133", 3, "AD1EX"),
    ("134", 3, "D0=C"),
    ("137", 3, "CD1EX"),
    ("138", 3, "D0=AS"),
    ("13B", 3, "AD1XS"),
    ("13C", 3, "D0=CS"),
    ("13F", 3, "CD1XS"),
    // 14x/15x: memory transfers
    ("140", 3, "DAT0=A  A"),
    ("143", 3, "A=DAT1  A"),
    ("14E", 3, "C=DAT0  B"),
    ("1527", 4, "A=DAT0  W"),
    ("1551", 4, "DAT1=C  WP"),
    ("15A4", 4, "A=DAT0  5"),
    ("15FF", 4, "C=DAT1  16"),
    ("1580", 4, "DAT0=A  1"),
    ("1508", 4, "NIBHEX  1508"),
    // 16x-1Fx: pointer arithmetic and loads
    ("160", 3, "D0=D0+  1"),
    ("17F", 3, "D1=D1+  16"),
    ("184", 3, "D0=D0-  5"),
    ("1C2", 3, "D1=D1-  3"),
    ("1921", 4, "D0=(2)  #12"),
    ("1A4321", 6, "D0=(4)  #1234"),
    ("1B54321", 7, "D0=(5)  #12345"),
    ("1D0F", 4, "D1=(2)  #F0"),
    ("1E0000", 6, "D1=(4)  #0000"),
    ("1F54321", 7, "D1=(5)  #12345"),
    // 2n, 3x
    ("25", 2, "P=      5"),
    ("2F", 2, "P=      15"),
    ("305", 3, "LC(1)   #5"),
    ("3454321", 7, "LC(5)   #12345"),
    ("3712345678", 10, "LC(8)   #87654321"),
    ("3F0123456789ABCDEF", 18, "LCHEX   FEDCBA9876543210"),
    // 4/5/6/7: short branches
    ("400", 3, "RTNC"),
    ("500", 3, "RTNNC"),
    ("4F0", 3, "GOC     #10010"),
    ("420", 3, "GOC     #10003"),
    ("5FF", 3, "GONC    #10000"),
    ("508", 3, "GONC    #0FF81"),
    ("6300", 4, "NOP4"),
    ("64000", 5, "NOP5"),
    ("6400F", 4, "GOTO    #10005"),
    ("6000", 4, "GOTO    #10001"),
    ("6FFF", 4, "GOTO    #10000"),
    ("6008", 4, "GOTO    #0F801"),
    ("7000", 4, "GOSUB   #10004"),
    ("7FFF", 4, "GOSUB   #10003"),
    // 80x: chip interface
    ("800", 3, "OUT=CS"),
    ("801", 3, "OUT=C"),
    ("802", 3, "A=IN"),
    ("803", 3, "C=IN"),
    ("804", 3, "UNCNFG"),
    ("805", 3, "CONFIG"),
    ("806", 3, "C=ID"),
    ("807", 3, "SHUTDN"),
    ("809", 3, "C+P+1"),
    ("80A", 3, "RESET"),
    ("80B", 3, "BUSCC"),
    ("80C3", 4, "C=P     3"),
    ("80D4", 4, "P=C     4"),
    ("80E", 3, "SREQ?"),
    ("80F5", 4, "CPEX    5"),
    // 808x
    ("8080", 4, "INTON"),
    ("80810", 5, "RSI"),
    ("80811", 5, "NIBHEX  80811"),
    ("808201", 6, "LA(1)   #1"),
    ("8082412345", 10, "LA(5)   #54321"),
    ("8082F0123456789ABCDEF", 21, "LAHEX   FEDCBA9876543210"),
    ("8083", 4, "BUSCB"),
    ("80845", 5, "ABIT=0  5"),
    ("8085F", 5, "ABIT=1  15"),
    ("8086000", 7, "?ABIT=0 0  RTNYES"),
    ("8087310", 7, "?ABIT=1 3  GOYES #10006"),
    ("80882", 5, "CBIT=0  2"),
    ("80893", 5, "CBIT=1  3"),
    ("808A400", 7, "?CBIT=0 4  RTNYES"),
    ("808BEFF", 7, "?CBIT=1 14  GOYES #10004"),
    ("808C", 4, "PC=(A)"),
    ("808D", 4, "BUSCD"),
    ("808E", 4, "PC=(C)"),
    ("808F", 4, "INTOFF"),
    // 81x
    ("810", 3, "ASLC"),
    ("813", 3, "DSLC"),
    ("814", 3, "ASRC"),
    ("817", 3, "DSRC"),
    ("81C", 3, "ASRB"),
    ("81F", 3, "DSRB"),
    ("818F04", 6, "A=A+CON A,5"),
    ("81868F", 6, "A=A-CON B,16"),
    ("8187B0", 6, "D=D-CON W,1"),
    ("818020", 6, "C=C+CON P,1"),
    ("8188", 4, "NIBHEX  8188"),
    ("81804", 5, "NIBHEX  81804"),
    ("819F2", 5, "CSRB.F  A"),
    ("81960", 5, "ASRB.F  B"),
    ("81974", 5, "NIBHEX  81974"),
    ("81A710", 6, "A=R0.F  W"),
    ("81AF0C", 6, "R4=C.F  A"),
    ("81A22A", 6, "CR2EX.F XS"),
    ("81A51B", 6, "C=R3.F  M"),
    ("81A005", 6, "NIBHEX  81A005"),
    ("81A030", 5, "NIBHEX  81A03"),
    ("81AE", 4, "NIBHEX  81AE"),
    ("81B2", 4, "PC=A"),
    ("81B3", 4, "PC=C"),
    ("81B4", 4, "A=PC"),
    ("81B5", 4, "C=PC"),
    ("81B6", 4, "APCEX"),
    ("81B7", 4, "CPCEX"),
    ("81B0", 4, "NIBHEX  81B0"),
    ("81B1", 4, "NIBHEX  81B1"),
    ("81BF", 4, "NIBHEX  81BF"),
    // 82x-8Fx
    ("820", 3, "NOP3"),
    ("821", 3, "XM=0"),
    ("822", 3, "SB=0"),
    ("824", 3, "SR=0"),
    ("828", 3, "MP=0"),
    ("82F", 3, "CLRHST"),
    ("823", 3, "HS=0    3"),
    ("83100", 5, "?XM=0  RTNYES"),
    ("83201", 5, "?SB=0  GOYES #10013"),
    ("83400", 5, "?SR=0  RTNYES"),
    ("83800", 5, "?MP=0  RTNYES"),
    ("83F00", 5, "?HS=0   15  RTNYES"),
    ("83000", 5, "?HS=0   0  RTNYES"),
    ("843", 3, "ST=0    3"),
    ("85F", 3, "ST=1    15"),
    ("86A00", 5, "?ST=0   10  RTNYES"),
    ("872FF", 5, "?ST=1   2  GOYES #10002"),
    ("88300", 5, "?P#     3  RTNYES"),
    ("89F00", 5, "?P=     15  RTNYES"),
    ("8A000", 5, "?A=B    A  RTNYES"),
    ("8A600", 5, "?A#C    A  RTNYES"),
    ("8A300", 5, "?C=D    A  RTNYES"),
    ("8AF00", 5, "?D#0    A  RTNYES"),
    ("8A800", 5, "?A=0    A  RTNYES"),
    ("8B000", 5, "?A>B    A  RTNYES"),
    ("8B200", 5, "?C>A    A  RTNYES"),
    ("8B700", 5, "?D<C    A  RTNYES"),
    ("8B800", 5, "?A>=B   A  RTNYES"),
    ("8BF00", 5, "?D<=C   A  RTNYES"),
    ("8C0000", 6, "GOLONG  #10002"),
    ("8CFFFF", 6, "GOLONG  #10001"),
    ("8C0008", 6, "GOLONG  #08002"),
    ("8D54321", 7, "GOVLNG  #12345"),
    ("8E0000", 6, "GOSUBL  #10006"),
    ("8F54321", 7, "GOSBVL  #12345"),
    // 9: tests with field
    ("90000", 5, "?A=B    P  RTNYES"),
    ("97F00", 5, "?D#0    W  RTNYES"),
    ("91500", 5, "?B#C    WP  RTNYES"),
    ("9B200", 5, "?C>A    X  RTNYES"),
    ("9EC00", 5, "?A<=B   B  RTNYES"),
    ("9CB10", 5, "?D>=C   S  GOYES #10004"),
    // A/B with field
    ("A00", 3, "A=A+B   P"),
    ("A74", 3, "A=A+A   W"),
    ("A6C", 3, "A=A-1   B"),
    ("A38", 3, "B=B+A   X"),
    ("A80", 3, "A=0     P"),
    ("AF4", 3, "A=B     W"),
    ("AD9", 3, "C=B     M"),
    ("AEC", 3, "ABEX    B"),
    ("AFF", 3, "CDEX    W"),
    ("B00", 3, "A=A-B   P"),
    ("B74", 3, "A=A+1   W"),
    ("B3C", 3, "A=B-A   X"),
    ("B2B", 3, "C=C-D   XS"),
    ("B88", 3, "A=-A    P"),
    ("BFC", 3, "A=-A-1  W"),
    ("BF0", 3, "ASL     W"),
    ("BF7", 3, "DSR     W"),
    // C-F: A field
    ("C0", 2, "A=A+B   A"),
    ("CB", 2, "C=C+D   A"),
    ("CF", 2, "D=D-1   A"),
    ("D0", 2, "A=0     A"),
    ("D7", 2, "D=C     A"),
    ("DE", 2, "ACEX    A"),
    ("E0", 2, "A=A-B   A"),
    ("EE", 2, "C=A-C   A"),
    ("E7", 2, "D=D+1   A"),
    ("F0", 2, "ASL     A"),
    ("F5", 2, "BSR     A"),
    ("FB", 2, "D=-D    A"),
    ("FF", 2, "D=-D-1  A"),
];

#[test]
fn opcode_table() {
    let mut failures = Vec::new();
    for &(hex, len, text) in TABLE {
        let d = dec_at(hex, PC);
        let got = disassemble(&d.instr);
        if d.len != len || got != text {
            failures.push(format!(
                "{hex}: expected ({len}, {text:?}), got ({}, {got:?}) {:?}",
                d.len, d.instr
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn decoded_values() {
    assert_eq!(
        dec_at("3454321", PC).instr,
        Instruction::LoadConst {
            reg: Reg::C,
            nibbles: 5,
            value: 0x12345,
        }
    );
    assert_eq!(
        dec_at("1B54321", PC).instr,
        Instruction::PtrLoad {
            ptr: Ptr::D0,
            nibbles: 5,
            value: 0x12345,
        }
    );
    assert_eq!(
        dec_at("8A010", PC).instr,
        Instruction::TestCmp {
            op: crate::cpu::instr::Cmp::Eq,
            lhs: Reg::A,
            rhs: Reg::B,
            field: Field::A,
            yes: OnTrue::GoYes(PC + 3 + 1),
        }
    );
    assert_eq!(
        dec_at("818F04", PC).instr,
        Instruction::AddConst {
            reg: Reg::A,
            field: Field::A,
            n: 5,
        }
    );
    match dec_at("81B0", PC).instr {
        Instruction::Invalid { nibbles, len } => {
            assert_eq!(len, 4);
            assert_eq!(&nibbles[..4], &[8, 1, 0xB, 0]);
        }
        other => panic!("81B0 decoded as {other:?}"),
    }
}

#[test]
fn addresses_wrap_at_20_bits() {
    // GOVLNG straddling the top of the address space.
    let d = dec_at("8D54321", 0xFFFFE);
    assert_eq!(d.len, 7);
    assert_eq!(d.instr, Instruction::GoVLong { target: 0x12345 });
    // Forward GOTO from the last nibble wraps to the bottom.
    let d = dec_at("6100", 0xFFFFF);
    assert_eq!(d.instr, Instruction::Goto { target: 0x00001 });
    // Backward GOC from address 0 wraps to the top.
    let d = dec_at("4EF", 0);
    assert_eq!(d.instr, Instruction::Goc { target: 0xFFFFF });
    // pc itself is masked.
    let d = dec_at("01", 0x1F_FFFF);
    assert_eq!(d.instr, Instruction::Rtn);
}

/// Decode a 16-nibble buffer (zeros after the prefix, reads past its end
/// return 0) and check the invariants: no panic, length 1..=21, the decoder
/// read only the nibbles it reports (plus the one-nibble NOP5 lookahead),
/// and the disassembler accepts the result.
fn check_prefix(buf: &[u8; 16]) {
    let mut max_read = 0u32;
    let d = decode(
        |a| {
            max_read = max_read.max(a + 1);
            buf.get(a as usize).copied().unwrap_or(0)
        },
        0,
    );
    assert!((1..=21).contains(&d.len), "{buf:?}: len {}", d.len);
    let lookahead = matches!(d.instr, Instruction::Goto { .. }) && d.len == 4;
    let allowed = u32::from(d.len) + u32::from(lookahead);
    assert!(
        max_read <= allowed,
        "{buf:?}: read {max_read} nibbles, len {}",
        d.len
    );
    if let Instruction::Invalid { len, .. } = d.instr {
        assert_eq!(len, d.len);
    }
    let text = disassemble(&d.instr);
    assert!(!text.is_empty());
}

#[test]
fn exhaustive_prefixes_never_panic() {
    // Every 1-, 2- and 3-nibble prefix.
    for len in 1..=3u32 {
        for v in 0..16u32.pow(len) {
            let mut buf = [0u8; 16];
            for i in 0..len {
                buf[i as usize] = ((v >> (4 * i)) & 0xF) as u8;
            }
            check_prefix(&buf);
        }
    }
    // Every 4-nibble prefix starting with 1 or 8, and every 5-nibble
    // prefix starting with 80 or 81 (where the long encodings live).
    for first in [1u8, 8] {
        for v in 0..0x1000u32 {
            let mut buf = [0u8; 16];
            buf[0] = first;
            for i in 0..3 {
                buf[i + 1] = ((v >> (4 * i)) & 0xF) as u8;
            }
            check_prefix(&buf);
        }
    }
    for second in [0u8, 1] {
        for v in 0..0x1000u32 {
            let mut buf = [0u8; 16];
            buf[0] = 8;
            buf[1] = second;
            for i in 0..3 {
                buf[i + 2] = ((v >> (4 * i)) & 0xF) as u8;
            }
            check_prefix(&buf);
        }
    }
    // All-F tails exercise the longest forms (LA/LC with 16 nibbles).
    for first in 0..16u8 {
        let mut b = [0xFu8; 16];
        b[0] = first;
        check_prefix(&b);
    }
}
