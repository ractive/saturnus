//! The 28S's command line read from RAM on its ROM (skipped unless
//! `SATURNUS_ROM_DIR` holds `hp28s.rom`): open or not, its text and
//! cursor, its entry modes, as keys type, move, insert, enter and cancel
//! (wiki: hardware/hp28s "Command line").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use saturnus_core::Model;
use saturnus_host::Emulator;
use saturnus_objects::cmdline;

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
/// 30 s (the cursor blinks).
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

/// Click each key; `shift+k` presses `k` shifted.
fn keys(e: &mut Emulator, keys: &str) {
    for k in keys.split_whitespace() {
        if let Some((shift, key)) = k.split_once('+') {
            assert!(e.press_shifted(key, shift), "{k}");
            e.release(key);
        } else {
            assert!(e.press(k), "{k}");
            e.release(k);
        }
        e.pump();
        settle(e);
    }
}

/// The line as `(active, text, cursor)`.
fn line(e: &Emulator) -> (bool, String, usize) {
    let l = e.command_line().unwrap();
    (l.active, l.text, l.cursor)
}

#[test]
fn the_28s_command_line_follows_the_keys() {
    let Some(rom) = rom("hp28s.rom") else {
        eprintln!("skipped: SATURNUS_ROM_DIR/hp28s.rom not found");
        return;
    };
    let mut e = Emulator::new(Model::Hp28s, &rom).unwrap();
    run(&mut e, 3000.0);
    settle(&mut e);
    // ATTN takes "Memory Lost" away.
    keys(&mut e, "on");
    assert_eq!(line(&e), (false, String::new(), 0));

    keys(&mut e, "lettera letterb letterc");
    assert_eq!(line(&e), (true, "ABC".into(), 3));
    // The ROM starts in replace mode; INS (the first menu key) inserts.
    keys(&mut e, "e e letterx");
    assert_eq!(line(&e), (true, "AXC".into(), 2));
    assert!(!cmdline::editor(e.machine()).unwrap().insert);
    keys(&mut e, "a lettery");
    assert_eq!(line(&e), (true, "AXYC".into(), 3));
    assert!(cmdline::editor(e.machine()).unwrap().insert);
    // ATTN cancels: the line closes and its text is gone.
    keys(&mut e, "on");
    assert_eq!(line(&e), (false, String::new(), 0));

    // « types the 28S's own code 146 and starts alpha entry.
    keys(&mut e, "program 1");
    assert_eq!(line(&e), (true, "«1".into(), 2));
    assert!(cmdline::editor(e.machine()).unwrap().program);
    keys(&mut e, "on quote letterx");
    assert_eq!(line(&e), (true, "'X".into(), 2));
    assert!(cmdline::editor(e.machine()).unwrap().algebraic);
    keys(&mut e, "enter");
    assert_eq!(line(&e), (false, String::new(), 0));

    // A line that does not parse stays open under the error.
    keys(&mut e, "1 point point enter");
    assert!(line(&e).0, "open after Syntax Error");
    assert_eq!(line(&e).1, "1..");
    // Emptied, the line closes.
    keys(&mut e, "on 7 backspace");
    assert_eq!(line(&e), (false, String::new(), 0));

    // EDIT opens level 1 (the 'X' entered above) on the line.
    keys(&mut e, "shift+enter");
    let (open, text, _) = line(&e);
    assert!(open, "EDIT is open");
    assert_eq!(text, "'X'");
    keys(&mut e, "on");
    assert!(!line(&e).0);
}
