//! The 49G types right after a cold boot and takes long programs
//! (iteration 64), through the protocol's engine on a hand-moved clock,
//! as the Web Worker and the other hosts drive it. Skipped for each ROM
//! `SATURNUS_ROM_DIR` lacks (`rom-2.10.49g`, `rom.49g` (1.19-6),
//! `gxrom-r` as the control, `sxrom-j`).
//!
//! What the modern-UI study saw (2026-10-18), and what it was:
//! - A `run` within about 3 s of a cold boot failed with "could not bring
//!   the keyboard to AlphaLocked": the send came while "Try To Recover
//!   Memory?" or the 49G's "Memory Clear" box still had the keyboard (the
//!   engine answers them once the screen is up). A send now waits for
//!   the answer.
//! - A 146-point plot program sent in one `run` left stray objects on the
//!   49G's stack: it was typed exactly, but evaluating it takes the 49G
//!   about 38 s, longer than `run`'s 30 s wait, so the reply said
//!   `running` and the stack read then held the program's working
//!   objects. `run` now waits up to 120 s.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::path::PathBuf;

use saturnus_core::Model;
use saturnus_host::protocol::{Clock, Engine, Output, Pacing};
use serde_json::{Value, json};

fn rom(file: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    std::fs::read(PathBuf::from(dir).join(file)).ok()
}

/// A hand-moved clock.
#[derive(Default)]
struct Manual(Cell<f64>);

impl Clock for Manual {
    fn now_ms(&self) -> f64 {
        self.0.get()
    }
}

/// The engine of a host that answers the boot's question (the Worker's).
struct Host {
    engine: Engine,
    clock: Manual,
    tag: u64,
}

impl Host {
    /// A cold boot of `rom` as `model`, nothing run yet.
    fn boot(model: Model, rom: &[u8]) -> Host {
        let mut engine = Engine::new("test", Pacing::NATIVE);
        engine.set_answer_recover(true);
        let mut h = Host {
            engine,
            clock: Manual::default(),
            tag: 0,
        };
        let b = h.engine.boot(&h.clock, model.name(), rom, "rom").unwrap();
        assert!(!b.restored);
        h.take(None);
        h
    }

    fn now(&self) -> f64 {
        self.clock.0.get()
    }

    fn take(&mut self, tag: Option<u64>) -> Option<Result<Value, String>> {
        let mut reply = None;
        for out in self.engine.take_output() {
            if let Output::Reply(r) = out
                && Some(r.tag) == tag
            {
                reply = Some(r.result);
            }
        }
        reply
    }

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
        let cap = self.now() + 300_000.0;
        loop {
            if let Some(r) = self.take(tag) {
                return r;
            }
            assert!(self.fire(cap), "no reply to {msg}");
        }
    }

    fn run(&mut self, text: &str) -> Value {
        self.call(json!({"cmd": "run", "text": text}))
            .unwrap_or_else(|e| panic!("run {text:?}: {e}"))
    }

    /// The stack's levels as the calculator shows them, level 1 first.
    fn stack(&self) -> Vec<String> {
        self.engine
            .emulator()
            .unwrap()
            .stack()
            .unwrap()
            .iter()
            .map(|v| v["text"].as_str().unwrap_or("?").to_string())
            .collect()
    }

    /// The variable `name` of the current directory, as the calculator
    /// shows it.
    fn variable(&self, name: &str) -> Option<String> {
        let m = self.engine.emulator().unwrap().machine();
        let names = saturnus_objects::NameTable::of(m);
        let mem = saturnus_objects::UserMemory::of(m)
            .unwrap()
            .with_names(&names);
        let var = mem.tree().unwrap().into_iter().find(|v| v.name == name)?;
        let obj = mem.object_at(var.address).unwrap();
        Some(saturnus_objects::display(
            &obj,
            &saturnus_objects::Settings::standard(m.model()),
        ))
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn a_send_right_after_a_cold_boot_types() {
    let roms = [
        (Model::Hp49g, "rom-2.10.49g"),
        (Model::Hp49g, "rom.49g"),
        (Model::Hp48gx, "gxrom-r"),
        (Model::Hp48sx, "sxrom-j"),
    ];
    for (model, file) in roms {
        let Some(rom) = rom(file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        // At once (the 49G's RAM holds no command line yet), with the
        // question up and the CPU asleep waiting for a key, and with the
        // 49G's "Memory Clear" box up.
        for at in [0.0, 500.0, 2000.0] {
            let mut h = Host::boot(model, &rom);
            h.run_until(at);
            let starting = h.engine.status().starting;
            let r = h.run("12");
            assert_eq!(r["error"], Value::Null, "{file} at {at} ms: {r}");
            assert_eq!(r["closed"], true, "{file} at {at} ms: {r}");
            let first_ms = r["emulatedMs"].as_f64().unwrap_or(0.0);
            // And the next send right after it.
            let r = h.run("34");
            assert_eq!(r["error"], Value::Null, "{file} at {at} ms: {r}");
            // The 49G boots algebraic: each ENTER leaves its echo too.
            let values: Vec<String> = h
                .stack()
                .into_iter()
                .filter(|l| !l.starts_with(':'))
                .collect();
            assert_eq!(values, ["34", "12"], "{file} at {at} ms");
            assert!(!h.engine.status().starting, "{file}: answered");
            eprintln!(
                "{file} at {at} ms (starting {starting}): the first send took {first_ms:.0} ms emulated"
            );
        }
    }
}

/// The owner's ChatGPT test (2026-10-24): on a freshly started 49G this
/// text failed with "Invalid Syntax" at the FOR, as the 49G boots in
/// algebraic mode, which reads a program as algebraic. In RPN mode
/// (`CF(-95)`, as the AI tools' instructions say; `setFlag` -95 off
/// here) it goes in as on the 48GX, with its line breaks and indentation
/// or without.
#[test]
fn the_owners_programs_need_rpn_mode_on_the_49g() {
    let text = "« → n\n  « 0 1 n FOR i i SQ + NEXT »\n» 'SSQ' STO\n« → a b n\n  « a b FOR x x 2 ^ NEXT n →LIST »\n» 'SQL' STO";
    let one_line = squash(text);
    let roms = [
        (Model::Hp49g, "rom-2.10.49g"),
        (Model::Hp49g, "rom.49g"),
        (Model::Hp48gx, "gxrom-r"),
    ];
    for (model, file) in roms {
        let Some(rom) = rom(file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        if model == Model::Hp49g {
            // As it boots: refused, the line left open.
            let mut h = Host::boot(model, &rom);
            let r = h.run(text);
            assert_eq!(r["error"], "Invalid Syntax", "{file}: {r}");
            assert_eq!(r["closed"], false, "{file}");
        }
        for sent in [text, one_line.as_str()] {
            let mut h = Host::boot(model, &rom);
            if model == Model::Hp49g {
                h.call(json!({"cmd": "setFlag", "flag": -95, "on": false}))
                    .unwrap();
            }
            let r = h.run(sent);
            assert_eq!(r["error"], Value::Null, "{file}: {r}");
            assert_eq!(r["closed"], true, "{file}");
            assert!(h.stack().is_empty(), "{file}: {:?}", h.stack());
            assert_eq!(
                h.variable("SSQ").map(|s| squash(&s)).as_deref(),
                Some("« → n « 0 1 n FOR i i SQ + NEXT » »"),
                "{file}"
            );
            h.run("10 SSQ");
            assert_eq!(h.stack(), ["385"], "{file}");
            h.run("DROP 1 5 5 SQL");
            assert_eq!(h.stack(), ["{ 1 4 9 16 25 }"], "{file}");
        }
    }
}

#[test]
fn a_long_program_runs_as_on_the_48gx() {
    // The study's plot program, as its driver sent it, stored as well.
    let plot = "« -6.2832 6.2832 146 → a b n « 0 n 1 - FOR i a b a - i * n 1 - / + 'X' STO 'SIN(X)' →NUM NEXT n →LIST 'X' PURGE » »";
    // Nested programs, local names, a loop, quoted names, a test, a
    // string and a list, on several lines.
    let other = "« → x y\n  « 0 1 x FOR i i y * + NEXT 'S' STO\n    « S 2 * » EVAL\n    IF DUP 10 > THEN \"big\" ELSE \"small\" END\n    { x y } 'S' PURGE\n  »\n»";
    let roms = [
        (Model::Hp48gx, "gxrom-r"),
        (Model::Hp49g, "rom-2.10.49g"),
        (Model::Hp49g, "rom.49g"),
    ];
    for (model, file) in roms {
        let Some(rom) = rom(file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        let mut h = Host::boot(model, &rom);
        if model == Model::Hp49g {
            // RPN, as the study's driver typed `CF(-95)`; a write at once
            // after the boot waits for the question's answer too.
            h.call(json!({"cmd": "setFlag", "flag": -95, "on": false}))
                .unwrap();
        }
        let r = h.run(&format!("{plot} DUP 'PLT' STO EVAL"));
        assert_eq!(r["error"], Value::Null, "{file}: {r}");
        assert_eq!(r["closed"], true, "{file}: {r}");
        assert_eq!(r["running"], false, "{file}: done when the reply comes");
        assert_eq!(
            squash(&h.variable("PLT").expect("PLT stored")),
            squash(plot),
            "{file}: the program reads back as typed"
        );
        let stack = h.engine.emulator().unwrap().stack().unwrap();
        assert_eq!(stack.len(), 1, "{file}: only the list: {:?}", h.stack());
        assert_eq!(stack[0]["type"], "list", "{file}");
        assert_eq!(stack[0]["items"].as_array().map(Vec::len), Some(146));
        eprintln!(
            "{file}: plot in {:.1} s emulated",
            r["emulatedMs"].as_f64().unwrap() / 1000.0
        );

        h.run("DROP");
        let r = h.run(&format!("{other} 'P2' STO"));
        assert_eq!(r["error"], Value::Null, "{file}: {r}");
        assert_eq!(
            squash(&h.variable("P2").expect("P2 stored")),
            squash(other),
            "{file}"
        );
        h.run("3 4 P2");
        assert_eq!(h.stack(), ["{ x y }", "\"big\"", "48"], "{file}");
    }
}
