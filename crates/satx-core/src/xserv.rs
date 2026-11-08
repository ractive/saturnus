//! XSERV: the command server on top of XModem (49g+/50g, the 49G after
//! ROM 1.10, and the XSrvr48 library on the 48): packet framing, commands,
//! the directory listing, the detection probe, and a client over the link.
//!
//! **Tested on a real 49G** (ROM 1.18, 2026-11-08): `satx xserv mem`, `ls`,
//! `get`, `put` and `eval` worked, and 4 KB took 8.4 s against
//! Kermit's 16.7 s. **And on the emulated 49G** (saturnus, ROM 2.15,
//! 2026-11-08): typed `XSERV` shows "Xmodem server"; every command below,
//! `Q` included, and the Kermit server nested through `E` (`SERVER`, then
//! Kermit's FINISH returns to XSERV). Not yet tried on a 49g+ or 50g. The
//! framing follows the wiki's description of HP's own client code (wiki:
//! protocols/xserv). The framing and parsing here are sans-IO; the client
//! ([`XservSession`], and the XSERV operations of
//! [`Machine`](crate::machine::Machine)) talks to the link of
//! [`crate::link`], as the Kermit and XModem code do.
//!
//! # Framing
//!
//! A packet is a 2-byte length (high byte first), the bytes, and a 1-byte
//! checksum: the sum of the bytes mod 256. The receiving side answers ACK,
//! or NAK to get the packet again (the client retries a command packet up
//! to 5 times and a reply up to 4).
//!
//! # Commands
//!
//! The host sends one command byte once the line is quiet, then, depending
//! on the command, a command packet ([`XservCommand::packet`]) and an XModem
//! transfer or a reply packet:
//!
//! | Byte | Command | Then |
//! | --- | --- | --- |
//! | `P` | [`XservCommand::Put`] | packet with the name, XModem send by the host |
//! | `G` | [`XservCommand::Get`] | packet with the name, XModem receive by the host |
//! | `E` | [`XservCommand::Execute`] | packet with RPL text |
//! | `M` | [`XservCommand::Memory`] | reply packet: the free memory as text, `245478.` |
//! | `L` | [`XservCommand::List`] | reply packet, one [`DirRecord`] per variable |
//! | `Q` | [`XservCommand::Quit`] | nothing: XSERV ends (ROM 2.15) |
//!
//! # A busy calculator
//!
//! `E` is ACKed before its RPL runs, and while it runs the calculator drops
//! the bytes it gets: an `L` sent 0.3 s into a 5 s loop never got an answer,
//! an `M` sent every second got one answer, when the loop was done
//! (emulated 49G, 2026-11-08). The client therefore resends `M` and `L`
//! when nothing comes back within [`XservOptions::answer`], and before
//! `E`, `G` or `P` after anything that may have left the calculator busy
//! it asks `M` until it answers: a command byte lost while the packet that
//! follows it is read as command bytes would do harm.
//!
//! # Detection
//!
//! A Kermit server ignores a lone `M` (the real 49G's and the emulated
//! 48SX's, 48GX's and 49G's) and XSERV ignores the Kermit packets of a
//! speed search, so a host can send `M` first and listen:
//! [`memory_reply`] tells XSERV's answer in what came back.

use std::fmt;

use xmodem_proto::codec::{ACK, NAK};

use crate::calc::validate_name;
use crate::charset::{decode, encode, encode_command};
use crate::link::Link;
use crate::object::ObjectType;
use crate::reply::{Entry, parse_real};
use crate::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use crate::time::Instant;
#[cfg(not(target_arch = "wasm32"))]
use crate::transport::Transport;
use crate::xmodem::{self, XmodemOptions, XmodemReceived, XmodemReport};
use crate::{Error, Result};

/// Most data bytes a packet can carry (16-bit length).
pub const MAX_DATA: usize = 0xFFFF;

/// Why a packet or a directory listing could not be built or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// More than [`MAX_DATA`] bytes to send.
    TooLong(usize),
    /// The frame or record ends early.
    Truncated,
    /// Bytes after the checksum of a single frame.
    TrailingBytes(usize),
    /// The checksum byte does not match the data.
    BadChecksum {
        /// Sum of the data bytes mod 256.
        expected: u8,
        /// The checksum byte received.
        got: u8,
    },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::TooLong(n) => write!(f, "{n} bytes do not fit one XSERV packet"),
            FrameError::Truncated => f.write_str("XSERV packet or record ends early"),
            FrameError::TrailingBytes(n) => write!(f, "{n} bytes after the XSERV packet"),
            FrameError::BadChecksum { expected, got } => {
                write!(f, "XSERV checksum {got:#04x}, expected {expected:#04x}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Sum of `data` mod 256.
pub fn checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

/// Frame `data` as one packet: length (high byte first), data, checksum.
pub fn encode_packet(data: &[u8]) -> std::result::Result<Vec<u8>, FrameError> {
    let len = u16::try_from(data.len()).map_err(|_| FrameError::TooLong(data.len()))?;
    let mut out = Vec::with_capacity(data.len() + 3);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data);
    out.push(checksum(data));
    Ok(out)
}

/// Read exactly one complete packet and return its data.
pub fn decode_packet(frame: &[u8]) -> std::result::Result<Vec<u8>, FrameError> {
    let mut deframer = Deframer::new();
    deframer.push(frame);
    let data = deframer.next_packet().ok_or(FrameError::Truncated)??;
    match deframer.pending() {
        0 => Ok(data),
        n => Err(FrameError::TrailingBytes(n)),
    }
}

/// Splits a byte stream into packets, in any chunking.
#[derive(Clone, Debug, Default)]
pub struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    /// Empty deframer.
    pub fn new() -> Self {
        Deframer::default()
    }

    /// Append bytes read from the link.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Bytes buffered but not yet returned as a packet.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    /// Drop everything buffered (after a NAK, the sender starts over).
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// The next complete packet's data, or why it is bad (answer NAK); `None`
    /// until a whole packet is buffered. A bad packet is consumed.
    pub fn next_packet(&mut self) -> Option<std::result::Result<Vec<u8>, FrameError>> {
        let (&hi, &lo) = (self.buf.first()?, self.buf.get(1)?);
        let len = usize::from(u16::from_be_bytes([hi, lo]));
        let got = *self.buf.get(2 + len)?;
        let frame: Vec<u8> = self.buf.drain(..3 + len).collect();
        let data = frame.get(2..2 + len).unwrap_or_default().to_vec();
        let expected = checksum(&data);
        Some(if got == expected {
            Ok(data)
        } else {
            Err(FrameError::BadChecksum { expected, got })
        })
    }
}

/// An XSERV command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XservCommand {
    /// `P`: store the file that follows by XModem as variable NAME.
    Put(String),
    /// `G`: send variable NAME by XModem.
    Get(String),
    /// `E`: execute RPL text, e.g. `HOME DIR1` to change directory.
    Execute(String),
    /// `M`: the free memory in bytes, as a reply packet holding the number
    /// as text (`245478.`, the 49G's trailing point).
    Memory,
    /// `L`: list the current directory, as a reply packet of
    /// [`DirRecord`]s.
    List,
    /// `Q`: end XSERV, no reply (Conn4x's client code; on the emulated 49G,
    /// ROM 2.15, the calculator is back at its stack after it).
    Quit,
}

impl XservCommand {
    /// The command byte.
    pub fn byte(&self) -> u8 {
        match self {
            XservCommand::Put(_) => b'P',
            XservCommand::Get(_) => b'G',
            XservCommand::Execute(_) => b'E',
            XservCommand::Memory => b'M',
            XservCommand::List => b'L',
            XservCommand::Quit => b'Q',
        }
    }

    /// The command packet that follows the command byte, framed; `None` for
    /// `M`, `L` and `Q`. Names must be valid variable names; names and RPL text
    /// are encoded in the HP character set (RPL text may use ASCII
    /// trigraphs such as `\->`).
    pub fn packet(&self) -> Result<Option<Vec<u8>>> {
        let data = match self {
            XservCommand::Put(name) | XservCommand::Get(name) => {
                validate_name(name)?;
                encode(name)?
            }
            XservCommand::Execute(text) => encode_command(text)?,
            XservCommand::Memory | XservCommand::List | XservCommand::Quit => return Ok(None),
        };
        encode_packet(&data)
            .map(Some)
            .map_err(|e| Error::Reply(e.to_string()))
    }

    /// Whether the calculator answers with a reply packet.
    pub fn has_reply(&self) -> bool {
        matches!(self, XservCommand::Memory | XservCommand::List)
    }
}

/// One variable in an `L` reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirRecord {
    /// Name, decoded from the HP character set.
    pub name: String,
    /// Low 16 bits of the object's prolog, e.g. `0x2A96` for a directory,
    /// `0x2A2C` for a string.
    pub prolog: u16,
    /// Size in nibbles.
    pub size_nibbles: u32,
    /// The object's CRC as the calculator reports it.
    pub crc: u16,
}

impl DirRecord {
    /// Whether the prolog is a directory's (#2A96).
    pub fn is_directory(&self) -> bool {
        self.prolog == 0x2A96
    }

    /// The type name as the Kermit server's `G D` listing prints it
    /// (`Real Number`, `Directory`); `Object #2A4E` for a prolog not known.
    pub fn kind(&self) -> String {
        ObjectType::from_prolog(u32::from(self.prolog)).map_or_else(
            || format!("Object #{:04X}", self.prolog),
            |t| t.name().to_string(),
        )
    }

    /// The size as the Kermit server's `G D` listing gives it, in bytes:
    /// the object's nibbles, which XSERV gives, and the variable's entry in
    /// its directory, 4.5 bytes and one per name character (the name's
    /// length before and after it, 2 nibbles each, and the 5-nibble link).
    /// Checked on seven variables of the emulated 49G, ROM 2.15
    /// (2026-11-08): one- to eight-character names, two directories, a
    /// list, a string, reals and a program, every one as Kermit's.
    pub fn size(&self) -> f64 {
        let name = encode(&self.name).map_or(self.name.chars().count(), |b| b.len());
        f64::from(self.size_nibbles) / 2.0 + 4.5 + name as f64
    }

    /// The record as a listing entry, sized and named as the Kermit
    /// server's ([`DirRecord::size`], [`DirRecord::kind`]).
    pub fn entry(&self) -> Entry {
        Entry {
            name: self.name.clone(),
            size: self.size(),
            kind: self.kind(),
            checksum: self.crc,
        }
    }
}

/// Parse the data of an `L` reply: per variable, 1 byte name length, the
/// name, 2 bytes prolog (low 16 bits, low byte first), 3 bytes size in
/// nibbles (low byte first), 2 bytes CRC, low byte first too (the Kermit
/// server's checksums of the same variables, emulated 49G, 2026-11-08).
pub fn parse_dir_list(data: &[u8]) -> std::result::Result<Vec<DirRecord>, FrameError> {
    let mut records = Vec::new();
    let mut rest = data;
    while let Some((&len, tail)) = rest.split_first() {
        let len = usize::from(len);
        let record = tail.get(..len + 7).ok_or(FrameError::Truncated)?;
        let (name, fields) = record.split_at(len);
        let byte = |i: usize| fields.get(i).copied().unwrap_or_default();
        records.push(DirRecord {
            name: decode(name),
            prolog: u16::from_le_bytes([byte(0), byte(1)]),
            size_nibbles: u32::from_le_bytes([byte(2), byte(3), byte(4), 0]),
            crc: u16::from_le_bytes([byte(5), byte(6)]),
        });
        rest = tail.get(len + 7..).unwrap_or_default();
    }
    Ok(records)
}

/// The free memory in an answer to `M` heard on the link: the first
/// well-formed packet in `heard` (garbage before it is skipped: an idle
/// Kermit server's NAK, a reply at the wrong speed) whose data reads as a
/// number. `None` when there is none, or not all of it yet.
pub fn memory_reply(heard: &[u8]) -> Option<f64> {
    (0..heard.len()).find_map(|at| {
        let rest = heard.get(at..)?;
        let (&hi, &lo) = (rest.first()?, rest.get(1)?);
        let len = usize::from(u16::from_be_bytes([hi, lo]));
        // The number is a few digits: a longer "packet" is noise.
        if !(1..=MEMORY_REPLY_MAX).contains(&len) {
            return None;
        }
        let data = rest.get(2..2 + len)?;
        if *rest.get(2 + len)? != checksum(data) {
            return None;
        }
        parse_real(std::str::from_utf8(data).ok()?)
    })
}

/// The longest `M` reply [`memory_reply`] takes.
const MEMORY_REPLY_MAX: usize = 24;

/// Tries for a command packet before giving up (HP's client: 5).
const COMMAND_TRIES: u32 = 5;
/// Bad replies before giving up (HP's client: 4).
const REPLY_FAILURES: u32 = 4;

/// Timing of an XSERV client.
#[derive(Clone, Debug)]
pub struct XservOptions {
    /// How long the line must be quiet before a command byte.
    pub quiet: Duration,
    /// How long a command waits for the first byte of its answer (the reply
    /// packet, or the ACK of its packet) before it is sent again.
    pub answer: Duration,
    /// How long `M` and `L` are sent again while nothing answers: a
    /// calculator running the RPL of an `E` answers once it is done.
    pub busy: Duration,
    /// How long the rest of a packet may take once it has started, and the
    /// ACK of a command packet.
    pub timeout: Duration,
}

impl Default for XservOptions {
    /// 200 ms quiet, 1.5 s for the first byte, 60 s busy, 5 s timeout. The
    /// emulated 49G answers `M` and `L` within 0.15 s, and ACKs a command
    /// packet at once.
    fn default() -> Self {
        XservOptions {
            quiet: Duration::from_millis(200),
            answer: Duration::from_millis(1500),
            busy: Duration::from_secs(60),
            timeout: Duration::from_secs(5),
        }
    }
}

/// The client's state between commands.
#[derive(Clone, Debug)]
pub(crate) struct XservState {
    pub(crate) options: XservOptions,
    /// The calculator may still be busy (after an `E`, a transfer, a nested
    /// Kermit session): ask `M` before a command with a packet.
    pub(crate) settle: bool,
}

impl XservState {
    pub(crate) fn new(options: XservOptions) -> Self {
        XservState {
            options,
            settle: true,
        }
    }
}

/// What the calculator answered to a command byte.
enum Answer {
    /// A reply packet (ACKed).
    Packet(Vec<u8>),
    /// Nothing at all within the wait.
    Silence,
}

/// Wait until the line has been quiet for `quiet` (late bytes of an
/// earlier exchange are dropped).
async fn quiet(link: &Link, quiet: Duration) -> Result<()> {
    link.drain(quiet).await?;
    Ok(())
}

/// Read one reply packet: [`Answer::Silence`] when no byte comes within
/// `first`; a good packet is ACKed, a bad one NAKed (the calculator sends it
/// again) up to [`REPLY_FAILURES`] times.
async fn read_reply(link: &Link, first: Duration, timeout: Duration) -> Result<Answer> {
    let mut deframer = Deframer::new();
    let mut failures = 0;
    let mut heard = false;
    let mut deadline = link.now() + first;
    loop {
        while let Some(packet) = deframer.next_packet() {
            match packet {
                Ok(data) => {
                    link.write(vec![ACK])?;
                    link.flush().await?;
                    return Ok(Answer::Packet(data));
                }
                Err(e) => {
                    failures += 1;
                    if failures >= REPLY_FAILURES {
                        return Err(Error::Reply(format!("XSERV reply: {e}")));
                    }
                    deframer.clear();
                    link.write(vec![NAK])?;
                    link.flush().await?;
                    deadline = link.now() + timeout;
                }
            }
        }
        let bytes = link.read(Some(deadline)).await?;
        if bytes.is_empty() {
            if !heard {
                return Ok(Answer::Silence);
            }
            failures += 1;
            if failures >= REPLY_FAILURES {
                return Err(Error::Reply(format!(
                    "XSERV reply: nothing complete after {REPLY_FAILURES} tries"
                )));
            }
            deframer.clear();
            link.write(vec![NAK])?;
            link.flush().await?;
            deadline = link.now() + timeout;
            continue;
        }
        heard = true;
        deframer.push(&bytes);
        deadline = link.now() + timeout;
    }
}

/// Send `M` or `L` and read its reply, sending the byte again while nothing
/// answers, for up to [`XservOptions::busy`].
async fn ask(link: &Link, options: &XservOptions, byte: u8) -> Result<Vec<u8>> {
    let give_up = link.now() + options.busy;
    loop {
        quiet(link, options.quiet).await?;
        link.write(vec![byte])?;
        link.flush().await?;
        if let Answer::Packet(data) = read_reply(link, options.answer, options.timeout).await? {
            return Ok(data);
        }
        if link.now() >= give_up {
            return Err(Error::Reply(format!(
                "XSERV {}: no answer within {:.0} s; is XSERV running on the calculator?",
                char::from(byte),
                options.busy.as_secs_f64()
            )));
        }
    }
}

/// `true` on ACK, `false` on NAK or nothing within `timeout`; other bytes
/// are skipped.
async fn wait_ack(link: &Link, timeout: Duration) -> Result<bool> {
    let deadline = link.now() + timeout;
    loop {
        let bytes = link.read(Some(deadline)).await?;
        if bytes.is_empty() {
            return Ok(false);
        }
        for b in bytes {
            match b {
                ACK => return Ok(true),
                NAK => return Ok(false),
                _ => {}
            }
        }
    }
}

/// Send `command` (the command byte, and its packet until ACKed); for `M`
/// and `L` return the reply's data. See the module documentation for when
/// it waits for a busy calculator.
pub(crate) async fn command(
    link: &Link,
    state: &mut XservState,
    command: &XservCommand,
) -> Result<Option<Vec<u8>>> {
    let options = state.options.clone();
    if command.has_reply() {
        let data = ask(link, &options, command.byte()).await?;
        state.settle = false;
        return Ok(Some(data));
    }
    let packet = command.packet()?;
    let Some(packet) = packet else {
        // Q: nothing comes back.
        quiet(link, options.quiet).await?;
        link.write(vec![command.byte()])?;
        link.flush().await?;
        state.settle = true;
        return Ok(None);
    };
    if state.settle {
        ask(link, &options, b'M').await?;
        state.settle = false;
    }
    quiet(link, options.quiet).await?;
    link.write(vec![command.byte()])?;
    link.flush().await?;
    // From here on the calculator may be busy, whatever happens.
    state.settle = true;
    for _ in 0..COMMAND_TRIES {
        link.write(packet.clone())?;
        link.flush().await?;
        if wait_ack(link, options.timeout).await? {
            return Ok(None);
        }
        quiet(link, options.quiet).await?;
    }
    Err(Error::Reply(format!(
        "XSERV {}: no ACK for the command packet after {COMMAND_TRIES} tries",
        char::from(command.byte())
    )))
}

/// The listing of the current directory (`L`).
pub(crate) async fn list(link: &Link, state: &mut XservState) -> Result<Vec<DirRecord>> {
    let data = command(link, state, &XservCommand::List)
        .await?
        .unwrap_or_default();
    parse_dir_list(&data).map_err(|e| Error::Reply(format!("XSERV L: {e}")))
}

/// The free memory in bytes (`M`).
pub(crate) async fn memory(link: &Link, state: &mut XservState) -> Result<f64> {
    let data = command(link, state, &XservCommand::Memory)
        .await?
        .unwrap_or_default();
    std::str::from_utf8(&data)
        .ok()
        .and_then(parse_real)
        .ok_or_else(|| {
            Error::Reply(format!(
                "XSERV M: {:?} is no number",
                String::from_utf8_lossy(&data)
            ))
        })
}

/// Send `M` once and wait `wait` for the answer: the free memory, or `None`
/// when nothing that reads as one comes (no XSERV there). Late input is
/// dropped first; what arrives besides the answer is ignored.
pub(crate) async fn probe(
    link: &Link,
    state: &mut XservState,
    wait: Duration,
) -> Result<Option<f64>> {
    quiet(link, state.options.quiet).await?;
    link.write(vec![b'M'])?;
    link.flush().await?;
    let deadline = link.now() + wait;
    let mut heard = Vec::new();
    loop {
        if let Some(free) = memory_reply(&heard) {
            link.write(vec![ACK])?;
            link.flush().await?;
            state.settle = false;
            return Ok(Some(free));
        }
        let bytes = link.read(Some(deadline)).await?;
        if bytes.is_empty() {
            return Ok(None);
        }
        heard.extend_from_slice(&bytes);
    }
}

/// End XSERV (`Q`), then ask `M` once: `true` when nothing answers (XSERV
/// has ended), `false` when it still runs (a ROM without `Q`).
pub(crate) async fn quit(link: &Link, state: &mut XservState) -> Result<bool> {
    command(link, state, &XservCommand::Quit).await?;
    let wait = state.options.answer;
    Ok(probe(link, state, wait).await?.is_none())
}

/// Download `name` from the current directory (`G`, then XModem).
pub(crate) async fn get(
    link: &Link,
    state: &mut XservState,
    name: &str,
    options: &XmodemOptions,
) -> Result<XmodemReceived> {
    command(link, state, &XservCommand::Get(name.to_string())).await?;
    xmodem::receive(link, options).await
}

/// Upload `data` as `name` into the current directory (`P`, then XModem).
pub(crate) async fn put(
    link: &Link,
    state: &mut XservState,
    name: &str,
    data: &[u8],
    options: &XmodemOptions,
) -> Result<XmodemReport> {
    command(link, state, &XservCommand::Put(name.to_string())).await?;
    xmodem::send(link, options, data).await
}

/// The XModem options of an XSERV transfer: the 49G's (HP's CRC, 1k
/// blocks), the transfer starting at once, so the start window is
/// `start_timeout`.
pub fn transfer_options(start_timeout: Duration) -> XmodemOptions {
    let mut options = XmodemOptions::for_model(crate::Model::Hp49G).unwrap_or_default();
    options.start_timeout = start_timeout;
    options
}

/// A calculator running XSERV, over a blocking [`Transport`] with the
/// system clock. For a link driven from outside, see the XSERV operations
/// of [`Machine`](crate::machine::Machine), which run the same code.
#[cfg(not(target_arch = "wasm32"))]
pub struct XservSession {
    transport: Box<dyn Transport>,
    link: Link,
    state: XservState,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for XservSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XservSession")
            .field("options", &self.state.options)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl XservSession {
    /// Wrap an open link.
    pub fn new(transport: Box<dyn Transport>, options: XservOptions) -> Self {
        XservSession {
            transport,
            link: Link::new(Instant::now()),
            state: XservState::new(options),
        }
    }

    /// Give the link back.
    pub fn into_transport(self) -> Box<dyn Transport> {
        self.transport
    }

    fn drive<T>(
        &mut self,
        f: impl AsyncFnOnce(&Link, &mut XservState) -> T,
        progress: &mut dyn FnMut(&crate::link::Progress),
    ) -> T {
        let link = self.link.clone();
        let state = &mut self.state;
        crate::link::drive(self.transport.as_mut(), &link, f(&link, state), progress)
    }

    /// Send `M` and wait `wait` for XSERV's answer: the free memory, or
    /// `None` when nothing answers as XSERV does.
    pub fn probe(&mut self, wait: Duration) -> Result<Option<f64>> {
        self.drive(async |l, s| probe(l, s, wait).await, &mut |_| {})
    }

    /// The listing of the current directory (`L`).
    pub fn list(&mut self) -> Result<Vec<DirRecord>> {
        self.drive(async |l, s| list(l, s).await, &mut |_| {})
    }

    /// The free memory in bytes (`M`).
    pub fn memory(&mut self) -> Result<f64> {
        self.drive(async |l, s| memory(l, s).await, &mut |_| {})
    }

    /// Run RPL text (`E`); its results stay on the calculator's stack.
    pub fn execute(&mut self, rpl: &str) -> Result<()> {
        let cmd = XservCommand::Execute(rpl.to_string());
        self.drive(
            async |l, s| command(l, s, &cmd).await.map(|_| ()),
            &mut |_| {},
        )
    }

    /// End XSERV (`Q`): `true` when it has ended, `false` when it still
    /// answers.
    pub fn quit(&mut self) -> Result<bool> {
        self.drive(async |l, s| quit(l, s).await, &mut |_| {})
    }

    /// Download `name` from the current directory.
    pub fn get(
        &mut self,
        name: &str,
        options: &XmodemOptions,
        progress: &mut dyn FnMut(&xmodem_proto::Event),
    ) -> Result<XmodemReceived> {
        self.drive(async |l, s| get(l, s, name, options).await, &mut |p| {
            if let crate::link::Progress::Xmodem(e) = p {
                progress(e);
            }
        })
    }

    /// Upload `data` as `name` into the current directory.
    pub fn put(
        &mut self,
        name: &str,
        data: &[u8],
        options: &XmodemOptions,
        progress: &mut dyn FnMut(&xmodem_proto::Event),
    ) -> Result<XmodemReport> {
        self.drive(
            async |l, s| put(l, s, name, data, options).await,
            &mut |p| {
                if let crate::link::Progress::Xmodem(e) = p {
                    progress(e);
                }
            },
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn encode_by_hand() {
        // "AB": length 0x0002, 0x41 + 0x42 = 0x83.
        assert_eq!(
            encode_packet(b"AB").unwrap(),
            [0x00, 0x02, b'A', b'B', 0x83]
        );
        assert_eq!(encode_packet(b"").unwrap(), [0x00, 0x00, 0x00]);
        // 300 bytes of 0xFF: length 0x012C, sum 300 * 255 mod 256 = 0xD4.
        let big = encode_packet(&[0xFF; 300]).unwrap();
        assert_eq!(&big[..2], &[0x01, 0x2C]);
        assert_eq!(big.len(), 303);
        assert_eq!(big[302], 0xD4);
        assert_eq!(
            encode_packet(&vec![0; MAX_DATA + 1]),
            Err(FrameError::TooLong(MAX_DATA + 1))
        );
        assert_eq!(
            encode_packet(&vec![0; MAX_DATA]).unwrap().len(),
            MAX_DATA + 3
        );
    }

    #[test]
    fn decode_by_hand() {
        assert_eq!(
            decode_packet(&[0x00, 0x02, b'A', b'B', 0x83]).unwrap(),
            b"AB"
        );
        assert_eq!(
            decode_packet(&[0x00, 0x02, b'A', b'B', 0x84]),
            Err(FrameError::BadChecksum {
                expected: 0x83,
                got: 0x84
            })
        );
        assert_eq!(
            decode_packet(&[0x00, 0x02, b'A']),
            Err(FrameError::Truncated)
        );
        assert_eq!(decode_packet(&[0x00]), Err(FrameError::Truncated));
        assert_eq!(
            decode_packet(&[0x00, 0x00, 0x00, 0x07]),
            Err(FrameError::TrailingBytes(1))
        );
    }

    #[test]
    fn deframer_handles_any_chunking() {
        let mut wire = encode_packet(b"HOME").unwrap();
        wire.extend(encode_packet(&[0x10; 256]).unwrap());
        let mut d = Deframer::new();
        let mut got = Vec::new();
        for byte in &wire {
            d.push(&[*byte]);
            while let Some(p) = d.next_packet() {
                got.push(p.unwrap());
            }
        }
        assert_eq!(got, vec![b"HOME".to_vec(), vec![0x10; 256]]);
        assert_eq!(d.pending(), 0);
    }

    #[test]
    fn deframer_consumes_a_bad_packet() {
        let mut d = Deframer::new();
        d.push(&[0x00, 0x01, b'x', 0x00]);
        d.push(&encode_packet(b"y").unwrap());
        assert!(matches!(
            d.next_packet(),
            Some(Err(FrameError::BadChecksum { .. }))
        ));
        assert_eq!(d.next_packet(), Some(Ok(b"y".to_vec())));
        d.push(&[0x00]);
        d.clear();
        assert_eq!(d.next_packet(), None);
    }

    #[test]
    fn commands() {
        let put = XservCommand::Put("ABC".into());
        assert_eq!(put.byte(), b'P');
        // 0x41 + 0x42 + 0x43 = 0xC6.
        assert_eq!(
            put.packet().unwrap(),
            Some(vec![0x00, 0x03, b'A', b'B', b'C', 0xC6])
        );
        assert_eq!(XservCommand::Get("X".into()).byte(), b'G');
        assert_eq!(
            XservCommand::Get("X".into()).packet().unwrap(),
            Some(vec![0x00, 0x01, b'X', b'X'])
        );
        assert!(XservCommand::Get("1X".into()).packet().is_err());
        assert!(XservCommand::Put("A B".into()).packet().is_err());

        // `\->` becomes the HP's arrow (0x8D).
        let exec = XservCommand::Execute("\\->A".into());
        assert_eq!(exec.byte(), b'E');
        assert_eq!(
            exec.packet().unwrap(),
            Some(vec![0x00, 0x02, 0x8D, b'A', 0x8D + b'A'])
        );
        assert_eq!(XservCommand::Memory.byte(), b'M');
        assert_eq!(XservCommand::List.byte(), b'L');
        assert_eq!(XservCommand::Memory.packet().unwrap(), None);
        assert_eq!(XservCommand::List.packet().unwrap(), None);
        assert_eq!(XservCommand::Quit.byte(), b'Q');
        assert_eq!(XservCommand::Quit.packet().unwrap(), None);
        assert!(!XservCommand::Quit.has_reply());
        assert!(XservCommand::List.has_reply() && XservCommand::Memory.has_reply());
        assert!(!put.has_reply());
    }

    #[test]
    fn dir_list_by_hand() {
        let data = [
            // "DIR1": directory #2A96, 22 nibbles, CRC 0x1234.
            &[4][..],
            b"DIR1",
            &[0x96, 0x2A, 22, 0, 0, 0x34, 0x12],
            // "S": string #2A2C, 0x012345 nibbles, CRC 0xBEEF.
            &[1],
            b"S",
            &[0x2C, 0x2A, 0x45, 0x23, 0x01, 0xEF, 0xBE],
        ]
        .concat();
        let list = parse_dir_list(&data).unwrap();
        assert_eq!(
            list,
            vec![
                DirRecord {
                    name: "DIR1".into(),
                    prolog: 0x2A96,
                    size_nibbles: 22,
                    crc: 0x1234
                },
                DirRecord {
                    name: "S".into(),
                    prolog: 0x2A2C,
                    size_nibbles: 0x012345,
                    crc: 0xBEEF
                },
            ]
        );
        assert!(list[0].is_directory() && !list[1].is_directory());
        assert_eq!(parse_dir_list(&[]).unwrap(), vec![]);
        assert_eq!(
            parse_dir_list(&data[..data.len() - 1]),
            Err(FrameError::Truncated)
        );
        // HP characters in names are decoded (0x8D is the arrow).
        let arrow = parse_dir_list(&[1, 0x8D, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(arrow[0].name, "→");
    }

    #[test]
    fn a_record_has_the_kermit_size_and_kind() {
        // Emulated 49G, ROM 2.15: XSERV 10.5 bytes, Kermit 16 for `A`; 22.5
        // and 31 for directory SUBD; 10.5 and 23 for ABCDEFGH.
        let r = |name: &str, prolog: u16, nibbles: u32| DirRecord {
            name: name.into(),
            prolog,
            size_nibbles: nibbles,
            crc: 0x7847,
        };
        assert_eq!(r("A", 0x2933, 21).size(), 16.0);
        assert_eq!(r("SUBD", 0x2A96, 45).size(), 31.0);
        assert_eq!(r("ABCDEFGH", 0x2933, 21).size(), 23.0);
        let e = r("SUBD", 0x2A96, 45).entry();
        assert_eq!(e.kind, "Directory");
        assert!(e.is_directory());
        assert_eq!(e.checksum, 0x7847);
        assert_eq!(r("X", 0x2A2C, 10).kind(), "String");
        assert_eq!(r("X", 0x1234, 10).kind(), "Object #1234");
    }

    #[test]
    fn the_memory_reply_is_found_in_what_was_heard() {
        let reply = encode_packet(b"245478.").unwrap();
        assert_eq!(memory_reply(&reply), Some(245_478.0));
        // An idle Kermit NAK before it, and the rest of the line after.
        let mut heard = b"\x01# N3\r".to_vec();
        heard.extend(&reply);
        heard.extend(b"xyz");
        assert_eq!(memory_reply(&heard), Some(245_478.0));
        assert_eq!(memory_reply(&reply[..reply.len() - 1]), None);
        let mut bad = reply.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert_eq!(memory_reply(&bad), None);
        assert_eq!(memory_reply(&encode_packet(b"hello").unwrap()), None);
        assert_eq!(memory_reply(b"\x01# N3\r"), None);
        assert_eq!(memory_reply(&[]), None);
    }

    use crate::transport::MemoryTransport;
    use std::sync::{Arc, Mutex};

    type Log = Arc<Mutex<Vec<Vec<u8>>>>;

    fn fast() -> XservOptions {
        XservOptions {
            quiet: Duration::from_millis(1),
            answer: Duration::from_millis(30),
            busy: Duration::from_millis(400),
            timeout: Duration::from_millis(30),
        }
    }

    /// A session against `peer`, which sees and answers every write.
    fn session(
        mut peer: impl FnMut(&[u8]) -> Vec<Vec<u8>> + Send + 'static,
    ) -> (XservSession, Log) {
        let log: Log = Arc::default();
        let seen = log.clone();
        let t = MemoryTransport::new(move |w| {
            seen.lock().unwrap().push(w.to_vec());
            peer(w)
        });
        (XservSession::new(Box::new(t), fast()), log)
    }

    #[test]
    fn a_probe_acks_the_answer_and_finds_nothing_in_silence() {
        let (mut s, log) = session(|w| match w {
            b"M" => vec![encode_packet(b"1000.").unwrap()],
            _ => vec![],
        });
        assert_eq!(s.probe(Duration::from_millis(50)).unwrap(), Some(1000.0));
        assert_eq!(*log.lock().unwrap(), [b"M".to_vec(), vec![ACK]]);
        let (mut s, log) = session(|_| vec![]);
        assert_eq!(s.probe(Duration::from_millis(50)).unwrap(), None);
        assert_eq!(*log.lock().unwrap(), [b"M".to_vec()]);
    }

    /// The calculator drops two `L` bytes while it is busy, then answers:
    /// the client sends it again.
    #[test]
    fn a_busy_calculator_gets_the_command_again() {
        let records = [&[1][..], b"S", &[0x2C, 0x2A, 0x0A, 0, 0, 0xEF, 0xBE]].concat();
        let reply = encode_packet(&records).unwrap();
        let mut dropped = 2;
        let (mut s, log) = session(move |w| match w {
            b"L" if dropped > 0 => {
                dropped -= 1;
                vec![]
            }
            b"L" => vec![reply.clone()],
            _ => vec![],
        });
        let list = s.list().unwrap();
        assert_eq!(list[0].name, "S");
        assert_eq!(
            *log.lock().unwrap(),
            [b"L".to_vec(), b"L".to_vec(), b"L".to_vec(), vec![ACK]]
        );
    }

    /// Nothing answers at all: an error naming XSERV, after the busy time.
    #[test]
    fn no_answer_gives_up() {
        let (mut s, _) = session(|_| vec![]);
        let err = s.memory().unwrap_err();
        assert!(err.to_string().contains("is XSERV running"), "{err}");
    }

    /// Before the first command with a packet, and after an `E`, the client
    /// asks `M` until the calculator answers; after an `L` it does not.
    #[test]
    fn a_command_packet_waits_for_a_ready_calculator() {
        let listing = encode_packet(&[]).unwrap();
        let (mut s, log) = session(move |w| match w {
            b"M" => vec![encode_packet(b"5.").unwrap()],
            b"L" => vec![listing.clone()],
            [b'E' | b'P' | b'G'] | [ACK] => vec![],
            _ => vec![vec![ACK]],
        });
        s.execute("HOME").unwrap();
        s.execute("'X' PURGE").unwrap();
        assert!(s.list().unwrap().is_empty());
        s.execute("1").unwrap();
        let sent: Vec<Vec<u8>> = log.lock().unwrap().clone();
        let bytes: Vec<&[u8]> = sent.iter().map(Vec::as_slice).collect();
        let home = encode_packet(b"HOME").unwrap();
        let purge = encode_packet(b"'X' PURGE").unwrap();
        let one = encode_packet(b"1").unwrap();
        assert_eq!(
            bytes,
            [
                &b"M"[..],
                &[ACK],
                b"E",
                &home,
                b"M",
                &[ACK],
                b"E",
                &purge,
                b"L",
                &[ACK],
                b"E",
                &one,
            ]
        );
    }

    #[test]
    fn quit_says_whether_xserv_ended() {
        let (mut s, log) = session(|_| vec![]);
        assert!(s.quit().unwrap());
        assert_eq!(*log.lock().unwrap(), [b"Q".to_vec(), b"M".to_vec()]);
        let (mut s, _) = session(|w| match w {
            b"M" => vec![encode_packet(b"5.").unwrap()],
            _ => vec![],
        });
        assert!(!s.quit().unwrap());
    }

    /// A NAKed command packet is sent again; a reply with a bad checksum is
    /// NAKed and read again.
    #[test]
    fn naks_both_ways() {
        let mut naks = 1;
        let (mut s, log) = session(move |w| match w {
            b"M" => vec![encode_packet(b"5.").unwrap()],
            [b'E'] | [ACK] => vec![],
            _ if naks > 0 => {
                naks -= 1;
                vec![vec![NAK]]
            }
            _ => vec![vec![ACK]],
        });
        s.execute("X").unwrap();
        let packet = encode_packet(b"X").unwrap();
        assert_eq!(
            *log.lock().unwrap(),
            [
                b"M".to_vec(),
                vec![ACK],
                b"E".to_vec(),
                packet.clone(),
                packet
            ]
        );

        let good = encode_packet(b"42").unwrap();
        let mut bad = good.clone();
        *bad.last_mut().unwrap() ^= 0xFF;
        let (mut s, log) = session(move |w| match w {
            b"M" => vec![bad.clone()],
            [NAK] => vec![good.clone()],
            _ => vec![],
        });
        assert_eq!(s.memory().unwrap(), 42.0);
        assert_eq!(*log.lock().unwrap(), [b"M".to_vec(), vec![NAK], vec![ACK]]);
    }
}
