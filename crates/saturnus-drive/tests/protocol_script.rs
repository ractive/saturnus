//! The cross-host test: `web/test/protocol-script.json` through the
//! protocol's state machine with a fake clock and through the native
//! runner (the Tauri app's and `saturnus run`'s driver) on a real thread;
//! both must give `web/test/protocol-script.expected.json`, as the Web
//! Worker does with the real wasm (`web/test/protocol-script.test.mjs`).
//! The runner's events reach its sink and its replies their callers apart,
//! so for it the events are compared in order and the replies by step.
//! `SATURNUS_BLESS=1` writes the expected transcript from the state
//! machine's run.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use saturnus_drive::runner::{Request, Runner, Sink};
use saturnus_host::host::{base64, base64_decode};
use saturnus_host::protocol::{Clock, Engine, Output, Pacing};
use serde_json::{Value, json};

const ZEROS: usize = 256 * 1024;

fn web_test(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/test")
        .join(name)
}

fn steps() -> Vec<Value> {
    let script: Value =
        serde_json::from_str(&std::fs::read_to_string(web_test("protocol-script.json")).unwrap())
            .unwrap();
    script["steps"].as_array().unwrap().clone()
}

/// The step's message with `v: 1` unless it has a `v`.
fn message(step: &Value) -> Value {
    let mut msg = step["msg"].clone();
    if msg.get("v").is_none() {
        msg["v"] = json!(1);
    }
    msg
}

/// A reply as the transcript has it, with the step's `any` fields as `*`.
fn reply(steps: &[Value], i: usize, result: Result<Value, String>) -> Value {
    match result {
        Ok(mut v) => {
            for f in steps[i]["any"].as_array().into_iter().flatten() {
                let f = f.as_str().unwrap();
                if v.get(f).is_some() {
                    v[f] = json!("*");
                }
            }
            json!({"reply": i, "ok": true, "result": v})
        }
        Err(e) => json!({"reply": i, "ok": false, "error": e}),
    }
}

fn expected() -> Vec<Value> {
    let path = web_test("protocol-script.expected.json");
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn check(name: &str, transcript: &[Value], expected: &[Value]) {
    for (i, (a, b)) in transcript.iter().zip(expected).enumerate() {
        assert_eq!(a, b, "{name}: entry {i}");
    }
    assert_eq!(transcript.len(), expected.len(), "{name}: {transcript:#?}");
}

/// The events of a transcript, and its replies by step.
fn split(transcript: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let (mut replies, events): (Vec<Value>, Vec<Value>) = transcript
        .iter()
        .cloned()
        .partition(|e| e.get("reply").is_some());
    replies.sort_by_key(|r| r["reply"].as_u64());
    (events, replies)
}

/// A clock that moves 0.25 ms each time it is read.
struct Fake(Cell<f64>);

impl Clock for Fake {
    fn now_ms(&self) -> f64 {
        let t = self.0.get();
        self.0.set(t + 0.25);
        t
    }
}

#[test]
fn the_state_machine_with_a_fake_clock() {
    let steps = steps();
    let mut engine = Engine::new("test", Pacing::WORKER);
    let clock = Fake(Cell::new(0.0));
    let mut transcript = Vec::new();
    let mut saved = None;
    for (i, step) in steps.iter().enumerate() {
        let mut msg = message(step);
        let mut bytes = None;
        if step["rom"] == "zeros" {
            bytes = Some(vec![0u8; ZEROS]);
            msg["romName"] = json!("zeros");
        }
        if step["state"] == "saved" {
            bytes = saved.clone();
        }
        let tag = (step["noId"] != true).then_some(i as u64);
        engine.command(&clock, &msg, bytes, tag);
        // A few turns of the host's timer between the commands (none
        // after a piped one).
        let turns = if step["pipe"] == true { 1 } else { 3 };
        for turn in 0..turns {
            if let Some(due) = engine.deadline() {
                while clock.0.get() < due {
                    clock.0.set(due);
                }
            }
            for out in engine.take_output() {
                match out {
                    Output::Event(e) => transcript.push(serde_json::to_value(e).unwrap()),
                    Output::Reply(r) => {
                        let mut result = r.result;
                        if let Some((field, b)) = r.bytes {
                            if let Ok(v) = &mut result {
                                v[field] = json!(base64(&b));
                            }
                            saved = Some(b);
                        }
                        transcript.push(reply(&steps, r.tag as usize, result));
                    }
                    // Auto-save is off: this engine keeps no states.
                    Output::Save(_) => {}
                }
            }
            if turn + 1 < turns {
                engine.timer(&clock);
            }
        }
    }
    for out in engine.take_output() {
        if let Output::Event(e) = out {
            transcript.push(serde_json::to_value(e).unwrap());
        }
    }
    if std::env::var_os("SATURNUS_BLESS").is_some() {
        let text = serde_json::to_string_pretty(&transcript).unwrap();
        std::fs::write(web_test("protocol-script.expected.json"), text + "\n").unwrap();
    }
    check("state machine", &transcript, &expected());
}

#[derive(Clone, Default)]
struct Collect(Arc<Mutex<Vec<Value>>>);

impl Sink for Collect {
    fn event(&self, msg: Value) {
        self.0.lock().unwrap().push(msg);
    }
}

impl Collect {
    fn drain(&self) -> Vec<Value> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[test]
fn the_native_runner() {
    let steps = steps();
    let dir = std::env::temp_dir().join(format!("saturnus-script-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rom = dir.join("zeros");
    std::fs::write(&rom, vec![0u8; ZEROS]).unwrap();
    let events = Collect::default();
    let (tx, rx): (Sender<Request>, Receiver<Request>) = channel();
    let sink = events.clone();
    let thread = std::thread::spawn(move || {
        Runner::for_host(sink, "http").run(&rx);
    });
    let mut transcript = Vec::new();
    let mut piped: Vec<(usize, Receiver<Result<Value, String>>)> = Vec::new();
    let mut saved: Option<Vec<u8>> = None;
    for (i, step) in steps.iter().enumerate() {
        let mut msg = message(step);
        if step["state"] == "saved" {
            msg["state"] = json!(base64(saved.as_ref().unwrap()));
        }
        let (reply_tx, answer) = channel();
        let with_id = step["noId"] != true;
        tx.send(Request {
            msg,
            file: (step["rom"] == "zeros").then(|| rom.clone()),
            reply: with_id.then_some(reply_tx),
            ticket: None,
        })
        .unwrap();
        if !with_id {
            continue;
        }
        if step["pipe"] == true {
            piped.push((i, answer));
            continue;
        }
        let result = answer.recv_timeout(Duration::from_secs(30)).unwrap();
        if let Ok(v) = &result
            && let Some(s) = v.get("state").and_then(Value::as_str)
        {
            saved = Some(base64_decode(s).unwrap());
        }
        transcript.extend(events.drain());
        for (j, a) in piped.drain(..) {
            let r = a.recv_timeout(Duration::from_secs(30)).unwrap();
            transcript.push(reply(&steps, j, r));
        }
        transcript.push(reply(&steps, i, result));
    }
    drop(tx);
    thread.join().unwrap();
    transcript.extend(events.drain());
    let _ = std::fs::remove_dir_all(&dir);
    let (events, replies) = split(&transcript);
    let (want_events, want_replies) = split(&expected());
    check("native runner's events", &events, &want_events);
    check("native runner's replies", &replies, &want_replies);
}
