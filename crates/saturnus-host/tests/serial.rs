//! The page holds the emulated serial port (`serialOpen`, `serialWrite`,
//! the `serial` event) and talks Kermit to the ROM's own server, as the
//! transfer page does over a cable, on the real ROMs (skipped unless
//! `SATURNUS_ROM_DIR` holds `sxrom-j`, `gxrom-r`, `rom-2.10.49g`),
//! through the protocol's state machine on a clock moved by hand: a write
//! that runs keeps the port from the page; `SERVER` typed, its NAKs come
//! as `serial` events, a Send-Init written gets the server's ACK with its
//! parameters, the writes are refused while the page holds the port, a
//! FINISH ends the server, and once the port is closed a write runs
//! again.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::path::PathBuf;

use kermit_proto::codec::{BlockCheck, Deframer, Framing, Packet, parse_frame};
use kermit_proto::params::InitParams;
use saturnus_host::host::base64;
use saturnus_host::protocol::{Clock, Engine, Event, Output, Pacing, SERIAL_IN_USE};
use serde_json::{Value, json};

const MODELS: [(&str, &str); 3] = [
    ("48sx", "sxrom-j"),
    ("48gx", "gxrom-r"),
    ("49g", "rom-2.10.49g"),
];

fn rom(file: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("SATURNUS_ROM_DIR")?;
    std::fs::read(PathBuf::from(dir).join(file)).ok()
}

#[derive(Default)]
struct Manual(Cell<f64>);

impl Clock for Manual {
    fn now_ms(&self) -> f64 {
        self.0.get()
    }
}

struct Host {
    engine: Engine,
    clock: Manual,
    tag: u64,
    /// The packets the calculator sent on the page's port, in order.
    packets: Vec<Packet>,
    deframer: Deframer,
}

impl Host {
    fn now(&self) -> f64 {
        self.clock.now_ms()
    }

    /// The outputs so far: the `serial` events' packets kept, the reply
    /// to `tag` returned.
    fn take(&mut self, tag: Option<u64>) -> Option<Result<Value, String>> {
        let mut reply = None;
        for out in self.engine.take_output() {
            match out {
                Output::Reply(r) if Some(r.tag) == tag => reply = Some(r.result),
                Output::Event(Event::Serial(s)) => {
                    assert!(!s.bytes.is_empty(), "an empty serial event");
                    self.deframer.push(&s.bytes);
                    while let Some(f) = self.deframer.next_frame() {
                        self.packets
                            .push(parse_frame(&f, BlockCheck::Type1).unwrap());
                    }
                }
                _ => {}
            }
        }
        reply
    }

    fn fire(&mut self, until: f64) -> bool {
        match self.engine.deadline() {
            Some(due) if due <= until => {
                self.clock.0.set(self.now().max(due));
                self.engine.timer(&self.clock);
                true
            }
            _ => false,
        }
    }

    fn run_until(&mut self, until: f64) {
        while self.fire(until) {
            self.take(None);
        }
        self.clock.0.set(self.now().max(until));
        self.take(None);
    }

    /// Send `msg`; its tag.
    fn send(&mut self, mut msg: Value) -> u64 {
        msg["v"] = json!(1);
        self.tag += 1;
        self.engine.command(&self.clock, &msg, None, Some(self.tag));
        self.tag
    }

    fn wait(&mut self, tag: u64) -> Result<Value, String> {
        let cap = self.now() + 120_000.0;
        loop {
            if let Some(r) = self.take(Some(tag)) {
                return r;
            }
            assert!(self.fire(cap), "no reply to {tag}");
        }
    }

    fn call(&mut self, msg: Value) -> Result<Value, String> {
        let tag = self.send(msg);
        self.wait(tag)
    }

    /// Run until the calculator sent a packet of `kind`, at most `ms` of
    /// the clock; that packet.
    fn packet(&mut self, kind: u8, ms: f64) -> Packet {
        let end = self.now() + ms;
        loop {
            if let Some(p) = self.packets.iter().find(|p| p.kind == kind) {
                return p.clone();
            }
            assert!(
                self.now() < end,
                "no {:?} packet in {ms} ms; got {:?}",
                kind as char,
                self.packets
            );
            let next = self.now() + 50.0;
            self.run_until(next);
        }
    }
}

fn frame(seq: u8, kind: u8, data: &[u8]) -> String {
    let bytes = Packet::new(seq, kind, data.to_vec())
        .encode(BlockCheck::Type1, &Framing::default())
        .unwrap();
    base64(&bytes)
}

#[test]
fn kermit_through_the_page_s_serial_port() {
    for (model, file) in MODELS {
        let Some(rom) = rom(file) else {
            eprintln!("skipped: SATURNUS_ROM_DIR/{file} not found");
            continue;
        };
        let mut engine = Engine::new("test", Pacing::NATIVE);
        engine.set_answer_recover(true);
        let mut h = Host {
            engine,
            clock: Manual::default(),
            tag: 0,
            packets: Vec::new(),
            deframer: Deframer::new(),
        };
        h.engine.boot(&h.clock, model, &rom, "rom").unwrap();
        let mut t = 0.0;
        while h.engine.answering_recover() && t < 60_000.0 {
            t += 500.0;
            h.run_until(t);
        }
        h.run_until(t + 3_000.0);

        // A write that runs keeps the port from the page.
        let write = h.send(json!({"cmd": "createDir", "dir": ["HOME"], "name": "SERA"}));
        for cmd in ["serialOpen", "serialWrite"] {
            assert_eq!(
                h.call(json!({"cmd": cmd, "bytes": "AQ=="})).unwrap_err(),
                "a transfer is in progress (releaseAll stops it)",
                "{model}: {cmd}"
            );
        }
        h.wait(write)
            .unwrap_or_else(|e| panic!("{model}: the write: {e}"));

        // The page takes the port, then starts the server by typing.
        assert_eq!(h.call(json!({"cmd": "serialOpen"})), Ok(Value::Null));
        let r = h.call(json!({"cmd": "run", "text": "SERVER"})).unwrap();
        assert_eq!(r["error"], Value::Null, "{model}: {r}");
        assert_eq!(
            h.call(json!({"cmd": "purge", "dir": ["HOME"], "name": "SERA"})),
            Err(SERIAL_IN_USE.to_string()),
            "{model}"
        );
        // The waiting server NAKs; a Send-Init gets its ACK.
        h.packet(b'N', 30_000.0);
        let init = InitParams::default().encode();
        let sent = h.call(json!({"cmd": "serialWrite", "bytes": frame(0, b'I', &init)}));
        assert_eq!(sent, Ok(Value::Null), "{model}");
        let ack = h.packet(b'Y', 10_000.0);
        assert_eq!(ack.seq, 0, "{model}");
        let theirs = InitParams::decode(&ack.data);
        assert!(theirs.maxl >= 30, "{model}: {theirs:?}");
        eprintln!("{model}: the server's ACK to I: {theirs:?}");

        // FINISH (a transaction of its own, from 0) ends the server; the
        // page lets go; a write runs again.
        h.packets.clear();
        let sent = h.call(json!({"cmd": "serialWrite", "bytes": frame(0, b'G', b"F")}));
        assert_eq!(sent, Ok(Value::Null), "{model}");
        let ack = h.packet(b'Y', 10_000.0);
        assert_eq!(ack.seq, 0, "{model}");
        assert_eq!(h.call(json!({"cmd": "serialClose"})), Ok(Value::Null));
        let now = h.now();
        h.run_until(now + 5_000.0);
        h.call(json!({"cmd": "purge", "dir": ["HOME"], "name": "SERA"}))
            .unwrap_or_else(|e| panic!("{model}: the write after: {e}"));
        let tree = h.call(json!({"cmd": "memoryTree"})).unwrap();
        assert!(
            !tree.to_string().contains("SERA"),
            "{model}: purged: {tree}"
        );
    }
}
