//! The fast transfer (XModem) over the traces the transfer page recorded
//! against the emulated 48GX and 49G (`web/test/tx-e2e-xmodem.test.mjs`
//! with `SATURNUS_TX_TRACES`): the page's own requests, read from the
//! trace's notes and parsed as the page sends them, run against a
//! `saturnus-tx` device on a fake clock, the calculator replayed byte for
//! byte. The session: connect, a program put and got by Kermit, got again
//! by XModem (the server ends, the plan says `'PRG' XSEND`), the
//! reconnect, the file put back by XModem as PRG2 (`'PRG2' XRECV`), the
//! reconnect, PRG2 got by Kermit. Also what the device refuses before
//! the server ends.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use saturnus_tx::trace::{Replay, Step, Trace};
use saturnus_tx::{Device, DeviceOptions, Event, Outcome, Request, TxError};
use saturnus_tx_web::{Payload, parse_request};
use satx_core::TransferMode;
use satx_core::time::{Duration, Instant};
use satx_core::xmodem::XmodemDirection;
use serde_json::Value;

/// The device seed the page ran with (`XMODEM_SEED` of the e2e test).
const SEED: u64 = 0x7e57_0039;
/// The program the page put first (the e2e test's `PROGRAM`).
const PROGRAM: &[u8] = b"%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \"fast\" \\>>\r\n";

fn trace(model: &str) -> Trace {
    let text = match model {
        "48gx" => include_str!("../../saturnus-tx/traces/48gx-xmodem.trace"),
        "49g" => include_str!("../../saturnus-tx/traces/49g-xmodem.trace"),
        _ => panic!("no trace for {model}"),
    };
    Trace::parse(text).unwrap()
}

/// The page's requests, in order, from the trace's `request {json}` notes.
fn requests(trace: &Trace) -> Vec<Value> {
    trace
        .steps
        .iter()
        .filter_map(|(_, s)| match s {
            Step::Note(text) => text.strip_prefix("request "),
            _ => None,
        })
        .map(|json| serde_json::from_str(json).unwrap())
        .collect()
}

/// A device on a fake clock against a replayed calculator.
struct Host {
    device: Device,
    replay: Replay,
    clock: Instant,
    events: Vec<Event>,
    /// Ignore the device's actions once the trace is used up.
    deaf: bool,
}

impl Host {
    fn new(trace: &Trace) -> Self {
        let start = Instant::now();
        Host {
            device: Device::new(start, DeviceOptions::default(), SEED),
            replay: Replay::new(trace, start),
            clock: start,
            events: Vec::new(),
            deaf: false,
        }
    }

    /// Bytes the calculator sent between two requests (the restarted
    /// server's idle NAK before the reconnect), fed when they came.
    fn between(&mut self) {
        while let Some(due) = self.replay.next_input() {
            self.clock = self.clock.max(due);
            if let Some(bytes) = self.replay.take_input(self.clock) {
                self.device.handle_input(self.clock, &bytes);
            }
        }
    }

    /// Run `request` to its result: the device's actions must be the
    /// trace's, the clock jumps to the next input or deadline.
    fn run(&mut self, request: Request) -> Result<Outcome, TxError> {
        self.between();
        self.device.start(self.clock, request).unwrap();
        for _ in 0..1_000_000 {
            while let Some(action) = self.device.poll_action() {
                // Past the end of a cut trace the calculator hears nothing.
                if !(self.deaf && self.replay.remaining() == 0) {
                    self.replay.action(self.clock, &action).unwrap();
                }
            }
            while let Some(event) = self.device.poll_event() {
                if !matches!(event, Event::Progress(_)) {
                    self.events.push(event);
                }
            }
            if let Some(result) = self.device.poll_result() {
                return result;
            }
            let next = match (self.replay.next_input(), self.device.next_timeout()) {
                (Some(a), Some(b)) => a.min(b),
                (a, b) => a.or(b).expect("the device waits for nothing"),
            };
            self.clock = self.clock.max(next);
            match self.replay.take_input(self.clock) {
                Some(bytes) => self.device.handle_input(self.clock, &bytes),
                None => self.device.handle_timeout(self.clock),
            }
        }
        panic!("no result");
    }
}

/// The recorded session on `model`: every request's outcome, and the
/// trace used up.
fn session(model: &str) -> (Host, Vec<Outcome>) {
    let trace = trace(model);
    let mut host = Host::new(&trace);
    let mut outcomes = Vec::new();
    let mut got: Option<Vec<u8>> = None;
    for msg in requests(&trace) {
        let kind = msg["type"].as_str().unwrap().to_string();
        let payload = match kind.as_str() {
            "put" => Payload::Bytes(PROGRAM.to_vec()),
            "xmodem-put" => Payload::Bytes(got.clone().expect("the fast get's file")),
            _ => Payload::None,
        };
        let request = parse_request(&msg, payload).unwrap();
        let outcome = host
            .run(request)
            .unwrap_or_else(|e| panic!("{model}: {kind}: {e}"));
        if kind == "xmodem-get"
            && let Outcome::File(file) = &outcome
        {
            got = Some(file.data.clone());
        }
        outcomes.push(outcome);
    }
    assert_eq!(host.replay.remaining(), 0, "{model}: the trace is used up");
    (host, outcomes)
}

fn files(outcomes: &[Outcome]) -> Vec<(String, Vec<u8>)> {
    outcomes
        .iter()
        .filter_map(|o| match o {
            Outcome::File(f) => Some((f.name.clone(), f.data.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn the_fast_transfer_replays_on_the_48gx_and_49g() {
    for (model, header) in [("48gx", &b"HPHP48-"[..]), ("49g", &b"HPHP49-"[..])] {
        let (host, outcomes) = session(model);
        // Kermit's PRG, XModem's PRG, Kermit's PRG2: the same file.
        let files = files(&outcomes);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["PRG", "PRG", "PRG2"], "{model}");
        assert!(files[0].1.starts_with(header), "{model}");
        assert_eq!(files[1].1, files[0].1, "{model}: XSEND, padding cut");
        assert_eq!(files[2].1, files[0].1, "{model}: XRECV stored the same");
        assert!(
            outcomes
                .iter()
                .any(|o| matches!(o, Outcome::Stored(n) if n == "PRG2")),
            "{model}"
        );
        // The plans: what the user typed, and the 49G switched to RPN.
        let plans: Vec<_> = host
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Xmodem(p) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(plans.len(), 2, "{model}");
        assert_eq!(plans[0].keys, "'PRG' XSEND");
        assert_eq!(plans[0].direction, XmodemDirection::FromCalculator);
        assert_eq!(plans[1].keys, "'PRG2' XRECV");
        assert_eq!(plans[1].switched_to_rpn, model == "49g", "{model}");
        // The user's transfer mode from the first connect, kept through both.
        let modes: Vec<TransferMode> = outcomes
            .iter()
            .filter_map(|o| match o {
                Outcome::Connected(c) => Some(c.user_mode),
                _ => None,
            })
            .collect();
        assert_eq!(modes.len(), 3, "{model}");
        assert!(modes.iter().all(|m| *m == modes[0]), "{model}: {modes:?}");
        assert!(!host.device.left_server());
    }
}

/// The session up to the fast get, then that request's own steps.
#[test]
fn after_the_fast_transfer_only_a_connect() {
    let trace = trace("48gx");
    let mut host = Host::new(&trace);
    let reqs = requests(&trace);
    let at = reqs.iter().position(|r| r["type"] == "xmodem-get").unwrap();
    for msg in &reqs[..at] {
        let payload = if msg["type"] == "put" {
            Payload::Bytes(PROGRAM.to_vec())
        } else {
            Payload::None
        };
        host.run(parse_request(msg, payload).unwrap()).unwrap();
    }
    host.run(parse_request(&reqs[at], Payload::None).unwrap())
        .unwrap();
    assert!(host.device.left_server());
    assert!(host.device.connection().is_none());
    let list = Request::List {
        dir: vec!["HOME".into()],
    };
    assert!(matches!(host.run(list), Err(TxError::LeftServer)));
}

/// Refused while the server still runs: nothing of the fast transfer is
/// sent, and the device stays connected.
#[test]
fn refusals_before_the_server_ends() {
    let trace = trace("48gx");
    let mut host = Host::new(&trace);
    let reqs = requests(&trace);
    // Connect only.
    host.run(parse_request(&reqs[0], Payload::None).unwrap())
        .unwrap();
    let home = || vec!["HOME".to_string()];
    let wait = Duration::from_secs(1);
    // A text file, a 49G file, a bad name: refused before any byte.
    for (data, name) in [
        (PROGRAM.to_vec(), "A"),
        (b"HPHP49-C\x2c\x2a\x20\x00\x00".to_vec(), "A"),
        (b"HPHP48-R\x2c\x2a\x20\x00\x00".to_vec(), "1A"),
    ] {
        let r = host.run(Request::XmodemPut {
            dir: home(),
            name: name.into(),
            data,
            wait,
        });
        assert!(
            matches!(r, Err(TxError::NotForXmodem(_) | TxError::BadName(_))),
            "{r:?}"
        );
    }
    assert!(host.device.connection().is_some());
    assert!(!host.device.left_server());
}

/// The 48S/SX has no fast transfer.
#[test]
fn no_fast_transfer_on_the_48sx() {
    assert!(!saturnus_tx::xmodem::has_xmodem(saturnus_tx::Model::Hp48Sx));
    assert!(matches!(
        saturnus_tx::xmodem::options(saturnus_tx::Model::Hp48Sx, Duration::from_secs(1)),
        Err(TxError::NotForXmodem(_))
    ));
}

/// The 49G's HOME holds CASDIR, a directory: a fast get of it is refused
/// before the server ends (a directory goes by the normal copy).
#[test]
fn a_directory_is_not_got_fast() {
    let trace = trace("49g");
    let mut host = Host::new(&trace);
    let reqs = requests(&trace);
    host.run(parse_request(&reqs[0], Payload::None).unwrap())
        .unwrap();
    let r = host.run(Request::XmodemGet {
        dir: vec!["HOME".into()],
        name: "CASDIR".into(),
        wait: Duration::from_secs(1),
    });
    assert!(
        matches!(r, Err(TxError::IsDirectory(ref n)) if n == "CASDIR"),
        "{r:?}"
    );
    assert!(host.device.connection().is_some());
    assert!(!host.device.left_server());
}

/// FINISH sent but its answer lost: the server may have ended, so the
/// device counts as out of server mode (the reconnect then puts the 49G's
/// algebraic mode back) instead of keeping a session that no longer
/// answers.
#[test]
fn a_lost_finish_answer_still_counts_as_left() {
    let full = trace("49g");
    let reqs = requests(&full);
    let at = reqs.iter().position(|r| r["type"] == "xmodem-get").unwrap();
    // The trace up to the fast get's FINISH, its answer cut.
    let mut cut = Trace::new();
    let mut gets = 0;
    for (t, step) in &full.steps {
        cut.push(*t, step.clone());
        if let Step::Note(n) = step
            && n.contains("\"xmodem-get\"")
        {
            gets += 1;
        }
        if gets == 1 && matches!(step, Step::Out(b) if b.windows(2).any(|w| w == b"GF")) {
            break;
        }
    }
    let mut host = Host::new(&cut);
    host.deaf = true;
    for msg in &reqs[..at] {
        let payload = if msg["type"] == "put" {
            Payload::Bytes(PROGRAM.to_vec())
        } else {
            Payload::None
        };
        host.run(parse_request(msg, payload).unwrap()).unwrap();
    }
    let r = host.run(parse_request(&reqs[at], Payload::None).unwrap());
    assert!(r.is_err(), "{r:?}");
    assert!(host.device.left_server());
    assert!(host.device.connection().is_none());
    let list = Request::List {
        dir: vec!["HOME".into()],
    };
    assert!(matches!(host.run(list), Err(TxError::LeftServer)));
}
