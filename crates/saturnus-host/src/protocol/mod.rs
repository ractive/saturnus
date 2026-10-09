//! The front end's protocol (`web/protocol.md`) and its pacing, once for
//! every host: [`Engine`] takes commands and gives replies and events,
//! and runs the machine against a clock the host passes in. It has no
//! clock, timer, thread or I/O of its own; a host is a thin driver around
//! it:
//!
//! - feed it the page's messages ([`Engine::command`]) with the host's
//!   [`Clock`];
//! - after every call, deliver [`Engine::take_output`] in order (the events
//!   a command caused come before its reply) and arm one timer for
//!   [`Engine::deadline`], which calls [`Engine::timer`].
//!
//! The Web Worker (`web/worker.js`, through `saturnus-web`) drives it with
//! `performance.now()` and `setTimeout`; the native machine thread
//! (`saturnus-drive`'s runner, for the desktop app and `saturnus run`)
//! with `Instant` and its channel's `recv_timeout`. What only one host has
//! stays with it: files and dialogs natively, IndexedDB and the ROM slots
//! in the browser, the native commands (`screen`, `info`, `model`,
//! `peek`, `poke`, `keyScript`), which the runner serves around the engine
//! ([`Engine::answer`], [`Engine::exclusive`]).
//!
//! Sends (`insert`, `run`, `replace`, `typeText`) and the writes to the
//! user memory (`storeFile`, `fetchFile`, `purge`, `rename`, `createDir`,
//! `changeDir`, `setFlag`, `storeText`: a hidden Kermit transaction,
//! [`crate::transfer`]) run in
//! turns between other messages; meanwhile the commands in
//! [`REFUSED_WHILE_TYPING`] are refused, `releaseAll` stops the send or
//! the write, and a send of more than 12 characters, or any write, holds
//! the frames, keys and errors until it ends.
//!
//! A host that keeps states ([`Engine::set_auto_save`]) gets the machine's
//! state as [`Output::Save`] once it changed and settled (`protocol/autosave.rs`),
//! and boots a model with the state it kept ([`Engine::boot_restoring`]).

mod autosave;
mod pacing;
mod watch;

#[cfg(test)]
mod tests;

pub use autosave::DELAY_MS as AUTO_SAVE_MS;
pub use pacing::{LoopState, MAX_BEHIND_MS, Pacing, Speed};
pub use watch::{EVENT_MS as MEMORY_EVENT_MS, LOOK_MS as MEMORY_LOOK_MS};

use saturnus::{Machine, Model};
use serde::Serialize;
use serde_json::{Value, json};

use crate::host::{Frame, KeysDown, base64_decode, model_for_rom};
use crate::transfer::{MAX_FILE_BYTES, Op, Target};
use crate::{Emulator, Error, Result};
use autosave::AutoSave;
use pacing::Loop;
use watch::Watch;

/// The protocol version.
pub const VERSION: u64 = 1;
/// Largest state accepted (`loadState`): the largest is the 49G's, 2.6 MB
/// (its 2 MiB flash and 512 KiB RAM); 4 MiB leaves half again as much.
pub const MAX_STATE_BYTES: usize = 4 * 1024 * 1024;
/// Emulated ms one step of a send runs.
pub const TYPING_STEP_MS: f64 = 20.0;
/// Wall time a send may take in all, in ms (also the native key scripts').
pub const WALL_LIMIT_MS: f64 = 30_000.0;
/// The commands refused while a send is typing: they press keys, swap or
/// reset the machine, or read the user memory the send is changing.
pub const REFUSED_WHILE_TYPING: [&str; 29] = [
    "keyDown",
    "keyUp",
    "typeLetter",
    "typeKeys",
    "insert",
    "typeText",
    "run",
    "replace",
    "boot",
    "bootModel",
    "chooseRom",
    "reset",
    "saveState",
    "loadState",
    "memoryTree",
    "stack",
    "flags",
    "objectAt",
    "keyScript",
    "poke",
    "storeFile",
    "fetchFile",
    "purge",
    "rename",
    "createDir",
    "changeDir",
    "setFlag",
    "storeText",
    "editText",
];
/// The commands that write the user memory through the Kermit server.
pub const WRITE_COMMANDS: [&str; 8] = [
    "storeFile",
    "fetchFile",
    "purge",
    "rename",
    "createDir",
    "changeDir",
    "setFlag",
    "storeText",
];

/// The host's clock: milliseconds from any fixed start, never going back.
pub trait Clock {
    /// Now, in ms.
    fn now_ms(&self) -> f64;
}

/// The `status` event: sent whenever one of its fields changed.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename = "status", rename_all = "camelCase")]
pub struct Status {
    /// The booted model's name.
    pub model: Option<&'static str>,
    /// The ROM file's name.
    pub rom_name: String,
    /// The machine runs (booted, not paused, not halted).
    pub running: bool,
    /// Why the CPU stopped, or `None`.
    pub halted: Option<String>,
    /// The speed setting.
    pub speed: Speed,
    /// What the loop does.
    #[serde(rename = "loop")]
    pub loop_state: LoopState,
    /// A long send holds the frames.
    pub busy: bool,
}

/// The `error` event: a command without an `id` failed, or a key the
/// machine refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename = "error")]
pub struct ErrorEvent {
    /// For the user.
    pub message: String,
}

/// The `memoryChanged` event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename = "memoryChanged")]
pub struct MemoryChanged {}

/// An event for the page (`web/protocol.md`, "Events"); serializes as
/// the protocol's JSON object.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Event {
    /// `frame`.
    Frame(Frame),
    /// `keys`.
    Keys(KeysDown),
    /// `status`.
    Status(Status),
    /// `error`.
    Error(ErrorEvent),
    /// `memoryChanged`.
    MemoryChanged(MemoryChanged),
}

/// The reply to a command that asked for one.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    /// The host's tag for the command ([`Engine::command`]).
    pub tag: u64,
    /// The result, or the error's message.
    pub result: std::result::Result<Value, String>,
    /// Bytes that belong in the result's field of this name (`saveState`'s
    /// `state`): the Worker passes them as a `Uint8Array`, JSON hosts as
    /// base64.
    pub bytes: Option<(&'static str, Vec<u8>)>,
}

/// The machine's state for the host's store: it changed and has settled
/// ([`Engine::set_auto_save`]). The host keeps it in the model's auto slot,
/// apart from the user's own saved state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    /// The model's name.
    pub model: &'static str,
    /// The ROM file's name.
    pub rom_name: String,
    /// CPU cycles since power-on.
    pub cycles: u64,
    /// The whole machine state (`saveState`'s).
    pub state: Vec<u8>,
}

/// What the engine has for the host, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    /// An event for every page.
    Event(Event),
    /// A reply for one caller.
    Reply(Reply),
    /// A state for the host's store, not for the page.
    Save(Saved),
}

/// `boot`'s result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Booted {
    /// The model booted (the ROM may have chosen another).
    pub model: &'static str,
    /// The ROM file's name.
    pub rom_name: String,
    /// The host's kept state was restored ([`Engine::boot_restoring`]);
    /// only `true` is sent.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub restored: bool,
    /// Why the kept state did not load (another ROM, an older format):
    /// the machine cold-booted instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_error: Option<String>,
}

/// `stats`'s result: counters for tests (`web/protocol.md`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// CPU cycles since power-on.
    pub cycles: u64,
    /// Emulated ms since power-on.
    pub emulated_ms: f64,
    /// Wall ms spent emulating (passes, wakes, sends, key scripts).
    pub work_ms: f64,
    /// Passes run.
    pub ticks: u64,
    /// Wakes from a sleep.
    pub wakes: u64,
    /// Looks at the user memory for `memoryChanged`.
    pub memory_looks: u64,
    /// Wall ms those looks took.
    pub memory_ms: f64,
    /// What the loop does.
    #[serde(rename = "loop")]
    pub loop_state: LoopState,
    /// Emulated ms owed to the wall clock: unpaid, plus the current sleep.
    pub owed_ms: f64,
    /// The host's clock.
    pub now_ms: f64,
}

/// Why a host's own run of the machine failed ([`Engine::exclusive`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunError {
    /// For the user.
    pub message: String,
    /// The CPU halted (the machine stops until a reset or a state load).
    pub halted: bool,
}

/// A send in progress.
#[derive(Clone, Debug)]
struct Send {
    /// Where its reply goes; `None`: errors become `error` events.
    tag: Option<u64>,
    /// It holds the frames (`status` `busy`).
    freezes: bool,
    /// A write through the Kermit server rather than typing.
    transfer: bool,
    /// When it started, on the host's clock.
    started: f64,
    /// When its next turn is due.
    due: f64,
}

/// The protocol's state machine (see the module docs).
#[derive(Debug)]
pub struct Engine {
    /// The host's name in `hello` ("worker", "tauri", "http").
    host: &'static str,
    emu: Option<Emulator>,
    model: Option<Model>,
    rom_name: String,
    halted: Option<String>,
    lp: Loop,
    watch: Watch,
    send: Option<Send>,
    out: Vec<Output>,
    last_status: Option<Status>,
    /// When the last `frame` event went out.
    last_frame: Option<f64>,
    autosave: AutoSave,
}

/// The string field `name` of `msg`.
fn str_field<'a>(msg: &'a Value, name: &str) -> Result<&'a str> {
    msg.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string field {name:?}").into())
}

/// A typed answer as the reply's JSON value.
fn value_of<T: Serialize>(v: &T) -> Result<Value> {
    serde_json::to_value(v).map_err(|e| e.to_string().into())
}

fn unknown_key(e: &Emulator, key: &str) -> Error {
    format!("no key {key:?} on the {}", e.model().name()).into()
}

/// What a command handler answers: a reply now (with bytes for a field),
/// or later (a send).
enum Answer {
    Now(Value, Option<(&'static str, Vec<u8>)>),
    Later,
}

impl From<Value> for Answer {
    fn from(v: Value) -> Self {
        Answer::Now(v, None)
    }
}

impl Engine {
    /// An engine with no machine yet, for the host called `host`, paced
    /// as `pacing` says.
    pub fn new(host: &'static str, pacing: Pacing) -> Engine {
        Engine {
            host,
            emu: None,
            model: None,
            rom_name: String::new(),
            halted: None,
            lp: Loop::new(pacing),
            watch: Watch::default(),
            send: None,
            out: Vec::new(),
            last_status: None,
            last_frame: None,
            autosave: AutoSave::default(),
        }
    }

    /// Whether the host keeps states: then the machine's state comes out
    /// as [`Output::Save`] once it changed and settled (off by default).
    pub fn set_auto_save(&mut self, on: bool) {
        self.autosave.on = on;
        if !on {
            self.autosave.clear();
        }
    }

    /// Saves handed to the host so far.
    pub fn auto_saves(&self) -> u64 {
        self.autosave.saves
    }

    /// A change is waiting to be saved.
    pub fn save_owed(&self) -> bool {
        self.autosave.owed()
    }

    /// Hand out the owed save now, without the delay, if the machine has
    /// settled (a computing one is not saved: its last settled state is
    /// in the store). A host calls it before a boot, and stores what it
    /// gives before it reads or clears the slot: a model switch keeps the
    /// machine it leaves, a reboot of the same model restores the newest
    /// state, and a fresh start deletes the slot after this save, so the
    /// old machine never comes back.
    pub fn save_now(&mut self) {
        if self.autosave.owed() && self.settled() {
            self.emit_save();
        }
    }

    /// The protocol version check and the command's name, refused while a
    /// send is typing if it is in [`REFUSED_WHILE_TYPING`]: what every
    /// command passes first, also those a host serves itself.
    pub fn admit<'a>(&self, msg: &'a Value) -> Result<&'a str> {
        match msg.get("v") {
            Some(v) if v.as_u64() == Some(VERSION) => {}
            Some(v) => {
                return Err(format!(
                    "protocol version {v} not supported (this host speaks {VERSION})"
                )
                .into());
            }
            None => {
                return Err(
                    format!("protocol version missing (this host speaks {VERSION})").into(),
                );
            }
        }
        let cmd = str_field(msg, "cmd")?;
        match self.refusal(cmd) {
            Some(e) => Err(e),
            None => Ok(cmd),
        }
    }

    /// Why `cmd` is refused now (a send is typing and it is in
    /// [`REFUSED_WHILE_TYPING`]), or `None`.
    pub fn refusal(&self, cmd: &str) -> Option<Error> {
        let send = self.send.as_ref()?;
        REFUSED_WHILE_TYPING.contains(&cmd).then(|| {
            if send.transfer {
                "a transfer is in progress (releaseAll stops it)".into()
            } else {
                "typing is in progress (releaseAll stops it)".into()
            }
        })
    }

    /// One protocol message. `bytes` are its *bytes* field when the host
    /// has them apart from the JSON (the Worker's `rom` and `state`);
    /// without them a `state` field is base64. With a `reply` tag the
    /// reply comes out as [`Output::Reply`] after the events the command
    /// caused (a send's when it is done); without one an error comes out
    /// as an `error` event.
    pub fn command(
        &mut self,
        clock: &dyn Clock,
        msg: &Value,
        bytes: Option<Vec<u8>>,
        reply: Option<u64>,
    ) {
        let answer = self
            .admit(msg)
            .and_then(|cmd| self.handle(clock, cmd, msg, bytes, reply));
        match answer {
            Ok(Answer::Later) => self.flush(clock, true),
            Ok(Answer::Now(v, b)) => self.respond(clock, reply, Ok(v), b),
            Err(e) => self.respond(clock, reply, Err(e), None),
        }
    }

    /// Answer a command the host served itself: the events so far, then
    /// the reply (or, without a `reply` tag, an `error` event for an
    /// error).
    pub fn answer(&mut self, clock: &dyn Clock, reply: Option<u64>, result: Result<Value>) {
        self.respond(clock, reply, result, None);
    }

    fn respond(
        &mut self,
        clock: &dyn Clock,
        reply: Option<u64>,
        result: Result<Value>,
        bytes: Option<(&'static str, Vec<u8>)>,
    ) {
        self.flush(clock, true);
        self.reply_to(reply, result, bytes);
    }

    fn reply_to(
        &mut self,
        reply: Option<u64>,
        result: Result<Value>,
        bytes: Option<(&'static str, Vec<u8>)>,
    ) {
        match (reply, result) {
            (Some(tag), result) => self.out.push(Output::Reply(Reply {
                tag,
                result: result.map_err(|e| e.to_string()),
                bytes,
            })),
            (None, Err(e)) => self.event(Event::Error(ErrorEvent {
                message: e.to_string(),
            })),
            (None, Ok(_)) => {}
        }
    }

    fn event(&mut self, e: Event) {
        self.out.push(Output::Event(e));
    }

    /// The replies and events so far, in order.
    pub fn take_output(&mut self) -> Vec<Output> {
        std::mem::take(&mut self.out)
    }

    /// When [`Engine::timer`] is due on the host's clock; `None` while
    /// only a message can change anything (paused, idle with nothing to
    /// watch).
    pub fn deadline(&self) -> Option<f64> {
        [
            self.send.as_ref().map(|s| s.due),
            self.lp.due(),
            self.watch.due,
            // A busy machine's own timers bring it to the save.
            self.settled().then(|| self.autosave.due()).flatten(),
        ]
        .into_iter()
        .flatten()
        .reduce(f64::min)
    }

    /// The host's timer fired: run what is due (a send's turn, a pass, a
    /// wake, a look at the memory).
    pub fn timer(&mut self, clock: &dyn Clock) {
        let now = clock.now_ms();
        let slack = self.lp.pacing.timer_slack_ms;
        if self.send.as_ref().is_some_and(|s| s.due <= now + slack) {
            self.typing_turn(clock);
        }
        if let Some(e) = self.emu.as_mut()
            && let Err(err) = self.lp.fire(e, clock, now)
        {
            self.halt(clock, err);
        }
        if self.watch.due.is_some_and(|d| d <= now + slack) {
            self.watch.due = None;
        }
        // Between passes a frame waits for the throttle; otherwise it
        // goes now.
        self.flush(clock, !self.lp.passing());
    }

    /// Bytes arrived for the machine from outside (the serial bridge): a
    /// sleeping CPU first runs the time that passed, then sees when the
    /// UART wakes it.
    pub fn input(&mut self, clock: &dyn Clock) {
        self.autosave.touch(clock.now_ms());
        if self.lp.sleeping() {
            self.wake(clock);
            self.flush(clock, !self.lp.passing());
        }
    }

    /// The tag of the send in progress, if it has one.
    pub fn sending(&self) -> Option<u64> {
        self.send.as_ref().and_then(|s| s.tag)
    }

    /// Stop the send in progress; its reply is the error `why`.
    pub fn cancel_send(&mut self, clock: &dyn Clock, why: &str) {
        if self.send.is_some() {
            self.end_typing(clock, Some(why.into()));
        }
    }

    /// What `status` says now.
    pub fn status(&self) -> Status {
        Status {
            model: self.model.map(Model::name),
            rom_name: self.rom_name.clone(),
            running: self.lp.running,
            halted: self.halted.clone(),
            speed: self.lp.speed,
            loop_state: self.lp.state(),
            busy: self.send.as_ref().is_some_and(|s| s.freezes),
        }
    }

    /// The machine, for what a host does beside the protocol (the serial
    /// bridge); `None` before a boot.
    pub fn emulator_mut(&mut self) -> Option<&mut Emulator> {
        self.emu.as_mut()
    }

    /// The machine, for the commands a host serves itself.
    pub fn emulator(&self) -> Result<&Emulator> {
        self.emu.as_ref().ok_or_else(|| "no ROM loaded".into())
    }

    /// The machine, once the host is done with the engine.
    pub fn into_emulator(self) -> Option<Emulator> {
        self.emu
    }

    /// Start running `emu`, a machine the host built (and perhaps ran)
    /// from the ROM file `rom_name`.
    pub fn start(&mut self, clock: &dyn Clock, emu: Emulator, rom_name: &str) {
        self.model = Some(emu.model());
        self.emu = Some(emu);
        self.rom_name = rom_name.to_string();
        self.halted = None;
        self.watch.force = true;
        self.autosave.clear();
        self.set_running(clock, true);
        self.flush(clock, true);
    }

    /// Write `nibbles` from `address` on as CPU writes would (`poke`):
    /// the display may show it, the user memory may have changed, a
    /// sleeping CPU may see it.
    pub fn poke(&mut self, clock: &dyn Clock, address: u32, nibbles: &[u8]) -> Result<()> {
        if let Some(e) = self.refusal("poke") {
            return Err(e);
        }
        let e = self.emu.as_mut().ok_or("no ROM loaded")?;
        for (a, &n) in (address..).zip(nibbles) {
            e.machine_mut().poke(a, n);
        }
        e.reshow();
        self.watch.force = true;
        if self.send.is_none() {
            self.lp.schedule(&*e, clock.now_ms());
        }
        self.autosave.touch(clock.now_ms());
        Ok(())
    }

    /// Run `f` on the machine outside the pacing, as fast as the host can
    /// (a native key script), with every key released first; then follow
    /// the wall clock again from where it left the machine. `f` gets the
    /// machine and gives it back with its result. Refused during a send,
    /// as `keyScript`.
    pub fn exclusive<T>(
        &mut self,
        clock: &dyn Clock,
        f: impl FnOnce(Machine) -> (Machine, std::result::Result<T, RunError>),
    ) -> Result<T> {
        if let Some(e) = self.refusal("keyScript") {
            return Err(e);
        }
        if let Some(h) = &self.halted {
            return Err(format!("the CPU is halted: {h}").into());
        }
        // A sleeping machine first catches up the time that passed.
        self.wake(clock);
        let mut emu = self.emu.take().ok_or("no ROM loaded")?;
        emu.release_keys();
        let start = clock.now_ms();
        let (machine, result) = f(emu.into_machine());
        self.lp.work_ms += clock.now_ms() - start;
        self.emu = Some(Emulator::from_machine(machine));
        self.autosave.touch(clock.now_ms());
        match result {
            Ok(t) => {
                self.set_running(clock, self.lp.running);
                Ok(t)
            }
            Err(e) => {
                if e.halted {
                    self.halted = Some(e.message.clone());
                    self.set_running(clock, false);
                } else {
                    self.set_running(clock, self.lp.running);
                }
                Err(e.message.into())
            }
        }
    }

    /// Build `model` (a preference: a ROM that only fits another model
    /// boots that one) from `rom` and run it; the machine before keeps
    /// running if this one cannot be built.
    pub fn boot(
        &mut self,
        clock: &dyn Clock,
        model: &str,
        rom: &[u8],
        rom_name: &str,
    ) -> Result<Booted> {
        self.boot_restoring(clock, model, rom, rom_name, None)
    }

    /// [`Engine::boot`], then restore `kept`, the state the host kept for
    /// this model, before the machine runs a cycle: the calculator is as
    /// it was, with no "Try To Recover Memory?". A state that does not
    /// load (another ROM, another model, an older format) leaves the cold
    /// boot, and `restore_error` says why. The machine it replaces is not
    /// saved here: the host calls [`Engine::save_now`] and stores that
    /// save before it reads or clears the slot, so the slot it reads is
    /// the newest and a fresh start's stays empty. An owed save left now
    /// is dropped.
    pub fn boot_restoring(
        &mut self,
        clock: &dyn Clock,
        model: &str,
        rom: &[u8],
        rom_name: &str,
        kept: Option<&[u8]>,
    ) -> Result<Booted> {
        if let Some(e) = self.refusal("boot") {
            return Err(e);
        }
        let model = model_for_rom(rom, model.parse()?);
        let mut emu = Emulator::new(model, rom)?;
        let mut restored = false;
        let mut restore_error = None;
        if let Some(state) = kept {
            if state.len() > MAX_STATE_BYTES {
                restore_error = Some(format!("state is larger than {MAX_STATE_BYTES} bytes"));
            } else {
                match emu.load_state(state) {
                    Ok(()) => restored = true,
                    Err(e) => restore_error = Some(e.to_string()),
                }
            }
        }
        self.emu = Some(emu);
        self.model = Some(model);
        self.rom_name = rom_name.to_string();
        self.halted = None;
        self.watch.force = true;
        self.autosave.clear();
        self.set_running(clock, true);
        self.flush(clock, true);
        Ok(Booted {
            model: model.name(),
            rom_name: self.rom_name.clone(),
            restored,
            restore_error,
        })
    }

    /// The whole machine state (`saveState`).
    pub fn save_state(&self) -> Result<Vec<u8>> {
        if let Some(e) = self.refusal("saveState") {
            return Err(e);
        }
        Ok(self.emulator()?.save_state())
    }

    /// Restore a saved state of the same model and ROM (`loadState`);
    /// releases the keys.
    pub fn load_state(&mut self, clock: &dyn Clock, state: &[u8]) -> Result<()> {
        if let Some(e) = self.refusal("loadState") {
            return Err(e);
        }
        if state.len() > MAX_STATE_BYTES {
            return Err(format!("state is larger than {MAX_STATE_BYTES} bytes").into());
        }
        let e = self.emu.as_mut().ok_or("no ROM loaded")?;
        e.release_keys();
        e.load_state(state)?;
        e.reshow();
        self.halted = None;
        self.watch.force = true;
        self.autosave.touch(clock.now_ms());
        self.set_running(clock, self.lp.running);
        self.flush(clock, true);
        Ok(())
    }

    /// Stats now.
    pub fn stats(&self, clock: &dyn Clock) -> Stats {
        let now = clock.now_ms();
        Stats {
            cycles: self.emu.as_ref().map_or(0, |e| e.machine().cycles()),
            emulated_ms: self.emu.as_ref().map_or(0.0, Emulator::emulated_ms),
            work_ms: self.lp.work_ms,
            ticks: self.lp.ticks,
            wakes: self.lp.wakes,
            memory_looks: self.watch.looks,
            memory_ms: self.watch.spent_ms,
            loop_state: self.lp.state(),
            owed_ms: self.lp.owed_ms(now),
            now_ms: now,
        }
    }

    fn emu(&mut self) -> Result<&mut Emulator> {
        self.emu.as_mut().ok_or_else(|| "no ROM loaded".into())
    }

    /// One admitted command.
    fn handle(
        &mut self,
        clock: &dyn Clock,
        cmd: &str,
        msg: &Value,
        bytes: Option<Vec<u8>>,
        reply: Option<u64>,
    ) -> Result<Answer> {
        Ok(match cmd {
            "hello" => {
                // A (re)loaded page starts from nothing: the status, the
                // keys down and the current frame go out again, before
                // this reply. The page fetches the model's skin itself.
                self.last_status = None;
                self.watch.on = false;
                if let Some(e) = self.emu.as_mut() {
                    e.reshow();
                }
                let models: Vec<&str> = Model::ALL.iter().map(|m| m.name()).collect();
                json!({"protocol": VERSION, "host": self.host, "models": models}).into()
            }
            "skin" => {
                let m: Model = str_field(msg, "model")?.parse()?;
                value_of(&crate::skins::skin_view(m))?.into()
            }
            "layout" => {
                let m: Model = str_field(msg, "model")?.parse()?;
                value_of(&crate::layout::grid(m))?.into()
            }
            "boot" => {
                let model = str_field(msg, "model")?;
                let rom = bytes.ok_or("missing bytes field \"rom\"")?;
                let name = msg.get("romName").and_then(Value::as_str).unwrap_or("");
                value_of(&self.boot(clock, model, &rom, name)?)?.into()
            }
            "keyDown" => {
                let key = str_field(msg, "key")?;
                let shift = msg.get("shift").and_then(Value::as_str);
                let e = self.emu()?;
                let queued = match shift {
                    None => e.press(key),
                    Some(shift) => {
                        if !e.has_key(shift) {
                            return Err(unknown_key(e, shift));
                        }
                        e.press_shifted(key, shift)
                    }
                };
                if !queued {
                    if let Some(shift) = shift.filter(|_| e.has_key(key)) {
                        return Err(format!("{shift:?} is not a shift key").into());
                    }
                    return Err(unknown_key(e, key));
                }
                self.autosave.touch(clock.now_ms());
                self.after_keys(clock);
                Value::Null.into()
            }
            "keyUp" => {
                let key = str_field(msg, "key")?;
                let e = self.emu()?;
                if !e.has_key(key) {
                    return Err(unknown_key(e, key));
                }
                e.release(key);
                e.pump();
                self.autosave.touch(clock.now_ms());
                Value::Null.into()
            }
            "keyUpAll" => {
                // The window lost the focus during a send: the send holds
                // no queued key, and its own key comes up when its press
                // ends.
                if self.send.is_none()
                    && let Some(e) = self.emu.as_mut()
                {
                    e.release_held();
                    e.pump();
                }
                Value::Null.into()
            }
            "typeLetter" => {
                let letter = str_field(msg, "letter")?;
                let Some(e) = self.emu.as_mut() else {
                    return Ok(Value::Bool(false).into());
                };
                let ok = e.type_letter(letter);
                self.autosave.touch(clock.now_ms());
                self.after_keys(clock);
                Value::Bool(ok).into()
            }
            "typeKeys" => {
                let keys: Vec<&str> = msg
                    .get("keys")
                    .and_then(Value::as_array)
                    .ok_or("missing array field \"keys\"")?
                    .iter()
                    .map(Value::as_str)
                    .collect::<Option<_>>()
                    .ok_or("\"keys\" must hold key names (strings)")?;
                let e = self.emu()?;
                // All or nothing: one unknown name refuses the sequence.
                if let Some(k) = keys.iter().find(|k| !e.has_key(k)) {
                    return Err(unknown_key(e, k));
                }
                e.queue().type_keys(&keys);
                self.autosave.touch(clock.now_ms());
                self.after_keys(clock);
                Value::Null.into()
            }
            "releaseAll" => {
                // Also stops a send in progress (its reply is an error).
                self.cancel_send(clock, "cancelled");
                if let Some(e) = self.emu.as_mut() {
                    e.release_keys();
                }
                Value::Null.into()
            }
            "setSpeed" => {
                // Asleep, emulated time runs at 1x at any speed: only the
                // passes change.
                self.lp.speed = Speed::parse(str_field(msg, "speed")?);
                Value::Null.into()
            }
            "pause" => {
                // Anything but `paused: false` pauses.
                let paused = msg.get("paused").and_then(Value::as_bool).unwrap_or(true);
                self.set_running(clock, !paused);
                Value::Null.into()
            }
            "reset" => {
                let e = self.emu()?;
                e.release_keys();
                e.reset();
                self.halted = None;
                self.autosave.touch(clock.now_ms());
                self.set_running(clock, true);
                Value::Null.into()
            }
            "saveState" => {
                let state = self.save_state()?;
                let cycles = self.emulator()?.machine().cycles();
                Answer::Now(json!({"cycles": cycles}), Some(("state", state)))
            }
            "loadState" => {
                let state = match bytes {
                    Some(b) => b,
                    None => {
                        let b64 = str_field(msg, "state")?;
                        if b64.len() > MAX_STATE_BYTES.div_ceil(3) * 4 {
                            return Err(
                                format!("state is larger than {MAX_STATE_BYTES} bytes").into()
                            );
                        }
                        base64_decode(b64).map_err(|e| format!("state: {e}"))?
                    }
                };
                self.load_state(clock, &state)?;
                json!({}).into()
            }
            "visibility" => {
                let hidden = msg.get("hidden").and_then(Value::as_bool).unwrap_or(false);
                // The last moment a phone's page may run: an unsaved
                // change is saved now, or once a send, a write or a
                // computation has settled. In the browser a computation
                // stops while hidden, so its save waits until the page is
                // shown again.
                if hidden {
                    self.autosave.hide();
                }
                let idle = self.send.is_none();
                self.lp
                    .set_hidden(hidden, self.emu.as_ref(), idle, clock.now_ms());
                Value::Null.into()
            }
            "stats" => value_of(&self.stats(clock))?.into(),
            "watchMemory" => {
                let on = msg.get("on").and_then(Value::as_bool).unwrap_or(false);
                self.watch.subscribe(on, self.emu.as_ref());
                let Some(e) = &self.emu else {
                    return Ok(json!({"supported": null, "reason": null}).into());
                };
                let reason = e.memory_refusal();
                json!({"supported": reason.is_none(), "reason": reason}).into()
            }
            "memoryTree" => value_of(&self.emu()?.memory_tree()?)?.into(),
            "stack" => Value::Array(self.emu()?.stack()?).into(),
            "flags" => value_of(&self.emu()?.flags()?)?.into(),
            "objectAt" => {
                let address = msg
                    .get("address")
                    .and_then(Value::as_u64)
                    .ok_or("missing number field \"address\"")?;
                let address = u32::try_from(address)
                    .ok()
                    .filter(|&a| a <= 0xF_FFFF)
                    .ok_or("address is outside the address space")?;
                self.emu()?.object_at(address)?.into()
            }
            "commandLine" => value_of(&self.emu()?.command_line()?)?.into(),
            "editText" => {
                let (dir, target) = self.text_target(msg)?;
                let (text, was) = self.emu()?.edit_text(&dir, &target)?;
                json!({"text": text, "was": was}).into()
            }
            "insert" | "typeText" => self.start_typing(clock, "insert", msg, reply)?,
            "run" => self.start_typing(clock, "run", msg, reply)?,
            c if WRITE_COMMANDS.contains(&c) => {
                let op = self.write_op(c, msg, bytes)?;
                self.start_transfer(clock, op, reply)?
            }
            "replace" => self.start_typing(clock, "replace", msg, reply)?,
            other => return Err(format!("unknown command {other:?}").into()),
        })
    }

    /// Commands queued keys: wake a sleeping machine (its time first, then
    /// the keys; the wake starts passes for the queue), then feed the
    /// queue.
    fn after_keys(&mut self, clock: &dyn Clock) {
        self.wake(clock);
        if let Some(e) = self.emu.as_mut() {
            e.pump();
        }
    }

    /// Leave a sleep now (a halt on the way stops the machine).
    fn wake(&mut self, clock: &dyn Clock) {
        if let Some(e) = self.emu.as_mut()
            && let Err(err) = self.lp.wake(e, clock)
        {
            self.halt(clock, err);
        }
    }

    fn halt(&mut self, clock: &dyn Clock, err: Error) {
        self.halted = Some(err.to_string());
        self.set_running(clock, false);
    }

    /// The Run/Pause switch, and the restart after anything that changed
    /// the machine under the loop.
    fn set_running(&mut self, clock: &dyn Clock, on: bool) {
        self.lp.running = on && self.emu.is_some() && self.halted.is_none();
        self.lp.stop();
        // A send drives the machine itself; the loop resumes when it ends.
        if self.send.is_some() {
            return;
        }
        if let Some(e) = &self.emu {
            self.lp.start(e, clock.now_ms());
        }
    }

    /// Start a send of `msg`'s `text` with `verb`: the loop stops and the
    /// send runs in turns ([`Engine::timer`]) until it is done.
    fn start_typing(
        &mut self,
        clock: &dyn Clock,
        verb: &str,
        msg: &Value,
        reply: Option<u64>,
    ) -> Result<Answer> {
        let text = str_field(msg, "text")?;
        self.emu()?;
        if let Some(h) = &self.halted {
            return Err(format!("the CPU is halted: {h}").into());
        }
        // A sleeping machine first catches up the time that passed.
        self.wake(clock);
        let freezes = self.emu()?.start_typing(verb, text)?;
        self.lp.stop();
        let now = clock.now_ms();
        self.autosave.touch(now);
        self.send = Some(Send {
            tag: reply,
            freezes,
            transfer: false,
            started: now,
            due: now,
        });
        Ok(Answer::Later)
    }

    /// The write a command asks for: `dir` (the current directory when
    /// absent), `name`, `to`, `flag`, `on`, and `storeFile`'s file
    /// (`bytes`, else the base64 field `data`).
    fn write_op(&mut self, cmd: &str, msg: &Value, bytes: Option<Vec<u8>>) -> Result<Op> {
        let given = match msg.get("dir") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_array()
                    .and_then(|a| {
                        a.iter()
                            .map(|s| s.as_str().map(str::to_string))
                            .collect::<Option<Vec<_>>>()
                    })
                    .ok_or("\"dir\" must be an array of directory names")?,
            ),
        };
        let name = || str_field(msg, "name").map(str::to_string);
        let dir = Vec::new();
        if cmd == "storeText" {
            let text = str_field(msg, "text")?.to_string();
            let (dir, target) = self.text_target(msg)?;
            // What the editor opened must still be there: a save never
            // replaces what it did not show. Compared by the object
            // (`editText`'s `was`: size and checksum), not its text, which
            // follows the display mode.
            if let Some(was) = msg.get("was").and_then(Value::as_str) {
                self.emu()?.check_unchanged(&dir, &target, was)?;
            }
            return Ok(Op::StoreText { dir, target, text });
        }
        let mut op = match cmd {
            "storeFile" => {
                let data = match bytes {
                    Some(b) => b,
                    None => {
                        let b64 = str_field(msg, "data")?;
                        if b64.len() > MAX_FILE_BYTES.div_ceil(3) * 4 {
                            return Err(
                                format!("the file is larger than {MAX_FILE_BYTES} bytes").into()
                            );
                        }
                        base64_decode(b64).map_err(|e| format!("data: {e}"))?
                    }
                };
                Op::Store {
                    dir,
                    name: name()?,
                    data,
                }
            }
            "fetchFile" => Op::Fetch { dir, name: name()? },
            "purge" => Op::Purge { dir, name: name()? },
            "rename" => Op::Rename {
                dir,
                name: name()?,
                to: str_field(msg, "to")?.to_string(),
            },
            "createDir" => Op::CreateDir { dir, name: name()? },
            "changeDir" if given.is_none() => {
                return Err("missing array field \"dir\"".into());
            }
            "changeDir" => Op::ChangeDir { dir },
            _ => {
                let flag = msg
                    .get("flag")
                    .and_then(Value::as_i64)
                    .and_then(|f| i32::try_from(f).ok())
                    .ok_or("missing number field \"flag\"")?;
                let on = msg
                    .get("on")
                    .and_then(Value::as_bool)
                    .ok_or("missing boolean field \"on\"")?;
                Op::SetFlag { flag, on }
            }
        };
        // The fields first, then the calculator.
        if let Op::Store { dir, .. }
        | Op::Fetch { dir, .. }
        | Op::Purge { dir, .. }
        | Op::Rename { dir, .. }
        | Op::CreateDir { dir, .. }
        | Op::ChangeDir { dir } = &mut op
        {
            *dir = match given {
                Some(d) => d,
                None => self.emu()?.memory_tree()?.path,
            };
        }
        Ok(op)
    }

    /// The directory and the target of `editText` and `storeText`: `dir`
    /// (the current directory when absent) and `name`, or `level`.
    fn text_target(&mut self, msg: &Value) -> Result<(Vec<String>, Target)> {
        let target = match (msg.get("name"), msg.get("level")) {
            (Some(_), None) => Target::Variable(str_field(msg, "name")?.to_string()),
            (None, Some(l)) => Target::Level(
                l.as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|&n| n >= 1)
                    .ok_or("\"level\" must be a stack level (1 or more)")?,
            ),
            _ => return Err("give either \"name\" or \"level\"".into()),
        };
        let dir = match msg.get("dir") {
            None | Some(Value::Null) => self.emu()?.memory_tree()?.path,
            Some(v) => v
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .map(|s| s.as_str().map(str::to_string))
                        .collect::<Option<Vec<_>>>()
                })
                .ok_or("\"dir\" must be an array of directory names")?,
        };
        Ok((dir, target))
    }

    /// Start the write `op`: the loop stops and it runs in turns, the
    /// screen held, until it is done.
    fn start_transfer(&mut self, clock: &dyn Clock, op: Op, reply: Option<u64>) -> Result<Answer> {
        self.emu()?;
        if let Some(h) = &self.halted {
            return Err(format!("the CPU is halted: {h}").into());
        }
        self.wake(clock);
        self.emu()?.start_transfer(op)?;
        self.lp.stop();
        let now = clock.now_ms();
        self.autosave.touch(now);
        self.send = Some(Send {
            tag: reply,
            freezes: true,
            transfer: true,
            started: now,
            due: now,
        });
        Ok(Answer::Later)
    }

    /// One turn of the send: steps until it is done or the turn's wall
    /// time is used up.
    fn typing_turn(&mut self, clock: &dyn Clock) {
        let (Some(send), Some(e)) = (self.send.as_ref(), self.emu.as_mut()) else {
            return;
        };
        let started = send.started;
        let transfer = send.transfer;
        let start = clock.now_ms();
        let mut result = Ok(false);
        while matches!(result, Ok(false)) && clock.now_ms() - start < self.lp.pacing.typing_tick_ms
        {
            result = if transfer {
                e.transfer_step(TYPING_STEP_MS)
            } else {
                e.typing_step(TYPING_STEP_MS)
            };
        }
        let now = clock.now_ms();
        self.lp.work_ms += now - start;
        match result {
            Ok(false) if now - started > WALL_LIMIT_MS => self.end_typing(
                clock,
                Some(
                    format!(
                        "{} ran out of wall-clock time ({} s)",
                        if transfer { "the transfer" } else { "typing" },
                        WALL_LIMIT_MS / 1000.0
                    )
                    .into(),
                ),
            ),
            Ok(false) => {
                if let Some(s) = self.send.as_mut() {
                    s.due = now;
                }
                self.flush(clock, true);
            }
            Ok(true) => self.end_typing(clock, None),
            Err(err) => self.end_typing(clock, Some(err)),
        }
    }

    /// Finish the send: its result, or `error` (the send is stopped).
    fn end_typing(&mut self, clock: &dyn Clock, error: Option<Error>) {
        let Some(send) = self.send.take() else {
            return;
        };
        // The delay counts from its end.
        self.autosave.touch(clock.now_ms());
        let mut bytes = None;
        let result = match (error, self.emu.as_mut()) {
            (_, None) => Err("no ROM loaded".into()),
            (None, Some(e)) if send.transfer => e.transfer_result().and_then(|(r, file)| {
                bytes = file.map(|f| ("data", f));
                value_of(&r)
            }),
            (None, Some(e)) => e.typing_result().and_then(|r| value_of(&r)),
            (Some(err), Some(e)) => {
                e.stop_typing();
                e.stop_transfer();
                Err(err)
            }
        };
        if send.transfer {
            // What the server drew is gone; the memory changed.
            if let Some(e) = self.emu.as_mut() {
                e.reshow();
            }
            self.watch.force = true;
        }
        // The status says the screen is live again before its frame.
        if send.freezes {
            self.send_status();
        }
        match &result {
            Err(e @ Error::Halted(_)) => {
                let e = e.clone();
                self.halt(clock, e);
            }
            _ => self.set_running(clock, self.lp.running),
        }
        self.respond(clock, send.tag, result, bytes);
    }

    /// Send the status if it changed, the errors, the keys and a frame if
    /// they changed (none of these while a long send holds them; a frame
    /// between passes only every [`Pacing::frame_ms`] unless `now`), and
    /// look at the user memory if that is due.
    fn flush(&mut self, clock: &dyn Clock, now: bool) {
        let frozen = self.send.as_ref().is_some_and(|s| s.freezes);
        if !frozen && let Some(e) = self.emu.as_mut() {
            let errors = e.take_errors();
            let keys = e.keys_if_changed();
            let t = clock.now_ms();
            let due = now
                || self
                    .last_frame
                    .is_none_or(|f| t - f >= self.lp.pacing.frame_ms);
            let frame = if due { e.frame_if_changed() } else { None };
            for err in errors {
                self.event(Event::Error(ErrorEvent {
                    message: err.to_string(),
                }));
            }
            if let Some(k) = keys {
                self.event(Event::Keys(k));
            }
            if let Some(f) = frame {
                self.last_frame = Some(t);
                self.event(Event::Frame(f));
            }
        }
        self.send_status();
        self.poll_memory(clock);
        self.poll_save(clock);
    }

    /// The machine is in a state worth keeping: no send or write in
    /// progress, the CPU asleep with no key down or queued, not halted.
    fn settled(&self) -> bool {
        self.send.is_none()
            && self.halted.is_none()
            && self.emu.as_ref().is_some_and(pacing::asleep)
    }

    /// Hand the state to the host if a save is due and the machine has
    /// settled (`protocol/autosave.rs`).
    fn poll_save(&mut self, clock: &dyn Clock) {
        let settled = self.settled();
        if self
            .autosave
            .poll(clock.now_ms(), self.lp.pacing.timer_slack_ms, settled)
        {
            self.emit_save();
        }
    }

    fn emit_save(&mut self) {
        let Some(e) = self.emu.as_ref() else {
            return;
        };
        let saved = Saved {
            model: e.model().name(),
            rom_name: self.rom_name.clone(),
            cycles: e.machine().cycles(),
            state: e.save_state(),
        };
        self.autosave.clear();
        self.autosave.saves += 1;
        self.out.push(Output::Save(saved));
    }

    fn send_status(&mut self) {
        let status = self.status();
        if self.last_status.as_ref() != Some(&status) {
            self.last_status = Some(status.clone());
            self.event(Event::Status(status));
        }
    }

    /// Tell a watching page that the user memory changed, if a look is
    /// due and shows it (see [`watch`]): not while the machine computes,
    /// nor while a send types.
    fn poll_memory(&mut self, clock: &dyn Clock) {
        let quiet = self.send.is_none()
            && self
                .emu
                .as_ref()
                .is_none_or(|e| !self.lp.running || pacing::asleep(e));
        if self.watch.poll(self.emu.as_ref(), quiet, clock) {
            self.event(Event::MemoryChanged(MemoryChanged {}));
        }
    }
}
