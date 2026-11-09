//! The client state machine: one request at a time, HP's timing and
//! retries (wiki: protocols/filer-hplx, "Connect and disconnect" and
//! "Timeouts and retries").
//!
//! - CONNECT (command 0x40, counter 0) waits 2 s for the first byte of the
//!   reply and is sent up to three times; when all three fail the whole
//!   round runs once more, so up to six sends. A reply with status 0xFF3B
//!   in revision 2 makes the client try once more in revision 1.
//! - Every other request waits 5.9 s for the first byte of its reply and
//!   is sent up to three times, the same frame with the same counter each
//!   time (the server answers a repeated counter with its previous reply
//!   without running the command again, so a resent read or write is
//!   safe). Once the first byte has come the whole frame must follow
//!   within a window that depends on the speed ([`frame_window`]).
//! - The counter starts at 1 after CONNECT and advances only after a
//!   matching reply; after 0xFF comes 0x00.
//! - A reply with the wrong counter or direction byte or no reply after
//!   all sends ends the session: the client sends one DISCONNECT and
//!   reports [`Error`]. A CONNECT refusal to an ordinary request (the
//!   server left the session) ends it too, but without the DISCONNECT: the
//!   server has no session to end and would only answer with another
//!   refusal, which the next CONNECT could take for its own.
//! - DISCONNECT (0x41, counter 0) is sent once; its reply is not awaited.
//!
//! Two deliberate differences from HP's client, both only more forgiving:
//! a frame that arrives broken (bad CRC, illegal DLE) is resent at once
//! instead of after the timeout (unless a good reply came with it or
//! another frame is still arriving), and a late duplicate of the previous
//! reply (same command and counter) is skipped instead of ending the
//! session.

use std::collections::VecDeque;
use std::fmt;

use crate::codec::{DeframeError, Deframer, Frame, REPLY, REQUEST, REVISION};
use crate::command::{Code, Malformed, Request, Response};
use crate::error::{Failure, status};
use crate::time::{Duration, Instant};

/// Tunables of a [`Client`].
///
/// Non-exhaustive: start from [`Config::default`] and set the fields you
/// need.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Config {
    /// The line speed, for the frame window and the time a frame takes to
    /// go out (default 19200).
    pub baud: u32,
    /// The wait for the first byte of a CONNECT reply (default 2.0 s).
    pub connect_wait: Duration,
    /// The wait for the first byte of any other reply (default 5.9 s).
    pub reply_wait: Duration,
    /// The window for the rest of a reply once its first byte came;
    /// `None` (default) takes [`frame_window`] of the speed.
    pub frame_window: Option<Duration>,
    /// Sends of the same frame before a request fails (default 3).
    pub attempts: u32,
    /// Rounds of `attempts` sends for CONNECT (default 2).
    pub connect_rounds: u32,
    /// The protocol revision to ask for (default 2).
    pub revision: u8,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            baud: 19200,
            connect_wait: ticks(36),
            reply_wait: ticks(108),
            frame_window: None,
            attempts: 3,
            connect_rounds: 2,
            revision: REVISION,
        }
    }
}

/// PC clock ticks (1/18.2 s) as a duration.
const fn ticks(n: u64) -> Duration {
    Duration::from_micros(n * 10_000_000 / 182)
}

/// HP's window for a whole reply once its first byte came: 2.0 s at 19200
/// and faster, 5.0 s at 9600, 5.9 s at 4800, 11.9 s at 2400, 24.7 s at
/// 1200, 79 s below.
pub fn frame_window(baud: u32) -> Duration {
    ticks(match baud {
        19200.. => 36,
        9600.. => 91,
        4800.. => 108,
        2400.. => 216,
        1200.. => 450,
        _ => 1440,
    })
}

/// The time `bytes` take on the wire at `baud` (10 bits a byte).
fn wire_time(bytes: usize, baud: u32) -> Duration {
    let baud = u64::from(baud.max(1));
    Duration::from_micros((bytes as u64).saturating_mul(10_000_000) / baud)
}

/// Why the session ended.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// No reply, or no complete valid frame, after every send (HP's
    /// 0xFF39).
    NoReply {
        /// The request.
        code: Code,
        /// How many times it was sent.
        sends: u32,
    },
    /// A reply with the wrong counter, command or direction byte (HP's
    /// 0xFF3A).
    BadReply {
        /// The request.
        code: Code,
        /// Its counter.
        counter: u8,
        /// The reply's command byte.
        got_command: u8,
        /// The reply's counter.
        got_counter: u8,
        /// The reply's direction byte.
        got_direction: u8,
    },
    /// The server answered an ordinary request with a CONNECT refusal: it
    /// has no session open (it left it, or was restarted).
    NotConnected {
        /// How many times the request had been sent when the refusal
        /// came. 1: the refusal answered the first send, so the server ran
        /// nothing of it; more: an earlier send may have run before the
        /// session ended, its reply lost.
        sends: u32,
    },
    /// CONNECT was refused with this status.
    Refused(u16),
    /// A reply that does not fit its command's layout.
    Malformed(Malformed),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoReply { code, sends } => {
                write!(f, "no reply to {code:?} after {sends} sends")
            }
            Error::BadReply {
                code,
                counter,
                got_command,
                got_counter,
                got_direction,
            } => write!(
                f,
                "reply out of step: sent {code:?} with counter {counter}, got command {got_command:#04x} counter {got_counter} direction {got_direction:#04x}"
            ),
            Error::NotConnected { .. } => f.write_str("the server has no session open"),
            Error::Refused(code) => match crate::error::status_meaning(*code) {
                Some(m) => write!(f, "connect refused: {m} (status {code:#06x})"),
                None => write!(f, "connect refused with status {code:#06x}"),
            },
            Error::Malformed(m) => m.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

/// Why a call could not start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartError {
    /// A request is still waiting for its reply.
    Busy,
    /// No session is open: [`Client::connect`] first.
    NotConnected,
    /// CONNECT and DISCONNECT go through [`Client::connect`] and
    /// [`Client::disconnect`].
    SessionCommand,
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StartError::Busy => "a request is still waiting for its reply",
            StartError::NotConnected => "no session open",
            StartError::SessionCommand => "connect and disconnect have their own calls",
        })
    }
}

impl std::error::Error for StartError {}

/// What the client reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The session is open, in this revision.
    Connected {
        /// 2, or 1 after a fallback.
        revision: u8,
    },
    /// The reply to the request: its values, or why the server refused it.
    /// The session stays open either way.
    Reply(Result<Response, Failure>),
    /// The session ended by [`Client::disconnect`].
    Disconnected,
    /// The session ended on a link failure; a DISCONNECT is queued,
    /// except after [`Error::NotConnected`].
    Error(Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Closed,
    Connecting,
    Open,
}

#[derive(Debug)]
struct Pending {
    code: Code,
    counter: u8,
    frame: Vec<u8>,
    sends: u32,
    max_sends: u32,
    /// Set when the frame was handed out by `poll_output`.
    deadline: Option<Instant>,
    /// The first byte of a reply came.
    receiving: bool,
    /// Queued and not handed out yet.
    queued: bool,
    /// CONNECT already fell back to revision 1.
    fell_back: bool,
}

/// The client of a Filer session (the PC side; the palmtop serves).
#[derive(Debug)]
pub struct Client {
    config: Config,
    state: State,
    counter: u8,
    revision: u8,
    deframer: Deframer,
    out: VecDeque<Vec<u8>>,
    events: VecDeque<Event>,
    pending: Option<Pending>,
    /// Command and counter of the last reply taken, to skip a late
    /// duplicate of it.
    last: Option<(u8, u8)>,
}

impl Client {
    /// A client with no session open.
    pub fn new(config: Config) -> Self {
        let revision = config.revision;
        Client {
            config,
            state: State::Closed,
            counter: 0,
            revision,
            deframer: Deframer::new(),
            out: VecDeque::new(),
            events: VecDeque::new(),
            pending: None,
            last: None,
        }
    }

    /// The configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Whether a session is open.
    pub fn is_open(&self) -> bool {
        self.state == State::Open
    }

    /// Whether a request (or CONNECT) waits for its reply.
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    /// The session's revision (2, or 1 after a fallback).
    pub fn revision(&self) -> u8 {
        self.revision
    }

    /// The counter the next request goes out with.
    pub fn counter(&self) -> u8 {
        self.counter
    }

    /// Open a session: queue CONNECT. A session already open is reset (the
    /// server does the same).
    pub fn connect(&mut self, now: Instant) -> Result<(), StartError> {
        let _ = now;
        if self.pending.is_some() {
            return Err(StartError::Busy);
        }
        self.state = State::Connecting;
        // A DISCONNECT still queued from a failed session would only make
        // an idle server answer with a refusal that the CONNECT would take
        // for its own; the CONNECT resets the session anyway.
        self.out.clear();
        self.revision = self.config.revision;
        self.counter = 0;
        self.last = None;
        self.begin(Code::Connect, 0, Vec::new(), false);
        Ok(())
    }

    /// Send a request; its [`Event::Reply`] follows.
    pub fn send(&mut self, now: Instant, request: &Request) -> Result<(), StartError> {
        let _ = now;
        if matches!(request, Request::Connect | Request::Disconnect) {
            return Err(StartError::SessionCommand);
        }
        if self.pending.is_some() {
            return Err(StartError::Busy);
        }
        if self.state != State::Open {
            return Err(StartError::NotConnected);
        }
        self.begin(request.code(), self.counter, request.encode(), false);
        Ok(())
    }

    /// End the session: queue one DISCONNECT and report
    /// [`Event::Disconnected`] at once (the reply is not awaited). A
    /// request still waiting is dropped. Does nothing when no session is
    /// open or opening.
    pub fn disconnect(&mut self, now: Instant) {
        let _ = now;
        if self.state == State::Closed {
            return;
        }
        self.close();
        self.events.push_back(Event::Disconnected);
    }

    /// Feed bytes read from the link, in any chunking.
    pub fn handle_input(&mut self, now: Instant, bytes: &[u8]) {
        let window = self.window();
        let Some(p) = self.pending.as_mut() else {
            // Nothing is awaited: late bytes are dropped.
            self.deframer.clear();
            return;
        };
        if bytes.is_empty() {
            return;
        }
        if p.deadline.is_some() && !p.receiving {
            p.receiving = true;
            p.deadline = Some(now + window);
        }
        self.deframer.push(bytes);
        // The request awaited, to tell whether the frames answered it.
        let awaited = |p: &Pending| (p.code, p.counter, p.sends, p.fell_back);
        let before = self.pending.as_ref().map(awaited);
        let mut broken = false;
        // Every frame the bytes completed first: a good reply may follow a
        // broken one in the same chunk, and a resend empties the input.
        while let Some(frame) = self.deframer.next_frame() {
            if self.pending.is_none() {
                break;
            }
            match frame {
                Ok(frame) => self.on_frame(now, frame),
                // Cut short by the next frame, which is kept and may be
                // the reply: wait for it.
                Err(DeframeError::Cut) => {}
                Err(_) => broken = true,
            }
        }
        // A broken frame and nothing else on its way: resend at once
        // rather than wait.
        if broken && !self.deframer.in_frame() && self.pending.as_ref().map(awaited) == before {
            self.retry(now);
        }
    }

    /// Tell the client that time has passed; call it when
    /// [`Client::next_timeout`] is reached (it does nothing early).
    pub fn handle_timeout(&mut self, now: Instant) {
        if self
            .pending
            .as_ref()
            .and_then(|p| p.deadline)
            .is_some_and(|d| now >= d)
        {
            self.retry(now);
        }
    }

    /// The next frame to write, whole, in one write.
    pub fn poll_output(&mut self, now: Instant) -> Option<Vec<u8>> {
        let frame = self.out.pop_front()?;
        if let Some(p) = self.pending.as_mut()
            && p.queued
            && self.out.is_empty()
        {
            p.queued = false;
            let wait = if p.code == Code::Connect {
                self.config.connect_wait
            } else {
                self.config.reply_wait
            };
            p.deadline = Some(now + wire_time(frame.len(), self.config.baud) + wait);
            p.receiving = false;
        }
        Some(frame)
    }

    /// The next event.
    pub fn poll_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// When the client wants [`Client::handle_timeout`]; `None` while
    /// nothing is awaited.
    pub fn next_timeout(&self) -> Option<Instant> {
        self.pending.as_ref().and_then(|p| p.deadline)
    }

    fn window(&self) -> Duration {
        self.config
            .frame_window
            .unwrap_or_else(|| frame_window(self.config.baud))
    }

    fn begin(&mut self, code: Code, counter: u8, data: Vec<u8>, fell_back: bool) {
        let frame = Frame::request(code.byte(), counter, self.revision, data).encode();
        let rounds = if code == Code::Connect {
            self.config.connect_rounds.max(1)
        } else {
            1
        };
        self.pending = Some(Pending {
            code,
            counter,
            frame,
            sends: 0,
            max_sends: self.config.attempts.max(1).saturating_mul(rounds),
            deadline: None,
            receiving: false,
            queued: false,
            fell_back,
        });
        self.transmit();
    }

    /// Queue the pending frame (again), after emptying the input.
    fn transmit(&mut self) {
        self.deframer.clear();
        if let Some(p) = self.pending.as_mut() {
            p.sends += 1;
            p.queued = true;
            p.deadline = None;
            p.receiving = false;
            self.out.push_back(p.frame.clone());
        }
    }

    fn retry(&mut self, _now: Instant) {
        let Some(p) = self.pending.as_ref() else {
            return;
        };
        if p.sends < p.max_sends {
            self.transmit();
            return;
        }
        let error = Error::NoReply {
            code: p.code,
            sends: p.sends,
        };
        self.fail(error);
    }

    /// End the session on a link failure. A server that said it has no
    /// session gets no DISCONNECT.
    fn fail(&mut self, error: Error) {
        if matches!(error, Error::NotConnected { .. }) {
            self.close_quietly();
        } else {
            self.close();
        }
        self.events.push_back(Event::Error(error));
    }

    fn close(&mut self) {
        self.close_quietly();
        self.out.push_back(
            Frame::request(Code::Disconnect.byte(), 0, self.revision, Vec::new()).encode(),
        );
    }

    /// Close without a DISCONNECT.
    fn close_quietly(&mut self) {
        self.pending = None;
        self.state = State::Closed;
        self.deframer.clear();
        self.out.clear();
        self.counter = 0;
        self.last = None;
    }

    fn on_frame(&mut self, now: Instant, frame: Frame) {
        let Some(p) = self.pending.as_ref() else {
            return;
        };
        let (code, counter, sends, fell_back) = (p.code, p.counter, p.sends, p.fell_back);
        if frame.direction == REQUEST {
            // Our own frame echoed (an infrared link); not a reply.
            return;
        }
        let status_word = frame
            .data
            .get(..2)
            .map(|s| u16::from_le_bytes([s[0], s[1]]));
        if code == Code::Connect {
            if frame.direction != REPLY
                || frame.command != Code::Connect.byte()
                || frame.counter != 0
            {
                return self.fail(bad_reply(code, counter, &frame));
            }
            match status_word {
                Some(status::OK) => {
                    self.pending = None;
                    self.state = State::Open;
                    self.counter = 1;
                    self.events.push_back(Event::Connected {
                        revision: self.revision,
                    });
                }
                Some(status::NOT_CONNECTED) if self.revision != 1 && !fell_back => {
                    self.revision = 1;
                    self.begin(Code::Connect, 0, Vec::new(), true);
                }
                Some(other) => self.fail(Error::Refused(other)),
                None => self.fail(Error::Malformed(Malformed {
                    code,
                    data: frame.data,
                })),
            }
            return;
        }
        let matches = frame.command == code.byte() && frame.counter == counter;
        if frame.direction != REPLY {
            return self.fail(bad_reply(code, counter, &frame));
        }
        if !matches {
            if frame.command == Code::Connect.byte()
                && frame.counter == 0
                && status_word == Some(status::NOT_CONNECTED)
            {
                return self.fail(Error::NotConnected { sends });
            }
            if self.last == Some((frame.command, frame.counter)) {
                // A late duplicate of the previous reply.
                return;
            }
            return self.fail(bad_reply(code, counter, &frame));
        }
        match Response::decode(code, &frame.data) {
            Ok(reply) => {
                self.pending = None;
                self.last = Some((frame.command, frame.counter));
                self.counter = self.counter.wrapping_add(1);
                self.events.push_back(Event::Reply(reply));
            }
            Err(m) => self.fail(Error::Malformed(m)),
        }
        let _ = now;
    }
}

fn bad_reply(code: Code, counter: u8, frame: &Frame) -> Error {
    Error::BadReply {
        code,
        counter,
        got_command: frame.command,
        got_counter: frame.counter,
        got_direction: frame.direction,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::error::FsError;

    fn reply(code: u8, counter: u8, data: &[u8]) -> Vec<u8> {
        Frame::reply(code, counter, 2, data.to_vec()).encode()
    }

    fn connected() -> (Client, Instant) {
        let t = Instant::now();
        let mut c = Client::new(Config::default());
        c.connect(t).unwrap();
        let out = c.poll_output(t).unwrap();
        assert_eq!(
            out,
            [
                0x16, 0x16, 0x16, 0x10, 0x02, 0x01, 0x40, 0x00, 0x01, 0x02, 0x10, 0x03, 0x10, 0xBF
            ]
        );
        c.handle_input(t, &reply(0x40, 0, &[0, 0]));
        assert_eq!(c.poll_event(), Some(Event::Connected { revision: 2 }));
        assert!(c.is_open());
        (c, t)
    }

    #[test]
    fn connect_then_requests_count_from_one() {
        let (mut c, t) = connected();
        c.send(t, &Request::Drives).unwrap();
        let f = c.poll_output(t).unwrap();
        assert_eq!(f[6..8], [0x1B, 1]);
        assert_eq!(c.send(t, &Request::Drives), Err(StartError::Busy));
        c.handle_input(t, &reply(0x1B, 1, &[0, 0, 0, 0, 2, 0, b'C', 0, 0, 0, 0, 0]));
        assert_eq!(
            c.poll_event(),
            Some(Event::Reply(Ok(Response::Drives(b"C".to_vec()))))
        );
        assert_eq!(c.counter(), 2);
        c.send(t, &Request::FindNext).unwrap();
        assert_eq!(c.poll_output(t).unwrap()[6..8], [0x09, 2]);
    }

    #[test]
    fn connect_waits_two_seconds_and_is_sent_six_times() {
        let t = Instant::now();
        let mut c = Client::new(Config::default());
        c.connect(t).unwrap();
        let mut now = t;
        let mut frames = Vec::new();
        let error = loop {
            while let Some(f) = c.poll_output(now) {
                frames.push(f[6]);
            }
            if let Some(e) = c.poll_event() {
                break e;
            }
            let d = c.next_timeout().unwrap();
            let wait = d - now;
            assert!(
                wait > Duration::from_millis(1970) && wait < Duration::from_millis(2010),
                "{wait:?}"
            );
            c.handle_timeout(d - Duration::from_millis(1));
            assert_eq!(c.next_timeout(), Some(d));
            now = d;
            c.handle_timeout(now);
        };
        assert_eq!(
            error,
            Event::Error(Error::NoReply {
                code: Code::Connect,
                sends: 6
            })
        );
        while let Some(f) = c.poll_output(now) {
            frames.push(f[6]);
        }
        // Six CONNECTs, then one DISCONNECT.
        assert_eq!(frames, [0x40, 0x40, 0x40, 0x40, 0x40, 0x40, 0x41]);
        assert!(!c.is_open());
    }

    #[test]
    fn a_lost_reply_is_resent_with_the_same_counter() {
        let (mut c, t) = connected();
        c.send(t, &Request::TestName(b"C:\\".to_vec())).unwrap();
        let first = c.poll_output(t).unwrap();
        let d = c.next_timeout().unwrap();
        let wait = d - t;
        assert!(wait > Duration::from_millis(5900), "{wait:?}");
        assert!(wait < Duration::from_millis(5960), "{wait:?}");
        c.handle_timeout(d);
        let second = c.poll_output(d).unwrap();
        assert_eq!(first, second);
        c.handle_input(d, &reply(0x08, 1, &[0, 0, 0, 0, 2, 0]));
        assert_eq!(
            c.poll_event(),
            Some(Event::Reply(Ok(Response::Kind(crate::NameKind::Directory))))
        );
        // A late duplicate of that reply is skipped.
        c.send(d, &Request::FindNext).unwrap();
        c.poll_output(d).unwrap();
        c.handle_input(d, &reply(0x08, 1, &[0, 0, 0, 0, 2, 0]));
        assert!(c.poll_event().is_none());
        assert!(c.is_busy());
    }

    #[test]
    fn three_sends_then_disconnect() {
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        let mut now = t;
        let mut sends = 0;
        while c.is_busy() {
            while c.poll_output(now).is_some() {
                sends += 1;
            }
            now = c.next_timeout().unwrap();
            c.handle_timeout(now);
        }
        assert_eq!(sends, 3);
        assert_eq!(
            c.poll_event(),
            Some(Event::Error(Error::NoReply {
                code: Code::FindNext,
                sends: 3
            }))
        );
        assert_eq!(c.poll_output(now).unwrap()[6..8], [0x41, 0]);
        assert_eq!(
            c.send(now, &Request::FindNext),
            Err(StartError::NotConnected)
        );
    }

    #[test]
    fn the_frame_window_starts_with_the_first_byte() {
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        let wire = reply(0x09, 1, &[0, 0, 0, 0]);
        let later = t + Duration::from_secs(5);
        c.handle_input(later, &wire[..3]);
        let d = c.next_timeout().unwrap();
        assert!(d - later < Duration::from_millis(2000) && d - later > Duration::from_millis(1970));
    }

    #[test]
    fn a_broken_reply_is_resent_at_once() {
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        let first = c.poll_output(t).unwrap();
        let mut bad = reply(0x09, 1, &[0, 0, 0xD0, 0]);
        let n = bad.len();
        bad[n - 1] ^= 0xFF;
        c.handle_input(t, &bad);
        assert_eq!(c.poll_output(t), Some(first));
        c.handle_input(t, &reply(0x09, 1, &[0, 0, 0xD0, 0]));
        assert_eq!(
            c.poll_event(),
            Some(Event::Reply(Err(Failure::Fs(FsError::NOT_FOUND))))
        );
        assert!(c.is_open());
    }

    #[test]
    fn a_good_reply_behind_a_broken_one_is_taken() {
        let good = reply(0x09, 1, &[0, 0, 0xD0, 0]);
        // Cut short by the reply itself, in one chunk.
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        let cut = [&good[..8], &good[..]].concat();
        c.handle_input(t, &cut);
        assert_eq!(c.poll_output(t), None);
        assert_eq!(
            c.poll_event(),
            Some(Event::Reply(Err(Failure::Fs(FsError::NOT_FOUND))))
        );
        // A bad CRC and then the reply, in one chunk.
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        let mut bad = good.clone();
        let n = bad.len();
        bad[n - 1] ^= 0xFF;
        c.handle_input(t, &[bad.clone(), good.clone()].concat());
        assert_eq!(c.poll_output(t), None);
        assert_eq!(
            c.poll_event(),
            Some(Event::Reply(Err(Failure::Fs(FsError::NOT_FOUND))))
        );
        // A cut frame whose successor is still arriving: no resend yet.
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        c.handle_input(t, &[&good[..8], &good[..6]].concat());
        assert_eq!(c.poll_output(t), None);
        c.handle_input(t, &good[6..]);
        assert_eq!(
            c.poll_event(),
            Some(Event::Reply(Err(Failure::Fs(FsError::NOT_FOUND))))
        );
        // A bad CRC with a frame still arriving behind it: no resend yet.
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        c.handle_input(t, &[&bad[..], &good[..6]].concat());
        assert_eq!(c.poll_output(t), None);
        c.handle_input(t, &good[6..]);
        assert_eq!(
            c.poll_event(),
            Some(Event::Reply(Err(Failure::Fs(FsError::NOT_FOUND))))
        );
    }

    #[test]
    fn wrong_counter_or_a_lost_session_ends_it() {
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        c.handle_input(t, &reply(0x09, 7, &[0, 0, 0, 0]));
        assert!(matches!(
            c.poll_event(),
            Some(Event::Error(Error::BadReply { got_counter: 7, .. }))
        ));
        assert_eq!(c.poll_output(t).unwrap()[6], 0x41);

        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        c.handle_input(t, &reply(0x40, 0, &[0x3B, 0xFF]));
        assert_eq!(
            c.poll_event(),
            Some(Event::Error(Error::NotConnected { sends: 1 }))
        );
        // A refusal to a resend says so.
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        let d = c.next_timeout().unwrap();
        c.handle_timeout(d);
        c.poll_output(d).unwrap();
        c.handle_input(d, &reply(0x40, 0, &[0x3B, 0xFF]));
        assert_eq!(
            c.poll_event(),
            Some(Event::Error(Error::NotConnected { sends: 2 }))
        );
    }

    #[test]
    fn a_lost_session_gets_no_disconnect() {
        let (mut c, t) = connected();
        c.send(t, &Request::FindNext).unwrap();
        c.poll_output(t).unwrap();
        c.handle_input(t, &reply(0x40, 0, &[0x3B, 0xFF]));
        assert_eq!(
            c.poll_event(),
            Some(Event::Error(Error::NotConnected { sends: 1 }))
        );
        assert!(!c.is_open() && !c.is_busy());
        assert_eq!(c.poll_output(t), None);
        assert_eq!(c.next_timeout(), None);
        // A disconnect now has nothing to end either.
        c.disconnect(t);
        assert_eq!(c.poll_output(t), None);
        assert!(c.poll_event().is_none());
        // The next CONNECT is the only frame out.
        c.connect(t).unwrap();
        assert_eq!(
            c.poll_output(t).unwrap()[5..10],
            [0x01, 0x40, 0x00, 0x01, 0x02]
        );
        assert_eq!(c.poll_output(t), None);
        c.handle_input(t, &reply(0x40, 0, &[0, 0]));
        assert_eq!(c.poll_event(), Some(Event::Connected { revision: 2 }));
    }

    #[test]
    fn connect_falls_back_to_revision_one() {
        let t = Instant::now();
        let mut c = Client::new(Config::default());
        c.connect(t).unwrap();
        c.poll_output(t).unwrap();
        c.handle_input(t, &reply(0x40, 0, &[0x3B, 0xFF]));
        let f = c.poll_output(t).unwrap();
        assert_eq!(f[5..10], [0x01, 0x40, 0x00, 0x01, 0x01]);
        c.handle_input(t, &Frame::reply(0x40, 0, 1, vec![0, 0]).encode());
        assert_eq!(c.poll_event(), Some(Event::Connected { revision: 1 }));
        // A second refusal does not fall back again.
        let mut c = Client::new(Config::default());
        c.connect(t).unwrap();
        c.poll_output(t).unwrap();
        c.handle_input(t, &reply(0x40, 0, &[0x3B, 0xFF]));
        c.poll_output(t).unwrap();
        c.handle_input(t, &reply(0x40, 0, &[0x3B, 0xFF]));
        assert_eq!(c.poll_event(), Some(Event::Error(Error::Refused(0xFF3B))));
    }

    #[test]
    fn disconnect_is_sent_once_and_not_awaited() {
        let (mut c, t) = connected();
        c.disconnect(t);
        assert_eq!(c.poll_event(), Some(Event::Disconnected));
        assert_eq!(
            c.poll_output(t).unwrap(),
            [
                0x16, 0x16, 0x16, 0x10, 0x02, 0x01, 0x41, 0x00, 0x01, 0x02, 0x10, 0x03, 0x2D, 0x7F
            ]
        );
        assert!(c.poll_output(t).is_none());
        assert!(c.next_timeout().is_none());
        c.disconnect(t);
        assert!(c.poll_event().is_none());
    }

    #[test]
    fn counter_wraps_after_ff() {
        let (mut c, t) = connected();
        c.counter = 0xFF;
        c.send(t, &Request::FindClose).unwrap();
        c.poll_output(t).unwrap();
        c.handle_input(t, &reply(0x16, 0xFF, &[0, 0, 0, 0, 0, 0]));
        assert_eq!(c.poll_event(), Some(Event::Reply(Ok(Response::Done))));
        assert_eq!(c.counter(), 0);
    }

    #[test]
    fn frame_windows() {
        assert_eq!(frame_window(115_200), frame_window(19200));
        assert!(frame_window(9600) > Duration::from_millis(4990));
        assert!(frame_window(300) > Duration::from_secs(79));
    }
}
