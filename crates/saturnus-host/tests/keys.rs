//! The key queue on the 48SX ROM (skipped unless `SATURNUS_ROM_DIR` holds
//! `sxrom-j`): two quick Ctrl+clicks on √x square twice whatever the
//! phase of the ROM's timer when they come. The ROM stays awake about
//! 90 ms or about 520 ms after a key goes up, by that phase, and may drop
//! a key pressed in that time; a queue that pressed after 300-400 ms lost
//! the second shift in about one phase in five and took √x for x².
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use saturnus::Model;
use saturnus_host::Emulator;

fn rom() -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    std::fs::read(PathBuf::from(dir).join("sxrom-j")).ok()
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
    assert!(e.press(key));
    e.release(key);
    e.pump();
    settle(e);
}

#[test]
fn quick_ctrl_clicks_square_twice_at_every_phase() {
    let Some(rom) = rom() else {
        eprintln!("skipped: SATURNUS_ROM_DIR/sxrom-j not found");
        return;
    };
    let mut e = Emulator::new(Model::Hp48sx, &rom).unwrap();
    run(&mut e, 3000.0);
    settle(&mut e);
    // NO to "Try To Recover Memory?", then 3 ENTER.
    for k in ["f", "3", "enter"] {
        click(&mut e, k);
    }
    assert_eq!(top(&e), "3");
    let state = e.save_state();
    let mut wrong = Vec::new();
    for n in 0..60 {
        // The first click `phase` ms in, the second `gap` ms after it.
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
        "3 squared twice, at (phase, gap, result): {wrong:?}"
    );
}
