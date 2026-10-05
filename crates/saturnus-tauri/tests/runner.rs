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
        reply: Some(reply),
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
    let booted = call(
        &tx,
        json!({"cmd": "boot", "model": "49g", "romPath": rom.display().to_string()}),
    )
    .unwrap();
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
        reply: Some(reply),
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
    let saved = call(
        &tx,
        json!({"cmd": "saveState", "path": file.display().to_string()}),
    )
    .unwrap();
    assert_eq!(saved["path"], file.display().to_string());
    let at_save = frame(&events);
    press(&tx, "9");
    wait_for(&events, "a 9 on the command line", |e| frame(e) != at_save);
    call(
        &tx,
        json!({"cmd": "loadState", "path": file.display().to_string()}),
    )
    .unwrap();
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
