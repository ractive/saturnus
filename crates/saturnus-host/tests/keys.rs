//! The key queue on the ROMs whose calculators stay awake long after a
//! key (skipped for each ROM `SATURNUS_ROM_DIR` lacks). The 48SX ROM
//! stays awake about 250 ms or, by its timer's phase, about 525 ms after
//! a shift, and may drop a key pressed in that time: a queue that pressed
//! after 300-400 ms lost the second of two quick Ctrl+clicks' shifts in
//! about one phase in five and took √x for x². Each model's queue waits
//! its own busy gap (`saturnus_host::host::busy_gap_ms`).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use saturnus::Model;
use saturnus_host::Emulator;
use saturnus_host::host::{Keyboard, busy_gap_ms};

fn rom(file: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    std::fs::read(PathBuf::from(dir).join(file)).ok()
}

/// Run `ms` emulated ms in the host's slices, the queue fed after each.
fn run(e: &mut Emulator, ms: f64) {
    let mut left = ms;
    while left > 0.0 {
        left -= e.run_slice(left, true).unwrap();
    }
}

/// Run until the queue is empty and the CPU has slept for 1.5 s, at most
/// 30 s (the boot prompt blinks).
fn settle(e: &mut Emulator) {
    let mut quiet = 0.0;
    for _ in 0..3000 {
        run(e, 10.0);
        quiet = if !e.queue().busy() && e.machine().is_shutdown() {
            quiet + 10.0
        } else {
            0.0
        };
        if quiet > 1500.0 {
            return;
        }
    }
}

fn top(e: &Emulator) -> String {
    e.stack()
        .unwrap()
        .first()
        .and_then(|v| v["text"].as_str().map(str::to_string))
        .unwrap_or_default()
}

fn click(e: &mut Emulator, key: &str) {
    assert!(e.press(key), "{key}");
    e.release(key);
    e.pump();
    settle(e);
}

/// The calculator booted from `file`, past its start-up prompt, then
/// `keys` clicked one by one; None without the ROM.
fn booted(model: Model, file: &str, keys: &[&str]) -> Option<Emulator> {
    let Some(rom) = rom(file) else {
        eprintln!("skipped: SATURNUS_ROM_DIR/{file} not found");
        return None;
    };
    let mut e = Emulator::new(model, &rom).unwrap();
    run(&mut e, 3000.0);
    settle(&mut e);
    for k in keys {
        click(&mut e, k);
    }
    Some(e)
}

/// Two quick Ctrl+clicks on √x, the first `phase` ms in and the second
/// `gap` ms after it, from 3 on the stack: 81 at every phase.
fn quick_ctrl_clicks_square_twice(model: Model, file: &str) {
    let Some(mut e) = booted(model, file, &["f", "3", "enter"]) else {
        return;
    };
    assert_eq!(top(&e), "3");
    let state = e.save_state();
    let mut wrong = Vec::new();
    for n in 0..60 {
        let phase = f64::from(n) * 7.3;
        let gap = f64::from(n % 13) * 11.0;
        e.load_state(&state).unwrap();
        e.release_keys();
        run(&mut e, phase);
        assert!(e.press_shifted("sqrt", "leftshift"));
        e.release("sqrt");
        run(&mut e, gap);
        assert!(e.press_shifted("sqrt", "leftshift"));
        e.release("sqrt");
        e.pump();
        settle(&mut e);
        let got = top(&e);
        if got != "81" {
            wrong.push((phase, gap, got));
        }
    }
    assert!(
        wrong.is_empty(),
        "{model:?}: 3 squared twice, at (phase, gap, result): {wrong:?}"
    );
}

#[test]
fn quick_ctrl_clicks_square_twice_on_the_48sx() {
    quick_ctrl_clicks_square_twice(Model::Hp48sx, "sxrom-j");
}

#[test]
fn quick_ctrl_clicks_square_twice_on_the_48gx() {
    quick_ctrl_clicks_square_twice(Model::Hp48gx, "gxrom-r");
}

/// `keys` queued at once, at 30 timer phases: no press after the first
/// goes down while the ROM is still awake from the one before (it would
/// risk being dropped), as the model's busy gap is longer than the ROM's
/// time awake after these keys.
fn no_press_while_awake(model: Model, file: &str, boot: &[&str], keys: &[&str]) {
    let Some(mut e) = booted(model, file, boot) else {
        return;
    };
    let state = e.save_state();
    let mut awake = Vec::new();
    for n in 0..30 {
        let phase = f64::from(n) * 7.3;
        e.load_state(&state).unwrap();
        e.release_keys();
        run(&mut e, phase);
        for k in keys {
            assert!(e.press(k), "{k}");
            e.release(k);
        }
        e.pump();
        // The keys down after each slice, and whether the ROM slept when
        // the queue pressed them (the pump runs after the slice).
        let mut last: Vec<&str> = Vec::new();
        let mut pressed = 0;
        for _ in 0..1000 {
            let idle = e.machine().is_shutdown();
            let t = e.machine().now_ms();
            e.pump();
            let down = e.queue().down();
            for k in down.iter().filter(|k| !last.contains(k)) {
                pressed += 1;
                if pressed > 1 && !idle {
                    awake.push((phase, (*k).to_string(), t));
                }
            }
            last = down;
            if !e.queue().busy() {
                break;
            }
            e.run_slice(10.0, true).unwrap();
        }
        assert_eq!(pressed, keys.len(), "{model:?}: every key went down");
    }
    assert!(
        awake.is_empty(),
        "{model:?} (busy gap {} ms): pressed while awake at (phase, key, ms): {awake:?}",
        busy_gap_ms(model)
    );
}

#[test]
fn queued_keys_wait_for_the_49g() {
    no_press_while_awake(
        Model::Hp49g,
        "rom.49g",
        &["f", "f"],
        &["enter", "1", "enter", "2"],
    );
}

#[test]
fn queued_keys_wait_for_the_38g() {
    no_press_while_awake(
        Model::Hp38g,
        "38G_A167.ROM",
        &[],
        &["1", "enter", "shift", "2"],
    );
}

#[test]
fn queued_keys_wait_for_the_48gx() {
    no_press_while_awake(
        Model::Hp48gx,
        "gxrom-r",
        &["f"],
        &["1", "enter", "leftshift", "2"],
    );
}
