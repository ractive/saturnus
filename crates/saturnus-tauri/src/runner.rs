//! The machine thread: owns the emulator, paces it against the wall clock
//! and answers the front end's protocol (`web/protocol.md`). Plain Rust,
//! no Tauri: commands arrive on a channel, events leave through a
//! [`Sink`], so it runs and is tested without a window.
//!
//! The rules are the Web Worker's (`web/worker.js`): while the CPU
//! computes or keys are queued the [`Pacer`] hands out emulated time in
//! 1 ms slices of wall time (times the speed), dropping a lag longer than
//! 200 ms; while it sleeps in SHUTDN with nothing queued the thread blocks
//! until the next timer event or a command, then runs all the time that
//! passed (up to 12 hours), owing what does not fit its budget.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use saturnus::Model;
use saturnus_drive::pacer::Pacer;
use saturnus_web::Emulator;
use saturnus_web::host::model_for_rom_name;
use serde_json::{Value, json};

/// The protocol version this host speaks.
pub const PROTOCOL: u64 = 1;
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
/// Sleep when the machine is ahead of the wall clock.
const AHEAD_SLEEP: Duration = Duration::from_micros(500);

/// Where events go: the Tauri window, or a test's collector.
pub trait Sink: Send + 'static {
    /// Deliver one event (a protocol message with a `type`).
    fn event(&self, msg: Value);
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
}

impl<S: Sink> std::fmt::Debug for Runner<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner")
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

fn json_of(s: &str) -> Result<Value, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}

impl<S: Sink> Runner<S> {
    /// A runner with no machine yet.
    pub fn new(sink: S) -> Self {
        let now = Instant::now();
        Self {
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
        }
    }

    /// Serve `rx` until every sender is gone.
    pub fn run(mut self, rx: &Receiver<Request>) {
        loop {
            // Before blocking, send what the frame throttle held back: a
            // display that changed within 16 ms of the last frame would
            // otherwise stay unsent for the whole sleep.
            if self.mode != Mode::Busy {
                self.flush(true);
            }
            let req = match self.mode {
                Mode::Stopped => match rx.recv() {
                    Ok(r) => Some(r),
                    Err(_) => return,
                },
                Mode::Busy => match rx.try_recv() {
                    Ok(r) => Some(r),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                },
                Mode::Sleep(until) => {
                    let wait = until.saturating_duration_since(Instant::now());
                    match rx.recv_timeout(wait) {
                        Ok(r) => Some(r),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            };
            if let Some(req) = req {
                self.serve(req);
                continue;
            }
            match self.mode {
                Mode::Busy => self.pass(),
                Mode::Sleep(_) => self.wake(),
                Mode::Stopped => {}
            }
        }
    }

    /// Answer one request.
    /// The events the command caused go out before its reply
    /// (`web/protocol.md`): a caller that has the reply has the state.
    pub fn serve(&mut self, req: Request) {
        let result = self.handle(&req.msg, req.file.as_deref());
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
                "{f:?} is not accepted: this host chooses files in its own dialogs"
            ));
        }
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
                if let Some(e) = self.emu.as_mut() {
                    e.reshow();
                }
                let models: Vec<&str> = Model::ALL.iter().map(|m| m.name()).collect();
                Ok(json!({"protocol": PROTOCOL, "host": "tauri", "models": models}))
            }
            "skin" => {
                let m = saturnus_web::model_from_name(str_field(msg, "model")?)?;
                json_of(&saturnus_web::skins::skin_json(m))
            }
            "layout" => json_of(&saturnus_web::host::layout_of(str_field(msg, "model")?)?),
            "boot" => {
                let path = file("boot")?.to_path_buf();
                self.boot(str_field(msg, "model")?, &path)
            }
            "keyDown" => {
                let key = str_field(msg, "key")?.to_string();
                if let Some(e) = self.emu.as_mut() {
                    e.queue().press(&key);
                    self.after_keys();
                }
                Ok(Value::Null)
            }
            "keyUp" => {
                let key = str_field(msg, "key")?.to_string();
                if let Some(e) = self.emu.as_mut() {
                    e.queue().release(&key);
                    e.pump();
                }
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
                    .map(|a| {
                        a.iter()
                            .filter_map(|k| k.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(e) = self.emu.as_mut() {
                    let names: Vec<&str> = keys.iter().map(String::as_str).collect();
                    e.queue().type_keys(&names);
                    self.after_keys();
                }
                Ok(Value::Null)
            }
            "releaseAll" => {
                if let Some(e) = self.emu.as_mut() {
                    e.release_all_inner();
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
                e.release_all_inner();
                e.reset();
                self.halted = None;
                self.set_running(true);
                Ok(Value::Null)
            }
            "saveState" => {
                let path = file("saveState")?.to_path_buf();
                let state = self.emu()?.save_state();
                write_atomic(&path, &state, |f, b| f.write_all(b))
                    .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
                Ok(json!({"path": path.display().to_string()}))
            }
            "loadState" => {
                let path = file("loadState")?.to_path_buf();
                let data = read_capped(&path, MAX_STATE_FILE)?;
                let e = self.emu()?;
                e.release_all_inner();
                e.load_state_inner(&data)?;
                e.reshow();
                self.halted = None;
                self.set_running(self.running);
                Ok(json!({"path": path.display().to_string()}))
            }
            // The window's visibility does not pause a native machine.
            "visibility" => Ok(Value::Null),
            "stats" => Ok(self.stats()),
            other => Err(format!("unknown command {other:?}")),
        }
    }

    fn boot(&mut self, preferred: &str, path: &Path) -> Result<Value, String> {
        let rom = read_capped(path, max_rom_file())?;
        let model = model_for_rom_name(&rom, preferred)?;
        let emu = Emulator::new_inner(model.name(), &rom)?;
        self.emu = Some(emu);
        self.model = Some(model);
        self.rom_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.halted = None;
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
                left -= e.run_slice_inner(left, keys)?;
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
            if e.keys_busy() || e.idle_ms_inner().is_some() {
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
        let idle = e.idle_ms_inner();
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
        let status = json!({
            "type": "status",
            "model": self.model.map(|m| m.name()),
            "romName": self.rom_name,
            "running": self.running,
            "halted": self.halted,
            "speed": self.speed.name(),
            "loop": self.loop_name(),
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
            "rebases": self.rebases + self.pacer.rebases,
            "loop": self.loop_name(),
            "owedMs": owed,
            "nowMs": self.started.elapsed().as_secs_f64() * 1000.0,
        })
    }
}

/// Start the machine thread; commands go into the returned sender.
pub fn spawn<S: Sink>(sink: S) -> std::io::Result<Sender<Request>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("saturnus-machine".into())
        .spawn(move || Runner::new(sink).run(&rx))?;
    Ok(tx)
}
