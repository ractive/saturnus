//! Executed-instruction profile for timing studies (feature `profile`).
//!
//! Counts, per instruction kind, how often it ran and the cycles
//! `cpu::cycles` charged for it, plus totals of opcode nibbles fetched, data
//! nibbles moved by DAT instructions, taken branches, cycles spent in the
//! interrupt handler, and cycles per memory region the PC was in. Used to
//! see where emulated time goes over a benchmark (iteration 7); not part of
//! the emulation and not saved in states.

use std::collections::BTreeMap;

use crate::bus::{Chip, Select};
use crate::cpu::{DatSize, Instruction, Step};

/// Count and charged cycles of one kind of instruction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bucket {
    /// Times executed.
    pub count: u64,
    /// Cycles charged (before the display stall).
    pub cycles: u64,
}

impl Bucket {
    fn add(&mut self, cycles: u32) {
        self.count += 1;
        self.cycles += u64::from(cycles);
    }
}

/// Aggregates over every executed instruction since the last reset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    /// By instruction kind (the `Instruction` variant name).
    pub by_kind: BTreeMap<String, Bucket>,
    /// By memory region the instruction was fetched from.
    pub by_region: BTreeMap<String, Bucket>,
    /// All instructions.
    pub total: Bucket,
    /// Instructions executed inside the interrupt handler.
    pub in_interrupt: Bucket,
    /// Opcode nibbles fetched.
    pub opcode_nibbles: u64,
    /// Data nibbles read by DAT loads and PC=(A)/(C).
    pub data_read_nibbles: u64,
    /// Data nibbles written by DAT stores.
    pub data_write_nibbles: u64,
    /// DAT load / store instructions.
    pub data_reads: u64,
    /// DAT store instructions.
    pub data_writes: u64,
    /// Tests, conditional jumps and returns that were taken.
    pub taken: u64,
    /// Instructions that wrote the PC (jumps, calls, returns, taken
    /// branches).
    pub pc_writes: u64,
    /// Cycles added by the display stall.
    pub stall_cycles: u64,
}

fn kind(instr: &Instruction) -> String {
    let s = format!("{instr:?}");
    s.split([' ', '{', '(']).next().unwrap_or("").to_string()
}

fn dat_nibbles(size: DatSize, p: u8) -> u64 {
    match size {
        DatSize::Field(f) => {
            let (lo, hi) = f.range(p);
            u64::from(hi - lo) + 1
        }
        DatSize::Nibbles(n) => u64::from(n),
    }
}

fn region(sel: Select) -> &'static str {
    match sel {
        Select::Chip { chip, .. } => match chip {
            Chip::Hdw => "hdw",
            Chip::Nce2 => "nce2 (ram)",
            Chip::Ce1 => "ce1",
            Chip::Ce2 => "ce2",
            Chip::Nce3 => "nce3",
        },
        _ => "nce1 (rom)",
    }
}

impl Profile {
    /// Record one executed instruction.
    pub(crate) fn record(&mut self, s: &Step, p: u8, in_interrupt: bool, sel: Select) {
        use Instruction as I;
        self.by_kind
            .entry(kind(&s.instr))
            .or_default()
            .add(s.cycles);
        self.by_region
            .entry(region(sel).to_string())
            .or_default()
            .add(s.cycles);
        self.total.add(s.cycles);
        if in_interrupt {
            self.in_interrupt.add(s.cycles);
        }
        self.opcode_nibbles += u64::from(s.len);
        if s.taken {
            self.taken += 1;
        }
        match s.instr {
            I::DatRead { size, .. } => {
                self.data_reads += 1;
                self.data_read_nibbles += dat_nibbles(size, p);
            }
            I::DatWrite { size, .. } => {
                self.data_writes += 1;
                self.data_write_nibbles += dat_nibbles(size, p);
            }
            I::PcEqInd { .. } => {
                self.data_reads += 1;
                self.data_read_nibbles += 5;
            }
            _ => {}
        }
        let writes_pc = s.taken
            || matches!(
                s.instr,
                I::Rtn
                    | I::RtnSxm
                    | I::RtnSc
                    | I::RtnCc
                    | I::Rti
                    | I::Goto { .. }
                    | I::Gosub { .. }
                    | I::GoLong { .. }
                    | I::GoVLong { .. }
                    | I::GosubL { .. }
                    | I::GosbVL { .. }
                    | I::PcEqReg { .. }
                    | I::PcEqInd { .. }
                    | I::RegPcEx { .. }
            );
        if writes_pc {
            self.pc_writes += 1;
        }
    }

    /// A text report, kinds sorted by charged cycles.
    pub fn report(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let t = self.total;
        let pct = |c: u64| 100.0 * c as f64 / t.cycles.max(1) as f64;
        let _ = writeln!(
            out,
            "instructions {} cycles {} (avg {:.2}/instr) stall {} opcode nibbles {} (avg {:.2})",
            t.count,
            t.cycles,
            t.cycles as f64 / t.count.max(1) as f64,
            self.stall_cycles,
            self.opcode_nibbles,
            self.opcode_nibbles as f64 / t.count.max(1) as f64
        );
        let _ = writeln!(
            out,
            "dat reads {} ({} nibbles) dat writes {} ({} nibbles) taken {} pc writes {} \
             interrupt {} instr / {} cycles ({:.1}%)",
            self.data_reads,
            self.data_read_nibbles,
            self.data_writes,
            self.data_write_nibbles,
            self.taken,
            self.pc_writes,
            self.in_interrupt.count,
            self.in_interrupt.cycles,
            pct(self.in_interrupt.cycles)
        );
        for (r, b) in &self.by_region {
            let _ = writeln!(
                out,
                "region {r:12} {:>10} instr {:>11} cycles {:5.1}%",
                b.count,
                b.cycles,
                pct(b.cycles)
            );
        }
        let mut kinds: Vec<_> = self.by_kind.iter().collect();
        kinds.sort_by_key(|k| std::cmp::Reverse(k.1.cycles));
        for (k, b) in kinds {
            let _ = writeln!(
                out,
                "{k:18} {:>10} instr {:>11} cycles {:5.1}% avg {:.2}",
                b.count,
                b.cycles,
                pct(b.cycles),
                b.cycles as f64 / b.count.max(1) as f64
            );
        }
        out
    }
}
