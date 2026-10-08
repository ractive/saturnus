//! The hidden Kermit transaction: what changes the calculator's user
//! memory from outside (`storeFile`, `fetchFile`, `purge`, `rename`,
//! `changeDir`, `setFlag`, `storeText` in `web/protocol.md`) goes through
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
//! Where the server cannot be used, keys are the fallback: the 49G in
//! algebraic mode (a server entered from it leaves the stack packed in a
//! list) sets and clears flags by typing `SF(n)` or `CF(n)`; its other
//! writes are refused. Wiki: protocols/kermit, protocols/server-commands.

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
    /// Make `dir` the current directory.
    ChangeDir { dir: Vec<String> },
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
}

impl Step {
    /// Whether the step runs after an error.
    fn cleanup(&self) -> bool {
        match self {
            Step::Host { cleanup, .. } => *cleanup,
            Step::Finish | Step::Settle => true,
            _ => false,
        }
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
    Hold {
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
}

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
/// or `None`: a `}` that closes more than the text opened would end the
/// list early and run what follows. Strings and `@` comments are skipped.
fn closes_too_much(text: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut string = false;
    let mut comment = false;
    for c in text.chars() {
        match c {
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
    }
    None
}

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

/// Whether the 49G is in algebraic mode (system flag -95).
fn algebraic(model: Model, flags: &Flags) -> bool {
    model == Model::Hp49g && flags.get(-95) == Some(true)
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
        if alg {
            return Err(
                "the 49G is in algebraic mode, where its Kermit server packs the stack \
                        into a list: switch it to RPN (clear flag -95) first"
                    .to_string()
                    .into(),
            );
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
            Op::StoreText { dir, target, text } => {
                if let Some(why) = closes_too_much(&text) {
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
            baseline: None,
            error: None,
            result,
            fetched: None,
            text_variable: false,
            started: m.cycles(),
            done: false,
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
                    if let Some(e) = &o.error {
                        self.fail(format!("the calculator refused it: {e}"));
                    } else if o.closed == Some(false) {
                        self.fail("the command line stayed open".into());
                    }
                    Ok(None)
                }
                Err(e) => {
                    self.fail(e.to_string());
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
            Running::Hold { until } => {
                if m.cycles() < until {
                    m.run_cycles((until.min(stop) - m.cycles()).max(1))?;
                    return Ok(Some(Running::Hold { until }));
                }
                let _ = m.key_up(Key::On);
                Ok(Some(Running::Settle(SettleState::new(m, FINISH_SETTLE_MS))))
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
                match Job::new(m, Verb::Run, text) {
                    Ok(job) => self.running = Some(Running::Typing(Box::new(job))),
                    Err(e) => self.fail(e.to_string()),
                }
                return Ok(());
            }
            Step::WaitNak => {
                let cap = m.cycles() + ms_cycles(m, SERVER_CAP_MS);
                self.running = Some(Running::WaitNak { cap });
                return Ok(());
            }
            Step::Settle => {
                self.running = Some(Running::Settle(SettleState::new(m, FINISH_SETTLE_MS)));
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

    /// The server no longer answers as it should: record `error`, drop the
    /// plan and press ON (it ends the server), then wait for the stack.
    fn abort(&mut self, m: &mut Machine, error: String) -> Option<Running> {
        self.error.get_or_insert(error);
        self.steps.clear();
        if !self.server {
            return None;
        }
        self.server = false;
        let _ = m.key_down(Key::On);
        let until = m.cycles() + ms_cycles(m, ON_HOLD_MS);
        Some(Running::Hold { until })
    }

    /// Stop where it is (the host gave up): a running server is ended with
    /// ON and the calculator given time to return to its stack.
    pub fn stop(&mut self, m: &mut Machine) {
        let running = self.running.take();
        let waiting = matches!(running, Some(Running::WaitNak { .. }));
        match running {
            Some(Running::Typing(mut job)) => job.stop(m),
            Some(Running::Drop { .. }) => {
                let _ = m.key_up(Key::Backspace);
            }
            // ON is down while a dead server is being ended.
            Some(Running::Hold { .. }) => {
                let _ = m.key_up(Key::On);
            }
            _ => {}
        }
        self.steps.clear();
        let server = self.server || waiting;
        if server {
            let _ = m.key_down(Key::On);
            let _ = m.run_cycles(ms_cycles(m, ON_HOLD_MS));
            let _ = m.key_up(Key::On);
            let _ = m.run_cycles(ms_cycles(m, FINISH_SETTLE_MS));
            self.server = false;
        }
        self.wire.discard(m);
        self.done = true;
    }

    /// The result of a write that is done: the reply and, for a fetch, the
    /// file.
    pub fn finish(self, m: &Machine) -> crate::Result<(TransferResult, Option<Vec<u8>>)> {
        if let Some(e) = self.error {
            return Err(e.into());
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

    /// The text to edit of the variable `target` in `dir` or of a stack
    /// level (`editText`): written for compiling again
    /// ([`saturnus_objects::decompile::edit_text`]).
    pub fn edit_text(&self, dir: &[String], target: &Target) -> crate::Result<String> {
        let names = self.names();
        let u = UserMemory::of(&self.machine)
            .map_err(|e| format!("{e:#}"))?
            .with_names(&names);
        let address = match target {
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
        };
        Ok(u.edit_text_at(address).map_err(|e| format!("{e:#}"))?)
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
                if let Some(mut t) = self.transfer.take() {
                    t.stop(&mut self.machine);
                }
                Err(e)
            }
        }
    }

    /// Stop the write where it is (a running server is ended with ON).
    pub fn stop_transfer(&mut self) {
        if let Some(mut t) = self.transfer.take() {
            t.stop(&mut self.machine);
        }
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

    /// A stop while ON is held to end a dead server lets go of ON.
    #[test]
    fn a_stop_during_the_hold_releases_on() {
        let mut m = Machine::new(Model::Hp48sx, &vec![0u8; Model::Hp48sx.rom_bytes()]).unwrap();
        let mut t = Transfer::planned(&m, VecDeque::new(), TransferResult::default());
        t.server = true;
        t.running = t.abort(&mut m, "Kermit: timeout".into());
        assert!(matches!(t.running, Some(Running::Hold { .. })));
        assert!(m.key_is_down(Key::On));
        t.stop(&mut m);
        assert!(!m.key_is_down(Key::On), "ON released");
        assert!(t.done);
        assert_eq!(t.error.as_deref(), Some("Kermit: timeout"));
    }

    #[test]
    fn texts_for_store_text() {
        assert_eq!(closes_too_much("« { 1 } \"}\" @ } @ »"), None);
        assert_eq!(closes_too_much("{ 1 } @ comment }\n"), None);
        assert!(closes_too_much("} 'P' PURGE {").is_some());
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
