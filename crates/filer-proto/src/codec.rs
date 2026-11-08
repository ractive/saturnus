//! The frame layer: lead-in, header, DLE doubling, CRC-16/ARC, and a
//! deframer that resynchronises on line noise (wiki: protocols/filer-hplx,
//! "Packet framing").
//!
//! A frame on the wire:
//!
//! ```text
//! SYN SYN SYN DLE STX | SOH command counter direction revision data.. | DLE ETX crc-lo crc-hi
//! ```
//!
//! Between `SOH` and the last data byte every `DLE` (0x10) is sent twice.
//! The CRC covers that part (each doubled DLE once) and the `ETX`; its two
//! bytes go out as they are, never doubled.

use std::collections::VecDeque;
use std::fmt;

/// Synchronous idle; three of them start every frame.
pub const SYN: u8 = 0x16;
/// Data link escape.
pub const DLE: u8 = 0x10;
/// Start of text (after the leading DLE).
pub const STX: u8 = 0x02;
/// End of text (after the closing DLE).
pub const ETX: u8 = 0x03;
/// Start of header, the first byte of the body.
pub const SOH: u8 = 0x01;
/// The direction byte of a request (client to server).
pub const REQUEST: u8 = 0x01;
/// The direction byte of a reply (server to client).
pub const REPLY: u8 = 0x81;
/// The protocol revision of a normal session; 1 is for older peers.
pub const REVISION: u8 = 2;
/// The most data bytes in one read or write block.
pub const MAX_BLOCK: usize = 0x800;
/// The longest body (SOH to the last data byte, after undoubling) a
/// receiver takes; a longer one is dropped.
pub const MAX_BODY: usize = 0x820;
/// Header bytes in the body: SOH, command, counter, direction, revision.
pub const HEADER_LEN: usize = 5;

/// CRC-16/ARC: reflected polynomial 0xA001, start 0, no final XOR. Its
/// check value over `b"123456789"` is 0xBB3D.
pub fn crc16(bytes: &[u8]) -> u16 {
    crc16_update(0, bytes)
}

/// Continue a CRC-16/ARC over more bytes.
pub fn crc16_update(mut crc: u16, bytes: &[u8]) -> u16 {
    for &b in bytes {
        crc ^= u16::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

/// One frame, without the lead-in, the DLE doubling and the CRC.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    /// The command code (see [`crate::Code`]).
    pub command: u8,
    /// The packet counter.
    pub counter: u8,
    /// [`REQUEST`] or [`REPLY`].
    pub direction: u8,
    /// The protocol revision byte, [`REVISION`] normally.
    pub revision: u8,
    /// The command's data; in a reply it starts with the status word.
    pub data: Vec<u8>,
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("command", &format_args!("{:#04x}", self.command))
            .field("counter", &self.counter)
            .field("direction", &format_args!("{:#04x}", self.direction))
            .field("revision", &self.revision)
            .field("data", &format_args!("{}", hex(&self.data)))
            .finish()
    }
}

impl Frame {
    /// A request frame.
    pub fn request(command: u8, counter: u8, revision: u8, data: Vec<u8>) -> Self {
        Frame {
            command,
            counter,
            direction: REQUEST,
            revision,
            data,
        }
    }

    /// A reply frame.
    pub fn reply(command: u8, counter: u8, revision: u8, data: Vec<u8>) -> Self {
        Frame {
            command,
            counter,
            direction: REPLY,
            revision,
            data,
        }
    }

    /// The bytes on the wire.
    pub fn encode(&self) -> Vec<u8> {
        let header = [
            SOH,
            self.command,
            self.counter,
            self.direction,
            self.revision,
        ];
        let mut out = Vec::with_capacity(self.data.len() + 16);
        out.extend_from_slice(&[SYN, SYN, SYN, DLE, STX]);
        let mut crc = 0;
        for part in [&header[..], &self.data[..]] {
            crc = crc16_update(crc, part);
            for &b in part {
                out.push(b);
                if b == DLE {
                    out.push(DLE);
                }
            }
        }
        crc = crc16_update(crc, &[ETX]);
        out.extend_from_slice(&[DLE, ETX]);
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }

    /// Parse a body (SOH to the last data byte, DLEs undoubled).
    fn from_body(body: &[u8]) -> Result<Self, DeframeError> {
        match body {
            [SOH, command, counter, direction, revision, data @ ..] => Ok(Frame {
                command: *command,
                counter: *counter,
                direction: *direction,
                revision: *revision,
                data: data.to_vec(),
            }),
            [SOH, ..] | [] => Err(DeframeError::Short(body.len())),
            [other, ..] => Err(DeframeError::NoSoh(*other)),
        }
    }
}

/// Why a frame was dropped. A receiver drops such frames silently: only
/// the sender's timeout recovers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeframeError {
    /// DLE followed by a byte other than DLE, ETX or STX inside a body.
    BadDle(u8),
    /// A DLE STX inside a body: the frame was cut short and a new one
    /// started (the new one is kept).
    Cut,
    /// The body grew past [`MAX_BODY`].
    TooLong,
    /// The CRC bytes do not match.
    Crc {
        /// The CRC computed over the body.
        computed: u16,
        /// The CRC that came with it.
        received: u16,
    },
    /// The body is shorter than the header.
    Short(usize),
    /// The body does not start with SOH (the byte it starts with).
    NoSoh(u8),
}

impl fmt::Display for DeframeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeframeError::BadDle(b) => write!(f, "DLE followed by {b:#04x}"),
            DeframeError::Cut => f.write_str("frame cut short by the next one"),
            DeframeError::TooLong => write!(f, "body longer than {MAX_BODY} bytes"),
            DeframeError::Crc { computed, received } => {
                write!(f, "CRC {received:#06x}, expected {computed:#06x}")
            }
            DeframeError::Short(n) => write!(f, "body of {n} bytes, shorter than the header"),
            DeframeError::NoSoh(b) => write!(f, "body starts with {b:#04x}, not SOH"),
        }
    }
}

impl std::error::Error for DeframeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Waiting for a SYN; everything else is noise.
    Hunt,
    /// One or more SYNs seen.
    Syn,
    /// SYN(s), then DLE.
    SynDle,
    /// In the body.
    Body,
    /// In the body, after a DLE.
    BodyDle,
    /// After DLE ETX: the CRC's low byte comes next.
    CrcLow,
    /// The CRC's high byte comes next.
    CrcHigh(u8),
}

/// Turns received bytes, in any chunking, into frames.
///
/// It follows the receiver of HP's Filer: everything before a SYN is
/// skipped, any number of SYNs may come, then DLE STX starts the body.
/// One deliberate difference: a DLE STX inside a body (a frame cut short
/// and followed at once by the next) starts the next frame at once
/// instead of dropping both ([`DeframeError::Cut`] for the first).
#[derive(Debug)]
pub struct Deframer {
    state: State,
    body: Vec<u8>,
    out: VecDeque<Result<Frame, DeframeError>>,
}

impl Default for Deframer {
    fn default() -> Self {
        Self::new()
    }
}

impl Deframer {
    /// An empty deframer, hunting for a SYN.
    pub fn new() -> Self {
        Deframer {
            state: State::Hunt,
            body: Vec::new(),
            out: VecDeque::new(),
        }
    }

    /// Forget a partial frame and every frame not yet taken.
    pub fn clear(&mut self) {
        self.state = State::Hunt;
        self.body.clear();
        self.out.clear();
    }

    /// Whether a frame has started and is not complete yet.
    pub fn in_frame(&self) -> bool {
        !matches!(self.state, State::Hunt | State::Syn)
    }

    /// Feed received bytes.
    pub fn push(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.byte(b);
        }
    }

    /// The next complete frame, or why one was dropped.
    pub fn next_frame(&mut self) -> Option<Result<Frame, DeframeError>> {
        self.out.pop_front()
    }

    fn fail(&mut self, error: DeframeError) {
        self.out.push_back(Err(error));
        self.body.clear();
        self.state = State::Hunt;
    }

    fn byte(&mut self, b: u8) {
        match self.state {
            State::Hunt => {
                if b == SYN {
                    self.state = State::Syn;
                }
            }
            State::Syn => {
                self.state = match b {
                    SYN => State::Syn,
                    DLE => State::SynDle,
                    _ => State::Hunt,
                }
            }
            State::SynDle => {
                self.state = match b {
                    STX => {
                        self.body.clear();
                        State::Body
                    }
                    SYN => State::Syn,
                    _ => State::Hunt,
                }
            }
            State::Body => {
                if b == DLE {
                    self.state = State::BodyDle;
                } else {
                    self.body_byte(b);
                }
            }
            State::BodyDle => match b {
                DLE => {
                    self.state = State::Body;
                    self.body_byte(DLE);
                }
                ETX => self.state = State::CrcLow,
                STX => {
                    self.out.push_back(Err(DeframeError::Cut));
                    self.body.clear();
                    self.state = State::Body;
                }
                other => {
                    self.fail(DeframeError::BadDle(other));
                    // The byte may start the next frame.
                    self.byte(other);
                }
            },
            State::CrcLow => self.state = State::CrcHigh(b),
            State::CrcHigh(low) => {
                let received = u16::from_le_bytes([low, b]);
                let computed = crc16_update(crc16(&self.body), &[ETX]);
                let result = if computed == received {
                    Frame::from_body(&self.body)
                } else {
                    Err(DeframeError::Crc { computed, received })
                };
                self.out.push_back(result);
                self.body.clear();
                self.state = State::Hunt;
            }
        }
    }

    fn body_byte(&mut self, b: u8) {
        if self.body.len() >= MAX_BODY {
            self.fail(DeframeError::TooLong);
        } else {
            self.body.push(b);
            self.state = State::Body;
        }
    }
}

/// Hex dump, `01 40 00`.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// The wiki's example frames (protocols/filer-hplx, "Example frames").
    const CONNECT: [u8; 14] = [
        0x16, 0x16, 0x16, 0x10, 0x02, 0x01, 0x40, 0x00, 0x01, 0x02, 0x10, 0x03, 0x10, 0xBF,
    ];
    const DISCONNECT: [u8; 14] = [
        0x16, 0x16, 0x16, 0x10, 0x02, 0x01, 0x41, 0x00, 0x01, 0x02, 0x10, 0x03, 0x2D, 0x7F,
    ];

    #[test]
    fn crc_check_value() {
        assert_eq!(crc16(b"123456789"), 0xBB3D);
        assert_eq!(crc16(b""), 0);
    }

    #[test]
    fn example_frames_encode() {
        assert_eq!(Frame::request(0x40, 0, 2, vec![]).encode(), CONNECT);
        assert_eq!(Frame::request(0x41, 0, 2, vec![]).encode(), DISCONNECT);
    }

    #[test]
    fn example_frames_decode_with_an_undoubled_crc_dle() {
        let mut d = Deframer::new();
        d.push(&CONNECT);
        d.push(&DISCONNECT);
        let f = d.next_frame().unwrap().unwrap();
        assert_eq!(
            (f.command, f.counter, f.direction, f.revision),
            (0x40, 0, 1, 2)
        );
        assert!(f.data.is_empty());
        assert_eq!(d.next_frame().unwrap().unwrap().command, 0x41);
        assert!(d.next_frame().is_none());
    }

    /// A reply recorded from BSD Filer (the drive list, `C` only).
    #[test]
    fn oracle_reply_decodes() {
        let wire = [
            0x16, 0x16, 0x16, 0x10, 0x02, 0x01, 0x1b, 0x01, 0x81, 0x02, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x43, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x03, 0x83, 0x9c,
        ];
        let mut d = Deframer::new();
        d.push(&wire);
        let f = d.next_frame().unwrap().unwrap();
        assert_eq!(f.direction, REPLY);
        assert_eq!(f.data, [0, 0, 0, 0, 2, 0, b'C', 0, 0, 0, 0, 0]);
        assert_eq!(f.encode(), wire);
    }

    #[test]
    fn dle_in_header_and_data_is_doubled() {
        let f = Frame::request(0x10, 0x10, 2, vec![0x10, 0x20]);
        let wire = f.encode();
        assert_eq!(
            &wire[5..14],
            &[0x01, 0x10, 0x10, 0x10, 0x10, 0x01, 0x02, 0x10, 0x10]
        );
        let mut d = Deframer::new();
        for b in &wire {
            d.push(&[*b]);
        }
        assert_eq!(d.next_frame().unwrap().unwrap(), f);
    }

    #[test]
    fn noise_and_extra_syns_are_skipped() {
        let mut d = Deframer::new();
        d.push(&[0x00, 0xFF, 0x10, 0x02, 0x16, 0x41, 0x16, 0x16, 0x16, 0x16]);
        d.push(&CONNECT[3..]);
        assert_eq!(d.next_frame().unwrap().unwrap().command, 0x40);
        // DLE STX straight after noise is not a frame start.
        d.push(&[
            0x55, 0x10, 0x02, 0x01, 0x40, 0x00, 0x01, 0x02, 0x10, 0x03, 0x10, 0xBF,
        ]);
        assert!(d.next_frame().is_none());
    }

    #[test]
    fn bad_crc_drops_the_frame() {
        let mut wire = CONNECT.to_vec();
        wire[13] ^= 1;
        let mut d = Deframer::new();
        d.push(&wire);
        assert!(matches!(
            d.next_frame(),
            Some(Err(DeframeError::Crc { .. }))
        ));
        d.push(&CONNECT);
        assert!(d.next_frame().unwrap().is_ok());
    }

    #[test]
    fn a_cut_frame_does_not_lose_the_next() {
        let mut d = Deframer::new();
        d.push(&CONNECT[..8]);
        d.push(&DISCONNECT);
        assert_eq!(d.next_frame(), Some(Err(DeframeError::Cut)));
        assert_eq!(d.next_frame().unwrap().unwrap().command, 0x41);
    }

    #[test]
    fn illegal_dle_and_overlong_bodies() {
        let mut d = Deframer::new();
        d.push(&[0x16, 0x10, 0x02, 0x01, 0x10, 0x41]);
        assert_eq!(d.next_frame(), Some(Err(DeframeError::BadDle(0x41))));
        d.push(&[0x16, 0x10, 0x02]);
        d.push(&vec![0x20; MAX_BODY + 1]);
        assert_eq!(d.next_frame(), Some(Err(DeframeError::TooLong)));
        // A body of exactly MAX_BODY is taken.
        let f = Frame::request(0x15, 1, 2, vec![0x20; MAX_BODY - HEADER_LEN]);
        d.push(&f.encode());
        assert_eq!(d.next_frame().unwrap().unwrap(), f);
    }

    #[test]
    fn short_and_soh_less_bodies() {
        let raw = |body: &[u8]| {
            let crc = crc16_update(crc16(body), &[ETX]);
            let mut w = vec![SYN, DLE, STX];
            w.extend_from_slice(body);
            w.extend_from_slice(&[DLE, ETX]);
            w.extend_from_slice(&crc.to_le_bytes());
            w
        };
        let mut d = Deframer::new();
        d.push(&raw(&[SOH, 0x40, 0]));
        assert_eq!(d.next_frame(), Some(Err(DeframeError::Short(3))));
        d.push(&raw(&[0x02, 0x40, 0, 1, 2]));
        assert_eq!(d.next_frame(), Some(Err(DeframeError::NoSoh(2))));
    }
}
