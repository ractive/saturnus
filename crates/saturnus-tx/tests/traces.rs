//! The device over the traces recorded from saturnus (`just tx-traces`),
//! on a fake clock: the whole session on each model, and the failures made
//! from them (no answer, the wrong speed, the wrong parity, a NAK storm, a
//! cut link, names refused or in use).

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

/// Nothing answers at any speed: four settings tried, 6 s each, no
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
    assert_eq!(host.elapsed(), Duration::from_secs(24));
    // Each speed: open, the probe in three framings, again halfway.
    assert_eq!(peer.actions.len(), 28);
    let probes: Vec<&Action> = peer.actions[1..7].iter().collect();
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
    // 6 s at 9600 and at 4800, then the connection at 2400.
    assert!(
        (12..14).contains(&host.elapsed().as_secs()),
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
        .skip_while(|(_, s)| !matches!(s, Step::Out(_)))
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
        assert!(host.elapsed() < Duration::from_secs(2), "{parity:?}");
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
