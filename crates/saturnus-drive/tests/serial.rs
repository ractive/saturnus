//! The serial port through the native hosts' machine thread (the Tauri
//! app's and `saturnus run`'s runner), on the real ROMs (skipped unless
//! `SATURNUS_ROM_DIR` holds `gxrom-r`, `rom-2.10.49g`), on the wall
//! clock: the page takes the port (`serialOpen`), types SERVER, hears the
//! server's NAKs as `serial` events (base64, as every JSON host sends
//! them), writes a Kermit Send-Init (`serialWrite`, base64) and gets the
//! server's ACK; a write (a hidden transaction) is refused meanwhile, and
//! runs again once FINISH has ended the server and the port is closed.
//! The same through the state machine: `saturnus-host`'s
//! `tests/serial.rs`; in the Worker: `web/test/serial-rom.test.mjs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use saturnus_core::Machine;
use saturnus_drive::runner::{Hook, Request, Runner, Service, Sink, StateDir};
use saturnus_host::host::{base64, base64_decode};
use serde_json::{Value, json};

const MODELS: [(&str, &str); 2] = [("48gx", "gxrom-r"), ("49g", "rom-2.10.49g")];

#[derive(Clone, Default)]
struct Collect(Arc<Mutex<Vec<u8>>>);

impl Sink for Collect {
    fn event(&self, msg: Value) {
        if msg["type"] == "serial" {
            let bytes = base64_decode(msg["bytes"].as_str().unwrap()).unwrap();
            assert!(!bytes.is_empty(), "an empty serial event");
            self.0.lock().unwrap().extend(bytes);
        }
    }
}

fn call(tx: &Sender<Request>, file: Option<PathBuf>, mut msg: Value) -> Result<Value, String> {
    msg["v"] = json!(1);
    let (reply, answer) = channel();
    tx.send(Request {
        msg,
        file,
        reply: Some(reply),
        ticket: None,
    })
    .unwrap();
    answer.recv_timeout(Duration::from_secs(120)).unwrap()
}

/// A Kermit packet with a type-1 check, ended by CR.
fn packet(seq: u8, kind: u8, data: &[u8]) -> Vec<u8> {
    let tochar = |n: usize| (n + 32) as u8;
    let mut body = vec![tochar(3 + data.len()), tochar(usize::from(seq)), kind];
    body.extend_from_slice(data);
    let s: usize = body.iter().map(|&b| usize::from(b)).sum();
    let mut p = vec![1];
    p.extend(body);
    p.extend([tochar((s + ((s & 192) >> 6)) & 63), 13]);
    p
}

/// The packets in `bytes`: `(seq, kind, data)`.
fn packets(bytes: &[u8]) -> Vec<(u8, u8, Vec<u8>)> {
    let mut out = Vec::new();
    for (i, _) in bytes.iter().enumerate().filter(|(_, b)| **b == 1) {
        let Some(len) = bytes.get(i + 1).map(|&l| usize::from(l.wrapping_sub(32))) else {
            continue;
        };
        if len < 3 || i + 2 + len > bytes.len() {
            continue;
        }
        out.push((
            bytes[i + 2] - 32,
            bytes[i + 3],
            bytes[i + 4..i + 1 + len].to_vec(),
        ));
    }
    out
}

/// Wait until the calculator sent a packet of `kind` (after `from` bytes).
fn heard(events: &Collect, from: usize, kind: u8) -> (u8, u8, Vec<u8>) {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let all = events.0.lock().unwrap().clone();
        if let Some(p) = packets(&all[from.min(all.len())..])
            .into_iter()
            .find(|p| p.1 == kind)
        {
            return p;
        }
        assert!(
            Instant::now() < end,
            "no {:?} packet: {:?}",
            kind as char,
            packets(&all)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn kermit_through_the_page_s_serial_port_on_the_machine_thread() {
    for (model, file) in MODELS {
        let Some(rom) = std::env::var_os("SATURNUS_ROM_DIR")
            .map(|d| PathBuf::from(d).join(file))
            .filter(|p| p.exists())
        else {
            eprintln!("skipped: SATURNUS_ROM_DIR/{file} not found");
            continue;
        };
        let dir =
            std::env::temp_dir().join(format!("saturnus-serial-{}-{model}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let events = Collect::default();
        let (tx, rx) = channel();
        let sink = events.clone();
        let states = dir.clone();
        let thread = std::thread::spawn(move || {
            let mut r = Runner::for_host(sink, "tauri");
            // A host that keeps the calculator answers "Try To Recover Memory?".
            r.set_store(Box::new(StateDir(states)));
            r.run(&rx);
        });
        call(&tx, Some(rom), json!({"cmd": "boot", "model": model})).unwrap();
        // Up: the memory reads, and the calculator waits for a key (the
        // boot's question answered).
        let ready = Instant::now() + Duration::from_secs(90);
        let mut idle = 0;
        while idle < 5 {
            let up = call(&tx, None, json!({"cmd": "stack"})).is_ok()
                && call(&tx, None, json!({"cmd": "stats"})).unwrap()["loop"] == "sleep";
            idle = if up { idle + 1 } else { 0 };
            assert!(
                Instant::now() < ready,
                "{model}: the calculator never came up"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        call(&tx, None, json!({"cmd": "setSpeed", "speed": "max"})).unwrap();

        assert_eq!(
            call(&tx, None, json!({"cmd": "serialOpen"})),
            Ok(Value::Null)
        );
        let r = call(&tx, None, json!({"cmd": "run", "text": "SERVER"})).unwrap();
        assert_eq!(r["error"], Value::Null, "{model}: {r}");
        assert_eq!(
            call(
                &tx,
                None,
                json!({"cmd": "createDir", "dir": ["HOME"], "name": "SERA"})
            ),
            Err("the serial port is in use (serialClose frees it)".to_string()),
            "{model}"
        );
        heard(&events, 0, b'N');
        // Send-Init with the defaults: MAXL 80, TIME 5, no padding, CR,
        // #, no 8th-bit or repeat prefix, check 1.
        let init = b"p% @-# 1 ";
        let from = events.0.lock().unwrap().len();
        let write = json!({"cmd": "serialWrite", "bytes": base64(&packet(0, b'I', init))});
        assert_eq!(call(&tx, None, write), Ok(Value::Null), "{model}");
        let (seq, _, data) = heard(&events, from, b'Y');
        assert_eq!(seq, 0, "{model}");
        assert!(data[0] - 32 >= 30, "{model}: the server's MAXL");

        let from = events.0.lock().unwrap().len();
        let finish = json!({"cmd": "serialWrite", "bytes": base64(&packet(0, b'G', b"F"))});
        assert_eq!(call(&tx, None, finish), Ok(Value::Null), "{model}");
        assert_eq!(
            heard(&events, from, b'Y').0,
            0,
            "{model}: FINISH acknowledged"
        );
        assert_eq!(
            call(&tx, None, json!({"cmd": "serialClose"})),
            Ok(Value::Null)
        );
        std::thread::sleep(Duration::from_secs(2));
        call(
            &tx,
            None,
            json!({"cmd": "createDir", "dir": ["HOME"], "name": "SERA"}),
        )
        .unwrap_or_else(|e| panic!("{model}: the write after: {e}"));
        drop(tx);
        thread.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A serial bridge whose peer swallows whatever the calculator sends:
/// each turn with the machine drains the UART. Counts the turns without
/// one once the machine is up (before the boot there is none).
struct Greedy {
    up: bool,
    without: Arc<Mutex<usize>>,
}

impl Hook for Greedy {
    fn service(&mut self, machine: Option<&mut Machine>) -> Service {
        match machine {
            Some(m) => {
                self.up = true;
                m.serial_drain();
            }
            None if self.up => *self.without.lock().unwrap() += 1,
            None => {}
        }
        Service::Idle
    }
    fn interval(&self) -> Duration {
        Duration::from_millis(1)
    }
}

/// A hidden Kermit write (`/v1/memory`) owns the UART: the host's bridge
/// (the CLI's `--serial`) gets no machine while it runs, so it can neither
/// steal the server's packets nor push its peer's bytes in between.
#[test]
fn a_hidden_write_keeps_the_uart_from_the_serial_bridge() {
    for (model, file) in MODELS {
        let Some(rom) = std::env::var_os("SATURNUS_ROM_DIR")
            .map(|d| PathBuf::from(d).join(file))
            .filter(|p| p.exists())
        else {
            eprintln!("skipped: SATURNUS_ROM_DIR/{file} not found");
            continue;
        };
        let dir =
            std::env::temp_dir().join(format!("saturnus-greedy-{}-{model}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (tx, rx) = channel();
        let states = dir.clone();
        let without = Arc::new(Mutex::new(0));
        let hook = Greedy {
            up: false,
            without: Arc::clone(&without),
        };
        let thread = std::thread::spawn(move || {
            let mut r = Runner::for_host(Collect::default(), "http");
            r.set_store(Box::new(StateDir(states)));
            r.set_hook(Box::new(hook));
            r.run(&rx);
        });
        call(&tx, Some(rom), json!({"cmd": "boot", "model": model})).unwrap();
        let ready = Instant::now() + Duration::from_secs(90);
        let mut idle = 0;
        while idle < 5 {
            let up = call(&tx, None, json!({"cmd": "stack"})).is_ok()
                && call(&tx, None, json!({"cmd": "stats"})).unwrap()["loop"] == "sleep";
            idle = if up { idle + 1 } else { 0 };
            assert!(
                Instant::now() < ready,
                "{model}: the calculator never came up"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        call(&tx, None, json!({"cmd": "setSpeed", "speed": "max"})).unwrap();
        call(
            &tx,
            None,
            json!({"cmd": "createDir", "dir": ["HOME"], "name": "GRDY"}),
        )
        .unwrap_or_else(|e| panic!("{model}: the write beside a greedy bridge: {e}"));
        assert!(
            *without.lock().unwrap() > 0,
            "{model}: the hook was left out while the write ran"
        );
        drop(tx);
        thread.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
