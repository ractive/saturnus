//! Auto-save on the real ROMs (iteration 27; skipped unless
//! `SATURNUS_ROM_DIR` holds `sxrom-j`, `gxrom-r`, `rom-2.10.49g`), through
//! the protocol's state machine on a clock moved by hand: values put on
//! the stack and a variable stored, the state the engine hands out after
//! the delay restores a fresh machine with the same stack, variable and
//! screen, no "Try To Recover Memory?". An idle calculator is not saved
//! again, and nothing is saved while a write runs, even with the page
//! hidden; it is saved as soon as the write ends.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::path::PathBuf;

use saturnus::io::Key;
use saturnus::{Machine, Model};
use saturnus_host::protocol::{AUTO_SAVE_MS, Clock, Engine, Output, Pacing, RunError, Saved};
use serde_json::{Value, json};

const MODELS: [(Model, &str); 3] = [
    (Model::Hp48sx, "sxrom-j"),
    (Model::Hp48gx, "gxrom-r"),
    (Model::Hp49g, "rom-2.10.49g"),
];

fn rom(file: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    std::fs::read(PathBuf::from(dir).join(file)).ok()
}

#[derive(Default)]
struct Manual(Cell<f64>);

impl Clock for Manual {
    fn now_ms(&self) -> f64 {
        self.0.get()
    }
}

fn per_ms(m: &Machine) -> u64 {
    u64::from(m.model().clock_hz()) / 1000
}

/// Run until the screen has stayed the same for 300 ms with the CPU
/// asleep, at most `cap_ms`.
fn settle(m: &mut Machine, cap_ms: u64) {
    let end = m.cycles() + cap_ms * per_ms(m);
    let mut last = m.lcd();
    let mut since = m.cycles();
    while m.cycles() < end {
        m.run_cycles(5 * per_ms(m)).unwrap();
        let lcd = m.lcd();
        let lit = lcd.pixels.iter().any(|r| r.iter().any(|&p| p));
        if lcd != last {
            last = lcd;
            since = m.cycles();
        } else if m.is_shutdown() && lit && m.cycles() - since > 300 * per_ms(m) {
            return;
        }
    }
}

/// The engine of a host that keeps states, on a hand-moved clock.
struct Host {
    engine: Engine,
    clock: Manual,
    tag: u64,
    saves: Vec<Saved>,
}

impl Host {
    fn new() -> Host {
        let mut engine = Engine::new("test", Pacing::NATIVE);
        engine.set_auto_save(true);
        Host {
            engine,
            clock: Manual::default(),
            tag: 0,
            saves: Vec::new(),
        }
    }

    fn now(&self) -> f64 {
        self.clock.now_ms()
    }

    /// Keep the saves; the reply to `tag`, if it came.
    fn take(&mut self, tag: Option<u64>) -> Option<Result<Value, String>> {
        let mut reply = None;
        for out in self.engine.take_output() {
            match out {
                Output::Save(s) => self.saves.push(s),
                Output::Reply(r) if Some(r.tag) == tag => reply = Some(r.result),
                _ => {}
            }
        }
        reply
    }

    /// One timer, at its deadline (at most `until`); `false` when none
    /// is due by then.
    fn fire(&mut self, until: f64) -> bool {
        match self.engine.deadline() {
            Some(due) if due <= until => {
                self.clock.0.set(self.now().max(due));
                self.engine.timer(&self.clock);
                true
            }
            _ => false,
        }
    }

    /// Run the timers until wall time `until`.
    fn run_until(&mut self, until: f64) {
        while self.fire(until) {
            self.take(None);
        }
        self.clock.0.set(self.now().max(until));
    }

    /// A command and its reply, the timers running until it comes.
    fn call(&mut self, mut msg: Value) -> Result<Value, String> {
        msg["v"] = json!(1);
        self.tag += 1;
        let tag = Some(self.tag);
        self.engine.command(&self.clock, &msg, None, tag);
        let cap = self.now() + 120_000.0;
        loop {
            if let Some(r) = self.take(tag) {
                return r;
            }
            assert!(self.fire(cap), "no reply to {msg}");
        }
    }

    /// Run `f` on the machine (the engine's exclusive run).
    fn on_machine(&mut self, f: impl FnOnce(&mut Machine)) {
        self.engine
            .exclusive(&self.clock, |mut m| {
                f(&mut m);
                (m, Ok::<(), RunError>(()))
            })
            .unwrap();
        self.take(None);
    }
}

fn stack(e: &Engine) -> Vec<String> {
    e.emulator()
        .unwrap()
        .stack()
        .unwrap()
        .iter()
        .map(|v| v["text"].as_str().unwrap_or("?").to_string())
        .collect()
}

fn variable(e: &Engine, name: &str) -> Option<Value> {
    let tree = e.emulator().unwrap().memory_tree().unwrap();
    let v = tree.variables.iter().find(|v| v.name == name)?;
    Some(serde_json::to_value(v).unwrap())
}

/// A cold boot to the stack, through the engine: the recover question
/// answered as the transfer tests do, the 49G in RPN mode.
fn cold(model: Model, rom: &[u8]) -> Host {
    let mut h = Host::new();
    let b = h.engine.boot(&h.clock, model.name(), rom, "rom").unwrap();
    assert!(!b.restored);
    h.take(None);
    assert!(h.saves.is_empty(), "a boot is not a change");
    let presses = if model == Model::Hp49g { 2 } else { 1 };
    h.on_machine(|m| {
        settle(m, 60_000);
        for _ in 0..presses {
            m.key_down(Key::F).unwrap();
            m.run_cycles(30 * per_ms(m)).unwrap();
            m.key_up(Key::F).unwrap();
            settle(m, 10_000);
        }
    });
    if model == Model::Hp49g {
        h.call(json!({"cmd": "setFlag", "flag": -95, "on": false}))
            .unwrap();
    }
    h
}

#[test]
fn the_calculator_keeps_its_state() {
    for (model, file) in MODELS {
        let Some(rom) = rom(file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        let mut h = cold(model, &rom);
        h.call(json!({"cmd": "run", "text": "42 'V' STO 1 2"}))
            .unwrap();
        let changed = h.now();
        assert_eq!(stack(&h.engine), ["2", "1"], "{model:?}");
        h.run_until(changed + AUTO_SAVE_MS + 1_000.0);
        assert!(!h.saves.is_empty(), "{model:?}: saved after the delay");
        let saved = h.saves.last().unwrap().clone();
        assert_eq!(saved.model, model.name());
        let screen = h.engine.emulator().unwrap().machine().lcd();
        let v = variable(&h.engine, "V").expect("V is stored");

        // Idle: never saved again.
        let n = h.saves.len();
        h.run_until(h.now() + 120_000.0);
        assert_eq!(h.saves.len(), n, "{model:?}: idle, nothing written");

        // A fresh machine from the saved state, before it runs a cycle.
        let mut fresh = Host::new();
        let b = fresh
            .engine
            .boot_restoring(&fresh.clock, model.name(), &rom, "rom", Some(&saved.state))
            .unwrap();
        assert!(b.restored, "{model:?}: {b:?}");
        assert_eq!(stack(&fresh.engine), ["2", "1"], "{model:?}");
        assert_eq!(variable(&fresh.engine, "V"), Some(v), "{model:?}");
        // It runs on at the stack: no recover question, the same screen.
        fresh.run_until(3_000.0);
        assert_eq!(stack(&fresh.engine), ["2", "1"], "{model:?}");
        assert_eq!(
            fresh.engine.emulator().unwrap().machine().lcd(),
            screen,
            "{model:?}: the screen as it was"
        );
        assert!(
            fresh.saves.is_empty(),
            "{model:?}: a restore is not a change"
        );
        // And it works: a key computes on the restored stack.
        fresh.call(json!({"cmd": "run", "text": "+"})).unwrap();
        assert_eq!(stack(&fresh.engine), ["3"], "{model:?}");
        eprintln!(
            "{}: state of {} bytes restored",
            model.name(),
            saved.state.len()
        );
    }
}

#[test]
fn a_write_is_never_saved_half_done() {
    for (model, file) in MODELS {
        let Some(rom) = rom(file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        let mut h = cold(model, &rom);
        h.run_until(h.now() + AUTO_SAVE_MS + 1_000.0);
        let before = h.saves.len();
        // A write through the Kermit server, the page hidden at its start.
        h.tag += 1;
        let tag = Some(h.tag);
        h.engine.command(
            &h.clock,
            &json!({"v": 1, "cmd": "setFlag", "flag": 5, "on": true}),
            None,
            tag,
        );
        h.engine.command(
            &h.clock,
            &json!({"v": 1, "cmd": "visibility", "hidden": true}),
            None,
            None,
        );
        let mut reply = h.take(tag);
        while reply.is_none() {
            assert!(h.engine.status().busy, "{model:?}: the write runs");
            assert_eq!(h.saves.len(), before, "{model:?}: saved mid-write");
            assert!(h.fire(h.now() + 120_000.0));
            reply = h.take(tag);
        }
        reply.unwrap().unwrap();
        // Hidden: saved as soon as it has settled, without the delay.
        h.run_until(h.now() + 1_000.0);
        assert_eq!(h.saves.len(), before + 1, "{model:?}: saved once settled");
        assert!(
            h.engine
                .emulator()
                .unwrap()
                .flags()
                .unwrap()
                .get(5)
                .unwrap()
        );
    }
}
