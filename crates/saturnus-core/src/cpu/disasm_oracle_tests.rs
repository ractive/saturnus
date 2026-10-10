//! Oracle test against HP's own assembler opcode table, `SASM.OPC` from the
//! 1993 HP48 SDK. Gated on `SATURNUS_LITERATURE_DIR`; skipped (with a note)
//! when unset. The file is read at test time and never copied into the repo.
//!
//! File layout (inspected with xxd): a header line `Opcodes-4:<count>\n`,
//! then fixed 66-byte records. Per record, at byte offset:
//!
//! - 0..16  mnemonic, NUL padded
//! - 16..18 flags (not used here)
//! - 18..25 primary opcode template, ASCII hex, NUL padded; empty for
//!   pseudo-ops
//! - 25..39 alternate opcode template (the A-field short form, or the
//!   whole-register short form of a `.F` instruction)
//! - 39     nibble position of the field selector code (0: none)
//! - 40     nibble position of the numeric parameter
//! - 41     parameter length in nibbles (0: none, 0xFF: variable)
//! - 42..46 parameter bias, i32 little-endian (encoded = value + bias)
//! - 46     1 when the parameter is a relative branch offset
//! - 48     field code for which the alternate template is used (F = A
//!   field, 7 = W field)
//!
//! Every record is instantiated with each field selector and with
//! representative parameter values, decoded, disassembled and compared with
//! the mnemonic text (whitespace normalised). Several mnemonics share one
//! opcode (?A=B / ?B=A, ?ST#1 / ?ST=0, D0=HEX / D0=(2) ...); an
//! instantiation whose disassembly is another record's text for the same
//! nibbles counts as verified through a synonym.

use std::collections::HashMap;

use crate::cpu::decode::decode;
use crate::cpu::disasm::disassemble;

const RECORD: usize = 66;
const PC: u32 = 0x10000;

struct Record {
    name: String,
    t1: Vec<u8>,
    t2: Vec<u8>,
    field_pos: usize,
    param_pos: usize,
    param_len: u8,
    bias: i32,
    relative: bool,
    alt_code: u8,
}

/// Parse an ASCII hex template; `None` if it contains non-hex characters.
fn hex(s: &[u8]) -> Option<Vec<u8>> {
    s.iter()
        .take_while(|&&b| b != 0)
        .map(|&b| char::from(b).to_digit(16).map(|d| d as u8))
        .collect()
}

fn cstr(s: &[u8]) -> String {
    s.iter()
        .take_while(|&&b| b != 0)
        .map(|&b| char::from(b))
        .collect()
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

const FIELDS: [&str; 8] = ["P", "WP", "XS", "X", "S", "M", "B", "W"];

/// One concrete opcode with the text the record says it stands for.
struct Inst {
    rec: usize,
    nibbles: Vec<u8>,
    expected: String,
}

/// Why a record cannot be instantiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Skip {
    /// No opcode: assembler directive or data pseudo-op.
    PseudoOp,
    /// Template is not hex (GOSHORT/JUMP name other mnemonics).
    Macro,
    /// Second half of a test (GOYES/RTNYES) on its own.
    TestHalf,
    /// LC(N)/LA(N): count-only start of a multi-line load.
    CountOnlyLoad,
    /// LCASC/LAASC: ASCII operand.
    AsciiLoad,
    /// NIBFS: emits a field code as data.
    DataNibble,
}

fn classify(r: &Record, raw_t1: &[u8]) -> Option<Skip> {
    let n = r.name.as_str();
    if raw_t1.first().is_none_or(|&b| b == 0) {
        return Some(Skip::PseudoOp);
    }
    if hex(raw_t1).is_none() {
        return Some(Skip::Macro);
    }
    if n == "RTNYES" || n == "GOYES" {
        return Some(Skip::TestHalf);
    }
    if n.ends_with("(N)") {
        return Some(Skip::CountOnlyLoad);
    }
    if n.ends_with("ASC") {
        return Some(Skip::AsciiLoad);
    }
    if n == "NIBFS" {
        return Some(Skip::DataNibble);
    }
    None
}

fn put(v: &mut Vec<u8>, pos: usize, n: u8) {
    if v.len() <= pos {
        v.resize(pos + 1, 0);
    }
    v[pos] = n;
}

/// Little-endian value of nibbles.
fn le(n: &[u8]) -> u64 {
    n.iter()
        .rev()
        .fold(0u64, |acc, &d| (acc << 4) | u64::from(d))
}

/// Instantiate a record into concrete opcodes with expected text.
fn instantiate(idx: usize, r: &Record) -> Vec<Inst> {
    let name = r.name.as_str();
    let is_test = name.starts_with('?');
    let fsd = r.field_pos != 0 && r.field_pos == r.param_pos;

    // (template, field text)
    let mut forms: Vec<(Vec<u8>, Option<String>)> = Vec::new();
    if r.field_pos == 0 {
        forms.push((r.t1.clone(), None));
    } else {
        for (code, f) in FIELDS.iter().enumerate() {
            let mut t = r.t1.clone();
            let base = t.get(r.field_pos).copied().unwrap_or(0);
            put(&mut t, r.field_pos, base + code as u8);
            forms.push((t, Some((*f).to_string())));
        }
        if !r.t2.is_empty() && r.alt_code == 0xF {
            forms.push((r.t2.clone(), Some("A".to_string())));
        } else {
            let mut t = r.t1.clone();
            put(&mut t, r.field_pos, 0xF);
            forms.push((t, Some("A".to_string())));
        }
        if !r.t2.is_empty() && r.alt_code == 7 {
            forms.push((r.t2.clone(), Some("W".to_string())));
        }
        if fsd {
            // Nibble-count forms: operation nibble + 8, count - 1.
            for d in 1..=16u8 {
                let mut t = r.t1.clone();
                let op = t[r.field_pos - 1] + 8;
                put(&mut t, r.field_pos - 1, op);
                put(&mut t, r.field_pos, d - 1);
                forms.push((t, Some(d.to_string())));
            }
        }
    }

    let mut out = Vec::new();
    for (t, field) in forms {
        // (opcode, parameter text)
        let mut params: Vec<(Vec<u8>, Option<String>)> = Vec::new();
        if r.param_len == 0 || fsd {
            params.push((t, None));
        } else if r.param_len == 0xFF {
            // Variable-length hex loads: D0=HEX/D1=HEX with 2 digits,
            // LCHEX/LAHEX with 16 (count nibble just before the digits).
            let digits: usize = if name.starts_with('D') { 2 } else { 16 };
            let mut v = t.clone();
            if digits == 16 {
                put(&mut v, r.param_pos - 1, 15);
            }
            let ds: Vec<u8> = (1..=digits).map(|i| (i % 16) as u8).collect();
            for (i, &d) in ds.iter().enumerate() {
                put(&mut v, r.param_pos + i, d);
            }
            params.push((v, Some(format!("{:0w$X}", le(&ds), w = digits))));
        } else if r.param_len == 1 && !r.relative {
            for raw in 0..16u8 {
                let mut v = t.clone();
                put(&mut v, r.param_pos, raw);
                let value = i32::from(raw) - r.bias;
                // LC(1)/LA(1) load a hex constant; the rest are counts.
                let text = if name.contains('(') {
                    format!("#{value:X}")
                } else {
                    value.to_string()
                };
                params.push((v, Some(text)));
            }
        } else {
            let len = usize::from(r.param_len);
            let ds: Vec<u8> = (1..=len).map(|i| i as u8).collect();
            let mut v = t.clone();
            for (i, &d) in ds.iter().enumerate() {
                put(&mut v, r.param_pos + i, d);
            }
            let raw = le(&ds);
            let text = if r.relative {
                let bits = 4 * len as u32;
                let off = ((raw << (64 - bits)) as i64 >> (64 - bits)) as i32;
                // Calls count from the end of the instruction, jumps from the
                // first offset nibble (SASM manual 6.5-6.6 ranges).
                let base =
                    PC as usize + r.param_pos + if name.starts_with("GOSUB") { len } else { 0 };
                format!("#{:05X}", (base as u32).wrapping_add_signed(off) & 0xF_FFFF)
            } else if name.starts_with("GO") {
                format!("#{raw:05X}")
            } else {
                format!("#{raw:0len$X}")
            };
            params.push((v, Some(text)));
        }
        for (v, ptext) in params {
            let modifier = match (&field, &ptext) {
                (Some(f), Some(p)) => format!("{f},{p}"),
                (Some(f), None) => f.clone(),
                (None, Some(p)) => p.clone(),
                (None, None) => String::new(),
            };
            let base_text = norm(&format!("{name} {modifier}"));
            if is_test {
                let yy = v.len();
                let mut rtn = v.clone();
                put(&mut rtn, yy + 1, 0);
                out.push(Inst {
                    rec: idx,
                    nibbles: rtn,
                    expected: format!("{base_text} RTNYES"),
                });
                let mut go = v.clone();
                put(&mut go, yy, 4);
                put(&mut go, yy + 1, 0);
                out.push(Inst {
                    rec: idx,
                    nibbles: go,
                    expected: format!("{base_text} GOYES #{:05X}", PC as usize + yy + 4),
                });
            } else {
                out.push(Inst {
                    rec: idx,
                    nibbles: v,
                    expected: base_text,
                });
            }
        }
    }
    out
}

#[test]
fn sasm_opc_oracle() {
    let Ok(dir) = std::env::var("SATURNUS_LITERATURE_DIR") else {
        println!("SATURNUS_LITERATURE_DIR not set: SASM.OPC oracle test skipped");
        return;
    };
    let path = std::path::Path::new(&dir).join("raw/saturn-hardware/hp48-sdk-1993/SASM.OPC");
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let Some(nl) = data.iter().position(|&b| b == b'\n') else {
        panic!("SASM.OPC: no header line");
    };
    let header = String::from_utf8_lossy(&data[..nl]).to_string();
    let Some(count) = header
        .rsplit(':')
        .next()
        .and_then(|s| s.trim().parse::<usize>().ok())
    else {
        panic!("SASM.OPC: no record count in header {header:?}");
    };
    let body = &data[nl + 1..];
    assert_eq!(body.len(), count * RECORD, "unexpected SASM.OPC size");

    let mut records = Vec::new();
    let mut skipped: HashMap<Skip, Vec<String>> = HashMap::new();
    let mut insts = Vec::new();
    for (idx, raw) in body.as_chunks::<RECORD>().0.iter().enumerate() {
        let r = Record {
            name: cstr(&raw[0..16]),
            t1: hex(&raw[18..25]).unwrap_or_default(),
            t2: hex(&raw[25..39]).unwrap_or_default(),
            field_pos: usize::from(raw[39]),
            param_pos: usize::from(raw[40]),
            param_len: raw[41],
            bias: i32::from_le_bytes([raw[42], raw[43], raw[44], raw[45]]),
            relative: raw[46] == 1,
            alt_code: raw[48],
        };
        match classify(&r, &raw[18..25]) {
            Some(s) => skipped.entry(s).or_default().push(r.name.clone()),
            None => insts.extend(instantiate(idx, &r)),
        }
        records.push(r);
    }

    // All texts that name each concrete opcode.
    let mut names: HashMap<Vec<u8>, Vec<String>> = HashMap::new();
    for i in &insts {
        names
            .entry(i.nibbles.clone())
            .or_default()
            .push(i.expected.clone());
    }

    // Per record: (exact matches, synonym matches, failures).
    let mut per_rec: HashMap<usize, (usize, usize, usize)> = HashMap::new();
    let mut failures = Vec::new();
    for i in &insts {
        let nibs = &i.nibbles;
        let d = decode(
            |a| {
                let k = a.wrapping_sub(PC) & 0xF_FFFF;
                nibs.get(k as usize).copied().unwrap_or(0)
            },
            PC,
        );
        let got = norm(&disassemble(&d.instr));
        let e = per_rec.entry(i.rec).or_default();
        let len_ok = usize::from(d.len) == nibs.len();
        if len_ok && got == i.expected {
            e.0 += 1;
        } else if len_ok && names.get(nibs).is_some_and(|v| v.contains(&got)) {
            e.1 += 1;
        } else {
            e.2 += 1;
            let hexs: String = nibs.iter().map(|n| format!("{n:X}")).collect();
            failures.push(format!(
                "{} [{hexs}]: expected {:?} (len {}), got {got:?} (len {})",
                records[i.rec].name,
                i.expected,
                nibs.len(),
                d.len
            ));
        }
    }

    let exact = per_rec.values().filter(|c| c.2 == 0 && c.1 == 0).count();
    let synonym = per_rec.values().filter(|c| c.2 == 0 && c.1 > 0).count();
    let failed = per_rec.values().filter(|c| c.2 > 0).count();
    println!(
        "SASM.OPC: {count} records; verified {}/{count} ({exact} exact, {synonym} via synonym), \
         {failed} failed, {} instantiations",
        exact + synonym,
        insts.len()
    );
    let mut kinds: Vec<_> = skipped.iter().collect();
    kinds.sort();
    for (kind, names) in kinds {
        println!("  not verifiable, {kind:?}: {}", names.len());
    }
    assert_eq!(
        exact + synonym + failed + skipped.values().map(Vec::len).sum::<usize>(),
        count
    );
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
