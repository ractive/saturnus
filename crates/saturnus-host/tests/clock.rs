//! Setting the clock on the real ROMs (each model skipped unless
//! `SATURNUS_ROM_DIR` holds its ROM): a fixed date and time set through
//! the host, then read back the calculator's own way. The 48SX, 48GX and
//! 49G (in RPN and in algebraic mode, and with flag -42's day-first
//! dates) read `TIME` and `DATE` from the stack, which is otherwise as it
//! was; the 38G, 39G, 40G, 27S, 17BII and 17B show the time on their
//! screen, compared with a golden (`tests/golden/clock-*.txt`, written
//! with `SATURNUS_BLESS=1`; each read by eye when written), the 38G's
//! and 39G's HOME history as it was. A calculator with a command line
//! open, or off, is refused.
//!
//! And the clock reads the wall clock once set: on every model set by
//! keys and on the 48GX, the set runs through the protocol's engine on a
//! clock moved by hand at 1x and at Max, and the time the calculator then
//! shows (read off its screen, or its stack) is the time requested plus
//! the wall time since, within a second.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::path::PathBuf;

use saturnus_core::io::Key;
use saturnus_core::{Machine, Model};
use saturnus_host::Emulator;
use saturnus_host::clock::{self, ClockTime, Now, Pace};
use saturnus_host::protocol::{Clock, Engine, LoopState, Output, Pacing};
use saturnus_host::typing::{Job, Verb};
use serde_json::json;

fn rom(file: &str) -> Option<Vec<u8>> {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("skipped: SATURNUS_ROM_DIR is not set");
        return None;
    };
    let r = std::fs::read(PathBuf::from(dir).join(file)).ok();
    if r.is_none() {
        eprintln!("skipped: {file} not in SATURNUS_ROM_DIR");
    }
    r
}

fn per_ms(m: &Machine) -> u64 {
    u64::from(m.model().clock_hz()) / 1000
}

/// The date and time every test sets: 13 October 2026, 14:05:30.
fn when() -> ClockTime {
    ClockTime::new(2026, 10, 13, 14, 5, 30).unwrap()
}

/// Run `ms` emulated ms in the host's slices, the key queue fed.
fn run(e: &mut Emulator, ms: f64) {
    let mut left = ms;
    while left > 0.0 {
        left -= e.run_slice(left, true).unwrap();
    }
}

/// Run until the queue is empty and the CPU has slept `quiet_ms`, at
/// most 60 s.
fn settle(e: &mut Emulator, quiet_ms: f64) {
    let mut quiet = 0.0;
    for _ in 0..6000 {
        run(e, 10.0);
        quiet = if !e.keys_busy() && e.machine().is_shutdown() {
            quiet + 10.0
        } else {
            0.0
        };
        if quiet > quiet_ms {
            return;
        }
    }
}

fn click(e: &mut Emulator, key: &str) {
    e.queue().type_keys(&[key]);
    e.pump();
    settle(e, 800.0);
}

fn stack_texts(e: &Emulator) -> Vec<String> {
    e.stack()
        .unwrap()
        .iter()
        .map(|v| v["text"].as_str().unwrap_or("?").to_string())
        .collect()
}

/// Type `text` and ENTER on a 48 or 49.
fn type_run(e: &mut Emulator, text: &str) {
    let m = e.machine_mut();
    let mut job = Job::new(m, Verb::Run, text).unwrap();
    let step = 50 * per_ms(m);
    while !job.step(m, step).unwrap() {}
    assert_eq!(job.outcome().error, None, "{text}");
}

/// A 48 or 49, cold-booted, past the boot's question (and the 49G's
/// box), at its stack.
fn boot_rpl(model: Model, file: &str) -> Option<Emulator> {
    let mut e = Emulator::new(model, &rom(file)?).unwrap();
    run(&mut e, 5000.0);
    settle(&mut e, 1500.0);
    let n = if model == Model::Hp49g { 2 } else { 1 };
    for _ in 0..n {
        click(&mut e, "f");
    }
    settle(&mut e, 1500.0);
    Some(e)
}

/// A host clock that stands still: the rehearsals take no wall time.
#[derive(Default)]
struct Manual(Cell<f64>);

impl Clock for Manual {
    fn now_ms(&self) -> f64 {
        self.0.get()
    }
}

/// `t` at the start of its second, now on a clock that stands still.
fn now_of(t: ClockTime) -> Now {
    Now {
        time: t,
        ms: 0.0,
        wall_ms: 0.0,
    }
}

/// Set the clock on a 48 or 49 as the protocol does (a write that takes
/// no wall time).
fn set_rpl(e: &mut Emulator, t: ClockTime) {
    assert_eq!(e.clock_refusal(), None);
    let (op, _) = e
        .clock_write(&now_of(t), &Pace::UNBOUNDED, &Manual::default())
        .unwrap();
    e.start_transfer(op).unwrap();
    while !e.transfer_step(20.0).unwrap() {}
    let (r, _) = e.transfer_result().unwrap();
    assert_eq!(r.error, None, "{r:?}");
}

/// H.MMSSss as seconds of the day.
fn seconds_of(hms: f64) -> f64 {
    let h = hms.trunc();
    let mm = ((hms - h) * 100.0).trunc();
    let ss = ((hms - h) * 100.0 - mm) * 100.0;
    h * 3600.0 + mm * 60.0 + ss
}

fn number(text: &str) -> f64 {
    text.replace(',', ".")
        .parse()
        .unwrap_or_else(|_| panic!("{text:?}"))
}

/// The time on the calculator is the time set, give or take the emulated
/// seconds since (typing `TIME` takes some; how close is
/// `the_clock_reads_the_wall_clock_once_set`'s to check).
fn assert_time(model: Model, text: &str) {
    let got = seconds_of(number(text));
    let want = 14.0 * 3600.0 + 5.0 * 60.0 + 30.0;
    assert!(
        (want - 5.0..want + 20.0).contains(&got),
        "{}: TIME is {text}",
        model.name()
    );
}

#[test]
fn the_48_and_49_set_their_clock_by_command() {
    for (model, file) in [
        (Model::Hp48sx, "sxrom-j"),
        (Model::Hp48gx, "gxrom-r"),
        (Model::Hp49g, "rom-2.10.49g"),
    ] {
        let Some(mut e) = boot_rpl(model, file) else {
            continue;
        };
        if model == Model::Hp49g {
            // It boots in algebraic mode: set the clock there first.
            assert_eq!(e.flags().unwrap().get(-95), Some(true));
            let before = stack_texts(&e);
            set_rpl(&mut e, when());
            assert_eq!(
                stack_texts(&e),
                before,
                "algebraic mode's echoes are dropped"
            );
            type_run(&mut e, "DATE");
            assert_eq!(stack_texts(&e)[0], "10.132026");
            type_run(&mut e, "CF(-95)");
            type_run(&mut e, "CLEAR");
        }
        // A stack to keep, then the clock.
        type_run(&mut e, "42 \"x\"");
        let before = stack_texts(&e);
        set_rpl(&mut e, when());
        assert_eq!(stack_texts(&e), before, "{}: the stack", model.name());
        type_run(&mut e, "TIME DATE");
        let s = stack_texts(&e);
        eprintln!("{}: TIME DATE gave {s:?}", model.name());
        assert_eq!(s[0], "10.132026", "{}: DATE", model.name());
        assert_time(model, &s[1]);
        type_run(&mut e, "DROP2");
        // Flag -42: the date is day first.
        type_run(&mut e, "-42 SF");
        set_rpl(&mut e, ClockTime::new(2026, 2, 3, 9, 8, 7).unwrap());
        type_run(&mut e, "DATE");
        assert_eq!(
            stack_texts(&e)[0],
            "3.022026",
            "{}: day first",
            model.name()
        );
        type_run(&mut e, "DROP -42 CF");
        assert_eq!(stack_texts(&e), before);
        // A command line open is refused, nothing typed.
        let m = e.machine_mut();
        let mut job = Job::new(m, Verb::Insert, "12").unwrap();
        let step = 50 * per_ms(m);
        while !job.step(m, step).unwrap() {}
        assert_eq!(e.clock_refusal().as_deref(), Some(clock::LINE_OPEN));
        assert_eq!(e.command_line().unwrap().text, "12");
    }
}

/// The screen as text, compared with (or, with `SATURNUS_BLESS`, written
/// to) `tests/golden/<name>.txt`.
fn golden(e: &Emulator, name: &str) {
    let text = e.machine().lcd().to_text();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.txt"));
    if std::env::var_os("SATURNUS_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
    }
    let want = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(text, want, "{name}: the screen differs from the golden");
}

/// Set the clock by keys and wait until they are done.
fn set_keys(e: &mut Emulator, t: ClockTime) {
    assert_eq!(e.clock_refusal(), None);
    let start = e.emulated_ms();
    e.set_clock_by_keys(&now_of(t), &Pace::REAL_TIME, &Manual::default())
        .unwrap();
    settle(e, 1500.0);
    eprintln!(
        "{}: set by keys in {:.1} s emulated",
        e.model().name(),
        (e.emulated_ms() - start) / 1000.0
    );
}

#[test]
fn the_aplet_models_store_time_and_date_in_home() {
    for (model, file) in [
        (Model::Hp38g, "38G_A167.ROM"),
        (Model::Hp39g, "rom.39g"),
        (Model::Hp40g, "rom.39g"),
    ] {
        let Some(rom) = rom(file) else {
            continue;
        };
        let mut e = Emulator::new(model, &rom).unwrap();
        run(&mut e, 5000.0);
        settle(&mut e, 1500.0);
        click(&mut e, "f"); // "Memory Clear": OK
        click(&mut e, "home");
        // A line of history to keep.
        for k in ["2", "plus", "3", "enter"] {
            click(&mut e, k);
        }
        let before = e.machine().lcd().to_text();
        let start = e.emulated_ms();
        set_keys(&mut e, when());
        assert_eq!(
            e.machine().lcd().to_text(),
            before,
            "{}: HOME's history as it was",
            model.name()
        );
        // Read back: TIME and DATE recalled in HOME.
        for name in ["TIME", "DATE"] {
            for c in name.chars() {
                assert!(e.queue().type_letter(c));
            }
            e.queue().type_keys(&["enter"]);
            e.pump();
            settle(&mut e, 1500.0);
            eprintln!(
                "{}: {name} read {:.1} s after the set began",
                model.name(),
                (e.emulated_ms() - start) / 1000.0
            );
        }
        golden(&e, &format!("clock-{}", model.name()));
    }
}

#[test]
/// The 27S, 17BII, 17B and 19BII: by keys in TIME SET.
fn the_pioneers_set_their_clock_in_time_set() {
    for (model, file) in [
        (Model::Hp27s, "hp27s.rom"),
        (Model::Hp17bii, "hp17bii.rom"),
        (Model::Hp17b, "hp17b.rom"),
        (Model::Hp19bii, "hp19bii.rom"),
    ] {
        let Some(rom) = rom(file) else {
            continue;
        };
        let mut e = Emulator::new(model, &rom).unwrap();
        run(&mut e, 3000.0);
        settle(&mut e, 800.0);
        if model != Model::Hp27s && rom.len() > 64 * 1024 {
            // The language menu takes no other key: refused there.
            assert_eq!(e.clock_refusal().as_deref(), Some(clock::NO_LANGUAGE));
            // ENGL: the second menu key on the 17Bs, the sixth on the 19BII.
            click(&mut e, if model == Model::Hp19bii { "f" } else { "b" });
            assert_eq!(e.clock_refusal(), None, "{}", model.name());
        }
        // A pending shift would turn the plan's keys into others: refused.
        click(&mut e, "shift");
        assert_eq!(e.clock_refusal().as_deref(), Some(clock::LINE_OPEN));
        click(&mut e, "shift");
        let start = e.emulated_ms();
        set_keys(&mut e, when());
        // Read back: the TIME menu shows the date and time.
        let open: &[&str] = if model == Model::Hp27s {
            &["shift", "divide"]
        } else {
            &["shift", "exit", "d"]
        };
        for k in open {
            click(&mut e, k);
        }
        eprintln!(
            "{}: TIME shown {:.1} s after the set began",
            model.name(),
            (e.emulated_ms() - start) / 1000.0
        );
        golden(&e, &format!("clock-{}", model.name()));
        // Day first, 24-hour: the settings are read, not assumed.
        click(&mut e, "d"); // SET
        click(&mut e, "d"); // M/D
        click(&mut e, "e"); // 12/24
        let exit: &[&str] = if model == Model::Hp27s {
            &["exit", "exit"]
        } else {
            &["shift", "exit"]
        };
        for k in exit {
            click(&mut e, k);
        }
        set_keys(&mut e, ClockTime::new(2026, 2, 3, 9, 8, 7).unwrap());
        for k in open {
            click(&mut e, k);
        }
        golden(&e, &format!("clock-{}-dmy-24h", model.name()));
    }
}

#[test]
fn a_calculator_that_is_off_is_refused() {
    let Some(mut e) = boot_rpl(Model::Hp48gx, "gxrom-r") else {
        return;
    };
    // Right shift, ON: OFF.
    click(&mut e, "rightshift");
    e.machine_mut().key_down(Key::On).unwrap();
    run(&mut e, 100.0);
    e.machine_mut().key_up(Key::On).unwrap();
    settle(&mut e, 1500.0);
    assert!(!e.machine().display_on());
    assert_eq!(e.clock_refusal().as_deref(), Some(clock::OFF));
}

/// One text line of the screen: the column masks of its glyphs, split at
/// blank columns, each relative to the line's top row.
type Glyphs = Vec<Vec<u16>>;

/// The screen's text lines: bands of rows with a dark pixel between
/// blank rows (at most 16 rows high; a taller band, a menu bar or a
/// frame, is no text).
fn text_lines(m: &Machine) -> Vec<Glyphs> {
    let rows = &m.framebuffer().pixels.pixels;
    let blank = |r: &Vec<bool>| !r.iter().any(|&p| p);
    let mut lines = Vec::new();
    let mut y = 0;
    while y < rows.len() {
        if blank(&rows[y]) {
            y += 1;
            continue;
        }
        let top = y;
        while y < rows.len() && !blank(&rows[y]) {
            y += 1;
        }
        if y - top > 16 {
            continue;
        }
        let width = rows[top].len();
        let column = |x: usize| -> u16 {
            (top..y)
                .filter(|&r| rows[r][x])
                .map(|r| 1 << (r - top))
                .sum()
        };
        let mut glyphs = Vec::new();
        let mut cur = Vec::new();
        let mut gap = 0;
        for x in 0..width {
            let c = column(x);
            if c == 0 {
                if !cur.is_empty() {
                    glyphs.push(std::mem::take(&mut cur));
                }
                gap += 1;
            } else {
                // Three blank columns or more: a space (an empty glyph).
                if gap >= 3 && !glyphs.is_empty() {
                    glyphs.push(Vec::new());
                }
                gap = 0;
                cur.push(c);
            }
        }
        if !cur.is_empty() {
            glyphs.push(cur);
        }
        lines.push(glyphs);
    }
    lines
}

/// The digits of a model's font, 0 to 9, learnt from the screen after
/// `1234567890` was keyed: the first line whose glyphs wider than two
/// columns (not a comma or a point) are ten different ones.
fn learn_digits(m: &Machine) -> Vec<Vec<u16>> {
    for line in text_lines(m) {
        let wide: Vec<Vec<u16>> = line.into_iter().filter(|g| g.len() > 2).collect();
        let mut distinct = wide.clone();
        distinct.sort();
        distinct.dedup();
        if wide.len() == 10 && distinct.len() == 10 {
            let mut digits = wide[9..].to_vec();
            digits.extend_from_slice(&wide[..9]);
            return digits;
        }
    }
    panic!("no line of ten digits:\n{}", m.lcd().to_text());
}

/// The screen's lines as text: the learnt digits, `:` and `.` for the
/// narrow glyphs with a dot high and low or only low, a space for a wide
/// gap, `?` for anything else.
fn read_lines(m: &Machine, digits: &[Vec<u16>]) -> Vec<String> {
    text_lines(m)
        .into_iter()
        .map(|line| {
            line.iter()
                .map(|g| {
                    if g.is_empty() {
                        return ' ';
                    }
                    if let Some(d) = digits.iter().position(|d| d == g) {
                        return char::from(b'0' + d as u8);
                    }
                    let all = g.iter().fold(0, |a, c| a | c);
                    match (g.len() <= 2, all.count_ones(), all.trailing_zeros()) {
                        (true, 4, z) if z > 0 => ':',
                        (true, 2, z) if z >= 4 => '.',
                        _ => '?',
                    }
                })
                .collect()
        })
        .collect()
}

/// The protocol's engine on a clock moved by hand, a machine started.
struct Host {
    engine: Engine,
    clock: Manual,
    tag: u64,
}

impl Host {
    fn new(e: Emulator, speed: &str) -> Host {
        let mut h = Host {
            engine: Engine::new("test", Pacing::WORKER),
            clock: Manual::default(),
            tag: 0,
        };
        h.clock.0.set(1_000_000.0);
        h.engine.start(&h.clock, e, "rom");
        h.command(json!({"cmd": "setSpeed", "speed": speed}));
        h
    }

    fn command(&mut self, mut msg: serde_json::Value) -> u64 {
        msg["v"] = json!(1);
        self.tag += 1;
        self.engine.command(&self.clock, &msg, None, Some(self.tag));
        self.tag
    }

    /// Run the host's timers until the set is done: no keys queued, no
    /// write, the loop asleep, the reply in; the wall time then.
    fn run_set(&mut self, tag: u64) -> f64 {
        let mut replied = false;
        for _ in 0..1_000_000 {
            for out in self.engine.take_output() {
                if let Output::Reply(r) = out
                    && r.tag == tag
                {
                    assert!(r.result.is_ok(), "{:?}", r.result);
                    replied = true;
                }
            }
            let e = self.engine.emulator().unwrap();
            if replied
                && !e.keys_busy()
                && !e.transferring()
                && self.engine.status().loop_state == LoopState::Sleep
            {
                return self.clock.now_ms();
            }
            let due = self.engine.deadline().expect("a timer");
            self.clock.0.set(self.clock.now_ms().max(due));
            self.engine.timer(&self.clock);
        }
        panic!("the set did not end");
    }
}

/// Queue `keys` (and the letters of `letters` before them) and run until
/// they have played and the CPU sleeps; the emulated ms before the last
/// key's turn came and after it all.
fn read_with(e: &mut Emulator, letters: &str, keys: &[&str]) -> (f64, f64) {
    for c in letters.chars() {
        assert!(e.queue().type_letter(c));
    }
    let (last, first) = keys.split_last().unwrap();
    e.queue().type_keys(first);
    e.pump();
    while e.keys_busy() || !e.machine().is_shutdown() {
        run(e, 1.0);
    }
    let start = e.emulated_ms();
    e.queue().type_keys(&[last]);
    e.pump();
    while e.keys_busy() || !e.machine().is_shutdown() {
        run(e, 1.0);
    }
    (start, e.emulated_ms())
}

/// A machine of `model` from `file` at rest where a set may start, and its
/// digits learnt; `None` without the ROM.
fn ready(model: Model, file: &str) -> Option<(Emulator, Vec<Vec<u16>>)> {
    if model == Model::Hp48gx {
        return boot_rpl(model, file).map(|e| (e, Vec::new()));
    }
    let mut e = Emulator::new(model, &rom(file)?).unwrap();
    run(&mut e, 5000.0);
    settle(&mut e, 1500.0);
    let digits = "1234567890"
        .chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>();
    match model {
        Model::Hp38g | Model::Hp39g | Model::Hp40g => {
            click(&mut e, "f"); // "Memory Clear": OK
            click(&mut e, "home");
            for d in &digits {
                click(&mut e, d);
            }
            click(&mut e, "enter");
        }
        _ => {
            if model == Model::Hp19bii {
                click(&mut e, "f"); // ENGL
            } else if model != Model::Hp27s {
                click(&mut e, "b"); // ENGL
            }
            for d in &digits {
                click(&mut e, d);
            }
        }
    }
    let learnt = learn_digits(e.machine());
    if model == Model::Hp19bii {
        // ◀ deletes what was keyed, a digit at a time.
        for _ in 0..12 {
            click(&mut e, "backspace");
        }
    } else if !matches!(model, Model::Hp38g | Model::Hp39g | Model::Hp40g) {
        click(&mut e, "exit"); // CLR, as the owner leaves it
        click(&mut e, "exit");
    }
    settle(&mut e, 1500.0);
    Some((e, learnt))
}

/// Seconds of the day of the time the calculator shows, read after the
/// set; the emulated ms when the readout began and ended.
fn shown_time(e: &mut Emulator, digits: &[Vec<u16>]) -> (f64, f64, f64) {
    let model = e.model();
    match model {
        Model::Hp48gx => {
            let start = e.emulated_ms();
            type_run(e, "TIME");
            let end = e.emulated_ms();
            let s = stack_texts(e);
            type_run(e, "DROP");
            (seconds_of(number(&s[0])), start, end)
        }
        Model::Hp38g | Model::Hp39g | Model::Hp40g => {
            let (start, end) = read_with(e, "TIME", &["enter"]);
            let lines = read_lines(e.machine(), digits);
            // The result: the last line that is a number H.MMSS (its
            // trailing zeros dropped).
            let text = lines
                .iter()
                .rev()
                .map(|l| l.replace(' ', ""))
                .find(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit() || c == '.'))
                .unwrap_or_else(|| panic!("{}: no time in {lines:?}", model.name()));
            let (h, frac) = text.split_once('.').unwrap_or((&text, ""));
            let frac = format!("{frac:0<4}");
            let hms: f64 = format!("{h}.{frac}").parse().unwrap();
            (seconds_of(hms), start, end)
        }
        _ => {
            let open: &[&str] = if model == Model::Hp27s {
                &["shift", "divide"]
            } else {
                &["shift", "exit", "d"]
            };
            let (start, end) = read_with(e, "", open);
            let lines = read_lines(e.machine(), digits);
            // H:MM:SS, 12-hour or 24-hour: compared modulo 12 hours.
            let text = lines
                .iter()
                .find_map(|l| {
                    let parts: Vec<&str> = l.split(':').collect();
                    (parts.len() == 3).then(|| {
                        let h: String = parts[0].rsplit(' ').next().unwrap_or_default().to_string();
                        let m = parts[1].trim();
                        let s: String = parts[2].trim().chars().take(2).collect();
                        format!("{h}:{m}:{s}")
                    })
                })
                .unwrap_or_else(|| panic!("{}: no time in {lines:?}", model.name()));
            let v: Vec<f64> = text.split(':').map(|p| p.parse().unwrap()).collect();
            (v[0] * 3600.0 + v[1] * 60.0 + v[2], start, end)
        }
    }
}

/// On every model set by keys and on the 48GX, at 1x and at Max (60x on
/// the Worker's pacing): the time requested, 14:05:30.200 and .700, set through
/// the engine on a clock moved by hand; once the set is done the time the
/// calculator shows is the time requested plus the wall time since,
/// within a second. The wall time is the hand clock's up to the set's
/// end (less what the sleeping calculator still owes it), then the
/// emulated time of the readout, which runs as the sleeping calculator
/// does, at 1x.
#[test]
fn the_clock_reads_the_wall_clock_once_set() {
    for (model, file) in [
        (Model::Hp48gx, "gxrom-r"),
        (Model::Hp38g, "38G_A167.ROM"),
        (Model::Hp39g, "rom.39g"),
        (Model::Hp40g, "rom.39g"),
        (Model::Hp27s, "hp27s.rom"),
        (Model::Hp17bii, "hp17bii.rom"),
        (Model::Hp17b, "hp17b.rom"),
        (Model::Hp19bii, "hp19bii.rom"),
    ] {
        let Some((e, digits)) = ready(model, file) else {
            continue;
        };
        for (speed, ms) in [("1", 200), ("1", 700), ("max", 200), ("max", 700)] {
            let requested = 14.0 * 3600.0 + 5.0 * 60.0 + 30.0 + f64::from(ms) / 1000.0;
            let mut h = Host::new(Emulator::from_machine(e.machine().clone()), speed);
            let begin = h.clock.now_ms();
            let tag = h.command(json!({
                "cmd": "setClock", "year": 2026, "month": 10, "day": 13,
                "hour": 14, "minute": 5, "second": 30, "millisecond": ms,
            }));
            let end = h.run_set(tag);
            let owed = h.engine.stats(&h.clock).owed_ms;
            let set_wall = (end - owed - begin) / 1000.0;
            let mut e = h.engine.into_emulator().unwrap();
            let at = e.emulated_ms();
            let (shown, start, stop) = shown_time(&mut e, &digits);
            // The wall clock while the readout drew the time.
            let early = requested + set_wall + (start - at) / 1000.0;
            let late = requested + set_wall + (stop - at) / 1000.0;
            // Modulo 12 hours (a Pioneer may show 2:05 for 14:05).
            let shown = early + (shown - early + 21_600.0).rem_euclid(43_200.0) - 21_600.0;
            // The Pioneers show whole seconds: `shown` stands for
            // [shown, shown + 1) there (the 48's and the aplets' TIME has
            // fractions). How far that lies from [early, late]:
            let width = if shown.fract() == 0.0 { 1.0 } else { 0.0 };
            let off = if shown + width < early {
                shown + width - early
            } else if shown > late {
                shown - late
            } else {
                0.0
            };
            eprintln!(
                "{} at {speed}, .{ms}: set in {set_wall:.2} s of wall time; shows {shown:.2} \
                 while the wall clock is {early:.2}..{late:.2} ({off:+.2} s)",
                model.name()
            );
            assert!(
                off.abs() <= 1.0,
                "{} at {speed}, .{ms}: the clock is {off:+.2} s off",
                model.name()
            );
        }
    }
}
