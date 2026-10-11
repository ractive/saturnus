//! The device over the traces recorded from saturnus (`just tx-traces`),
//! on a fake clock: the whole session on each model, and the failures made
//! from them (no answer, the wrong speed, the wrong parity, a NAK storm, a
//! cut link, names refused or in use).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod scenario;

use std::collections::VecDeque;
use std::io;

use kermit_proto::codec::{BlockCheck, Deframer, Framing, Packet, parse_frame};
use saturnus_tx::trace::{Replay, Step, Trace};
use saturnus_tx::{
    Action, Device, DeviceOptions, Event, LinkSettings, Outcome, Parity, Request, TxError,
};
use satx_core::time::{Duration, Instant};

fn trace(model: &str) -> Trace {
    let text = match model {
        "48sx" => include_str!("../traces/48sx-session.trace"),
        "48gx" => include_str!("../traces/48gx-session.trace"),
        "49g" => include_str!("../traces/49g-session.trace"),
        _ => panic!("no trace for {model}"),
    };
    Trace::parse(text).unwrap()
}

/// The calculator side of a test.
trait Peer {
    /// The device did `action` at `now`.
    fn action(&mut self, now: Instant, action: &Action);
    /// When the next bytes arrive.
    fn next_input(&self) -> Option<Instant>;
    /// The bytes due at `now`.
    fn take_input(&mut self, now: Instant) -> Option<Vec<u8>>;
}

impl Peer for Replay {
    fn action(&mut self, now: Instant, action: &Action) {
        Replay::action(self, now, action).unwrap();
    }
    fn next_input(&self) -> Option<Instant> {
        Replay::next_input(self)
    }
    fn take_input(&mut self, now: Instant) -> Option<Vec<u8>> {
        Replay::take_input(self, now)
    }
}

/// A trace replayed to its end, then a closure answering each write.
struct Then<F> {
    replay: Replay,
    answer: F,
    due: VecDeque<(Instant, Vec<u8>)>,
    actions: Vec<Action>,
}

impl<F: FnMut(&Action) -> Vec<Vec<u8>>> Peer for Then<F> {
    fn action(&mut self, now: Instant, action: &Action) {
        if self.replay.remaining() > 0 {
            return Peer::action(&mut self.replay, now, action);
        }
        self.actions.push(action.clone());
        let mut at = now;
        for bytes in (self.answer)(action) {
            at += Duration::from_millis(5);
            self.due.push_back((at, bytes));
        }
    }
    fn next_input(&self) -> Option<Instant> {
        Peer::next_input(&self.replay).or(self.due.front().map(|(at, _)| *at))
    }
    fn take_input(&mut self, now: Instant) -> Option<Vec<u8>> {
        if let Some(bytes) = Peer::take_input(&mut self.replay, now) {
            return Some(bytes);
        }
        if self.due.front().is_some_and(|(at, _)| *at <= now) {
            return self.due.pop_front().map(|(_, bytes)| bytes);
        }
        None
    }
}

fn then<F: FnMut(&Action) -> Vec<Vec<u8>>>(trace: &Trace, start: Instant, answer: F) -> Then<F> {
    Then {
        replay: Replay::new(trace, start),
        answer,
        due: VecDeque::new(),
        actions: Vec::new(),
    }
}

/// A device and its fake clock.
struct Host {
    device: Device,
    clock: Instant,
    start: Instant,
    events: Vec<Event>,
}

impl Host {
    fn new() -> Self {
        let start = Instant::now();
        Host {
            device: Device::new(start, DeviceOptions::default(), scenario::SEED),
            clock: start,
            start,
            events: Vec::new(),
        }
    }

    fn elapsed(&self) -> Duration {
        self.clock - self.start
    }

    /// Run `request` to its end against `peer`: actions go to the peer,
    /// the clock jumps to the next input or deadline.
    fn run(&mut self, peer: &mut dyn Peer, request: Request) -> Result<Outcome, TxError> {
        self.device.start(self.clock, request).unwrap();
        for _ in 0..100_000 {
            while let Some(action) = self.device.poll_action() {
                peer.action(self.clock, &action);
            }
            while let Some(event) = self.device.poll_event() {
                if !matches!(event, Event::Progress(_)) {
                    self.events.push(event);
                }
            }
            if let Some(result) = self.device.poll_result() {
                return result;
            }
            let next = match (peer.next_input(), self.device.next_timeout()) {
                (Some(a), Some(b)) => a.min(b),
                (a, b) => a.or(b).expect("the device waits for nothing"),
            };
            self.clock = self.clock.max(next);
            match peer.take_input(self.clock) {
                Some(bytes) => self.device.handle_input(self.clock, &bytes),
                None => self.device.handle_timeout(self.clock),
            }
        }
        panic!("no result");
    }
}

impl Host {
    /// The actions until the first write, letting the device's short
    /// waits (the turnaround, the discard of stale input) pass.
    fn until_write(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        loop {
            while let Some(action) = self.device.poll_action() {
                let write = matches!(action, Action::Write(_));
                actions.push(action);
                if write {
                    return actions;
                }
            }
            let deadline = self.device.next_timeout().expect("a deadline");
            self.clock = self.clock.max(deadline);
            self.device.handle_timeout(self.clock);
        }
    }
}

/// The recorded session replays on each model, byte for byte, and the
/// trace is used up.
#[test]
fn the_recorded_sessions_replay() {
    for (model, _) in scenario::MODELS {
        let trace = trace(model);
        let mut host = Host::new();
        let mut replay = Replay::new(&trace, host.clock);
        let mut driver = |_: &str, request| host.run(&mut replay, request);
        scenario::run(model, &mut driver).unwrap_or_else(|e| panic!("{model}: {e}"));
        assert_eq!(replay.remaining(), 0, "{model}: trace left over");
        assert_eq!(
            host.events,
            [
                Event::Listening(LinkSettings::DEFAULT),
                Event::Found(LinkSettings::DEFAULT)
            ],
            "{model}"
        );
        assert!(host.device.connection().is_none(), "{model}");
    }
}

/// Flag -35 is kept: the session found it clear (a fresh calculator), set
/// it for binary transfers, and the disconnect clears it again before
/// FINISH.
#[test]
fn flag_35_is_set_for_the_session_and_put_back() {
    for (model, _) in scenario::MODELS {
        let trace = trace(model);
        let commands: Vec<String> = trace
            .steps
            .iter()
            .filter_map(|(_, s)| match s {
                Step::Out(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
                _ => None,
            })
            .filter(|s| s.contains("-35 "))
            .collect();
        let first = commands.first().expect("-35 commands");
        assert!(first.contains("-35 FS?"), "{model}: {commands:?}");
        assert!(commands[1].contains("-35 SF"), "{model}: {commands:?}");
        let last = commands.last().unwrap();
        assert!(last.contains("-35 CF"), "{model}: {commands:?}");
        let finish = trace.before_note("request disconnect").steps.len();
        let after: Vec<_> = trace.steps[finish..].iter().collect();
        let cf = after
            .iter()
            .position(|(_, s)| matches!(s, Step::Out(b) if b.windows(6).any(|w| w == b"-35 CF")))
            .expect("CF in disconnect");
        let f = after
            .iter()
            .position(|(_, s)| matches!(s, Step::Out(b) if b.windows(2).any(|w| w == b"GF")))
            .expect("FINISH in disconnect");
        assert!(cf < f, "{model}");
    }
}

/// Nothing answers at any speed: four settings tried, 3 s each, no
/// connection.
#[test]
fn no_answer_tries_every_speed() {
    let mut host = Host::new();
    let mut peer = then(&Trace::new(), host.clock, |_| Vec::new());
    let result = host.run(&mut peer, Request::Connect(Default::default()));
    let Err(TxError::NoAnswer { tried }) = result else {
        panic!("{result:?}");
    };
    let speeds: Vec<u32> = tried.iter().map(|s| s.baud).collect();
    assert_eq!(speeds, [9600, 4800, 2400, 1200]);
    assert_eq!(host.elapsed(), Duration::from_secs(12));
    // Each speed: open, probe, probe again halfway.
    assert_eq!(peer.actions.len(), 12);
    assert!(
        TxError::NoAnswer { tried }
            .to_string()
            .contains("is it in server mode")
    );
    assert!(host.device.connection().is_none());
    // The device is free for another try.
    assert!(!host.device.is_busy());
}

/// `trace` as heard when the calculator talks at 2400 baud: garbage at
/// 9600, silence at 4800, then the recorded session.
fn at_2400(trace: &Trace) -> Trace {
    let shift = Duration::from_secs(6);
    let mut out = Trace::new();
    let probe = trace
        .steps
        .iter()
        .find_map(|(_, s)| match s {
            Step::Out(b) => Some(b.clone()),
            _ => None,
        })
        .unwrap();
    out.push(Duration::ZERO, Step::Open(LinkSettings::DEFAULT));
    out.push(Duration::ZERO, Step::Out(probe.clone()));
    out.push(
        Duration::from_millis(40),
        Step::In(vec![0x80, 0x00, 0xf8, 0x78]),
    );
    out.push(Duration::from_millis(1500), Step::Out(probe.clone()));
    out.push(Duration::from_millis(1540), Step::In(vec![0x00, 0xfe]));
    let slow = |baud| LinkSettings {
        baud,
        parity: Parity::None,
    };
    out.push(Duration::from_secs(3), Step::Open(slow(4800)));
    out.push(Duration::from_secs(3), Step::Out(probe.clone()));
    out.push(Duration::from_millis(4500), Step::Out(probe));
    for (at, step) in &trace.steps {
        let step = match step {
            Step::Open(s) if *s == LinkSettings::DEFAULT => Step::Open(slow(2400)),
            other => other.clone(),
        };
        out.push(*at + shift, step);
    }
    out
}

/// The calculator at 2400 baud: 9600 hears garbage, 4800 nothing, 2400 the
/// probe's answer; the rest of the session is the recorded one.
#[test]
fn the_wrong_speed_is_searched_past() {
    let trace = at_2400(&trace("48sx"));
    let mut host = Host::new();
    let mut replay = Replay::new(&trace, host.clock);
    let found = host.run(&mut replay, Request::Connect(Default::default()));
    let Ok(Outcome::Connected(c)) = found else {
        panic!("{found:?}");
    };
    assert_eq!(c.settings.baud, 2400);
    assert_eq!(c.settings.parity, Parity::None);
    let at = |baud| LinkSettings {
        baud,
        parity: Parity::None,
    };
    assert_eq!(
        host.events,
        [
            Event::Listening(at(9600)),
            Event::Listening(at(4800)),
            Event::Listening(at(2400)),
            Event::Found(at(2400)),
        ]
    );
}

/// Bit 7 of each byte as the parity bit of the low seven.
fn with_parity(bytes: &[u8], odd: bool) -> Vec<u8> {
    bytes
        .iter()
        .map(|&b| {
            let ones = (b & 0x7f).count_ones() % 2 == 1;
            if ones != odd { b | 0x80 } else { b & 0x7f }
        })
        .collect()
}

/// The calculator set to odd or even parity (IOPAR): its answer to the
/// probe reads only with bit 7 masked, and bit 7 is that parity; the port
/// is reopened with it and Kermit asks for 8th-bit prefixing.
#[test]
fn the_wrong_parity_is_found() {
    let recorded = trace("48gx");
    for (parity, odd) in [(Parity::Odd, true), (Parity::Even, false)] {
        let answer: Vec<u8> = recorded
            .before_note("request list")
            .steps
            .iter()
            .skip_while(|(_, s)| !matches!(s, Step::Out(_)))
            .skip(1)
            .take_while(|(_, s)| matches!(s, Step::In(_)))
            .flat_map(|(_, s)| match s {
                Step::In(b) => b.clone(),
                _ => Vec::new(),
            })
            .collect();
        assert!(answer.windows(2).any(|w| w == b"Y~"), "{answer:?}");
        let mut host = Host::new();
        host.device
            .start(host.clock, Request::Connect(Default::default()))
            .unwrap();
        let first: Vec<Action> = std::iter::from_fn(|| host.device.poll_action()).collect();
        assert_eq!(first[0], Action::Open(LinkSettings::DEFAULT));
        assert!(matches!(&first[1], Action::Write(_)));
        host.clock += Duration::from_millis(30);
        host.device
            .handle_input(host.clock, &with_parity(&answer, odd));
        let next = host.until_write();
        let found = LinkSettings { baud: 9600, parity };
        assert_eq!(next[0], Action::Open(found), "{parity:?}");
        assert_eq!(found.data_bits(), 7);
        let events: Vec<Event> = std::iter::from_fn(|| host.device.poll_event()).collect();
        assert_eq!(events.last(), Some(&Event::Found(found)));
        // The sync's C packet goes out next (the recorded one: the same
        // seed), and its exchange asks for 8th-bit prefixing.
        let Action::Write(sync) = &next[1] else {
            panic!("{next:?}");
        };
        let recorded_sync = recorded
            .steps
            .iter()
            .filter_map(|(_, s)| match s {
                Step::Out(b) => Some(b),
                _ => None,
            })
            .nth(1)
            .unwrap();
        assert_eq!(sync, recorded_sync);
        let s_packet = Packet::new(0, b'S', b"~* @-#Y3".to_vec())
            .encode(BlockCheck::Type1, &Framing::default())
            .unwrap();
        host.clock += Duration::from_millis(30);
        host.device.handle_input(host.clock, &s_packet);
        let Some(Action::Write(ack)) = host.device.poll_action() else {
            panic!("no ACK");
        };
        let ack = parse_frame(&ack[..ack.len() - 1], BlockCheck::Type1).unwrap();
        assert_eq!((ack.kind, ack.data.get(6)), (b'Y', Some(&b'&')), "{ack:?}");
        host.device.abort(host.clock);
    }
}

/// Every packet of a GET is NAKed: Kermit gives up after its retries, the
/// error is the device's result and the connection stays.
#[test]
fn a_nak_storm_fails_the_get() {
    let trace = trace("48sx").before_note("request get PRG binary");
    let mut host = Host::new();
    let mut nak = Deframer::new();
    let mut peer = then(&trace, host.clock, move |action| {
        let Action::Write(bytes) = action else {
            return Vec::new();
        };
        nak.push(bytes);
        std::iter::from_fn(|| nak.next_frame())
            .filter_map(|f| parse_frame(&f, BlockCheck::Type1).ok())
            .map(|p| {
                Packet::new(p.seq, b'N', Vec::new())
                    .encode(BlockCheck::Type1, &Framing::default())
                    .unwrap()
            })
            .collect()
    });
    let mut driver = |label: &str, request| {
        if label == "get PRG binary" {
            return Err(TxError::Busy);
        }
        host.run(&mut peer, request)
    };
    let stopped = scenario::run("48sx", &mut driver).unwrap_err();
    assert!(stopped.starts_with("get PRG"), "{stopped}");
    let result = host.run(
        &mut peer,
        Request::Get {
            dir: vec![],
            name: "PRG".into(),
            format: saturnus_tx::Format::Binary,
        },
    );
    assert!(
        matches!(result, Err(TxError::Calc(satx_core::Error::Kermit(_)))),
        "{result:?}"
    );
    let writes = peer
        .actions
        .iter()
        .filter(|a| matches!(a, Action::Write(_)))
        .count();
    assert!(writes > 5, "{writes} writes");
    assert!(host.device.connection().is_some());
}

/// The link is cut in the middle of a GET: the request fails with the I/O
/// error at once.
#[test]
fn a_cut_link_fails_the_request() {
    let trace = trace("49g").before_note("request get PRG binary");
    let mut host = Host::new();
    let mut replay = Replay::new(&trace, host.clock);
    let mut driver = |label: &str, request| {
        if label == "get PRG binary" {
            return Err(TxError::Busy);
        }
        host.run(&mut replay, request)
    };
    scenario::run("49g", &mut driver).unwrap_err();
    host.device
        .start(
            host.clock,
            Request::Get {
                dir: vec!["HOME".into()],
                name: "PRG".into(),
                format: saturnus_tx::Format::Binary,
            },
        )
        .unwrap();
    assert!(matches!(host.until_write().last(), Some(Action::Write(_))));
    host.clock += Duration::from_millis(3);
    host.device
        .handle_link_error(host.clock, &io::Error::from(io::ErrorKind::UnexpectedEof));
    let result = host.device.poll_result();
    assert!(
        matches!(&result, Some(Err(TxError::Calc(satx_core::Error::Io(e)))) if e.kind() == io::ErrorKind::UnexpectedEof),
        "{result:?}"
    );
    assert!(!host.device.is_busy());
}

/// Refusals before a byte is sent: not connected, a name the calculator
/// would refuse, a file that is no object without `as_string`. The name in
/// use is refused after the listing (recorded: `put PRG clash`).
#[test]
fn refusals_before_any_transfer() {
    let mut host = Host::new();
    let mut silent = then(&Trace::new(), host.clock, |_| Vec::new());
    let list = host.run(&mut silent, Request::List { dir: vec![] });
    assert!(matches!(list, Err(TxError::NotConnected)), "{list:?}");

    let trace = trace("48gx").before_note("request put PRG text");
    let mut replay = Replay::new(&trace, host.clock);
    let mut driver = |label: &str, request| {
        if label == "put PRG text" {
            return Err(TxError::Busy);
        }
        host.run(&mut replay, request)
    };
    scenario::run("48gx", &mut driver).unwrap_err();
    let put = |name: &str, data: &[u8]| Request::Put {
        dir: vec![],
        name: name.into(),
        data: data.to_vec(),
        replace: false,
        as_string: false,
    };
    for (request, want) in [
        (put("1X", scenario::PROGRAM), "BadName"),
        (put("A B", scenario::PROGRAM), "BadName"),
        (put("NOTE", scenario::NOTE), "NotAnObject"),
        (Request::Connect(Default::default()), "AlreadyConnected"),
    ] {
        let result = host.run(&mut replay, request);
        let got = format!("{result:?}");
        assert!(got.contains(want), "{got}");
    }
    assert!(host.device.poll_action().is_none());
}

/// The recorded name clash: the second put of PRG without `replace` is
/// refused from the listing and sends nothing else; with `replace` the
/// old variable is purged first.
#[test]
fn a_name_in_use_is_refused_then_replaced() {
    let trace = trace("48sx");
    let clash = trace.before_note("request put PRG replace");
    let start = clash.before_note("request put PRG clash").steps.len();
    let outs: Vec<String> = clash.steps[start..]
        .iter()
        .filter_map(|(_, s)| match s {
            Step::Out(b) => Some(String::from_utf8_lossy(b).into_owned()),
            _ => None,
        })
        .collect();
    // One G D listing and its ACKs, no SEND.
    assert!(outs[0].contains("GD"), "{outs:?}");
    assert!(
        !outs.iter().any(|o| o.contains("FPRG") || o.contains(" S~")),
        "{outs:?}"
    );
    let replace = trace.before_note("request list HOME again");
    let from = replace.before_note("request put PRG replace").steps.len();
    let purge = replace.steps[from..]
        .iter()
        .any(|(_, s)| matches!(s, Step::Out(b) if b.windows(11).any(|w| w == b"'PRG' PURGE")));
    assert!(purge);
}
