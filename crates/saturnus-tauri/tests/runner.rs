//! The Tauri app's machine thread, driven through its protocol as the
//! window drives it, without a window. Needs a 48SX ROM: skipped unless
//! `SATURNUS_ROM_DIR` holds `sxrom-j` (as the workspace's e2e tests).
//!
//! The functional test waits for states (events), never for fixed times,
//! so it holds on a loaded machine. The pacing test measures emulated
//! against wall time, which only means something on a quiet machine: it
//! runs only when `SATURNUS_PACE_SECS` (seconds per measurement) is set.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use saturnus_tauri::runner::{Request, Sink, spawn};
use serde_json::{Value, json};

#[derive(Clone, Default)]
struct Collect(Arc<Mutex<Vec<Value>>>);

impl Sink for Collect {
    fn event(&self, msg: Value) {
        self.0.lock().unwrap().push(msg);
    }
}

impl Collect {
    fn last(&self, kind: &str) -> Option<Value> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|m| m["type"] == kind)
            .cloned()
    }
}

fn rom() -> Option<PathBuf> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    let p = PathBuf::from(dir).join("sxrom-j");
    p.exists().then_some(p)
}

fn call(tx: &Sender<Request>, mut msg: Value) -> Result<Value, String> {
    msg["v"] = json!(1);
    let (reply, answer) = channel();
    tx.send(Request {
        msg,
        file: None,
        reply: Some(reply),
        ticket: None,
    })
    .unwrap();
    answer.recv().unwrap()
}

/// A command with the file the host chose for it (the dialog's answer).
fn call_file(
    tx: &Sender<Request>,
    mut msg: Value,
    file: &std::path::Path,
) -> Result<Value, String> {
    msg["v"] = json!(1);
    let (reply, answer) = channel();
    tx.send(Request {
        msg,
        file: Some(file.to_path_buf()),
        reply: Some(reply),
        ticket: None,
    })
    .unwrap();
    answer.recv().unwrap()
}

fn press(tx: &Sender<Request>, key: &str) {
    call(tx, json!({"cmd": "keyDown", "key": key})).unwrap();
    call(tx, json!({"cmd": "keyUp", "key": key})).unwrap();
}

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Decode a frame's pixels: rows of booleans.
fn pixels(frame: &Value) -> Vec<Vec<bool>> {
    let w = frame["width"].as_u64().unwrap() as usize;
    let h = frame["height"].as_u64().unwrap() as usize;
    let b64 = frame["pixels"].as_str().unwrap();
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for c in b64.bytes().filter(|&c| c != b'=') {
        acc = (acc << 6) | alphabet.iter().position(|&a| a == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((acc >> bits) as u8);
        }
    }
    let row = w.div_ceil(8);
    (0..h)
        .map(|y| {
            (0..w)
                .map(|x| bytes[y * row + x / 8] & (0x80 >> (x % 8)) != 0)
                .collect()
        })
        .collect()
}

fn stats(tx: &Sender<Request>) -> (f64, f64, f64, u64) {
    let s = call(tx, json!({"cmd": "stats"})).unwrap();
    eprintln!("  stats {s}");
    (
        s["emulatedMs"].as_f64().unwrap() + s["owedMs"].as_f64().unwrap(),
        s["nowMs"].as_f64().unwrap(),
        s["workMs"].as_f64().unwrap(),
        s["ticks"].as_u64().unwrap(),
    )
}

/// Emulated ms (paid and owed) per wall ms over `secs`.
fn measure(tx: &Sender<Request>, secs: u64) -> (f64, f64, u64) {
    let (a, wa, ka, ta) = stats(tx);
    sleep(secs * 1000);
    let (b, wb, kb, tb) = stats(tx);
    eprintln!(
        "  emulated {:.1} ms over {:.1} ms wall, work {:.1} ms, {} passes",
        b - a,
        wb - wa,
        kb - ka,
        tb - ta
    );
    ((b - a) / (wb - wa), kb - ka, tb - ta)
}

/// Wait up to 30 s of wall time for `ok` to hold on the collected events.
fn wait_for(events: &Collect, what: &str, ok: impl Fn(&Collect) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while !ok(events) {
        if std::time::Instant::now() >= deadline {
            let screen: String = events
                .last("frame")
                .map(|f| {
                    pixels(&f)
                        .iter()
                        .map(|r| {
                            r.iter()
                                .map(|&b| if b { '#' } else { '.' })
                                .collect::<String>()
                                + "\n"
                        })
                        .collect()
                })
                .unwrap_or_default();
            let keys: Vec<String> = events
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|m| m["type"] == "keys" || m["type"] == "error")
                .map(|m| m.to_string())
                .collect();
            panic!(
                "timed out waiting for {what}; status {:?}\nkey events {keys:?}\n{screen}",
                events.last("status"),
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Wait until the ROM has settled into its idle key wait: 1.5 s of
/// emulated time from now, then asleep with no key down. Emulated, not
/// wall, time, so a loaded host waits longer and nothing races: after NO
/// the 48SX clears memory for about 1 s (dozing in between) and drops a
/// key pressed in its first ~550 ms.
fn settled(tx: &Sender<Request>, events: &Collect) {
    let emulated = |tx: &Sender<Request>| {
        call(tx, json!({"cmd": "stats"})).unwrap()["emulatedMs"]
            .as_f64()
            .unwrap()
    };
    let until = emulated(tx) + 1500.0;
    let tx2 = tx.clone();
    wait_for(events, "the ROM idle", move |e| {
        emulated(&tx2) >= until
            && loop_is(e, "sleep")
            && e.last("keys").is_some_and(|k| k["down"] == json!([]))
    });
}

fn loop_is(events: &Collect, state: &str) -> bool {
    events.last("status").is_some_and(|s| s["loop"] == state)
}

fn frame(events: &Collect) -> Vec<Vec<bool>> {
    pixels(&events.last("frame").unwrap())
}

/// The ROM's 5 x 9 "A" at the left edge of some row.
fn has_a(p: &[Vec<bool>]) -> bool {
    let glyph = [
        ".###.", "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#", "#...#",
    ];
    (0..=p.len() - glyph.len()).any(|y| {
        glyph.iter().enumerate().all(|(dy, row)| {
            row.chars()
                .enumerate()
                .all(|(x, c)| p[y + dy][x] == (c == '#'))
        })
    })
}

/// A 48SX booted to an empty stack ("Try To Recover Memory?" NO), asleep.
fn booted(rom: &std::path::Path) -> (Collect, Sender<Request>) {
    let events = Collect::default();
    let tx = spawn(events.clone()).unwrap();
    // The 49G is preferred, but a 256 KB ROM only fits the 48SX.
    let booted = call_file(&tx, json!({"cmd": "boot", "model": "49g"}), rom).unwrap();
    assert_eq!(booted["model"], "48sx");
    assert_eq!(booted["romName"], "sxrom-j");
    // The prompt is up once the ROM waits for a key.
    settled(&tx, &events);
    let prompt = frame(&events);
    press(&tx, "f");
    wait_for(&events, "the stack", |e| frame(e) != prompt);
    settled(&tx, &events);
    (events, tx)
}

/// Type `1 30000 START NEXT` and ENTER: the CPU computes for a while.
fn start_loop(tx: &Sender<Request>, events: &Collect) {
    for c in "1 30000 START NEXT".chars() {
        match c {
            ' ' => press(tx, "space"),
            d if d.is_ascii_digit() => press(tx, &d.to_string()),
            l => {
                call(tx, json!({"cmd": "typeLetter", "letter": l.to_string()})).unwrap();
            }
        }
    }
    // Typed out once the queue is empty and the ROM idles again.
    settled(tx, events);
    press(tx, "enter");
    wait_for(events, "the loop running", |e| loop_is(e, "frame"));
}

#[test]
fn boots_types_saves_and_pauses() {
    let Some(rom) = rom() else {
        eprintln!("skipped: SATURNUS_ROM_DIR/sxrom-j not found");
        return;
    };
    let (events, tx) = booted(&rom);
    let hello = call(&tx, json!({"cmd": "hello"})).unwrap();
    assert_eq!(hello["host"], "tauri");
    let (reply, answer) = channel();
    tx.send(Request {
        msg: json!({"cmd": "hello", "v": 2}),
        file: None,
        reply: Some(reply),
        ticket: None,
    })
    .unwrap();
    assert!(answer.recv().unwrap().is_err(), "another protocol version");
    let status = events.last("status").unwrap();
    assert_eq!(status["running"], true);
    assert_eq!(status["model"], "48sx");

    // A letter typed as the page types it: alpha, then the key.
    assert!(!has_a(&frame(&events)));
    let ok = call(&tx, json!({"cmd": "typeLetter", "letter": "A"})).unwrap();
    assert_eq!(ok, true);
    wait_for(&events, "an A on the command line", |e| has_a(&frame(e)));
    wait_for(&events, "every key up", |e| {
        e.last("keys").is_some_and(|k| k["down"] == json!([]))
    });

    // State: clear the command line (its cursor blinks), save, change,
    // load: the frame that comes with the load's reply is the saved one.
    press(&tx, "on");
    wait_for(&events, "the cleared command line", |e| !has_a(&frame(e)));
    settled(&tx, &events);
    let file = std::env::temp_dir().join(format!("saturnus-tauri-{}.state", std::process::id()));
    let saved = call_file(&tx, json!({"cmd": "saveState"}), &file).unwrap();
    assert_eq!(saved["path"], file.display().to_string());
    let at_save = frame(&events);
    press(&tx, "9");
    wait_for(&events, "a 9 on the command line", |e| frame(e) != at_save);
    call_file(&tx, json!({"cmd": "loadState"}), &file).unwrap();
    assert_eq!(frame(&events), at_save, "the loaded state is shown");
    let _ = std::fs::remove_file(&file);

    // Pause at Max while the CPU computes: it stops at once (commands are
    // read between passes of at most 11 ms of work), and the status event
    // that says so arrives before the reply.
    call(&tx, json!({"cmd": "setSpeed", "speed": "max"})).unwrap();
    start_loop(&tx, &events);
    let asked = std::time::Instant::now();
    call(&tx, json!({"cmd": "pause", "paused": true})).unwrap();
    let took = asked.elapsed();
    let status = events.last("status").unwrap();
    assert_eq!(status["loop"], "stopped");
    assert_eq!(status["running"], false);
    assert_eq!(status["speed"], "max");
    eprintln!("pause at max answered in {took:?}");
    assert!(took < Duration::from_millis(500), "pause took {took:?}");
    let (cycles, ..) = stats(&tx);
    let (again, ..) = stats(&tx);
    assert_eq!(cycles, again, "no cycles run after the pause");
}

/// Real time, idle and computing, at 1x and 4x; Max faster than 4x.
/// Opt-in (`SATURNUS_PACE_SECS`): a loaded machine cannot keep a debug
/// build at four times a 48SX's clock, and the pacer drops a lag by design.
#[test]
fn keeps_real_time() {
    let Some(rom) = rom() else {
        eprintln!("skipped: SATURNUS_ROM_DIR/sxrom-j not found");
        return;
    };
    let Some(secs) = std::env::var("SATURNUS_PACE_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    else {
        eprintln!("skipped: set SATURNUS_PACE_SECS to measure pacing");
        return;
    };
    let (events, tx) = booted(&rom);

    eprintln!("idle at 1x for {secs} s:");
    let (rate, work, passes) = measure(&tx, secs);
    assert!((rate - 1.0).abs() < 0.01, "rate {rate}");
    assert!(work < 20.0 * secs as f64, "busy {work} ms while idle");
    assert!(passes < 10, "{passes} passes while idle");

    start_loop(&tx, &events);
    eprintln!("computing at 1x for {secs} s:");
    let (rate, _, passes) = measure(&tx, secs);
    assert!((rate - 1.0).abs() < 0.02, "rate {rate}");
    assert!(passes > 100 * secs, "paced passes");

    call(&tx, json!({"cmd": "setSpeed", "speed": "4"})).unwrap();
    eprintln!("at 4x for {secs} s:");
    let (rate, _, _) = measure(&tx, secs);
    assert!((rate - 4.0).abs() < 0.1, "rate {rate}");
    call(&tx, json!({"cmd": "setSpeed", "speed": "max"})).unwrap();
    eprintln!("at max for {secs} s:");
    let (rate, _, _) = measure(&tx, secs);
    assert!(rate > 4.0, "rate {rate}");
}

/// The speed applies to computing only: asleep, the calculator's clock
/// keeps to the wall clock at 4x and at Max (its auto-off, its cursor
/// blink). Idle costs no work, so this holds on a loaded machine too.
#[test]
fn idles_in_real_time_at_any_speed() {
    let Some(rom) = rom() else {
        eprintln!("skipped: SATURNUS_ROM_DIR/sxrom-j not found");
        return;
    };
    let (_events, tx) = booted(&rom);
    for speed in ["4", "max"] {
        call(&tx, json!({"cmd": "setSpeed", "speed": speed})).unwrap();
        eprintln!("idle at {speed} for 10 s:");
        let (rate, _, _) = measure(&tx, 10);
        assert!((rate - 1.0).abs() < 0.02, "idle at {speed}: rate {rate}");
    }
    // Resuming while asleep at Max runs nothing until the next timer
    // event: the clock gains nothing on the wall clock (a pause holds it).
    let (a, wa, ..) = stats(&tx);
    for _ in 0..5 {
        call(&tx, json!({"cmd": "pause", "paused": true})).unwrap();
        call(&tx, json!({"cmd": "pause", "paused": false})).unwrap();
        sleep(20);
    }
    sleep(500);
    let (b, wb, ..) = stats(&tx);
    let (gained, wall) = (b - a, wb - wa);
    eprintln!("resumed at max: emulated {gained:.1} ms over {wall:.1} ms wall");
    assert!(
        gained <= wall + 20.0,
        "resumed at max: {gained} ms over {wall} ms"
    );
}

/// The page cannot name a file, a command that needs one fails without
/// it, and files larger than any legitimate ROM or state are refused
/// without being read whole. Needs no ROM.
#[test]
fn files_come_from_the_host_and_are_capped() {
    use saturnus_tauri::runner::{MAX_STATE_FILE, max_rom_file, read_capped};
    let tx = spawn(Collect::default()).unwrap();
    for msg in [
        json!({"cmd": "boot", "model": "48sx", "romPath": "/etc/hosts"}),
        json!({"cmd": "saveState", "path": "/tmp/x"}),
        json!({"cmd": "loadState", "path": "/etc/hosts"}),
        // Refused even with a host-chosen file beside it.
    ] {
        let e = call(&tx, msg.clone()).unwrap_err();
        assert!(e.contains("not accepted"), "{e}");
        let e = call_file(&tx, msg, std::path::Path::new("/etc/hosts")).unwrap_err();
        assert!(e.contains("not accepted"), "{e}");
    }
    let e = call(&tx, json!({"cmd": "boot", "model": "48sx"})).unwrap_err();
    assert!(e.contains("needs a file"), "{e}");

    assert_eq!(max_rom_file(), 4 * 1024 * 1024);
    let dir = std::env::temp_dir().join(format!("saturnus-cap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Sparse: 1 GiB on paper, nothing on disk; refused by its length.
    let huge = dir.join("huge.rom");
    std::fs::File::create(&huge)
        .unwrap()
        .set_len(1 << 30)
        .unwrap();
    let started = std::time::Instant::now();
    let e = call_file(&tx, json!({"cmd": "boot", "model": "48sx"}), &huge).unwrap_err();
    assert!(e.contains("larger than"), "{e}");
    assert!(
        !e.contains(&dir.display().to_string()),
        "no directory in errors: {e}"
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    // One byte over the cap: refused; at the cap: read.
    let over = dir.join("over.state");
    std::fs::File::create(&over)
        .unwrap()
        .set_len(MAX_STATE_FILE + 1)
        .unwrap();
    assert!(
        read_capped(&over, MAX_STATE_FILE)
            .unwrap_err()
            .contains("larger than")
    );
    std::fs::File::create(&over)
        .unwrap()
        .set_len(MAX_STATE_FILE)
        .unwrap();
    assert_eq!(
        read_capped(&over, MAX_STATE_FILE).unwrap().len() as u64,
        MAX_STATE_FILE
    );
    // A device that never ends is cut at the cap, not read until memory
    // runs out.
    #[cfg(unix)]
    {
        let e = read_capped(std::path::Path::new("/dev/zero"), max_rom_file()).unwrap_err();
        assert!(e.contains("larger than"), "{e}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A reloaded page says `hello` again and gets the current state at once:
/// status and frame arrive before the reply, with no change on the LCD.
#[test]
fn hello_resends_the_state_to_a_reloaded_page() {
    let Some(rom) = rom() else {
        eprintln!("skipped: SATURNUS_ROM_DIR/sxrom-j not found");
        return;
    };
    let (events, tx) = booted(&rom);
    // Paused, so nothing can change the display meanwhile.
    call(&tx, json!({"cmd": "pause", "paused": true})).unwrap();
    let count = |kind: &str| {
        events
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m["type"] == kind)
            .count()
    };
    let shown = frame(&events);
    let (frames, statuses, keys) = (count("frame"), count("status"), count("keys"));
    let hello = call(&tx, json!({"cmd": "hello"})).unwrap();
    assert_eq!(hello["host"], "tauri");
    assert_eq!(count("frame"), frames + 1, "the frame again");
    assert_eq!(count("status"), statuses + 1, "the status again");
    assert_eq!(count("keys"), keys + 1, "the keys again");
    assert_eq!(frame(&events), shown);
    let status = events.last("status").unwrap();
    assert_eq!(status["model"], "48sx");
    assert_eq!(status["romName"], "sxrom-j");
    assert_eq!(status["running"], false);
}

/// Saving a state over an existing file never leaves it half written: a
/// write that fails midway (a full disk) keeps the old file and leaves no
/// temporary file behind; a write that succeeds replaces it.
#[test]
fn a_failed_state_write_keeps_the_old_file() {
    use saturnus_tauri::runner::write_atomic;
    let dir = std::env::temp_dir().join(format!("saturnus-atomic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("calc.state");
    std::fs::write(&file, b"old state").unwrap();
    let new = vec![7u8; 100_000];
    let e = write_atomic(&file, &new, |f, b| {
        use std::io::Write as _;
        f.write_all(&b[..b.len() / 2])?;
        Err(std::io::Error::other("disk full"))
    })
    .unwrap_err();
    assert_eq!(e.to_string(), "disk full");
    assert_eq!(std::fs::read(&file).unwrap(), b"old state");
    let left: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        left,
        [std::ffi::OsString::from("calc.state")],
        "no temporary file left"
    );
    write_atomic(&file, &new, |f, b| {
        use std::io::Write as _;
        f.write_all(b)
    })
    .unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), new);
    let _ = std::fs::remove_dir_all(&dir);
}

fn count(events: &Collect, kind: &str) -> usize {
    events
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|m| m["type"] == kind)
        .count()
}

/// `watchMemory`: a page that watches hears `memoryChanged` when the
/// stack changes, not while the calculator idles, and reads the change.
#[test]
fn memory_changes_reach_a_watching_page() {
    let Some(rom) = rom() else {
        eprintln!("skipped: SATURNUS_ROM_DIR/sxrom-j not found");
        return;
    };
    let (events, tx) = booted(&rom);
    // Nobody watches: keys change the stack, no event.
    press(&tx, "1");
    press(&tx, "enter");
    settled(&tx, &events);
    assert_eq!(count(&events, "memoryChanged"), 0);

    let w = call(&tx, json!({"cmd": "watchMemory", "on": true})).unwrap();
    assert_eq!(w, json!({"supported": true, "reason": null}));
    let stack = call(&tx, json!({"cmd": "stack"})).unwrap();
    assert_eq!(stack, json!([{"type": "real", "value": 1.0, "text": "1"}]));
    // Idle (the ROM's timer wakes pass meanwhile): nothing to tell.
    settled(&tx, &events);
    assert_eq!(count(&events, "memoryChanged"), 0);

    press(&tx, "2");
    press(&tx, "enter");
    wait_for(&events, "memoryChanged", |e| count(e, "memoryChanged") > 0);
    settled(&tx, &events);
    let stack = call(&tx, json!({"cmd": "stack"})).unwrap();
    assert_eq!(stack.as_array().map(Vec::len), Some(2));
    // Two keys, at most a few events, and none once it idles again.
    let told = count(&events, "memoryChanged");
    assert!((1..=4).contains(&told), "{told} events");
    settled(&tx, &events);
    assert_eq!(count(&events, "memoryChanged"), told);

    call(&tx, json!({"cmd": "watchMemory", "on": false})).unwrap();
    press(&tx, "3");
    press(&tx, "enter");
    settled(&tx, &events);
    assert_eq!(count(&events, "memoryChanged"), told);
}

/// A long send holds the frames: between the `status` that raises `busy`
/// and the one that clears it no `frame` goes out, and the typed line is
/// shown after; a short one raises no `busy`.
#[test]
fn a_long_send_freezes_the_screen() {
    let Some(rom) = rom() else {
        eprintln!("skipped: SATURNUS_ROM_DIR/sxrom-j not found");
        return;
    };
    let (events, tx) = booted(&rom);
    let mark = events.0.lock().unwrap().len();
    let r = call(
        &tx,
        json!({"cmd": "insert", "text": "« 1 1 100 FOR i i + NEXT » 'S' STO"}),
    )
    .unwrap();
    assert_eq!(r["typed"], 34, "{r}");
    let log: Vec<Value> = events.0.lock().unwrap()[mark..].to_vec();
    let busy_at = log
        .iter()
        .position(|m| m["type"] == "status" && m["busy"] == true)
        .expect("busy raised");
    let free_at = log
        .iter()
        .position(|m| m["type"] == "status" && m["busy"] == false)
        .expect("busy cleared");
    let frames = |a: usize, b: usize| log[a..b].iter().filter(|m| m["type"] == "frame").count();
    assert!(busy_at < free_at);
    assert_eq!(frames(busy_at, free_at), 0, "frames while typing");
    assert!(frames(free_at, log.len()) >= 1, "the typed line is shown");
    eprintln!(
        "  {} events while typing {} keys in {} emulated ms",
        free_at - busy_at - 1,
        r["keys"],
        r["emulatedMs"]
    );
    // A command name is short: no busy, and it closes the line with ENTER.
    let mark = events.0.lock().unwrap().len();
    let r = call(&tx, json!({"cmd": "run", "text": "DROP"})).unwrap();
    assert_eq!(r["closed"], true, "{r}");
    let busy = events.0.lock().unwrap()[mark..]
        .iter()
        .any(|m| m["type"] == "status" && m["busy"] == true);
    assert!(!busy, "a short send does not freeze");
    let cl = call(&tx, json!({"cmd": "commandLine"})).unwrap();
    assert_eq!(cl, json!({"active": false, "text": "", "cursor": 0}));
}
