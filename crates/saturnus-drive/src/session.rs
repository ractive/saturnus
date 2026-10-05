//! Drives a [`Machine`] in emulated time: fixed runs, key script actions
//! and the "wait until idle" heuristic.

use std::collections::VecDeque;

use anyhow::{Context, Result, bail};
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
    /// Print warnings on stderr as they happen (the CLI) or only collect
    /// them for [`Session::take_warnings`] (the MCP server).
    echo_warnings: bool,
    warnings: Vec<String>,
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
            echo_warnings: true,
            warnings: Vec::new(),
        }
    }

    /// Whether warnings (a `wait-idle` that hit its cap) also go to stderr;
    /// on by default. They are collected either way.
    pub fn set_echo_warnings(&mut self, echo: bool) {
        self.echo_warnings = echo;
    }

    /// The warnings since the last call.
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    /// CPU cycles per emulated millisecond.
    pub fn cycles_per_ms(&self) -> u64 {
        self.cycles_per_ms
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

    /// Check that every key `lines` use is on the running model's
    /// keyboard, before anything runs: a key from the other model's set
    /// (`prg` on the 49G, `apps` on a 48) would otherwise do nothing and
    /// leave a wrong screen.
    pub fn check_keys(&self, lines: &[Line]) -> Result<()> {
        for line in lines {
            let key = match line.action {
                Action::Press { key, .. } | Action::Down(key) | Action::Up(key) => key,
                Action::Wait { .. } | Action::WaitIdle { .. } => continue,
            };
            if !self.machine.has_key(key) {
                bail!(
                    "key \"{}\" is not on the {} keyboard (line {})",
                    key.name(),
                    self.machine.model().name(),
                    line.number
                );
            }
        }
        Ok(())
    }

    /// Execute one script line. A key the model lacks is an error (see
    /// [`Session::check_keys`]).
    pub fn apply(&mut self, line: &Line) -> Result<()> {
        let at = || format!("line {}", line.number);
        match line.action {
            Action::Press { key, hold_ms } => {
                self.machine.key_down(key).with_context(at)?;
                self.run(self.ms_to_cycles(hold_ms))?;
                self.machine.key_up(key).with_context(at)?;
                self.wait_idle(crate::script::DEFAULT_IDLE_CAP_MS, line.number)?;
            }
            Action::Down(key) => self.machine.key_down(key).with_context(at)?,
            Action::Up(key) => self.machine.key_up(key).with_context(at)?,
            Action::Wait { ms } => self.run(self.ms_to_cycles(ms))?,
            Action::WaitIdle { cap_ms } => {
                self.wait_idle(cap_ms, line.number)?;
            }
        }
        Ok(())
    }

    /// Whether a stable screen counts as settled: something is drawn, or
    /// the display is switched off (an OFF calculator). A lit display that
    /// stays blank is a boot still in progress: the 49G ROM spends seconds
    /// in SHUTDN timer waits with an empty screen before its first prompt
    /// (the saturnng container's `wait_stable` also waits for lit pixels).
    fn settled(&self, lcd: &Lcd) -> bool {
        !self.machine.hw.io.display_on() || lcd.pixels.iter().any(|r| r.iter().any(|&p| p))
    }

    /// Run until the LCD has not changed for [`IDLE_STABLE_MS`] while the
    /// CPU sits in SHUTDN (the ROM's key wait) with a settled
    /// screen ([`Self::settled`]), or until `cap_ms` passed.
    /// Reaching the cap is a warning (see [`Session::take_warnings`]), not
    /// an error: a blinking cursor or a running program never goes idle.
    /// Returns whether the calculator went idle.
    pub fn wait_idle(&mut self, cap_ms: u64, line: usize) -> Result<bool> {
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
            } else if now - since >= stable && self.machine.is_shutdown() && self.settled(&lcd) {
                if self.verbose {
                    eprintln!(
                        "line {line}: idle after {} ms (cycle {now})",
                        (since - start) / self.cycles_per_ms
                    );
                }
                return Ok(true);
            }
        }
        let warning = format!("key script line {line}: not idle after {cap_ms} ms, continuing");
        if self.echo_warnings {
            eprintln!("warning: {warning}");
        }
        self.warnings.push(warning);
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use saturnus::Model;

    fn session(model: Model) -> Session {
        let machine = Machine::new(model, &vec![0u8; model.rom_bytes()]).unwrap();
        Session::new(machine, 0, false)
    }

    #[test]
    fn keys_off_the_model_are_refused_before_running() {
        let script = crate::script::parse("wait 10\nenter\nprg\n").unwrap();
        let err = session(Model::Hp49g).check_keys(&script).unwrap_err();
        assert_eq!(
            err.to_string(),
            "key \"prg\" is not on the 49g keyboard (line 3)"
        );
        let err = session(Model::Hp48sx)
            .check_keys(&crate::script::parse("down apps\n").unwrap())
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "key \"apps\" is not on the 48sx keyboard (line 1)"
        );
        assert!(session(Model::Hp48sx).check_keys(&script).is_ok());
    }

    #[test]
    fn apply_refuses_a_key_off_the_model() {
        let mut s = session(Model::Hp49g);
        let line = &crate::script::parse("down mth\n").unwrap()[0];
        let err = s.apply(line).unwrap_err();
        assert_eq!(
            format!("{err:#}"),
            "line 1: key \"mth\" is not on the 49g keyboard"
        );
    }
}
