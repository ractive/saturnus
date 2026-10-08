//! The palette's editor on the real ROMs (skipped unless
//! `SATURNUS_ROM_DIR` holds `sxrom-j`, `gxrom-r`, `rom-2.10.49g`): a
//! program in a variable pulled (`editText`), changed, saved (`storeText`)
//! and read back equal to the edited text; a no-op save keeps the
//! checksum, for programs and for every other kind of object with a text;
//! a syntax error is the calculator's own and leaves the object, the stack
//! and the directory as they were; a stack level replaced; a live `EDIT`
//! session replaced with `replace`, staying in the edit.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::time::Instant;

use saturnus::io::Key;
use saturnus::{Machine, Model};
use saturnus_host::Emulator;
use saturnus_host::transfer::{Op, TEXT_VARIABLE, Target, TransferResult};
use saturnus_host::typing::{Job, Verb};
use saturnus_objects::cmdline;

const MODELS: [(Model, &str); 3] = [
    (Model::Hp48sx, "sxrom-j"),
    (Model::Hp48gx, "gxrom-r"),
    (Model::Hp49g, "rom-2.10.49g"),
];

fn rom(file: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    std::fs::read(PathBuf::from(dir).join(file)).ok()
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

/// Press `k` as the typing engine does, then wait for the screen.
fn press(m: &mut Machine, k: Key) {
    m.key_down(k).unwrap();
    m.run_cycles(30 * per_ms(m)).unwrap();
    m.key_up(k).unwrap();
    let cap = m.cycles() + 5000 * per_ms(m);
    while !m.is_shutdown() && m.cycles() < cap {
        m.run_cycles(per_ms(m) / 4).unwrap();
    }
    m.run_cycles(100 * per_ms(m)).unwrap();
    settle(m, 10_000);
}

fn transfer(e: &mut Emulator, op: Op) -> saturnus_host::Result<TransferResult> {
    e.start_transfer(op)?;
    while !e.transfer_step(20.0)? {}
    Ok(e.transfer_result()?.0)
}

/// A cold-booted machine at the stack, the 49G in RPN mode.
fn boot(model: Model, file: &str) -> Option<Emulator> {
    let mut m = Machine::new(model, &rom(file)?).unwrap();
    settle(&mut m, 60_000);
    let n = if model == Model::Hp49g { 2 } else { 1 };
    for _ in 0..n {
        press(&mut m, Key::F);
    }
    let mut e = Emulator::from_machine(m);
    if model == Model::Hp49g {
        let r = transfer(
            &mut e,
            Op::SetFlag {
                flag: -95,
                on: false,
            },
        )
        .unwrap();
        assert!(r.keys);
    }
    Some(e)
}

/// Type `text` and ENTER.
fn run(e: &mut Emulator, text: &str) {
    send(e, Verb::Run, text);
}

fn send(e: &mut Emulator, verb: Verb, text: &str) {
    let m = e.machine_mut();
    let mut job = Job::new(m, verb, text).unwrap();
    while !job.step(m, 50 * per_ms(m)).unwrap() {}
    assert_eq!(job.outcome().error, None, "{text}");
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn var(name: &str) -> Target {
    Target::Variable(name.into())
}

/// The variable `name` of `dir`: (checksum, size).
fn checksum(e: &Emulator, dir: &[&str], name: &str) -> (u32, f64) {
    let tree = e.memory_tree().unwrap();
    let mut vars = &tree.variables;
    for d in dir.iter().skip(1) {
        vars = vars
            .iter()
            .find(|v| v.name == *d)
            .and_then(|v| v.variables.as_ref())
            .unwrap();
    }
    let v = vars.iter().find(|v| v.name == name).unwrap();
    (v.checksum.into(), v.size)
}

fn has_text_variable(e: &Emulator) -> bool {
    fn any(vars: &[saturnus_objects::Variable]) -> bool {
        vars.iter()
            .any(|v| v.name.starts_with(TEXT_VARIABLE) || v.variables.as_deref().is_some_and(any))
    }
    any(&e.memory_tree().unwrap().variables)
}

fn stack_texts(e: &Emulator) -> Vec<String> {
    e.stack()
        .unwrap()
        .iter()
        .map(|v| v["text"].as_str().unwrap_or("?").to_string())
        .collect()
}

/// Save `text` into `target` of `dir`; prints the wall time.
fn store(e: &mut Emulator, dir: &[&str], target: Target, text: &str) -> TransferResult {
    let t = Instant::now();
    let r = transfer(
        e,
        Op::StoreText {
            dir: strings(dir),
            target,
            text: text.into(),
        },
    )
    .unwrap_or_else(|err| panic!("{}: {text}: {err}", e.model().name()));
    eprintln!(
        "{}: store {} chars: {:.0} ms wall, {:.0} ms emulated{}",
        e.model().name(),
        text.chars().count(),
        t.elapsed().as_secs_f64() * 1000.0,
        r.emulated_ms,
        r.error
            .as_deref()
            .map(|e| format!(", refused: {e}"))
            .unwrap_or_default()
    );
    r
}

#[test]
fn the_editor_round_trips_through_the_calculator() {
    for (model, file) in MODELS {
        let Some(mut e) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        let name = model.name();
        run(&mut e, "« 1 2 + » 'P' STO 'D' CRDIR 42 7");
        let home = ["HOME"];
        let stack = stack_texts(&e);
        let flags = e.flags().unwrap();
        let screen = e.machine().lcd();

        // Pulled: the calculator's own text.
        let text = e.edit_text(&strings(&home), &var("P")).unwrap();
        assert_eq!(text, "« 1 2 + »", "{name}");

        // A no-op save keeps the object, byte for byte.
        let before = checksum(&e, &home, "P");
        let r = store(&mut e, &home, var("P"), &text);
        assert_eq!(r.error, None, "{name}");
        assert_eq!(checksum(&e, &home, "P"), before, "{name}: no-op save");
        assert_eq!(stack_texts(&e), stack, "{name}: the stack");
        assert_eq!(e.flags().unwrap(), flags, "{name}: the flags");
        assert_eq!(e.machine().lcd(), screen, "{name}: the screen");
        assert!(!has_text_variable(&e), "{name}: the text is gone");

        // Changed and saved: read back equal to the edited text. The
        // characters `;` and `→` are no problem (the 48SX has no key for
        // `;`).
        let edited = "« → X « X 2 ^ \"a;b\" DROP » »";
        let r = store(&mut e, &home, var("P"), edited);
        assert_eq!(r.error, None, "{name}");
        assert_eq!(e.edit_text(&strings(&home), &var("P")).unwrap(), edited);
        assert_ne!(checksum(&e, &home, "P"), before, "{name}");
        // A multi-line edit compiles to the calculator's own text.
        let r = store(&mut e, &home, var("P"), "«\n  1 2 +\n  3 *\n»");
        assert_eq!(r.error, None, "{name}");
        assert_eq!(
            e.edit_text(&strings(&home), &var("P")).unwrap(),
            "« 1 2 + 3 * »"
        );
        let saved = checksum(&e, &home, "P");

        // A syntax error: the calculator's message, the object intact.
        let r = store(&mut e, &home, var("P"), "« 1 2 + 3 * ) »");
        assert_eq!(r.error.as_deref(), Some("Invalid Syntax"), "{name}");
        assert_eq!(checksum(&e, &home, "P"), saved, "{name}: intact");
        assert_eq!(stack_texts(&e), stack, "{name}: the stack");
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);
        assert!(!has_text_variable(&e), "{name}: the text is gone");
        // Two objects are not one.
        let r = store(&mut e, &home, var("P"), "1 2");
        assert_eq!(
            r.error.as_deref(),
            Some("the text holds more than one object")
        );
        assert_eq!(checksum(&e, &home, "P"), saved, "{name}: intact");
        assert_eq!(stack_texts(&e), stack, "{name}: the stack");
        // A text that would close the list it is compiled in.
        let err = transfer(
            &mut e,
            Op::StoreText {
                dir: strings(&home),
                target: var("P"),
                text: "} 'P' PURGE {".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("closes a list"), "{err}");

        // A long program (typing it would take minutes of emulated time
        // on the 48SX): read back as sent.
        let long = format!(
            "« {} »",
            (0..400)
                .map(|i| format!("{i} DROP"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let t = Instant::now();
        let r = store(&mut e, &home, var("L"), &long);
        assert_eq!(r.error, None, "{name}");
        assert_eq!(e.edit_text(&strings(&home), &var("L")).unwrap(), long);
        // Wall time only in a release build (a debug build is ~20 times slower).
        if !cfg!(debug_assertions) {
            assert!(t.elapsed().as_secs() < 2, "{name}: {:?}", t.elapsed());
        }

        // A variable in another directory, from HOME.
        store(&mut e, &["HOME", "D"], var("Q"), "{ 1 \"x\" 'A+1' }");
        assert_eq!(
            e.edit_text(&strings(&["HOME", "D"]), &var("Q")).unwrap(),
            "{ 1 \"x\" 'A+1' }"
        );
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);

        // No-op saves of other objects keep their checksums, also in FIX 3
        // (the text keeps every digit).
        run(&mut e, "3 FIX");
        let sources = [
            "1.23456789012",
            "(1.5,-2)",
            "\"two\nlines\"",
            "'X^2+SIN(Y)'",
            ":T:5",
            "2_m/s^2",
            "# 1234h",
            "{ 1.5 :A:{ X } [ 1 2 ] }",
            "[[ 1 2 ] [ 3 4 ]]",
            "« IF 1 THEN \"y\" ELSE 2 END »",
        ];
        for (i, source) in sources.iter().enumerate() {
            let n = format!("V{i}");
            run(&mut e, &format!("{source} '{n}' STO"));
            let text = e.edit_text(&strings(&home), &var(&n)).unwrap();
            let before = checksum(&e, &home, &n);
            let r = store(&mut e, &home, var(&n), &text);
            assert_eq!(r.error, None, "{name}: {source} as {text:?}");
            assert_eq!(
                checksum(&e, &home, &n),
                before,
                "{name}: {source} as {text:?}"
            );
        }
        run(&mut e, "STD");

        // A stack level: level 2 replaced, the others where they were.
        assert_eq!(stack_texts(&e), ["7", "42"], "{name}");
        let text = e.edit_text(&[], &Target::Level(2)).unwrap();
        assert_eq!(text, "42");
        let r = store(&mut e, &home, Target::Level(2), "« 43 »");
        assert_eq!(r.error, None, "{name}");
        assert_eq!(stack_texts(&e), ["7", "« 43 »"], "{name}");
        let r = store(&mut e, &home, Target::Level(1), "« 8");
        assert_eq!(r.error.as_deref(), Some("Invalid Syntax"), "{name}");
        assert_eq!(stack_texts(&e), ["7", "« 43 »"], "{name}");
        let err = transfer(
            &mut e,
            Op::StoreText {
                dir: vec![],
                target: Target::Level(3),
                text: "1".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("no level 3"), "{err}");

        // A live EDIT session: the command line pulled, replaced, and the
        // calculator still in the edit with the new text.
        let edit = if model == Model::Hp49g {
            vec![Key::Down]
        } else {
            vec![Key::LeftShift, Key::Neg]
        };
        for k in edit {
            press(e.machine_mut(), k);
        }
        let line = cmdline::command_line(e.machine()).unwrap();
        assert!(line.active, "{name}: EDIT is open");
        assert_eq!(line.text, "7", "{name}");
        send(&mut e, Verb::Replace, "« 7 8 »");
        let line = cmdline::command_line(e.machine()).unwrap();
        assert!(line.active, "{name}: still in the edit");
        assert_eq!(line.text, "« 7 8 »", "{name}");
        // A save refuses while the line is open.
        let err = transfer(
            &mut e,
            Op::StoreText {
                dir: strings(&home),
                target: var("P"),
                text: "1".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("command line is open"), "{err}");
        run(&mut e, "");
        assert_eq!(stack_texts(&e), ["« 7 8 »", "« 43 »"], "{name}");
    }
}
