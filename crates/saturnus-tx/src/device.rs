//! One calculator on one link, without I/O: a [`Device`] is fed bytes, the
//! time and link errors, and hands out [`Action`]s, [`Event`]s and the
//! [`Outcome`] of each [`Request`].
//!
//! The calling convention is `satx-core`'s
//! [`Machine`], one level up again: a request is a
//! sequence of the machine's operations with decisions between them, and
//! the device may also ask the host to reopen the port with other
//! settings (the speed and parity search).
//!
//! After every [`Device::start`] and `handle_*` call the host takes every
//! [`Action`] in order (an [`Action::Open`] reopens the port before the
//! writes that follow it; input read before the reopen is not fed any
//! more), the events and the result, and arms one timer for
//! [`Device::next_timeout`].
//!
//! # Connect
//!
//! [`Request::Connect`] with `find` (the default) opens the port at 9600
//! baud, 8 data bits, no parity, sends a Kermit I packet, which the server
//! only ACKs, framed for no, odd and even parity [`PROBE_GAP`] apart, and
//! listens for [`DeviceOptions::listen`] (6 s, about one idle-NAK period;
//! the three probes again halfway) for that ACK or an idle server's NAK.
//! Nothing heard: 4800, 2400 and 1200 are tried the same way. The packet heard also
//! tells the parity (see [`hear`]): odd or even, the port is reopened with
//! it. Then the sync, `VERSION` for the
//! model, and flag -35 read and set (binary) for the session; disconnect
//! puts flag -35 back and ends server mode (FINISH).
//!
//! # What changes the calculator
//!
//! Nothing is replaced or removed unless the request says so: a put to a
//! name in use is [`TxError::NameClash`] (or [`TxError::NamesInUse`] for a
//! tree) until it is sent again with `replace`, which puts into an unused
//! name first and swaps it in once stored; a directory that is not empty
//! is purged only by a [`Request::Purge`] that names how many variables go
//! with it ([`TxError::NotEmpty`] tells the count); [`Request::Restore`] is
//! its own request. The host asks the user before each of these.
//!
//! After a [`Request::Restore`] the calculator has warm-started and left
//! server mode: the device is disconnected and [`Device::restarted`] says
//! so; every request but [`Request::Connect`] is [`TxError::Restarted`]
//! until `SERVER` runs on the calculator again and the host reconnects.
//!
//! [`hear`]: crate::hear

use std::collections::VecDeque;
use std::fmt;
use std::future::{Future, poll_fn};
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

use satx_core::calc::validate_name;
use satx_core::machine::{Busy, Machine, Op, Progress, Reply};
pub use satx_core::reply::Entry;
use satx_core::reply::Listing;
use satx_core::time::{Duration, Instant};
use satx_core::{Model, Options, TransferMode};

use crate::names::{self, FileKind};
use crate::ops;
use crate::search::{hear, probe};

/// The speeds the search tries, in order (IOPAR's four, wiki:
/// protocols/iopar).
pub const SPEEDS: [u32; 4] = [9600, 4800, 2400, 1200];

/// The temporary variable a replacing put goes to first, with a number
/// that makes it unused (`SATXPUT1`).
const REPLACE_VAR: &str = "SATXPUT";

/// Time between the probe's three framings (no, odd, even parity): the
/// answer to one, about 30 bytes both ways, takes under 300 ms at 1200
/// baud.
pub const PROBE_GAP: Duration = Duration::from_millis(500);

/// Parity of the serial link. With parity the HP's frame keeps 8 bits, the
/// parity in bit 7, so a host port opens with 7 data bits and this parity
/// (Web Serial: `dataBits: 7`), and Kermit asks for 8th-bit prefixing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Parity {
    /// No parity, 8 data bits (the calculator's default).
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
}

impl Parity {
    /// `none`, `odd` or `even`.
    pub fn name(self) -> &'static str {
        match self {
            Parity::None => "none",
            Parity::Odd => "odd",
            Parity::Even => "even",
        }
    }

    /// The parity named by [`Parity::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        [Parity::None, Parity::Odd, Parity::Even]
            .into_iter()
            .find(|p| p.name() == name)
    }
}

/// How the host opens the port: always one stop bit and no flow control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LinkSettings {
    /// Bits per second: 1200, 2400, 4800 or 9600.
    pub baud: u32,
    /// The parity.
    pub parity: Parity,
}

impl LinkSettings {
    /// 9600 baud, 8 data bits, no parity: every calculator's default.
    pub const DEFAULT: LinkSettings = LinkSettings {
        baud: 9600,
        parity: Parity::None,
    };

    /// Data bits per character on the host's side: 8, or 7 with parity.
    pub fn data_bits(self) -> u8 {
        if self.parity == Parity::None { 8 } else { 7 }
    }
}

impl Default for LinkSettings {
    fn default() -> Self {
        LinkSettings::DEFAULT
    }
}

impl fmt::Display for LinkSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.parity {
            Parity::None => write!(f, "{} baud", self.baud),
            parity => write!(f, "{} baud, {} parity", self.baud, parity.name()),
        }
    }
}

/// Tunables of a [`Device`].
#[derive(Clone, Debug)]
pub struct DeviceOptions {
    /// The Kermit session's options (timeouts, retries, turnaround).
    pub session: Options,
    /// How long the search listens at each speed for an answer to its
    /// probes, which it sends at the start and halfway.
    pub listen: Duration,
}

impl Default for DeviceOptions {
    fn default() -> Self {
        DeviceOptions {
            session: Options::default(),
            listen: Duration::from_secs(6),
        }
    }
}

/// How [`Request::Connect`] finds the calculator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectOptions {
    /// Search the speed and the parity (the default); off, connect with
    /// `settings` only.
    pub find: bool,
    /// The settings when `find` is off.
    pub settings: LinkSettings,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        ConnectOptions {
            find: true,
            settings: LinkSettings::DEFAULT,
        }
    }
}

/// How a variable is got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// An exact copy: a binary transfer file (`NAME.hp`).
    Binary,
    /// The calculator's text form (`NAME.txt`, flag -35 clear for the one
    /// transfer).
    Text,
}

/// What a host asks a [`Device`] to do. Directories are absolute paths,
/// `["HOME", "D1"]`; an empty path or one without the leading `HOME` is
/// taken from HOME.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Request {
    /// Find the calculator, sync, read the model, set binary transfers.
    /// [`Outcome::Connected`].
    Connect(ConnectOptions),
    /// The variables of a directory. [`Outcome::Listing`].
    List {
        /// The directory.
        dir: Vec<String>,
    },
    /// Get a variable. [`Outcome::File`].
    Get {
        /// The directory.
        dir: Vec<String>,
        /// The variable name.
        name: String,
        /// Binary or text.
        format: Format,
    },
    /// Put a file into a variable. [`Outcome::Stored`].
    Put {
        /// The directory.
        dir: Vec<String>,
        /// The variable name (see [`names::var_name`]).
        name: String,
        /// The file: binary, `%%HP:` text, or anything else as a string.
        data: Vec<u8>,
        /// Replace a variable of that name ([`TxError::NameClash`]
        /// otherwise); never a directory. The file goes to an unused name
        /// first and replaces the old variable only once it is stored.
        replace: bool,
        /// Store a file that is neither binary nor text as a string
        /// ([`TxError::NotAnObject`] otherwise).
        as_string: bool,
    },
    /// Put flag -35 back as it was and end server mode.
    /// [`Outcome::Disconnected`].
    Disconnect,
    /// Make a directory. [`Outcome::Created`]; [`TxError::NameClash`] for
    /// a name in use.
    Mkdir {
        /// The directory to make it in.
        dir: Vec<String>,
        /// The new directory's name.
        name: String,
    },
    /// Rename a variable or a directory. [`Outcome::Renamed`];
    /// [`TxError::NameClash`] when `to` is in use.
    Rename {
        /// The directory.
        dir: Vec<String>,
        /// The existing name.
        from: String,
        /// The new name.
        to: String,
    },
    /// Purge a variable, or a directory with everything in it (`PGDIR`).
    /// [`Outcome::Purged`]. A directory that holds variables is purged
    /// only when `count` is the number of variables in it, at every level
    /// ([`TxError::NotEmpty`] tells it).
    Purge {
        /// The directory.
        dir: Vec<String>,
        /// The variable or directory.
        name: String,
        /// For a directory: how many variables the user agreed go with it.
        count: Option<usize>,
    },
    /// Get a directory and everything in it, one binary get per variable.
    /// `["HOME"]` copies the whole calculator. [`Outcome::Tree`].
    GetTree {
        /// The directory.
        dir: Vec<String>,
        /// Binary (exact copies), or each variable's text form (for a
        /// calculator of the other family, see [`crate::copy`]).
        format: Format,
        /// Kinds (as `G D` names them) not to get: listed in
        /// [`Tree::skipped`] instead ([`crate::copy::untranslatable_kinds`]).
        skip: Vec<String>,
    },
    /// Put a tree of directories and files. [`Outcome::TreeStored`].
    PutTree {
        /// The directory to put into.
        dir: Vec<String>,
        /// A new directory in `dir` that receives the tree
        /// ([`TxError::NameClash`] when the name is in use); `None` puts
        /// the tree into `dir` itself, into the directories it already
        /// has.
        name: Option<String>,
        /// The directories and files, a directory before what is in it.
        items: Vec<TreeItem>,
        /// Replace variables in use ([`TxError::NamesInUse`] otherwise),
        /// as [`Request::Put`] does; a directory is never replaced.
        replace: bool,
        /// Store files that are no calculator object as strings
        /// ([`TxError::NotAnObject`] otherwise).
        as_string: bool,
    },
    /// The free memory and what HOME holds, for the backup dialog.
    /// [`Outcome::Memory`].
    Memory,
    /// Back up HOME (`ARCHIVE` to port 0, get it, purge the copies).
    /// Refused with [`TxError::NoRoom`] before anything changes when the
    /// free memory cannot hold the copies ([`Memory::fits`]).
    /// [`Outcome::Backup`].
    Backup,
    /// Replace everything on the calculator with a backup (a directory
    /// object): put, copied to port 0, `RESTORE`. The calculator
    /// warm-starts and leaves server mode. [`Outcome::Restored`].
    Restore {
        /// The backup file.
        data: Vec<u8>,
    },
    /// The graphics screen (PICT). [`Outcome::Pict`].
    Pict,
}

/// One directory or variable of a tree got or put, by its path below the
/// tree's directory (`["SUB", "X"]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeItem {
    /// The names from the tree's directory down, the item's own last.
    pub path: Vec<String>,
    /// The file (a binary transfer file when got); `None` for a directory.
    pub data: Option<Vec<u8>>,
}

impl TreeItem {
    /// Whether the item is a directory.
    pub fn is_directory(&self) -> bool {
        self.data.is_none()
    }
}

/// A directory got with everything in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    /// The directory, from HOME.
    pub dir: Vec<String>,
    /// Its directories and variables, a directory before its contents.
    pub items: Vec<TreeItem>,
    /// The variables left out for their kind: path below the directory,
    /// and the kind.
    pub skipped: Vec<(Vec<String>, String)>,
}

/// Room for a backup. Measured on the emulated 48SX, 48GX and 49G
/// (2026-10-11): `ARCHIVE` takes HOME's size plus about 80 bytes, storing
/// the recalled copy in a variable as much again, so the peak is twice
/// HOME's size and about 175 bytes; [`BACKUP_MARGIN`] is kept on top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    /// Free bytes (the `G D` header on the 48GX and 49G, `MEM` on the
    /// 48SX).
    pub free: f64,
    /// Bytes HOME holds: the sum of its variables' sizes as `G D` lists
    /// them (a directory's with its contents).
    pub home: f64,
    /// Bytes a backup needs free: twice `home` and [`BACKUP_MARGIN`].
    pub needed: f64,
}

/// What a backup needs free on top of two copies of HOME.
pub const BACKUP_MARGIN: f64 = 1024.0;

impl Memory {
    /// Memory for `home` bytes in HOME and `free` bytes free.
    pub fn new(free: f64, home: f64) -> Self {
        Memory {
            free,
            home,
            needed: 2.0 * home + BACKUP_MARGIN,
        }
    }

    /// Whether a backup fits.
    pub fn fits(&self) -> bool {
        self.free >= self.needed
    }
}

/// A backup of HOME.
#[derive(Clone, Debug, PartialEq)]
pub struct Backup {
    /// The calculator's model (for the file name,
    /// [`names::backup_file_name`]).
    pub model: Model,
    /// The binary transfer file of HOME, a directory object.
    pub data: Vec<u8>,
    /// The memory before the backup.
    pub memory: Memory,
}

/// The graphics screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pict {
    /// Width in pixels (0 when nothing was ever drawn).
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// The binary transfer file of the GROB.
    pub data: Vec<u8>,
}

/// The calculator found by [`Request::Connect`].
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    /// The settings it answered at.
    pub settings: LinkSettings,
    /// The model.
    pub model: Model,
    /// The ROM version text; `None` on the 48S/SX.
    pub version: Option<String>,
    /// The transfer mode the user had (flag -35), put back at disconnect.
    pub user_mode: TransferMode,
}

/// A variable got from the calculator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    /// The variable name.
    pub name: String,
    /// The file name for it (see [`names::file_name`]).
    pub file_name: String,
    /// Binary or text.
    pub format: Format,
    /// The file.
    pub data: Vec<u8>,
}

/// The result of a [`Request`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Outcome {
    /// Connected.
    Connected(Connection),
    /// A directory listing; its path is the one asked for when the model
    /// does not tell it (the 48S/SX).
    Listing(Listing),
    /// A variable.
    File(File),
    /// The name the calculator stored the file under.
    Stored(String),
    /// Server mode ended.
    Disconnected,
    /// The directory made.
    Created(String),
    /// A variable or directory renamed.
    Renamed {
        /// The old name.
        from: String,
        /// The new name.
        to: String,
    },
    /// A variable or directory purged.
    Purged {
        /// Its name.
        name: String,
        /// How many variables went with a directory.
        count: usize,
    },
    /// A directory with everything in it.
    Tree(Tree),
    /// A tree put.
    TreeStored {
        /// Directories made.
        directories: usize,
        /// Variables stored.
        variables: usize,
    },
    /// The free memory and HOME's size.
    Memory(Memory),
    /// A backup of HOME.
    Backup(Backup),
    /// The backup is restored: the calculator warm-started and left server
    /// mode, the device is disconnected ([`Device::restarted`]).
    Restored,
    /// The graphics screen.
    Pict(Pict),
}

/// What the host does for the device, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// (Re)open the port with these settings before the next write; input
    /// read before is not fed any more.
    Open(LinkSettings),
    /// Write these bytes as one write.
    Write(Vec<u8>),
}

/// What happens on the way, for the host's status line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The search listens at these settings.
    Listening(LinkSettings),
    /// The calculator answers at these settings.
    Found(LinkSettings),
    /// Kermit progress (see [`Device::set_progress`]).
    Progress(Progress),
    /// A tree request moves on to this variable or directory (its path
    /// from HOME).
    Item(Vec<String>),
}

/// Why a [`Request`] failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TxError {
    /// A request is already running.
    #[error("another operation is still running")]
    Busy,
    /// The request needs a connection.
    #[error("not connected to a calculator")]
    NotConnected,
    /// [`Request::Connect`] while connected.
    #[error("already connected")]
    AlreadyConnected,
    /// Nothing answered at any of these settings.
    #[error("no calculator answered (tried {}); is it in server mode?", list(.tried))]
    NoAnswer {
        /// The settings listened at.
        tried: Vec<LinkSettings>,
    },
    /// A put to a name in use without `replace`.
    #[error("{0} already exists")]
    NameClash(String),
    /// A put would replace a directory.
    #[error("{0} is a directory")]
    IsDirectory(String),
    /// A put of a file that is neither binary nor text without
    /// `as_string`.
    #[error("the file is not a calculator object")]
    NotAnObject,
    /// A name the calculator refuses.
    #[error("the calculator does not take {0:?} as a name")]
    BadName(String),
    /// No variable or directory of that name.
    #[error("{0} does not exist")]
    NotFound(String),
    /// A purge of a directory that holds variables without their count.
    #[error("{name} holds {count} variable(s)")]
    NotEmpty {
        /// The directory.
        name: String,
        /// How many variables it holds, at every level.
        count: usize,
    },
    /// A tree put to names in use without `replace` (their paths, `/`
    /// between the names).
    #[error("{} already exist(s)", .0.join(", "))]
    NamesInUse(Vec<String>),
    /// A tree put would make a directory where a variable is.
    #[error("{0} is a variable, not a directory")]
    NotADirectory(String),
    /// Two items of a tree put have the same name.
    #[error("two files would both be {0}")]
    Duplicate(String),
    /// Not enough free memory for a backup.
    #[error(
        "a backup needs {needed} bytes free and the calculator has {free}; \
         copy everything to a folder instead"
    )]
    NoRoom {
        /// Bytes needed.
        needed: f64,
        /// Bytes free.
        free: f64,
    },
    /// A tree put failed part of the way: what it made and stored before.
    #[error(
        "{source}; {variables} variable(s) and {directories} director(y/ies) were stored before{}",
        if *.root_made { ", the new directory included" } else { "" }
    )]
    Partial {
        /// Directories made, the new one included.
        directories: usize,
        /// Variables stored.
        variables: usize,
        /// Whether the tree's own new directory was made (then purging it
        /// takes the rest away).
        root_made: bool,
        /// Why it stopped.
        source: Box<TxError>,
    },
    /// The calculator restarted after a restore.
    #[error("the calculator restarted; start SERVER on it and reconnect")]
    Restarted,
    /// The protocol, the link or the calculator.
    #[error(transparent)]
    Calc(#[from] satx_core::Error),
}

fn list(tried: &[LinkSettings]) -> String {
    tried
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The device's result type.
pub type Result<T> = std::result::Result<T, TxError>;

/// What the device keeps while connected.
#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) connection: Connection,
    /// The calculator's current directory, when known.
    pub(crate) cwd: Option<Vec<String>>,
}

#[derive(Debug)]
struct Shared {
    now: Instant,
    options: DeviceOptions,
    /// State of the seed generator for each new [`Machine`].
    seed: u64,
    actions: VecDeque<Action>,
    events: VecDeque<Event>,
    keep_progress: bool,
    /// Input goes to `heard` (the search) instead of the machine.
    listening: bool,
    heard: Vec<u8>,
    /// When the running request's own wait ends.
    wake: Option<Instant>,
    /// The link failed while listening.
    link_error: Option<(io::ErrorKind, String)>,
    machine: Option<Machine>,
    /// The machine's result, not yet taken by the request.
    op_result: Option<satx_core::Result<Reply>>,
    session: Option<Session>,
    /// A restore warm-started the calculator; cleared by a connect.
    restarted: bool,
    /// The backup a restore left in port 0 (`:0:SATXRS`) is not purged
    /// yet: tried at each connect and before each request until it is.
    leftover: bool,
}

impl Shared {
    fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Move what the machine produced to the device's queues.
    fn pump(&mut self) {
        let Some(machine) = self.machine.as_mut() else {
            return;
        };
        while let Some(bytes) = machine.poll_transmit() {
            self.actions.push_back(Action::Write(bytes));
        }
        while let Some(event) = machine.poll_event() {
            self.events.push_back(Event::Progress(event));
        }
        if let Some(result) = machine.poll_result() {
            self.op_result = Some(result);
        }
    }
}

type Task = Pin<Box<dyn Future<Output = Result<Outcome>> + Send>>;

/// A calculator on a link, without I/O. See the [module documentation](self).
pub struct Device {
    shared: Arc<Mutex<Shared>>,
    task: Option<Task>,
    result: Option<Result<Outcome>>,
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Device")
            .field("busy", &self.is_busy())
            .field("connection", &self.connection())
            .finish_non_exhaustive()
    }
}

impl Device {
    /// A device for a link the host is about to open (the first request,
    /// [`Request::Connect`], says how). `seed` should be random: it seeds
    /// the sync markers.
    pub fn new(now: Instant, options: DeviceOptions, seed: u64) -> Self {
        Device {
            shared: Arc::new(Mutex::new(Shared {
                now,
                options,
                seed,
                actions: VecDeque::new(),
                events: VecDeque::new(),
                keep_progress: true,
                listening: false,
                heard: Vec::new(),
                wake: None,
                link_error: None,
                machine: None,
                op_result: None,
                session: None,
                restarted: false,
                leftover: false,
            })),
            task: None,
            result: None,
        }
    }

    fn shared(&self) -> MutexGuard<'_, Shared> {
        lock(&self.shared)
    }

    /// Begin `request`; its first actions are ready at once. A result not
    /// yet taken is dropped.
    pub fn start(&mut self, now: Instant, request: Request) -> std::result::Result<(), Busy> {
        if self.task.is_some() {
            return Err(Busy);
        }
        set_now(&mut self.shared(), now);
        self.result = None;
        let ctx = Ctx(Arc::clone(&self.shared));
        self.task = Some(Box::pin(async move { perform(&ctx, request).await }));
        self.step();
        Ok(())
    }

    /// Feed bytes read from the link at `now`.
    pub fn handle_input(&mut self, now: Instant, bytes: &[u8]) {
        {
            let mut sh = self.shared();
            set_now(&mut sh, now);
            if sh.listening {
                sh.heard.extend_from_slice(bytes);
            } else if let Some(machine) = sh.machine.as_mut() {
                machine.handle_input(now, bytes);
            }
        }
        self.step();
    }

    /// Tell the device the time is `now`.
    pub fn handle_timeout(&mut self, now: Instant) {
        {
            let mut sh = self.shared();
            set_now(&mut sh, now);
            if let Some(machine) = sh.machine.as_mut() {
                machine.handle_timeout(now);
            }
        }
        self.step();
    }

    /// The link failed or closed: the running request fails.
    pub fn handle_link_error(&mut self, now: Instant, error: &io::Error) {
        {
            let mut sh = self.shared();
            set_now(&mut sh, now);
            if sh.listening {
                sh.link_error = Some((error.kind(), error.to_string()));
            } else if let Some(machine) = sh.machine.as_mut() {
                machine.handle_link_error(now, error);
            }
        }
        self.step();
    }

    /// The next thing to do for the device.
    pub fn poll_action(&mut self) -> Option<Action> {
        self.shared().actions.pop_front()
    }

    /// The next event.
    pub fn poll_event(&mut self) -> Option<Event> {
        self.shared().events.pop_front()
    }

    /// Whether Kermit progress events are kept (default `true`); they carry
    /// the file contents.
    pub fn set_progress(&mut self, keep: bool) {
        let mut sh = self.shared();
        sh.keep_progress = keep;
        if let Some(machine) = sh.machine.as_mut() {
            machine.set_events(keep);
        }
        if !keep {
            sh.events.retain(|e| !matches!(e, Event::Progress(_)));
        }
    }

    /// The result of the last request, once it is over.
    pub fn poll_result(&mut self) -> Option<Result<Outcome>> {
        self.result.take()
    }

    /// When to call [`Device::handle_timeout`] at the latest; `None` while
    /// no request runs.
    pub fn next_timeout(&self) -> Option<Instant> {
        self.task.as_ref()?;
        let sh = self.shared();
        let machine = sh.machine.as_ref().and_then(Machine::next_timeout);
        match (sh.wake, machine) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Whether a request runs.
    pub fn is_busy(&self) -> bool {
        self.task.is_some()
    }

    /// The connected calculator.
    pub fn connection(&self) -> Option<Connection> {
        self.shared().session.as_ref().map(|s| s.connection.clone())
    }

    /// Whether a restore has warm-started the calculator since the last
    /// connect: it has left server mode; `SERVER` on it, then
    /// [`Request::Connect`].
    pub fn restarted(&self) -> bool {
        self.shared().restarted
    }

    /// Drop the running request without telling the calculator (see
    /// [`Machine::abort`]). The connection stays; the next request starts
    /// from HOME. No-op when idle.
    pub fn abort(&mut self, now: Instant) {
        if self.task.take().is_none() {
            return;
        }
        let mut sh = self.shared();
        set_now(&mut sh, now);
        if let Some(machine) = sh.machine.as_mut() {
            machine.abort(now);
        }
        sh.actions.clear();
        sh.events.clear();
        sh.listening = false;
        sh.heard.clear();
        sh.wake = None;
        sh.op_result = None;
        if let Some(session) = sh.session.as_mut() {
            session.cwd = None;
        }
    }

    /// Poll the request until it waits for input or the time.
    fn step(&mut self) {
        loop {
            self.shared().pump();
            lock(&self.shared).wake = None;
            let Some(task) = self.task.as_mut() else {
                return;
            };
            let mut cx = Context::from_waker(Waker::noop());
            let polled = task.as_mut().poll(&mut cx);
            let mut sh = self.shared();
            sh.pump();
            match polled {
                Poll::Ready(result) => {
                    sh.listening = false;
                    sh.wake = None;
                    drop(sh);
                    self.task = None;
                    self.result = Some(result);
                    return;
                }
                // The machine finished an operation the request has not
                // taken yet.
                Poll::Pending if sh.op_result.is_some() => {}
                Poll::Pending => return,
            }
        }
    }
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    // Nothing panics while holding the lock.
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The clock never runs backwards.
fn set_now(sh: &mut Shared, now: Instant) {
    if now > sh.now {
        sh.now = now;
    }
}

/// The request's handle on the shared state.
pub(crate) struct Ctx(Arc<Mutex<Shared>>);

impl Ctx {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        lock(&self.0)
    }

    /// The connected model.
    pub(crate) fn model(&self) -> Result<Model> {
        self.session(|s| s.connection.model)
    }

    /// The calculator warm-started: no session, no machine.
    pub(crate) fn restarted(&self) {
        let mut sh = self.lock();
        sh.session = None;
        sh.machine = None;
        sh.restarted = true;
        sh.leftover = true;
    }

    pub(crate) fn event(&self, event: Event) {
        self.lock().events.push_back(event);
    }

    fn write(&self, bytes: Vec<u8>) {
        self.lock().actions.push_back(Action::Write(bytes));
    }

    fn open(&self, settings: LinkSettings) {
        self.lock().actions.push_back(Action::Open(settings));
    }

    /// Run one machine operation.
    pub(crate) async fn op(&self, op: Op) -> Result<Reply> {
        {
            let mut sh = self.lock();
            let now = sh.now;
            let machine = sh.machine.as_mut().ok_or(TxError::NotConnected)?;
            machine.start(now, op).map_err(|Busy| TxError::Busy)?;
        }
        poll_fn(|_| match self.lock().op_result.take() {
            Some(result) => Poll::Ready(result),
            None => Poll::Pending,
        })
        .await
        .map_err(TxError::from)
    }

    /// Collect input until a packet is heard or `until`.
    async fn listen(&self, until: Instant) -> Result<Vec<u8>> {
        {
            let mut sh = self.lock();
            sh.listening = true;
            sh.heard.clear();
            sh.link_error = None;
        }
        let heard = poll_fn(|_| {
            let mut sh = self.lock();
            if let Some((kind, msg)) = sh.link_error.take() {
                return Poll::Ready(Err(TxError::Calc(io::Error::new(kind, msg).into())));
            }
            if hear(&sh.heard).is_some() || sh.now >= until {
                return Poll::Ready(Ok(std::mem::take(&mut sh.heard)));
            }
            sh.wake = Some(until);
            Poll::Pending
        })
        .await;
        self.lock().listening = false;
        heard
    }

    pub(crate) fn session<T>(&self, f: impl FnOnce(&mut Session) -> T) -> Result<T> {
        self.lock()
            .session
            .as_mut()
            .map(f)
            .ok_or(TxError::NotConnected)
    }
}

async fn perform(ctx: &Ctx, request: Request) -> Result<Outcome> {
    if ctx.lock().restarted && !matches!(request, Request::Connect(_)) {
        return Err(TxError::Restarted);
    }
    let retry = {
        let sh = ctx.lock();
        sh.leftover && sh.session.is_some()
    };
    if retry && !matches!(request, Request::Connect(_) | Request::Disconnect) {
        purge_leftover(ctx).await;
    }
    match request {
        Request::Connect(options) => connect(ctx, options).await,
        Request::List { dir } => list_dir(ctx, &absolute(dir)).await.map(Outcome::Listing),
        Request::Get { dir, name, format } => get(ctx, &absolute(dir), name, format).await,
        Request::Put {
            dir,
            name,
            data,
            replace,
            as_string,
        } => {
            let dir = absolute(dir);
            put(ctx, &dir, name, data, replace, as_string).await
        }
        Request::Disconnect => disconnect(ctx).await,
        Request::Mkdir { dir, name } => ops::mkdir(ctx, &absolute(dir), name).await,
        Request::Rename { dir, from, to } => ops::rename(ctx, &absolute(dir), from, to).await,
        Request::Purge { dir, name, count } => ops::purge(ctx, &absolute(dir), name, count).await,
        Request::GetTree { dir, format, skip } => {
            ops::get_tree(ctx, absolute(dir), format, &skip).await
        }
        Request::PutTree {
            dir,
            name,
            items,
            replace,
            as_string,
        } => {
            let tree = ops::PutTree {
                name,
                items,
                replace,
                as_string,
            };
            ops::put_tree(ctx, &absolute(dir), tree).await
        }
        Request::Memory => ops::memory(ctx).await.map(Outcome::Memory),
        Request::Backup => ops::backup(ctx).await,
        Request::Restore { data } => ops::restore(ctx, data).await,
        Request::Pict => ops::pict(ctx).await,
    }
}

/// `dir` from HOME.
pub(crate) fn absolute(dir: Vec<String>) -> Vec<String> {
    if dir.first().map(String::as_str) == Some("HOME") {
        dir
    } else {
        std::iter::once("HOME".to_string()).chain(dir).collect()
    }
}

/// Probe each speed and listen for the answer (see `search`).
async fn search(ctx: &Ctx) -> Result<LinkSettings> {
    let mut tried = Vec::new();
    for baud in SPEEDS {
        let settings = LinkSettings {
            baud,
            parity: Parity::None,
        };
        ctx.open(settings);
        ctx.event(Event::Listening(settings));
        let (start, listen) = {
            let sh = ctx.lock();
            (sh.now, sh.options.listen)
        };
        // Probe twice: the calculator may still be busy with the first.
        let mut heard = Vec::new();
        'probing: for round in [start, start + listen / 2] {
            let parities = [Parity::None, Parity::Odd, Parity::Even];
            for (i, parity) in parities.into_iter().enumerate() {
                ctx.write(probe(parity));
                let until = if i + 1 < parities.len() {
                    ctx.lock().now + PROBE_GAP
                } else {
                    round + listen / 2
                };
                heard = ctx.listen(until).await?;
                if hear(&heard).is_some() {
                    break 'probing;
                }
            }
        }
        tried.push(settings);
        if let Some(parity) = hear(&heard) {
            let found = LinkSettings { baud, parity };
            if found != settings {
                ctx.open(found);
            }
            ctx.event(Event::Found(found));
            return Ok(found);
        }
    }
    Err(TxError::NoAnswer { tried })
}

async fn connect(ctx: &Ctx, options: ConnectOptions) -> Result<Outcome> {
    if ctx.lock().session.is_some() {
        return Err(TxError::AlreadyConnected);
    }
    {
        let mut sh = ctx.lock();
        sh.machine = None;
        sh.op_result = None;
    }
    let settings = if options.find {
        search(ctx).await?
    } else {
        ctx.open(options.settings);
        options.settings
    };
    {
        let mut sh = ctx.lock();
        let mut session = sh.options.session.clone();
        if settings.parity != Parity::None {
            // A 7-bit link: the 8th bit travels prefixed.
            session.kermit.eight_bit_clean = false;
        }
        if options.find {
            // The search has read the stale input already.
            session.drain = Duration::ZERO;
        }
        let seed = sh.next_seed();
        let mut machine = Machine::new(sh.now, session, seed);
        machine.set_events(sh.keep_progress);
        sh.machine = Some(machine);
    }
    expect_done(ctx.op(Op::Connect).await?)?;
    let version = match ctx.op(Op::Version).await? {
        Reply::Version(v) => v,
        other => return Err(unexpected("VERSION", &other)),
    };
    let model = Model::from_version(version.as_deref());
    let user_mode = match ctx.op(Op::TransferMode).await? {
        Reply::TransferMode(mode) => mode,
        other => return Err(unexpected("-35 FS?", &other)),
    };
    expect_done(ctx.op(Op::SetTransferMode(TransferMode::Binary)).await?)?;
    let connection = Connection {
        settings,
        model,
        version,
        user_mode,
    };
    {
        let mut sh = ctx.lock();
        sh.restarted = false;
        sh.session = Some(Session {
            connection: connection.clone(),
            cwd: None,
        });
    }
    if ctx.lock().leftover {
        purge_leftover(ctx).await;
    }
    Ok(Outcome::Connected(connection))
}

/// Purge the backup a restore left in port 0 (`:0:SATXRS`); purging a
/// missing port object is no error (wiki: protocols/server-commands). The
/// connection is good either way and the leftover only takes memory, so a
/// failure is not the request's: it is tried again before the next one.
async fn purge_leftover(ctx: &Ctx) {
    if let Ok(Reply::Done) = ctx.op(Op::PurgeRestoreLeftover).await {
        ctx.lock().leftover = false;
    }
}

/// Make `dir` the calculator's current directory.
pub(crate) async fn cd(ctx: &Ctx, dir: &[String]) -> Result<()> {
    if ctx.session(|s| s.cwd.as_deref() == Some(dir))? {
        return Ok(());
    }
    ctx.session(|s| s.cwd = None)?;
    expect_done(ctx.op(Op::Cd(dir.to_vec())).await?)?;
    ctx.session(|s| s.cwd = Some(dir.to_vec()))
}

pub(crate) async fn list_dir(ctx: &Ctx, dir: &[String]) -> Result<Listing> {
    ctx.session(|_| ())?;
    cd(ctx, dir).await?;
    match ctx.op(Op::List).await? {
        Reply::Listing(mut listing) => {
            listing.path.get_or_insert_with(|| dir.to_vec());
            Ok(listing)
        }
        other => Err(unexpected("G D", &other)),
    }
}

/// Binary again after a transfer in text mode; the first error wins.
async fn back_to_binary<T>(ctx: &Ctx, result: Result<T>) -> Result<T> {
    let back = ctx.op(Op::SetTransferMode(TransferMode::Binary)).await;
    let value = result?;
    expect_done(back?)?;
    Ok(value)
}

pub(crate) async fn get(
    ctx: &Ctx,
    dir: &[String],
    name: String,
    format: Format,
) -> Result<Outcome> {
    ctx.session(|_| ())?;
    validate_name(&name).map_err(|_| TxError::BadName(name.clone()))?;
    cd(ctx, dir).await?;
    let mode = match format {
        Format::Binary => TransferMode::Binary,
        Format::Text => TransferMode::Ascii,
    };
    let got = ctx
        .op(Op::Get {
            name: name.clone(),
            mode,
        })
        .await;
    let got = if mode == TransferMode::Ascii {
        back_to_binary(ctx, got).await?
    } else {
        got?
    };
    match got {
        Reply::Data(data) => Ok(Outcome::File(File {
            file_name: names::file_name(&name, format),
            name,
            format,
            data,
        })),
        other => Err(unexpected("GET", &other)),
    }
}

async fn put(
    ctx: &Ctx,
    dir: &[String],
    name: String,
    data: Vec<u8>,
    replace: bool,
    as_string: bool,
) -> Result<Outcome> {
    ctx.session(|_| ())?;
    validate_name(&name).map_err(|_| TxError::BadName(name.clone()))?;
    let kind = FileKind::of(&data);
    if kind == FileKind::Other && !as_string {
        return Err(TxError::NotAnObject);
    }
    let listing = list_dir(ctx, dir).await?;
    put_in(ctx, &listing.entries, name, data, replace)
        .await
        .map(Outcome::Stored)
}

/// Put `data` as `name` into the current directory, whose variables are
/// `entries` (the name checked, the file one [`put`] takes).
pub(crate) async fn put_in(
    ctx: &Ctx,
    entries: &[Entry],
    name: String,
    data: Vec<u8>,
    replace: bool,
) -> Result<String> {
    let kind = FileKind::of(&data);
    let mut target = name.clone();
    if let Some(entry) = entries.iter().find(|e| e.name == name) {
        if entry.is_directory() {
            return Err(TxError::IsDirectory(name));
        }
        if !replace {
            return Err(TxError::NameClash(name));
        }
        // Into a name nobody uses first: a put that fails leaves the old
        // variable as it was.
        target = (1..)
            .map(|n| format!("{REPLACE_VAR}{n}"))
            .find(|t| !entries.iter().any(|e| &e.name == t))
            .unwrap_or_else(|| REPLACE_VAR.to_string());
    }
    let mode = match kind {
        FileKind::Text => TransferMode::Ascii,
        FileKind::Binary | FileKind::Other => TransferMode::Binary,
    };
    let stored = ctx
        .op(Op::Put {
            name: target.clone(),
            data,
            mode,
        })
        .await;
    let stored = if mode == TransferMode::Ascii {
        back_to_binary(ctx, stored).await
    } else {
        stored
    };
    let stored = match stored {
        Ok(Reply::Stored(stored)) => stored,
        Ok(other) => return Err(unexpected("SEND", &other)),
        Err(e) => {
            if target != name {
                // Best effort: the error being reported matters more.
                let _ = ctx.op(Op::Remove(target)).await;
            }
            return Err(e);
        }
    };
    if target == name {
        return Ok(stored);
    }
    // One command: STO replaces the old variable, PURGE drops the copy.
    let swap = format!("'{stored}' RCL '{name}' STO '{stored}' PURGE");
    let swapped = match ctx.op(Op::Run(swap)).await {
        Ok(Reply::Stack(reply)) => match reply.error {
            None => return Ok(name),
            Some(message) => TxError::Calc(satx_core::Error::Calculator {
                message,
                stack: reply.levels,
            }),
        },
        Ok(other) => unexpected("STO", &other),
        Err(e) => e,
    };
    // Best effort, as for a failed send: the old variable is as it was
    // (or already replaced); the copy in the unused name goes.
    let _ = ctx.op(Op::Remove(stored)).await;
    Err(swapped)
}

async fn disconnect(ctx: &Ctx) -> Result<Outcome> {
    let user_mode = ctx.session(|s| s.connection.user_mode)?;
    // FINISH even when flag -35 could not be put back.
    let restored = ctx.op(Op::SetTransferMode(user_mode)).await;
    let finished = ctx.op(Op::Finish).await;
    let finished = finished.and_then(expect_done);
    if finished.is_ok() {
        // Server mode has ended: the session is over either way.
        let mut sh = ctx.lock();
        sh.session = None;
        sh.machine = None;
    }
    // Otherwise the session stays for another try.
    expect_done(restored?)?;
    finished?;
    Ok(Outcome::Disconnected)
}

pub(crate) fn expect_done(reply: Reply) -> Result<()> {
    match reply {
        Reply::Done => Ok(()),
        other => Err(unexpected("the operation", &other)),
    }
}

pub(crate) fn unexpected(what: &str, reply: &Reply) -> TxError {
    TxError::Calc(satx_core::Error::Reply(format!(
        "{what}: unexpected result {reply:?}"
    )))
}
