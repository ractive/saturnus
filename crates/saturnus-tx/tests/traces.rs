//! The device over the traces recorded from saturnus (`just tx-traces`),
//! on a fake clock: the whole session and the ops session on each model,
//! and the failures made from them (no answer, the wrong speed, the wrong
//! parity, a NAK storm, a cut link, names refused or in use, a purge
//! without its count, a backup without room, requests after a restore);
//! and a scripted calculator in XSERV (`tests/xserv.rs` runs the real
//! one on the emulated 49G).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod scenario;

use std::collections::VecDeque;
use std::io;

use kermit_proto::codec::{BlockCheck, Deframer, Framing, Packet, parse_frame};
use kermit_proto::prefix::{self, Quoting};
use saturnus_tx::trace::{Replay, Step, Trace};
use saturnus_tx::{
    Action, Device, DeviceOptions, Event, LinkSettings, Outcome, Parity, Request, TxError, probe,
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

/// The ops session's trace (the 49G's without its backup part).
fn ops_trace(model: &str) -> Trace {
    let text = match model {
        "48sx" => include_str!("../traces/48sx-ops.trace"),
        "48gx" => include_str!("../traces/48gx-ops.trace"),
        "49g" => include_str!("../traces/49g-ops.trace"),
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
        self.drive(peer, |_| false)
            .expect("stopped without a result")
    }

    /// Drive the running request until its result, or `None` once the
    /// device's writes since the start of this call satisfy `stop`.
    fn drive(
        &mut self,
        peer: &mut dyn Peer,
        stop: impl Fn(&[Vec<u8>]) -> bool,
    ) -> Option<Result<Outcome, TxError>> {
        let mut writes = Vec::new();
        for _ in 0..100_000 {
            while let Some(action) = self.device.poll_action() {
                peer.action(self.clock, &action);
                if let Action::Write(bytes) = action {
                    writes.push(bytes);
                }
            }
            while let Some(event) = self.device.poll_event() {
                if !matches!(event, Event::Progress(_)) {
                    self.events.push(event);
                }
            }
            if let Some(result) = self.device.poll_result() {
                return Some(result);
            }
            if stop(&writes) {
                return None;
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

/// Nothing answers at any speed: four settings tried, 0.5 s for XSERV's
/// answer to `M` and 6 s for the Kermit server's each, no connection.
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
    assert_eq!(host.elapsed(), Duration::from_secs(26));
    // Each speed: open, XSERV's M, the probe in three framings, again
    // halfway.
    assert_eq!(peer.actions.len(), 32);
    assert_eq!(peer.actions[1], Action::Write(b"M".to_vec()));
    let probes: Vec<&Action> = peer.actions[2..8].iter().collect();
    let framed: Vec<Action> = [Parity::None, Parity::Odd, Parity::Even]
        .into_iter()
        .map(|p| Action::Write(probe(p)))
        .collect();
    assert_eq!(probes[..3], framed.iter().collect::<Vec<_>>()[..]);
    assert_eq!(probes[3..], framed.iter().collect::<Vec<_>>()[..]);
    assert!(
        TxError::NoAnswer { tried }
            .to_string()
            .contains("is it in server mode")
    );
    assert!(host.device.connection().is_none());
    // The device is free for another try.
    assert!(!host.device.is_busy());
}

/// A calculator at 2400 baud: garbage answers the probes at 9600, silence
/// at 4800, and from the reopen at 2400 on the recorded session.
struct At2400 {
    recorded: Trace,
    baud: u32,
    replay: Option<Replay>,
    garbage: VecDeque<(Instant, Vec<u8>)>,
}

impl Peer for At2400 {
    fn action(&mut self, now: Instant, action: &Action) {
        if let Action::Open(s) = action {
            self.baud = s.baud;
            if s.baud == 2400 {
                let mut replay = Replay::new(&self.recorded, now);
                replay
                    .action(now, &Action::Open(LinkSettings::DEFAULT))
                    .unwrap();
                self.replay = Some(replay);
                return;
            }
        }
        match (&mut self.replay, self.baud) {
            (Some(replay), _) => Peer::action(replay, now, action),
            (None, 9600) => self.garbage.push_back((
                now + Duration::from_millis(40),
                vec![0x80, 0x00, 0xf8, 0x78],
            )),
            _ => {}
        }
    }
    fn next_input(&self) -> Option<Instant> {
        match &self.replay {
            Some(replay) => Peer::next_input(replay),
            None => self.garbage.front().map(|(at, _)| *at),
        }
    }
    fn take_input(&mut self, now: Instant) -> Option<Vec<u8>> {
        match &mut self.replay {
            Some(replay) => Peer::take_input(replay, now),
            None if self.garbage.front().is_some_and(|(at, _)| *at <= now) => {
                self.garbage.pop_front().map(|(_, b)| b)
            }
            None => None,
        }
    }
}

/// The calculator at 2400 baud: 9600 hears garbage, 4800 nothing, 2400 the
/// probe's answer; the rest of the session is the recorded one.
#[test]
fn the_wrong_speed_is_searched_past() {
    let mut host = Host::new();
    let mut peer = At2400 {
        recorded: trace("48sx"),
        baud: 0,
        replay: None,
        garbage: VecDeque::new(),
    };
    let found = host.run(&mut peer, Request::Connect(Default::default()));
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
    // 6.5 s at 9600 and at 4800, 0.5 s for XSERV at 2400, then the
    // connection.
    assert!(
        (14..17).contains(&host.elapsed().as_secs()),
        "{:?}",
        host.elapsed()
    );
    // The rest of the session goes on from the recorded one.
    let list = host.run(&mut peer, Request::List { dir: vec![] });
    assert!(matches!(list, Ok(Outcome::Listing(_))), "{list:?}");
}

/// Bit 7 of each byte as the parity bit of the low seven.
fn with_parity(bytes: &[u8], parity: Parity) -> Vec<u8> {
    bytes
        .iter()
        .map(|&b| {
            let ones = (b & 0x7f).count_ones() % 2 == 1;
            let set = match parity {
                Parity::None => false,
                Parity::Odd => !ones,
                Parity::Even => ones,
            };
            if set { b | 0x80 } else { b & 0x7f }
        })
        .collect()
}

/// The calculator set to no, odd or even parity (IOPAR) answers only the
/// probe framed for its parity, with its parity on its answer: the search
/// finds it, reopens the port with it, and Kermit asks for 8th-bit
/// prefixing on a 7-bit link.
#[test]
fn the_parity_is_found() {
    let recorded = trace("48gx");
    let answer: Vec<u8> = recorded
        .before_note("request list")
        .steps
        .iter()
        // The Kermit probe, after XSERV's M.
        .skip_while(|(_, s)| !matches!(s, Step::Out(b) if b.first() == Some(&0x01)))
        .skip(1)
        .take_while(|(_, s)| matches!(s, Step::In(_)))
        .flat_map(|(_, s)| match s {
            Step::In(b) => b.clone(),
            _ => Vec::new(),
        })
        .collect();
    assert!(answer.windows(2).any(|w| w == b"Y~"), "{answer:?}");
    for parity in [Parity::None, Parity::Odd, Parity::Even] {
        let mut host = Host::new();
        let ours = probe(parity);
        let reply = with_parity(&answer, parity);
        let mut peer = then(&Trace::new(), host.clock, move |action| match action {
            Action::Write(b) if *b == ours => vec![reply.clone()],
            _ => Vec::new(),
        });
        host.device
            .start(host.clock, Request::Connect(Default::default()))
            .unwrap();
        // Up to the sync's C packet.
        let found = LinkSettings { baud: 9600, parity };
        loop {
            while let Some(action) = host.device.poll_action() {
                peer.action(host.clock, &action);
            }
            let synced =
                |a: &Action| matches!(a, Action::Write(b) if b.windows(5).any(|w| w == b"SATX-"));
            if peer.actions.iter().any(synced) {
                break;
            }
            let next = match (peer.next_input(), host.device.next_timeout()) {
                (Some(a), Some(b)) => a.min(b),
                (a, b) => a.or(b).expect("a deadline"),
            };
            host.clock = host.clock.max(next);
            match peer.take_input(host.clock) {
                Some(bytes) => host.device.handle_input(host.clock, &bytes),
                None => host.device.handle_timeout(host.clock),
            }
        }
        let probes: Vec<Parity> = [Parity::None, Parity::Odd, Parity::Even]
            .into_iter()
            .filter(|p| peer.actions.contains(&Action::Write(probe(*p))))
            .collect();
        let expected: Vec<Parity> = match parity {
            Parity::None => vec![Parity::None],
            Parity::Odd => vec![Parity::None, Parity::Odd],
            Parity::Even => vec![Parity::None, Parity::Odd, Parity::Even],
        };
        assert_eq!(probes, expected, "{parity:?}");
        let events: Vec<Event> = std::iter::from_fn(|| host.device.poll_event()).collect();
        assert!(
            events.contains(&Event::Found(found)),
            "{parity:?}: {events:?}"
        );
        let opens: Vec<&Action> = peer
            .actions
            .iter()
            .filter(|a| matches!(a, Action::Open(_)))
            .collect();
        let mut expected_opens = vec![Action::Open(LinkSettings::DEFAULT)];
        if parity != Parity::None {
            expected_opens.push(Action::Open(found));
        }
        assert_eq!(
            opens,
            expected_opens.iter().collect::<Vec<_>>(),
            "{parity:?}"
        );
        // XSERV's 0.5 s, then the probes 0.5 s apart.
        assert!(host.elapsed() < Duration::from_millis(2500), "{parity:?}");
        if parity == Parity::None {
            continue;
        }
        assert_eq!(found.data_bits(), 7);
        // The sync's exchange asks for 8th-bit prefixing.
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
    let mut peer = then(&trace, host.clock, nak_all());
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
    let sync = host.run(&mut silent, Request::Sync);
    assert!(matches!(sync, Err(TxError::NotConnected)), "{sync:?}");

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
    // Into SATXPUT1 first, then one command swaps it in.
    let outs: Vec<&Vec<u8>> = replace.steps[from..]
        .iter()
        .filter_map(|(_, s)| match s {
            Step::Out(b) => Some(b),
            _ => None,
        })
        .collect();
    let file = outs.iter().position(|b| contains(b, b"FSATXPUT1"));
    let swap = outs
        .iter()
        .position(|b| contains(b, b"'SATXPUT1' RCL 'PRG' STO 'SATXPUT1' PURGE"));
    assert!(file.is_some() && swap.is_some() && file < swap, "{outs:?}");
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// The Kermit packet type of a write.
fn kind(bytes: &[u8]) -> Option<u8> {
    bytes.get(3).copied()
}

fn wire(seq: u8, kind: u8, data: &[u8]) -> Vec<u8> {
    Packet::new(seq, kind, data.to_vec())
        .encode(BlockCheck::Type1, &Framing::default())
        .unwrap()
}

/// Answers every packet written with a NAK of its sequence number.
fn nak_all() -> impl FnMut(&Action) -> Vec<Vec<u8>> {
    let mut deframer = Deframer::new();
    move |action| {
        let Action::Write(bytes) = action else {
            return Vec::new();
        };
        deframer.push(bytes);
        std::iter::from_fn(|| deframer.next_frame())
            .filter_map(|f| parse_frame(&f, BlockCheck::Type1).ok())
            .map(|p| wire(p.seq, b'N', b""))
            .collect()
    }
}

/// A Kermit server that answers every host command with `Empty Stack`,
/// ACKs FINISH and NAKs everything else.
fn server() -> impl FnMut(&Action) -> Vec<Vec<u8>> {
    let mut deframer = Deframer::new();
    let mut queue: Vec<Vec<u8>> = Vec::new();
    move |action| {
        let Action::Write(bytes) = action else {
            return Vec::new();
        };
        deframer.push(bytes);
        let mut out = Vec::new();
        while let Some(frame) = deframer.next_frame() {
            let Ok(p) = parse_frame(&frame, BlockCheck::Type1) else {
                continue;
            };
            match p.kind {
                b'C' => {
                    let text = prefix::encode_all(b"Empty Stack\r\n", &Quoting::default());
                    queue = vec![
                        wire(1, b'X', b""),
                        wire(2, b'D', &text),
                        wire(3, b'Z', b""),
                        wire(4, b'B', b""),
                    ];
                    out.push(wire(0, b'S', b"~* @-#Y1"));
                }
                b'G' if p.data == b"F" => out.push(wire(p.seq, b'Y', b"")),
                b'Y' => out.extend(queue.get(usize::from(p.seq)).cloned()),
                _ => out.push(wire(p.seq, b'N', b"")),
            }
        }
        out
    }
}

/// `trace` up to the request `label`, and of that request up to (not
/// including) the first write `stop` matches.
fn cut(trace: &Trace, label: &str, stop: impl Fn(&[u8]) -> bool) -> Trace {
    let note = format!("request {label}");
    let start = trace.before_note(&note).steps.len();
    let end = trace.steps[start..]
        .iter()
        .position(|(_, s)| matches!(s, Step::Out(b) if stop(b)))
        .map_or(trace.steps.len(), |i| start + i);
    Trace {
        steps: trace.steps[..end].to_vec(),
    }
}

/// The recorded session of `model` on `host` against `peer`, up to the
/// request `label`, which is not run.
fn session_until(host: &mut Host, peer: &mut dyn Peer, model: &str, label: &str) {
    let mut driver = |l: &str, request| {
        if l == label {
            return Err(TxError::Busy);
        }
        host.run(peer, request)
    };
    let stopped = scenario::run(model, &mut driver).unwrap_err();
    assert!(stopped.contains("Busy"), "{stopped}");
}

/// A replacing put whose transfer fails leaves the old variable alone: the
/// file was going to an unused name, nothing purges or overwrites `PRG`.
#[test]
fn a_failed_replace_keeps_the_old_variable() {
    let trace = cut(&trace("48sx"), "put PRG replace", |b| kind(b) == Some(b'S'));
    let mut host = Host::new();
    let mut peer = then(&trace, host.clock, nak_all());
    session_until(&mut host, &mut peer, "48sx", "put PRG replace");
    let mut file = b"HPHP48-J".to_vec();
    file.extend([0x2c, 0x2a, 0x20, 0x00, 0x00]);
    let result = host.run(
        &mut peer,
        Request::Put {
            dir: vec![],
            name: "PRG".into(),
            data: file,
            replace: true,
            as_string: false,
        },
    );
    assert!(
        matches!(result, Err(TxError::Calc(satx_core::Error::Kermit(_)))),
        "{result:?}"
    );
    // The first write after the listing is the SEND into the unused name,
    // not a purge of PRG.
    assert!(
        matches!(peer.actions.first(), Some(Action::Write(b)) if kind(b) == Some(b'S')),
        "{:?}",
        peer.actions.first()
    );
    for action in &peer.actions {
        let Action::Write(b) = action else { continue };
        assert!(
            !contains(b, b"'PRG'") && !contains(b, b"PURGE PRG"),
            "{b:?}"
        );
    }
    assert!(host.device.connection().is_some());
}

/// A text get that fails in the transfer still sets binary transfers
/// again (`-35 SF`).
#[test]
fn a_failed_text_get_still_restores_binary() {
    let trace = cut(&trace("48gx"), "get PRG2 text", |b| kind(b) == Some(b'R'));
    let mut host = Host::new();
    let mut peer = then(&trace, host.clock, nak_all());
    session_until(&mut host, &mut peer, "48gx", "get PRG2 text");
    let result = host.run(
        &mut peer,
        Request::Get {
            dir: vec![],
            name: "PRG2".into(),
            format: saturnus_tx::Format::Text,
        },
    );
    assert!(
        matches!(result, Err(TxError::Calc(satx_core::Error::Kermit(_)))),
        "{result:?}"
    );
    let written: Vec<&Vec<u8>> = peer
        .actions
        .iter()
        .filter_map(|a| match a {
            Action::Write(b) => Some(b),
            Action::Open(_) => None,
        })
        .collect();
    let r = written.iter().position(|b| kind(b) == Some(b'R')).unwrap();
    let sf = written.iter().position(|b| contains(b, b"-35 SF"));
    assert!(sf.is_some_and(|sf| sf > r), "{written:?}");
}

/// A text get aborted in the middle (flag -35 clear on the calculator): the
/// next binary get sets binary again before its GET.
#[test]
fn an_aborted_text_get_then_a_binary_get_sets_binary_first() {
    let trace = cut(&trace("49g"), "get PRG2 text", |b| kind(b) == Some(b'R'));
    let mut host = Host::new();
    let mut replay = Replay::new(&trace, host.clock);
    session_until(&mut host, &mut replay, "49g", "get PRG2 text");
    host.device
        .start(
            host.clock,
            Request::Get {
                dir: vec![],
                name: "PRG2".into(),
                format: saturnus_tx::Format::Text,
            },
        )
        .unwrap();
    // Up to the GET: flag -35 cleared, the R packet about to go out.
    let mut silent = then(&trace, host.clock, |_| Vec::new());
    silent.replay = replay;
    let stopped = host.drive(&mut silent, |w| w.iter().any(|b| kind(b) == Some(b'R')));
    assert!(stopped.is_none(), "{stopped:?}");
    host.device.abort(host.clock);

    let mut peer = then(&Trace::new(), host.clock, server());
    let result = host.run(
        &mut peer,
        Request::Get {
            dir: vec![],
            name: "PRG".into(),
            format: saturnus_tx::Format::Binary,
        },
    );
    assert!(result.is_err(), "{result:?}");
    let written: Vec<&Vec<u8>> = peer
        .actions
        .iter()
        .filter_map(|a| match a {
            Action::Write(b) => Some(b),
            Action::Open(_) => None,
        })
        .collect();
    let sf = written.iter().position(|b| contains(b, b"-35 SF")).unwrap();
    let r = written.iter().position(|b| kind(b) == Some(b'R')).unwrap();
    assert!(sf < r, "{written:?}");
}

/// After an aborted request, `Sync` sends the sync marker as a host
/// command (a server that does not echo it: not in step, said so).
#[test]
fn a_sync_after_an_abort_sends_the_marker() {
    let trace = cut(&trace("49g"), "get PRG2 text", |b| kind(b) == Some(b'R'));
    let mut host = Host::new();
    let mut replay = Replay::new(&trace, host.clock);
    session_until(&mut host, &mut replay, "49g", "get PRG2 text");
    host.device
        .start(
            host.clock,
            Request::Get {
                dir: vec![],
                name: "PRG2".into(),
                format: saturnus_tx::Format::Text,
            },
        )
        .unwrap();
    let mut silent = then(&trace, host.clock, |_| Vec::new());
    silent.replay = replay;
    let stopped = host.drive(&mut silent, |w| w.iter().any(|b| kind(b) == Some(b'R')));
    assert!(stopped.is_none(), "{stopped:?}");
    host.device.abort(host.clock);

    let mut peer = then(&Trace::new(), host.clock, server());
    let result = host.run(&mut peer, Request::Sync);
    let got = format!("{result:?}");
    assert!(got.contains("sync marker"), "{got}");
    let marker = peer
        .actions
        .iter()
        .any(|a| matches!(a, Action::Write(b) if kind(b) == Some(b'C') && contains(b, b"SATX-")));
    assert!(marker, "{:?}", peer.actions);
}

/// A disconnect whose `-35 CF` fails still sends FINISH. FINISH answered:
/// the session is over (and the error reported); FINISH failing too: the
/// session stays for another try.
#[test]
fn a_disconnect_finishes_even_when_the_flag_fails() {
    for finish_answered in [true, false] {
        let trace = trace("48sx").before_note("request disconnect");
        let mut host = Host::new();
        let mut naks = nak_all();
        let mut peer = then(&trace, host.clock, move |action| match action {
            Action::Write(b) if finish_answered && kind(b) == Some(b'G') => {
                vec![wire(0, b'Y', b"")]
            }
            other => naks(other),
        });
        session_until(&mut host, &mut peer, "48sx", "disconnect");
        let result = host.run(&mut peer, Request::Disconnect);
        assert!(result.is_err(), "{result:?}");
        let written: Vec<&Vec<u8>> = peer
            .actions
            .iter()
            .filter_map(|a| match a {
                Action::Write(b) => Some(b),
                Action::Open(_) => None,
            })
            .collect();
        let cf = written.iter().position(|b| contains(b, b"-35 CF")).unwrap();
        let finish = written
            .iter()
            .position(|b| kind(b) == Some(b'G') && contains(b, b"GF"));
        assert!(finish.is_some_and(|f| f > cf), "{written:?}");
        assert_eq!(
            host.device.connection().is_some(),
            !finish_answered,
            "finish answered: {finish_answered}"
        );
    }
}

/// The ops session replayed: a new device for the fresh calculator,
/// nothing to do for `SERVER` (the trace has the calculator's side).
struct ReplayEnv<'a> {
    host: Host,
    peer: &'a mut dyn Peer,
    /// Requests run instead of the recorded ones, by label.
    swap: Vec<(&'static str, Request)>,
    /// The device's state right after each request, by label.
    seen: Vec<(String, bool, bool)>,
}

impl scenario::Env for ReplayEnv<'_> {
    fn run(&mut self, label: &str, request: Request) -> Result<Outcome, TxError> {
        let request = match self.swap.iter().position(|(l, _)| *l == label) {
            Some(i) => self.swap.remove(i).1,
            None => request,
        };
        let result = self.host.run(self.peer, request);
        let d = &self.host.device;
        self.seen
            .push((label.to_string(), d.connection().is_some(), d.restarted()));
        result
    }
    fn fresh(&mut self) {
        self.host.device = Device::new(self.host.clock, DeviceOptions::default(), scenario::SEED);
    }
    fn server(&mut self) {}
}

fn replay_env<'a>(peer: &'a mut dyn Peer) -> ReplayEnv<'a> {
    ReplayEnv {
        host: Host::new(),
        peer,
        swap: Vec::new(),
        seen: Vec::new(),
    }
}

/// The recorded ops replay on each model byte for byte: make, rename,
/// purge with the count, a directory got, purged, put back and got again
/// the same, a tree into HOME refused then replaced, PICT; on the 48SX and
/// 48GX also the memory, a backup and its restore onto a fresh calculator
/// with the same HOME (the 49G's backup holds ROM bytes, so its trace
/// stops before; `just tx-traces` runs it live).
#[test]
fn the_recorded_ops_replay() {
    for (model, _) in scenario::MODELS {
        let trace = ops_trace(model);
        let mut replay = Replay::new(&trace, Instant::now());
        let mut env = replay_env(&mut replay);
        let result = if model == "49g" {
            scenario::ops_dirs(model, &mut env).map(|_| ())
        } else {
            scenario::ops(model, &mut env)
        };
        result.unwrap_or_else(|e| panic!("{model}: {e}"));
        let items = env
            .host
            .events
            .iter()
            .filter(|e| matches!(e, Event::Item(_)))
            .count();
        assert!(items > 10, "{model}: {items} item events");
        let seen = std::mem::take(&mut env.seen);
        drop(env);
        assert_eq!(replay.remaining(), 0, "{model}: trace left over");
        if model == "49g" {
            continue;
        }
        let state = |label: &str| {
            seen.iter()
                .find(|(l, _, _)| l == label)
                .map(|(_, connected, restarted)| (*connected, *restarted))
        };
        // Restored: no connection, restarted, until the next connect.
        assert_eq!(state("restore"), Some((false, true)), "{model}");
        assert_eq!(state("list after restore"), Some((false, true)), "{model}");
        assert_eq!(
            state("connect after restore"),
            Some((true, false)),
            "{model}"
        );
    }
}

/// After the restore the device refuses every request but a connect, and
/// writes nothing; the connect purges the backup left in port 0.
#[test]
fn after_a_restore_only_a_connect_goes_out() {
    let trace = ops_trace("48gx");
    let mut replay = Replay::new(&trace, Instant::now());
    let mut env = replay_env(&mut replay);
    // Up to the request after the restore.
    let mut until = Until {
        env: &mut env,
        label: "list after restore",
    };
    let d2 = scenario::ops_dirs("48gx", &mut until).unwrap();
    let stopped = scenario::ops_backup("48gx", &mut until, &d2).unwrap_err();
    assert!(stopped.contains("Busy"), "{stopped}");
    let host = &mut env.host;
    assert!(host.device.restarted());
    for request in [
        Request::List { dir: vec![] },
        Request::Backup,
        Request::Pict,
        Request::Disconnect,
    ] {
        host.device.start(host.clock, request).unwrap();
        assert!(host.device.poll_action().is_none());
        let result = host.device.poll_result();
        assert!(
            matches!(result, Some(Err(TxError::Restarted))),
            "{result:?}"
        );
        assert!(host.device.restarted());
    }
    assert!(
        TxError::Restarted
            .to_string()
            .contains("restarted; start SERVER"),
        "{}",
        TxError::Restarted
    );
    // The recorded reconnect sends `:0:SATXRS PURGE` once connected (the
    // restored flags have -35 as the user had it, clear: `-35 SF` first).
    let connect = trace.before_note("request list restored HOME");
    let from = trace
        .before_note("request connect after restore")
        .steps
        .len();
    let outs: Vec<&Vec<u8>> = connect.steps[from..]
        .iter()
        .filter_map(|(_, s)| match s {
            Step::Out(b) => Some(b),
            _ => None,
        })
        .collect();
    let sf = outs.iter().position(|b| contains(b, b"-35 SF"));
    let purge = outs.iter().position(|b| contains(b, b":0:SATXRS PURGE"));
    assert!(sf.is_some() && purge.is_some() && sf < purge, "{outs:?}");
}

/// An env that stops (`Busy`) at the request `label`.
struct Until<'e, 'a> {
    env: &'e mut ReplayEnv<'a>,
    label: &'static str,
}

impl scenario::Env for Until<'_, '_> {
    fn run(&mut self, label: &str, request: Request) -> Result<Outcome, TxError> {
        if label == self.label {
            return Err(TxError::Busy);
        }
        self.env.run(label, request)
    }
    fn fresh(&mut self) {
        self.env.fresh();
    }
    fn server(&mut self) {
        self.env.server();
    }
}

/// A purge of a directory that is not empty is refused with its count,
/// also when the count given is not the one it holds; the recorded
/// listings are the same either way.
#[test]
fn a_purge_needs_the_right_count() {
    for wrong in [None, Some(3), Some(5)] {
        let trace = ops_trace("48sx");
        let mut replay = Replay::new(&trace, Instant::now());
        let mut env = replay_env(&mut replay);
        env.swap.push((
            "purge D1",
            Request::Purge {
                dir: vec!["HOME".into()],
                name: "D1".into(),
                count: wrong,
            },
        ));
        scenario::ops_dirs("48sx", &mut env).unwrap_or_else(|e| panic!("{wrong:?}: {e}"));
    }
    // What the refused purge sent: listings only, no PGDIR.
    let trace = ops_trace("48sx");
    let refused = trace.before_note("request purge D1 count");
    let from = trace.before_note("request purge D1").steps.len();
    let outs: Vec<String> = refused.steps[from..]
        .iter()
        .filter_map(|(_, s)| match s {
            Step::Out(b) => Some(String::from_utf8_lossy(b).into_owned()),
            _ => None,
        })
        .collect();
    assert!(outs.iter().any(|o| o.contains("GD")), "{outs:?}");
    assert!(!outs.iter().any(|o| o.contains("PGDIR")), "{outs:?}");
    let purged = trace.before_note("request put-tree D1 again");
    let from = refused.steps.len();
    assert!(
        purged.steps[from..]
            .iter()
            .any(|(_, s)| matches!(s, Step::Out(b) if contains(b, b"'D1' PGDIR"))),
        "no PGDIR"
    );
}

/// Kermit's reply to a host command or `G D`: the server's S packet,
/// then after the host's ACK the text in X, D, Z, B packets.
fn text_reply(text: &[u8]) -> Vec<Vec<u8>> {
    let data = prefix::encode_all(text, &Quoting::default());
    let mut out = vec![wire(1, b'X', b"")];
    for (i, chunk) in data.chunks(80).enumerate() {
        out.push(wire(2 + i as u8, b'D', chunk));
    }
    let n = out.len() as u8 + 1;
    out.push(wire(n, b'Z', b""));
    out.push(wire(n + 1, b'B', b""));
    out
}

/// A server answering `G D` with `listing` and every command with
/// `Empty Stack`.
fn lister(listing: &'static [u8]) -> impl FnMut(&Action) -> Vec<Vec<u8>> {
    let mut deframer = Deframer::new();
    let mut queue: Vec<Vec<u8>> = Vec::new();
    move |action| {
        let Action::Write(bytes) = action else {
            return Vec::new();
        };
        deframer.push(bytes);
        let mut out = Vec::new();
        while let Some(frame) = deframer.next_frame() {
            let Ok(p) = parse_frame(&frame, BlockCheck::Type1) else {
                continue;
            };
            match (p.kind, p.data.as_slice()) {
                (b'G', b"D") => {
                    queue = text_reply(listing);
                    out.push(wire(0, b'S', b"~* @-#Y1"));
                }
                (b'C', _) => {
                    queue = text_reply(b"Empty Stack\r\n");
                    out.push(wire(0, b'S', b"~* @-#Y1"));
                }
                (b'Y', _) => out.extend(queue.get(usize::from(p.seq)).cloned()),
                _ => out.push(wire(p.seq, b'N', b"")),
            }
        }
        out
    }
}

/// A backup that does not fit is refused after the listing, before any
/// command: HOME holds 4000 bytes, 5000 are free, 9024 needed.
#[test]
fn a_backup_without_room_is_refused_first() {
    let trace = ops_trace("48gx").before_note(scenario::BACKUP_PART);
    let host = Host::new();
    let mut peer = then(
        &trace,
        host.clock,
        lister(b"{ HOME } 5000.\r\nBIG 4000. String 1234.\r\n"),
    );
    let mut replay_env = ReplayEnv {
        host,
        peer: &mut peer,
        swap: Vec::new(),
        seen: Vec::new(),
    };
    scenario::ops_dirs("48gx", &mut replay_env).unwrap();
    let ReplayEnv { mut host, .. } = replay_env;
    let memory = host.run(&mut peer, Request::Memory);
    let Ok(Outcome::Memory(m)) = memory else {
        panic!("{memory:?}");
    };
    assert_eq!((m.free, m.home, m.needed), (5000.0, 4000.0, 9024.0));
    assert!(!m.fits());
    let sent = peer.actions.len();
    let backup = host.run(&mut peer, Request::Backup);
    assert!(
        matches!(
            backup,
            Err(TxError::NoRoom {
                needed: 9024.0,
                free: 5000.0
            })
        ),
        "{backup:?}"
    );
    assert!(
        backup
            .unwrap_err()
            .to_string()
            .contains("copy everything to a folder")
    );
    // Only the listing went out: no ARCHIVE.
    for action in &peer.actions[sent..] {
        let Action::Write(b) = action else { continue };
        assert!(!contains(b, b"ARCHIVE"), "{b:?}");
    }
    assert!(host.device.connection().is_some());
}

/// A tree put is refused before anything is sent: a name the calculator
/// refuses, two files with one name, a file that is no object.
#[test]
fn a_bad_tree_is_refused_before_any_transfer() {
    let trace = ops_trace("48gx").before_note("request put-tree D1");
    let mut replay = Replay::new(&trace, Instant::now());
    let mut env = replay_env(&mut replay);
    let mut until = Until {
        env: &mut env,
        label: "put-tree D1",
    };
    scenario::ops_dirs("48gx", &mut until).unwrap_err();
    let host = &mut env.host;
    let tree = |items: Vec<saturnus_tx::TreeItem>| Request::PutTree {
        dir: vec![],
        name: Some("T".into()),
        items,
        replace: false,
        as_string: false,
    };
    let item = |path: &[&str], data: Option<&[u8]>| saturnus_tx::TreeItem {
        path: path.iter().map(|s| s.to_string()).collect(),
        data: data.map(<[u8]>::to_vec),
    };
    for (request, want) in [
        (
            tree(vec![item(&["1X"], Some(scenario::PROGRAM))]),
            "BadName",
        ),
        (
            tree(vec![
                item(&["A"], Some(scenario::PROGRAM)),
                item(&["A"], Some(scenario::PROGRAM)),
            ]),
            "Duplicate",
        ),
        (
            tree(vec![item(&["N"], Some(scenario::NOTE))]),
            "NotAnObject",
        ),
        (
            Request::PutTree {
                dir: vec![],
                name: Some("A B".into()),
                items: vec![],
                replace: false,
                as_string: false,
            },
            "BadName",
        ),
    ] {
        host.device.start(host.clock, request).unwrap();
        assert!(host.device.poll_action().is_none());
        let got = format!("{:?}", host.device.poll_result());
        assert!(got.contains(want), "{got}");
    }
}

/// The writes of the request `label` in `trace`.
fn writes_of(trace: &Trace, label: &str) -> Vec<Vec<u8>> {
    let from = trace.before_note(&format!("request {label}")).steps.len();
    trace.steps[from + 1..]
        .iter()
        .take_while(|(_, s)| !matches!(s, Step::Note(n) if n.starts_with("request ")))
        .filter_map(|(_, s)| match s {
            Step::Out(b) => Some(b.clone()),
            _ => None,
        })
        .collect()
}

/// RESTORE keeps the calculator's flags and ends server mode, so a
/// restore puts flag -35 back to the user's setting (clear on a fresh
/// calculator) after the backup is in port 0 and before RESTORE; the
/// reconnect finds it clear and sets binary for the session.
#[test]
fn a_restore_leaves_the_users_transfer_mode() {
    for model in ["48sx", "48gx"] {
        let trace = ops_trace(model);
        let outs = writes_of(&trace, "restore");
        let at = |needle: &[u8]| outs.iter().position(|b| contains(b, needle));
        let (copy, cf, restore) = (
            at(b":0:SATXRS STO"),
            at(b"-35 CF"),
            at(b":0:SATXRS RESTORE"),
        );
        assert!(
            copy.is_some() && copy < cf && cf < restore,
            "{model}: copy {copy:?} CF {cf:?} RESTORE {restore:?}"
        );
        assert!(
            !writes_of(&trace, "backup")
                .iter()
                .any(|b| contains(b, b"-35 CF")),
            "{model}"
        );
        let outs = writes_of(&trace, "connect after restore");
        let at = |needle: &[u8]| outs.iter().position(|b| contains(b, needle));
        let (fs, sf, purge) = (at(b"-35 FS?"), at(b"-35 SF"), at(b":0:SATXRS PURGE"));
        assert!(
            fs.is_some() && fs < sf && sf < purge,
            "{model}: FS? {fs:?} SF {sf:?} PURGE {purge:?}"
        );
    }
}

/// A purge of the restore's leftover that fails at the reconnect does not
/// fail the connect, and is tried again before the next request.
#[test]
fn a_failed_leftover_purge_is_tried_again() {
    let trace = cut(&ops_trace("48gx"), "connect after restore", |b| {
        contains(b, b":0:SATXRS PURGE")
    });
    let mut peer = then(&trace, Instant::now(), nak_all());
    let mut env = replay_env(&mut peer);
    let mut until = Until {
        env: &mut env,
        label: "list restored HOME",
    };
    let d2 = scenario::ops_dirs("48gx", &mut until).unwrap();
    let stopped = scenario::ops_backup("48gx", &mut until, &d2).unwrap_err();
    assert!(stopped.contains("Busy"), "{stopped}");
    let ReplayEnv { mut host, .. } = env;
    assert!(host.device.connection().is_some());
    assert!(!host.device.restarted());
    let list = host.run(&mut peer, Request::List { dir: vec![] });
    assert!(list.is_err(), "{list:?}");
    let purges = peer
        .actions
        .iter()
        .filter(|a| matches!(a, Action::Write(b) if contains(b, b":0:SATXRS PURGE")))
        .count();
    assert_eq!(purges, 2, "{:?}", peer.actions);
}

/// A sync is not preceded by the leftover's purge: the link is out of step
/// then, and a late reply read as the purge's would mark it done.
#[test]
fn a_sync_leaves_the_leftover_purge_for_the_next_request() {
    let trace = cut(&ops_trace("48gx"), "connect after restore", |b| {
        contains(b, b":0:SATXRS PURGE")
    });
    let mut peer = then(&trace, Instant::now(), nak_all());
    let mut env = replay_env(&mut peer);
    let mut until = Until {
        env: &mut env,
        label: "list restored HOME",
    };
    let d2 = scenario::ops_dirs("48gx", &mut until).unwrap();
    let stopped = scenario::ops_backup("48gx", &mut until, &d2).unwrap_err();
    assert!(stopped.contains("Busy"), "{stopped}");
    let ReplayEnv { mut host, .. } = env;
    let sync = host.run(&mut peer, Request::Sync);
    assert!(sync.is_err(), "{sync:?}");
    let purges = peer
        .actions
        .iter()
        .filter(|a| matches!(a, Action::Write(b) if contains(b, b":0:SATXRS PURGE")))
        .count();
    assert_eq!(purges, 1, "{:?}", peer.actions);
}

/// A tree got as text leaves out the kinds asked for and lists them; it
/// does not even ask for them.
#[test]
fn a_tree_get_skips_kinds() {
    for skip in [true, false] {
        let trace = ops_trace("48gx").before_note(scenario::BACKUP_PART);
        let mut peer = then(
            &trace,
            Instant::now(),
            lister(b"{ HOME } 5000.\r\nLIB 300. Library 1234.\r\n"),
        );
        let mut env = replay_env(&mut peer);
        scenario::ops_dirs("48gx", &mut env).unwrap();
        let ReplayEnv { mut host, .. } = env;
        let sent = peer.actions.len();
        let got = host.run(
            &mut peer,
            Request::GetTree {
                dir: vec!["HOME".into()],
                format: saturnus_tx::Format::Text,
                skip: if skip {
                    vec!["Library".into()]
                } else {
                    Vec::new()
                },
            },
        );
        let asked = peer.actions[sent..]
            .iter()
            .any(|a| matches!(a, Action::Write(b) if kind(b) == Some(b'R')));
        if skip {
            let Ok(Outcome::Tree(tree)) = got else {
                panic!("{got:?}");
            };
            assert!(tree.items.is_empty());
            assert_eq!(
                tree.skipped,
                [(vec!["LIB".to_string()], "Library".to_string())]
            );
            assert!(!asked);
        } else {
            assert!(got.is_err() && asked, "{got:?}");
        }
    }
}

/// A tree put that fails part of the way says what it made and stored.
#[test]
fn a_tree_put_that_stops_says_what_it_left() {
    let full = ops_trace("48sx");
    let from = full.before_note("request put-tree D1").steps.len();
    let second_send = full.steps[from..]
        .iter()
        .enumerate()
        .filter(|(_, (_, s))| matches!(s, Step::Out(b) if kind(b) == Some(b'S')))
        .nth(1)
        .map(|(i, _)| from + i)
        .unwrap();
    let trace = Trace {
        steps: full.steps[..second_send].to_vec(),
    };
    let mut peer = then(&trace, Instant::now(), nak_all());
    let mut env = replay_env(&mut peer);
    let stopped = scenario::ops_dirs("48sx", &mut env).unwrap_err();
    assert!(stopped.starts_with("put-tree D1"), "{stopped}");
    assert!(
        stopped.contains("Partial { directories: 2, variables: 1, root_made: true"),
        "{stopped}"
    );
}

/// A replacing put whose swap fails removes the copy in the unused name.
#[test]
fn a_failed_swap_removes_its_copy() {
    let trace = cut(&trace("48sx"), "put PRG replace", |b| {
        contains(b, b"'SATXPUT1' RCL")
    });
    let mut deframer = Deframer::new();
    let mut queue: Vec<Vec<u8>> = Vec::new();
    let answer = move |action: &Action| {
        let Action::Write(bytes) = action else {
            return Vec::new();
        };
        deframer.push(bytes);
        let mut out = Vec::new();
        while let Some(frame) = deframer.next_frame() {
            let Ok(p) = parse_frame(&frame, BlockCheck::Type1) else {
                continue;
            };
            let reply: Option<&[u8]> = match (p.kind, p.data.as_slice()) {
                (b'G', b"D") => Some(b"PRG 20. Program 1.\r\nSATXPUT1 20. Program 1.\r\n"),
                (b'C', d) if contains(d, b"RCL") => {
                    Some(b"Error: Insufficient Memory\r\n1: 'SATXPUT1'\r\n")
                }
                (b'C', _) => Some(b"Empty Stack\r\n"),
                _ => None,
            };
            match (reply, p.kind) {
                (Some(text), _) => {
                    queue = text_reply(text);
                    out.push(wire(0, b'S', b"~* @-#Y1"));
                }
                (None, b'Y') => out.extend(queue.get(usize::from(p.seq)).cloned()),
                (None, _) => out.push(wire(p.seq, b'N', b"")),
            }
        }
        out
    };
    let mut peer = then(&trace, Instant::now(), answer);
    let mut host = Host::new();
    let mut driver = |_: &str, request| host.run(&mut peer, request);
    let stopped = scenario::run("48sx", &mut driver).unwrap_err();
    assert!(
        stopped.starts_with("put PRG") && stopped.contains("Insufficient Memory"),
        "{stopped}"
    );
    assert!(
        peer.actions
            .iter()
            .any(|a| matches!(a, Action::Write(b) if contains(b, b"'SATXPUT1' PURGE"))),
        "{:?}",
        peer.actions
    );
}

/// An XSERV packet: length (high byte first), data, sum.
fn xpacket(data: &[u8]) -> Vec<u8> {
    let mut out = vec![(data.len() >> 8) as u8, data.len() as u8];
    out.extend_from_slice(data);
    out.push(data.iter().fold(0u8, |a, &b| a.wrapping_add(b)));
    out
}

/// An `L` record: name, prolog, nibbles, CRC.
fn record(name: &str, prolog: u16, nibbles: u32) -> Vec<u8> {
    let mut r = vec![name.len() as u8];
    r.extend_from_slice(name.as_bytes());
    r.extend_from_slice(&prolog.to_le_bytes());
    r.extend_from_slice(&nibbles.to_le_bytes()[..3]);
    r.extend_from_slice(&[0x34, 0x12]);
    r
}

/// Which server the scripted 49G runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Serving {
    Xserv,
    Kermit,
    /// After `Q`, or the warm start of a restore.
    Nothing,
}

/// A 49G in XSERV, scripted: HOME holds IOPAR and directory D (holding
/// X). XSERV answers `M` and `L`, ACKs every command packet and runs the
/// `E` texts Files sends (HOME, a directory name, the guarded changes,
/// the identification, `SERVER`), ignores what is no command byte, and
/// ends at `Q`. `SERVER` starts the Kermit server, which answers host
/// commands (the sync marker echoed, `-35 FS?` clear, else an empty
/// stack) and `G D` (IOPAR), sends any file asked for as `hello`, takes any file sent,
/// warm-starts at `RESTORE` and goes back to XSERV at FINISH. It counts
/// what reaches the wrong server.
struct XservCalc {
    serving: Serving,
    /// The prolog `1. R→I` stores: an integer on the 49G, a name on a 48.
    id_prolog: u16,
    id_stored: bool,
    cwd: Vec<String>,
    made: Vec<String>,
    expect_packet: bool,
    /// The `E` texts and the Kermit host commands, in order.
    executed: Vec<String>,
    /// FINISH gets no ACK (the calculator goes back to XSERV anyway).
    lose_finish_ack: bool,
    /// When FINISH came: XSERV drops an `M` within the second after it.
    finished_at: Option<Instant>,
    deframer: Deframer,
    queue: Vec<Vec<u8>>,
    due: VecDeque<(Instant, Vec<u8>)>,
    actions: Vec<Action>,
    /// Kermit packets that reached XSERV.
    kermit_in_xserv: usize,
    /// XSERV command bytes other than `M` that reached the Kermit server.
    xserv_in_kermit: usize,
}

impl XservCalc {
    fn new() -> Self {
        XservCalc {
            serving: Serving::Xserv,
            id_prolog: 0x2614,
            id_stored: false,
            cwd: vec!["HOME".into()],
            made: Vec::new(),
            expect_packet: false,
            executed: Vec::new(),
            lose_finish_ack: false,
            finished_at: None,
            deframer: Deframer::new(),
            queue: Vec::new(),
            due: VecDeque::new(),
            actions: Vec::new(),
            kermit_in_xserv: 0,
            xserv_in_kermit: 0,
        }
    }

    fn xserv(&mut self, now: Instant, w: &[u8]) -> Vec<Vec<u8>> {
        if self.expect_packet && w.len() >= 3 {
            self.expect_packet = false;
            let text = String::from_utf8_lossy(&w[2..w.len() - 1]).to_string();
            if text == "SERVER" {
                self.serving = Serving::Kermit;
            } else if text.contains("SATXID") {
                self.id_stored = text.contains(" STO ");
                self.cwd = vec!["HOME".into()];
            } else if text.contains("'N' CRDIR") {
                self.made.push("N".into());
            } else if !text.contains("IFERR") {
                for word in text.split(' ') {
                    match word {
                        "HOME" => self.cwd = vec!["HOME".into()],
                        name => self.cwd.push(name.to_string()),
                    }
                }
            }
            self.executed.push(text);
            return vec![vec![0x06]];
        }
        match w {
            b"M" if self
                .finished_at
                .is_some_and(|at| now < at + Duration::from_secs(1)) =>
            {
                Vec::new()
            }
            b"M" => vec![xpacket(b"245478.")],
            b"L" => {
                let data: Vec<u8> = if self.cwd == ["HOME"] {
                    let mut d = [record("IOPAR", 0x2A74, 40), record("D", 0x2A96, 30)].concat();
                    if self.id_stored {
                        d.extend(record("SATXID", self.id_prolog, 10));
                    }
                    d
                } else {
                    let mut d = record("X", 0x2933, 21);
                    for n in &self.made {
                        d.extend(record(n, 0x2A96, 10));
                    }
                    d
                };
                vec![xpacket(&data)]
            }
            b"E" => {
                self.expect_packet = true;
                Vec::new()
            }
            b"Q" => {
                self.serving = Serving::Nothing;
                Vec::new()
            }
            [0x01, _, ..] => {
                self.kermit_in_xserv += 1;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn kermit(&mut self, now: Instant, w: &[u8]) -> Vec<Vec<u8>> {
        if let [b'G' | b'P' | b'E' | b'L' | b'Q'] = w {
            self.xserv_in_kermit += 1;
        }
        self.deframer.push(w);
        let mut out = Vec::new();
        while let Some(frame) = self.deframer.next_frame() {
            let Ok(p) = parse_frame(&frame, BlockCheck::Type1) else {
                continue;
            };
            let data = prefix::decode(&p.data, &Quoting::default()).unwrap_or_default();
            match (p.kind, data.as_slice()) {
                (b'I' | b'S', _) => out.push(wire(p.seq, b'Y', b"~* @-#Y1")),
                (b'C', command) => {
                    let command = String::from_utf8_lossy(command).to_string();
                    self.executed.push(command.clone());
                    if command.contains("RESTORE") {
                        self.serving = Serving::Nothing;
                        return out;
                    }
                    let text = if command.starts_with('"') {
                        format!("1: {command}\r\n")
                    } else if command.contains("FS?") {
                        "1: 0.\r\n".to_string()
                    } else {
                        "Empty Stack\r\n".to_string()
                    };
                    self.queue = text_reply(text.as_bytes());
                    out.push(wire(0, b'S', b"~* @-#Y1"));
                }
                (b'R', _) => {
                    let mut q = text_reply(b"hello");
                    q[0] = wire(1, b'F', b"X");
                    self.queue = q;
                    out.push(wire(0, b'S', b"~* @-#Y1"));
                }
                (b'G', b"D") => {
                    self.queue = text_reply(b"IOPAR 29.5 List 1.\r\n");
                    out.push(wire(0, b'S', b"~* @-#Y1"));
                }
                (b'G', b"F") => {
                    self.serving = Serving::Xserv;
                    self.finished_at = Some(now);
                    if self.lose_finish_ack {
                        self.lose_finish_ack = false;
                    } else {
                        out.push(wire(p.seq, b'Y', b""));
                    }
                    return out;
                }
                (b'Y', _) => out.extend(self.queue.get(usize::from(p.seq)).cloned()),
                (b'F' | b'D' | b'Z' | b'B' | b'A', _) => out.push(wire(p.seq, b'Y', b"")),
                _ => out.push(wire(p.seq, b'N', b"")),
            }
        }
        out
    }
}

impl Peer for XservCalc {
    fn action(&mut self, now: Instant, action: &Action) {
        self.actions.push(action.clone());
        let Action::Write(w) = action else {
            return;
        };
        let replies = match self.serving {
            Serving::Xserv => self.xserv(now, w),
            Serving::Kermit => self.kermit(now, w),
            Serving::Nothing => Vec::new(),
        };
        let mut at = now;
        for bytes in replies {
            at += Duration::from_millis(5);
            self.due.push_back((at, bytes));
        }
    }
    fn next_input(&self) -> Option<Instant> {
        self.due.front().map(|(at, _)| *at)
    }
    fn take_input(&mut self, now: Instant) -> Option<Vec<u8>> {
        if self.due.front().is_some_and(|(at, _)| *at <= now) {
            return self.due.pop_front().map(|(_, bytes)| bytes);
        }
        None
    }
}

/// Connected to the scripted 49G in XSERV.
fn xserv_connected(peer: &mut XservCalc) -> Host {
    let mut host = Host::new();
    let connected = host.run(peer, Request::Connect(Default::default()));
    let Ok(Outcome::Connected(c)) = connected else {
        panic!("{connected:?}");
    };
    assert_eq!(c.server, saturnus_tx::Server::Xserv);
    host
}

fn text_get(name: &str) -> Request {
    Request::Get {
        dir: vec![],
        name: name.into(),
        format: saturnus_tx::Format::Text,
    }
}

/// The scripted 49G in XSERV: found, told from a 48, listed and changed.
#[test]
fn a_calculator_in_xserv_is_found_listed_and_changed() {
    let mut peer = XservCalc::new();
    let mut host = xserv_connected(&mut peer);
    assert_eq!(host.device.connection().unwrap().model, satx_core::Model::Hp49G);
    // Found by the first M: no Kermit probe went out; the identification
    // stored its variable and purged it again.
    assert!(
        host.elapsed() < Duration::from_secs(1),
        "{:?}",
        host.elapsed()
    );
    assert!(
        !peer
            .actions
            .iter()
            .any(|a| *a == Action::Write(probe(Parity::None)))
    );
    assert!(!peer.id_stored);
    assert_eq!(peer.executed.len(), 2, "{:?}", peer.executed);
    assert!(peer.executed[0].starts_with("HOME DEPTH "), "{:?}", peer.executed);
    peer.executed.clear();

    // HOME: the sizes as the Kermit server gives them.
    let home = host.run(&mut peer, Request::List { dir: vec![] });
    let Ok(Outcome::Listing(l)) = home else {
        panic!("{home:?}");
    };
    let iopar = &l.entries[0];
    assert_eq!(
        (iopar.name.as_str(), iopar.size, iopar.kind.as_str()),
        ("IOPAR", 29.5, "List")
    );
    assert!(l.entries[1].is_directory());

    // Into D (seen as a directory in HOME's listing), a new directory there.
    let d = vec!["HOME".to_string(), "D".to_string()];
    let made = host.run(
        &mut peer,
        Request::Mkdir {
            dir: d.clone(),
            name: "N".into(),
        },
    );
    assert!(matches!(made, Ok(Outcome::Created(_))), "{made:?}");
    let clash = host.run(
        &mut peer,
        Request::Mkdir {
            dir: d.clone(),
            name: "X".into(),
        },
    );
    assert!(matches!(clash, Err(TxError::NameClash(_))), "{clash:?}");
    // A directory nobody listed is not evaluated.
    let missing = host.run(
        &mut peer,
        Request::List {
            dir: vec!["HOME".into(), "NOPE".into()],
        },
    );
    assert!(matches!(missing, Err(TxError::NotFound(_))), "{missing:?}");
    let executed = &peer.executed;
    assert_eq!(executed.len(), 3, "{executed:?}");
    assert_eq!(executed[0], "D");
    assert!(
        executed[1].starts_with("DEPTH ")
            && executed[1].contains(" IFERR \"'N' CRDIR\" STR")
            && executed[1].contains(" THEN DEPTH satxd - DROPN END "),
        "{executed:?}"
    );
    assert_eq!(executed[2], "HOME");

    // Refused before the round trip to the Kermit server: a file that is
    // no object, a tree with a bad name.
    peer.executed.clear();
    let note = host.run(
        &mut peer,
        Request::Put {
            dir: vec![],
            name: "N".into(),
            data: b"hello".to_vec(),
            replace: false,
            as_string: false,
        },
    );
    assert!(matches!(note, Err(TxError::NotAnObject)), "{note:?}");
    let tree = host.run(
        &mut peer,
        Request::PutTree {
            dir: vec![],
            name: None,
            items: vec![saturnus_tx::TreeItem {
                path: vec!["1X".into()],
                data: Some(b"hello".to_vec()),
            }],
            replace: false,
            as_string: true,
        },
    );
    assert!(matches!(tree, Err(TxError::BadName(_))), "{tree:?}");
    assert!(peer.executed.is_empty(), "{:?}", peer.executed);

    // No fast transfer; the disconnect ends XSERV.
    let fast = host.run(
        &mut peer,
        Request::XmodemGet {
            dir: d,
            name: "X".into(),
            wait: Duration::from_secs(1),
        },
    );
    assert!(matches!(fast, Err(TxError::NotForXmodem(_))), "{fast:?}");
    let quit = host.run(&mut peer, Request::Disconnect);
    assert!(
        matches!(quit, Ok(Outcome::XservQuit { ended: true })),
        "{quit:?}"
    );
    assert!(host.device.connection().is_none());
}

/// An HP 48 running XSrvr48 answers `M` as the 49G does; its `1. R→I`
/// stores a name, and the connect is refused (no `-95. SF` goes to it).
#[test]
fn a_48_in_xserv_is_refused() {
    let mut peer = XservCalc::new();
    peer.id_prolog = 0x2E48;
    let mut host = Host::new();
    let connected = host.run(&mut peer, Request::Connect(Default::default()));
    assert!(matches!(connected, Err(TxError::Xsrvr48)), "{connected:?}");
    assert!(host.device.connection().is_none());
    assert!(!peer.id_stored, "the variable is purged");
}

/// A text get through the nested Kermit server and back; the Kermit
/// server found flag -35 clear, and FINISH comes after `-35 CF`.
#[test]
fn a_text_get_goes_through_the_nested_kermit_server() {
    let mut peer = XservCalc::new();
    let mut host = xserv_connected(&mut peer);
    let got = host.run(&mut peer, text_get("X"));
    assert!(
        matches!(&got, Ok(Outcome::File(f)) if f.data == b"hello"),
        "{got:?}"
    );
    assert_eq!(peer.serving, Serving::Xserv);
    assert_eq!((peer.kermit_in_xserv, peer.xserv_in_kermit), (0, 0));
    assert!(peer.executed.iter().any(|c| c == "-35 CF"), "{:?}", peer.executed);
    let list = host.run(&mut peer, Request::List { dir: vec![] });
    assert!(matches!(list, Ok(Outcome::Listing(_))), "{list:?}");
}

/// Stopped just as the nested Kermit server is synced: the next request
/// finds the Kermit server (XSERV's `M` gets no answer), syncs it, ends it
/// with FINISH and lists on XSERV. Nothing reaches the wrong server.
#[test]
fn a_stop_while_entering_the_kermit_server_is_sorted_out() {
    let mut peer = XservCalc::new();
    let mut host = xserv_connected(&mut peer);
    host.device.start(host.clock, text_get("X")).unwrap();
    let stopped = host.drive(&mut peer, |w| w.iter().any(|b| b.first() == Some(&0x01)));
    assert!(stopped.is_none(), "{stopped:?}");
    assert_eq!(peer.serving, Serving::Kermit);
    host.device.abort(host.clock);
    let list = host.run(&mut peer, Request::List { dir: vec![] });
    assert!(matches!(list, Ok(Outcome::Listing(_))), "{list:?}");
    assert_eq!(peer.serving, Serving::Xserv);
    assert_eq!((peer.kermit_in_xserv, peer.xserv_in_kermit), (0, 0));
    // The flag was never read, so it was not set back either.
    assert!(!peer.executed.iter().any(|c| c.contains("-35")), "{:?}", peer.executed);
}

/// FINISH's ACK is lost: the request fails, but the calculator is in XSERV
/// again; the next request asks `M` (XSERV answers once its second after
/// FINISH is over) and goes on, no Kermit packet sent to XSERV.
#[test]
fn a_lost_finish_ack_is_sorted_out() {
    let mut peer = XservCalc::new();
    let mut host = xserv_connected(&mut peer);
    peer.lose_finish_ack = true;
    let got = host.run(&mut peer, text_get("X"));
    assert!(got.is_err(), "{got:?}");
    assert_eq!(peer.serving, Serving::Xserv);
    let resent = peer.kermit_in_xserv;
    let list = host.run(&mut peer, Request::List { dir: vec![] });
    assert!(matches!(list, Ok(Outcome::Listing(_))), "{list:?}");
    assert_eq!(peer.kermit_in_xserv, resent, "no Kermit packet after the failure");
    assert_eq!(peer.xserv_in_kermit, 0);
}

/// A restore over XSERV leaves flag -35 as the nested Kermit server found
/// it (clear), not the binary of the XSERV connection.
#[test]
fn a_restore_over_xserv_puts_the_users_transfer_mode_back() {
    let mut peer = XservCalc::new();
    let mut host = xserv_connected(&mut peer);
    // An empty directory as the 49G's backup file.
    let backup = [&b"HPHP49-C"[..], &[0x96, 0x2A, 0, 0, 0, 0, 0]].concat();
    let restored = host.run(&mut peer, Request::Restore { data: backup });
    assert!(matches!(restored, Ok(Outcome::Restored)), "{restored:?}");
    let restore = peer
        .executed
        .iter()
        .position(|c| c.contains("RESTORE"))
        .expect("RESTORE");
    let cf = peer.executed.iter().rposition(|c| c == "-35 CF");
    assert!(cf.is_some_and(|cf| cf < restore), "{:?}", peer.executed);
}
