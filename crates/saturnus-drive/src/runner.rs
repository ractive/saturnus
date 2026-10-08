//! The machine thread of the native hosts: a thin driver around the
//! protocol's state machine ([`saturnus_host::protocol::Engine`], shared
//! with the Web Worker), which owns the emulator, paces it and answers the
//! front end's protocol (`web/protocol.md`). Commands arrive on a channel,
//! events leave through a [`Sink`], replies through each command's own
//! channel; the thread blocks until the engine's next deadline or the next
//! command. Two hosts use it: the Tauri app (its window) and `saturnus
//! run` (the HTTP control API, with the serial bridge as a [`Hook`]).
//!
//! What the engine leaves to a native host is here: the files of `boot`,
//! `saveState`, `loadState`, `storeFile` and `fetchFile` that the host
//! chose ([`crate::files`]), and
//! the commands only the native hosts serve (`screen`, `info`, `model`,
//! `peek`, `poke`, `keyScript`). Key scripts run at once in emulated time
//! on this thread, as the CLI runs a key script, and reply when the
//! calculator is idle again; nothing else is served meanwhile. Sends
//! (`insert`, `run`, ...) run in the engine's turns, between other
//! commands.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};

use saturnus::Machine;
use saturnus_host::host::{base64, pack_bits};
use saturnus_host::protocol::{self, Clock, Engine, Output, Pacing, RunError};
use saturnus_host::{AnnunciatorFlags, Emulator};
use serde_json::{Value, json};

use crate::script;
use crate::session::{Limits, Session};

pub use crate::files::{
    MAX_STATE_FILE, max_rom_file, read_capped, write_atomic, write_atomic_with,
};
pub use saturnus_host::host::base64_decode;

/// The protocol version this host speaks.
pub const PROTOCOL: u64 = protocol::VERSION;
/// The 20-bit nibble address space every model's CPU sees.
pub const ADDRESS_SPACE: u32 = 0x10_0000;
/// Most nibbles one `peek` or `poke` reads or writes.
pub const MAX_MEM_NIBBLES: usize = 64 * 1024;
/// Wall time a key script or a send may take on the machine thread (it
/// runs in emulated time, much faster than real time while the calculator
/// waits for keys).
pub const SCRIPT_WALL_LIMIT: Duration = Duration::from_millis(protocol::WALL_LIMIT_MS as u64);

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
    /// Stops a running key script or send at its next slice or turn.
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
    /// will. `false`: it has started; a key script or a send is then
    /// stopped at its next slice or turn (within about 50 emulated ms),
    /// anything else finishes, and its reply still comes.
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

/// Fields that name files: the page may not send them to this host.
const PATH_FIELDS: [&str; 2] = ["romPath", "path"];

/// The commands only the native hosts serve (`web/protocol.md`).
const NATIVE_COMMANDS: [&str; 6] = ["screen", "info", "model", "peek", "poke", "keyScript"];

pub use saturnus_host::protocol::WRITE_COMMANDS;

/// Largest file `storeFile` reads.
pub const MAX_FILE: usize = saturnus_host::transfer::MAX_FILE_BYTES;

/// The file name of `path`, for replies (never the whole path: a reply
/// may reach the page).
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The variable a file is stored as when the page named none: the file's
/// name without its extension (`PROG.hp` is `PROG`).
pub fn variable_name(path: &Path) -> String {
    path.file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The wall clock, in ms since the runner started.
#[derive(Debug, Clone, Copy)]
struct Wall(Instant);

impl Clock for Wall {
    fn now_ms(&self) -> f64 {
        self.0.elapsed().as_secs_f64() * 1000.0
    }
}

/// A reply the engine still owes.
#[derive(Debug)]
struct Pending {
    reply: Sender<Result<Value, String>>,
    abort: Option<Arc<AtomicBool>>,
    /// `fetchFile` with a file the host chose: the fetched bytes go there.
    save: Option<PathBuf>,
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

/// A typed answer as the reply's JSON value.
fn value_of<T: serde::Serialize>(v: &T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// An error as the reply's or event's message: the protocol's edge.
fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The machine thread's state.
pub struct Runner<S: Sink> {
    sink: S,
    engine: Engine,
    clock: Wall,
    /// The host's name in `hello` and `info` ("tauri", "http").
    host: &'static str,
    /// What the host runs beside the machine.
    hook: Option<Box<dyn Hook>>,
    /// Fields the host adds to `info` (its endpoints, the ROM's hash).
    info_extra: serde_json::Map<String, Value>,
    /// Replies still owed, by the tag the engine knows them by.
    pending: HashMap<u64, Pending>,
    next_tag: u64,
    /// The abort flag of the command being handled, if its sender can
    /// withdraw it.
    abort: Option<Arc<AtomicBool>>,
}

impl<S: Sink> std::fmt::Debug for Runner<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner")
            .field("host", &self.host)
            .field("status", &self.engine.status())
            .finish_non_exhaustive()
    }
}

impl<S: Sink> Runner<S> {
    /// A runner with no machine yet, for the Tauri app.
    pub fn new(sink: S) -> Self {
        Self::for_host(sink, "tauri")
    }

    /// A runner with no machine yet for the host called `host`.
    pub fn for_host(sink: S, host: &'static str) -> Self {
        Self {
            sink,
            engine: Engine::new(host, Pacing::NATIVE),
            clock: Wall(Instant::now()),
            host,
            hook: None,
            info_extra: serde_json::Map::new(),
            pending: HashMap::new(),
            next_tag: 0,
            abort: None,
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
        self.engine.start(&self.clock, emu, rom_name);
        self.deliver();
    }

    /// The emulator, once [`Runner::run`] has returned.
    pub fn into_emulator(self) -> Option<Emulator> {
        self.engine.into_emulator()
    }

    /// One turn of the hook; `true` means stop.
    fn service(&mut self) -> bool {
        let Some(hook) = self.hook.as_mut() else {
            return false;
        };
        match hook.service(self.engine.emulator_mut().map(Emulator::machine_mut)) {
            Service::Idle => false,
            Service::Stop => true,
            Service::Input => {
                self.engine.input(&self.clock);
                self.deliver();
                false
            }
        }
    }

    /// Serve `rx` until every sender is gone or the hook says stop; returns
    /// the runner, which still holds the machine.
    pub fn run(mut self, rx: &Receiver<Request>) -> Self {
        loop {
            if self.service() {
                return self;
            }
            self.check_abort();
            // The engine's next deadline and the hook's turn bound the wait.
            let now = self.clock.now_ms();
            let due = self
                .engine
                .deadline()
                .map(|d| Duration::from_secs_f64(((d - now) / 1000.0).max(0.0)));
            let wait = [due, self.hook.as_ref().map(|h| h.interval())]
                .into_iter()
                .flatten()
                .min();
            let req = match wait {
                Some(d) if d.is_zero() => match rx.try_recv() {
                    Ok(r) => Some(r),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return self,
                },
                Some(d) => match rx.recv_timeout(d) {
                    Ok(r) => Some(r),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return self,
                },
                None => match rx.recv() {
                    Ok(r) => Some(r),
                    Err(_) => return self,
                },
            };
            match req {
                Some(req) => self.serve(req),
                None => {
                    self.engine.timer(&self.clock);
                    self.deliver();
                }
            }
        }
    }

    /// A send whose sender gave up is stopped.
    fn check_abort(&mut self) {
        let aborted = self
            .engine
            .sending()
            .and_then(|tag| self.pending.get(&tag))
            .and_then(|p| p.abort.as_ref())
            .is_some_and(|a| a.load(Ordering::SeqCst));
        if aborted {
            self.engine.cancel_send(&self.clock, "cancelled");
            self.deliver();
        }
    }

    /// Answer one request. The events the command caused go out before
    /// its reply (`web/protocol.md`): a caller that has the reply has the
    /// state. A withdrawn command (see [`Ticket`]) is answered
    /// [`CANCELLED`] without running.
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
        let tag = req.reply.map(|reply| {
            self.next_tag += 1;
            self.pending.insert(
                self.next_tag,
                Pending {
                    reply,
                    abort: self.abort.clone(),
                    save: None,
                },
            );
            self.next_tag
        });
        self.handle(&req.msg, req.file.as_deref(), tag);
        self.abort = None;
        self.deliver();
    }

    /// Hand the engine's output on: events to the sink, replies to their
    /// callers (a *bytes* field as base64).
    fn deliver(&mut self) {
        for out in self.engine.take_output() {
            match out {
                Output::Event(e) => {
                    if let Ok(v) = serde_json::to_value(&e) {
                        self.sink.event(v);
                    }
                }
                Output::Reply(r) => {
                    let Some(p) = self.pending.remove(&r.tag) else {
                        continue;
                    };
                    let mut result = r.result;
                    if let (Ok(v), Some((field, bytes))) = (&mut result, r.bytes) {
                        match &p.save {
                            Some(path) => {
                                if let Err(e) = write_atomic(path, &bytes, |f, b| f.write_all(b)) {
                                    result = Err(format!("cannot write {}: {e}", file_name(path)));
                                } else {
                                    v["file"] = json!(file_name(path));
                                }
                            }
                            None => v[field] = json!(base64(&bytes)),
                        }
                    }
                    let _ = p.reply.send(result);
                }
            }
        }
    }

    /// One protocol command; `file` is the host's choice for the commands
    /// that need one.
    fn handle(&mut self, msg: &Value, file: Option<&Path>, tag: Option<u64>) {
        let clock = self.clock;
        if let Some(f) = PATH_FIELDS.iter().find(|f| msg.get(**f).is_some()) {
            let e = format!(
                "{f:?} is not accepted: a message never names a file, the host chooses them"
            );
            self.engine.answer(&clock, tag, Err(e.into()));
            return;
        }
        let cmd = match self.engine.admit(msg) {
            Ok(cmd) => cmd,
            Err(e) => {
                self.engine.answer(&clock, tag, Err(e));
                return;
            }
        };
        let result = match (cmd, file) {
            ("boot", file) => {
                let rom = file
                    .ok_or_else(|| "boot needs a file".to_string())
                    .and_then(|p| read_capped(p, max_rom_file()).map(|r| (p, r)));
                match rom {
                    Ok((path, rom)) => {
                        let mut msg = msg.clone();
                        msg["romName"] = json!(
                            path.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        );
                        self.engine.command(&clock, &msg, Some(rom), tag);
                        return;
                    }
                    Err(e) => Err(e),
                }
            }
            ("storeFile", Some(path)) => match read_capped(path, MAX_FILE as u64) {
                Ok(data) => {
                    let mut msg = msg.clone();
                    if msg.get("name").is_none() {
                        msg["name"] = json!(variable_name(path));
                    }
                    self.engine.command(&clock, &msg, Some(data), tag);
                    return;
                }
                Err(e) => Err(e),
            },
            ("fetchFile", Some(path)) => {
                if let Some(p) = tag.and_then(|t| self.pending.get_mut(&t)) {
                    p.save = Some(path.to_path_buf());
                }
                self.engine.command(&clock, msg, None, tag);
                return;
            }
            ("saveState", Some(path)) => self.engine.save_state().map_err(text).and_then(|state| {
                write_atomic(path, &state, |f, b| f.write_all(b))
                    .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                Ok(json!({"path": path.display().to_string()}))
            }),
            ("loadState", Some(path)) => read_capped(path, MAX_STATE_FILE).and_then(|data| {
                self.engine.load_state(&clock, &data).map_err(text)?;
                Ok(json!({"path": path.display().to_string()}))
            }),
            (cmd, _) if NATIVE_COMMANDS.contains(&cmd) => self.native(cmd, msg),
            _ => {
                self.engine.command(&clock, msg, None, tag);
                return;
            }
        };
        self.engine.answer(&clock, tag, result.map_err(Into::into));
    }

    /// The commands only the native hosts serve.
    fn native(&mut self, cmd: &str, msg: &Value) -> Result<Value, String> {
        match cmd {
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
            other => Err(format!("unknown command {other:?}")),
        }
    }

    /// Run `f` on the machine in a [`Session`] (emulated time, as fast as
    /// the host can, at most [`SCRIPT_WALL_LIMIT`] of wall time) with every
    /// key released first; then follow the wall clock again from where it
    /// left the machine. Replies `{emulatedMs, warnings}`.
    fn with_session(
        &mut self,
        f: impl FnOnce(&mut Session) -> anyhow::Result<()>,
    ) -> Result<Value, String> {
        let limits = Limits {
            abort: self.abort.clone(),
            deadline: Some(Instant::now() + SCRIPT_WALL_LIMIT),
        };
        let clock = self.clock;
        self.engine
            .exclusive(&clock, |machine| {
                let mut s = Session::new(machine, 0, false);
                s.set_echo_warnings(false);
                s.set_limits(limits);
                let from = s.machine.cycles();
                let result = f(&mut s);
                if result.is_err() {
                    s.machine.release_all_keys();
                }
                let warnings = s.take_warnings();
                let ms = (s.machine.cycles() - from) as f64 * 1000.0
                    / f64::from(s.machine.model().clock_hz());
                let result = result
                    .map(|()| json!({"emulatedMs": ms, "warnings": warnings}))
                    .map_err(|e| RunError {
                        message: format!("{e:#}"),
                        halted: e.downcast_ref::<crate::session::Halted>().is_some(),
                    });
                (s.machine, result)
            })
            .map_err(text)
    }

    /// The display now: the `frame` event's fields plus `rows` (text, `#`
    /// dark, `.` light, as the CLI's `.txt` screens) and `displayOn`; with
    /// `png: true` a 1-bit PNG instead (`scale` 1-8), as base64.
    fn screen(&mut self, msg: &Value) -> Result<Value, String> {
        let m = self.engine.emulator().map_err(text)?.machine();
        let fb = m.framebuffer();
        let (width, height) = (saturnus::LCD_WIDTH, fb.pixels.height());
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
            "annunciators": value_of(&AnnunciatorFlags(fb.annunciators))?,
            "contrast": fb.contrast,
            "contrastRange": [range.start(), range.end()],
            "contrastDefault": m.model().default_contrast(),
            "displayOn": m.display_on(),
        }))
    }

    /// The host, the machine and how it runs; the host's own fields
    /// ([`Runner::set_info`]) added.
    fn info(&self) -> Value {
        let s = self.engine.status();
        let m = self.engine.emulator().ok().map(Emulator::machine);
        let mut v = json!({
            "protocol": PROTOCOL,
            "host": self.host,
            "model": s.model,
            "romName": s.rom_name,
            "running": s.running,
            "halted": s.halted,
            "speed": s.speed,
            "loop": s.loop_state,
            "displayOn": m.map(Machine::display_on),
            "cycles": m.map(Machine::cycles),
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
        let m = self.engine.emulator().map_err(text)?.machine();
        let model = m.model();
        let height = m.lcd().height();
        Ok(json!({
            "model": model.name(),
            "clockHz": model.clock_hz(),
            "width": saturnus::LCD_WIDTH,
            "height": height,
            "hasSerial": model.has_serial(),
            "layout": value_of(&saturnus_host::layout::grid(model))?,
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
        let m = self.engine.emulator().map_err(text)?.machine();
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
        let clock = self.clock;
        self.engine.poke(&clock, address, &values).map_err(text)?;
        Ok(json!({"address": address, "length": values.len()}))
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
    use saturnus::Model;

    struct NoSink;
    impl Sink for NoSink {
        fn event(&self, _: Value) {}
    }

    /// A runner on a 48SX with a ROM of zeros (it runs nonsense).
    fn runner() -> Runner<NoSink> {
        let mut r = Runner::for_host(NoSink, "http");
        r.start(
            Emulator::new(Model::Hp48sx, &vec![0u8; 256 * 1024]).unwrap(),
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
        let busy = |r: &Runner<NoSink>| r.engine.emulator().unwrap().keys_busy();
        assert!(!busy(&r), "the press was queued");
        send(
            &mut r,
            json!({"cmd": "keyDown", "key": "on"}),
            Some(Ticket::new()),
        )
        .unwrap();
        assert!(busy(&r));
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

    /// A send runs between commands; a withdrawn one is stopped at its
    /// next turn and replies "cancelled".
    #[test]
    fn a_running_send_is_stopped_when_withdrawn() {
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || runner().run(&rx));
        let t = Ticket::new();
        let (reply, answer) = std::sync::mpsc::channel();
        tx.send(Request {
            msg: json!({"v": 1, "cmd": "insert", "text": "« 1 2 + » EVAL"}),
            file: None,
            reply: Some(reply),
            ticket: Some(Arc::clone(&t)),
        })
        .unwrap();
        // Served meanwhile; a key is refused.
        let (k, key) = std::sync::mpsc::channel();
        tx.send(Request {
            msg: json!({"v": 1, "cmd": "keyDown", "key": "on"}),
            file: None,
            reply: Some(k),
            ticket: None,
        })
        .unwrap();
        let e = key.recv().unwrap().unwrap_err();
        assert!(e.contains("typing is in progress"), "{e}");
        assert!(!t.cancel(), "it had started");
        let e = answer
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err();
        assert_eq!(e, "cancelled");
        drop(tx);
        thread.join().unwrap();
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
        assert!(
            !r.engine.emulator().unwrap().keys_busy(),
            "nothing was queued"
        );
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
            json!({"cmd": "peek", "address": 0, "length": 1}),
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
        r.start(Emulator::new(Model::Hp42s, &rom).unwrap(), "42s.rom");
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
    fn files_are_the_hosts_and_states_travel_as_base64_without_one() {
        let mut r = runner();
        let e = send(
            &mut r,
            json!({"cmd": "boot", "model": "48sx", "romPath": "/x"}),
            None,
        )
        .unwrap_err();
        assert!(e.contains("never names a file"), "{e}");
        let e = send(&mut r, json!({"cmd": "boot", "model": "48sx"}), None).unwrap_err();
        assert_eq!(e, "boot needs a file");
        let saved = send(&mut r, json!({"cmd": "saveState"}), None).unwrap();
        let state = saved["state"].as_str().unwrap().to_string();
        assert!(base64_decode(&state).unwrap().len() > 1000);
        let r2 = send(&mut r, json!({"cmd": "loadState", "state": state}), None).unwrap();
        assert_eq!(r2, json!({}));
        let info = send(&mut r, json!({"cmd": "info"}), None).unwrap();
        assert_eq!(
            (info["host"].as_str(), info["romName"].as_str()),
            (Some("http"), Some("zeros"))
        );
    }
}
