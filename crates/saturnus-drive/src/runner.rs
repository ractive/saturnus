//! The machine thread of the native hosts: owns the emulator, paces it
//! against the wall clock and answers the front end's protocol
//! (`web/protocol.md`). Plain Rust: commands arrive on a channel, events
//! leave through a [`Sink`], so it runs and is tested without a window or
//! a socket. Two hosts use it: the Tauri app (its window) and `saturnus
//! run` (the HTTP control API, with the serial bridge as a [`Hook`]).
//!
//! The rules are the Web Worker's (`web/worker.js`): while the CPU
//! computes or keys are queued the [`Pacer`] hands out emulated time in
//! 1 ms slices of wall time (times the speed), dropping a lag longer than
//! 200 ms; while it sleeps in SHUTDN with nothing queued the thread blocks
//! until the next timer event or a command, then runs all the time that
//! passed (up to 12 hours), owing what does not fit its budget.
//!
//! Key scripts (`keyScript`) and typed text (`insert`, `run`, `replace`,
//! `typeText`; `runner/typing.rs`) run at once in emulated time on this
//! thread, as the CLI runs a key script, and reply when the calculator is
//! idle again; the clock then follows the wall clock from where they left
//! it.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use saturnus::cpu::Bus as _;
use saturnus::{Machine, Model};
use saturnus_host::Emulator;
use saturnus_host::host::{base64, model_for_rom_name, pack_bits};
use serde_json::{Value, json};

use crate::pacer::Pacer;

mod typing;
use crate::script;
use crate::session::{Limits, Session};

/// The protocol version this host speaks.
pub const PROTOCOL: u64 = 1;
/// The 20-bit nibble address space every model's CPU sees.
pub const ADDRESS_SPACE: u32 = 0x10_0000;
/// Most nibbles one `peek` or `poke` reads or writes.
pub const MAX_MEM_NIBBLES: usize = 64 * 1024;
/// Wall time a key script or typed text may take on the machine thread
/// (it runs in emulated time, much faster than real time while the
/// calculator waits for keys).
pub const SCRIPT_WALL_LIMIT: Duration = Duration::from_secs(30);
/// Emulated ms one pass at "Max" may run at most.
const MAX_EMULATED_PER_PASS_MS: f64 = 1000.0;
/// Wall time one pass at "Max" may spend emulating.
const MAX_BUDGET: Duration = Duration::from_millis(11);
/// Emulated time per wall time while sleeping at "Max".
const MAX_RATE: f64 = 60.0;
/// Most emulated time a wake catches up: 12 hours (see `web/worker.js`).
const MAX_BEHIND_MS: f64 = 12.0 * 3600.0 * 1000.0;
/// Wall time one wake may spend catching up.
const WAKE_BUDGET: Duration = Duration::from_millis(22);
/// Shortest time between two `frame` events.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// Wall time one paced pass may spend catching up with the clock.
const PASS_BUDGET: Duration = Duration::from_millis(4);
/// Shortest time between two `memoryChanged` events.
const MEMORY_EVENT_INTERVAL: Duration = Duration::from_millis(250);
/// Shortest time between two looks at the user memory.
const MEMORY_LOOK_INTERVAL: Duration = Duration::from_millis(100);
/// Sleep when the machine is ahead of the wall clock.
const AHEAD_SLEEP: Duration = Duration::from_micros(500);

/// Where events go: the Tauri window, the CLI's log, or a test's
/// collector.
pub trait Sink: Send + 'static {
    /// Deliver one event (a protocol message with a `type`).
    fn event(&self, msg: Value);
}

/// What a [`Hook`] tells the loop after a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    /// Nothing happened.
    Idle,
    /// Input reached the machine (serial bytes): a sleeping CPU must catch
    /// up its time and look at it.
    Input,
    /// Leave the loop ([`Runner::run`] returns).
    Stop,
}

/// Work a host does beside the machine on its thread, between passes: the
/// CLI's serial bridge and its Ctrl-C. Called on every turn of the loop,
/// and at least every [`Hook::interval`] while the machine sleeps or is
/// stopped.
pub trait Hook: Send {
    /// One turn, with the machine if there is one.
    fn service(&mut self, machine: Option<&mut Machine>) -> Service;
    /// Longest the loop may block between two turns.
    fn interval(&self) -> Duration;
}

/// A command for the machine thread, with the channel for its reply when
/// the page asked for one.
#[derive(Debug)]
pub struct Request {
    /// The protocol message, as the page sent it. It never names a file.
    pub msg: Value,
    /// The file of a `boot`, `saveState` or `loadState`, chosen by the
    /// host (a native dialog), never by the page.
    pub file: Option<PathBuf>,
    /// Where the reply goes.
    pub reply: Option<Sender<Result<Value, String>>>,
    /// Lets the sender withdraw the command (the HTTP host after a
    /// timeout or a disconnect); `None` for hosts that always wait.
    pub ticket: Option<Arc<Ticket>>,
}

/// The reply error of a command withdrawn before the machine took it.
pub const CANCELLED: &str = "cancelled before it ran";

/// A command's state, shared by its sender and the machine thread, so a
/// sender that gives up knows for certain whether the command ran: either
/// the machine takes it first ([`Ticket::start`]) or the sender withdraws
/// it first ([`Ticket::cancel`]), never both.
#[derive(Debug, Default)]
pub struct Ticket {
    state: AtomicU8,
    /// Stops a running key script or typed text at its next slice.
    abort: Arc<AtomicBool>,
}

const QUEUED: u8 = 0;
const STARTED: u8 = 1;
const WITHDRAWN: u8 = 2;

impl Ticket {
    /// A ticket for a command about to be queued.
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// The machine thread takes the command; `false` if it was withdrawn
    /// (it must then not run).
    pub fn start(&self) -> bool {
        self.state
            .compare_exchange(QUEUED, STARTED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// The sender gives up. `true`: the command did not run and never
    /// will. `false`: it has started; a key script or typed text is then
    /// stopped at its next slice (within about 50 emulated ms), anything
    /// else finishes, and its reply still comes.
    pub fn cancel(&self) -> bool {
        let withdrawn = self
            .state
            .compare_exchange(QUEUED, WITHDRAWN, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
        if !withdrawn {
            self.abort.store(true, Ordering::SeqCst);
        }
        withdrawn
    }
}

/// Largest ROM file read: the largest image any model accepts (an unpacked
/// 49G, 4 MiB); anything longer is refused before it is read whole.
pub fn max_rom_file() -> u64 {
    Model::ALL
        .iter()
        .map(|m| 2 * m.rom_bytes() as u64)
        .max()
        .unwrap_or(0)
}

/// Largest state file read: the largest state is the 49G's, 2.6 MB (its
/// 2 MiB flash and 512 KiB RAM); 4 MiB leaves half again as much.
pub const MAX_STATE_FILE: u64 = 4 * 1024 * 1024;

/// Replace `path` with `bytes` so that it holds either the old content or
/// the new, never a part: write a temporary file beside it with `write`
/// (the seam tests use to fail a write), sync it, then rename it over
/// `path` (`rename` replaces an existing file on Windows too). On failure
/// the temporary file is removed and `path` is untouched.
pub fn write_atomic(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().map_or_else(
        || std::ffi::OsString::from("state"),
        std::ffi::OsStr::to_os_string,
    );
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(&name);
    tmp_name.push(format!(".{}.tmp", std::process::id()));
    let tmp = dir.join(tmp_name);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        write(&mut f, bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Fields that name files: the page may not send them to this host.
const PATH_FIELDS: [&str; 2] = ["romPath", "path"];

/// Read `path` if it holds at most `cap` bytes; a longer file (or a
/// device that never ends) is refused after reading `cap + 1` bytes.
pub fn read_capped(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let f = std::fs::File::open(path).map_err(|e| format!("cannot read {name}: {e}"))?;
    if f.metadata().is_ok_and(|m| m.is_file() && m.len() > cap) {
        return Err(format!("{name} is larger than {cap} bytes"));
    }
    let mut data = Vec::new();
    f.take(cap + 1)
        .read_to_end(&mut data)
        .map_err(|e| format!("cannot read {name}: {e}"))?;
    if data.len() as u64 > cap {
        return Err(format!("{name} is larger than {cap} bytes"));
    }
    Ok(data)
}

/// The speed setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    /// This many times real time.
    Times(u32),
    /// As fast as the host can while staying responsive.
    Max,
}

impl Speed {
    fn parse(s: &str) -> Self {
        match s {
            "2" => Self::Times(2),
            "4" => Self::Times(4),
            "max" => Self::Max,
            _ => Self::Times(1),
        }
    }

    fn name(self) -> String {
        match self {
            Self::Times(n) => n.to_string(),
            Self::Max => "max".to_string(),
        }
    }

    /// Emulated ms per wall ms while asleep.
    fn rate(self) -> f64 {
        match self {
            Self::Times(n) => f64::from(n),
            Self::Max => MAX_RATE,
        }
    }
}

/// What the thread does between commands.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    /// Nothing to run (no machine, paused or halted).
    Stopped,
    /// The CPU computes or keys are queued: run paced passes.
    Busy,
    /// The CPU sleeps; wake at this instant for its next timer event.
    Sleep(Instant),
}

/// The machine thread's state.
pub struct Runner<S: Sink> {
    sink: S,
    emu: Option<Emulator>,
    model: Option<Model>,
    rom_name: String,
    running: bool,
    halted: Option<String>,
    speed: Speed,
    mode: Mode,
    pacer: Pacer,
    /// Wall clock up to which emulated time is accounted for while asleep.
    slept_at: Instant,
    /// Emulated ms owed to the wall clock (a wake's unpaid rest).
    behind_ms: f64,
    last_frame: Option<Instant>,
    last_status: Option<Value>,
    started: Instant,
    work: Duration,
    ticks: u64,
    wakes: u64,
    /// Lags the pacer dropped, before the current pacer.
    rebases: u64,
    /// The host's name in `hello` and `info` ("tauri", "http").
    host: &'static str,
    /// What the host runs beside the machine.
    hook: Option<Box<dyn Hook>>,
    /// Fields the host adds to `info` (its endpoints, the ROM's hash).
    info_extra: serde_json::Map<String, Value>,
    /// The abort flag of the command being handled, if its sender can
    /// withdraw it.
    abort: Option<Arc<AtomicBool>>,
    /// The user memory as the page was last told (`memoryChanged`).
    memory: MemoryWatch,
    /// A long send is typing: the frames are held (`status` `busy`).
    busy: bool,
}

/// The state behind `watchMemory` and the `memoryChanged` event.
#[derive(Debug, Default)]
struct MemoryWatch {
    /// A page asked for `memoryChanged` events.
    on: bool,
    /// The change counter (or the read error) the page knows.
    last: Option<String>,
    /// When it was last read.
    at: Option<Instant>,
    /// When the page was last told.
    told: Option<Instant>,
    /// The machine's cycle count then: no cycles, no change.
    cycles: u64,
    /// Memory changed without cycles (a new machine, a state, a poke).
    force: bool,
    /// Looks so far and the wall time they took, for `stats`.
    looks: u64,
    spent: Duration,
}

impl<S: Sink> std::fmt::Debug for Runner<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner")
            .field("host", &self.host)
            .field("model", &self.model)
            .field("running", &self.running)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

fn str_field<'a>(msg: &'a Value, name: &str) -> Result<&'a str, String> {
    msg.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string field {name:?}"))
}

fn u64_field(msg: &Value, name: &str) -> Result<u64, String> {
    msg.get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing number field {name:?}"))
}

fn json_of(s: &str) -> Result<Value, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}

/// Decode standard base64 (padding optional, no whitespace): the JSON form
/// of the protocol's *bytes* fields.
pub fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn value(c: u8) -> Option<u32> {
        Some(u32::from(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        }))
    }
    let s = s.trim_end_matches('=').as_bytes();
    if s.len() % 4 == 1 {
        return Err("bad base64 length".to_string());
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= value(c).ok_or("bad base64 character")? << (18 - 6 * i);
        }
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Ok(out)
}

/// The error for a key name the running model does not have.
fn unknown_key(e: &Emulator, key: &str) -> String {
    format!("no key {key:?} on the {}", e.machine().model().name())
}

impl<S: Sink> Runner<S> {
    /// A runner with no machine yet, for the Tauri app.
    pub fn new(sink: S) -> Self {
        Self::for_host(sink, "tauri")
    }

    /// A runner with no machine yet for the host called `host`.
    pub fn for_host(sink: S, host: &'static str) -> Self {
        let now = Instant::now();
        Self {
            host,
            hook: None,
            info_extra: serde_json::Map::new(),
            abort: None,
            busy: false,
            sink,
            emu: None,
            model: None,
            rom_name: String::new(),
            running: false,
            halted: None,
            speed: Speed::Times(1),
            mode: Mode::Stopped,
            pacer: Pacer::new(1, now, 0),
            slept_at: now,
            behind_ms: 0.0,
            last_frame: None,
            last_status: None,
            started: now,
            work: Duration::ZERO,
            ticks: 0,
            wakes: 0,
            rebases: 0,
            memory: MemoryWatch::default(),
        }
    }

    /// Run `hook` beside the machine (see [`Hook`]).
    pub fn set_hook(&mut self, hook: Box<dyn Hook>) {
        self.hook = Some(hook);
    }

    /// Fields `info` adds to its own (an object's fields; anything else
    /// is ignored).
    pub fn set_info(&mut self, extra: Value) {
        if let Value::Object(m) = extra {
            self.info_extra = m;
        }
    }

    /// Start running `emu`, a machine the host built from the ROM it was
    /// given (`rom_name` is the file's name, for `status` and `info`).
    pub fn start(&mut self, emu: Emulator, rom_name: &str) {
        self.model = Some(emu.machine().model());
        self.emu = Some(emu);
        self.rom_name = rom_name.to_string();
        self.halted = None;
        self.memory.force = true;
        self.set_running(true);
    }

    /// The emulator, once [`Runner::run`] has returned.
    pub fn into_emulator(self) -> Option<Emulator> {
        self.emu
    }

    /// One turn of the hook; `true` means stop.
    fn service(&mut self) -> bool {
        let Some(hook) = self.hook.as_mut() else {
            return false;
        };
        match hook.service(self.emu.as_mut().map(Emulator::machine_mut)) {
            Service::Idle => false,
            Service::Stop => true,
            Service::Input => {
                // The bytes are timed from now: a sleeping CPU first runs
                // the time that passed, then sees when the UART wakes it.
                if matches!(self.mode, Mode::Sleep(_)) {
                    self.wake();
                } else if self.mode == Mode::Busy {
                    self.schedule();
                }
                false
            }
        }
    }

    /// Serve `rx` until every sender is gone or the hook says stop; returns
    /// the runner, which still holds the machine.
    pub fn run(mut self, rx: &Receiver<Request>) -> Self {
        loop {
            // Before blocking, send what the frame throttle held back: a
            // display that changed within 16 ms of the last frame would
            // otherwise stay unsent for the whole sleep.
            let mut memory_due = None;
            if self.mode != Mode::Busy {
                self.flush(true);
                memory_due = self.poll_memory();
            }
            if self.service() {
                return self;
            }
            // The next look at the memory and the hook's turn bound the wait.
            let cap = [
                self.hook.as_ref().map(|h| h.interval()),
                memory_due.map(|t| t.saturating_duration_since(Instant::now())),
            ]
            .into_iter()
            .flatten()
            .min();
            let wait = |d: Option<Duration>| match (d, cap) {
                (Some(d), Some(c)) => Some(d.min(c)),
                (d, c) => d.or(c),
            };
            let req = match self.mode {
                Mode::Busy => match rx.try_recv() {
                    Ok(r) => Some(r),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return self,
                },
                mode => {
                    let until = match mode {
                        Mode::Sleep(until) => Some(until.saturating_duration_since(Instant::now())),
                        _ => None,
                    };
                    match wait(until) {
                        None => match rx.recv() {
                            Ok(r) => Some(r),
                            Err(_) => return self,
                        },
                        Some(d) => match rx.recv_timeout(d) {
                            Ok(r) => Some(r),
                            Err(RecvTimeoutError::Timeout) => None,
                            Err(RecvTimeoutError::Disconnected) => return self,
                        },
                    }
                }
            };
            if let Some(req) = req {
                self.serve(req);
                continue;
            }
            match self.mode {
                Mode::Busy => self.pass(),
                // A hook's turn may end the wait before the timer event.
                Mode::Sleep(until) if Instant::now() >= until => self.wake(),
                Mode::Sleep(_) | Mode::Stopped => {}
            }
        }
    }

    /// Answer one request.
    /// The events the command caused go out before its reply
    /// (`web/protocol.md`): a caller that has the reply has the state.
    /// A withdrawn command (see [`Ticket`]) is answered [`CANCELLED`]
    /// without running.
    pub fn serve(&mut self, req: Request) {
        if let Some(t) = &req.ticket
            && !t.start()
        {
            if let Some(tx) = req.reply {
                let _ = tx.send(Err(CANCELLED.to_string()));
            }
            return;
        }
        self.abort = req.ticket.as_ref().map(|t| Arc::clone(&t.abort));
        let result = self.handle(&req.msg, req.file.as_deref());
        self.abort = None;
        self.flush(true);
        match req.reply {
            Some(tx) => {
                let _ = tx.send(result);
            }
            None => {
                if let Err(message) = result {
                    self.sink
                        .event(json!({"type": "error", "message": message}));
                }
            }
        }
    }

    fn emu(&mut self) -> Result<&mut Emulator, String> {
        self.emu.as_mut().ok_or_else(|| "no ROM loaded".to_string())
    }

    /// One protocol command; `file` is the host's choice for the commands
    /// that need one.
    pub fn handle(&mut self, msg: &Value, file: Option<&Path>) -> Result<Value, String> {
        if let Some(f) = PATH_FIELDS.iter().find(|f| msg.get(**f).is_some()) {
            return Err(format!(
                "{f:?} is not accepted: a message never names a file, the host chooses them"
            ));
        }
        let path = file;
        let file = |what: &str| file.ok_or_else(|| format!("{what} needs a file"));
        let v = msg.get("v").and_then(Value::as_u64);
        if v != Some(PROTOCOL) {
            return Err(format!(
                "protocol version {v:?} not supported (this host speaks {PROTOCOL})"
            ));
        }
        let cmd = str_field(msg, "cmd")?;
        match cmd {
            "hello" => {
                // A (re)loaded page starts from nothing: the status, the
                // keys down and the current frame go out again, before this
                // reply (`serve` flushes first). The page fetches the
                // model's skin and layout itself when it sees the model.
                self.last_status = None;
                self.memory.on = false;
                if let Some(e) = self.emu.as_mut() {
                    e.reshow();
                }
                let models: Vec<&str> = Model::ALL.iter().map(|m| m.name()).collect();
                Ok(json!({"protocol": PROTOCOL, "host": self.host, "models": models}))
            }
            "skin" => {
                let m = saturnus_host::model_from_name(str_field(msg, "model")?)?;
                json_of(&saturnus_host::skins::skin_json(m))
            }
            "layout" => json_of(&saturnus_host::host::layout_of(str_field(msg, "model")?)?),
            "boot" => {
                let path = file("boot")?.to_path_buf();
                self.boot(str_field(msg, "model")?, &path)
            }
            "keyDown" => {
                let key = str_field(msg, "key")?.to_string();
                let e = self.emu()?;
                if !e.queue().press(&key) {
                    return Err(unknown_key(e, &key));
                }
                self.after_keys();
                Ok(Value::Null)
            }
            "keyUp" => {
                let key = str_field(msg, "key")?.to_string();
                let e = self.emu()?;
                if !e.queue().has_key(&key) {
                    return Err(unknown_key(e, &key));
                }
                e.queue().release(&key);
                e.pump();
                Ok(Value::Null)
            }
            "keyUpAll" => {
                if let Some(e) = self.emu.as_mut() {
                    e.queue().release_held();
                    e.pump();
                }
                Ok(Value::Null)
            }
            "typeLetter" => {
                let ch = str_field(msg, "letter")?.chars().next();
                let Some(e) = self.emu.as_mut() else {
                    return Ok(Value::Bool(false));
                };
                let ok = ch.is_some_and(|c| e.queue().type_letter(c));
                self.after_keys();
                Ok(Value::Bool(ok))
            }
            "typeKeys" => {
                let keys: Vec<String> = msg
                    .get("keys")
                    .and_then(Value::as_array)
                    .ok_or("missing array field \"keys\"")?
                    .iter()
                    .map(|k| k.as_str().map(String::from))
                    .collect::<Option<_>>()
                    .ok_or("\"keys\" must hold key names (strings)")?;
                let e = self.emu()?;
                // All or nothing: one unknown name refuses the sequence.
                if let Some(k) = keys.iter().find(|k| !e.queue().has_key(k)) {
                    return Err(unknown_key(e, k));
                }
                let names: Vec<&str> = keys.iter().map(String::as_str).collect();
                e.queue().type_keys(&names);
                self.after_keys();
                Ok(Value::Null)
            }
            "releaseAll" => {
                if let Some(e) = self.emu.as_mut() {
                    e.release_keys();
                }
                Ok(Value::Null)
            }
            "setSpeed" => {
                self.set_speed(Speed::parse(str_field(msg, "speed")?));
                Ok(Value::Null)
            }
            "pause" => {
                let paused = msg.get("paused").and_then(Value::as_bool).unwrap_or(true);
                self.set_running(!paused);
                Ok(Value::Null)
            }
            "reset" => {
                let e = self.emu()?;
                e.release_keys();
                e.reset();
                self.halted = None;
                self.set_running(true);
                Ok(Value::Null)
            }
            "saveState" => {
                let state = self.emu()?.save_state();
                // Without a host-chosen file the state travels as bytes
                // (base64 in JSON), as in the Worker.
                let Some(path) = path else {
                    let cycles = self.emu()?.machine().cycles();
                    return Ok(json!({"state": base64(&state), "cycles": cycles}));
                };
                write_atomic(path, &state, |f, b| f.write_all(b))
                    .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                Ok(json!({"path": path.display().to_string()}))
            }
            "loadState" => {
                let data = match path {
                    Some(p) => read_capped(p, MAX_STATE_FILE)?,
                    None => {
                        let b64 = str_field(msg, "state")?;
                        if b64.len() as u64 > MAX_STATE_FILE.div_ceil(3) * 4 {
                            return Err(format!("state is larger than {MAX_STATE_FILE} bytes"));
                        }
                        base64_decode(b64).map_err(|e| format!("state: {e}"))?
                    }
                };
                let e = self.emu()?;
                e.release_keys();
                e.load_state(&data)?;
                e.reshow();
                self.memory.force = true;
                self.halted = None;
                self.set_running(self.running);
                Ok(match path {
                    Some(p) => json!({"path": p.display().to_string()}),
                    None => json!({}),
                })
            }
            // The window's visibility does not pause a native machine.
            "visibility" => Ok(Value::Null),
            "stats" => Ok(self.stats()),
            "screen" => self.screen(msg),
            "info" => Ok(self.info()),
            "model" => self.model_info(),
            "peek" => self.peek(msg),
            "poke" => self.poke(msg),
            "keyScript" => {
                let lines =
                    script::parse_keys(str_field(msg, "script")?).map_err(|e| format!("{e:#}"))?;
                self.with_session(|s| {
                    s.check_keys(&lines)?;
                    for line in &lines {
                        s.apply(line)?;
                    }
                    Ok(())
                })
            }
            "watchMemory" => {
                self.memory.on = msg.get("on").and_then(Value::as_bool).unwrap_or(false);
                // The page reads after this reply: events are for what
                // changes from here on.
                self.memory.last = self.memory_state();
                self.memory.at = None;
                self.memory.told = None;
                self.memory.force = false;
                let Some(e) = &self.emu else {
                    return Ok(json!({"supported": null, "reason": null}));
                };
                self.memory.cycles = e.machine().cycles();
                let reason = e.memory_refusal();
                Ok(json!({"supported": reason.is_none(), "reason": reason}))
            }
            // Typing (protocol.md, "Typing"; `runner/typing.rs`).
            "typeText" | "insert" => self.send_text("insert", msg),
            "run" => self.send_text("run", msg),
            "replace" => self.send_text("replace", msg),
            "commandLine" => self.command_line(),
            "memoryTree" => {
                serde_json::to_value(self.emu()?.memory_tree()?).map_err(|e| e.to_string())
            }
            "stack" => Ok(Value::Array(self.emu()?.stack()?)),
            "flags" => serde_json::to_value(self.emu()?.flags()?).map_err(|e| e.to_string()),
            "objectAt" => {
                let address = u32::try_from(u64_field(msg, "address")?)
                    .ok()
                    .filter(|&a| a < ADDRESS_SPACE)
                    .ok_or("address is outside the address space")?;
                Ok(self.emu()?.object_at(address)?)
            }
            other => Err(format!("unknown command {other:?}")),
        }
    }

    /// The change counter of the user memory, or the reason it cannot be
    /// read (no HOME yet, a model without one); `None` without a machine.
    fn memory_state(&self) -> Option<String> {
        let e = self.emu.as_ref()?;
        Some(
            e.memory_changes()
                .map(|c| format!("{c:016X}"))
                .unwrap_or_else(|err| format!("error: {err}")),
        )
    }

    /// Tell a watching page that the user memory changed, if it did: looked
    /// at only while the machine is not computing (the ROM's structures are
    /// whole when it waits for a key), only after it ran, at most every
    /// [`MEMORY_LOOK_INTERVAL`] and not within [`MEMORY_EVENT_INTERVAL`]
    /// of the last event. Returns when to look again, if a look is owed.
    fn poll_memory(&mut self) -> Option<Instant> {
        if !self.memory.on {
            return None;
        }
        let cycles = self.emu.as_ref()?.machine().cycles();
        if cycles == self.memory.cycles && !self.memory.force {
            return None;
        }
        let now = Instant::now();
        let due = [
            self.memory.at.map(|t| t + MEMORY_LOOK_INTERVAL),
            self.memory.told.map(|t| t + MEMORY_EVENT_INTERVAL),
        ]
        .into_iter()
        .flatten()
        .max();
        if let Some(due) = due
            && due > now
        {
            return Some(due);
        }
        self.memory.at = Some(now);
        self.memory.cycles = cycles;
        self.memory.force = false;
        let state = self.memory_state();
        self.memory.looks += 1;
        self.memory.spent += now.elapsed();
        if state != self.memory.last {
            self.memory.last = state;
            self.memory.told = Some(now);
            self.sink.event(json!({"type": "memoryChanged"}));
        }
        None
    }

    /// Run `f` on the machine in a [`Session`] (emulated time, as fast as
    /// the host can, at most [`SCRIPT_WALL_LIMIT`] of wall time) with every
    /// key released first; then follow the wall clock again from where it
    /// left the machine. Replies `{emulatedMs, warnings}`.
    fn with_session(
        &mut self,
        f: impl FnOnce(&mut Session) -> anyhow::Result<()>,
    ) -> Result<Value, String> {
        if let Some(h) = &self.halted {
            return Err(format!("the CPU is halted: {h}"));
        }
        // A sleeping machine first catches up the time that passed.
        if matches!(self.mode, Mode::Sleep(_)) {
            self.wake();
        }
        let mut emu = self.emu.take().ok_or("no ROM loaded")?;
        emu.release_keys();
        let mut s = Session::new(emu.into_machine(), 0, false);
        s.set_echo_warnings(false);
        let started = Instant::now();
        s.set_limits(Limits {
            abort: self.abort.clone(),
            deadline: Some(started + SCRIPT_WALL_LIMIT),
        });
        let from = s.machine.cycles();
        let result = f(&mut s);
        self.work += started.elapsed();
        if result.is_err() {
            s.machine.hw.keyboard.release_all();
        }
        let warnings = s.take_warnings();
        let ms =
            (s.machine.cycles() - from) as f64 * 1000.0 / f64::from(s.machine.model().clock_hz());
        self.emu = Some(Emulator::from_machine(s.machine));
        match result {
            Ok(()) => {
                self.set_running(self.running);
                Ok(json!({"emulatedMs": ms, "warnings": warnings}))
            }
            Err(e) => {
                let message = format!("{e:#}");
                if message.contains("CPU halted") {
                    self.halt(message.clone());
                } else {
                    self.set_running(self.running);
                }
                Err(message)
            }
        }
    }

    /// The display now: the `frame` event's fields plus `rows` (text, `#`
    /// dark, `.` light, as the CLI's `.txt` screens) and `displayOn`; with
    /// `png: true` a 1-bit PNG instead (`scale` 1-8), as base64.
    fn screen(&mut self, msg: &Value) -> Result<Value, String> {
        let m = self.emu()?.machine();
        let fb = m.framebuffer();
        let (width, height) = (saturnus::machine::LCD_WIDTH, fb.pixels.height());
        if msg.get("png").and_then(Value::as_bool) == Some(true) {
            let scale = msg.get("scale").and_then(Value::as_u64).unwrap_or(1);
            let scale = u32::try_from(scale).map_err(|_| "scale is out of range")?;
            let png = crate::screen::png_bytes(&fb.pixels, scale).map_err(|e| format!("{e:#}"))?;
            let s = scale as usize;
            return Ok(json!({"png": base64(&png), "width": width * s, "height": height * s}));
        }
        let range = m.model().contrast_range();
        Ok(json!({
            "width": width,
            "height": height,
            "rows": fb.pixels.to_text().lines().collect::<Vec<_>>(),
            "pixels": base64(&pack_bits(&fb.pixels.pixels)),
            "annunciators": json_of(&saturnus_host::annunciators_json(&fb.annunciators))?,
            "contrast": fb.contrast,
            "contrastRange": [range.start(), range.end()],
            "displayOn": m.display_on(),
        }))
    }

    /// The host, the machine and how it runs; the host's own fields
    /// ([`Runner::set_info`]) added.
    fn info(&self) -> Value {
        let mut v = json!({
            "protocol": PROTOCOL,
            "host": self.host,
            "model": self.model.map(|m| m.name()),
            "romName": self.rom_name,
            "running": self.running,
            "halted": self.halted,
            "speed": self.speed.name(),
            "loop": self.loop_name(),
            "displayOn": self.emu.as_ref().map(|e| e.machine().display_on()),
            "cycles": self.emu.as_ref().map(|e| e.machine().cycles()),
        });
        if let Value::Object(m) = &mut v {
            for (k, x) in &self.info_extra {
                m.insert(k.clone(), x.clone());
            }
        }
        v
    }

    /// The running model: name, clock, display size, serial port and its
    /// keys (the `layout` result).
    fn model_info(&mut self) -> Result<Value, String> {
        let m = self.emu()?.machine();
        let model = m.model();
        let height = m.lcd().height();
        Ok(json!({
            "model": model.name(),
            "clockHz": model.clock_hz(),
            "width": saturnus::machine::LCD_WIDTH,
            "height": height,
            "hasSerial": model.has_serial(),
            "layout": json_of(&saturnus_host::host::layout_of(model.name())?)?,
        }))
    }

    /// The `address` field and a length, checked against the address space
    /// and [`MAX_MEM_NIBBLES`].
    fn mem_range(msg: &Value, length: usize) -> Result<u32, String> {
        let address = u64_field(msg, "address")?;
        if address >= u64::from(ADDRESS_SPACE) {
            return Err(format!(
                "address #{address:X} is outside the address space (#00000-#FFFFF)"
            ));
        }
        if length == 0 || length > MAX_MEM_NIBBLES {
            return Err(format!(
                "length {length} is out of range: 1 to {MAX_MEM_NIBBLES} nibbles"
            ));
        }
        if address + length as u64 > u64::from(ADDRESS_SPACE) {
            return Err(format!(
                "#{address:X} + {length} nibbles runs past the end of the address space (#FFFFF)"
            ));
        }
        // Below 2^20, checked above.
        Ok(address as u32)
    }

    /// `length` nibbles from `address` through the current memory mapping,
    /// without side effects, as hex digits.
    fn peek(&mut self, msg: &Value) -> Result<Value, String> {
        let length = usize::try_from(u64_field(msg, "length")?).unwrap_or(usize::MAX);
        let address = Self::mem_range(msg, length)?;
        let m = self.emu()?.machine();
        let nibbles: String = (address..address + length as u32)
            .map(|a| char::from(b"0123456789ABCDEF"[usize::from(m.peek(a) & 0xF)]))
            .collect();
        Ok(json!({"address": address, "nibbles": nibbles}))
    }

    /// Write the hex digits `nibbles` from `address` on, as CPU writes
    /// through the current mapping (ROM ignores them, I/O registers react).
    fn poke(&mut self, msg: &Value) -> Result<Value, String> {
        let hex = str_field(msg, "nibbles")?;
        let address = Self::mem_range(msg, hex.len())?;
        let values: Vec<u8> = hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect::<Option<_>>()
            .ok_or("nibbles must be hex digits")?;
        let e = self.emu()?;
        for (a, &n) in (address..).zip(&values) {
            e.machine_mut().hw.write_nibble(a, n);
        }
        // The CPU may sleep on stale facts; the display may show the write.
        e.reshow();
        self.memory.force = true;
        self.schedule();
        Ok(json!({"address": address, "length": values.len()}))
    }

    fn boot(&mut self, preferred: &str, path: &Path) -> Result<Value, String> {
        let rom = read_capped(path, max_rom_file())?;
        let model = model_for_rom_name(&rom, preferred)?;
        let emu = Emulator::new(model.name(), &rom)?;
        self.emu = Some(emu);
        self.model = Some(model);
        self.rom_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.halted = None;
        self.memory.force = true;
        self.set_running(true);
        Ok(json!({"model": model.name(), "romName": self.rom_name}))
    }

    /// Commands queued keys: wake a sleeping machine (its time first, then
    /// the keys), then feed the queue and run passes for it.
    fn after_keys(&mut self) {
        if matches!(self.mode, Mode::Sleep(_)) {
            self.wake();
        }
        if let Some(e) = self.emu.as_mut() {
            e.pump();
        }
        self.schedule();
    }

    fn set_running(&mut self, on: bool) {
        self.running = on && self.emu.is_some() && self.halted.is_none();
        self.behind_ms = 0.0;
        self.mode = Mode::Stopped;
        if self.running {
            self.anchor();
            self.mode = Mode::Busy;
        }
    }

    fn set_speed(&mut self, speed: Speed) {
        // A sleeping machine first catches up at the old rate ...
        if matches!(self.mode, Mode::Sleep(_)) {
            self.wake();
        }
        self.speed = speed;
        self.anchor();
        // ... and sleeps on the new schedule.
        self.schedule();
    }

    /// Follow the wall clock from now.
    fn anchor(&mut self) {
        self.rebases += self.pacer.rebases;
        if let Some(e) = &self.emu {
            let m = e.machine();
            let factor = match self.speed {
                Speed::Times(n) => n,
                Speed::Max => 1,
            };
            self.pacer =
                Pacer::with_speed(m.model().clock_hz(), factor, Instant::now(), m.cycles());
        }
    }

    fn halt(&mut self, err: String) {
        self.halted = Some(err);
        self.set_running(false);
    }

    /// Run `ms` emulated ms in slices, feeding the keys if `keys`, within
    /// `budget` of wall time; returns the emulated ms left unrun.
    fn run_slices(&mut self, ms: f64, budget: Duration, keys: bool) -> Result<f64, String> {
        let start = Instant::now();
        let mut left = ms;
        let result = (|| {
            let Some(e) = self.emu.as_mut() else {
                return Ok(());
            };
            while left > 0.0 {
                left -= e.run_slice(left, keys)?;
                if start.elapsed() > budget {
                    break;
                }
            }
            Ok(())
        })();
        self.work += start.elapsed();
        result.map(|()| left.max(0.0))
    }

    /// One paced pass while busy.
    fn pass(&mut self) {
        self.ticks += 1;
        let Some(e) = &self.emu else {
            self.mode = Mode::Stopped;
            return;
        };
        let m = e.machine();
        let clock_hz = f64::from(m.model().clock_hz());
        let cycles = m.cycles();
        let result = if self.speed == Speed::Max {
            let owed = self.behind_ms;
            self.run_slices(owed + MAX_EMULATED_PER_PASS_MS, MAX_BUDGET, true)
                .map(|left| self.behind_ms = (left - MAX_EMULATED_PER_PASS_MS).max(0.0))
        } else {
            self.paced(clock_hz, cycles)
        };
        if let Err(err) = result {
            self.halt(err);
            return;
        }
        self.schedule();
        self.flush(false);
    }

    /// A paced pass: owed time from a sleep first, then the pacer's slices
    /// until the machine has caught up with the wall clock or the pass has
    /// used its budget; a short sleep when it is ahead.
    fn paced(&mut self, clock_hz: f64, mut cycles: u64) -> Result<(), String> {
        let start = Instant::now();
        if self.behind_ms > 0.0 {
            self.behind_ms = self.run_slices(self.behind_ms, WAKE_BUDGET, true)?;
            if let Some(e) = &self.emu {
                cycles = e.machine().cycles();
            }
            // The pacer follows the wall clock from here.
            self.pacer.rebase(Instant::now(), cycles);
            return Ok(());
        }
        let mut ran = false;
        while start.elapsed() < PASS_BUDGET {
            let n = self.pacer.budget(Instant::now(), cycles);
            if n == 0 {
                break;
            }
            ran = true;
            self.run_slices(n as f64 * 1000.0 / clock_hz, PASS_BUDGET, true)?;
            let Some(e) = &self.emu else { break };
            cycles = e.machine().cycles();
            // Keys or a sleep change the schedule: let the loop look.
            if e.keys_busy() || e.idle_ms().is_some() {
                break;
            }
        }
        if !ran {
            std::thread::sleep(AHEAD_SLEEP);
        }
        Ok(())
    }

    /// Busy while the CPU computes or keys are queued; else sleep until its
    /// next timer event (now, if time is owed).
    fn schedule(&mut self) {
        if !self.running {
            self.mode = Mode::Stopped;
            return;
        }
        let Some(e) = &self.emu else {
            self.mode = Mode::Stopped;
            return;
        };
        let idle = e.idle_ms();
        match idle {
            Some(idle_ms) if !e.keys_busy() => {
                if !matches!(self.mode, Mode::Sleep(_)) {
                    // Wall time is accounted for up to where the pacer
                    // has the machine (a pass may end a little behind or
                    // ahead of the clock).
                    self.slept_at = match self.speed {
                        Speed::Max => Instant::now(),
                        Speed::Times(_) => self.pacer.instant_of(e.machine().cycles()),
                    };
                    // Show what the ROM drew before it went to sleep.
                    self.flush(true);
                }
                let wall_ms = if self.behind_ms > 0.0 {
                    0.0
                } else {
                    (idle_ms / self.speed.rate()).min(1e9) + 1.0
                };
                self.mode = Mode::Sleep(self.slept_at + Duration::from_secs_f64(wall_ms / 1000.0));
            }
            _ => {
                if matches!(self.mode, Mode::Sleep(_)) {
                    self.anchor();
                }
                self.mode = Mode::Busy;
            }
        }
    }

    /// Leave the sleep: run all the emulated time that passed (cheap while
    /// the CPU sleeps); what does not fit the budget stays owed.
    fn wake(&mut self) {
        if !matches!(self.mode, Mode::Sleep(_)) {
            return;
        }
        self.wakes += 1;
        let now = Instant::now();
        let slept = now.saturating_duration_since(self.slept_at).as_secs_f64() * 1000.0;
        self.behind_ms = (self.behind_ms + slept * self.speed.rate()).min(MAX_BEHIND_MS);
        match self.run_slices(self.behind_ms, WAKE_BUDGET, false) {
            Ok(left) => self.behind_ms = left,
            Err(err) => {
                self.halt(err);
                return;
            }
        }
        self.slept_at = now;
        self.anchor();
        // Sleep on (from `now`) or start passes.
        self.mode = Mode::Sleep(now);
        self.schedule();
        if matches!(self.mode, Mode::Sleep(_)) {
            self.slept_at = now;
        }
        self.flush(false);
    }

    /// Send the status if it changed, and errors, keys and a frame if they
    /// changed (a frame at most every [`FRAME_INTERVAL`] unless `now`).
    fn flush(&mut self, now: bool) {
        if let Some(e) = self.emu.as_mut() {
            for message in e.queue().take_errors() {
                self.sink
                    .event(json!({"type": "error", "message": message}));
            }
            if let Some(k) = e.keys_if_changed().and_then(|s| json_of(&s).ok()) {
                self.sink.event(k);
            }
            let due = now
                || self
                    .last_frame
                    .is_none_or(|t| t.elapsed() >= FRAME_INTERVAL);
            if due && let Some(f) = e.frame_if_changed().and_then(|s| json_of(&s).ok()) {
                self.last_frame = Some(Instant::now());
                self.sink.event(f);
            }
        }
        self.send_status();
    }

    /// Send the status if it changed.
    fn send_status(&mut self) {
        let status = json!({
            "type": "status",
            "model": self.model.map(|m| m.name()),
            "romName": self.rom_name,
            "running": self.running,
            "halted": self.halted,
            "speed": self.speed.name(),
            "loop": self.loop_name(),
            "busy": self.busy,
        });
        if self.last_status.as_ref() != Some(&status) {
            self.last_status = Some(status.clone());
            self.sink.event(status);
        }
    }

    fn loop_name(&self) -> &'static str {
        match self.mode {
            Mode::Stopped => "stopped",
            Mode::Busy => "frame",
            Mode::Sleep(_) => "sleep",
        }
    }

    fn stats(&self) -> Value {
        let (cycles, emulated_ms) = self.emu.as_ref().map_or((0, 0.0), |e| {
            let m = e.machine();
            let c = m.cycles();
            (c, c as f64 * 1000.0 / f64::from(m.model().clock_hz()))
        });
        let owed = self.behind_ms
            + match self.mode {
                Mode::Sleep(_) => {
                    self.slept_at.elapsed().as_secs_f64() * 1000.0 * self.speed.rate()
                }
                _ => 0.0,
            };
        json!({
            "cycles": cycles,
            "emulatedMs": emulated_ms,
            "workMs": self.work.as_secs_f64() * 1000.0,
            "ticks": self.ticks,
            "wakes": self.wakes,
            "memoryLooks": self.memory.looks,
            "memoryMs": self.memory.spent.as_secs_f64() * 1000.0,
            "rebases": self.rebases + self.pacer.rebases,
            "loop": self.loop_name(),
            "owedMs": owed,
            "nowMs": self.started.elapsed().as_secs_f64() * 1000.0,
        })
    }
}

/// Start the Tauri app's machine thread; commands go into the returned
/// sender.
pub fn spawn<S: Sink>(sink: S) -> std::io::Result<Sender<Request>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("saturnus-machine".into())
        .spawn(move || {
            Runner::new(sink).run(&rx);
        })?;
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoSink;
    impl Sink for NoSink {
        fn event(&self, _: Value) {}
    }

    /// A runner on a 48SX with a ROM of zeros (it runs nonsense).
    fn runner() -> Runner<NoSink> {
        let mut r = Runner::for_host(NoSink, "http");
        r.start(
            Emulator::new("48sx", &vec![0u8; 256 * 1024]).unwrap(),
            "zeros",
        );
        r
    }

    fn send(
        r: &mut Runner<NoSink>,
        mut msg: Value,
        ticket: Option<Arc<Ticket>>,
    ) -> Result<Value, String> {
        msg["v"] = json!(PROTOCOL);
        let (reply, answer) = std::sync::mpsc::channel();
        r.serve(Request {
            msg,
            file: None,
            reply: Some(reply),
            ticket,
        });
        answer.recv().unwrap()
    }

    #[test]
    fn a_withdrawn_command_does_not_run() {
        let mut r = runner();
        let t = Ticket::new();
        assert!(t.cancel(), "not started yet");
        let e = send(&mut r, json!({"cmd": "keyDown", "key": "on"}), Some(t)).unwrap_err();
        assert_eq!(e, CANCELLED);
        assert!(!r.emu.as_ref().unwrap().keys_busy(), "the press was queued");
        send(
            &mut r,
            json!({"cmd": "keyDown", "key": "on"}),
            Some(Ticket::new()),
        )
        .unwrap();
        assert!(r.emu.as_ref().unwrap().keys_busy());
    }

    #[test]
    fn a_running_script_is_stopped_when_withdrawn() {
        let mut r = runner();
        let t = Ticket::new();
        let canceller = {
            let t = Arc::clone(&t);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                assert!(!t.cancel(), "it had started");
            })
        };
        let start = Instant::now();
        let e = send(
            &mut r,
            json!({"cmd": "keyScript", "script": "wait 600000"}),
            Some(t),
        )
        .unwrap_err();
        canceller.join().unwrap();
        assert!(e.contains("cancelled"), "{e}");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn key_commands_refuse_unknown_keys_and_no_machine() {
        let mut r = runner();
        for msg in [
            json!({"cmd": "keyDown", "key": "bogus"}),
            json!({"cmd": "keyDown", "key": "apps"}),
            json!({"cmd": "keyUp", "key": "bogus"}),
            json!({"cmd": "typeKeys", "keys": ["1", "bogus"]}),
            json!({"cmd": "typeKeys", "keys": ["1", 2]}),
            json!({"cmd": "typeKeys"}),
        ] {
            assert!(send(&mut r, msg.clone(), None).is_err(), "{msg}");
        }
        assert!(!r.emu.as_ref().unwrap().keys_busy(), "nothing was queued");
        send(&mut r, json!({"cmd": "keyUp", "key": "on"}), None).unwrap();
        send(
            &mut r,
            json!({"cmd": "typeKeys", "keys": ["1", "enter"]}),
            None,
        )
        .unwrap();
        let mut none = Runner::new(NoSink);
        for msg in [
            json!({"cmd": "keyDown", "key": "on"}),
            json!({"cmd": "keyUp", "key": "on"}),
            json!({"cmd": "typeKeys", "keys": ["on"]}),
        ] {
            let e = send(&mut none, msg, None).unwrap_err();
            assert!(e.contains("no ROM"), "{e}");
        }
    }

    #[test]
    fn typing_refuses_before_pressing_a_key() {
        let mut none = Runner::new(NoSink);
        for msg in [
            json!({"cmd": "insert", "text": "1"}),
            json!({"cmd": "commandLine"}),
        ] {
            let e = send(&mut none, msg, None).unwrap_err();
            assert!(e.contains("no ROM"), "{e}");
        }
        let mut r = Runner::new(NoSink);
        let rom = vec![0u8; Model::Hp42s.rom_bytes()];
        r.start(Emulator::new("42s", &rom).unwrap(), "42s.rom");
        for msg in [
            json!({"cmd": "run", "text": "1"}),
            json!({"cmd": "typeText", "text": "1"}),
            json!({"cmd": "commandLine"}),
        ] {
            let e = send(&mut r, msg.clone(), None).unwrap_err();
            assert!(e.contains("42S has no RPL command line"), "{msg}: {e}");
        }
        let e = send(&mut r, json!({"cmd": "insert"}), None).unwrap_err();
        assert!(e.contains("\"text\""), "{e}");
    }

    #[test]
    fn base64_round_trips() {
        for n in 0..40 {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(base64_decode(&base64(&bytes)).unwrap(), bytes);
        }
        assert_eq!(base64_decode("aGk").unwrap(), b"hi");
        assert!(base64_decode("a").is_err());
        assert!(base64_decode("a*bc").is_err());
    }
}
