//! The Kermit server state machine: the calculator is the client and we
//! are the "disk drive" it sends to and gets from, as the 38G, 39G and 40G
//! expect (wiki: protocols/aplet-transfer).
//!
//! The calculator opens every transaction: an I packet (parameters only),
//! an R packet (it wants a file; the caller decides, [`Server::serve`] or
//! [`Server::refuse`]) or an S packet (it sends files, which arrive as
//! [`ServerEvent::Received`]). Between transactions the server waits
//! without a deadline. Generic commands (G) are refused with an E packet.
//!
//! The seam is the client's: [`Server::handle_input`],
//! [`Server::handle_timeout`], [`Server::poll_output`],
//! [`Server::poll_event`] and [`Server::next_timeout`], each taking the
//! caller's `now`.

use crate::time::{Duration, Instant};
use std::collections::VecDeque;

use crate::client::{Error, OutgoingFile};
use crate::codec::{BlockCheck, CR, Deframer, FrameError, Framing, Packet, parse_frame, unchar};
use crate::params::{InitParams, Negotiated, negotiate};
use crate::prefix::{self, Quoting};

/// Tunables of a [`Server`].
///
/// Non-exhaustive: start from [`ServerConfig::default`] and set the fields
/// you need.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ServerConfig {
    /// Block check we accept in our answer to the client's Send-Init
    /// (default type 3; the client's own choice wins when it asks for
    /// less).
    pub block_check: BlockCheck,
    /// `true` (default): QBIN `Y`, no 8th-bit prefixing unless the client
    /// asks; `false`: we ask for it with `&`.
    pub eight_bit_clean: bool,
    /// Inside a transaction: how long we wait for the client's next packet
    /// before we resend (or NAK) (default 10 s, the 38G's own TIME).
    pub timeout: Duration,
    /// Resends (or NAKs) per packet before the transaction fails (default 5).
    pub retries: u32,
    /// Delay before each packet we send in reply to input (default 0).
    pub packet_pause: Duration,
    /// Most decoded data bytes accepted in one receive transaction; more
    /// fails it with [`Error::TooLarge`] and an E packet. Default 4 MiB;
    /// `None` means no limit.
    pub max_size: Option<usize>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            block_check: BlockCheck::Type3,
            eight_bit_clean: true,
            timeout: Duration::from_secs(10),
            retries: 5,
            packet_pause: Duration::ZERO,
            max_size: Some(4 << 20),
        }
    }
}

/// Something that happened on the server's side of the link.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ServerEvent {
    /// The client asks for a file (R). Answer with [`Server::serve`] or
    /// [`Server::refuse`]; the server waits for the answer.
    Request {
        /// The file name, decoded.
        name: Vec<u8>,
    },
    /// A file starts: one we serve (the name from the client's ACK to F,
    /// else ours) or one the client sends (its F packet).
    FileStart {
        /// File name.
        name: Vec<u8>,
        /// `true` when we send it.
        sending: bool,
    },
    /// Bytes of the current file acknowledged (send) or received.
    Progress {
        /// Bytes so far.
        done: u64,
        /// The file's size when we send it; `None` when receiving.
        total: Option<u64>,
    },
    /// A file the client sent is complete.
    Received {
        /// Its name (the F packet).
        name: Vec<u8>,
        /// Its bytes.
        data: Vec<u8>,
    },
    /// The client discarded the file it was sending (Z with `D`).
    Discarded {
        /// Its name.
        name: Vec<u8>,
    },
    /// The file we served is acknowledged to its end (its Z).
    Sent {
        /// Its name.
        name: Vec<u8>,
    },
    /// An E packet from the client, with its decoded text. Ends a running
    /// transaction; also the client's answer to [`Server::refuse`].
    Remote(Vec<u8>),
    /// A transaction failed on our side (timeout, protocol, too large,
    /// cancelled); an E packet went out where one makes sense.
    Error(Error),
    /// A transaction ended normally (its B acknowledged).
    Done,
}

/// Why [`Server::serve`] or [`Server::refuse`] was refused: no request is
/// waiting for an answer (for [`Server::reject`]: no file is being
/// received).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoRequest;

impl std::fmt::Display for NoRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("no file request is waiting for an answer")
    }
}

impl std::error::Error for NoRequest {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Init,
    File,
    Data { chunk: usize },
    Eof,
    Break,
}

#[derive(Debug)]
struct Sending {
    file: OutgoingFile,
    pos: usize,
    step: Step,
}

#[derive(Debug)]
enum Phase {
    /// Between transactions.
    Idle,
    /// An R arrived; the caller has not answered yet.
    Requested,
    /// We send the requested file; `self.seq` is the packet awaiting its ACK.
    Send(Sending),
    /// The client sends; `self.seq` is the packet expected.
    Receive {
        name: Option<Vec<u8>>,
        data: Vec<u8>,
    },
}

/// Sans-IO Kermit server. See the module documentation.
#[derive(Debug)]
pub struct Server {
    config: ServerConfig,
    ours: InitParams,
    deframer: Deframer,
    phase: Phase,
    neg: Option<Negotiated>,
    seq: u8,
    /// The packet to resend on a timeout or NAK (send), or on a duplicate
    /// of the packet it answered (receive and between transactions).
    last_sent: Vec<u8>,
    /// Between transactions: the seq and type of the packet `last_sent`
    /// answered, so a retransmission of it gets the same answer.
    answered: Option<(u8, u8)>,
    queue: VecDeque<(Instant, Vec<u8>)>,
    events: VecDeque<ServerEvent>,
    deadline: Option<Instant>,
    retries: u32,
    received: usize,
}

impl Server {
    /// New server, waiting for the client's first packet.
    pub fn new(config: ServerConfig) -> Self {
        let ours = InitParams {
            maxl: 94,
            time: config.timeout.as_secs().clamp(1, 94) as u8,
            npad: 0,
            padc: 0,
            eol: CR,
            qctl: b'#',
            qbin: if config.eight_bit_clean { b'Y' } else { b'&' },
            chkt: config.block_check.to_char(),
            rept: b' ',
        };
        Server {
            config,
            ours,
            deframer: Deframer::new(),
            phase: Phase::Idle,
            neg: None,
            seq: 0,
            last_sent: Vec::new(),
            answered: None,
            queue: VecDeque::new(),
            events: VecDeque::new(),
            deadline: None,
            retries: 0,
            received: 0,
        }
    }

    /// Our Send-Init parameters (what we answer an I or S with).
    pub fn params(&self) -> InitParams {
        self.ours
    }

    /// Feed bytes read from the link.
    pub fn handle_input(&mut self, now: Instant, bytes: &[u8]) {
        self.deframer.push(bytes);
        while let Some(frame) = self.deframer.next_frame() {
            let kind = frame.get(3).copied();
            let check = match (&self.phase, self.neg) {
                (Phase::Receive { .. }, Some(n)) if kind != Some(b'S') => n.check,
                (Phase::Send(s), Some(n)) if s.step != Step::Init => n.check,
                _ => BlockCheck::Type1,
            };
            let mut parsed = parse_frame(&frame, check);
            if let (Phase::Idle, Err(FrameError::BadCheck), Some(n)) =
                (&self.phase, &parsed, self.neg)
            {
                // A retransmission of the last packet of a transaction that
                // ran with another block check (the B after a CRC receive).
                parsed = parse_frame(&frame, n.check);
            }
            // A SEQ byte outside `tochar(0..=63)` (below space, or above
            // `_`) is no packet to answer: `parse_frame` would fold it into
            // 0..=63, and the answer would carry a seq the client never sent.
            if frame.get(2).is_some_and(|c| !(32..=95).contains(c)) {
                parsed = Err(FrameError::BadLength);
            }
            let raw_seq = frame.get(2).map(|&c| unchar(c));
            match self.phase {
                Phase::Idle => self.on_idle(now, parsed),
                Phase::Requested => self.on_requested(now, parsed),
                Phase::Send(_) => self.on_send(now, parsed, raw_seq),
                Phase::Receive { .. } => self.on_receive(now, parsed),
            }
        }
    }

    /// Call when the time from [`Server::next_timeout`] has passed.
    pub fn handle_timeout(&mut self, now: Instant) {
        let Some(d) = self.deadline else { return };
        if now < d {
            return;
        }
        self.deadline = None;
        match self.phase {
            Phase::Receive { .. } => {
                let nak = self.nak();
                self.retry_with(now, nak);
            }
            Phase::Send(_) => {
                let bytes = self.last_sent.clone();
                self.retry_with(now, bytes);
            }
            Phase::Idle | Phase::Requested => {}
        }
    }

    /// Next packet to write (one whole packet, as the client's). `None` if
    /// nothing is due at `now`.
    pub fn poll_output(&mut self, now: Instant) -> Option<Vec<u8>> {
        match self.queue.front() {
            Some((due, _)) if *due <= now => {}
            _ => return None,
        }
        let (_, bytes) = self.queue.pop_front()?;
        if matches!(self.phase, Phase::Send(_) | Phase::Receive { .. }) {
            self.deadline = now.checked_add(self.config.timeout);
        }
        Some(bytes)
    }

    /// Next event, in order.
    pub fn poll_event(&mut self) -> Option<ServerEvent> {
        self.events.pop_front()
    }

    /// When the server wants to be called again: a queued packet becomes
    /// due, or the client's next packet is late. `None` between
    /// transactions: the server waits for the client as long as it takes.
    pub fn next_timeout(&self) -> Option<Instant> {
        let due = self.queue.iter().map(|(t, _)| *t).min();
        match (due, self.deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// No transaction is running and no request waits for an answer.
    pub fn is_idle(&self) -> bool {
        matches!(self.phase, Phase::Idle)
    }

    /// A request ([`ServerEvent::Request`]) waits for [`Server::serve`] or
    /// [`Server::refuse`].
    pub fn is_requested(&self) -> bool {
        matches!(self.phase, Phase::Requested)
    }

    /// Answer the waiting request with `file`: S, F, D.., Z, B. The F
    /// carries `file.name`.
    pub fn serve(&mut self, now: Instant, file: OutgoingFile) -> Result<(), NoRequest> {
        if !self.is_requested() {
            return Err(NoRequest);
        }
        self.neg = None;
        self.seq = 0;
        self.retries = 0;
        self.answered = None;
        self.phase = Phase::Send(Sending {
            file,
            pos: 0,
            step: Step::Init,
        });
        let bytes =
            Packet::new(0, b'S', self.ours.encode()).wire(BlockCheck::Type1, &Framing::default());
        self.reply(now, bytes);
        Ok(())
    }

    /// Refuse the waiting request with an E packet carrying `text` (e.g.
    /// "File not found"). The calculator answers with its own E, which
    /// comes as [`ServerEvent::Remote`].
    pub fn refuse(&mut self, now: Instant, text: &[u8]) -> Result<(), NoRequest> {
        if !self.is_requested() {
            return Err(NoRequest);
        }
        let data = prefix::encode_all(text, &Quoting::default());
        let bytes = Packet::new(self.seq, b'E', data).wire(BlockCheck::Type1, &Framing::default());
        self.queue.push_back((self.paused(now), bytes));
        self.go_idle();
        Ok(())
    }

    /// Refuse the file being received, right after its
    /// [`ServerEvent::FileStart`]: an E packet carrying `text` (in place of
    /// the ACK to the F if that is not written yet) and
    /// [`ServerEvent::Error`] ([`Error::Cancelled`]). Refused when no file
    /// is being received.
    pub fn reject(&mut self, now: Instant, text: &[u8]) -> Result<(), NoRequest> {
        if !matches!(self.phase, Phase::Receive { .. }) {
            return Err(NoRequest);
        }
        // The E answers the client's last packet (the F), by its number.
        self.seq = (self.seq + 63) % 64;
        self.fail(now, Error::Cancelled, Some(text));
        Ok(())
    }

    /// Abort a running transaction with an E packet "Cancelled" and
    /// [`ServerEvent::Error`] ([`Error::Cancelled`]); a waiting request is
    /// refused the same way. Nothing between transactions.
    pub fn cancel(&mut self, now: Instant) {
        if !self.is_idle() {
            self.fail(now, Error::Cancelled, Some(b"Cancelled"));
        }
    }

    // ---- helpers ----

    fn check(&self) -> BlockCheck {
        self.neg.map_or(BlockCheck::Type1, |n| n.check)
    }

    fn framing(&self) -> Framing {
        self.neg.map_or_else(Framing::default, |n| n.framing)
    }

    fn send_quoting(&self) -> Quoting {
        self.neg.map_or_else(Quoting::default, |n| n.send)
    }

    fn recv_quoting(&self) -> Quoting {
        self.neg.map_or_else(Quoting::default, |n| n.recv)
    }

    fn paused(&self, now: Instant) -> Instant {
        now.checked_add(self.config.packet_pause).unwrap_or(now)
    }

    fn nak(&self) -> Vec<u8> {
        Packet::new(self.seq, b'N', Vec::new()).wire(self.check(), &self.framing())
    }

    fn reply(&mut self, now: Instant, bytes: Vec<u8>) {
        self.last_sent = bytes.clone();
        self.deadline = None;
        let due = self.paused(now);
        self.queue.push_back((due, bytes));
    }

    fn reply_packet(&mut self, now: Instant, seq: u8, kind: u8, data: Vec<u8>) {
        let bytes = Packet::new(seq, kind, data).wire(self.check(), &self.framing());
        self.reply(now, bytes);
    }

    fn retry_with(&mut self, now: Instant, bytes: Vec<u8>) {
        self.retries += 1;
        if self.retries > self.config.retries {
            self.fail(now, Error::Timeout, Some(b"Too many retries"));
        } else {
            self.deadline = None;
            let due = self.paused(now);
            self.queue.push_back((due, bytes));
        }
    }

    fn go_idle(&mut self) {
        self.phase = Phase::Idle;
        self.deadline = None;
        self.retries = 0;
    }

    /// The transaction ends with `event`; the last packet sent answers a
    /// retransmission of the client's packet `(seq, kind)`.
    fn finish(&mut self, event: ServerEvent, answered: Option<(u8, u8)>) {
        self.events.push_back(event);
        self.go_idle();
        self.answered = answered;
    }

    fn fail(&mut self, now: Instant, err: Error, text: Option<&[u8]>) {
        self.queue.clear();
        if let Some(text) = text {
            let data = prefix::encode_all(text, &self.send_quoting());
            let bytes = Packet::new(self.seq, b'E', data).wire(self.check(), &self.framing());
            self.queue.push_back((now, bytes));
        }
        self.finish(ServerEvent::Error(err), None);
    }

    fn protocol(&mut self, now: Instant, msg: String, text: &[u8]) {
        self.fail(now, Error::Protocol(msg), Some(text));
    }

    fn remote(&mut self, data: &[u8], q: &Quoting) {
        let text = prefix::decode(data, q).unwrap_or_else(|_| data.to_vec());
        self.finish(ServerEvent::Remote(text), None);
    }

    fn decode(&mut self, now: Instant, data: &[u8], q: &Quoting) -> Option<Vec<u8>> {
        match prefix::decode(data, q) {
            Ok(v) => Some(v),
            Err(e) => {
                self.protocol(now, e.to_string(), b"Bad prefix");
                None
            }
        }
    }

    // ---- between transactions ----

    fn on_idle(&mut self, now: Instant, parsed: Result<Packet, FrameError>) {
        let Ok(p) = parsed else { return };
        if self.answered == Some((p.seq, p.kind)) && p.kind != b'E' && p.kind != b'N' {
            // A retransmission of what we answered last: the same answer.
            let due = self.paused(now);
            self.queue.push_back((due, self.last_sent.clone()));
            return;
        }
        match p.kind {
            b'I' => {
                self.neg = None;
                let bytes = Packet::new(p.seq, b'Y', self.ours.encode())
                    .wire(BlockCheck::Type1, &Framing::default());
                self.reply(now, bytes);
                self.answered = Some((p.seq, b'I'));
            }
            b'S' => {
                let theirs = InitParams::decode(&p.data);
                let n = negotiate(&self.ours, &theirs);
                // The ACK to S uses type 1 but already the client's framing.
                let bytes = Packet::new(p.seq, b'Y', self.ours.encode())
                    .wire(BlockCheck::Type1, &n.framing);
                self.reply(now, bytes);
                self.neg = Some(n);
                self.seq = (p.seq + 1) % 64;
                self.retries = 0;
                self.received = 0;
                self.answered = None;
                self.phase = Phase::Receive {
                    name: None,
                    data: Vec::new(),
                };
            }
            b'R' => {
                let Some(name) = self.decode(now, &p.data, &Quoting::default()) else {
                    return;
                };
                self.neg = None;
                self.seq = p.seq;
                self.answered = None;
                self.phase = Phase::Requested;
                self.events.push_back(ServerEvent::Request { name });
            }
            b'E' => {
                let text = prefix::decode(&p.data, &Quoting::default()).unwrap_or(p.data);
                self.events.push_back(ServerEvent::Remote(text));
            }
            b'N' | b'Y' => {}
            _ => {
                // G (generic), C (host command) and the rest: not served.
                let data = prefix::encode_all(b"Unimplemented", &Quoting::default());
                let bytes =
                    Packet::new(p.seq, b'E', data).wire(BlockCheck::Type1, &Framing::default());
                self.reply(now, bytes);
                self.answered = Some((p.seq, p.kind));
            }
        }
    }

    fn on_requested(&mut self, now: Instant, parsed: Result<Packet, FrameError>) {
        let Ok(p) = parsed else { return };
        match p.kind {
            b'E' => {
                let text = prefix::decode(&p.data, &Quoting::default()).unwrap_or(p.data);
                self.finish(ServerEvent::Remote(text), None);
            }
            // The client gave up on its request and starts over.
            b'I' | b'S' => {
                self.go_idle();
                self.on_idle(now, Ok(p));
            }
            // A retransmitted R (or anything else) waits for the caller.
            _ => {}
        }
    }

    // ---- receiving ----

    fn on_receive(&mut self, now: Instant, parsed: Result<Packet, FrameError>) {
        let n = self.seq;
        let p = match parsed {
            Ok(p) => p,
            Err(FrameError::BadCheck) => {
                let nak = self.nak();
                self.retry_with(now, nak);
                return;
            }
            Err(FrameError::BadLength) => return,
        };
        if p.kind == b'E' {
            let q = self.recv_quoting();
            self.remote(&p.data, &q);
            return;
        }
        if p.seq != n {
            if p.seq == (n + 63) % 64 && p.kind != b'N' {
                // A duplicate (the 38G and 39G send their S twice): the
                // same answer again, counted as a retry (only a new packet
                // resets the count), so a peer replaying one packet cannot
                // hold the transaction open.
                let bytes = self.last_sent.clone();
                self.retry_with(now, bytes);
            }
            return;
        }
        let q = self.recv_quoting();
        match p.kind {
            b'F' => {
                let Some(name) = self.decode(now, &p.data, &q) else {
                    return;
                };
                self.events.push_back(ServerEvent::FileStart {
                    name: name.clone(),
                    sending: false,
                });
                if let Phase::Receive { name: n, data } = &mut self.phase {
                    *n = Some(name);
                    data.clear();
                }
                // The ACK to F carries the name the file is stored under.
                self.reply_packet(now, n, b'Y', p.data);
            }
            b'D' => {
                let Some(chunk) = self.decode(now, &p.data, &q) else {
                    return;
                };
                self.received = self.received.saturating_add(chunk.len());
                if let Some(limit) = self.config.max_size
                    && self.received > limit
                {
                    self.fail(now, Error::TooLarge { limit }, Some(b"Too large"));
                    return;
                }
                let Phase::Receive {
                    name: Some(_),
                    data,
                } = &mut self.phase
                else {
                    self.protocol(now, "D before F".to_string(), b"Unexpected packet");
                    return;
                };
                data.extend_from_slice(&chunk);
                let done = data.len() as u64;
                self.events
                    .push_back(ServerEvent::Progress { done, total: None });
                self.reply_packet(now, n, b'Y', Vec::new());
            }
            b'Z' => {
                let Some(flag) = self.decode(now, &p.data, &q) else {
                    return;
                };
                let Phase::Receive { name, data } = &mut self.phase else {
                    return;
                };
                let Some(name) = name.take() else {
                    self.protocol(now, "Z before F".to_string(), b"Unexpected packet");
                    return;
                };
                let data = std::mem::take(data);
                self.events.push_back(if flag == b"D" {
                    ServerEvent::Discarded { name }
                } else {
                    ServerEvent::Received { name, data }
                });
                self.reply_packet(now, n, b'Y', Vec::new());
            }
            b'A' | b'X' => self.reply_packet(now, n, b'Y', Vec::new()),
            b'B' => {
                if matches!(&self.phase, Phase::Receive { name: Some(_), .. }) {
                    self.protocol(
                        now,
                        "B before the Z of the current file".to_string(),
                        b"Unexpected B",
                    );
                    return;
                }
                self.reply_packet(now, n, b'Y', Vec::new());
                self.finish(ServerEvent::Done, Some((n, b'B')));
                return;
            }
            b'N' => return,
            other => {
                self.protocol(
                    now,
                    format!("unexpected packet {:?}", char::from(other)),
                    b"Unexpected packet",
                );
                return;
            }
        }
        self.seq = (n + 1) % 64;
        self.retries = 0;
    }

    // ---- sending ----

    fn on_send(&mut self, now: Instant, parsed: Result<Packet, FrameError>, raw_seq: Option<u8>) {
        let n = self.seq;
        let p = match parsed {
            Ok(p) => p,
            Err(FrameError::BadCheck) => {
                if raw_seq == Some(n) {
                    let bytes = self.last_sent.clone();
                    self.retry_with(now, bytes);
                }
                return;
            }
            Err(FrameError::BadLength) => return,
        };
        match p.kind {
            b'E' => {
                let q = self.recv_quoting();
                self.remote(&p.data, &q);
            }
            b'Y' if p.seq == n => {
                let sent = std::mem::take(&mut self.last_sent);
                self.queue.retain(|(_, b)| *b != sent);
                self.last_sent = sent;
                self.on_ack(now, &p.data);
            }
            b'N' if p.seq == (n + 1) % 64 => self.on_ack(now, &[]),
            b'N' if p.seq == n => {
                let bytes = self.last_sent.clone();
                self.retry_with(now, bytes);
            }
            _ => {}
        }
    }

    fn sending(&mut self) -> Option<&mut Sending> {
        match &mut self.phase {
            Phase::Send(s) => Some(s),
            _ => None,
        }
    }

    fn send_next(&mut self, now: Instant, kind: u8, data: Vec<u8>) {
        self.seq = (self.seq + 1) % 64;
        self.reply_packet(now, self.seq, kind, data);
    }

    fn max_data(&self) -> usize {
        let maxl = self.neg.map_or(80, |n| usize::from(n.peer_maxl));
        Packet::max_data(maxl, self.check())
    }

    fn on_ack(&mut self, now: Instant, data: &[u8]) {
        self.retries = 0;
        let Some(step) = self.sending().map(|s| s.step) else {
            return;
        };
        match step {
            Step::Init => {
                let theirs = InitParams::decode(data);
                self.neg = Some(negotiate(&self.ours, &theirs));
                let max = self.max_data();
                let q = self.send_quoting();
                let Some(s) = self.sending() else { return };
                let (name, used) = prefix::encode(&s.file.name, &q, max);
                if used != s.file.name.len() {
                    self.protocol(
                        now,
                        format!("file name does not fit in a packet of {max} data bytes"),
                        b"File name too long",
                    );
                    return;
                }
                s.step = Step::File;
                self.send_next(now, b'F', name);
            }
            Step::File => {
                let name = if data.is_empty() {
                    self.sending()
                        .map(|s| s.file.name.clone())
                        .unwrap_or_default()
                } else {
                    let q = self.recv_quoting();
                    let Some(name) = self.decode(now, data, &q) else {
                        return;
                    };
                    name
                };
                self.events.push_back(ServerEvent::FileStart {
                    name,
                    sending: true,
                });
                self.next_data(now);
            }
            Step::Data { chunk } => {
                let Some(s) = self.sending() else { return };
                s.pos += chunk;
                let done = s.pos as u64;
                let total = s.file.data.len() as u64;
                self.events.push_back(ServerEvent::Progress {
                    done,
                    total: Some(total),
                });
                self.next_data(now);
            }
            Step::Eof => {
                let Some(s) = self.sending() else { return };
                s.step = Step::Break;
                let name = s.file.name.clone();
                self.events.push_back(ServerEvent::Sent { name });
                self.send_next(now, b'B', Vec::new());
            }
            Step::Break => self.finish(ServerEvent::Done, None),
        }
    }

    fn next_data(&mut self, now: Instant) {
        let max = self.max_data();
        let q = self.send_quoting();
        let Some(s) = self.sending() else { return };
        let rest = s.file.data.get(s.pos..).unwrap_or_default();
        if rest.is_empty() {
            s.step = Step::Eof;
            self.send_next(now, b'Z', Vec::new());
        } else {
            let (enc, used) = prefix::encode(rest, &q, max);
            s.step = Step::Data { chunk: used };
            self.send_next(now, b'D', enc);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
pub(crate) mod test_support {
    use super::*;
    use crate::trace::{self, Direction};

    /// Replay `text` at a fixed `now` against a new server: In lines are
    /// fed to it, Out lines must match its output. `answer(name)` answers
    /// each request: `Some(data)` serves it under that name, `None` refuses
    /// it with "File not found". Returns the server and its events.
    pub(crate) fn replay_server(
        config: ServerConfig,
        text: &str,
        answer: &dyn Fn(&[u8]) -> Option<Vec<u8>>,
    ) -> (Server, Vec<ServerEvent>) {
        let now = Instant::now();
        let mut s = Server::new(config);
        let mut events = Vec::new();
        for (i, (dir, bytes)) in trace::parse(text).unwrap().into_iter().enumerate() {
            match dir {
                Direction::Out => {
                    let got = s.poll_output(now);
                    assert_eq!(
                        got.as_deref().map(trace::escape),
                        Some(trace::escape(&bytes)),
                        "trace entry {i}: expected output > {}",
                        trace::escape(&bytes)
                    );
                }
                Direction::In => s.handle_input(now, &bytes),
            }
            while let Some(e) = s.poll_event() {
                if let ServerEvent::Request { name } = &e {
                    match answer(name) {
                        Some(data) => s
                            .serve(
                                now,
                                OutgoingFile {
                                    name: name.clone(),
                                    data,
                                },
                            )
                            .unwrap(),
                        None => s.refuse(now, b"File not found").unwrap(),
                    }
                }
                events.push(e);
            }
        }
        let extra = s.poll_output(now);
        assert_eq!(
            extra.as_deref().map(trace::escape),
            None,
            "unexpected trailing output"
        );
        (s, events)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::test_support::replay_server;
    use super::*;
    use crate::trace;

    const T1: BlockCheck = BlockCheck::Type1;
    const T3: BlockCheck = BlockCheck::Type3;
    /// The 38G's and 39G's Send-Init (wiki: protocols/aplet-transfer).
    const CALC: &[u8] = b"~* @-#Y3";
    /// The calculator's ACK to our S.
    const CALC_ACK: &[u8] = b"~& @-# 1";
    /// Our params with the default config.
    const OURS: &[u8] = b"~* @-#Y3 ";

    fn wire(seq: u8, kind: u8, data: &[u8], check: BlockCheck) -> Vec<u8> {
        Packet::new(seq, kind, data.to_vec())
            .encode(check, &Framing::default())
            .unwrap()
    }

    fn out(seq: u8, kind: u8, data: &[u8], check: BlockCheck) -> String {
        format!("> {}\n", trace::escape(&wire(seq, kind, data, check)))
    }

    fn inp(seq: u8, kind: u8, data: &[u8], check: BlockCheck) -> String {
        format!("< {}\n", trace::escape(&wire(seq, kind, data, check)))
    }

    fn none(_: &[u8]) -> Option<Vec<u8>> {
        None
    }

    #[test]
    fn our_params_encode() {
        assert_eq!(Server::new(ServerConfig::default()).params().encode(), OURS);
    }

    #[test]
    fn info_then_refused_request() {
        let mut t = inp(0, b'I', CALC, T1) + &out(0, b'Y', OURS, T1);
        // The calculator's I comes twice: the same answer twice.
        t += &inp(0, b'I', CALC, T1);
        t += &out(0, b'Y', OURS, T1);
        t += &inp(0, b'R', b"HP38DIR.CUR", T1);
        t += &out(0, b'E', b"File not found", T1);
        t += &inp(0, b'E', b"Transfer Failed#M#J", T1);
        let (s, ev) = replay_server(ServerConfig::default(), &t, &none);
        assert_eq!(
            ev,
            vec![
                ServerEvent::Request {
                    name: b"HP38DIR.CUR".to_vec()
                },
                ServerEvent::Remote(b"Transfer Failed\r\n".to_vec()),
            ]
        );
        assert!(s.is_idle());
        assert_eq!(s.next_timeout(), None);
    }

    #[test]
    fn serves_a_file() {
        let mut t = inp(0, b'R', b"FUNCTION.000", T1);
        t += &out(0, b'S', OURS, T1);
        t += &inp(0, b'Y', CALC_ACK, T1);
        t += &out(1, b'F', b"FUNCTION.000", T1);
        t += &inp(1, b'Y', b"FUNCTION.000", T1);
        t += &out(2, b'D', b"HP38BinB#H", T1);
        t += &inp(2, b'Y', b"", T1);
        t += &out(3, b'Z', b"", T1);
        t += &inp(3, b'Y', b"", T1);
        t += &out(4, b'B', b"", T1);
        t += &inp(4, b'Y', b"", T1);
        let answer = |_: &[u8]| Some(b"HP38BinB\x08".to_vec());
        let (s, ev) = replay_server(ServerConfig::default(), &t, &answer);
        let name = b"FUNCTION.000".to_vec();
        assert_eq!(
            ev,
            vec![
                ServerEvent::Request { name: name.clone() },
                ServerEvent::FileStart {
                    name: name.clone(),
                    sending: true
                },
                ServerEvent::Progress {
                    done: 9,
                    total: Some(9)
                },
                ServerEvent::Sent { name },
                ServerEvent::Done,
            ]
        );
        assert!(s.is_idle());
    }

    #[test]
    fn receives_a_file_with_crc_and_a_doubled_s() {
        let mut t = inp(0, b'S', CALC, T1) + &out(0, b'Y', OURS, T1);
        t += &inp(0, b'S', CALC, T1);
        t += &out(0, b'Y', OURS, T1);
        t += &inp(1, b'F', b"XYZ00000.000", T3);
        t += &out(1, b'Y', b"XYZ00000.000", T3);
        t += &inp(2, b'D', b"HP39BinB#CXYZ", T3);
        t += &out(2, b'Y', b"", T3);
        t += &inp(3, b'Z', b"", T3);
        t += &out(3, b'Y', b"", T3);
        t += &inp(4, b'B', b"", T3);
        t += &out(4, b'Y', b"", T3);
        // The B again (our ACK lost): the same ACK.
        t += &inp(4, b'B', b"", T3);
        t += &out(4, b'Y', b"", T3);
        let (s, ev) = replay_server(ServerConfig::default(), &t, &none);
        let name = b"XYZ00000.000".to_vec();
        assert_eq!(
            ev,
            vec![
                ServerEvent::FileStart {
                    name: name.clone(),
                    sending: false
                },
                ServerEvent::Progress {
                    done: 12,
                    total: None
                },
                ServerEvent::Received {
                    name,
                    data: b"HP39BinB\x03XYZ".to_vec()
                },
                ServerEvent::Done,
            ]
        );
        assert!(s.is_idle());
    }

    #[test]
    fn type1_when_asked() {
        let cfg = ServerConfig {
            block_check: T1,
            ..ServerConfig::default()
        };
        let ours = b"~* @-#Y1 ";
        let mut t = inp(0, b'S', CALC, T1) + &out(0, b'Y', ours, T1);
        t += &inp(1, b'F', b"A.000", T1);
        t += &out(1, b'Y', b"A.000", T1);
        t += &inp(2, b'Z', b"D", T1);
        t += &out(2, b'Y', b"", T1);
        t += &inp(3, b'B', b"", T1);
        t += &out(3, b'Y', b"", T1);
        let (_, ev) = replay_server(cfg, &t, &none);
        assert_eq!(
            ev,
            vec![
                ServerEvent::FileStart {
                    name: b"A.000".to_vec(),
                    sending: false
                },
                ServerEvent::Discarded {
                    name: b"A.000".to_vec()
                },
                ServerEvent::Done,
            ]
        );
    }

    #[test]
    fn generic_commands_are_refused() {
        let t = inp(0, b'G', b"D", T1) + &out(0, b'E', b"Unimplemented", T1);
        let (s, ev) = replay_server(ServerConfig::default(), &t, &none);
        assert!(ev.is_empty());
        assert!(s.is_idle());
    }

    #[test]
    fn calculator_error_ends_a_receive() {
        let mut t = inp(0, b'S', CALC, T1) + &out(0, b'Y', OURS, T1);
        t += &inp(1, b'E', b"Cancelled", T3);
        let (s, ev) = replay_server(ServerConfig::default(), &t, &none);
        assert_eq!(ev, vec![ServerEvent::Remote(b"Cancelled".to_vec())]);
        assert!(s.is_idle());
    }

    /// A file refused at its F: an E with the F's number instead of the
    /// ACK, and the transaction is over.
    #[test]
    fn a_rejected_file_gets_an_e_instead_of_the_ack() {
        let now = Instant::now();
        let mut s = Server::new(ServerConfig::default());
        s.handle_input(now, &wire(0, b'S', CALC, T1));
        assert_eq!(s.poll_output(now), Some(wire(0, b'Y', OURS, T1)));
        s.handle_input(now, &wire(1, b'F', b"GAME.000", T3));
        assert_eq!(
            s.poll_event(),
            Some(ServerEvent::FileStart {
                name: b"GAME.000".to_vec(),
                sending: false
            })
        );
        s.reject(now, b"File exists").unwrap();
        assert_eq!(s.poll_output(now), Some(wire(1, b'E', b"File exists", T3)));
        assert_eq!(s.poll_output(now), None);
        assert_eq!(s.poll_event(), Some(ServerEvent::Error(Error::Cancelled)));
        assert!(s.is_idle());
        assert_eq!(s.reject(now, b"x"), Err(NoRequest));
    }

    /// A client that replays its previous packet forever gets the same
    /// answer `retries` times, then the transaction ends with the retry
    /// error.
    #[test]
    fn a_replaying_client_runs_out_of_retries() {
        let now = Instant::now();
        let cfg = ServerConfig {
            retries: 2,
            ..ServerConfig::default()
        };
        let mut s = Server::new(cfg);
        s.handle_input(now, &wire(0, b'S', CALC, T1));
        assert_eq!(s.poll_output(now), Some(wire(0, b'Y', OURS, T1)));
        for _ in 0..2 {
            s.handle_input(now, &wire(0, b'S', CALC, T1));
            assert_eq!(s.poll_output(now), Some(wire(0, b'Y', OURS, T1)));
        }
        s.handle_input(now, &wire(0, b'S', CALC, T1));
        assert_eq!(
            s.poll_output(now),
            Some(wire(1, b'E', b"Too many retries", T3))
        );
        assert_eq!(s.poll_event(), Some(ServerEvent::Error(Error::Timeout)));
        assert!(s.is_idle());
    }

    #[test]
    fn receive_timeouts_nak_then_fail() {
        let now = Instant::now();
        let cfg = ServerConfig {
            retries: 1,
            ..ServerConfig::default()
        };
        let mut s = Server::new(cfg);
        s.handle_input(now, &wire(0, b'S', CALC, T1));
        assert_eq!(s.poll_output(now), Some(wire(0, b'Y', OURS, T1)));
        let t1 = s.next_timeout().unwrap();
        assert_eq!(t1, now + Duration::from_secs(10));
        s.handle_timeout(t1);
        assert_eq!(s.poll_output(t1), Some(wire(1, b'N', b"", T3)));
        let t2 = s.next_timeout().unwrap();
        s.handle_timeout(t2);
        assert_eq!(
            s.poll_output(t2),
            Some(wire(1, b'E', b"Too many retries", T3))
        );
        assert_eq!(s.poll_event(), Some(ServerEvent::Error(Error::Timeout)));
        assert!(s.is_idle());
        assert_eq!(s.next_timeout(), None);
    }

    #[test]
    fn send_resends_on_nak_and_timeout() {
        let now = Instant::now();
        let mut s = Server::new(ServerConfig::default());
        s.handle_input(now, &wire(0, b'R', b"A", T1));
        assert!(s.is_requested());
        assert_eq!(
            s.poll_event(),
            Some(ServerEvent::Request {
                name: b"A".to_vec()
            })
        );
        s.serve(
            now,
            OutgoingFile {
                name: b"A".to_vec(),
                data: b"x".to_vec(),
            },
        )
        .unwrap();
        let first = s.poll_output(now).unwrap();
        s.handle_input(now, &wire(0, b'N', b"", T1));
        assert_eq!(s.poll_output(now), Some(first.clone()));
        let t = s.next_timeout().unwrap();
        s.handle_timeout(t);
        assert_eq!(s.poll_output(t), Some(first));
        assert_eq!(
            s.serve(
                now,
                OutgoingFile {
                    name: vec![],
                    data: vec![]
                }
            ),
            Err(NoRequest)
        );
    }

    #[test]
    fn cancel_while_requested() {
        let now = Instant::now();
        let mut s = Server::new(ServerConfig::default());
        s.handle_input(now, &wire(0, b'R', b"A", T1));
        s.cancel(now);
        assert_eq!(s.poll_output(now), Some(wire(0, b'E', b"Cancelled", T1)));
        assert_eq!(
            s.poll_event(),
            Some(ServerEvent::Request {
                name: b"A".to_vec()
            })
        );
        assert_eq!(s.poll_event(), Some(ServerEvent::Error(Error::Cancelled)));
        assert!(s.is_idle());
    }

    #[test]
    fn a_seq_out_of_range_is_ignored() {
        let now = Instant::now();
        let mut s = Server::new(ServerConfig::default());
        for seq_byte in [0x1f_u8, 0x7e] {
            for kind in *b"SIR" {
                let mut frame = vec![crate::codec::SOH, crate::codec::tochar(3), seq_byte, kind];
                let chk = T1.compute(&frame[1..]);
                frame.extend_from_slice(&chk);
                frame.push(CR);
                s.handle_input(now, &frame);
            }
        }
        assert_eq!(s.poll_output(now), None);
        assert_eq!(s.poll_event(), None);
        assert!(s.is_idle());
    }
}
