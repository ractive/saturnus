//! Drives a [`kermit_proto::Client`] over a byte link.
//!
//! The loop is the one from `kermit-proto`'s docs: write each queued packet
//! with one write (the HP overruns on inter-byte gaps), handle the events,
//! read until the client's next deadline and feed input or a timeout back.
//! After `Done` or `Error` it keeps going until the final ACK or E packet is
//! out, and through the client's linger after a receive (re-ACKing a
//! repeated `B`). Input left over from the idle server (it NAKs once, a
//! server timeout after its last packet, and then stays silent) is drained
//! when the session is created.
//!
//! A no-progress watchdog is the backstop above the client's retries: a
//! transaction in which no new packet is accepted for longer than the
//! client could need to give up on one packet ([`stall_limit`]) is
//! cancelled with [`Error::Stalled`], also when the peer keeps the line
//! busy (a packet replayed over and over, noise at the right rate). The
//! limit counts time without progress on the caller's clock, so a long
//! transfer that moves on, or a host command the calculator computes for a
//! long time and answers within the timeout, never trips it.
//!
//! The loop is written once, as async code on the crate's link (see
//! [`machine`](crate::machine)); [`Session`] runs it over a [`Transport`]
//! with the system clock, [`Machine`](crate::Machine) with the caller's.

use kermit_proto::{Client, Command, Config, Event, StartError};

use crate::charset;
#[cfg(not(target_arch = "wasm32"))]
use crate::link;
use crate::link::{Link, Progress};
use crate::time::{Duration, Instant};
#[cfg(not(target_arch = "wasm32"))]
use crate::transport::{self, Transport};
use crate::watchdog::Watchdog;
use crate::{Error, Result};

/// Read wait while a transaction runs but the client has no deadline.
const IDLE_WAIT: Duration = Duration::from_millis(100);
/// Shortest read wait.
const MIN_WAIT: Duration = Duration::from_millis(10);
/// Read wait while discarding input that is already waiting before a host
/// command ([`Session::transact_with`]): long enough to collect what the
/// link has buffered, short enough to cost nothing noticeable.
const PENDING_WAIT: Duration = Duration::from_millis(1);
/// Most time spent discarding waiting input before a host command (a link
/// that never goes quiet must not stall the command).
const PENDING_MAX: Duration = Duration::from_millis(50);

/// How long a Kermit transaction may go without a new packet accepted
/// before the watchdog cancels it: one more try than the most the client
/// makes for one packet (its retries, or the first packet's with the one
/// resend an immediate NAK grants), each a receive timeout plus the packet
/// pause, and one more for slack.
pub(crate) fn stall_limit(config: &Config) -> Duration {
    let first = config
        .first_packet_retries
        .map_or(config.retries, |r| r.saturating_add(1));
    let tries = config.retries.max(first).saturating_add(2);
    config
        .timeout
        .saturating_add(config.packet_pause)
        .saturating_mul(tries)
}

/// Tunables for a [`Session`].
#[derive(Clone, Debug)]
pub struct Options {
    /// Kermit client configuration. The default lingers after a receive
    /// ([`Config::linger`]) as long as the turnaround, which the linger
    /// counts toward: no extra delay between transactions.
    pub kermit: Config,
    /// How long to discard input after opening (stale NAKs).
    pub drain: Duration,
    /// Pause between the end of one transaction and the start of the next.
    /// The HP drops a command packet that arrives right after the final ACK
    /// of the previous transaction, and only answers after its own timeout
    /// (about 6 s) NAKs it; 100 ms was enough on the emulator. Counted
    /// from the `Done` or `Error`, so the client's linger runs inside it.
    pub turnaround: Duration,
}

impl Default for Options {
    fn default() -> Self {
        let turnaround = Duration::from_millis(200);
        let mut kermit = Config::default();
        kermit.linger = turnaround;
        Options {
            kermit,
            drain: Duration::from_millis(500),
            turnaround,
        }
    }
}

/// A file received with a GET.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceivedFile {
    /// Name from the F packet, decoded from the HP character set.
    pub name: String,
    /// File contents.
    pub data: Vec<u8>,
}

/// What a transaction produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transcript {
    /// Received files (GET); discarded files are left out.
    pub files: Vec<ReceivedFile>,
    /// Names the calculator stored sent files under (SEND), decoded.
    pub stored_names: Vec<String>,
    /// All server text (host command and directory replies), raw HP bytes.
    pub text: Vec<u8>,
}

/// The Kermit side of a link, without the I/O: the client, its
/// configuration and the pacing between transactions.
#[derive(Debug)]
pub(crate) struct KermitCore {
    pub(crate) link: Link,
    client: Client,
    config: Config,
    turnaround: Duration,
    /// When the last transaction ended.
    last_end: Option<Instant>,
    /// See [`Session::answered`].
    answered: bool,
    /// See [`Machine::set_emulated_link`](crate::Machine::set_emulated_link).
    pub(crate) emulated_link: bool,
    /// The watchdog's limit instead of [`stall_limit`] of the config (tests
    /// shorten it below what the client's own retries take).
    pub(crate) stall_limit: Option<Duration>,
}

impl KermitCore {
    pub(crate) fn new(link: Link, options: &Options) -> Self {
        KermitCore {
            link,
            client: Client::new(options.kermit.clone()),
            config: options.kermit.clone(),
            turnaround: options.turnaround,
            last_end: None,
            answered: false,
            emulated_link: false,
            stall_limit: None,
        }
    }

    pub(crate) fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn set_config(&mut self, config: Config) {
        self.client = Client::new(config.clone());
        self.config = config;
    }

    pub(crate) fn answered(&self) -> bool {
        self.answered
    }

    /// Count a transaction as ended at `now` (an aborted one): the next
    /// waits for the turnaround.
    pub(crate) fn ended_at(&mut self, now: Instant) {
        self.last_end = Some(now);
    }

    /// Run one transaction to completion; every event goes to the link.
    pub(crate) async fn transact(&mut self, command: Command) -> Result<Transcript> {
        if let Some(end) = self.last_end {
            self.link.sleep_until(end + self.turnaround).await;
        }
        if matches!(command, Command::Host(_)) && !self.emulated_link {
            self.discard_pending().await?;
        }
        let result = self.run(command).await;
        self.answered = self.client.peer_params().is_some();
        if result.is_err() {
            self.client = Client::new(self.config.clone());
            self.last_end = Some(self.link.now());
        }
        result
    }

    /// Discard input that arrived between transactions, before a host
    /// command goes out: a stale NAK of the idle server (it NAKs once, a
    /// server timeout after its last packet, and then stays silent; measured
    /// on saturnus and saturnng, real hardware unverified) that would
    /// otherwise be read right after the `C`, inside the immediate-NAK
    /// window of host commands. Nothing of the coming transaction can be in
    /// it yet.
    async fn discard_pending(&mut self) -> Result<()> {
        let end = self.link.now() + PENDING_MAX;
        loop {
            let deadline = self.link.now() + PENDING_WAIT;
            let got = self.link.read(Some(deadline)).await?;
            if got.is_empty() || self.link.now() >= end {
                return Ok(());
            }
        }
    }

    async fn run(&mut self, command: Command) -> Result<Transcript> {
        let sending = matches!(command, Command::Send(_));
        let command_text = match &command {
            Command::Host(bytes) | Command::Get(bytes) => charset::decode(bytes),
            _ => String::new(),
        };
        self.client
            .start(self.link.now(), command)
            .map_err(|e| match e {
                StartError::TooLong { len, max } => Error::CommandTooLong {
                    command: command_text,
                    len,
                    max,
                },
                StartError::Busy => Error::Reply("kermit client busy".into()),
                other => Error::Reply(format!("kermit client: {other}")),
            })?;

        let limit = self
            .stall_limit
            .unwrap_or_else(|| stall_limit(&self.config));
        let mut watchdog = Watchdog::new(self.link.now(), limit, limit);
        let mut accepted = self.client.accepted();
        let mut stalled = false;

        let mut transcript = Transcript::default();
        let mut current: Option<ReceivedFile> = None;
        let mut outcome: Option<Result<()>> = None;
        'run: loop {
            let now = self.link.now();
            while let Some(packet) = self.client.poll_output(now) {
                match self.link.write(packet) {
                    Ok(()) => {}
                    // A re-ACK during the linger: the transaction is
                    // complete, a failing link does not undo it.
                    Err(_) if matches!(outcome, Some(Ok(()))) => break 'run,
                    Err(e) => return Err(e.into()),
                }
            }
            // Written before the events count: a failed write of the final
            // ACK fails the transaction, a re-ACK's only ends the linger.
            match self.link.flush().await {
                Ok(()) => {}
                Err(_) if matches!(outcome, Some(Ok(()))) => break 'run,
                Err(e) => return Err(e.into()),
            }
            while let Some(event) = self.client.poll_event() {
                self.link.emit(Progress::Kermit(event.clone()));
                match event {
                    Event::FileStart { name } if sending => {
                        transcript.stored_names.push(charset::decode(&name));
                    }
                    Event::FileStart { name } => {
                        current = Some(ReceivedFile {
                            name: charset::decode(&name),
                            data: Vec::new(),
                        });
                    }
                    Event::Data(data) => {
                        if let Some(file) = current.as_mut() {
                            file.data.extend_from_slice(&data);
                        }
                    }
                    Event::FileEnd { discarded } => {
                        if let Some(file) = current.take()
                            && !discarded
                        {
                            transcript.files.push(file);
                        }
                    }
                    Event::ServerText(text) => transcript.text.extend_from_slice(&text),
                    Event::Progress { .. } => {}
                    Event::Done => {
                        outcome = Some(Ok(()));
                        // The final ACK is out (it is queued with Done):
                        // the turnaround starts, the linger runs inside it.
                        self.last_end = Some(now);
                    }
                    Event::Error(kermit_proto::Error::Remote(text)) => {
                        outcome = Some(Err(Error::Remote(charset::decode(&text))));
                    }
                    Event::Error(kermit_proto::Error::Cancelled) if stalled => {
                        outcome = Some(Err(Error::Stalled(limit)));
                    }
                    Event::Error(e) => outcome = Some(Err(Error::Kermit(e))),
                    _ => {}
                }
            }
            if outcome.is_none() && watchdog.expired(now) {
                // The E packet goes out and Error(Cancelled) comes next.
                stalled = true;
                self.client.cancel(now);
                continue;
            }
            let deadline = match (self.client.next_timeout(), &outcome) {
                (None, Some(_)) => break,
                (Some(t), _) => t,
                (None, None) => now + IDLE_WAIT,
            };
            let deadline = match watchdog.deadline() {
                Some(w) if outcome.is_none() => deadline.min(w),
                _ => deadline,
            };
            let input = match self.link.read(Some(deadline.max(now + MIN_WAIT))).await {
                Ok(input) => input,
                // Lingering after Done: the transcript is complete, a
                // failing link only ends the linger.
                Err(_) if matches!(outcome, Some(Ok(()))) => break,
                Err(e) => return Err(e.into()),
            };
            let now = self.link.now();
            if !input.is_empty() {
                self.client.handle_input(now, &input);
            }
            // Also after input: handle_input does not check the deadline, so
            // a peer that keeps sending garbage (noise, wrong speed) would
            // otherwise stall the transaction forever. A no-op until the
            // deadline has passed.
            self.client.handle_timeout(now);
            if self.client.accepted() != accepted {
                accepted = self.client.accepted();
                watchdog.progress(now);
            }
        }
        match outcome {
            Some(Err(e)) => Err(e),
            _ => Ok(transcript),
        }
    }
}

/// A Kermit connection to a calculator in server mode, over a blocking
/// [`Transport`] with the system clock. For a link driven from outside (a
/// browser), see [`machine::Machine`](crate::machine::Machine).
#[cfg(not(target_arch = "wasm32"))]
pub struct Session {
    pub(crate) transport: Box<dyn Transport>,
    pub(crate) core: KermitCore,
}

// The transport is a trait object without `Debug`.
#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Session {
    /// Wrap `transport`, discarding input for `options.drain` first.
    pub fn new(mut transport: Box<dyn Transport>, options: Options) -> Result<Self> {
        let link = Link::new(Instant::now());
        link::drive(
            transport.as_mut(),
            &link,
            link.drain(options.drain),
            &mut |_| {},
        )?;
        Ok(Session {
            transport,
            core: KermitCore::new(link, &options),
        })
    }

    /// Open `addr` (see [`transport::open`]) with default [`Options`].
    pub fn open(addr: &str) -> Result<Self> {
        Session::new(transport::open(addr)?, Options::default())
    }

    /// Give the link back, e.g. for an XModem transfer after
    /// [`Calculator::prepare_for_xmodem`](crate::Calculator::prepare_for_xmodem)
    /// ended server mode.
    pub fn into_transport(self) -> Box<dyn Transport> {
        self.transport
    }

    /// The Kermit configuration in use.
    pub fn config(&self) -> &Config {
        self.core.config()
    }

    /// Use `config` from the next transaction on.
    pub fn set_config(&mut self, config: Config) {
        self.core.set_config(config);
    }

    /// Whether the server sent its parameters (its `S`, or the ACK to an
    /// `I` or `S`) in the last transaction, also when it failed later: for
    /// a host command, whether the `C` was received and answered.
    pub fn answered(&self) -> bool {
        self.core.answered()
    }

    /// Run one transaction to completion.
    pub fn transact(&mut self, command: Command) -> Result<Transcript> {
        self.transact_with(command, &mut |_| {})
    }

    /// Run one transaction, passing every event to `progress`.
    pub fn transact_with(
        &mut self,
        command: Command,
        progress: &mut dyn FnMut(&Event),
    ) -> Result<Transcript> {
        let link = self.core.link.clone();
        link::drive(
            self.transport.as_mut(),
            &link,
            self.core.transact(command),
            &mut |p| {
                if let Progress::Kermit(event) = p {
                    progress(event);
                }
            },
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::MemoryTransport;
    use kermit_proto::OutgoingFile;
    use kermit_proto::codec::{BlockCheck, Deframer, Framing, Packet, parse_frame};
    use std::sync::{Arc, Mutex};

    const SX_HOST: &str = include_str!("../../kermit-proto/traces/48sx-host.trace");
    const SX_GET: &str = include_str!("../../kermit-proto/traces/48sx-get.trace");
    const SX_GET_MISSING: &str = include_str!("../../kermit-proto/traces/48sx-get-missing.trace");
    const SX_SEND: &str = include_str!("../../kermit-proto/traces/48sx-send.trace");
    const GX_GET_BINARY: &str = include_str!("../../kermit-proto/traces/48gx-get-binary.trace");

    /// No pauses: zero drain and turnaround, and no linger after a receive
    /// (the in-memory peers never repeat a B).
    fn options() -> Options {
        let mut kermit = Options::default().kermit;
        kermit.linger = Duration::ZERO;
        Options {
            kermit,
            drain: Duration::ZERO,
            turnaround: Duration::ZERO,
        }
    }

    fn session(trace: &str, options: Options) -> Session {
        Session::new(
            Box::new(MemoryTransport::from_trace(trace).unwrap()),
            options,
        )
        .unwrap()
    }

    fn host_reply() -> Vec<u8> {
        let mut text = b"1:".to_vec();
        text.extend_from_slice(&[b' '; 20]);
        text.extend_from_slice(b"42\r\n");
        text
    }

    #[test]
    fn host_command() {
        let mut s = session(SX_HOST, options());
        let mut events = 0;
        let t = s
            .transact_with(Command::Host(b"6 7 *".to_vec()), &mut |_| events += 1)
            .unwrap();
        assert_eq!(t.text, host_reply());
        assert!(t.files.is_empty() && t.stored_names.is_empty());
        assert_eq!(events, 2);
    }

    #[test]
    fn get_missing_is_remote_error() {
        let mut s = session(SX_GET_MISSING, options());
        let err = s.transact(Command::Get(b"NOSUCH".to_vec())).unwrap_err();
        assert!(
            matches!(&err, Error::Remote(m) if m == "Undefined Name"),
            "{err:?}"
        );
    }

    #[test]
    fn get_ascii() {
        let mut s = session(SX_GET, options());
        let t = s.transact(Command::Get(b"ALLB".to_vec())).unwrap();
        assert_eq!(t.files.len(), 1);
        assert_eq!(t.files[0].name, "ALLB");
        assert!(t.files[0].data.starts_with(b"%%HP: T(1)A(D)F(.);\r\n"));
    }

    #[test]
    fn get_binary() {
        let mut s = session(GX_GET_BINARY, options());
        let t = s.transact(Command::Get(b"ALLB".to_vec())).unwrap();
        assert_eq!(t.files.len(), 1);
        assert!(t.files[0].data.starts_with(b"HPHP48-R"));
    }

    #[test]
    fn send_reports_stored_name() {
        let mut s = session(SX_SEND, options());
        let file = OutgoingFile {
            name: b"ALLB".to_vec(),
            data: (0..=255).collect(),
        };
        let t = s.transact(Command::Send(vec![file])).unwrap();
        assert_eq!(t.stored_names, vec!["ALLB".to_string()]);
        assert!(t.files.is_empty());
    }

    #[test]
    fn turnaround_pause_between_transactions() {
        let trace = format!("{SX_HOST}\n{SX_HOST}");
        let options = Options {
            turnaround: Duration::from_millis(50),
            ..options()
        };
        let mut s = session(&trace, options);
        s.transact(Command::Host(b"6 7 *".to_vec())).unwrap();
        let start = Instant::now();
        s.transact(Command::Host(b"6 7 *".to_vec())).unwrap();
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn stale_nak_drained() {
        let trace = format!("< \\x01# N3\\r\n{SX_HOST}");
        let mut s = session(
            &trace,
            Options {
                drain: Duration::from_millis(50),
                turnaround: Duration::ZERO,
                ..options()
            },
        );
        let t = s.transact(Command::Host(b"6 7 *".to_vec())).unwrap();
        assert_eq!(t.text, host_reply());
    }

    /// PR #26 review, #1: input that arrived between transactions (here a
    /// stray packet after the first reply; on the link, the idle server's
    /// NAKs) is discarded before a host command goes out, so it is not read
    /// as the first answer to the `C`.
    #[test]
    fn input_between_host_commands_is_discarded() {
        use kermit_proto::codec::{Framing, Packet};
        let line = |dir: &str, kind: u8, data: &[u8]| {
            let p = Packet::new(0, kind, data.to_vec())
                .encode(kermit_proto::BlockCheck::Type1, &Framing::default())
                .unwrap();
            format!("{dir} {}\n", kermit_proto::trace::escape(&p))
        };
        let trace = line(">", b'C', b"A")
            + &line("<", b'Y', b"a")
            + &line("<", b'Y', b"stale")
            + &line(">", b'C', b"B")
            + &line("<", b'Y', b"b");
        let mut s = session(&trace, options());
        assert_eq!(s.transact(Command::Host(b"A".to_vec())).unwrap().text, b"a");
        assert_eq!(s.transact(Command::Host(b"B".to_vec())).unwrap().text, b"b");
    }

    #[test]
    fn error_resets_client() {
        let mut s = session(SX_HOST, options());
        let err = s.transact(Command::Host(b"6 7 +".to_vec())).unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err:?}");
        // The mismatching write was not consumed, so a clean client can
        // replay the trace from the start.
        let t = s.transact(Command::Host(b"6 7 *".to_vec())).unwrap();
        assert_eq!(t.text, host_reply());
    }

    #[test]
    fn command_too_long() {
        let mut s = session("", options());
        let err = s.transact(Command::Host(vec![b'1'; 200])).unwrap_err();
        assert!(
            matches!(&err, Error::CommandTooLong { command, len, .. } if command.len() == 200 && *len == 200),
            "{err:?}"
        );
    }

    /// A Kermit server that sends file `X` (two D packets) in reply to R and
    /// drops the first transmission of D seq 2.
    fn lossy_server(naks: Arc<Mutex<Vec<u8>>>) -> impl FnMut(&[u8]) -> Vec<Vec<u8>> + Send {
        let check = BlockCheck::Type1;
        let mut deframer = Deframer::new();
        let mut dropped = false;
        move |bytes| {
            let wire = |seq: u8, kind: u8, data: &[u8]| {
                Packet::new(seq, kind, data.to_vec())
                    .encode(check, &Framing::default())
                    .unwrap()
            };
            deframer.push(bytes);
            let mut replies = Vec::new();
            while let Some(frame) = deframer.next_frame() {
                let p = parse_frame(&frame, check).unwrap();
                match (p.kind, p.seq) {
                    (b'R', _) => replies.push(wire(0, b'S', b"~* @-#Y1 ")),
                    (b'Y', 0) => replies.push(wire(1, b'F', b"X")),
                    (b'Y', 1) if !dropped => dropped = true,
                    (b'Y', 1) => {}
                    (b'N', 2) => {
                        naks.lock().unwrap().push(2);
                        replies.push(wire(2, b'D', b"hello "));
                    }
                    (b'Y', 2) => replies.push(wire(3, b'D', b"world")),
                    (b'Y', 3) => replies.push(wire(4, b'Z', b"")),
                    (b'Y', 4) => replies.push(wire(5, b'B', b"")),
                    (b'Y', 5) => {}
                    (kind, seq) => panic!("unexpected packet {} seq {seq}", char::from(kind)),
                }
            }
            replies
        }
    }

    /// A server that sends file `X` ("hi") in reply to R, and a link that
    /// fails every read once our ACK to its B is out.
    struct LostAfterFinalAck {
        inner: MemoryTransport,
        acked: Arc<Mutex<bool>>,
    }

    impl Transport for LostAfterFinalAck {
        fn write_packet(&mut self, packet: &[u8]) -> std::io::Result<()> {
            self.inner.write_packet(packet)
        }

        fn read(&mut self, buf: &mut [u8], timeout: Duration) -> std::io::Result<usize> {
            if *self.acked.lock().unwrap() {
                return Err(std::io::Error::other("link lost"));
            }
            self.inner.read(buf, timeout)
        }
    }

    #[test]
    fn link_lost_while_lingering_keeps_the_transcript() {
        let acked = Arc::new(Mutex::new(false));
        let flag = Arc::clone(&acked);
        let check = BlockCheck::Type1;
        let mut deframer = Deframer::new();
        let server = move |bytes: &[u8]| {
            let wire = |seq: u8, kind: u8, data: &[u8]| {
                Packet::new(seq, kind, data.to_vec())
                    .encode(check, &Framing::default())
                    .unwrap()
            };
            deframer.push(bytes);
            let mut replies = Vec::new();
            while let Some(frame) = deframer.next_frame() {
                let p = parse_frame(&frame, check).unwrap();
                match (p.kind, p.seq) {
                    (b'R', _) => replies.push(wire(0, b'S', b"~* @-#Y1 ")),
                    (b'Y', 0) => replies.push(wire(1, b'F', b"X")),
                    (b'Y', 1) => replies.push(wire(2, b'D', b"hi")),
                    (b'Y', 2) => replies.push(wire(3, b'Z', b"")),
                    (b'Y', 3) => replies.push(wire(4, b'B', b"")),
                    (b'Y', 4) => *flag.lock().unwrap() = true,
                    (kind, seq) => panic!("unexpected packet {} seq {seq}", char::from(kind)),
                }
            }
            replies
        };
        let mut kermit = Options::default().kermit;
        kermit.linger = Duration::from_secs(5);
        let options = Options {
            kermit,
            ..options()
        };
        let transport = LostAfterFinalAck {
            inner: MemoryTransport::new(server),
            acked: Arc::clone(&acked),
        };
        let mut s = Session::new(Box::new(transport), options).unwrap();
        let start = Instant::now();
        let t = s.transact(Command::Get(b"X".to_vec())).unwrap();
        assert!(*acked.lock().unwrap());
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "linger ended early"
        );
        assert_eq!(
            t.files,
            vec![ReceivedFile {
                name: "X".into(),
                data: b"hi".to_vec()
            }]
        );
    }

    /// A link that fails every write once the server's B is out: the ACK
    /// to the B, which comes with Done.
    struct FinalAckLost {
        inner: MemoryTransport,
        b_sent: Arc<Mutex<bool>>,
    }

    impl Transport for FinalAckLost {
        fn write_packet(&mut self, packet: &[u8]) -> std::io::Result<()> {
            if *self.b_sent.lock().unwrap() {
                return Err(std::io::Error::other("link lost"));
            }
            self.inner.write_packet(packet)
        }

        fn read(&mut self, buf: &mut [u8], timeout: Duration) -> std::io::Result<usize> {
            self.inner.read(buf, timeout)
        }
    }

    /// The final ACK's write fails the transaction: it is written before
    /// Done counts, as when the loop wrote to the transport directly.
    #[test]
    fn final_ack_lost_fails_the_transaction() {
        let b_sent = Arc::new(Mutex::new(false));
        let flag = Arc::clone(&b_sent);
        let check = BlockCheck::Type1;
        let mut deframer = Deframer::new();
        let server = move |bytes: &[u8]| {
            let wire = |seq: u8, kind: u8, data: &[u8]| {
                Packet::new(seq, kind, data.to_vec())
                    .encode(check, &Framing::default())
                    .unwrap()
            };
            deframer.push(bytes);
            let mut replies = Vec::new();
            while let Some(frame) = deframer.next_frame() {
                let p = parse_frame(&frame, check).unwrap();
                match (p.kind, p.seq) {
                    (b'R', _) => replies.push(wire(0, b'S', b"~* @-#Y1 ")),
                    (b'Y', 0) => replies.push(wire(1, b'F', b"X")),
                    (b'Y', 1) => replies.push(wire(2, b'D', b"hi")),
                    (b'Y', 2) => replies.push(wire(3, b'Z', b"")),
                    (b'Y', 3) => {
                        *flag.lock().unwrap() = true;
                        replies.push(wire(4, b'B', b""));
                    }
                    (kind, seq) => panic!("unexpected packet {} seq {seq}", char::from(kind)),
                }
            }
            replies
        };
        let transport = FinalAckLost {
            inner: MemoryTransport::new(server),
            b_sent,
        };
        let mut s = Session::new(Box::new(transport), options()).unwrap();
        let err = s.transact(Command::Get(b"X".to_vec())).unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err:?}");
    }

    /// Reads that never stop delivering noise: no SOH, so no frame ever
    /// completes. Fails the read after `MAX_READS` so a regression cannot
    /// hang the test.
    struct NoisyLine {
        reads: usize,
    }

    const MAX_READS: usize = 5_000;

    impl Transport for NoisyLine {
        fn write_packet(&mut self, _packet: &[u8]) -> std::io::Result<()> {
            Ok(())
        }

        fn read(&mut self, buf: &mut [u8], _timeout: Duration) -> std::io::Result<usize> {
            self.reads += 1;
            if self.reads > MAX_READS {
                return Err(std::io::Error::other("read cap hit: timeout never fired"));
            }
            std::thread::sleep(Duration::from_millis(1));
            let noise = b"zq~";
            buf[..noise.len()].copy_from_slice(noise);
            Ok(noise.len())
        }
    }

    #[test]
    fn continuous_garbage_still_times_out() {
        let mut kermit = Options::default().kermit;
        kermit.timeout = Duration::from_millis(20);
        kermit.retries = 2;
        let options = Options {
            kermit,
            drain: Duration::ZERO,
            turnaround: Duration::ZERO,
        };
        let mut s = Session {
            transport: Box::new(NoisyLine { reads: 0 }),
            core: KermitCore::new(Link::new(Instant::now()), &options),
        };
        let start = Instant::now();
        let err = s.transact(Command::Host(b"6 7 *".to_vec())).unwrap_err();
        assert!(
            matches!(err, Error::Kermit(kermit_proto::Error::Timeout)),
            "{err:?}"
        );
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
    }

    /// A Kermit server that sends file `X` in reply to R, then answers
    /// every ACK of its F with the F again, forever: the previous packet
    /// replayed. Logs the E packets it gets.
    fn replaying_server(errors: Arc<Mutex<Vec<Vec<u8>>>>) -> impl FnMut(&[u8]) -> Vec<Vec<u8>> {
        let check = BlockCheck::Type1;
        let mut deframer = Deframer::new();
        move |bytes| {
            let wire = |seq: u8, kind: u8, data: &[u8]| {
                Packet::new(seq, kind, data.to_vec())
                    .encode(check, &Framing::default())
                    .unwrap()
            };
            deframer.push(bytes);
            let mut replies = Vec::new();
            while let Some(frame) = deframer.next_frame() {
                let p = parse_frame(&frame, check).unwrap();
                match (p.kind, p.seq) {
                    (b'R', _) => replies.push(wire(0, b'S', b"~* @-#Y1 ")),
                    (b'Y', 0 | 1) => replies.push(wire(1, b'F', b"X")),
                    (b'E', _) => errors.lock().unwrap().push(p.data.clone()),
                    (kind, seq) => panic!("unexpected packet {} seq {seq}", char::from(kind)),
                }
            }
            replies
        }
    }

    fn session_with(
        transport: Box<dyn Transport>,
        kermit: Config,
        stall: Option<Duration>,
    ) -> Session {
        let options = Options {
            kermit,
            drain: Duration::ZERO,
            turnaround: Duration::ZERO,
        };
        let mut core = KermitCore::new(Link::new(Instant::now()), &options);
        core.stall_limit = stall;
        Session { transport, core }
    }

    /// Iteration 48: a peer that replays its last packet forever, to a
    /// client whose retries never run out, is cancelled by the watchdog
    /// once no new packet came for the stall limit, with an E packet.
    #[test]
    fn a_replaying_peer_is_cancelled_by_the_watchdog() {
        let errors = Arc::new(Mutex::new(Vec::new()));
        let transport = MemoryTransport::new(replaying_server(Arc::clone(&errors)));
        let mut kermit = options().kermit;
        kermit.timeout = Duration::from_secs(2);
        kermit.retries = u32::MAX;
        let limit = Duration::from_millis(100);
        let mut s = session_with(Box::new(transport), kermit, Some(limit));
        let start = Instant::now();
        let err = s.transact(Command::Get(b"X".to_vec())).unwrap_err();
        let elapsed = start.elapsed();
        assert!(matches!(err, Error::Stalled(l) if l == limit), "{err:?}");
        assert!(err.to_string().contains("no progress for 0.1 s"), "{err}");
        assert!(
            elapsed >= limit && elapsed < Duration::from_secs(1),
            "{elapsed:?}"
        );
        assert_eq!(*errors.lock().unwrap(), vec![b"Cancelled".to_vec()]);
    }

    /// Iteration 48: noise that never forms a packet, to a client whose
    /// retries never run out, is cancelled by the watchdog.
    #[test]
    fn a_babbling_line_is_cancelled_by_the_watchdog() {
        let mut kermit = options().kermit;
        kermit.timeout = Duration::from_millis(20);
        kermit.retries = u32::MAX;
        let limit = Duration::from_millis(150);
        let mut s = session_with(Box::new(NoisyLine { reads: 0 }), kermit, Some(limit));
        let start = Instant::now();
        let err = s.transact(Command::Host(b"6 7 *".to_vec())).unwrap_err();
        let elapsed = start.elapsed();
        assert!(matches!(err, Error::Stalled(_)), "{err:?}");
        assert!(
            elapsed >= limit && elapsed < Duration::from_secs(2),
            "{elapsed:?}"
        );
    }

    /// Iteration 48: with the limit from the configuration, a replaying peer
    /// ends within it (the client's retries end it first: a repeated packet
    /// counts as a retry).
    #[test]
    fn a_replaying_peer_ends_within_the_stall_limit() {
        let errors = Arc::new(Mutex::new(Vec::new()));
        let transport = MemoryTransport::new(replaying_server(Arc::clone(&errors)));
        let mut kermit = options().kermit;
        kermit.timeout = Duration::from_millis(50);
        kermit.retries = 3;
        let limit = stall_limit(&kermit);
        assert_eq!(limit, Duration::from_millis(250));
        let mut s = session_with(Box::new(transport), kermit, None);
        let start = Instant::now();
        let err = s.transact(Command::Get(b"X".to_vec())).unwrap_err();
        assert!(
            matches!(
                err,
                Error::Kermit(kermit_proto::Error::Timeout) | Error::Stalled(_)
            ),
            "{err:?}"
        );
        assert!(start.elapsed() < limit, "{:?}", start.elapsed());
    }

    /// The limit: one try more than the client's most for one packet, each
    /// a timeout plus the packet pause; the first packet's retries count
    /// when they are more.
    #[test]
    fn the_stall_limit_covers_every_retry() {
        let mut c = Config::default();
        assert_eq!(stall_limit(&c), Duration::from_secs(20 * 7));
        c.first_packet_retries = Some(0);
        assert_eq!(stall_limit(&c), Duration::from_secs(20 * 7));
        c.first_packet_retries = Some(9);
        assert_eq!(stall_limit(&c), Duration::from_secs(20 * 12));
        c.packet_pause = Duration::from_secs(1);
        assert_eq!(stall_limit(&c), Duration::from_secs(21 * 12));
        c.retries = u32::MAX;
        assert_eq!(
            stall_limit(&c),
            Duration::from_secs(21 * u64::from(u32::MAX))
        );
    }

    #[test]
    fn lost_packet_is_recovered() {
        let naks = Arc::new(Mutex::new(Vec::new()));
        let transport = MemoryTransport::new(lossy_server(Arc::clone(&naks)));
        let mut kermit = options().kermit;
        kermit.timeout = Duration::from_millis(100);
        let options = Options {
            kermit,
            drain: Duration::ZERO,
            turnaround: Duration::ZERO,
        };
        let mut s = Session::new(Box::new(transport), options).unwrap();
        let start = Instant::now();
        let t = s.transact(Command::Get(b"X".to_vec())).unwrap();
        assert!(start.elapsed() < Duration::from_millis(800));
        assert_eq!(
            t.files,
            vec![ReceivedFile {
                name: "X".into(),
                data: b"hello world".to_vec()
            }]
        );
        assert_eq!(*naks.lock().unwrap(), vec![2]);
    }
}
