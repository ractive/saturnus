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
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use saturnus_core::io::Key;
use saturnus_core::{Machine, Model};
use saturnus_host::Emulator;
use saturnus_host::clock::{self, ClockTime};
use saturnus_host::transfer::Op;
use saturnus_host::typing::{Job, Verb};

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

/// Set the clock on a 48 or 49 as the protocol does.
fn set_rpl(e: &mut Emulator, t: ClockTime) {
    assert_eq!(e.clock_refusal(), None);
    e.start_transfer(Op::SetClock(t)).unwrap();
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
/// seconds since (typing `TIME` takes some).
fn assert_time(model: Model, text: &str) {
    let got = seconds_of(number(text));
    let want = 14.0 * 3600.0 + 5.0 * 60.0 + 30.0;
    assert!(
        (want..want + 20.0).contains(&got),
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
    e.set_clock_by_keys(&t).unwrap();
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
fn the_pioneers_set_their_clock_in_time_set() {
    for (model, file) in [
        (Model::Hp27s, "hp27s.rom"),
        (Model::Hp17bii, "hp17bii.rom"),
        (Model::Hp17b, "hp17b.rom"),
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
            click(&mut e, "b"); // ENGL
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
