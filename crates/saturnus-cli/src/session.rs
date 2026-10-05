//! Drives a [`Machine`] in emulated time: fixed runs, key script actions
//! and the "wait until idle" heuristic.

use std::collections::VecDeque;

use anyhow::{Result, bail};
use saturnus::Machine;
use saturnus::cpu::{ADDR_MASK, Decoded, decode, disassemble};
use saturnus::machine::Lcd;

use crate::script::{Action, Line};

/// How often `wait-idle` samples the LCD, in emulated milliseconds.
const IDLE_SAMPLE_MS: u64 = 2;
/// How long the LCD must stay unchanged, with the CPU in SHUTDN, before
/// `wait-idle` treats the calculator as idle, in emulated milliseconds.
const IDLE_STABLE_MS: u64 = 300;

/// A machine plus the bookkeeping for scripted runs.
#[derive(Debug)]
pub struct Session {
    /// The emulated calculator.
    pub machine: Machine,
    cycles_per_ms: u64,
    trace: Option<Trace>,
    verbose: bool,
}

/// Ring buffer of the last executed instructions.
#[derive(Debug)]
struct Trace {
    depth: usize,
    ring: VecDeque<(u32, Decoded)>,
}

impl Session {
    /// Wrap `machine`; `trace_depth > 0` records the last instructions,
    /// which forces instruction-by-instruction stepping.
    pub fn new(machine: Machine, trace_depth: usize, verbose: bool) -> Self {
        let cycles_per_ms = u64::from(machine.model().clock_hz()) / 1000;
        let trace = (trace_depth > 0).then(|| Trace {
            depth: trace_depth,
            ring: VecDeque::with_capacity(trace_depth),
        });
        Self {
            machine,
            cycles_per_ms,
            trace,
            verbose,
        }
    }

    /// Emulated milliseconds to CPU cycles.
    pub fn ms_to_cycles(&self, ms: u64) -> u64 {
        ms.saturating_mul(self.cycles_per_ms)
    }

    /// Run `n` cycles. A CPU halt is an error that carries the trace.
    pub fn run(&mut self, n: u64) -> Result<()> {
        let end = self.machine.cycles().saturating_add(n);
        let result = match self.trace.as_mut() {
            None => self.machine.run_cycles(n),
            Some(t) => {
                let mut r = Ok(());
                // A key change since the last run can wake a shut-down CPU
                // without any time passing; `run_cycles` would then execute
                // an instruction untraced. Detect that on a copy and only
                // wake the CPU (`step` never executes while shut down).
                if self.machine.is_shutdown() {
                    let mut probe = self.machine.clone();
                    if probe.step() == Ok(0) && !probe.is_shutdown() {
                        r = self.machine.step().map(|_| ());
                    }
                }
                while r.is_ok() && self.machine.cycles() < end {
                    if !self.machine.is_shutdown() {
                        let pc = self.machine.cpu.regs.pc;
                        if t.ring.len() == t.depth {
                            t.ring.pop_front();
                        }
                        let m = &self.machine;
                        t.ring
                            .push_back((pc, decode(|a| m.peek(a & ADDR_MASK), pc)));
                        r = self.machine.step().map(|_| ());
                    } else {
                        // One cycle at a time: a wake-up inside the sleep
                        // uses up the budget, so the first instruction
                        // after it is traced on the next iteration.
                        r = self.machine.run_cycles(1);
                    }
                }
                r
            }
        };
        if let Err(h) = result {
            bail!(
                "CPU halted at cycle {}: {h}{}",
                self.machine.cycles(),
                self.trace_text()
            );
        }
        Ok(())
    }

    /// The recorded trace as text (empty without `--trace`).
    pub fn trace_text(&self) -> String {
        let Some(t) = &self.trace else {
            return String::new();
        };
        let mut s = format!("\n--- last {} instructions ---", t.ring.len());
        for (pc, d) in &t.ring {
            s.push_str(&format!("\n#{pc:05X}  {}", disassemble(&d.instr)));
        }
        s
    }

    /// Execute one script line.
    pub fn apply(&mut self, line: &Line) -> Result<()> {
        match line.action {
            Action::Press { key, hold_ms } => {
                self.machine.key_down(key);
                self.run(self.ms_to_cycles(hold_ms))?;
                self.machine.key_up(key);
                self.wait_idle(crate::script::DEFAULT_IDLE_CAP_MS, line.number)?;
            }
            Action::Down(key) => self.machine.key_down(key),
            Action::Up(key) => self.machine.key_up(key),
            Action::Wait { ms } => self.run(self.ms_to_cycles(ms))?,
            Action::WaitIdle { cap_ms } => self.wait_idle(cap_ms, line.number)?,
        }
        Ok(())
    }

    /// Run until the LCD has not changed for [`IDLE_STABLE_MS`] while the
    /// CPU sits in SHUTDN (the ROM's key wait), or until `cap_ms` passed.
    /// Reaching the cap is reported on stderr, not an error: a blinking
    /// cursor or a running program never goes idle.
    fn wait_idle(&mut self, cap_ms: u64, line: usize) -> Result<()> {
        let start = self.machine.cycles();
        let cap = start.saturating_add(self.ms_to_cycles(cap_ms));
        let sample = self.ms_to_cycles(IDLE_SAMPLE_MS);
        let stable = self.ms_to_cycles(IDLE_STABLE_MS);
        let mut last: Lcd = self.machine.lcd();
        let mut since = self.machine.cycles();
        while self.machine.cycles() < cap {
            self.run(sample)?;
            let now = self.machine.cycles();
            let lcd = self.machine.lcd();
            if lcd != last {
                last = lcd;
                since = now;
            } else if now - since >= stable && self.machine.is_shutdown() {
                if self.verbose {
                    eprintln!(
                        "line {line}: idle after {} ms (cycle {now})",
                        (since - start) / self.cycles_per_ms
                    );
                }
                return Ok(());
            }
        }
        eprintln!("warning: key script line {line}: not idle after {cap_ms} ms, continuing");
        Ok(())
    }
}
