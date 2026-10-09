//! A cold boot's "Try To Recover Memory?" answered for the user, on the
//! real ROMs (skipped unless `SATURNUS_ROM_DIR` holds `sxrom-j`,
//! `gxrom-r`, `rom-2.10.49g`), through the protocol's state machine on a
//! clock moved by hand: a host that keeps the calculator lands on an
//! empty stack that takes typing; one that does not (key scripts) still
//! sees the question; a key pressed first leaves it to the user.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::path::PathBuf;

use saturnus_host::protocol::{Clock, Engine, Output, Pacing};
use serde_json::{Value, json};

const MODELS: [(&str, &str); 3] = [
    ("48sx", "sxrom-j"),
    ("48gx", "gxrom-r"),
    ("49g", "rom-2.10.49g"),
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

struct Host {
    engine: Engine,
    clock: Manual,
    tag: u64,
}

impl Host {
    fn new(answer: bool) -> Host {
        let mut engine = Engine::new("test", Pacing::NATIVE);
        engine.set_auto_save(true);
        engine.set_answer_recover(answer);
        Host {
            engine,
            clock: Manual::default(),
            tag: 0,
        }
    }

    fn now(&self) -> f64 {
        self.clock.now_ms()
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

    fn run_until(&mut self, until: f64) {
        while self.fire(until) {
            self.take(None);
        }
        self.clock.0.set(self.now().max(until));
    }

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

    fn boot(&mut self, model: &str, rom: &[u8]) {
        self.engine.boot(&self.clock, model, rom, "rom").unwrap();
    }

    /// The display as text rows, `#` dark.
    fn screen(&self) -> Vec<String> {
        let e = self.engine.emulator().unwrap();
        e.machine()
            .framebuffer()
            .pixels
            .pixels
            .iter()
            .map(|r| r.iter().map(|&p| if p { '#' } else { '.' }).collect())
            .collect()
    }
}

/// The boot's question: the menu row has labels under the first and the
/// sixth key only (the same shape `recover.rs` looks for).
fn asks(rows: &[String]) -> bool {
    let label = |i: usize| {
        rows[56..]
            .iter()
            .map(|r| r[i * 22..i * 22 + 21].matches('#').count())
            .sum::<usize>()
    };
    label(0) > 0 && label(5) > 0 && (1..5).all(|i| label(i) == 0)
}

#[test]
fn a_host_that_keeps_the_calculator_answers_no() {
    for (model, file) in MODELS {
        let Some(rom) = rom(file) else {
            eprintln!("skipped: SATURNUS_ROM_DIR/{file} not found");
            continue;
        };
        let mut h = Host::new(true);
        h.boot(model, &rom);
        let mut t = 0.0;
        while h.engine.answering_recover() && t < 60_000.0 {
            t += 500.0;
            h.run_until(t);
        }
        assert!(
            !h.engine.answering_recover(),
            "{model}: still waiting after {t} ms"
        );
        h.run_until(t + 3_000.0);
        let screen = h.screen();
        assert!(
            !asks(&screen),
            "{model}: the question is still up:\n{}",
            screen.join("\n")
        );
        // An empty stack that takes typing (a number: the 49G starts algebraic).
        assert_eq!(
            h.call(json!({"cmd": "stack"})).unwrap(),
            json!([]),
            "{model}"
        );
        let r = h
            .call(json!({"cmd": "run", "text": "3"}))
            .unwrap_or_else(|e| panic!("{model}: {e}\n{}", h.screen().join("\n")));
        assert_eq!(r["error"], Value::Null, "{model}: {r}");
        let levels = h.call(json!({"cmd": "stack"})).unwrap();
        assert_eq!(levels[0]["text"], "3", "{model}: {levels}");
    }
}

#[test]
fn key_scripts_and_a_first_key_leave_the_question_to_the_user() {
    for (model, file) in MODELS {
        let Some(rom) = rom(file) else {
            eprintln!("skipped: SATURNUS_ROM_DIR/{file} not found");
            continue;
        };
        // Not answered by a host that does not keep the calculator.
        let mut h = Host::new(false);
        h.boot(model, &rom);
        h.run_until(30_000.0);
        assert!(asks(&h.screen()), "{model}: the question, unanswered");
        // A key of the user's first: theirs to answer.
        let mut h = Host::new(true);
        h.boot(model, &rom);
        h.call(json!({"cmd": "keyDown", "key": "a"})).unwrap();
        h.call(json!({"cmd": "keyUp", "key": "a"})).unwrap();
        assert!(!h.engine.answering_recover(), "{model}: a key ends it");
    }
}
