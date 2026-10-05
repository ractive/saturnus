//! The control API against real ROMs, as separate processes: `saturnus
//! run` and `saturnus ctl`. Gated by `SATURNUS_ROM_DIR` (the test policy);
//! without it every test is skipped.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use common::{Instance, png_size};

fn rom(name: &str) -> Option<PathBuf> {
    let Some(dir) = std::env::var_os("SATURNUS_ROM_DIR") else {
        eprintln!("SATURNUS_ROM_DIR not set: skipping ROM test ({name})");
        return None;
    };
    Some(PathBuf::from(dir).join(name))
}

/// A golden screen of the core's e2e tests (131x64 text).
fn golden(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../saturnus/tests/golden")
        .join(format!("{name}.txt"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Boot a 48SX through `ctl`: the "Try To Recover Memory?" prompt, NO,
/// the empty stack; 2 ENTER 3 + gives 5; typed text arrives; a snapshot
/// taken and put back restores the screen.
#[test]
fn hp48sx_keys_screen_type_and_snapshot() {
    let Some(rom) = rom("sxrom-j") else { return };
    let run = Instance::start("48sx", &rom, &["--no-serial"]);
    run.ctl_ok(&["keys", "wait-idle 60000"]);
    assert_eq!(
        run.ctl_ok(&["screen"]),
        golden("48sx-try-to-recover-memory")
    );
    run.ctl_ok(&["keys", "f"]);
    assert_eq!(run.ctl_ok(&["screen"]), golden("48sx-memory-clear"));

    run.ctl_ok(&["keys", "2 ENTER 3 +"]);
    let stack: serde_json::Value = serde_json::from_str(&run.ctl_ok(&["stack"])).unwrap();
    assert_eq!(stack[0]["value"], 5.0, "{stack}");

    let state = run.dir.0.join("five.state");
    run.ctl_ok(&["snapshot", "get", state.to_str().unwrap()]);
    let five = run.ctl_ok(&["screen"]);

    run.ctl_ok(&["type", "ab"]);
    run.ctl_ok(&["keys", "enter"]);
    let stack: serde_json::Value = serde_json::from_str(&run.ctl_ok(&["stack"])).unwrap();
    assert_eq!(stack[0]["value"], "ab", "{stack}");
    assert_ne!(run.ctl_ok(&["screen"]), five);

    run.ctl_ok(&["snapshot", "put", state.to_str().unwrap()]);
    assert_eq!(run.ctl_ok(&["screen"]), five);

    let png = run.dir.0.join("five.png");
    run.ctl_ok(&["screen", "--png", png.to_str().unwrap()]);
    assert_eq!(png_size(&std::fs::read(&png).unwrap()), (131, 64));
    assert!(run.stop());
}

/// A Kermit packet with block check type 1 (wiki: protocols/kermit).
fn kermit_packet(seq: u8, kind: u8, data: &[u8]) -> Vec<u8> {
    let tochar = |x: u8| x + 32;
    let len = u8::try_from(data.len() + 3).unwrap();
    let mut p = vec![1, tochar(len), tochar(seq), kind];
    p.extend_from_slice(data);
    let sum: u32 = p[1..].iter().map(|&b| u32::from(b)).sum();
    p.push(tochar(((sum + ((sum & 192) / 64)) & 63) as u8));
    p.push(b'\r');
    p
}

/// The serial port answers while the control API is polled: the ROM's
/// Kermit server (started by `--autostart`) acknowledges a server init
/// while another thread reads the screen ten times a second.
#[test]
fn hp48sx_serial_answers_while_the_api_is_used() {
    let Some(rom) = rom("sxrom-j") else { return };
    let run = Arc::new(Instance::start(
        "48sx",
        &rom,
        &["--serial", "tcp:127.0.0.1:0", "--autostart"],
    ));
    let stop = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));
    let poller = {
        let (run, stop, polls) = (Arc::clone(&run), Arc::clone(&stop), Arc::clone(&polls));
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                run.ctl_ok(&["screen"]);
                polls.fetch_add(1, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(100));
            }
        })
    };
    std::thread::sleep(Duration::from_millis(500));
    let mut s = TcpStream::connect(("127.0.0.1", run.serial.unwrap())).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(&kermit_packet(0, b'I', b"~* @-#Y1")).unwrap();
    let mut reply = Vec::new();
    let mut b = [0u8; 64];
    while !reply.ends_with(b"\r") {
        let n = s.read(&mut b).unwrap();
        assert!(n > 0, "serial closed");
        reply.extend_from_slice(&b[..n]);
    }
    stop.store(true, Ordering::Relaxed);
    poller.join().unwrap();
    assert_eq!(reply.get(3), Some(&b'Y'), "{reply:?}");
    assert!(polls.load(Ordering::Relaxed) >= 3);
    drop(s);
    let run = Arc::into_inner(run).unwrap();
    assert!(run.stop());
}

/// The 42S (no serial port) serves the control API with `--serve`; its
/// display is 131x16, and so is its PNG; `--screen` and `--save` are
/// written when it stops.
#[test]
fn hp42s_serves_and_writes_outputs_on_stop() {
    let Some(rom) = rom("hp42s-c.rom") else {
        return;
    };
    let dir = common::TempDir::new("42s-out");
    let screen = dir.0.join("final.txt");
    let state = dir.0.join("final.state");
    let run = Instance::start(
        "42s",
        &rom,
        &[
            "--serve",
            "--screen",
            screen.to_str().unwrap(),
            "--save",
            state.to_str().unwrap(),
        ],
    );
    assert!(run.serial.is_none(), "the 42S has no serial port");
    run.ctl_ok(&["keys", "wait-idle 5000"]);
    let png = run.dir.0.join("42s.png");
    run.ctl_ok(&["screen", "--png", png.to_str().unwrap()]);
    assert_eq!(png_size(&std::fs::read(&png).unwrap()), (131, 16));
    let shown = run.ctl_ok(&["screen"]);
    assert_eq!(shown.lines().count(), 16);
    let stopped = run.stop();
    if cfg!(unix) {
        assert!(stopped);
        assert_eq!(std::fs::read_to_string(&screen).unwrap(), shown);
        assert!(std::fs::metadata(&state).unwrap().len() > 0);
    }
}
