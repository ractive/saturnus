//! The hidden Kermit transaction: what changes the calculator's user
//! memory from outside (`storeFile`, `fetchFile`, `purge`, `rename`,
//! `createDir`, `changeDir`, `setFlag`, `storeText`, `copy`, `move` in
//! `web/protocol.md`)
//! goes through
//! the ROM's own Kermit server, so its memory manager stays consistent;
//! RAM is never written. `storeText` (the palette's editor) sends its text
//! as a string variable and has the calculator compile it with a host
//! command ([`Op::StoreText`]).
//!
//! A [`Transfer`] types `SERVER` on the command line (the typing engine,
//! [`crate::typing`]), waits for the idle server's first NAK, runs its
//! Kermit transactions (`kermit-proto`'s client over the emulated serial
//! port, on emulated time only), ends the server with `G F` and waits until
//! the calculator shows its stack again. It is stepped in emulated time
//! like a send, so a host runs it in turns at the speed the machine allows
//! (a calculator waiting for a packet sleeps, which costs nearly nothing),
//! with the screen held: what the server draws is never shown.
//!
//! The stack is left as it was: every transaction starts with an empty host
//! command, whose reply counts the levels, and a command that failed has
//! its leftover arguments dropped before the server ends. Flag -35 (the
//! transfer format) is set as each transfer needs and put back; the
//! current directory is put back after a store, fetch, purge or rename in
//! another directory. As on a real calculator, the server keeps its I/O
//! parameters in `IOPAR` in HOME, creating it on first use (purged, it
//! comes back with the server's end).
//!
//! The 49G in algebraic mode (flag -95 set) cannot enter the server as it
//! is: a server entered from it leaves the stack packed in a list. Its
//! writes clear -95 by keys first (`CF(-95)`, its echo dropped), run the
//! server in RPN mode and set -95 again by keys once the server has ended
//! (`-95 SF`), on every path: success, the calculator's refusal, a dead
//! server ended with ON, a stop. A flag alone is set or cleared by keys
//! (`SF(n)`, `CF(n)`) without the server. Wiki: protocols/kermit,
//! protocols/server-commands.

use std::collections::VecDeque;

use kermit_proto::time::{Duration, Instant};
use kermit_proto::{Client, Command, Config, Event, OutgoingFile};
use saturnus::io::Key;
use saturnus::{Machine, Model};
use saturnus_objects::transfer::{self as tfile, is_plain_name};
use saturnus_objects::{Flags, UserMemory, Variable, charset, cmdline, prolog};

use crate::typing::{Job, Verb};

/// Largest file `storeFile` takes (the 49G's RAM is 512 KiB, a 48's at
/// most 256 KiB with cards; a transfer of this size takes minutes of
/// emulated time at 9600 baud).
pub const MAX_FILE_BYTES: usize = 512 * 1024;
/// Longest emulated wait for the idle server's first NAK after `SERVER`
/// (it NAKs about every 5 s while idle).
pub const SERVER_CAP_MS: u64 = 15_000;
/// Emulated pause before a packet that opens a Kermit transaction: a
/// command right after the final ACK of the one before is lost.
pub const TURNAROUND_MS: u64 = 200;
/// Emulated time after `G F` before the calculator takes keys again (the
/// 48SX loses keys typed within 2 s of FINISH).
pub const FINISH_SETTLE_MS: u64 = 2_500;
/// How long the screen must stay unchanged, with the CPU asleep, to count
/// as back at the stack.
const STABLE_MS: u64 = 300;
/// Longest wait for that, after [`FINISH_SETTLE_MS`].
const IDLE_CAP_MS: u64 = 10_000;
/// How long ON is held to stop a server that no longer answers.
const ON_HOLD_MS: u64 = 100;
/// Presses of ON at most to end a server on a stop.
const ON_TRIES: usize = 4;
/// Longest emulated time between an idle server's NAKs (about 5 s).
const SERVER_IDLE_NAK_MS: u64 = 7_000;
/// Reply timeout per packet on the link's clock (idle emulated time).
const TIMEOUT: Duration = Duration::from_secs(6);
/// Retransmissions per packet.
const RETRIES: u32 = 3;
/// Clock advance while a transaction runs but the client has no deadline.
const IDLE_WAIT: Duration = Duration::from_millis(100);
/// Shortest read wait.
const MIN_WAIT: Duration = Duration::from_millis(10);
/// Longest emulated time one read keeps running a busy calculator.
const MAX_BUSY: Duration = Duration::from_secs(600);
/// Emulated time run per step while reading.
const READ_STEP_MS: u64 = 1;
/// Emulated quiet time after the last transmitted byte that ends a read
/// (a byte takes about 1 ms at 9600 baud; a packet comes back to back).
const QUIET_MS: u64 = 4;
/// Emulated time per step while waiting for the server's first NAK.
const NAK_STEP_MS: u64 = 10;
/// Pause after a dropping key press (the ROM sees a release at its next
/// 1/16 s poll).
const DROP_GAP_MS: u64 = 100;

/// What a write does (`web/protocol.md`, "The user memory, written").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Store `data` (an HP binary file, or text) as `name` in `dir`.
    Store {
        dir: Vec<String>,
        name: String,
        data: Vec<u8>,
    },
    /// Fetch variable `name` of `dir` as an HP binary file.
    Fetch { dir: Vec<String>, name: String },
    /// Purge variable `name` of `dir` (a directory with all it holds).
    Purge { dir: Vec<String>, name: String },
    /// Rename variable `name` of `dir` to `to`.
    Rename {
        dir: Vec<String>,
        name: String,
        to: String,
    },
    /// Create the empty directory `name` in `dir` (`CRDIR`).
    CreateDir { dir: Vec<String>, name: String },
    /// Make `dir` the current directory.
    ChangeDir { dir: Vec<String> },
    /// Copy variable `name` of `dir` (a directory with all it holds) into
    /// directory `to`, replacing a variable of that name there only when
    /// `replace`; `remove` (a move) then purges the original, once the
    /// copy is checked to be the same object.
    Copy {
        dir: Vec<String>,
        name: String,
        to: Vec<String>,
        replace: bool,
        remove: bool,
    },
    /// Set (`on`) or clear flag `flag` (negative: a system flag).
    SetFlag { flag: i32, on: bool },
    /// Compile `text` on the calculator and put the object at `target`
    /// (`dir` is the variable's directory; a stack level is in the current
    /// one).
    StoreText {
        dir: Vec<String>,
        target: Target,
        text: String,
    },
}

/// Where `storeText` puts its object, and what `editText` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The variable of this name.
    Variable(String),
    /// This stack level (1 is the top).
    Level(usize),
}

/// The name of the string variable `storeText` sends its text in (a
/// number is added when the directory holds one already).
pub const TEXT_VARIABLE: &str = "SATEDIT";

/// A finished write's reply (`web/protocol.md`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    /// Emulated ms it took.
    pub emulated_ms: f64,
    /// `storeFile`: the name the calculator stored the file under (it adds
    /// `.1` when the name is taken and flag -36 is clear); `fetchFile`: the
    /// variable fetched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `fetchFile`: the file's size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
    /// Done by keys rather than the Kermit server (the 49G in algebraic
    /// mode).
    pub keys: bool,
    /// `storeText`: why the calculator did not compile the text (its own
    /// message, as `Invalid Syntax`, or that the text is not one object);
    /// nothing was stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One planned step.
#[derive(Clone, Debug)]
enum Step {
    /// Type the text and ENTER (`SERVER`, or a key fallback).
    Type(String),
    /// Wait for the idle server's first NAK.
    WaitNak,
    /// A host command; `cleanup` steps run after an error too.
    Host { text: String, cleanup: bool },
    /// SEND a file.
    Send { name: String, data: Vec<u8> },
    /// GET a variable in binary.
    Get { name: String },
    /// A host command that compiles the text sent as a string: its reply
    /// must show two levels more than the baseline, the list the text was
    /// wrapped in and its size (at most 2), 1.
    Compile { text: String },
    /// End the server.
    Finish,
    /// Wait until the calculator shows its stack again.
    Settle,
    /// Drop (the backspace key) what the stack holds beyond this many
    /// levels: the echo algebraic mode leaves.
    DropTo(usize),
    /// Set -95 again by keys (`-95 SF`, typed in RPN mode) if it is clear:
    /// the 49G was in algebraic mode when the write began.
    Algebraic,
    /// After a stop: a line typed in part is cancelled with ON.
    CancelLine,
    /// End a server that no longer answers, or one a stop interrupted:
    /// ON, then again while it still NAKs ([`ON_TRIES`] at most).
    EndServer,
}

impl Step {
    /// Whether the step runs after an error.
    fn cleanup(&self) -> bool {
        match self {
            Step::Host { cleanup, .. } => *cleanup,
            Step::Finish
            | Step::Settle
            | Step::DropTo(_)
            | Step::Algebraic
            | Step::CancelLine
            | Step::EndServer => true,
            _ => false,
        }
    }

    /// Whether the step runs after the server, so it stays when a dead
    /// server is ended with ON.
    fn after_server(&self) -> bool {
        matches!(self, Step::Algebraic | Step::DropTo(_))
    }
}

/// The serial link on emulated time: the client's clock is a fixed start
/// plus the emulated time the calculator idled while a reply was awaited,
/// so an exchange always leaves the same machine state.
#[derive(Debug)]
struct Wire {
    rx: VecDeque<u8>,
    start: Instant,
    idle: Duration,
}

impl Wire {
    fn now(&self) -> Instant {
        self.start + self.idle
    }

    /// Run `n` cycles; true if the calculator transmitted.
    fn run(&mut self, m: &mut Machine, n: u64) -> crate::Result<bool> {
        m.run_cycles(n.max(1))?;
        let out = m.serial_drain();
        self.rx.extend(&out);
        Ok(!out.is_empty())
    }

    /// Drop what the idle server sent (its periodic NAKs).
    fn discard(&mut self, m: &mut Machine) {
        self.rx.clear();
        m.serial_drain();
    }
}

/// What one Kermit transaction produced.
#[derive(Debug, Default)]
struct Transcript {
    files: Vec<Vec<u8>>,
    stored: Vec<String>,
    text: Vec<u8>,
}

/// Where a transaction's driver is.
#[derive(Debug)]
enum Phase {
    /// Hand out packets and events, then read.
    Pump,
    /// Run until `until` (a cycle count), then put `packet` on the line.
    Turnaround { until: u64, packet: Vec<u8> },
    /// Collect the reply until the line is quiet or `deadline`.
    Read {
        deadline: Instant,
        busy: Duration,
        last_byte: u64,
    },
}

/// One Kermit transaction in progress.
#[derive(Debug)]
struct Exchange {
    client: Client,
    sending: bool,
    out: VecDeque<Vec<u8>>,
    phase: Phase,
    transcript: Transcript,
    current: Option<Vec<u8>>,
    outcome: Option<Result<(), Failure>>,
}

/// Whether `packet` (SOH, LEN, SEQ, TYPE, ...) opens a transaction.
fn opens_transaction(packet: &[u8]) -> bool {
    let start = packet.iter().position(|&b| b == 0x01);
    start
        .and_then(|i| packet.get(i + 3))
        .is_some_and(|t| matches!(t, b'S' | b'R' | b'I' | b'G' | b'C'))
}

/// Why a Kermit transaction failed.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Failure {
    /// The calculator refused it with an E packet: its server is alive
    /// and waits for the next command.
    Remote(String),
    /// A timeout or a protocol error: the server cannot be trusted to
    /// answer any more.
    Link(String),
}

/// `kermit-proto`'s defaults with a shorter timeout, fewer retries and no
/// linger (the next command comes after the turnaround anyway).
fn config() -> Config {
    let mut c = Config::default();
    c.timeout = TIMEOUT;
    c.retries = RETRIES;
    c.linger = Duration::ZERO;
    c
}

fn ms_cycles(m: &Machine, ms: u64) -> u64 {
    ms * u64::from(m.model().clock_hz()) / 1000
}

impl Exchange {
    fn new(wire: &mut Wire, m: &mut Machine, command: Command) -> crate::Result<Exchange> {
        wire.discard(m);
        let sending = matches!(command, Command::Send(_));
        let mut client = Client::new(config());
        client
            .start(wire.now(), command)
            .map_err(|e| format!("Kermit: {e}"))?;
        Ok(Exchange {
            client,
            sending,
            out: VecDeque::new(),
            phase: Phase::Pump,
            transcript: Transcript::default(),
            current: None,
            outcome: None,
        })
    }

    /// Run until `stop` (a cycle count); the transcript once done.
    fn step(
        &mut self,
        wire: &mut Wire,
        m: &mut Machine,
        stop: u64,
    ) -> crate::Result<Option<Result<Transcript, Failure>>> {
        loop {
            match &mut self.phase {
                Phase::Pump => {
                    let now = wire.now();
                    while let Some(p) = self.client.poll_output(now) {
                        self.out.push_back(p);
                    }
                    self.events();
                    if let Some(packet) = self.out.pop_front() {
                        if opens_transaction(&packet) {
                            let until = m.cycles() + ms_cycles(m, TURNAROUND_MS);
                            self.phase = Phase::Turnaround { until, packet };
                        } else {
                            m.serial_push(&packet);
                        }
                        continue;
                    }
                    let deadline = match (self.client.next_timeout(), &self.outcome) {
                        (None, Some(_)) => {
                            let outcome = self.outcome.take().unwrap_or(Ok(()));
                            let t = std::mem::take(&mut self.transcript);
                            return Ok(Some(outcome.map(|()| t)));
                        }
                        (Some(t), _) => t.max(now + MIN_WAIT),
                        (None, None) => now + IDLE_WAIT,
                    };
                    self.phase = Phase::Read {
                        deadline,
                        busy: Duration::ZERO,
                        last_byte: m.cycles(),
                    };
                }
                Phase::Turnaround { until, packet } => {
                    if m.cycles() < *until {
                        if m.cycles() >= stop {
                            return Ok(None);
                        }
                        let n = (*until).min(stop) - m.cycles();
                        wire.run(m, n)?;
                    } else {
                        m.serial_push(packet);
                        self.phase = Phase::Pump;
                    }
                }
                Phase::Read {
                    deadline,
                    busy,
                    last_byte,
                } => {
                    let quiet = !wire.rx.is_empty()
                        && m.cycles() - *last_byte >= ms_cycles(m, QUIET_MS)
                        && m.serial_pending() == 0;
                    if quiet || wire.now() >= *deadline {
                        let input: Vec<u8> = wire.rx.drain(..).collect();
                        let now = wire.now();
                        if !input.is_empty() {
                            self.client.handle_input(now, &input);
                        }
                        self.client.handle_timeout(now);
                        self.phase = Phase::Pump;
                        continue;
                    }
                    if m.cycles() >= stop {
                        return Ok(None);
                    }
                    let step = Duration::from_millis(READ_STEP_MS);
                    if wire.run(m, ms_cycles(m, READ_STEP_MS))? {
                        *last_byte = m.cycles();
                    } else if wire.rx.is_empty() && !m.is_shutdown() && *busy < MAX_BUSY {
                        // Still computing the reply: not idle time.
                        *busy += step;
                        continue;
                    }
                    wire.idle += step;
                }
            }
        }
    }

    fn events(&mut self) {
        while let Some(event) = self.client.poll_event() {
            match event {
                Event::FileStart { name } if self.sending => {
                    self.transcript.stored.push(charset::decode(&name));
                }
                Event::FileStart { .. } => self.current = Some(Vec::new()),
                Event::Data(data) => {
                    if let Some(file) = self.current.as_mut() {
                        file.extend_from_slice(&data);
                    }
                }
                Event::FileEnd { discarded } => {
                    if let Some(file) = self.current.take()
                        && !discarded
                    {
                        self.transcript.files.push(file);
                    }
                }
                Event::ServerText(text) => self.transcript.text.extend_from_slice(&text),
                Event::Done => self.outcome = Some(Ok(())),
                Event::Error(kermit_proto::Error::Remote(text)) => {
                    self.outcome = Some(Err(Failure::Remote(format!(
                        "calculator error: {}",
                        charset::decode(&text)
                    ))));
                }
                Event::Error(e) => self.outcome = Some(Err(Failure::Link(format!("Kermit: {e}")))),
                _ => {}
            }
        }
    }
}

/// The step running now.
#[derive(Debug)]
enum Running {
    Typing(Box<Job>),
    WaitNak {
        cap: u64,
    },
    Exchange(Box<Exchange>, Step),
    /// Ending a server with ON: `tries` presses so far.
    EndServer {
        tries: usize,
        phase: EndPhase,
        until: u64,
    },
    /// Cancelling a line typed in part.
    CancelLine {
        phase: CancelPhase,
        until: u64,
    },
    Settle(SettleState),
    /// Presses of the backspace key still to make; the current one is
    /// down until `up` (a cycle count), then the CPU is waited for until
    /// `next`.
    Drop {
        left: usize,
        up: u64,
        next: u64,
    },
}

/// Where [`Running::EndServer`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndPhase {
    /// ON is down.
    Hold,
    /// The calculator returns to its stack.
    Settle,
    /// Does it still NAK (the server still runs)?
    Listen,
}

/// Where [`Running::CancelLine`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CancelPhase {
    /// The calculator reads the last key.
    Idle,
    /// ON is down.
    Hold,
    /// The line closes.
    Gap,
}

/// Waiting for the stack after `G F` (or after ON).
#[derive(Debug)]
struct SettleState {
    /// Run plainly until this cycle count.
    until: u64,
    /// Then until the screen is stable, at most until this one.
    cap: u64,
    last: Option<saturnus::Lcd>,
    since: u64,
}

impl SettleState {
    fn new(m: &Machine, first_ms: u64) -> SettleState {
        let until = m.cycles() + ms_cycles(m, first_ms);
        SettleState {
            until,
            cap: until + ms_cycles(m, IDLE_CAP_MS),
            last: None,
            since: until,
        }
    }
}

/// A write in progress: step it until [`Transfer::step`] says done.
#[derive(Debug)]
pub struct Transfer {
    steps: VecDeque<Step>,
    running: Option<Running>,
    wire: Wire,
    /// The server runs (typed and not ended).
    server: bool,
    /// `SERVER` is being typed: once its ENTER is in, the server may run
    /// before its first NAK says so.
    entering: bool,
    /// The number of stack levels before the first command.
    baseline: Option<usize>,
    /// The first error; the cleanup steps still run.
    error: Option<String>,
    result: TransferResult,
    fetched: Option<Vec<u8>>,
    /// The file sent is `storeText`'s string, which must keep its name.
    text_variable: bool,
    started: u64,
    done: bool,
    /// The 49G was in algebraic mode, with this many stack levels: -95 is
    /// set again after the server, also on a stop.
    algebraic: Option<usize>,
    /// The keys typed now set -95 again.
    restoring: bool,
    /// Why -95 could not be set again (reported with any other error).
    restore_error: Option<String>,
    /// Stopped ([`Transfer::stop`]): only the cleanup runs now.
    stopped: bool,
}

/// The error of a write that was stopped (the host says why).
pub const STOPPED: &str = "stopped";
/// Emulated time per turn of [`crate::Emulator::stop_transfer_now`].
const STOP_TURN_MS: f64 = 50.0;

/// What a 49G write types to clear algebraic mode, and to set it again.
const TO_RPN: &str = "CF(-95)";
const TO_ALGEBRAIC: &str = "-95 SF";

/// Why `model`'s user memory cannot be written, or `None`.
fn no_server(model: Model) -> Option<String> {
    (!matches!(model, Model::Hp48sx | Model::Hp48gx | Model::Hp49g)).then(|| {
        format!(
            "the {} has no Kermit server and no RPL user memory to write",
            model.name().to_uppercase()
        )
    })
}

/// `name` as a quoted global name for a host command.
fn quoted(name: &str) -> String {
    format!("'{name}'")
}

fn check_name(name: &str) -> crate::Result<()> {
    if is_plain_name(name) {
        Ok(())
    } else {
        Err(format!(
            "{name:?} is not a plain variable name (1 to 127 characters, no digit or point first, \
             no spaces, delimiters or operators)"
        )
        .into())
    }
}

/// `dir` without its leading `HOME`, every component a plain name.
fn components(dir: &[String]) -> crate::Result<&[String]> {
    let rest = match dir.split_first() {
        Some((home, rest)) if home == "HOME" => rest,
        _ => dir,
    };
    for name in rest {
        check_name(name)?;
    }
    Ok(rest)
}

/// The host command that makes `dir` current (`HOME A B`).
fn cd_command(dir: &[String]) -> String {
    std::iter::once("HOME")
        .chain(dir.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The variables of `dir` (components below HOME), or why not.
fn directory<'a>(tree: &'a [Variable], dir: &[String]) -> crate::Result<&'a [Variable]> {
    let mut vars = tree;
    let mut path = String::from("HOME");
    for name in dir {
        path.push(' ');
        path.push_str(name);
        vars = vars
            .iter()
            .find(|v| &v.name == name)
            .and_then(|v| v.variables.as_deref())
            .ok_or_else(|| format!("no directory {{ {path} }}"))?;
    }
    Ok(vars)
}

/// Whether `path` (components below HOME) lies inside `dir`/`name`.
fn inside(path: &[String], dir: &[String], name: &str) -> bool {
    path.len() > dir.len() && path.starts_with(dir) && path[dir.len()] == name
}

/// The flag's range on `model`: -64..64 on the 48s, -128..128 on the 49G.
fn flag_range(model: Model) -> i32 {
    if model == Model::Hp49g { 128 } else { 64 }
}

/// What the file is stored as: binary files as they are; text, when it is
/// UTF-8 in the calculator's character set, in that set (a PC's `«` is two
/// UTF-8 bytes, the calculator's one), otherwise as it is. Returns the
/// bytes and whether they travel in binary mode (flag -35 set).
pub fn store_bytes(data: &[u8]) -> (Vec<u8>, bool) {
    if tfile::is_binary_file(data) {
        return (data.to_vec(), true);
    }
    let encoded = std::str::from_utf8(data)
        .ok()
        .and_then(|t| charset::encode(t).ok());
    (encoded.unwrap_or_else(|| data.to_vec()), false)
}

/// A received binary file cut to its object's length: the calculator pads
/// the last packet.
pub fn trim_fetched(mut data: Vec<u8>) -> Vec<u8> {
    if tfile::is_binary_file(&data) {
        let nibbles = tfile::unpack(&data[tfile::HEADER_LEN..]);
        if let Ok(size) = prolog::object_size(&nibbles, 0) {
            data.truncate(tfile::HEADER_LEN + size.div_ceil(2));
        }
    }
    data
}

/// Why `text` cannot be compiled inside the list `storeText` wraps it in,
/// or `None`. The wrapper must end where the calculator reads its end, so
/// what could move that end is refused before anything runs:
///
/// - a `}` that closes more than the text opened (the list would end early
///   and what follows would run);
/// - a string left open (it would swallow the wrapper's `}`);
/// - a `"` or `@` right after a word's character: the ROM starts a string
///   or a comment there, inside the word (`X@ 1` is `X` and a comment,
///   `A"B"` is `A` and `"B"`; wiki: protocols/server-commands), which is
///   easy to misread, so it is refused rather than followed.
///
/// Strings (`"` to `"`) and comments (`@` to the next `@` or the line's
/// end) are skipped as the ROM reads them.
fn text_refusal(text: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut string = false;
    let mut comment = false;
    let mut prev: Option<char> = None;
    for c in text.chars() {
        let mid_word = prev.is_some_and(|p| {
            !p.is_whitespace()
                && !matches!(
                    p,
                    '{' | '}' | '[' | ']' | '(' | ')' | '«' | '»' | '\'' | '"'
                )
        });
        match c {
            '"' | '@' if !string && !comment && mid_word => {
                return Some(format!(
                    "a {c} right after a word starts a new token on the calculator: put a space before it"
                ));
            }
            '"' if !comment => string = !string,
            '@' if !string => comment = !comment,
            '\n' if comment => comment = false,
            '{' if !string && !comment => depth += 1,
            '}' if !string && !comment => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return Some("a } closes a list the text did not open".into()),
            },
            _ => {}
        }
        prev = Some(c);
    }
    string.then(|| "the text leaves a string open (a \" is missing)".into())
}

/// What `copy` and `move` must know of a variable and its target.
#[derive(Clone, Copy, Debug, Default)]
struct CopyCase {
    /// The variable is a directory.
    is_dir: bool,
    /// The target holds a variable of that name: `Some(true)` a directory.
    taken: Option<bool>,
    /// The user said to replace it.
    replace: bool,
    /// A move: the original is purged after the copy.
    remove: bool,
    /// The variable is a directory that holds the current one.
    holds_here: bool,
}

/// The host commands of `copy` (and `move`) of `name` from `from` into
/// `into` (components below HOME), or why not. The copy is the
/// calculator's `RCL` and `STO`; a move then compares the two objects'
/// `BYTES` (size and checksum) and purges the original only when they
/// agree, else fails with the original kept.
fn copy_plan(
    from: &[String],
    name: &str,
    into: &[String],
    c: &CopyCase,
) -> crate::Result<Vec<Step>> {
    let src = cd_command(from);
    let dst = cd_command(into);
    if from == into {
        return Err(format!("{name} is in {{ {dst} }} already").into());
    }
    if c.is_dir && inside(into, from, name) {
        return Err(format!("{name} cannot go into itself or a directory inside it").into());
    }
    if c.remove && c.holds_here {
        return Err(
            format!("{name} holds the current directory: change to another one first").into(),
        );
    }
    match c.taken {
        Some(_) if !c.replace => {
            return Err(format!("{name} already exists in {{ {dst} }}").into());
        }
        Some(true) => {
            return Err(format!(
                "{name} in {{ {dst} }} is a directory, which a copy does not replace: purge it first"
            )
            .into());
        }
        Some(false) if c.is_dir => {
            return Err(format!(
                "a directory does not replace the variable {name} in {{ {dst} }}: purge it first"
            )
            .into());
        }
        _ => {}
    }
    // One command per packet: the levels a failed one leaves are dropped
    // (the baseline), so the steps may pass objects on the stack.
    let q = quoted(name);
    let mut texts = vec![
        src.clone(),
        format!("{q} RCL"),
        dst.clone(),
        format!("{q} STO"),
    ];
    if c.remove {
        let verb = if c.is_dir { "PGDIR" } else { "PURGE" };
        texts.extend([
            format!("{q} RCL BYTES"),
            src,
            format!("{q} RCL BYTES"),
            "ROT == 3 ROLLD == AND".into(),
            format!("« {q} {verb} » « \"{COPY_DIFFERS}\" DOERR » IFTE"),
        ]);
    }
    Ok(texts
        .into_iter()
        .map(|text| Step::Host {
            text,
            cleanup: false,
        })
        .collect())
}

/// The calculator's error when a move's copy is not the same object: the
/// original is not purged.
const COPY_DIFFERS: &str = "Copy differs";

/// The string `storeText` sends: the text in a list, which compiles
/// without running anything in it.
fn wrapped(text: &str) -> String {
    format!("{{\n{text}\n}}")
}

/// [`TEXT_VARIABLE`], or with a number added, whichever `vars` lacks.
fn free_text_name(vars: &[Variable]) -> String {
    (1..)
        .map(|n| match n {
            1 => TEXT_VARIABLE.to_string(),
            n => format!("{TEXT_VARIABLE}{n}"),
        })
        .find(|name| vars.iter().all(|v| &v.name != name))
        .unwrap_or_default()
}

/// The count `storeText`'s compile replies with, 0, 1 or 2 (two or more),
/// read in any display mode (`1`, `1.`, `1.000`, `1,000`).
fn reply_count(level: &str) -> Option<u32> {
    let first = level.trim().chars().next()?;
    first.to_digit(10).filter(|&n| n <= 2)
}

/// The object at `address` as `size:checksum` (nibbles, hex CRC).
fn identity(u: &UserMemory<'_>, address: u32) -> crate::Result<String> {
    let (size, crc) = u.identity_at(address).map_err(|e| format!("{e:#}"))?;
    Ok(format!("{size}:{crc:04X}"))
}

/// Whether the 49G is in algebraic mode (system flag -95).
fn algebraic(model: Model, flags: &Flags) -> bool {
    model == Model::Hp49g && flags.get(-95) == Some(true)
}

/// Whether the calculator's flag -95 reads clear now (RPN mode).
fn rpn_now(m: &Machine) -> bool {
    UserMemory::of(m)
        .and_then(|u| u.flags())
        .is_ok_and(|f| f.get(-95) == Some(false))
}

impl Transfer {
    /// Check `op` against the calculator as it is and plan it; nothing
    /// runs yet, and an error leaves the machine untouched.
    pub fn new(m: &Machine, op: Op) -> crate::Result<Transfer> {
        if let Some(why) = no_server(m.model()) {
            return Err(why.into());
        }
        let u = UserMemory::of(m).map_err(|e| format!("{e:#}"))?;
        let flags = u.flags().map_err(|e| format!("{e:#}"))?;
        let tree = u.tree().map_err(|e| format!("{e:#}"))?;
        let path = u.current_path().map_err(|e| format!("{e:#}"))?;
        let here: Vec<String> = path.iter().skip(1).cloned().collect();
        let line = cmdline::command_line(m).map_err(|e| format!("{e:#}"))?;
        if line.active {
            return Err(
                "the command line is open: finish it (ENTER) or cancel it (ON) first"
                    .to_string()
                    .into(),
            );
        }
        let mut result = TransferResult::default();
        let mut steps = VecDeque::new();
        let alg = algebraic(m.model(), &flags);
        if let Op::SetFlag { flag, on } = op {
            let max = flag_range(m.model());
            if flag == 0 || flag.abs() > max {
                return Err(format!("flag {flag} is not between -{max} and {max} (not 0)").into());
            }
            let verb = if on { "SF" } else { "CF" };
            if alg {
                // A server entered in algebraic mode packs the stack in a
                // list: keys instead.
                let depth = u.stack_addresses().map_err(|e| format!("{e:#}"))?.len();
                steps.push_back(Step::Type(format!("{verb}({flag})")));
                steps.push_back(Step::DropTo(depth));
                result.keys = true;
                return Ok(Transfer::planned(m, steps, result));
            }
            steps.push_back(Step::Host {
                text: format!("{flag} {verb}"),
                cleanup: false,
            });
            return Transfer::serve(m, &flags, steps, result);
        }
        let mut back = None;
        let text_variable = matches!(op, Op::StoreText { .. });
        match op {
            Op::ChangeDir { dir } => {
                let dir = components(&dir)?;
                directory(&tree, dir)?;
                steps.push_back(Step::Host {
                    text: cd_command(dir),
                    cleanup: false,
                });
            }
            Op::Store { dir, name, data } => {
                check_name(&name)?;
                if data.len() > MAX_FILE_BYTES {
                    return Err(format!("the file is larger than {MAX_FILE_BYTES} bytes").into());
                }
                let dir = components(&dir)?;
                directory(&tree, dir)?;
                let (data, binary) = store_bytes(&data);
                back = Transfer::enter(&mut steps, dir, &here);
                Transfer::with_mode(&mut steps, &flags, binary, Step::Send { name, data });
            }
            Op::Fetch { dir, name } => {
                check_name(&name)?;
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                if !vars.iter().any(|v| v.name == name) {
                    return Err(format!("no variable {name} in {{ {} }}", cd_command(dir)).into());
                }
                back = Transfer::enter(&mut steps, dir, &here);
                Transfer::with_mode(&mut steps, &flags, true, Step::Get { name: name.clone() });
                result.name = Some(name);
            }
            Op::Purge { dir, name } => {
                check_name(&name)?;
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                let var = vars
                    .iter()
                    .find(|v| v.name == name)
                    .ok_or_else(|| format!("no variable {name} in {{ {} }}", cd_command(dir)))?;
                if var.variables.is_some() && inside(&here, dir, &name) {
                    return Err(format!(
                        "{name} holds the current directory: change to another one first"
                    )
                    .into());
                }
                let verb = if var.variables.is_some() {
                    "PGDIR"
                } else {
                    "PURGE"
                };
                back = Transfer::enter(&mut steps, dir, &here);
                steps.push_back(Step::Host {
                    text: format!("{} {verb}", quoted(&name)),
                    cleanup: false,
                });
            }
            Op::Rename { dir, name, to } => {
                check_name(&name)?;
                check_name(&to)?;
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                let var = vars
                    .iter()
                    .find(|v| v.name == name)
                    .ok_or_else(|| format!("no variable {name} in {{ {} }}", cd_command(dir)))?;
                if vars.iter().any(|v| v.name == to) {
                    return Err(format!("{to} already exists in {{ {} }}", cd_command(dir)).into());
                }
                if var.variables.is_some() && inside(&here, dir, &name) {
                    return Err(format!(
                        "{name} holds the current directory: change to another one first"
                    )
                    .into());
                }
                let verb = if var.variables.is_some() {
                    "PGDIR"
                } else {
                    "PURGE"
                };
                back = Transfer::enter(&mut steps, dir, &here);
                // Copy, then purge the old name: on a failure the old one
                // is still there.
                steps.push_back(Step::Host {
                    text: format!("{} RCL {} STO", quoted(&name), quoted(&to)),
                    cleanup: false,
                });
                steps.push_back(Step::Host {
                    text: format!("{} {verb}", quoted(&name)),
                    cleanup: false,
                });
            }
            Op::CreateDir { dir, name } => {
                check_name(&name)?;
                let dir = components(&dir)?;
                let vars = directory(&tree, dir)?;
                if vars.iter().any(|v| v.name == name) {
                    return Err(
                        format!("{name} already exists in {{ {} }}", cd_command(dir)).into(),
                    );
                }
                back = Transfer::enter(&mut steps, dir, &here);
                steps.push_back(Step::Host {
                    text: format!("{} CRDIR", quoted(&name)),
                    cleanup: false,
                });
            }
            Op::Copy {
                dir,
                name,
                to,
                replace,
                remove,
            } => {
                check_name(&name)?;
                let from = components(&dir)?;
                let into = components(&to)?;
                let vars = directory(&tree, from)?;
                let var = vars
                    .iter()
                    .find(|v| v.name == name)
                    .ok_or_else(|| format!("no variable {name} in {{ {} }}", cd_command(from)))?;
                let targets = directory(&tree, into)?;
                let is_dir = var.variables.is_some();
                for step in copy_plan(
                    from,
                    &name,
                    into,
                    &CopyCase {
                        is_dir,
                        taken: targets
                            .iter()
                            .find(|v| v.name == name)
                            .map(|v| v.variables.is_some()),
                        replace,
                        remove,
                        holds_here: is_dir && inside(&here, from, &name),
                    },
                )? {
                    steps.push_back(step);
                }
                // The commands change directory: back to the current one.
                if here.iter().all(|n| is_plain_name(n)) {
                    back = Some(cd_command(&here));
                }
            }
            Op::StoreText { dir, target, text } => {
                if let Some(why) = text_refusal(&text) {
                    return Err(why.into());
                }
                let family = tfile::Family::of(m.model());
                let string = saturnus_objects::Object::String {
                    value: wrapped(&text),
                };
                let file = tfile::encode_file(&string, family)
                    .map_err(|e| format!("{e:#}"))?
                    .ok_or("a string has a binary file")?;
                if file.len() > MAX_FILE_BYTES {
                    return Err(format!("the text is longer than {MAX_FILE_BYTES} bytes").into());
                }
                let place = match &target {
                    Target::Variable(name) => {
                        check_name(name)?;
                        let dir = components(&dir)?;
                        let vars = directory(&tree, dir)?;
                        if vars
                            .iter()
                            .any(|v| &v.name == name && v.variables.is_some())
                        {
                            return Err(format!("{name} is a directory, which has no text").into());
                        }
                        back = Transfer::enter(&mut steps, dir, &here);
                        format!("DROP 1 GET {} STO", quoted(name))
                    }
                    Target::Level(n) => {
                        let depth = u.stack_addresses().map_err(|e| format!("{e:#}"))?.len();
                        if *n == 0 || *n > depth {
                            return Err(
                                format!("there is no level {n} (the stack has {depth})").into()
                            );
                        }
                        // The new object goes where the old one was.
                        format!("DROP 1 GET {} ROLL DROP {n} ROLLD", n + 1)
                    }
                };
                let vars = match &target {
                    Target::Variable(_) => directory(&tree, components(&dir)?)?,
                    Target::Level(_) => directory(&tree, &here)?,
                };
                let temp = free_text_name(vars);
                Transfer::with_mode(
                    &mut steps,
                    &flags,
                    true,
                    Step::Send {
                        name: temp.clone(),
                        data: file,
                    },
                );
                // Recalled and purged before it is compiled, so a failed
                // compile leaves nothing behind.
                steps.push_back(Step::Compile {
                    text: format!("{t} RCL {t} PURGE STR→ DUP SIZE 2 MIN", t = quoted(&temp)),
                });
                steps.push_back(Step::Host {
                    text: place,
                    cleanup: false,
                });
            }
            Op::SetFlag { .. } => {}
        }
        if let Some(text) = back {
            steps.push_back(Step::Host {
                text,
                cleanup: true,
            });
        }
        let mut t = Transfer::serve(m, &flags, steps, result)?;
        if alg {
            let depth = u.stack_addresses().map_err(|e| format!("{e:#}"))?.len();
            t.in_rpn(depth);
        }
        t.text_variable = text_variable;
        Ok(t)
    }

    /// Change to `dir` unless it is current; the command that changes
    /// back, if one is needed and the current path can be typed.
    fn enter(steps: &mut VecDeque<Step>, dir: &[String], here: &[String]) -> Option<String> {
        if dir == here {
            return None;
        }
        steps.push_back(Step::Host {
            text: cd_command(dir),
            cleanup: false,
        });
        here.iter()
            .all(|n| is_plain_name(n))
            .then(|| cd_command(here))
    }

    /// `step` with flag -35 set to `binary`, and put back afterwards.
    fn with_mode(steps: &mut VecDeque<Step>, flags: &Flags, binary: bool, step: Step) {
        let was = flags.get(-35) == Some(true);
        let set = |on: bool| if on { "-35 SF" } else { "-35 CF" };
        if was != binary {
            steps.push_back(Step::Host {
                text: set(binary).into(),
                cleanup: false,
            });
        }
        steps.push_back(step);
        if was != binary {
            steps.push_back(Step::Host {
                text: set(was).into(),
                cleanup: true,
            });
        }
    }

    /// `steps` inside a server session: `SERVER`, its first NAK, the
    /// baseline, the steps, `G F` and the wait for the stack.
    fn serve(
        m: &Machine,
        flags: &Flags,
        steps: VecDeque<Step>,
        result: TransferResult,
    ) -> crate::Result<Transfer> {
        if m.model() != Model::Hp49g && flags.get(-33) == Some(true) {
            return Err(
                "flag -33 is set (I/O over infrared): clear it for transfers over the wire"
                    .to_string()
                    .into(),
            );
        }
        let mut all = VecDeque::from([
            Step::Type("SERVER".into()),
            Step::WaitNak,
            Step::Host {
                text: String::new(),
                cleanup: false,
            },
        ]);
        all.extend(steps);
        all.push_back(Step::Finish);
        all.push_back(Step::Settle);
        Ok(Transfer::planned(m, all, result))
    }

    /// The 49G in algebraic mode, with `depth` stack levels: RPN for the
    /// server (`CF(-95)` typed before it), algebraic again after it (`-95
    /// SF` typed once it has ended, also after an error or a dead server,
    /// and on a stop); the echo of each typed line dropped.
    fn in_rpn(&mut self, depth: usize) {
        self.steps.push_front(Step::DropTo(depth));
        self.steps.push_front(Step::Type(TO_RPN.into()));
        self.steps.push_back(Step::Algebraic);
        self.steps.push_back(Step::DropTo(depth));
        self.algebraic = Some(depth);
    }

    /// A write with nothing planned, for the tests of what holds one.
    #[cfg(test)]
    pub(crate) fn empty(m: &Machine) -> Transfer {
        Self::planned(m, VecDeque::new(), TransferResult::default())
    }

    fn planned(m: &Machine, steps: VecDeque<Step>, result: TransferResult) -> Transfer {
        Transfer {
            steps,
            running: None,
            wire: Wire {
                rx: VecDeque::new(),
                start: Instant::now(),
                idle: Duration::ZERO,
            },
            server: false,
            entering: false,
            baseline: None,
            error: None,
            result,
            fetched: None,
            text_variable: false,
            started: m.cycles(),
            done: false,
            algebraic: None,
            restoring: false,
            restore_error: None,
            stopped: false,
        }
    }

    /// Run at most `budget` cycles; true once the write is done (its
    /// result is then ready, [`Transfer::finish`]).
    pub fn step(&mut self, m: &mut Machine, budget: u64) -> crate::Result<bool> {
        let stop = m.cycles().saturating_add(budget);
        while !self.done {
            if m.cycles() >= stop {
                return Ok(false);
            }
            let Some(running) = self.running.take() else {
                match self.steps.pop_front() {
                    Some(step) => self.begin(m, step)?,
                    None => self.done = true,
                }
                continue;
            };
            self.running = self.advance(m, running, stop)?;
        }
        Ok(true)
    }

    /// Run `running` until it ends or `stop`; what still runs.
    fn advance(
        &mut self,
        m: &mut Machine,
        running: Running,
        stop: u64,
    ) -> crate::Result<Option<Running>> {
        match running {
            Running::Typing(mut job) => match job.step(m, stop - m.cycles()) {
                Ok(false) => Ok(Some(Running::Typing(job))),
                Ok(true) => {
                    let o = job.outcome();
                    let refused = if let Some(e) = &o.error {
                        Some(format!("the calculator refused it: {e}"))
                    } else if o.closed == Some(false) {
                        Some("the command line stayed open".to_string())
                    } else {
                        None
                    };
                    if std::mem::take(&mut self.restoring) {
                        self.restored(m, refused);
                    } else if let Some(e) = refused {
                        self.fail(e);
                    }
                    Ok(None)
                }
                Err(e) => {
                    if std::mem::take(&mut self.restoring) {
                        self.restored(m, Some(e.to_string()));
                    } else {
                        self.fail(e.to_string());
                    }
                    Ok(None)
                }
            },
            Running::WaitNak { cap } => {
                self.wire.run(m, ms_cycles(m, NAK_STEP_MS))?;
                let rx = self.wire.rx.make_contiguous();
                if rx.windows(4).any(|w| w[0] == 0x01 && w[3] == b'N') {
                    self.wire.rx.clear();
                    self.server = true;
                    return Ok(None);
                }
                if m.cycles() < cap {
                    return Ok(Some(Running::WaitNak { cap }));
                }
                self.wire.rx.clear();
                // It may run all the same, on another port: ON.
                self.server = true;
                Ok(self.abort(
                    m,
                    format!(
                        "no Kermit server answered within {} s of emulated time after SERVER",
                        SERVER_CAP_MS / 1000
                    ),
                ))
            }
            Running::Exchange(mut x, step) => {
                let Some(outcome) = x.step(&mut self.wire, m, stop)? else {
                    return Ok(Some(Running::Exchange(x, step)));
                };
                match outcome {
                    Ok(t) => {
                        self.finished(step, t);
                        Ok(None)
                    }
                    // The server counts as ended either way.
                    Err(_) if matches!(step, Step::Finish) => {
                        self.server = false;
                        Ok(None)
                    }
                    // The server refused it and still runs: the cleanup,
                    // G F and the wait for the stack follow.
                    Err(Failure::Remote(e)) => {
                        self.fail(e);
                        Ok(None)
                    }
                    Err(Failure::Link(e)) => Ok(self.abort(m, e)),
                }
            }
            Running::EndServer {
                tries,
                phase,
                until,
            } => {
                if phase == EndPhase::Listen {
                    // A byte from the calculator: the server still runs.
                    let talked = !m.serial_drain().is_empty();
                    if talked && tries < ON_TRIES {
                        return Ok(Some(self.press_on(m, tries)));
                    }
                    if talked || m.cycles() >= until {
                        self.server = false;
                        self.wire.discard(m);
                        return Ok(None);
                    }
                    let n = ms_cycles(m, NAK_STEP_MS).min(until - m.cycles());
                    m.run_cycles(n.max(1))?;
                    return Ok(Some(Running::EndServer {
                        tries,
                        phase,
                        until,
                    }));
                }
                if m.cycles() < until {
                    m.run_cycles((until.min(stop) - m.cycles()).max(1))?;
                    return Ok(Some(Running::EndServer {
                        tries,
                        phase,
                        until,
                    }));
                }
                Ok(Some(if phase == EndPhase::Hold {
                    let _ = m.key_up(Key::On);
                    Running::EndServer {
                        tries,
                        phase: EndPhase::Settle,
                        until: m.cycles() + ms_cycles(m, FINISH_SETTLE_MS),
                    }
                } else {
                    m.serial_drain();
                    Running::EndServer {
                        tries,
                        phase: EndPhase::Listen,
                        until: m.cycles() + ms_cycles(m, SERVER_IDLE_NAK_MS),
                    }
                }))
            }
            Running::CancelLine { phase, until } => {
                if phase == CancelPhase::Idle && m.is_shutdown() || m.cycles() >= until {
                    return Ok(match phase {
                        CancelPhase::Idle if cmdline::command_line(m).is_ok_and(|l| l.active) => {
                            m.key_down(Key::On)?;
                            Some(Running::CancelLine {
                                phase: CancelPhase::Hold,
                                until: m.cycles() + ms_cycles(m, ON_HOLD_MS),
                            })
                        }
                        CancelPhase::Hold => {
                            m.key_up(Key::On)?;
                            Some(Running::CancelLine {
                                phase: CancelPhase::Gap,
                                until: m.cycles() + ms_cycles(m, DROP_GAP_MS * 5),
                            })
                        }
                        _ => None,
                    });
                }
                let n = if phase == CancelPhase::Idle {
                    ms_cycles(m, 1)
                } else {
                    until.min(stop).saturating_sub(m.cycles())
                };
                m.run_cycles(n.max(1))?;
                Ok(Some(Running::CancelLine { phase, until }))
            }
            Running::Drop { left, up, next } => {
                let now = m.cycles();
                if now < up {
                    m.run_cycles((up.min(stop) - now).max(1))?;
                    if m.cycles() >= up {
                        m.key_up(Key::Backspace)?;
                    }
                    return Ok(Some(Running::Drop { left, up, next }));
                }
                if now < next && !m.is_shutdown() {
                    m.run_cycles(ms_cycles(m, 1))?;
                    return Ok(Some(Running::Drop { left, up, next }));
                }
                m.run_cycles(ms_cycles(m, DROP_GAP_MS))?;
                if left > 1 {
                    return Ok(Some(self.press_drop(m, left - 1)?));
                }
                Ok(None)
            }
            Running::Settle(mut s) => {
                if m.cycles() < s.until {
                    m.run_cycles((s.until.min(stop) - m.cycles()).max(1))?;
                    return Ok(Some(Running::Settle(s)));
                }
                m.run_cycles(ms_cycles(m, 5))?;
                let lcd = m.lcd();
                let now = m.cycles();
                let mut settled = now >= s.cap;
                if s.last.as_ref() != Some(&lcd) {
                    s.last = Some(lcd);
                    s.since = now;
                } else if m.is_shutdown() && now - s.since >= ms_cycles(m, STABLE_MS) {
                    settled = true;
                }
                if !settled {
                    return Ok(Some(Running::Settle(s)));
                }
                // The server's bytes are no one's.
                self.wire.discard(m);
                Ok(None)
            }
        }
    }

    /// Press the backspace key, the first of `left` presses.
    fn press_drop(&mut self, m: &mut Machine, left: usize) -> crate::Result<Running> {
        m.key_down(Key::Backspace)?;
        let up = m.cycles() + ms_cycles(m, crate::typing::HOLD_MS);
        Ok(Running::Drop {
            left,
            up,
            next: up + ms_cycles(m, crate::typing::KEY_CAP_MS),
        })
    }

    /// Start `step`.
    fn begin(&mut self, m: &mut Machine, step: Step) -> crate::Result<()> {
        let failed = self.error.is_some() || self.result.error.is_some();
        if failed && !step.cleanup() {
            return Ok(());
        }
        let command = match &step {
            Step::Type(text) => {
                self.entering = text == "SERVER";
                match Job::new(m, Verb::Run, text) {
                    Ok(job) => self.running = Some(Running::Typing(Box::new(job))),
                    Err(e) => self.fail(e.to_string()),
                }
                return Ok(());
            }
            Step::WaitNak => {
                self.entering = false;
                let cap = m.cycles() + ms_cycles(m, SERVER_CAP_MS);
                self.running = Some(Running::WaitNak { cap });
                return Ok(());
            }
            Step::Settle => {
                self.running = Some(Running::Settle(SettleState::new(m, FINISH_SETTLE_MS)));
                return Ok(());
            }
            Step::Algebraic => {
                if rpn_now(m) {
                    match Job::new(m, Verb::Run, TO_ALGEBRAIC) {
                        Ok(job) => {
                            self.restoring = true;
                            self.running = Some(Running::Typing(Box::new(job)));
                        }
                        Err(e) => self.restored(m, Some(e.to_string())),
                    }
                }
                return Ok(());
            }
            Step::CancelLine => {
                self.running = Some(Running::CancelLine {
                    phase: CancelPhase::Idle,
                    until: m.cycles() + ms_cycles(m, 1_000),
                });
                return Ok(());
            }
            Step::EndServer => {
                self.running = Some(self.press_on(m, 0));
                return Ok(());
            }
            Step::DropTo(depth) => {
                let now = UserMemory::of(m)
                    .and_then(|u| u.stack_addresses())
                    .map_err(|e| format!("{e:#}"))?
                    .len();
                if now > *depth {
                    self.running = Some(self.press_drop(m, now - depth)?);
                }
                return Ok(());
            }
            Step::Host { text, .. } | Step::Compile { text } => {
                let bytes = charset::encode_command(text)
                    .map_err(|c| format!("{c:?} is not in the HP character set"))?;
                Command::Host(bytes)
            }
            Step::Send { name, data } => Command::Send(vec![OutgoingFile {
                name: charset::encode(name)
                    .map_err(|c| format!("{c:?} is not in the HP character set"))?,
                data: data.clone(),
            }]),
            Step::Get { name } => Command::Get(
                charset::encode(name)
                    .map_err(|c| format!("{c:?} is not in the HP character set"))?,
            ),
            Step::Finish => {
                if !self.server {
                    return Ok(());
                }
                Command::Finish
            }
        };
        let x = Exchange::new(&mut self.wire, m, command)?;
        self.running = Some(Running::Exchange(Box::new(x), step));
        Ok(())
    }

    /// A transaction of `step` completed.
    fn finished(&mut self, step: Step, t: Transcript) {
        match step {
            Step::Host { text, .. } => {
                let reply = tfile::parse_stack(&charset::decode(&t.text));
                let levels = reply.levels.len();
                let Some(error) = reply.error else {
                    self.baseline.get_or_insert(levels);
                    return;
                };
                if self.error.is_none() {
                    let what = if text.is_empty() {
                        "reading the stack"
                    } else {
                        &text
                    };
                    self.error = Some(format!("{what}: {error}"));
                }
                // What the failed command left behind goes first.
                if let Some(extra) = self.baseline.and_then(|b| levels.checked_sub(b))
                    && extra > 0
                {
                    self.steps.push_front(Step::Host {
                        text: format!("{extra} DROPN"),
                        cleanup: true,
                    });
                }
            }
            Step::Compile { .. } => {
                let reply = tfile::parse_stack(&charset::decode(&t.text));
                let levels = reply.levels.len();
                let extra = self.baseline.and_then(|b| levels.checked_sub(b));
                let refusal = match (&reply.error, extra) {
                    (Some(e), _) => Some(e.clone()),
                    (None, Some(2)) => match reply.level(1).and_then(reply_count) {
                        Some(1) => None,
                        Some(0) => Some("the text holds no object".to_string()),
                        Some(_) => Some("the text holds more than one object".to_string()),
                        None => Some("the calculator's reply could not be read".to_string()),
                    },
                    _ => Some("the calculator's reply could not be read".to_string()),
                };
                if let Some(why) = refusal {
                    self.result.error = Some(why);
                    // What the compile left goes first.
                    if let Some(extra) = extra
                        && extra > 0
                    {
                        self.steps.push_front(Step::Host {
                            text: format!("{extra} DROPN"),
                            cleanup: true,
                        });
                    }
                }
            }
            Step::Send { name, .. } if self.text_variable => {
                let stored = t.stored.into_iter().next();
                if stored.as_deref() != Some(name.as_str()) {
                    self.fail(format!(
                        "the calculator stored the text as {}, not {name}",
                        stored.unwrap_or_default()
                    ));
                }
            }
            Step::Send { .. } => self.result.name = t.stored.into_iter().next(),
            Step::Get { name } => match t.files.into_iter().next() {
                Some(file) => {
                    let file = trim_fetched(file);
                    self.result.size = Some(file.len());
                    self.fetched = Some(file);
                }
                None => self.fail(format!("GET {name}: no file came back")),
            },
            Step::Finish => self.server = false,
            _ => {}
        }
    }

    /// Record `error`; the cleanup steps still run.
    fn fail(&mut self, error: String) {
        self.error.get_or_insert(error);
    }

    /// The keys that set -95 again are done (`refused`: why they failed):
    /// -95 must read set now, or the 49G stays in RPN mode, which is said
    /// with whatever else the write reports.
    fn restored(&mut self, m: &Machine, refused: Option<String>) {
        let why = refused.or_else(|| rpn_now(m).then(|| "flag -95 is still clear".to_string()));
        if let Some(why) = why {
            self.restore_error
                .get_or_insert(format!("the 49G stays in RPN mode: {why}"));
        }
    }

    /// The machine failed: let go of every key, run nothing more.
    fn abandon(mut self, m: &mut Machine) {
        match self.running.take() {
            Some(Running::Typing(mut job)) => job.stop(m),
            Some(Running::Drop { .. }) => {
                let _ = m.key_up(Key::Backspace);
            }
            Some(Running::EndServer { .. } | Running::CancelLine { .. }) => {
                let _ = m.key_up(Key::On);
            }
            _ => {}
        }
        self.wire.discard(m);
    }

    /// Press ON to end a server, the `tries`-th time.
    fn press_on(&mut self, m: &mut Machine, tries: usize) -> Running {
        let _ = m.key_down(Key::On);
        Running::EndServer {
            tries: tries + 1,
            phase: EndPhase::Hold,
            until: m.cycles() + ms_cycles(m, ON_HOLD_MS),
        }
    }

    /// The server no longer answers as it should: record `error`, drop the
    /// plan but what follows the server, and end it with ON.
    fn abort(&mut self, m: &mut Machine, error: String) -> Option<Running> {
        self.error.get_or_insert(error);
        self.steps.retain(Step::after_server);
        if !self.server {
            return None;
        }
        Some(self.press_on(m, 0))
    }

    /// Stop where it is (the host gave up): every key is let go at once,
    /// and the rest becomes cleanup that runs in turns like any step
    /// ([`Transfer::step`] until done): a line typed in part cancelled, a
    /// running server ended with ON (again while it still NAKs), the 49G
    /// back in algebraic mode. The result is then the error [`STOPPED`],
    /// or why algebraic mode could not be set again.
    pub fn stop(&mut self, m: &mut Machine) {
        if self.stopped || self.done {
            self.done = true;
            return;
        }
        self.stopped = true;
        self.error.get_or_insert_with(|| STOPPED.to_string());
        let running = self.running.take();
        let waiting = matches!(running, Some(Running::WaitNak { .. }));
        let ending = matches!(running, Some(Running::EndServer { .. }));
        let typing = matches!(running, Some(Running::Typing(_)));
        match running {
            Some(Running::Typing(mut job)) => job.stop(m),
            Some(Running::Drop { .. }) => {
                let _ = m.key_up(Key::Backspace);
            }
            Some(Running::EndServer { .. } | Running::CancelLine { .. }) => {
                let _ = m.key_up(Key::On);
            }
            _ => {}
        }
        self.restoring = false;
        self.steps.clear();
        if typing {
            self.steps.push_back(Step::CancelLine);
        }
        if self.server || waiting || ending || std::mem::take(&mut self.entering) {
            self.steps.push_back(Step::EndServer);
        }
        if let Some(depth) = self.algebraic {
            self.steps.push_back(Step::Settle);
            self.steps.push_back(Step::Algebraic);
            self.steps.push_back(Step::DropTo(depth));
        }
    }

    /// The result of a write that is done: the reply and, for a fetch, the
    /// file.
    pub fn finish(self, m: &Machine) -> crate::Result<(TransferResult, Option<Vec<u8>>)> {
        // A failed return to algebraic mode is said with the rest.
        match (self.error, self.restore_error) {
            (Some(e), Some(r)) if e == STOPPED => return Err(r.into()),
            (Some(e), Some(r)) => return Err(format!("{e}; {r}").into()),
            (Some(e), None) | (None, Some(e)) => return Err(e.into()),
            (None, None) => {}
        }
        let mut result = self.result;
        result.emulated_ms =
            (m.cycles() - self.started) as f64 * 1000.0 / f64::from(m.model().clock_hz());
        Ok((result, self.fetched))
    }
}

impl crate::Emulator {
    /// Start the write `op`; every key is released first. Fails, running
    /// nothing, when the calculator cannot take it now.
    pub fn start_transfer(&mut self, op: Op) -> crate::Result<()> {
        if self.typing.is_some() || self.transfer.is_some() {
            return Err("a send or a transfer is already in progress"
                .to_string()
                .into());
        }
        self.release_keys();
        self.transfer = Some(Transfer::new(&self.machine, op)?);
        Ok(())
    }

    /// The address of the variable `target` in `dir`, or of a stack level.
    fn target_address(
        &self,
        u: &UserMemory<'_>,
        dir: &[String],
        target: &Target,
    ) -> crate::Result<u32> {
        Ok(match target {
            Target::Variable(name) => {
                let tree = u.tree().map_err(|e| format!("{e:#}"))?;
                let dir = components(dir)?;
                let var = directory(&tree, dir)?
                    .iter()
                    .find(|v| &v.name == name)
                    .ok_or_else(|| format!("no variable {name} in {{ {} }}", cd_command(dir)))?;
                if var.variables.is_some() {
                    return Err(format!("{name} is a directory, which has no text").into());
                }
                var.address
            }
            Target::Level(n) => {
                let levels = u.stack_addresses().map_err(|e| format!("{e:#}"))?;
                *n.checked_sub(1)
                    .and_then(|i| levels.get(i))
                    .ok_or_else(|| {
                        format!("there is no level {n} (the stack has {})", levels.len())
                    })?
            }
        })
    }

    /// The text to edit of the variable `target` in `dir` or of a stack
    /// level (`editText`), written for compiling again
    /// ([`saturnus_objects::decompile::edit_text`]), and the object's
    /// identity ([`crate::Emulator::edit_identity`]).
    pub fn edit_text(&self, dir: &[String], target: &Target) -> crate::Result<(String, String)> {
        let names = self.names();
        let u = UserMemory::of(&self.machine)
            .map_err(|e| format!("{e:#}"))?
            .with_names(&names);
        let address = self.target_address(&u, dir, target)?;
        let text = u.edit_text_at(address).map_err(|e| format!("{e:#}"))?;
        Ok((text, identity(&u, address)?))
    }

    /// The identity of the object `target` holds: its size in nibbles and
    /// its checksum (`"46:1A2B"`), the same in any display mode; what
    /// `storeText`'s `was` names.
    pub fn edit_identity(&self, dir: &[String], target: &Target) -> crate::Result<String> {
        let u = UserMemory::of(&self.machine).map_err(|e| format!("{e:#}"))?;
        let address = self.target_address(&u, dir, target)?;
        identity(&u, address)
    }

    /// An error unless the object `target` holds is still `was` (its
    /// [`crate::Emulator::edit_identity`] when the editor opened it): a
    /// save never replaces what the editor did not show.
    pub fn check_unchanged(&self, dir: &[String], target: &Target, was: &str) -> crate::Result<()> {
        if self.edit_identity(dir, target).ok().as_deref() == Some(was) {
            return Ok(());
        }
        Err(match target {
            Target::Variable(n) => {
                format!("{n} changed on the calculator since it was opened: open it again")
            }
            Target::Level(n) => {
                format!("level {n} changed on the calculator since it was opened: open it again")
            }
        }
        .into())
    }

    /// Whether a write is in progress.
    pub fn transferring(&self) -> bool {
        self.transfer.is_some()
    }

    /// Run the write for at most `ms` emulated ms; true once it is done
    /// (its result is then ready). An error of the machine ends it.
    pub fn transfer_step(&mut self, ms: f64) -> crate::Result<bool> {
        let per_ms = f64::from(self.machine.model().clock_hz()) / 1000.0;
        let Some(t) = self.transfer.as_mut() else {
            return Err("no transfer in progress".to_string().into());
        };
        let budget = (ms.max(1.0) * per_ms) as u64;
        match t.step(&mut self.machine, budget) {
            Ok(done) => Ok(done),
            Err(e) => {
                // The machine failed (a halt): nothing more can run on it.
                if let Some(t) = self.transfer.take() {
                    t.abandon(&mut self.machine);
                }
                Err(e)
            }
        }
    }

    /// Stop the write where it is: its keys let go at once, then its
    /// cleanup (a running server ended with ON, the 49G's algebraic mode
    /// set again) runs in turns through [`crate::Emulator::transfer_step`] like
    /// the rest of the write, and [`crate::Emulator::transfer_result`] says how it
    /// ended ([`STOPPED`], or why the cleanup failed).
    pub fn stop_transfer(&mut self) {
        if let Some(t) = self.transfer.as_mut() {
            t.stop(&mut self.machine);
        }
    }

    /// Give the write up where it is, without its cleanup (its cleanup
    /// itself ran out of time): every key let go.
    pub fn abandon_transfer(&mut self) {
        if let Some(t) = self.transfer.take() {
            t.abandon(&mut self.machine);
        }
    }

    /// Stop the write and run its cleanup to the end at once (at most
    /// `cap_ms` of emulated time; for tests and hosts with no turns to
    /// give): the result, as [`crate::Emulator::transfer_result`].
    pub fn stop_transfer_now(
        &mut self,
        cap_ms: f64,
    ) -> crate::Result<(TransferResult, Option<Vec<u8>>)> {
        self.stop_transfer();
        let mut left = cap_ms;
        while self.transfer.is_some() && left > 0.0 {
            if self.transfer_step(STOP_TURN_MS)? {
                break;
            }
            left -= STOP_TURN_MS;
        }
        self.transfer_result()
    }

    /// The result of a finished write, and the file of a fetch.
    pub fn transfer_result(&mut self) -> crate::Result<(TransferResult, Option<Vec<u8>>)> {
        let t = self
            .transfer
            .take()
            .ok_or_else(|| "no transfer in progress".to_string())?;
        t.finish(&self.machine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn transaction_openers() {
        assert!(opens_transaction(b"\x01& C1 2 +X\r"));
        assert!(opens_transaction(b"\x01#\x20S~"));
        assert!(!opens_transaction(b"\x01#!Y5\r"));
        assert!(!opens_transaction(b""));
    }

    #[test]
    fn directories_and_names() {
        assert_eq!(components(&s(&["HOME", "A", "B"])).unwrap(), s(&["A", "B"]));
        assert_eq!(components(&s(&["A"])).unwrap(), s(&["A"]));
        assert!(components(&s(&["HOME", "A B"])).is_err());
        assert_eq!(cd_command(&s(&["A", "B"])), "HOME A B");
        assert_eq!(cd_command(&[]), "HOME");
        assert!(check_name("X1").is_ok());
        assert!(check_name("1X").is_err());
        assert!(check_name("A'B").is_err());
        assert!(inside(&s(&["A", "B"]), &[], "A"));
        assert!(!inside(&s(&["A"]), &s(&["A"]), "B"));
        assert!(!inside(&[], &[], "A"));
    }

    #[test]
    fn files_to_store() {
        let bin = b"HPHP48-X\x33\x92\x02".to_vec();
        assert_eq!(store_bytes(&bin), (bin.clone(), true));
        // UTF-8 text becomes the calculator's set: « is 171 there.
        let (b, binary) = store_bytes("%%HP: T(3);\n« 1 »".as_bytes());
        assert!(!binary);
        assert!(b.ends_with(&[171, b' ', b'1', b' ', 187]));
        // Not UTF-8: sent as it is.
        assert_eq!(store_bytes(&[0xFF, 0x41]), (vec![0xFF, 0x41], false));
    }

    #[test]
    fn fetched_files_lose_the_padding() {
        // A real 1.: prolog 02933, exponent 000, mantissa 1000..., sign 0.
        let real = saturnus_objects::Object::Real {
            value: saturnus_objects::Real::parse("1").unwrap(),
        };
        let file = tfile::encode_file(&real, tfile::Family::Hp48)
            .unwrap()
            .unwrap();
        let mut padded = file.clone();
        padded.extend([0, 0, 0]);
        assert_eq!(trim_fetched(padded), file);
        // Not an HP binary file: kept whole.
        assert_eq!(trim_fetched(b"text\0\0".to_vec()), b"text\0\0");
    }

    #[test]
    fn models_without_a_server_are_refused() {
        let m = Machine::new(Model::Hp38g, &vec![0u8; Model::Hp38g.rom_bytes()]).unwrap();
        let e = Transfer::new(&m, Op::ChangeDir { dir: s(&["HOME"]) }).unwrap_err();
        assert!(e.to_string().contains("38G has no Kermit server"), "{e}");
    }

    /// A host command that failed leaves its arguments: what is above the
    /// first reply's level count is dropped before anything else runs.
    #[test]
    fn a_failed_command_drops_what_it_left() {
        let m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
        let mut t = Transfer::planned(
            &m,
            VecDeque::from([Step::Host {
                text: "-35 CF".into(),
                cleanup: true,
            }]),
            TransferResult::default(),
        );
        let host = |text: &str| Step::Host {
            text: text.into(),
            cleanup: false,
        };
        let reply = |text: &str| Transcript {
            text: text.as_bytes().to_vec(),
            ..Transcript::default()
        };
        t.finished(host(""), reply("1: 42\r\n"));
        assert_eq!(t.baseline, Some(1));
        t.finished(
            host("'A' RCL 'B' STO"),
            reply("Error: Bad Argument Type\r\n3: 42\r\n2: 7\r\n1: 'B'\r\n"),
        );
        assert_eq!(
            t.error.as_deref(),
            Some("'A' RCL 'B' STO: Bad Argument Type")
        );
        let texts: Vec<_> = t
            .steps
            .iter()
            .map(|s| match s {
                Step::Host { text, cleanup } => (text.as_str(), *cleanup),
                _ => ("?", false),
            })
            .collect();
        assert_eq!(texts, [("2 DROPN", true), ("-35 CF", true)]);
        // Only cleanup steps run after an error.
        assert!(!host("X").cleanup() && Step::Finish.cleanup());
    }

    /// A stop while ON is held to end a dead server lets go of ON at once;
    /// the server's end then runs in turns, each within its budget of
    /// emulated time, not in one call.
    #[test]
    fn a_stop_lets_go_of_on_and_ends_the_server_in_turns() {
        let mut m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
        let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
        t.server = true;
        t.running = t.abort(&mut m, "Kermit: timeout".into());
        assert!(matches!(t.running, Some(Running::EndServer { .. })));
        assert!(m.key_is_down(Key::On));
        let before = m.cycles();
        t.stop(&mut m);
        assert_eq!(m.cycles(), before, "a stop runs nothing itself");
        assert!(!m.key_is_down(Key::On), "ON released");
        assert!(!t.done);
        assert!(matches!(t.steps.front(), Some(Step::EndServer)));
        // Each turn keeps to its budget (one emulated millisecond's
        // worth of slack for the step in progress).
        let budget = ms_cycles(&m, 50);
        let mut turns = 0;
        loop {
            let at = m.cycles();
            let done = t.step(&mut m, budget).unwrap();
            assert!(
                m.cycles() - at <= budget + ms_cycles(&m, 10),
                "turn {turns}"
            );
            turns += 1;
            if done {
                break;
            }
            assert!(turns < 2_000, "the stop ends");
        }
        assert!(turns > 1, "in turns: {turns}");
        assert!(!m.key_is_down(Key::On));
        assert_eq!(t.error.as_deref(), Some("Kermit: timeout"));
    }

    /// Setting -95 again can fail: that is reported, also with another
    /// error and after a stop, never dropped.
    #[test]
    fn a_failed_return_to_algebraic_mode_is_reported() {
        let m = Machine::new(Model::Hp49g, &vec![0u8; Model::Hp49g.rom_bytes()]).unwrap();
        let failed = |error: Option<&str>, stopped: bool| {
            let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
            t.error = error.map(str::to_string);
            t.stopped = stopped;
            t.restored(&m, Some("the calculator refused it: Invalid Syntax".into()));
            t.finish(&m).unwrap_err().to_string()
        };
        let why = "the 49G stays in RPN mode: the calculator refused it: Invalid Syntax";
        assert_eq!(failed(None, false), why);
        assert_eq!(
            failed(Some("CRDIR: Bad Argument Type"), false),
            format!("CRDIR: Bad Argument Type; {why}")
        );
        assert_eq!(failed(Some(STOPPED), true), why);
        // A stop with nothing else to say: stopped.
        let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
        t.error = Some(STOPPED.into());
        t.stopped = true;
        assert_eq!(t.finish(&m).unwrap_err().to_string(), STOPPED);
    }

    /// The 49G in algebraic mode: -95 cleared by keys before `SERVER`, set
    /// again by keys after the server's end; that runs after a refusal (an
    /// E packet, a failed command) and after a dead server ended with ON.
    #[test]
    fn algebraic_mode_is_set_again_on_every_path() {
        let mut m = Machine::new(Model::Hp49g, &vec![0u8; Model::Hp49g.rom_bytes()]).unwrap();
        let plan = || {
            let flags = Flags {
                system: vec![0; 2],
                user: vec![0; 2],
            };
            let steps = VecDeque::from([Step::Host {
                text: "'D' CRDIR".into(),
                cleanup: false,
            }]);
            let mut t = Transfer::serve(&m, &flags, steps, TransferResult::default()).unwrap();
            t.in_rpn(2);
            t
        };
        let name = |s: &Step| match s {
            Step::Type(t) => format!("type {t}"),
            Step::WaitNak => "nak".into(),
            Step::Host { text, .. } => format!("host {text}"),
            Step::Finish => "finish".into(),
            Step::Settle => "settle".into(),
            Step::DropTo(n) => format!("drop to {n}"),
            Step::Algebraic => "algebraic".into(),
            Step::EndServer => "end server".into(),
            _ => "?".into(),
        };
        let t = plan();
        assert_eq!(t.algebraic, Some(2), "a stop sets -95 again too");
        let all: Vec<String> = t.steps.iter().map(name).collect();
        assert_eq!(
            all,
            [
                "type CF(-95)",
                "drop to 2",
                "type SERVER",
                "nak",
                "host ",
                "host 'D' CRDIR",
                "finish",
                "settle",
                "algebraic",
                "drop to 2",
            ]
        );
        // After an error (the calculator's E packet, a failed command, a
        // refused line) only the cleanup runs: the server's end, then -95.
        let cleanup: Vec<String> = t.steps.iter().filter(|s| s.cleanup()).map(name).collect();
        assert_eq!(
            cleanup,
            ["drop to 2", "finish", "settle", "algebraic", "drop to 2"]
        );
        // A dead server (a timeout): ON, then -95 all the same.
        let mut t = plan();
        for _ in 0..6 {
            t.steps.pop_front();
        }
        t.server = true;
        t.running = t.abort(&mut m, "Kermit: timeout".into());
        assert!(matches!(t.running, Some(Running::EndServer { .. })));
        let left: Vec<String> = t.steps.iter().map(name).collect();
        assert_eq!(left, ["algebraic", "drop to 2"]);
        // A stop then: ON let go, the server's end and -95 still to come.
        t.stop(&mut m);
        assert!(!m.key_is_down(Key::On));
        let left: Vec<String> = t.steps.iter().map(name).collect();
        assert_eq!(left, ["end server", "settle", "algebraic", "drop to 2"]);
    }

    /// `copy` and `move`: `RCL` then `STO`; a move checks the copy's size
    /// and checksum before it purges the original. Refused: the same
    /// directory, a directory into itself, a taken name unless replaced
    /// (and never a directory, nor by one), moving the directory that
    /// holds the current one.
    #[test]
    fn copies_and_moves_are_planned() {
        let texts = |steps: Vec<Step>| -> Vec<String> {
            steps
                .into_iter()
                .map(|s| match s {
                    Step::Host { text, cleanup } => {
                        assert!(!cleanup, "{text}");
                        text
                    }
                    _ => "?".into(),
                })
                .collect()
        };
        let case = CopyCase::default();
        let a = s(&["A"]);
        let b = s(&["B", "C"]);
        assert_eq!(
            texts(copy_plan(&a, "X", &b, &case).unwrap()),
            ["HOME A", "'X' RCL", "HOME B C", "'X' STO"]
        );
        let mv = CopyCase {
            remove: true,
            ..case
        };
        assert_eq!(
            texts(copy_plan(&a, "X", &[], &mv).unwrap()),
            [
                "HOME A",
                "'X' RCL",
                "HOME",
                "'X' STO",
                "'X' RCL BYTES",
                "HOME A",
                "'X' RCL BYTES",
                "ROT == 3 ROLLD == AND",
                "« 'X' PURGE » « \"Copy differs\" DOERR » IFTE",
            ]
        );
        let dir = CopyCase {
            is_dir: true,
            remove: true,
            ..case
        };
        assert!(texts(copy_plan(&[], "D", &a, &dir).unwrap())[8].contains("'D' PGDIR"));
        let err = |from: &[String], into: &[String], c: CopyCase| {
            copy_plan(from, "D", into, &c).unwrap_err().to_string()
        };
        assert_eq!(err(&a, &a, case), "D is in { HOME A } already");
        assert_eq!(
            err(&[], &s(&["D", "E"]), dir),
            "D cannot go into itself or a directory inside it"
        );
        assert!(
            copy_plan(&[], "D", &s(&["DD"]), &dir).is_ok(),
            "only D itself"
        );
        assert!(
            err(
                &[],
                &a,
                CopyCase {
                    holds_here: true,
                    ..dir
                }
            )
            .contains("holds the current directory")
        );
        // A copy of the directory that holds the current one is fine.
        assert!(
            copy_plan(
                &[],
                "D",
                &a,
                &CopyCase {
                    holds_here: true,
                    is_dir: true,
                    ..case
                }
            )
            .is_ok()
        );
        let taken = CopyCase {
            taken: Some(false),
            ..case
        };
        assert_eq!(err(&[], &a, taken), "D already exists in { HOME A }");
        assert!(
            copy_plan(
                &[],
                "D",
                &a,
                &CopyCase {
                    replace: true,
                    ..taken
                }
            )
            .is_ok()
        );
        assert!(
            err(
                &[],
                &a,
                CopyCase {
                    replace: true,
                    taken: Some(true),
                    ..case
                }
            )
            .contains("is a directory")
        );
        assert!(
            err(
                &[],
                &a,
                CopyCase {
                    replace: true,
                    taken: Some(false),
                    is_dir: true,
                    ..case
                }
            )
            .contains("a directory does not replace")
        );
    }

    #[test]
    fn texts_for_store_text() {
        assert_eq!(text_refusal("« { 1 } \"}\" @ } @ »"), None);
        assert_eq!(text_refusal("{ 1 } @ comment }\n"), None);
        assert_eq!(text_refusal("{\"a\"} \"a\"\"b\" (1,2) 'X' @c@ »"), None);
        assert_eq!(text_refusal("\"a@b\" 3 @ \" @ 2"), None);
        assert!(
            text_refusal("} 'P' PURGE {")
                .unwrap()
                .contains("closes a list")
        );
        // The calculator reads `@` and `"` inside a word as a new token.
        for t in ["X@ } 'P' PURGE {", "A\"B } 'P' PURGE {", "1@ 2", "A\"B\" C"] {
            assert!(
                text_refusal(t).unwrap().contains("right after a word"),
                "{t}"
            );
        }
        // An open string would swallow the wrapper's end.
        assert!(
            text_refusal("\"} 'P' PURGE {")
                .unwrap()
                .contains("string open")
        );
        assert!(text_refusal("« \"a »").unwrap().contains("string open"));
        assert_eq!(wrapped("« 1 »"), "{\n« 1 »\n}");
        assert_eq!(reply_count("1"), Some(1));
        assert_eq!(reply_count(" 1.000"), Some(1));
        assert_eq!(reply_count("0,"), Some(0));
        assert_eq!(reply_count("2."), Some(2));
        assert_eq!(reply_count("\"x\""), None);
        let v = |name: &str| Variable {
            name: name.into(),
            kind: "Real Number".into(),
            size: 10.5,
            checksum: 0,
            address: 0,
            variables: None,
        };
        assert_eq!(free_text_name(&[v("A")]), TEXT_VARIABLE);
        assert_eq!(
            free_text_name(&[v(TEXT_VARIABLE), v(&format!("{TEXT_VARIABLE}2"))]),
            format!("{TEXT_VARIABLE}3")
        );
    }

    /// The compile's reply: the calculator's error, or the count of the
    /// objects in the text; what it left on the stack is dropped first.
    #[test]
    fn a_compile_reports_the_calculators_refusal() {
        let m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
        let reply = |text: &str| Transcript {
            text: text.as_bytes().to_vec(),
            ..Transcript::default()
        };
        let compile = || Step::Compile {
            text: "'T' RCL 'T' PURGE STR→ DUP SIZE 2 MIN".into(),
        };
        let fresh = || {
            let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
            t.baseline = Some(1);
            t
        };
        let mut t = fresh();
        t.finished(compile(), reply("3: 42\r\n2: { « 1 » }\r\n1: 1.00\r\n"));
        assert_eq!(t.result.error, None);
        assert!(t.steps.is_empty());

        let mut t = fresh();
        t.finished(
            compile(),
            reply("Error: Invalid Syntax\r\n2: 42\r\n1: \"{ « }\"\r\n"),
        );
        assert_eq!(t.result.error.as_deref(), Some("Invalid Syntax"));
        assert_eq!(t.error, None, "the transfer itself went well");
        assert!(matches!(&t.steps[0], Step::Host { text, cleanup: true } if text == "1 DROPN"));
        // The place step is skipped, the cleanup is not.
        assert!(
            !Step::Host {
                text: "x".into(),
                cleanup: false
            }
            .cleanup()
        );

        let mut t = fresh();
        t.finished(compile(), reply("3: 42\r\n2: { 1 2 }\r\n1: 2\r\n"));
        assert_eq!(
            t.result.error.as_deref(),
            Some("the text holds more than one object")
        );
        assert!(matches!(&t.steps[0], Step::Host { text, .. } if text == "2 DROPN"));
        let mut t = fresh();
        t.finished(compile(), reply("3: 42\r\n2: { }\r\n1: 0\r\n"));
        assert_eq!(t.result.error.as_deref(), Some("the text holds no object"));
    }

    #[test]
    fn a_directory_must_exist() {
        let tree = vec![Variable {
            name: "D".into(),
            kind: "Directory".into(),
            size: 10.0,
            checksum: 0,
            address: 0,
            variables: Some(vec![]),
        }];
        assert!(directory(&tree, &s(&["D"])).is_ok());
        let e = directory(&tree, &s(&["D", "E"])).unwrap_err();
        assert_eq!(e.to_string(), "no directory { HOME D E }");
    }
}
