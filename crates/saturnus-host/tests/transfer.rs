//! The hidden Kermit transaction on the real ROMs (skipped unless
//! `SATURNUS_ROM_DIR` holds `sxrom-j`, `gxrom-r`, `rom-2.10.49g`): a text
//! file and a binary file stored, fetched back byte for byte, renamed,
//! purged, a directory changed into and out of, flags set and cleared;
//! the stack, the current directory, flag -35 and the screen are as they
//! were, and each write takes well under a second of wall time in a
//! release build. The 49G's writes in algebraic mode, which come back to
//! it on every path; a stop anywhere leaves the calculator at its stack.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::time::Instant;

use saturnus::io::Key;
use saturnus::{Machine, Model};
use saturnus_host::Emulator;
use saturnus_host::transfer::{Op, TransferResult};
use saturnus_host::typing::{Job, Verb};

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

/// Press `k` as the typing engine does: hold, release, wait for sleep,
/// gap; then wait for the screen.
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
        // Algebraic mode: the flag is set by keys, then the server works.
        let r = write(
            &mut e,
            Op::SetFlag {
                flag: -95,
                on: false,
            },
        );
        assert!(r.keys, "{r:?}");
        assert_eq!(e.flags().unwrap().get(-95), Some(false));
        // The echo algebraic mode left is dropped.
        assert_eq!(stack_texts(&e), Vec::<String>::new());
    }
    Some(e)
}

/// Type `text` and ENTER.
fn run(e: &mut Emulator, text: &str) {
    let m = e.machine_mut();
    let mut job = Job::new(m, Verb::Run, text).unwrap();
    while !job.step(m, 50 * per_ms(m)).unwrap() {}
    assert_eq!(job.outcome().error, None, "{text}");
}

fn transfer(e: &mut Emulator, op: Op) -> saturnus_host::Result<(TransferResult, Option<Vec<u8>>)> {
    e.start_transfer(op)?;
    while !e.transfer_step(20.0)? {}
    e.transfer_result()
}

/// One write, which must succeed; prints its wall time.
fn write(e: &mut Emulator, op: Op) -> TransferResult {
    let what = format!("{op:?}").chars().take(60).collect::<String>();
    let t = Instant::now();
    let (r, _) = transfer(e, op).unwrap_or_else(|err| panic!("{what}: {err}"));
    eprintln!(
        "{}: {what}: {:.0} ms wall, {:.0} ms emulated",
        e.model().name(),
        t.elapsed().as_secs_f64() * 1000.0,
        r.emulated_ms
    );
    r
}

fn fetch(e: &mut Emulator, dir: &[&str], name: &str) -> Vec<u8> {
    let t = Instant::now();
    let (r, file) = transfer(
        e,
        Op::Fetch {
            dir: strings(dir),
            name: name.into(),
        },
    )
    .unwrap();
    eprintln!(
        "{}: fetch {name}: {:.0} ms wall, {:.0} ms emulated",
        e.model().name(),
        t.elapsed().as_secs_f64() * 1000.0,
        r.emulated_ms
    );
    let file = file.unwrap();
    assert_eq!(r.size, Some(file.len()));
    file
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn names(e: &Emulator, dir: &[&str]) -> Vec<String> {
    let tree = e.memory_tree().unwrap();
    let mut vars = &tree.variables;
    for d in dir.iter().skip(1) {
        vars = vars
            .iter()
            .find(|v| v.name == *d)
            .and_then(|v| v.variables.as_ref())
            .unwrap();
    }
    vars.iter().map(|v| v.name.clone()).collect()
}

fn stack_texts(e: &Emulator) -> Vec<String> {
    e.stack()
        .unwrap()
        .iter()
        .map(|v| v["text"].as_str().unwrap_or("?").to_string())
        .collect()
}

#[test]
fn writes_through_the_kermit_server() {
    for (model, file) in MODELS {
        let Some(mut e) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        run(&mut e, "'D' CRDIR 42 7");
        let stack = stack_texts(&e);
        let flags = e.flags().unwrap();
        let screen = e.machine().lcd();
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);

        // A text file: an ASCII transfer the calculator compiles.
        let text = "%%HP: T(3)A(D)F(.);\r\n« 1 2 + »\r\n";
        let r = write(
            &mut e,
            Op::Store {
                dir: strings(&["HOME", "D"]),
                name: "P".into(),
                data: text.as_bytes().to_vec(),
            },
        );
        assert_eq!(r.name.as_deref(), Some("P"));
        assert!(names(&e, &["HOME", "D"]).contains(&"P".to_string()));
        // Back in HOME, the stack untouched, -35 as it was.
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);
        assert_eq!(stack_texts(&e), stack);
        assert_eq!(e.flags().unwrap(), flags);

        // Fetched in binary; stored again under another name and fetched
        // back, it is the same file.
        let got = fetch(&mut e, &["HOME", "D"], "P");
        assert!(got.starts_with(b"HPHP4"), "{got:?}");
        assert_eq!(e.machine().lcd(), screen, "the screen after a fetch");
        let r = write(
            &mut e,
            Op::Store {
                dir: strings(&["HOME"]),
                name: "Q".into(),
                data: got.clone(),
            },
        );
        assert_eq!(r.name.as_deref(), Some("Q"));
        let again = fetch(&mut e, &["HOME"], "Q");
        assert_eq!(again, got);
        assert_eq!(stack_texts(&e), stack);
        assert_eq!(e.flags().unwrap(), flags);

        // Rename and purge.
        write(
            &mut e,
            Op::Rename {
                dir: strings(&["HOME"]),
                name: "Q".into(),
                to: "R".into(),
            },
        );
        let home = names(&e, &["HOME"]);
        assert!(home.contains(&"R".to_string()) && !home.contains(&"Q".to_string()));
        write(
            &mut e,
            Op::Purge {
                dir: strings(&["HOME"]),
                name: "R".into(),
            },
        );
        assert!(!names(&e, &["HOME"]).contains(&"R".to_string()));
        assert_eq!(stack_texts(&e), stack);

        // Into D and back.
        write(
            &mut e,
            Op::ChangeDir {
                dir: strings(&["HOME", "D"]),
            },
        );
        assert_eq!(e.memory_tree().unwrap().path, ["HOME", "D"]);
        write(
            &mut e,
            Op::ChangeDir {
                dir: strings(&["HOME"]),
            },
        );
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);

        // Flags.
        write(&mut e, Op::SetFlag { flag: 5, on: true });
        assert_eq!(e.flags().unwrap().get(5), Some(true));
        write(&mut e, Op::SetFlag { flag: 5, on: false });
        assert_eq!(e.flags().unwrap().get(5), Some(false));
        assert_eq!(stack_texts(&e), stack);
        assert_eq!(e.machine().lcd(), screen, "the screen after the writes");

        // HOME holds D and the server's IOPAR (and on the 49G, the CAS's
        // CASDIR).
        let mut home = names(&e, &["HOME"]);
        home.retain(|n| n != "CASDIR");
        assert_eq!(home, ["IOPAR", "D"]);

        // A command line the calculator refuses: reported, nothing changed
        // ('SIN' in quotes is the command, not a name).
        let err = transfer(
            &mut e,
            Op::Rename {
                dir: strings(&["HOME", "D"]),
                name: "P".into(),
                to: "SIN".into(),
            },
        )
        .unwrap_err();
        eprintln!("{}: {err}", model.name());
        assert!(err.to_string().contains("Invalid Syntax"), "{err}");
        assert_eq!(stack_texts(&e), stack);
        assert_eq!(names(&e, &["HOME", "D"]), ["P"]);
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);

        // The clock in the header (flag -40), and a transfer with it on.
        write(
            &mut e,
            Op::SetFlag {
                flag: -40,
                on: true,
            },
        );
        assert_eq!(e.flags().unwrap().get(-40), Some(true));
        let with_clock = e.machine().lcd();
        assert_ne!(with_clock, screen, "{}: the clock shows", model.name());
        assert_eq!(fetch(&mut e, &["HOME", "D"], "P").len(), got.len());
        write(
            &mut e,
            Op::SetFlag {
                flag: -40,
                on: false,
            },
        );
        assert_eq!(
            e.machine().lcd(),
            screen,
            "{}: the clock is gone",
            model.name()
        );

        // A store the calculator refuses itself (its E packet): with flag
        // -36 set (overwrite), a binary file into HOME under the name of
        // the directory D, while D is current. The server still runs, so
        // the cleanup runs: flag -35 is put back, the calculator changes
        // back to D, and the server ends with G F.
        write(
            &mut e,
            Op::SetFlag {
                flag: -36,
                on: true,
            },
        );
        write(
            &mut e,
            Op::ChangeDir {
                dir: strings(&["HOME", "D"]),
            },
        );
        let flags = e.flags().unwrap();
        let screen = e.machine().lcd();
        let t = Instant::now();
        let err = transfer(
            &mut e,
            Op::Store {
                dir: strings(&["HOME"]),
                name: "D".into(),
                data: got.clone(),
            },
        )
        .unwrap_err();
        eprintln!(
            "{}: refused store: {err} ({} ms wall)",
            model.name(),
            t.elapsed().as_millis()
        );
        assert!(
            err.to_string()
                .contains("calculator error: Directory Not Allowed"),
            "{err}"
        );
        assert_eq!(e.flags().unwrap(), flags, "{}: -35 put back", model.name());
        assert_eq!(e.memory_tree().unwrap().path, ["HOME", "D"]);
        assert_eq!(stack_texts(&e), stack);
        assert_eq!(e.machine().lcd(), screen, "{}: the screen", model.name());
        assert_eq!(names(&e, &["HOME", "D"]), ["P"]);
        // The server was left properly: the next writes work.
        write(
            &mut e,
            Op::ChangeDir {
                dir: strings(&["HOME"]),
            },
        );
        write(
            &mut e,
            Op::SetFlag {
                flag: -36,
                on: false,
            },
        );

        // Refusals that run nothing.
        let err = transfer(
            &mut e,
            Op::Purge {
                dir: strings(&["HOME"]),
                name: "NONE".into(),
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("no variable NONE"), "{err}");
    }
}

/// The directory of `name` in `dir`, which must be one.
fn subdir(e: &Emulator, dir: &[&str], name: &str) -> Vec<String> {
    let mut path = dir.to_vec();
    path.push(name);
    let tree = e.memory_tree().unwrap();
    let mut vars = &tree.variables;
    for d in path.iter().skip(1) {
        vars = vars
            .iter()
            .find(|v| v.name == *d)
            .and_then(|v| v.variables.as_ref())
            .unwrap_or_else(|| panic!("{name} is not a directory in {dir:?}"));
    }
    vars.iter().map(|v| v.name.clone()).collect()
}

/// `createDir`: `'name' CRDIR` in a directory, current or not; the
/// calculator stays where it was; a taken name, a bad name and a missing
/// directory are refused before anything runs, a command name by the
/// calculator.
#[test]
fn directories_created_through_the_kermit_server() {
    for (model, file) in MODELS {
        let Some(mut e) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        run(&mut e, "5 'X' STO 42 7");
        let stack = stack_texts(&e);
        let flags = e.flags().unwrap();
        let screen = e.machine().lcd();
        let mkdir = |dir: &[&str], name: &str| Op::CreateDir {
            dir: strings(dir),
            name: name.into(),
        };

        // In the current directory: an empty directory, nothing else
        // changed.
        let r = write(&mut e, mkdir(&["HOME"], "A"));
        assert!(!r.keys);
        assert!(subdir(&e, &["HOME"], "A").is_empty());
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);
        assert_eq!(stack_texts(&e), stack);
        assert_eq!(e.flags().unwrap(), flags);
        assert_eq!(e.machine().lcd(), screen, "{}: the screen", model.name());

        // Nested, from HOME: the calculator stays in HOME.
        write(&mut e, mkdir(&["HOME", "A"], "B"));
        assert_eq!(subdir(&e, &["HOME"], "A"), ["B"]);
        assert!(subdir(&e, &["HOME", "A"], "B").is_empty());
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);

        // From inside A, one in HOME: the calculator changes back to A.
        write(
            &mut e,
            Op::ChangeDir {
                dir: strings(&["HOME", "A"]),
            },
        );
        write(&mut e, mkdir(&["HOME"], "C"));
        assert!(subdir(&e, &["HOME"], "C").is_empty());
        assert_eq!(e.memory_tree().unwrap().path, ["HOME", "A"]);
        // And one deeper than the current directory.
        write(&mut e, mkdir(&["HOME", "A", "B"], "E"));
        assert!(subdir(&e, &["HOME", "A", "B"], "E").is_empty());
        assert_eq!(e.memory_tree().unwrap().path, ["HOME", "A"]);
        assert_eq!(stack_texts(&e), stack);
        write(
            &mut e,
            Op::ChangeDir {
                dir: strings(&["HOME"]),
            },
        );

        // Refused before anything runs: a name taken by a directory or by
        // a variable, a name that is not plain, a directory that is not
        // there.
        let home = names(&e, &["HOME"]);
        for (dir, name, why) in [
            (&["HOME"][..], "A", "A already exists in { HOME }"),
            (&["HOME"][..], "X", "X already exists in { HOME }"),
            (&["HOME", "A"][..], "B", "B already exists in { HOME A }"),
            (&["HOME"][..], "1A", "not a plain variable name"),
            (&["HOME"][..], "A B", "not a plain variable name"),
            (&["HOME"][..], "", "not a plain variable name"),
            (&["HOME", "NONE"][..], "F", "no directory { HOME NONE }"),
        ] {
            let err = transfer(&mut e, mkdir(dir, name)).unwrap_err();
            assert!(err.to_string().contains(why), "{name}: {err}");
        }
        assert_eq!(names(&e, &["HOME"]), home);

        // A command's name ('SIN' in quotes is the command): the
        // calculator refuses it, nothing changed.
        let err = transfer(&mut e, mkdir(&["HOME", "A"], "SIN")).unwrap_err();
        eprintln!("{}: {err}", model.name());
        assert!(err.to_string().contains("Invalid Syntax"), "{err}");
        assert_eq!(subdir(&e, &["HOME"], "A"), ["B"]);
        assert_eq!(e.memory_tree().unwrap().path, ["HOME"]);
        assert_eq!(stack_texts(&e), stack);
        assert_eq!(e.flags().unwrap(), flags);
        assert_eq!(e.machine().lcd(), screen, "{}: the screen", model.name());
    }
}

/// The 49G in algebraic mode (flag -95 set, as it boots): every write
/// goes through the server all the same, -95 cleared for it by keys and
/// set again after it; the stack, the current directory and the screen
/// are as they were. A refused write and a stop put -95 back too.
#[test]
fn writes_on_the_49g_in_algebraic_mode() {
    let Some(mut e) = boot(Model::Hp49g, "rom-2.10.49g") else {
        eprintln!("skipped: no rom-2.10.49g in SATURNUS_ROM_DIR");
        return;
    };
    run(&mut e, "42 7");
    // Algebraic again, through the server (RPN mode now).
    write(
        &mut e,
        Op::SetFlag {
            flag: -95,
            on: true,
        },
    );
    settle(e.machine_mut(), 5_000);
    let flags = e.flags().unwrap();
    assert_eq!(flags.get(-95), Some(true), "algebraic mode");
    let stack = stack_texts(&e);
    assert_eq!(stack.len(), 2, "{stack:?}");
    let screen = e.machine().lcd();
    let same = |e: &mut Emulator, what: &str| {
        settle(e.machine_mut(), 5_000);
        assert_eq!(
            e.flags().unwrap(),
            flags,
            "{what}: the flags, -95 set again"
        );
        assert_eq!(stack_texts(e), stack, "{what}: the stack");
        assert_eq!(
            e.memory_tree().unwrap().path,
            ["HOME"],
            "{what}: the directory"
        );
        assert_eq!(e.machine().lcd(), screen, "{what}: the screen");
    };

    let r = write(
        &mut e,
        Op::CreateDir {
            dir: strings(&["HOME"]),
            name: "D".into(),
        },
    );
    assert!(!r.keys, "through the server");
    assert!(names(&e, &["HOME"]).contains(&"D".to_string()));
    same(&mut e, "createDir");

    let text = "%%HP: T(3)A(D)F(.);\r\n« 1 2 + »\r\n";
    let r = write(
        &mut e,
        Op::Store {
            dir: strings(&["HOME", "D"]),
            name: "P".into(),
            data: text.as_bytes().to_vec(),
        },
    );
    assert_eq!(r.name.as_deref(), Some("P"));
    same(&mut e, "storeFile");

    let got = fetch(&mut e, &["HOME", "D"], "P");
    assert!(got.starts_with(b"HPHP49"), "{got:?}");
    same(&mut e, "fetchFile");

    write(
        &mut e,
        Op::Rename {
            dir: strings(&["HOME", "D"]),
            name: "P".into(),
            to: "Q".into(),
        },
    );
    assert_eq!(names(&e, &["HOME", "D"]), ["Q"]);
    same(&mut e, "rename");

    write(
        &mut e,
        Op::Purge {
            dir: strings(&["HOME", "D"]),
            name: "Q".into(),
        },
    );
    assert!(names(&e, &["HOME", "D"]).is_empty());
    same(&mut e, "purge");

    // Into D and back: -95 set again each time.
    write(
        &mut e,
        Op::ChangeDir {
            dir: strings(&["HOME", "D"]),
        },
    );
    settle(e.machine_mut(), 5_000);
    assert_eq!(e.memory_tree().unwrap().path, ["HOME", "D"]);
    assert_eq!(e.flags().unwrap().get(-95), Some(true));
    assert_eq!(stack_texts(&e), stack);
    write(
        &mut e,
        Op::ChangeDir {
            dir: strings(&["HOME"]),
        },
    );
    same(&mut e, "changeDir");

    // The calculator refuses (a command's name, `'SIN' CRDIR`): -95 set
    // again all the same.
    let err = transfer(
        &mut e,
        Op::CreateDir {
            dir: strings(&["HOME"]),
            name: "SIN".into(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("CRDIR"), "{err}");
    same(&mut e, "a refusal");

    // A stop while CF(-95) is typed, while the server runs, while it ends
    // and while -95 is typed again: -95 set again, the stack whole.
    for ms in [300.0, 3_000.0, 6_000.0, 9_000.0, 12_500.0] {
        e.start_transfer(Op::CreateDir {
            dir: strings(&["HOME"]),
            name: "E".into(),
        })
        .unwrap();
        let done = e.transfer_step(ms).unwrap();
        stop(&mut e);
        let what = format!("a stop after {ms} ms (done: {done})");
        assert_eq!(e.flags().unwrap().get(-95), Some(true), "{what}");
        assert_eq!(stack_texts(&e), stack, "{what}");
        assert!(!e.transferring());
        if names(&e, &["HOME"]).contains(&"E".to_string()) {
            write(
                &mut e,
                Op::Purge {
                    dir: strings(&["HOME"]),
                    name: "E".into(),
                },
            );
        }
    }
    same(&mut e, "the stops");
}

/// A stop anywhere in a write (typing `SERVER`, waiting for its NAK, in a
/// transaction, ending it) leaves the calculator at its stack: no line
/// typed in part, no server left running; the next write works and the
/// stack is as it was.
#[test]
fn a_stop_anywhere_leaves_the_calculator_at_its_stack() {
    for (model, file) in MODELS {
        let Some(mut e) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        run(&mut e, "42 7");
        let stack = stack_texts(&e);
        for ms in [200.0, 1_500.0, 3_000.0, 5_000.0, 7_000.0, 9_000.0] {
            e.start_transfer(Op::CreateDir {
                dir: strings(&["HOME"]),
                name: "S".into(),
            })
            .unwrap();
            let done = e.transfer_step(ms).unwrap();
            stop(&mut e);
            let what = format!("{}: a stop after {ms} ms (done: {done})", model.name());
            assert_eq!(stack_texts(&e), stack, "{what}");
            let line = saturnus_objects::cmdline::command_line(e.machine()).unwrap();
            assert!(!line.active, "{what}: {:?}", line.text);
            // The next write gets a server of its own.
            write(
                &mut e,
                Op::ChangeDir {
                    dir: strings(&["HOME"]),
                },
            );
            if names(&e, &["HOME"]).contains(&"S".to_string()) {
                write(
                    &mut e,
                    Op::Purge {
                        dir: strings(&["HOME"]),
                        name: "S".into(),
                    },
                );
            }
            assert_eq!(stack_texts(&e), stack, "{what}: after the next write");
        }
    }
}

/// Stop the write in progress and run its cleanup in turns of 50 ms of
/// emulated time, as a host does; at most a minute of it.
fn stop(e: &mut Emulator) {
    e.stop_transfer();
    let mut turns = 0;
    while e.transferring() {
        let per_turn = e.machine().cycles();
        let done = e.transfer_step(50.0).unwrap();
        let ran = e.machine().cycles() - per_turn;
        assert!(ran <= 60 * per_ms(e.machine()), "a turn ran {ran} cycles");
        if done {
            // A write already done when stopped keeps its result.
            let _ = e.transfer_result();
            break;
        }
        turns += 1;
        assert!(turns < 1_200, "the stop's cleanup ends within a minute");
    }
    settle(e.machine_mut(), 5_000);
}

/// The size and checksum of variable `name` in `dir` (from the tree).
fn identity_of(e: &Emulator, dir: &[&str], name: &str) -> Option<(u64, u16)> {
    let tree = e.memory_tree().unwrap();
    let mut vars = &tree.variables;
    for d in dir.iter().skip(1) {
        vars = vars.iter().find(|v| v.name == *d)?.variables.as_ref()?;
    }
    vars.iter()
        .find(|v| v.name == name)
        .map(|v| ((v.size * 2.0) as u64, v.checksum))
}

/// `copy` and `move` on each model: an object copied (the original
/// kept, a taken name refused unless replaced), an object moved, a
/// directory moved with what it holds, a directory into itself refused,
/// and a copy the calculator has no memory for, which keeps the original;
/// the stack, the current directory and the flags as they were.
#[test]
fn copies_and_moves_through_the_kermit_server() {
    for (model, file) in MODELS {
        let Some(mut e) = boot(model, file) else {
            eprintln!("skipped: no {file} in SATURNUS_ROM_DIR");
            continue;
        };
        run(
            &mut e,
            "'D' CRDIR 'E' CRDIR 5 'X' STO D 9 'Q' STO 'S' CRDIR HOME 42 7",
        );
        let stack = stack_texts(&e);
        let flags = e.flags().unwrap();
        let same = |e: &Emulator, what: &str| {
            assert_eq!(e.memory_tree().unwrap().path, ["HOME"], "{what}");
            assert_eq!(stack_texts(e), stack, "{what}");
            assert_eq!(e.flags().unwrap(), flags, "{what}");
        };
        let op = |name: &str, from: &[&str], to: &[&str], replace: bool, remove: bool| Op::Copy {
            dir: strings(from),
            name: name.into(),
            to: strings(to),
            replace,
            remove,
        };
        let x = identity_of(&e, &["HOME"], "X").unwrap();

        // A copy: both there, the same object.
        write(&mut e, op("X", &["HOME"], &["HOME", "E"], false, false));
        assert_eq!(identity_of(&e, &["HOME"], "X"), Some(x));
        assert_eq!(identity_of(&e, &["HOME", "E"], "X"), Some(x));
        same(&e, "copy");

        // The name taken: refused, then replaced when asked.
        let err = transfer(&mut e, op("X", &["HOME"], &["HOME", "E"], false, false)).unwrap_err();
        assert!(
            err.to_string().contains("X already exists in { HOME E }"),
            "{err}"
        );
        run(&mut e, "6 'X' STO");
        let x6 = identity_of(&e, &["HOME"], "X").unwrap();
        assert_ne!(x6, x);
        write(&mut e, op("X", &["HOME"], &["HOME", "E"], true, false));
        assert_eq!(identity_of(&e, &["HOME", "E"], "X"), Some(x6), "replaced");
        same(&mut e, "replace");

        // A move: gone from HOME, in D.
        write(&mut e, op("X", &["HOME"], &["HOME", "D"], false, true));
        assert_eq!(identity_of(&e, &["HOME"], "X"), None);
        assert_eq!(identity_of(&e, &["HOME", "D"], "X"), Some(x6));
        same(&e, "move");

        // A directory with what it holds: D into E.
        let d = identity_of(&e, &["HOME"], "D").unwrap();
        write(&mut e, op("D", &["HOME"], &["HOME", "E"], false, true));
        assert_eq!(identity_of(&e, &["HOME"], "D"), None);
        assert_eq!(identity_of(&e, &["HOME", "E"], "D"), Some(d));
        let mut held = names(&e, &["HOME", "E", "D"]);
        held.sort();
        assert_eq!(held, ["Q", "S", "X"]);
        same(&e, "move a directory");

        // Into itself, or a directory inside it: refused.
        for to in [&["HOME", "E"][..], &["HOME", "E", "D"][..]] {
            let err = transfer(&mut e, op("E", &["HOME"], to, false, true)).unwrap_err();
            assert!(
                err.to_string().contains("cannot go into itself")
                    || err.to_string().contains("already"),
                "{err}"
            );
        }
        let err = transfer(
            &mut e,
            op("E", &["HOME"], &["HOME", "E", "D", "S"], false, true),
        )
        .unwrap_err();
        assert!(err.to_string().contains("cannot go into itself"), "{err}");
        assert!(identity_of(&e, &["HOME"], "E").is_some());

        // No memory for the copy: the calculator's error, the original
        // kept, nothing in the target.
        // A real array of 3/5 of the free memory (MEM: about 30 KB on the
        // 48SX, 128 KB on the 48GX, 240 KB on the 49G).
        let free = match model {
            Model::Hp48sx => 30_000,
            Model::Hp48gx => 127_000,
            _ => 240_000,
        };
        let n = free * 3 / 5 / 8;
        run(&mut e, &format!("{{ {n} 1 }} 0. CON 'BIG' STO"));
        settle(e.machine_mut(), 10_000);
        let big = identity_of(&e, &["HOME"], "BIG").unwrap();
        let err = transfer(&mut e, op("BIG", &["HOME"], &["HOME", "E"], false, true)).unwrap_err();
        eprintln!("{}: {err}", model.name());
        assert!(err.to_string().contains("Memory"), "{err}");
        assert_eq!(
            identity_of(&e, &["HOME"], "BIG"),
            Some(big),
            "the original kept"
        );
        assert_eq!(identity_of(&e, &["HOME", "E"], "BIG"), None);
        same(&e, "no memory");
    }
}
