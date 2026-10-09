//! The engine with fake clocks: the loop's pacing against a fake core
//! (the speed, sleep and wake, catch-up, owed time, hidden), the memory
//! watch's rules, and every command on a 48SX with a ROM of zeros (it
//! computes forever and never sleeps, which keeps a send in progress).

use std::cell::Cell;
use std::rc::Rc;

use saturnus::Model;
use serde_json::{Value, json};

use super::pacing::{Core, Loop};
use super::watch::Watch;
use super::*;

/// A clock the test moves by hand, shared with a fake core that spends
/// wall time.
#[derive(Clone, Default)]
struct Manual(Rc<Cell<f64>>);

impl Clock for Manual {
    fn now_ms(&self) -> f64 {
        self.0.get()
    }
}

impl Manual {
    fn set(&self, t: f64) {
        self.0.set(t);
    }
    fn advance(&self, ms: f64) {
        self.0.set(self.0.get() + ms);
    }
}

/// A clock that moves `step` ms each time it is read: budgets run out.
struct Ticking(Cell<f64>, f64);

impl Ticking {
    fn set(&self, t: f64) {
        self.0.set(self.0.get().max(t));
    }
}

impl Clock for Ticking {
    fn now_ms(&self) -> f64 {
        let t = self.0.get();
        self.0.set(t + self.1);
        t
    }
}

/// A fake core with an emulated clock: the CPU computes until
/// `busy_until`, otherwise sleeps in SHUTDN until its next timer event
/// (every 500 ms, which wakes it for 0.5 ms, as a cursor blink: shorter
/// than the millisecond a wake runs past the event, as the ROM's are). Computing
/// costs `cost` wall ms per emulated ms on `wall`; sleeping costs nothing.
struct Fake {
    now: f64,
    busy_until: f64,
    keys: bool,
    cost: f64,
    wall: Manual,
    memory: u64,
}

const TICK: f64 = 500.0;

impl Fake {
    fn new(wall: &Manual) -> Fake {
        Fake {
            now: 0.0,
            busy_until: 0.0,
            keys: false,
            cost: 0.0,
            wall: wall.clone(),
            memory: 0,
        }
    }
}

impl Core for Fake {
    fn run_slice(&mut self, left: f64, _keys: bool) -> crate::Result<f64> {
        let step = match self.idle_ms() {
            None => left.min((self.busy_until - self.now).min(1.0)),
            Some(idle) => left.min(idle),
        };
        let busy = self.now < self.busy_until;
        let tick = (self.now / TICK).floor();
        self.now += step;
        if busy {
            self.wall.advance(step * self.cost);
        }
        if (self.now / TICK).floor() > tick {
            self.busy_until = self.busy_until.max(self.now + 0.5);
        }
        Ok(step)
    }
    fn idle_ms(&self) -> Option<f64> {
        if self.now < self.busy_until || self.keys {
            return None;
        }
        Some(((self.now / TICK).floor() + 1.0) * TICK - self.now)
    }
    fn keys_busy(&self) -> bool {
        self.keys
    }
    fn cycles(&self) -> u64 {
        (self.now * 1000.0) as u64
    }
    fn memory_changes(&self) -> crate::Result<u64> {
        Ok(self.memory)
    }
}

/// Fire the loop's timers as a host would, up to wall time `until`.
fn run_until(lp: &mut Loop, core: &mut Fake, clock: &Manual, until: f64) {
    while let Some(due) = lp.due() {
        if due > until {
            break;
        }
        clock.set(clock.now_ms().max(due));
        let now = clock.now_ms();
        lp.fire(core, clock, now).unwrap();
    }
    clock.set(clock.now_ms().max(until));
}

/// Emulated ms (paid and owed) per wall ms over `ms` of wall time.
fn rate(lp: &mut Loop, core: &mut Fake, clock: &Manual, ms: f64) -> (f64, f64) {
    let (e0, t0) = (core.now + lp.owed_ms(clock.now_ms()), clock.now_ms());
    run_until(lp, core, clock, t0 + ms);
    let (e1, t1) = (core.now + lp.owed_ms(clock.now_ms()), clock.now_ms());
    (e1 - e0, t1 - t0)
}

fn started(pacing: Pacing) -> (Loop, Fake, Manual) {
    let clock = Manual::default();
    let core = Fake::new(&clock);
    let mut lp = Loop::new(pacing);
    lp.running = true;
    lp.start(&core, 0.0);
    (lp, core, clock)
}

#[test]
fn idle_keeps_real_time_at_every_speed() {
    for pacing in [Pacing::WORKER, Pacing::NATIVE] {
        let (mut lp, mut core, clock) = started(pacing);
        run_until(&mut lp, &mut core, &clock, 50.0);
        for speed in [Speed::One, Speed::Four, Speed::Max] {
            lp.speed = speed;
            let (emulated, wall) = rate(&mut lp, &mut core, &clock, 10_000.0);
            // The time a blink computes runs at the speed.
            let tolerance = if speed == Speed::One {
                1.0
            } else {
                wall * 0.01
            };
            assert!(
                (emulated - wall).abs() < tolerance,
                "idle at {speed:?}: {emulated} over {wall}"
            );
            assert_eq!(lp.state(), LoopState::Sleep);
        }
        // Two wakes a second (the 500 ms ticks), no passes while idle.
        let ticks = lp.ticks;
        let wakes = lp.wakes;
        rate(&mut lp, &mut core, &clock, 10_000.0);
        assert_eq!(lp.ticks, ticks, "{pacing:?}");
        assert!(
            (19..=21).contains(&(lp.wakes - wakes)),
            "{}",
            lp.wakes - wakes
        );
    }
}

#[test]
fn scheduling_again_while_asleep_keeps_the_time_slept() {
    let (mut lp, mut core, clock) = started(Pacing::NATIVE);
    run_until(&mut lp, &mut core, &clock, 50.0);
    assert_eq!(lp.state(), LoopState::Sleep);
    let (e0, t0) = (core.now + lp.owed_ms(clock.now_ms()), clock.now_ms());
    // A poke reschedules the sleeping loop in the middle of a sleep.
    clock.advance(300.0);
    lp.schedule(&core, clock.now_ms());
    run_until(&mut lp, &mut core, &clock, t0 + 2_000.0);
    let (e1, t1) = (core.now + lp.owed_ms(clock.now_ms()), clock.now_ms());
    assert!(
        ((e1 - e0) - (t1 - t0)).abs() < 1.0,
        "{} over {}",
        e1 - e0,
        t1 - t0
    );
}

#[test]
fn computing_runs_at_the_speed() {
    for pacing in [Pacing::WORKER, Pacing::NATIVE] {
        for (speed, want) in [(Speed::One, 1.0), (Speed::Two, 2.0), (Speed::Four, 4.0)] {
            let (mut lp, mut core, clock) = started(pacing);
            lp.speed = speed;
            core.busy_until = f64::INFINITY;
            core.cost = 0.01;
            rate(&mut lp, &mut core, &clock, 100.0);
            let (emulated, wall) = rate(&mut lp, &mut core, &clock, 2000.0);
            let r = emulated / wall;
            assert!((r - want).abs() < want * 0.05, "{speed:?}: rate {r}");
            assert_eq!(lp.state(), LoopState::Frame);
        }
    }
}

#[test]
fn max_runs_as_fast_as_the_budget_allows_then_sleeps_at_1x() {
    let (mut lp, mut core, clock) = started(Pacing::WORKER);
    lp.speed = Speed::Max;
    // 1 wall ms per 50 emulated ms: a pass's 11 ms budget runs 550.
    core.cost = 0.02;
    core.busy_until = f64::INFINITY;
    let (emulated, wall) = rate(&mut lp, &mut core, &clock, 1000.0);
    assert!(emulated / wall > 20.0, "{emulated} over {wall}");
    // 100 emulated ms of work, then a second of sleep: the clock gains at
    // most the work, and the sleep is at 1x (where the passes fall
    // decides how much of the work shows).
    core.busy_until = core.now + 100.0;
    let (emulated, wall) = rate(&mut lp, &mut core, &clock, 1000.0);
    let drift = emulated - wall;
    assert!(drift > 0.0 && drift < 300.0, "{emulated} over {wall}");
    assert_eq!(lp.state(), LoopState::Sleep);
}

#[test]
fn resuming_while_asleep_at_max_gains_nothing() {
    let (mut lp, mut core, clock) = started(Pacing::WORKER);
    lp.speed = Speed::Max;
    run_until(&mut lp, &mut core, &clock, 100.0);
    let (e0, t0) = (core.now, clock.now_ms());
    for _ in 0..5 {
        lp.stop();
        lp.start(&core, clock.now_ms());
        run_until(&mut lp, &mut core, &clock, clock.now_ms() + 20.0);
    }
    let gained = (core.now + lp.owed_ms(clock.now_ms()) - e0) - (clock.now_ms() - t0);
    assert!(gained <= 20.0, "gained {gained} ms");
}

#[test]
fn a_wake_catches_up_at_most_twelve_hours_and_owes_what_does_not_fit() {
    let (mut lp, mut core, clock) = started(Pacing::WORKER);
    run_until(&mut lp, &mut core, &clock, 10.0);
    assert!(lp.sleeping());
    // A day passes before the timer fires (a laptop asleep).
    let e0 = core.now;
    clock.set(clock.now_ms() + 24.0 * 3600.0 * 1000.0);
    let now = clock.now_ms();
    lp.wake(&mut core, &clock).unwrap();
    assert!(
        (core.now - e0 - MAX_BEHIND_MS).abs() < 1000.0,
        "{}",
        core.now - e0
    );
    // The ROM wakes and computes 200 ms at 1 wall ms per 2 emulated ms:
    // the wake's 22 ms budget pays part, the rest is owed and paid first.
    lp.stop();
    lp.start(&core, now);
    run_until(&mut lp, &mut core, &clock, now + 600.0);
    core.cost = 0.5;
    core.busy_until = core.now + 200.0;
    lp.stop();
    lp.start(&core, clock.now_ms());
    let t = clock.now_ms();
    lp.fire(&mut core, &clock, t).unwrap();
    assert!(lp.owed_ms(clock.now_ms()) == 0.0 || lp.passing());
    let (emulated, wall) = rate(&mut lp, &mut core, &clock, 3000.0);
    assert!((emulated - wall).abs() < 30.0, "{emulated} over {wall}");
}

#[test]
fn owed_time_from_a_late_wake_is_paid_by_the_passes() {
    let (mut lp, mut core, clock) = started(Pacing::WORKER);
    run_until(&mut lp, &mut core, &clock, 10.0);
    // The timer comes 2 s late, and the ROM computes through it at 1 wall
    // ms per emulated ms: a 22 ms wake cannot pay 2 s.
    core.cost = 1.0;
    core.busy_until = core.now + 5000.0;
    clock.advance(2000.0);
    lp.wake(&mut core, &clock).unwrap();
    assert!(lp.owed_ms(clock.now_ms()) > 1000.0);
    assert!(lp.passing(), "passes pay it off");
    core.cost = 0.001;
    run_until(&mut lp, &mut core, &clock, clock.now_ms() + 2000.0);
    assert_eq!(lp.owed_ms(clock.now_ms()), 0.0);
}

#[test]
fn hidden_stops_the_passes_of_a_computing_worker_only() {
    let (mut lp, mut core, clock) = started(Pacing::WORKER);
    core.busy_until = f64::INFINITY;
    run_until(&mut lp, &mut core, &clock, 100.0);
    lp.set_hidden(true, Some(&core), true, clock.now_ms());
    run_until(&mut lp, &mut core, &clock, 200.0);
    assert_eq!(lp.state(), LoopState::Stopped, "no passes while hidden");
    let e = core.now;
    clock.advance(1000.0);
    assert_eq!(core.now, e);
    lp.set_hidden(false, Some(&core), true, clock.now_ms());
    assert_eq!(lp.state(), LoopState::Frame);
    // The native pacing ignores the page's visibility.
    let (mut lp, mut core, clock) = started(Pacing::NATIVE);
    core.busy_until = f64::INFINITY;
    lp.set_hidden(true, Some(&core), true, 0.0);
    let (emulated, wall) = rate(&mut lp, &mut core, &clock, 500.0);
    assert!((emulated / wall - 1.0).abs() < 0.05);
}

#[test]
fn a_pass_due_slightly_early_counts_on_a_browser_timer_only() {
    let (mut lp, mut core, clock) = started(Pacing::WORKER);
    core.busy_until = f64::INFINITY;
    run_until(&mut lp, &mut core, &clock, 100.0);
    let due = lp.due().unwrap();
    let ticks = lp.ticks;
    // setTimeout(f, 16.67) fires after 16 ms.
    clock.set(due - 0.67);
    lp.fire(&mut core, &clock, due - 0.67).unwrap();
    assert_eq!(lp.ticks, ticks + 1);
    let (mut lp, mut core, clock) = started(Pacing::NATIVE);
    core.busy_until = f64::INFINITY;
    run_until(&mut lp, &mut core, &clock, 100.0);
    let due = lp.due().unwrap();
    let ticks = lp.ticks;
    lp.fire(&mut core, &clock, due - 0.5).unwrap();
    assert_eq!(lp.ticks, ticks);
}

#[test]
fn the_memory_watch_looks_only_when_due_and_quiet() {
    let clock = Manual::default();
    let mut core = Fake::new(&clock);
    let mut w = Watch::default();
    w.subscribe(true, Some(&core));
    // No cycles, no look.
    assert!(!w.poll(Some(&core), true, &clock));
    assert_eq!(w.looks, 0);
    core.now = 10.0;
    core.memory = 1;
    // Computing: no look.
    assert!(!w.poll(Some(&core), false, &clock));
    assert!(w.poll(Some(&core), true, &clock), "changed");
    assert_eq!(w.looks, 1);
    // The next change within 250 ms of the event waits for a timer.
    core.now = 20.0;
    core.memory = 2;
    clock.set(50.0);
    assert!(!w.poll(Some(&core), true, &clock));
    assert_eq!(w.due, Some(250.0));
    assert!(!w.poll(Some(&core), true, &clock), "waits for its timer");
    clock.set(250.0);
    w.due = None;
    assert!(w.poll(Some(&core), true, &clock));
    // Ran without a change: a look, no event; then 100 ms apart.
    core.now = 30.0;
    clock.set(600.0);
    assert!(!w.poll(Some(&core), true, &clock));
    assert_eq!(w.looks, 3);
    core.now = 40.0;
    clock.set(650.0);
    assert!(!w.poll(Some(&core), true, &clock));
    assert_eq!(w.due, Some(700.0));
    // Forced (a loaded state) without cycles.
    w.due = None;
    clock.set(800.0);
    w.force = true;
    core.memory = 3;
    assert!(w.poll(Some(&core), true, &clock));
    // Not watching: nothing.
    w.subscribe(false, Some(&core));
    core.now = 50.0;
    core.memory = 4;
    clock.set(2000.0);
    assert!(!w.poll(Some(&core), true, &clock));
}

// ---- The engine on a 48SX with a ROM of zeros ----

const ZEROS: usize = 256 * 1024;

struct Host {
    engine: Engine,
    clock: Ticking,
    tag: u64,
}

impl Host {
    fn new(pacing: Pacing) -> Host {
        Host {
            engine: Engine::new("test", pacing),
            clock: Ticking(Cell::new(0.0), 5.0),
            tag: 0,
        }
    }

    fn booted() -> Host {
        let mut h = Host::new(Pacing::WORKER);
        let r = h.call(
            json!({"cmd": "boot", "model": "48sx", "romName": "zeros"}),
            Some(vec![0; ZEROS]),
        );
        assert_eq!(r.0.unwrap(), json!({"model": "48sx", "romName": "zeros"}));
        h
    }

    /// Send `msg` with a tag; its reply (if it came) and the events
    /// before it.
    fn call(
        &mut self,
        mut msg: Value,
        bytes: Option<Vec<u8>>,
    ) -> (std::result::Result<Value, String>, Vec<Event>) {
        msg["v"] = json!(1);
        self.tag += 1;
        let tag = self.tag;
        self.engine.command(&self.clock, &msg, bytes, Some(tag));
        let mut events = Vec::new();
        for out in self.engine.take_output() {
            match out {
                Output::Event(e) => events.push(e),
                Output::Reply(r) if r.tag == tag => {
                    let mut result = r.result;
                    if let (Ok(v), Some((field, b))) = (&mut result, r.bytes) {
                        v[field] = json!(b.len());
                    }
                    return (result, events);
                }
                Output::Reply(r) => panic!("a reply for {}: {:?}", r.tag, r.result),
                Output::Save(s) => panic!("a save of the {}", s.model),
            }
        }
        (Err("no reply yet".into()), events)
    }

    fn ok(&mut self, msg: Value) -> Value {
        let (r, _) = self.call(msg.clone(), None);
        r.unwrap_or_else(|e| panic!("{msg}: {e}"))
    }

    fn err(&mut self, msg: Value) -> String {
        let (r, _) = self.call(msg.clone(), None);
        r.expect_err(&msg.to_string())
    }

    /// Fire the timers for `turns` turns; the outputs.
    fn turns(&mut self, turns: usize) -> Vec<Output> {
        for _ in 0..turns {
            if let Some(due) = self.engine.deadline() {
                self.clock.set(due);
                self.engine.timer(&self.clock);
            }
        }
        self.engine.take_output()
    }
}

fn types(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            Event::Frame(_) => "frame",
            Event::Keys(_) => "keys",
            Event::Status(_) => "status",
            Event::Error(_) => "error",
            Event::MemoryChanged(_) => "memoryChanged",
        })
        .collect()
}

fn status(events: &[Event]) -> Option<&Status> {
    events.iter().rev().find_map(|e| match e {
        Event::Status(s) => Some(s),
        _ => None,
    })
}

#[test]
fn commands_before_a_boot() {
    let mut h = Host::new(Pacing::NATIVE);
    let hello = h.ok(json!({"cmd": "hello"}));
    assert_eq!(hello["protocol"], 1);
    assert_eq!(hello["host"], "test");
    assert_eq!(hello["models"][0], "48sx");
    assert_eq!(
        hello["version"],
        env!("CARGO_PKG_VERSION"),
        "the release, for About"
    );
    assert!(h.ok(json!({"cmd": "skin", "model": "49g"}))["keys"].is_array());
    assert!(h.ok(json!({"cmd": "layout", "model": "48sx"}))["keys"].is_array());
    assert!(
        h.err(json!({"cmd": "skin", "model": "hp99"}))
            .contains("hp99")
    );
    for msg in [
        json!({"cmd": "keyDown", "key": "on"}),
        json!({"cmd": "keyUp", "key": "on"}),
        json!({"cmd": "typeKeys", "keys": ["on"]}),
        json!({"cmd": "reset"}),
        json!({"cmd": "saveState"}),
        json!({"cmd": "stack"}),
        json!({"cmd": "stackTop"}),
        json!({"cmd": "commandLine"}),
        json!({"cmd": "insert", "text": "1"}),
    ] {
        assert_eq!(h.err(msg), "no ROM loaded");
    }
    assert_eq!(h.ok(json!({"cmd": "typeLetter", "letter": "A"})), false);
    assert_eq!(
        h.ok(json!({"cmd": "watchMemory", "on": true})),
        json!({"supported": null, "reason": null})
    );
    assert_eq!(h.ok(json!({"cmd": "keyUpAll"})), Value::Null);
    assert_eq!(h.ok(json!({"cmd": "releaseAll"})), Value::Null);
    assert_eq!(h.err(json!({"cmd": "eval"})), "unknown command \"eval\"");
    assert_eq!(h.err(json!({"x": 1})), "missing string field \"cmd\"");
    let mut msg = json!({"v": 2, "cmd": "hello"});
    h.engine.command(&h.clock, &msg, None, Some(99));
    let Some(Output::Reply(r)) = h.engine.take_output().pop() else {
        panic!("no reply");
    };
    assert_eq!(
        r.result.unwrap_err(),
        "protocol version 2 not supported (this host speaks 1)"
    );
    // Without a tag an error is an event.
    msg["v"] = json!(1);
    msg["cmd"] = json!("keyDown");
    h.engine.command(&h.clock, &msg, None, None);
    assert_eq!(
        h.engine.take_output(),
        vec![Output::Event(Event::Error(ErrorEvent {
            message: "missing string field \"key\"".into()
        }))]
    );
    assert_eq!(h.engine.deadline(), None, "nothing runs");
}

#[test]
fn a_boot_sends_its_events_before_its_reply() {
    let mut h = Host::new(Pacing::WORKER);
    let (r, events) = h.call(
        json!({"cmd": "boot", "model": "49g", "romName": "z"}),
        Some(vec![0; ZEROS]),
    );
    // A 256 KiB ROM fits the 48SX only.
    assert_eq!(r.unwrap(), json!({"model": "48sx", "romName": "z"}));
    assert_eq!(types(&events), ["keys", "frame", "status"]);
    let s = status(&events).unwrap();
    assert_eq!(
        (s.model, s.running, s.loop_state),
        (Some("48sx"), true, LoopState::Frame)
    );
    // A ROM that fits nothing keeps the machine that runs.
    let e = h.err(json!({"cmd": "boot", "model": "48sx"}));
    assert_eq!(e, "missing bytes field \"rom\"");
    let (r, _) = h.call(json!({"cmd": "boot", "model": "48sx"}), Some(vec![0; 1000]));
    assert!(r.unwrap_err().contains("1000"));
    assert_eq!(h.engine.status().rom_name, "z");
    // hello sends the state again.
    let (_, events) = h.call(json!({"cmd": "hello"}), None);
    assert_eq!(types(&events), ["keys", "frame", "status"]);
}

#[test]
fn keys_speed_pause_reset_and_states() {
    let mut h = Host::booted();
    for msg in [
        json!({"cmd": "keyDown", "key": "bogus"}),
        json!({"cmd": "keyDown", "key": "apps"}),
        json!({"cmd": "keyUp", "key": "bogus"}),
        json!({"cmd": "typeKeys", "keys": ["1", "bogus"]}),
        json!({"cmd": "typeKeys", "keys": ["1", 2]}),
        json!({"cmd": "typeKeys"}),
        json!({"cmd": "keyDown"}),
        json!({"cmd": "keyDown", "key": "sqrt", "shift": "bogus"}),
        json!({"cmd": "keyDown", "key": "sqrt", "shift": "alpha"}),
        json!({"cmd": "keyDown", "key": "bogus", "shift": "leftshift"}),
        json!({"cmd": "setSpeed"}),
    ] {
        h.err(msg);
    }
    let (r, _) = h.call(
        json!({"cmd": "keyDown", "key": "sqrt", "shift": "alpha"}),
        None,
    );
    assert!(r.unwrap_err().contains("not a shift key"));
    assert!(
        !h.engine.emulator().unwrap().keys_busy(),
        "nothing was queued"
    );
    let (_, events) = h.call(json!({"cmd": "keyDown", "key": "on"}), None);
    assert_eq!(types(&events), ["keys"]);
    h.ok(json!({"cmd": "keyDown", "key": "sqrt", "shift": "leftshift"}));
    h.ok(json!({"cmd": "keyUp", "key": "sqrt"}));
    let (_, events) = h.call(json!({"cmd": "releaseAll"}), None);
    assert_eq!(types(&events), ["keys"]);
    h.ok(json!({"cmd": "typeKeys", "keys": ["1", "enter"]}));
    assert_eq!(h.ok(json!({"cmd": "typeLetter", "letter": "q"})), true);
    assert_eq!(h.ok(json!({"cmd": "typeLetter", "letter": ""})), false);
    h.ok(json!({"cmd": "keyUpAll"}));
    let (_, events) = h.call(json!({"cmd": "setSpeed", "speed": "max"}), None);
    assert_eq!(status(&events).unwrap().speed, Speed::Max);
    assert_eq!(h.engine.status().speed.name(), "max");
    h.ok(json!({"cmd": "setSpeed", "speed": "7"}));
    assert_eq!(h.engine.status().speed, Speed::One);
    let (_, events) = h.call(json!({"cmd": "pause", "paused": true}), None);
    let s = status(&events).unwrap();
    assert_eq!((s.running, s.loop_state), (false, LoopState::Stopped));
    assert_eq!(h.engine.deadline(), None);
    h.ok(json!({"cmd": "pause", "paused": false}));
    assert!(h.engine.status().running);
    h.ok(json!({"cmd": "pause"}));
    assert!(
        !h.engine.status().running,
        "anything but paused: false pauses"
    );
    h.ok(json!({"cmd": "reset"}));
    assert!(h.engine.status().running, "a reset runs");
    // A state, as bytes and as base64.
    let saved = h.engine.save_state().unwrap();
    let r = h.ok(json!({"cmd": "saveState"}));
    assert_eq!(r["state"], saved.len());
    assert!(r["cycles"].is_u64());
    let (r, events) = h.call(json!({"cmd": "loadState"}), Some(saved.clone()));
    assert_eq!(r.unwrap(), json!({}));
    assert_eq!(types(&events), ["keys", "frame"], "the display again");
    let b64 = crate::host::base64(&saved);
    assert_eq!(
        h.ok(json!({"cmd": "loadState", "state": b64.clone()})),
        json!({})
    );
    // Presses queued and held before a load do not play into it.
    h.ok(json!({"cmd": "keyDown", "key": "1"}));
    h.ok(json!({"cmd": "keyDown", "key": "sqrt", "shift": "leftshift"}));
    h.ok(json!({"cmd": "typeKeys", "keys": ["2", "enter"]}));
    assert!(h.engine.emulator().unwrap().keys_busy());
    assert_eq!(h.ok(json!({"cmd": "loadState", "state": b64})), json!({}));
    assert!(
        !h.engine.emulator().unwrap().keys_busy(),
        "nothing queued or held"
    );
    h.ok(json!({"cmd": "keyUp", "key": "1"}));
    assert!(
        !h.err(json!({"cmd": "loadState", "state": "AAAA"}))
            .is_empty()
    );
    assert!(
        h.err(json!({"cmd": "loadState", "state": "*"}))
            .contains("base64")
    );
    let big = vec![0u8; MAX_STATE_BYTES + 1];
    let (r, _) = h.call(json!({"cmd": "loadState"}), Some(big));
    assert!(r.unwrap_err().contains("larger than"));
}

#[test]
fn reads_and_stats() {
    let mut h = Host::booted();
    assert_eq!(
        h.ok(json!({"cmd": "watchMemory", "on": true})),
        json!({"supported": true, "reason": null})
    );
    assert!(h.err(json!({"cmd": "memoryTree"})).contains("HOME"));
    assert!(h.ok(json!({"cmd": "flags"}))["set"].is_array());
    assert_eq!(
        h.err(json!({"cmd": "objectAt", "address": 0x10_0000})),
        "address is outside the address space"
    );
    assert_eq!(
        h.err(json!({"cmd": "objectAt"})),
        "missing number field \"address\""
    );
    assert_eq!(h.ok(json!({"cmd": "commandLine"}))["active"], false);
    h.turns(5);
    let stats = h.ok(json!({"cmd": "stats"}));
    let keys: Vec<&str> = stats
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        [
            "cycles",
            "emulatedMs",
            "loop",
            "memoryLooks",
            "memoryMs",
            "nowMs",
            "owedMs",
            "ticks",
            "wakes",
            "workMs"
        ]
    );
    assert!(stats["ticks"].as_u64().unwrap() >= 4);
    assert!(stats["cycles"].as_u64().unwrap() > 0);
    assert_eq!(stats["loop"], "frame");
    // A computing machine is not looked at; paused, it is.
    assert_eq!(stats["memoryLooks"], 0);
    h.ok(json!({"cmd": "pause", "paused": true}));
    assert_eq!(h.ok(json!({"cmd": "stats"}))["memoryLooks"], 1);
}

#[test]
fn visibility_stops_a_computing_worker_and_not_a_native_one() {
    let mut h = Host::booted();
    h.turns(2);
    let (_, events) = h.call(json!({"cmd": "visibility", "hidden": true}), None);
    assert!(events.is_empty(), "the pass timer runs out first");
    h.turns(3);
    assert_eq!(h.engine.status().loop_state, LoopState::Stopped);
    assert_eq!(h.engine.deadline(), None);
    h.ok(json!({"cmd": "visibility", "hidden": false}));
    assert_eq!(h.engine.status().loop_state, LoopState::Frame);
    let mut n = Host::new(Pacing::NATIVE);
    n.call(
        json!({"cmd": "boot", "model": "48sx"}),
        Some(vec![0; ZEROS]),
    )
    .0
    .unwrap();
    n.ok(json!({"cmd": "visibility", "hidden": true}));
    n.turns(5);
    assert_eq!(n.engine.status().loop_state, LoopState::Frame);
}

/// A send of more than 12 characters: `busy`, the refusals, no key, error
/// or frame event until it ends, `releaseAll` stops it (its reply first).
#[test]
fn a_long_send_refuses_commands_and_holds_the_screen() {
    let mut h = Host::booted();
    assert!(h.err(json!({"cmd": "insert"})).contains("\"text\""));
    let tag = h.tag + 1;
    let (r, events) = h.call(json!({"cmd": "insert", "text": "« 1 2 + » EVAL"}), None);
    assert_eq!(r.unwrap_err(), "no reply yet");
    let s = status(&events).unwrap();
    assert_eq!((s.busy, s.loop_state), (true, LoopState::Stopped));
    assert_eq!(h.engine.sending(), Some(tag));
    assert!(h.engine.deadline().is_some(), "its turns");
    for cmd in REFUSED_WHILE_TYPING {
        let e = h.err(json!({"cmd": cmd, "key": "on", "text": "1", "model": "48sx", "keys": [], "letter": "A", "address": 0}));
        assert_eq!(e, "typing is in progress (releaseAll stops it)", "{cmd}");
    }
    assert!(h.engine.load_state(&h.clock, &[]).is_err());
    // The others are served, and nothing they show gets out.
    let (r, events) = h.call(json!({"cmd": "hello"}), None);
    r.unwrap();
    assert_eq!(types(&events), ["status"], "no frame or keys while frozen");
    for msg in [
        json!({"cmd": "keyUpAll"}),
        json!({"cmd": "commandLine"}),
        json!({"cmd": "stats"}),
        json!({"cmd": "setSpeed", "speed": "2"}),
        json!({"cmd": "watchMemory", "on": true}),
    ] {
        h.ok(msg);
    }
    let outs = h.turns(3);
    assert!(
        outs.iter().all(|o| !matches!(
            o,
            Output::Event(
                Event::Frame(_) | Event::Keys(_) | Event::Error(_) | Event::MemoryChanged(_)
            )
        )),
        "{outs:?}"
    );
    assert!(
        h.engine.sending().is_some(),
        "a zero ROM never lets it finish"
    );
    // releaseAll: the send's reply (an error) comes before releaseAll's,
    // and the status says the screen is live before its frame.
    h.tag += 1;
    let mine = h.tag;
    h.engine.command(
        &h.clock,
        &json!({"v": 1, "cmd": "releaseAll"}),
        None,
        Some(mine),
    );
    let outs = h.engine.take_output();
    let order: Vec<String> = outs
        .iter()
        .map(|o| match o {
            Output::Reply(r) => format!("reply {} {:?}", r.tag, r.result),
            Output::Event(e) => types(std::slice::from_ref(e))[0].to_string(),
            Output::Save(_) => "save".to_string(),
        })
        .collect();
    let send_reply = format!("reply {tag} Err(\"cancelled\")");
    let at = |s: &str| {
        order
            .iter()
            .position(|o| o == s)
            .unwrap_or_else(|| panic!("{s} in {order:?}"))
    };
    assert!(
        at(&send_reply) < at(&format!("reply {mine} Ok(Null)")),
        "{order:?}"
    );
    assert!(at("status") < at("frame"), "{order:?}");
    assert_eq!(h.engine.sending(), None);
    assert!(h.engine.status().running);
    assert_eq!(h.engine.status().loop_state, LoopState::Frame);
    // The keys work again.
    h.ok(json!({"cmd": "keyDown", "key": "1"}));
}

/// A short send shows as it is typed; it ends with an error when it runs
/// out of wall time.
#[test]
fn a_short_send_runs_in_turns_and_has_a_wall_limit() {
    let mut h = Host::booted();
    let tag = h.tag + 1;
    let (_, events) = h.call(json!({"cmd": "run", "text": "1"}), None);
    assert!(!status(&events).unwrap().busy);
    // 30 ms per clock read: one 20 ms step per turn, so 30 s of wall time
    // pass long before the 30 s of emulated time a key may take.
    h.clock.1 = 30.0;
    let mut reply = None;
    for _ in 0..20_000 {
        for out in h.turns(1) {
            if let Output::Reply(r) = out {
                assert_eq!(r.tag, tag);
                reply = Some(r.result);
            }
        }
        if reply.is_some() {
            break;
        }
    }
    let e = reply.expect("a reply").unwrap_err();
    assert_eq!(e, "typing ran out of wall-clock time (30 s)");
    assert_eq!(h.engine.sending(), None);
}

#[test]
fn native_extras_poke_and_exclusive_runs() {
    let mut h = Host::booted();
    // No RAM is configured on a ROM of zeros; the write goes nowhere,
    // and the display is sent again.
    h.engine.poke(&h.clock, 0x70000, &[1, 2, 3]).unwrap();
    h.engine.timer(&h.clock);
    assert!(
        h.engine
            .take_output()
            .iter()
            .any(|o| matches!(o, Output::Event(Event::Frame(_))))
    );
    let r: Result<u64> = h.engine.exclusive(&h.clock, |mut m| {
        m.run_cycles(1000).ok();
        let c = m.cycles();
        (m, Ok(c))
    });
    assert!(r.unwrap() > 0);
    let r: Result<()> = h.engine.exclusive(&h.clock, |m| {
        (
            m,
            Err(RunError {
                message: "CPU halted: test".into(),
                halted: true,
            }),
        )
    });
    assert_eq!(r.unwrap_err().to_string(), "CPU halted: test");
    let s = h.engine.status();
    assert_eq!(
        (s.running, s.halted.as_deref()),
        (false, Some("CPU halted: test"))
    );
    assert!(
        h.err(json!({"cmd": "insert", "text": "1"}))
            .contains("halted")
    );
    // A reset runs again.
    h.ok(json!({"cmd": "reset"}));
    assert!(h.engine.status().running);
    // The model is the ROM's.
    assert_eq!(h.engine.emulator().unwrap().model(), Model::Hp48sx);
}

/// The writes check their fields and the calculator before anything runs:
/// with no ROM, with memory the ROM has not set up, with bad fields, no
/// transfer starts and nothing is held.
#[test]
fn writes_are_refused_before_anything_runs() {
    let mut h = Host::new(Pacing::WORKER);
    for cmd in WRITE_COMMANDS {
        let to = if cmd == "copy" || cmd == "move" {
            json!(["HOME", "B"])
        } else {
            json!("B")
        };
        let e = h.err(json!({"cmd": cmd, "name": "A", "to": to, "flag": 1, "on": true, "data": "", "text": "1", "dir": ["HOME"]}));
        assert_eq!(e, "no ROM loaded", "{cmd}");
    }
    let mut h = Host::booted();
    // The ROM of zeros never sets up a user memory.
    let e = h.err(json!({"cmd": "setFlag", "flag": 5, "on": true}));
    assert!(e.contains("user memory") || e.contains("not set up"), "{e}");
    assert!(
        h.err(json!({"cmd": "setFlag", "flag": 5}))
            .contains("\"on\"")
    );
    assert!(
        h.err(json!({"cmd": "setFlag", "on": true}))
            .contains("\"flag\"")
    );
    assert!(
        h.err(json!({"cmd": "purge", "dir": "HOME", "name": "A"}))
            .contains("\"dir\"")
    );
    assert!(
        h.err(json!({"cmd": "purge", "dir": ["HOME"]}))
            .contains("\"name\"")
    );
    assert!(
        h.err(json!({"cmd": "createDir", "dir": ["HOME"]}))
            .contains("\"name\"")
    );
    assert!(
        h.err(json!({"cmd": "createDir", "dir": "HOME", "name": "A"}))
            .contains("\"dir\"")
    );
    assert!(
        h.err(json!({"cmd": "storeFile", "dir": ["HOME"], "name": "A"}))
            .contains("\"data\"")
    );
    assert!(
        h.err(json!({"cmd": "move", "dir": ["HOME"], "name": "A", "to": "HOME/B"}))
            .contains("\"to\"")
    );
    // `storeText` and `editText` name a variable or a level, not both.
    for cmd in ["storeText", "editText"] {
        let msg = |extra: Value| {
            let mut m = json!({"cmd": cmd, "text": "1", "dir": ["HOME"]});
            for (k, v) in extra.as_object().unwrap() {
                m[k] = v.clone();
            }
            m
        };
        assert!(
            h.err(msg(json!({})))
                .contains("either \"name\" or \"level\"")
        );
        assert!(
            h.err(msg(json!({"name": "A", "level": 1})))
                .contains("either")
        );
        assert!(h.err(msg(json!({"level": 0}))).contains("\"level\""));
        assert!(h.err(msg(json!({"level": "1"}))).contains("\"level\""));
    }
    assert!(
        h.err(json!({"cmd": "storeText", "name": "A"}))
            .contains("\"text\"")
    );
    let big = "A".repeat(crate::transfer::MAX_FILE_BYTES / 3 * 4 + 8);
    assert!(
        h.err(json!({"cmd": "storeFile", "dir": ["HOME"], "name": "A", "data": big}))
            .contains("larger than")
    );
    assert_eq!(h.engine.sending(), None);
    assert!(!h.engine.status().busy);
}

// ---- Auto-save on a 48SX whose ROM only sleeps ----

/// A 48SX ROM that sleeps: `SHUTDN` and a `GOTO` back to it, at the reset
/// address and at the interrupt vector (#0000F), zeros elsewhere.
fn sleeper() -> Vec<u8> {
    let mut nibbles = vec![0u8; ZEROS * 2];
    for at in [0x0, 0xF] {
        // SHUTDN (807), GOTO -4 from its offset field (6 CFF).
        nibbles[at..at + 7].copy_from_slice(&[8, 0, 7, 6, 0xC, 0xF, 0xF]);
    }
    nibbles.chunks(2).map(|n| n[0] | (n[1] << 4)).collect()
}

/// The engine with auto-save on, the sleeper booted, on a clock moved by
/// hand.
struct Saving {
    engine: Engine,
    clock: Manual,
    saves: Vec<Saved>,
}

impl Saving {
    fn new(rom: &[u8]) -> Saving {
        let mut engine = Engine::new("test", Pacing::WORKER);
        engine.set_auto_save(true);
        let clock = Manual::default();
        engine.boot(&clock, "48sx", rom, "rom").unwrap();
        let mut s = Saving {
            engine,
            clock,
            saves: Vec::new(),
        };
        s.take();
        s
    }

    fn take(&mut self) {
        for out in self.engine.take_output() {
            if let Output::Save(saved) = out {
                self.saves.push(saved);
            }
        }
    }

    fn send(&mut self, msg: Value) {
        let mut msg = msg;
        msg["v"] = json!(1);
        self.engine.command(&self.clock, &msg, None, None);
        self.take();
    }

    /// Fire the timers as a host would until wall time `until`.
    fn run_until(&mut self, until: f64) {
        while let Some(due) = self.engine.deadline() {
            if due > until {
                break;
            }
            self.clock.set(self.clock.now_ms().max(due));
            self.engine.timer(&self.clock);
            self.take();
        }
        self.clock.set(self.clock.now_ms().max(until));
    }

    fn press(&mut self, key: &str) {
        self.send(json!({"cmd": "keyDown", "key": key}));
        self.run_until(self.clock.now_ms() + 50.0);
        self.send(json!({"cmd": "keyUp", "key": key}));
    }
}

#[test]
fn the_sleeper_sleeps() {
    let mut s = Saving::new(&sleeper());
    s.run_until(1_000.0);
    assert_eq!(s.engine.status().loop_state, LoopState::Sleep);
    assert!(s.engine.settled());
}

#[test]
fn an_idle_calculator_is_never_saved() {
    let mut s = Saving::new(&sleeper());
    s.run_until(600_000.0);
    s.send(json!({"cmd": "visibility", "hidden": true}));
    s.run_until(700_000.0);
    s.send(json!({"cmd": "visibility", "hidden": false}));
    s.run_until(800_000.0);
    assert!(s.saves.is_empty(), "{} saves", s.saves.len());
    assert_eq!(s.engine.auto_saves(), 0);
}

#[test]
fn a_key_is_saved_once_after_the_delay() {
    let mut s = Saving::new(&sleeper());
    s.run_until(1_000.0);
    s.press("1");
    let pressed = s.clock.now_ms();
    assert!(s.engine.save_owed());
    s.run_until(pressed + AUTO_SAVE_MS - 100.0);
    assert!(s.saves.is_empty(), "not before the delay");
    s.run_until(pressed + AUTO_SAVE_MS + 100.0);
    assert_eq!(s.saves.len(), 1, "one save after the delay");
    let saved = s.saves[0].clone();
    assert_eq!((saved.model, saved.rom_name.as_str()), ("48sx", "rom"));
    assert!(saved.state.len() > 1000);
    s.run_until(pressed + 120_000.0);
    assert_eq!(s.saves.len(), 1, "nothing changed since");
    // The saved state boots a fresh machine as it was.
    let mut fresh = Engine::new("test", Pacing::WORKER);
    let b = fresh
        .boot_restoring(&s.clock, "48sx", &sleeper(), "rom", Some(&saved.state))
        .unwrap();
    assert!(b.restored, "{b:?}");
    assert_eq!(
        fresh.emulator().unwrap().machine().cycles(),
        saved.cycles,
        "restored before running"
    );
    assert_eq!(fresh.take_output().len(), 3, "keys, frame, status; no save");
}

#[test]
fn keys_in_a_row_are_saved_once() {
    let mut s = Saving::new(&sleeper());
    for _ in 0..10 {
        s.press("1");
        s.run_until(s.clock.now_ms() + 1_000.0);
    }
    assert!(s.saves.is_empty(), "each key moves the save on");
    s.run_until(s.clock.now_ms() + AUTO_SAVE_MS);
    assert_eq!(s.saves.len(), 1);
}

#[test]
fn a_hidden_page_saves_at_once() {
    let mut s = Saving::new(&sleeper());
    s.run_until(1_000.0);
    s.press("1");
    s.run_until(s.clock.now_ms() + 100.0);
    s.send(json!({"cmd": "visibility", "hidden": true}));
    assert_eq!(s.saves.len(), 1, "in the visibility command's own output");
    s.run_until(s.clock.now_ms() + 60_000.0);
    assert_eq!(s.saves.len(), 1);
}

#[test]
fn a_key_down_is_not_saved_until_it_comes_up() {
    let mut s = Saving::new(&sleeper());
    s.run_until(1_000.0);
    s.send(json!({"cmd": "keyDown", "key": "1"}));
    s.send(json!({"cmd": "visibility", "hidden": true}));
    s.run_until(30_000.0);
    assert!(s.saves.is_empty(), "a key is down");
    s.send(json!({"cmd": "keyUp", "key": "1"}));
    s.run_until(60_000.0);
    // In the browser the machine stops while hidden unless it sleeps:
    // the save waits for the page to be shown and the machine to settle,
    // then comes without the delay.
    assert!(s.saves.is_empty(), "stopped while hidden");
    s.send(json!({"cmd": "visibility", "hidden": false}));
    s.run_until(60_100.0);
    assert_eq!(s.saves.len(), 1, "shown and settled: at once");
}

#[test]
fn nothing_is_saved_during_a_send_or_while_computing() {
    // The ROM of zeros computes forever: a change is never saved, hidden
    // or not, during a send or after it.
    let mut s = Saving::new(&vec![0; ZEROS]);
    s.send(json!({"cmd": "keyDown", "key": "1"}));
    s.send(json!({"cmd": "keyUp", "key": "1"}));
    s.run_until(10_000.0);
    s.send(json!({"cmd": "insert", "text": "« 1 2 + » EVAL"}));
    assert!(s.engine.status().busy, "the send runs");
    s.send(json!({"cmd": "visibility", "hidden": true}));
    s.run_until(20_000.0);
    s.send(json!({"cmd": "releaseAll"}));
    s.run_until(40_000.0);
    assert!(s.saves.is_empty());
    assert!(s.engine.save_owed(), "still owed");
}

#[test]
fn a_restore_that_does_not_load_boots_cold() {
    let clock = Manual::default();
    let mut e = Engine::new("test", Pacing::WORKER);
    let b = e
        .boot_restoring(&clock, "48sx", &sleeper(), "rom", Some(b"not a state"))
        .unwrap();
    assert!(!b.restored);
    assert!(
        b.restore_error.as_deref().unwrap().contains("magic"),
        "{b:?}"
    );
    // A state of another ROM.
    let zeros = Engine::new("test", Pacing::WORKER);
    let mut z = zeros;
    z.boot(&clock, "48sx", &vec![0; ZEROS], "zeros").unwrap();
    let other = z.save_state().unwrap();
    let b = e
        .boot_restoring(&clock, "48sx", &sleeper(), "rom", Some(&other))
        .unwrap();
    assert!(!b.restored);
    assert!(b.restore_error.is_some());
    // Only what changed is sent: a cold boot's reply is as before.
    assert_eq!(
        serde_json::to_value(Booted {
            model: "48sx",
            rom_name: "r".into(),
            restored: false,
            restore_error: None
        })
        .unwrap(),
        json!({"model": "48sx", "romName": "r"})
    );
}

/// A host's slot, filled and read in the order a host does around a
/// boot: `save_now`, store what it gave, then read (or clear, fresh) and
/// boot.
fn reboot(s: &mut Saving, slot: &mut Option<Vec<u8>>, rom: &[u8], fresh: bool) -> Booted {
    s.engine.save_now();
    s.take();
    if let Some(saved) = s.saves.drain(..).next_back() {
        *slot = Some(saved.state);
    }
    if fresh {
        *slot = None;
    }
    let b = s
        .engine
        .boot_restoring(&s.clock, "48sx", rom, "rom", slot.as_deref())
        .unwrap();
    s.take();
    assert!(s.saves.is_empty(), "a boot itself saves nothing");
    b
}

#[test]
fn a_reboot_restores_the_newest_state() {
    let mut s = Saving::new(&sleeper());
    s.run_until(1_000.0);
    s.press("1");
    s.run_until(s.clock.now_ms() + AUTO_SAVE_MS + 100.0);
    let mut slot = s.saves.drain(..).next_back().map(|x| x.state);
    let older = slot.clone().unwrap();
    // A newer change, within the delay: the reboot keeps it first.
    s.press("2");
    s.run_until(s.clock.now_ms() + 100.0);
    let cycles = s.engine.emulator().unwrap().machine().cycles();
    let b = reboot(&mut s, &mut slot, &sleeper(), false);
    assert!(b.restored);
    assert_ne!(
        slot.as_deref(),
        Some(&older[..]),
        "the newest is in the slot"
    );
    assert!(
        s.engine.emulator().unwrap().machine().cycles() >= cycles,
        "restored the newest state"
    );
    assert!(!s.engine.save_owed());
}

#[test]
fn a_fresh_start_never_brings_the_old_machine_back() {
    let mut s = Saving::new(&sleeper());
    let mut slot = None;
    s.run_until(1_000.0);
    s.press("1");
    s.run_until(s.clock.now_ms() + 100.0);
    assert!(s.engine.save_owed());
    let b = reboot(&mut s, &mut slot, &sleeper(), true);
    assert!(!b.restored);
    assert_eq!(slot, None, "the slot stays empty");
    s.run_until(s.clock.now_ms() + 60_000.0);
    assert!(s.saves.is_empty(), "and the old machine's save is not owed");
}

#[test]
fn a_model_switch_keeps_the_machine_it_leaves() {
    let mut s = Saving::new(&sleeper());
    s.run_until(1_000.0);
    s.press("1");
    s.run_until(s.clock.now_ms() + 100.0);
    s.engine.save_now();
    s.take();
    assert_eq!(s.saves.len(), 1);
    assert_eq!(s.saves[0].rom_name, "rom");
    s.engine
        .boot(&s.clock, "48sx", &vec![0; ZEROS], "zeros")
        .unwrap();
    s.take();
    assert_eq!(s.saves.len(), 1, "the boot adds none");
    assert!(!s.engine.save_owed(), "the new machine owes nothing");
    // Nothing owed, nothing to save.
    s.engine.save_now();
    s.take();
    assert_eq!(s.saves.len(), 1);
}

#[test]
fn auto_save_is_off_by_default() {
    let mut h = Host::booted();
    h.ok(json!({"cmd": "keyDown", "key": "1"}));
    assert!(!h.engine.save_owed());
}

/// Whatever replaces, resets, pokes or runs the machine past the pacing
/// ends the pending answer to a cold boot's question, whichever way the
/// host calls it (a command or the engine's method).
#[test]
fn a_machine_changed_ends_the_boots_answer() {
    let mut h = Host::new(Pacing::WORKER);
    h.engine.set_answer_recover(true);
    let boot = |h: &mut Host| {
        h.call(
            json!({"cmd": "boot", "model": "48sx", "romName": "zeros"}),
            Some(vec![0; ZEROS]),
        )
        .0
        .unwrap();
        assert!(h.engine.answering_recover(), "a cold 48SX boot is answered");
    };
    boot(&mut h);
    let state = h.engine.save_state().unwrap();
    h.engine.load_state(&h.clock, &state).unwrap();
    assert!(!h.engine.answering_recover(), "a state loaded");
    boot(&mut h);
    h.engine.poke(&h.clock, 0x80000, &[1]).unwrap();
    assert!(!h.engine.answering_recover(), "a poke");
    boot(&mut h);
    h.engine
        .exclusive(&h.clock, |m| (m, Ok::<(), RunError>(())))
        .unwrap();
    assert!(!h.engine.answering_recover(), "a key script");
    boot(&mut h);
    h.ok(json!({"cmd": "reset"}));
    assert!(!h.engine.answering_recover(), "a reset");
}

/// `unload` (a removed ROM): no machine, as before the first boot; the
/// keys are refused, nothing is saved, and a boot works again.
#[test]
fn unload_leaves_no_machine() {
    let mut h = Host::booted();
    h.engine.set_auto_save(true);
    h.ok(json!({"cmd": "keyDown", "key": "1"}));
    let (r, events) = h.call(json!({"cmd": "unload"}), None);
    assert_eq!(r.unwrap(), Value::Null);
    let st = status(&events).expect("a status");
    assert_eq!(st.model, None);
    assert!(!st.running);
    assert!(h.engine.emulator().is_err());
    assert!(!h.engine.save_owed(), "nothing owed: not written back");
    assert!(
        h.err(json!({"cmd": "keyDown", "key": "1"}))
            .contains("no ROM loaded")
    );
    h.call(
        json!({"cmd": "boot", "model": "48sx", "romName": "zeros"}),
        Some(vec![0; ZEROS]),
    )
    .0
    .unwrap();
    assert_eq!(h.engine.status().model, Some("48sx"));
}
