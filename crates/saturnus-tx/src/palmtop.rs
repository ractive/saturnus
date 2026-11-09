//! An HP 100LX or 200LX palmtop on one link, without I/O: its Filer in
//! server mode, over `filer-proto`'s [`Session`] (wiki:
//! protocols/filer-hplx, hardware/hp100lx-200lx). A [`Palmtop`] has the
//! [`Device`](crate::Device)'s seam: it is fed bytes, the time and link
//! errors, and hands out [`Action`]s, [`PalmtopEvent`]s and the result of
//! each [`PalmtopRequest`].
//!
//! On the palmtop: Filer, MENU, Communications, Remote Settings: Baud as
//! the link (19200 by default, [`DEFAULT_BAUD`]), Interface Com1, Server
//! Mode on, OK; then Filer stays open and in front.
//!
//! - **Places.** A directory is a list of names from the drive down,
//!   `["C:", "_DAT"]`; the empty list is the top, whose listing is the
//!   drives (`C:`, `A:`, `D:`).
//! - **Names.** DOS names, 8.3, upper case: a name the palmtop is to
//!   make (a put, a new directory, a rename's target) is checked first
//!   ([`dos_name`]); a longer one is refused with
//!   [`PalmtopError::BadName`] before anything is sent. Names read from
//!   the palmtop come as Latin-1 text and go back byte for byte.
//! - **Nothing replaced unasked.** A put to a name in use is
//!   [`PalmtopError::Exists`] until it is sent again with `replace`; a
//!   rename never goes over another name; [`PalmtopRequest::Remove`] of a
//!   directory removes everything in it, so the host asks first (its
//!   count comes from the listings).
//! - **Stop.** [`Palmtop::stop`] ends the running request at its next
//!   reply ([`Session::cancel`]): a get closes the file, a put closes and
//!   deletes the part written, a removal stops after the file or directory
//!   it is at. It fails with [`PalmtopError::Stopped`]; the session stays.
//! - **Not answering.** A request the palmtop does not answer after three
//!   sends (HP's timing: 5.9 s each) ends the session:
//!   [`PalmtopError::NotAnswering`], no connection, every request but
//!   [`PalmtopRequest::Connect`] refused until it answers again
//!   ([`Palmtop::not_answering`]).
//! - **Session ended on the palmtop.** A palmtop whose session was ended
//!   there (it went back to waiting in server mode) answers a request with
//!   a CONNECT refusal and runs nothing (wiki: protocols/filer-hplx,
//!   "Connect and disconnect"). When that refusal is the first reply of a
//!   request, the [`Palmtop`] connects again at the same speed and sends
//!   the request once more; a refusal after the request had begun or once
//!   it was stopped, a failed CONNECT or a second refusal ends the
//!   connection as a palmtop not answering does.
//!
//! No I/O, no threads, no clock: builds for `wasm32-unknown-unknown`.

use std::collections::VecDeque;
use std::fmt;

use filer_proto::path;
use filer_proto::time::{Duration, Instant};
use filer_proto::{
    Config, DirEntry, DosDateTime, Failure, FsError, Job, JobError, Output, Session, SessionEvent,
    attr,
};

use crate::device::{Action, LinkSettings, Parity};

/// The speed the palmtop is set to unless the user chose another: the
/// comp.sys.palmtops FAQ's safe limit for its 8250 UART, which has no
/// FIFO (wiki: hardware/hp100lx-200lx).
pub const DEFAULT_BAUD: u32 = 19200;

/// The speeds the Filer's Remote Settings offer (wiki:
/// protocols/filer-hplx). Above [`DEFAULT_BAUD`] the stock UART may lose
/// bytes.
pub const SPEEDS: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];

/// What a host asks a [`Palmtop`] to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PalmtopRequest {
    /// Open the port at `baud`, 8N1, and CONNECT.
    /// [`PalmtopOutcome::Connected`].
    Connect {
        /// One of [`SPEEDS`]; the palmtop must be set to the same.
        baud: u32,
    },
    /// A directory's entries; the top (`[]`) gives the drives.
    /// [`PalmtopOutcome::Listing`].
    List {
        /// The directory, from the drive.
        dir: Vec<String>,
    },
    /// A whole file. [`PalmtopOutcome::File`].
    Get {
        /// Its directory.
        dir: Vec<String>,
        /// Its name.
        name: String,
    },
    /// Write a whole file. [`PalmtopOutcome::Stored`] with the name made.
    Put {
        /// The directory.
        dir: Vec<String>,
        /// The name (8.3; made upper case).
        name: String,
        /// The contents.
        data: Vec<u8>,
        /// Replace a file of that name ([`PalmtopError::Exists`] otherwise).
        replace: bool,
    },
    /// Rename within a directory. [`PalmtopOutcome::Renamed`].
    Rename {
        /// The directory.
        dir: Vec<String>,
        /// The existing name.
        from: String,
        /// The new name (8.3).
        to: String,
    },
    /// Delete a file, or a directory with everything in it.
    /// [`PalmtopOutcome::Removed`].
    Remove {
        /// The directory it is in.
        dir: Vec<String>,
        /// Its name.
        name: String,
        /// Whether it is a directory.
        directory: bool,
    },
    /// Make a directory. [`PalmtopOutcome::Created`].
    MakeDir {
        /// Where.
        dir: Vec<String>,
        /// The name (8.3).
        name: String,
    },
    /// End the session: one DISCONNECT, not awaited (the palmtop stays in
    /// server mode). [`PalmtopOutcome::Disconnected`].
    Disconnect,
}

impl PalmtopRequest {
    /// A short name for traces and notes: `list C:\_DAT`.
    pub fn describe(&self) -> String {
        let at = |dir: &[String], name: Option<&str>| {
            let mut p = dir.join("\\");
            if let Some(n) = name {
                if !p.is_empty() {
                    p.push('\\');
                }
                p.push_str(n);
            }
            p
        };
        match self {
            PalmtopRequest::Connect { baud } => format!("connect {baud}"),
            PalmtopRequest::List { dir } if dir.is_empty() => "list drives".into(),
            PalmtopRequest::List { dir } => format!("list {}", at(dir, None)),
            PalmtopRequest::Get { dir, name } => format!("get {}", at(dir, Some(name))),
            PalmtopRequest::Put {
                dir, name, replace, ..
            } => format!(
                "put {}{}",
                at(dir, Some(name)),
                if *replace { " replace" } else { "" }
            ),
            PalmtopRequest::Rename { dir, from, to } => {
                format!("rename {} {to}", at(dir, Some(from)))
            }
            PalmtopRequest::Remove { dir, name, .. } => format!("remove {}", at(dir, Some(name))),
            PalmtopRequest::MakeDir { dir, name } => format!("mkdir {}", at(dir, Some(name))),
            PalmtopRequest::Disconnect => "disconnect".into(),
        }
    }
}

/// One entry of a listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PalmtopEntry {
    /// The name (`NOTES.TXT`, a drive's `C:`), Latin-1.
    pub name: String,
    /// A directory (or a drive).
    pub directory: bool,
    /// A drive of the top listing.
    pub drive: bool,
    /// Bytes; 0 for a directory.
    pub size: u32,
    /// The time of the last change; `None` for a drive.
    pub modified: Option<DosDateTime>,
    /// The DOS attribute byte ([`attr`]).
    pub attributes: u8,
}

impl PalmtopEntry {
    fn from_dir_entry(e: &DirEntry) -> Self {
        PalmtopEntry {
            name: e.name_lossy(),
            directory: e.is_directory(),
            drive: false,
            size: e.size,
            modified: Some(e.modified),
            attributes: e.attributes,
        }
    }

    /// Read-only.
    pub fn read_only(&self) -> bool {
        self.attributes & attr::READ_ONLY != 0
    }
}

/// The palmtop found by [`PalmtopRequest::Connect`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PalmtopConnection {
    /// The speed it answered at.
    pub baud: u32,
    /// The protocol revision (2, or 1 for an older Filer).
    pub revision: u8,
}

/// The result of a [`PalmtopRequest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PalmtopOutcome {
    /// Connected.
    Connected(PalmtopConnection),
    /// A listing: the drives at the top, else files and directories
    /// (without `.` and `..`).
    Listing {
        /// The directory listed.
        dir: Vec<String>,
        /// Its entries, as the palmtop gave them.
        entries: Vec<PalmtopEntry>,
    },
    /// A file.
    File {
        /// Its name.
        name: String,
        /// Its contents.
        data: Vec<u8>,
        /// Its time, when the palmtop gave one.
        modified: Option<DosDateTime>,
    },
    /// The name the file was stored under.
    Stored(String),
    /// Renamed.
    Renamed {
        /// The old name.
        from: String,
        /// The new name, as made.
        to: String,
    },
    /// Removed, with how many files and directories went with it.
    Removed {
        /// Its name.
        name: String,
        /// What a directory held, at every level.
        count: usize,
    },
    /// The directory made.
    Created(String),
    /// The session ended.
    Disconnected,
}

/// What happens on the way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PalmtopEvent {
    /// Bytes of a get or put so far, and all of them when known.
    Progress {
        /// Bytes so far.
        done: u64,
        /// All bytes, when known.
        total: Option<u64>,
    },
}

/// Why a name cannot be made on the palmtop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameProblem {
    /// No name.
    Empty,
    /// More than 8 characters before the dot, or more than 3 after.
    TooLong,
    /// More than one dot.
    Dots,
    /// A character DOS does not take in a name.
    Char(char),
    /// A name DOS keeps for a device (`AUX` is the serial port the
    /// session runs on), with any extension.
    Device(String),
}

/// The names DOS keeps for its devices: a file of that name, whatever its
/// extension, is the device.
pub const DEVICE_NAMES: [&str; 13] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3", "CLOCK$",
    "CONFIG$",
];

impl fmt::Display for NameProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameProblem::Empty => f.write_str("a name is needed"),
            NameProblem::TooLong | NameProblem::Dots => f.write_str(
                "the palmtop takes DOS names: up to 8 letters or digits, a dot and up to 3 more (like NOTES.TXT)",
            ),
            NameProblem::Char(' ') => f.write_str("the palmtop takes no spaces in a name"),
            NameProblem::Char(c) => write!(f, "the palmtop takes no {c:?} in a name"),
            NameProblem::Device(d) => write!(
                f,
                "{d} is the name of a device on the palmtop (DOS keeps it), so no file can have it: choose another"
            ),
        }
    }
}

/// The DOS name the palmtop makes of `name` (upper case), or why it
/// cannot: 1 to 8 characters, then optionally a dot and 1 to 3 more;
/// printable ASCII without spaces and `"*+,./:;<=>?[\]|` (wiki:
/// protocols/filer-hplx, result 200).
pub fn dos_name(name: &str) -> std::result::Result<String, NameProblem> {
    if name.is_empty() {
        return Err(NameProblem::Empty);
    }
    if let Some(c) = name
        .chars()
        .find(|c| !c.is_ascii_graphic() || "\"*+,/:;<=>?[\\]|".contains(*c))
    {
        return Err(NameProblem::Char(c));
    }
    let mut parts = name.split('.');
    let base = parts.next().unwrap_or_default();
    let ext = parts.next();
    if parts.next().is_some() {
        return Err(NameProblem::Dots);
    }
    if base.is_empty() || base.len() > 8 || ext.is_some_and(|e| e.is_empty() || e.len() > 3) {
        return Err(if base.is_empty() && ext.is_some() {
            NameProblem::Dots
        } else {
            NameProblem::TooLong
        });
    }
    let upper = base.to_ascii_uppercase();
    if DEVICE_NAMES.contains(&upper.as_str()) {
        return Err(NameProblem::Device(upper));
    }
    Ok(name.to_ascii_uppercase())
}

/// Why a [`PalmtopRequest`] failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PalmtopError {
    /// A request is already running.
    #[error("another operation is still running")]
    Busy,
    /// The request needs a connection.
    #[error("not connected to the palmtop")]
    NotConnected,
    /// [`PalmtopRequest::Connect`] while connected.
    #[error("already connected")]
    AlreadyConnected,
    /// A speed the Filer does not offer.
    #[error("the palmtop has no speed of {0} baud")]
    BadSpeed(u32),
    /// Nothing answered the CONNECT.
    #[error(
        "no palmtop answered at {baud} baud; is Filer open in front, with server mode on at {baud} baud?"
    )]
    NoAnswer {
        /// The speed tried.
        baud: u32,
    },
    /// The palmtop refused the CONNECT.
    #[error("the palmtop refused the connection ({0})")]
    Refused(String),
    /// A name the palmtop cannot make.
    #[error("{name}: {problem}")]
    BadName {
        /// The name.
        name: String,
        /// Why.
        problem: NameProblem,
    },
    /// The name is in use (a put without `replace`, a rename's target).
    #[error("{0} is already there")]
    Exists(String),
    /// No file or directory of that name.
    #[error("{0} is not there")]
    NotFound(String),
    /// The palmtop refused a request with a file-system error.
    #[error("{name}: {}", fs_text(*.error))]
    Fs {
        /// What it was about.
        name: String,
        /// The palmtop's code.
        error: FsError,
    },
    /// The palmtop refused a request at the protocol level.
    #[error("{name}: the palmtop refused the request ({failure})")]
    Status {
        /// What it was about.
        name: String,
        /// The status.
        failure: String,
    },
    /// A directory removal found a read-only file in it: nothing was
    /// deleted.
    #[error(
        "{dir}: {file} in it is read-only, so nothing was deleted; clear its read-only mark on the palmtop first"
    )]
    ReadOnlyInside {
        /// The directory to remove.
        dir: String,
        /// The read-only file, from that directory down.
        file: String,
    },
    /// The palmtop refused to remove a directory the walk had emptied: it
    /// still holds what the listing does not show.
    #[error("{0}: the directory still holds files the palmtop does not list (hidden or system)")]
    HiddenInside(String),
    /// The file is larger than a get reads.
    #[error("{0} is larger than Files reads (16 MB)")]
    TooLarge(String),
    /// [`Palmtop::stop`].
    #[error("stopped")]
    Stopped,
    /// The palmtop sent nothing after three sends of a request: the
    /// session is over.
    #[error(
        "the palmtop stopped answering: is Filer still open in front with server mode on? Then reconnect"
    )]
    NotAnswering,
    /// The host's link failed.
    #[error("the link failed: {0}")]
    Link(String),
}

/// A file-system code in plain words.
fn fs_text(e: FsError) -> String {
    match e {
        FsError::INVALID_NAME => {
            "the palmtop does not take that name (DOS names are 8.3, like NOTES.TXT)".into()
        }
        FsError::DISK_FULL => "the palmtop's disk is full".into(),
        FsError::NOT_FOUND => "not there any more".into(),
        FsError::PATH_NOT_FOUND => "that directory or drive is not there".into(),
        FsError::ACCESS_DENIED => {
            "the palmtop refused it: a read-only file, or a directory that is not empty".into()
        }
        FsError::WRITE_PROTECTED => {
            "the drive is write-protected (D: is the palmtop's ROM disk)".into()
        }
        FsError::NOT_READY => "the drive is not ready: is there a card in the slot?".into(),
        FsError::OTHER_DRIVE => "a file moves only within one drive".into(),
        FsError::EXISTS => "the name is in use".into(),
        FsError::CANNOT_MKDIR => {
            "the palmtop could not make the directory (the name in use?)".into()
        }
        e => format!("the palmtop refused it: {e}"),
    }
}

/// The device's result type.
pub type Result<T> = std::result::Result<T, PalmtopError>;

#[derive(Debug)]
enum Running {
    Connect { baud: u32 },
    List { dir: Vec<String> },
    Get { name: String },
    Put { name: String },
    Rename { from: String, to: String },
    MakeDir { name: String },
    Remove(Box<Remove>),
}

/// A removal: a file, or a directory walked first (every level listed),
/// then its files deleted and its directories removed, deepest first.
#[derive(Debug)]
struct Remove {
    name: String,
    phase: Phase,
    /// Directories still to list.
    to_list: Vec<Vec<u8>>,
    /// The files found.
    files: Vec<Vec<u8>>,
    /// The directories found, each before what is in it (the root first).
    dirs: Vec<Vec<u8>>,
    /// The path of the step on the line, for its error.
    at: Vec<u8>,
    /// What the directory held.
    count: usize,
    /// A read-only file the walk saw (from the directory down).
    read_only: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Phase {
    Listing,
    Files,
    Dirs,
}

/// A palmtop on a link, without I/O. See the [module documentation](self).
pub struct Palmtop {
    session: Option<Session>,
    connection: Option<PalmtopConnection>,
    running: Option<Running>,
    actions: VecDeque<Action>,
    events: VecDeque<PalmtopEvent>,
    result: Option<Result<PalmtopOutcome>>,
    not_answering: bool,
    /// [`Palmtop::stop`] asked the running request to end.
    stopping: bool,
    /// [`Palmtop::abort`] left the palmtop's session in an unknown step:
    /// the next request connects again first (CONNECT resets it).
    reconnect: bool,
    /// The request waiting for that reconnect.
    after_reconnect: Option<PalmtopRequest>,
    /// The running request, for a rerun after the palmtop's session ended.
    current: Option<PalmtopRequest>,
    /// A reply of the running request came.
    answered: bool,
    /// The running request is already the rerun.
    rerun: bool,
    /// The wait for a reply's first byte, when not HP's 5.9 s (tests).
    reply_wait: Option<Duration>,
}

impl fmt::Debug for Palmtop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Palmtop")
            .field("connection", &self.connection)
            .field("running", &self.running)
            .field("not_answering", &self.not_answering)
            .finish_non_exhaustive()
    }
}

impl Default for Palmtop {
    fn default() -> Self {
        Self::new()
    }
}

/// A DOS path from a directory and a name: `C:\_DAT\NOTES.TXT`; `None`
/// when the directory has no drive or a part is not Latin-1.
fn dos_path(dir: &[String], name: Option<&str>) -> Option<Vec<u8>> {
    let drive = dir.first()?;
    let d = drive.as_bytes();
    if d.len() != 2 || !d[0].is_ascii_alphabetic() || d[1] != b':' {
        return None;
    }
    let mut out = vec![d[0].to_ascii_uppercase(), b':', b'\\'];
    let latin1 = |s: &str| -> Option<Vec<u8>> {
        s.chars()
            .map(|c| u8::try_from(u32::from(c)).ok())
            .collect::<Option<Vec<u8>>>()
    };
    for part in dir[1..].iter().map(String::as_str).chain(name) {
        // Never a step up, the current directory, another drive or a
        // pattern: a removal walks what this names.
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains(['\\', '/', ':', '*', '?'])
        {
            return None;
        }
        out = path::join(&out, &latin1(part)?);
    }
    Some(out)
}

/// The listing pattern of a directory: `C:\_DAT\*.*`.
fn pattern(dir: &[u8]) -> Vec<u8> {
    path::join(dir, b"*.*")
}

impl Palmtop {
    /// A palmtop for a link the host is about to open (the first request,
    /// [`PalmtopRequest::Connect`], says at which speed).
    pub fn new() -> Self {
        Palmtop {
            session: None,
            connection: None,
            running: None,
            actions: VecDeque::new(),
            events: VecDeque::new(),
            result: None,
            not_answering: false,
            stopping: false,
            reconnect: false,
            after_reconnect: None,
            current: None,
            answered: false,
            rerun: false,
            reply_wait: None,
        }
    }

    /// A palmtop that waits `wait` (instead of HP's 5.9 s) for the first
    /// byte of each reply but CONNECT's, so it counts as not answering
    /// after three of them (tests).
    pub fn with_reply_wait(wait: Duration) -> Self {
        Palmtop {
            reply_wait: Some(wait),
            ..Self::new()
        }
    }

    /// The connected palmtop.
    pub fn connection(&self) -> Option<PalmtopConnection> {
        self.connection
    }

    /// Whether a request runs.
    pub fn is_busy(&self) -> bool {
        self.running.is_some()
    }

    /// Whether the palmtop stopped answering since the last connect
    /// ([`PalmtopError::NotAnswering`]).
    pub fn not_answering(&self) -> bool {
        self.not_answering
    }

    /// The next thing to do for the device.
    pub fn poll_action(&mut self) -> Option<Action> {
        self.actions.pop_front()
    }

    /// The next event.
    pub fn poll_event(&mut self) -> Option<PalmtopEvent> {
        self.events.pop_front()
    }

    /// The result of the last request, once it is over.
    pub fn poll_result(&mut self) -> Option<Result<PalmtopOutcome>> {
        self.result.take()
    }

    /// When to call [`Palmtop::handle_timeout`] at the latest; `None` while
    /// nothing waits.
    pub fn next_timeout(&self) -> Option<Instant> {
        self.running.as_ref()?;
        self.session.as_ref()?.next_timeout()
    }

    /// Begin `request`; its first actions are ready at once. A request
    /// refused before anything is sent (busy, a bad name) gives its
    /// result at once too.
    pub fn start(&mut self, now: Instant, request: PalmtopRequest) -> Result<()> {
        if self.running.is_some() {
            return Err(PalmtopError::Busy);
        }
        self.result = None;
        self.stopping = false;
        self.current = None;
        self.rerun = false;
        if let Err(e) = self.begin(now, request) {
            self.result = Some(Err(e));
        }
        // A job may end at once (the drives of a revision-1 Filer).
        self.pump(now);
        Ok(())
    }

    fn begin(&mut self, now: Instant, request: PalmtopRequest) -> Result<()> {
        if let PalmtopRequest::Connect { baud } = request {
            if self.connection.is_some() {
                return Err(PalmtopError::AlreadyConnected);
            }
            if !SPEEDS.contains(&baud) {
                return Err(PalmtopError::BadSpeed(baud));
            }
            return self.connect(now, baud);
        }
        if request == PalmtopRequest::Disconnect {
            if let Some(session) = self.session.as_mut() {
                session.disconnect(now);
            }
            self.drain_session(now);
            self.session = None;
            self.connection = None;
            self.reconnect = false;
            self.result = Some(Ok(PalmtopOutcome::Disconnected));
            return Ok(());
        }
        let Some(connection) = self.connection else {
            return Err(if self.not_answering {
                PalmtopError::NotAnswering
            } else {
                PalmtopError::NotConnected
            });
        };
        if self.reconnect {
            // Checked before the CONNECT, so a refusal costs nothing.
            check(&request)?;
            self.reconnect = false;
            self.after_reconnect = Some(request);
            return self.connect(now, connection.baud);
        }
        self.request(now, request)
    }

    fn connect(&mut self, now: Instant, baud: u32) -> Result<()> {
        let mut config = Config::default();
        config.baud = baud;
        if let Some(wait) = self.reply_wait {
            config.reply_wait = wait;
        }
        let mut session = Session::new(config);
        self.actions.push_back(Action::Open(LinkSettings {
            baud,
            parity: Parity::None,
        }));
        session
            .start(now, Job::Connect)
            .map_err(|e| PalmtopError::Link(e.to_string()))?;
        self.session = Some(session);
        self.running = Some(Running::Connect { baud });
        Ok(())
    }

    /// Send the first job of `request` on the open session.
    fn request(&mut self, now: Instant, request: PalmtopRequest) -> Result<()> {
        check(&request)?;
        self.current = Some(request.clone());
        self.answered = false;
        let bad = || PalmtopError::NotFound(request.describe());
        let (running, job) = match &request {
            PalmtopRequest::List { dir } if dir.is_empty() => (
                Running::List { dir: Vec::new() },
                Job::Drives { volumes: false },
            ),
            PalmtopRequest::List { dir } => {
                let p = dos_path(dir, None).ok_or_else(bad)?;
                (Running::List { dir: dir.clone() }, Job::List(pattern(&p)))
            }
            PalmtopRequest::Get { dir, name } => {
                let p = dos_path(dir, Some(name)).ok_or_else(bad)?;
                (Running::Get { name: name.clone() }, Job::Get(p))
            }
            PalmtopRequest::Put {
                dir,
                name,
                data,
                replace,
            } => {
                let stored = dos_name(name).map_err(|problem| PalmtopError::BadName {
                    name: name.clone(),
                    problem,
                })?;
                let p = dos_path(dir, Some(&stored)).ok_or_else(bad)?;
                (
                    Running::Put { name: stored },
                    Job::Put {
                        path: p,
                        data: data.clone(),
                        modified: None,
                        overwrite: *replace,
                    },
                )
            }
            PalmtopRequest::Rename { dir, from, to } => {
                let made = dos_name(to).map_err(|problem| PalmtopError::BadName {
                    name: to.clone(),
                    problem,
                })?;
                let old = dos_path(dir, Some(from)).ok_or_else(bad)?;
                let new = dos_path(dir, Some(&made)).ok_or_else(bad)?;
                if old.eq_ignore_ascii_case(&new) {
                    // The same DOS name: nothing to do.
                    self.result = Some(Ok(PalmtopOutcome::Renamed {
                        from: from.clone(),
                        to: from.clone(),
                    }));
                    return Ok(());
                }
                (
                    Running::Rename {
                        from: from.clone(),
                        to: made,
                    },
                    Job::Rename { from: old, to: new },
                )
            }
            PalmtopRequest::MakeDir { dir, name } => {
                let made = dos_name(name).map_err(|problem| PalmtopError::BadName {
                    name: name.clone(),
                    problem,
                })?;
                let p = dos_path(dir, Some(&made)).ok_or_else(bad)?;
                (Running::MakeDir { name: made }, Job::MakeDir(p))
            }
            PalmtopRequest::Remove {
                dir,
                name,
                directory,
            } => {
                let p = dos_path(dir, Some(name)).ok_or_else(bad)?;
                let mut remove = Remove {
                    name: name.clone(),
                    phase: Phase::Files,
                    to_list: Vec::new(),
                    files: Vec::new(),
                    dirs: Vec::new(),
                    at: p.clone(),
                    count: 0,
                    read_only: None,
                };
                let job = if *directory {
                    remove.phase = Phase::Listing;
                    remove.dirs.push(p.clone());
                    Job::List(pattern(&p))
                } else {
                    Job::Delete(p)
                };
                (Running::Remove(Box::new(remove)), job)
            }
            PalmtopRequest::Connect { .. } | PalmtopRequest::Disconnect => {
                return Err(PalmtopError::AlreadyConnected);
            }
        };
        self.send(now, running, job)
    }

    fn send(&mut self, now: Instant, running: Running, job: Job) -> Result<()> {
        let session = self.session.as_mut().ok_or(PalmtopError::NotConnected)?;
        session
            .start(now, job)
            .map_err(|e| PalmtopError::Link(e.to_string()))?;
        self.running = Some(running);
        Ok(())
    }

    /// Feed bytes read from the link at `now`.
    pub fn handle_input(&mut self, now: Instant, bytes: &[u8]) {
        if let Some(session) = self.session.as_mut() {
            // One reply at most per call: the next request goes out only
            // after it.
            let counter = session.client().counter();
            session.handle_input(now, bytes);
            if session.is_open() && session.client().counter() != counter {
                self.answered = true;
            }
        }
        self.pump(now);
    }

    /// Tell the device the time is `now`.
    pub fn handle_timeout(&mut self, now: Instant) {
        if let Some(session) = self.session.as_mut() {
            session.handle_timeout(now);
        }
        self.pump(now);
    }

    /// The link failed or closed: the running request fails, the session
    /// is over.
    pub fn handle_link_error(&mut self, now: Instant, message: &str) {
        let _ = now;
        self.session = None;
        self.connection = None;
        self.reconnect = false;
        self.after_reconnect = None;
        self.current = None;
        if self.running.take().is_some() {
            self.result = Some(Err(PalmtopError::Link(message.to_string())));
        }
    }

    /// Stop the running request at its next reply (see the [module
    /// documentation](self)); it fails with [`PalmtopError::Stopped`]. A
    /// connect is dropped at once. No-op when idle.
    pub fn stop(&mut self, now: Instant) {
        match self.running {
            None => {}
            Some(Running::Connect { .. }) => {
                self.running = None;
                self.session = None;
                self.after_reconnect = None;
                self.reconnect = self.connection.is_some();
                self.result = Some(Err(PalmtopError::Stopped));
            }
            Some(_) => {
                self.stopping = true;
                if let Some(session) = self.session.as_mut() {
                    session.cancel();
                }
            }
        }
        self.drain_session(now);
    }

    /// Drop the running request at once, without waiting for the palmtop:
    /// one DISCONNECT goes out and the next request connects again first
    /// (a file left open there is closed by that). No-op when idle.
    pub fn abort(&mut self, now: Instant) {
        if self.running.take().is_none() {
            return;
        }
        self.events.clear();
        self.result = None;
        self.after_reconnect = None;
        if let Some(session) = self.session.as_mut() {
            session.disconnect(now);
        }
        self.drain_session(now);
        self.reconnect = self.connection.is_some();
    }

    /// Move the session's frames to the actions.
    fn drain_session(&mut self, now: Instant) {
        if let Some(session) = self.session.as_mut() {
            while let Some(frame) = session.poll_output(now) {
                self.actions.push_back(Action::Write(frame));
            }
        }
    }

    fn pump(&mut self, now: Instant) {
        while let Some(event) = self.session.as_mut().and_then(Session::poll_event) {
            match event {
                SessionEvent::Progress { done, total } => {
                    if matches!(
                        self.running,
                        Some(Running::Get { .. } | Running::Put { .. })
                    ) {
                        self.events
                            .push_back(PalmtopEvent::Progress { done, total });
                    }
                }
                SessionEvent::Finished(result) => self.finished(now, result),
            }
        }
        self.drain_session(now);
    }

    /// The session's job ended.
    fn finished(&mut self, now: Instant, result: std::result::Result<Output, JobError>) {
        let Some(running) = self.running.take() else {
            return;
        };
        if let Err(JobError::Link(e)) = &result {
            // The session is over: CONNECT failed, a request went
            // unanswered, or the palmtop had ended its session.
            self.session = None;
            self.after_reconnect = None;
            let current = self.current.take();
            match running {
                Running::Connect { baud } => {
                    let reconnecting = self.connection.is_some();
                    self.connection = None;
                    self.result = Some(Err(match e {
                        filer_proto::Error::Refused(code) => {
                            PalmtopError::Refused(format!("status {code:#06x}"))
                        }
                        _ if reconnecting => {
                            self.not_answering = true;
                            PalmtopError::NotAnswering
                        }
                        _ => PalmtopError::NoAnswer { baud },
                    }));
                }
                _ => {
                    // The palmtop ran nothing of the request: connect again
                    // and send it once more.
                    if *e == filer_proto::Error::NotConnected
                        && !self.answered
                        && !self.rerun
                        && !self.stopping
                        && let (Some(connection), Some(request)) = (self.connection, current)
                    {
                        self.rerun = true;
                        self.after_reconnect = Some(request);
                        match self.connect(now, connection.baud) {
                            Ok(()) => return,
                            Err(err) => {
                                self.after_reconnect = None;
                                self.connection = None;
                                self.result = Some(Err(err));
                                return;
                            }
                        }
                    }
                    self.connection = None;
                    self.not_answering = true;
                    self.result = Some(Err(PalmtopError::NotAnswering));
                }
            }
            return;
        }
        if self.stopping && matches!(result, Err(JobError::Cancelled)) {
            self.result = Some(Err(PalmtopError::Stopped));
            return;
        }
        let outcome = match running {
            Running::Connect { baud } => match result {
                Ok(Output::Connected { revision }) => {
                    self.connection = Some(PalmtopConnection { baud, revision });
                    self.not_answering = false;
                    if let Some(request) = self.after_reconnect.take() {
                        if let Err(e) = self.request(now, request) {
                            self.result = Some(Err(e));
                        }
                        return;
                    }
                    Ok(PalmtopOutcome::Connected(PalmtopConnection {
                        baud,
                        revision,
                    }))
                }
                Ok(_) => Err(PalmtopError::Refused("an unexpected reply".into())),
                Err(e) => Err(PalmtopError::Refused(e.to_string())),
            },
            Running::List { dir } => match result {
                Ok(Output::Drives(drives)) => Ok(PalmtopOutcome::Listing {
                    dir,
                    entries: drives
                        .iter()
                        .map(|d| PalmtopEntry {
                            name: format!("{}:", char::from(d.letter)),
                            directory: true,
                            drive: true,
                            size: 0,
                            modified: None,
                            attributes: attr::DIRECTORY,
                        })
                        .collect(),
                }),
                Ok(Output::Listing(entries)) => Ok(PalmtopOutcome::Listing {
                    dir,
                    entries: entries.iter().map(PalmtopEntry::from_dir_entry).collect(),
                }),
                Ok(_) => Err(unexpected(&dir.join("\\"))),
                Err(e) => Err(job_error(e, &dir.join("\\"))),
            },
            Running::Get { name } => match result {
                Ok(Output::File { data, modified }) => Ok(PalmtopOutcome::File {
                    name,
                    data,
                    modified,
                }),
                Ok(_) => Err(unexpected(&name)),
                Err(e) => Err(job_error(e, &name)),
            },
            Running::Put { name } => match result {
                Ok(_) => Ok(PalmtopOutcome::Stored(name)),
                Err(e) => Err(job_error(e, &name)),
            },
            Running::Rename { from, to } => match result {
                Ok(_) => Ok(PalmtopOutcome::Renamed { from, to }),
                Err(JobError::Exists) => Err(PalmtopError::Exists(to)),
                Err(e) => Err(job_error(e, &from)),
            },
            Running::MakeDir { name } => match result {
                Ok(_) => Ok(PalmtopOutcome::Created(name)),
                // Made over a name in use.
                Err(JobError::Failed(Failure::Fs(
                    FsError::CANNOT_MKDIR | FsError::ACCESS_DENIED | FsError::EXISTS,
                ))) => Err(PalmtopError::Exists(name)),
                Err(e) => Err(job_error(e, &name)),
            },
            Running::Remove(remove) => match self.remove_step(now, remove, result) {
                Some(done) => done,
                None => return,
            },
        };
        self.result = Some(outcome);
    }

    /// The next step of a removal, or its result (`None`: a step is on
    /// the line).
    fn remove_step(
        &mut self,
        now: Instant,
        mut r: Box<Remove>,
        result: std::result::Result<Output, JobError>,
    ) -> Option<Result<PalmtopOutcome>> {
        let at = path::display(path::file_name(&r.at));
        match result {
            Ok(Output::Listing(entries)) if r.phase == Phase::Listing => {
                // The removed directory's own path, to name what is in it.
                let root_len = r.dirs.first().map_or(0, Vec::len);
                for e in &entries {
                    // Never a step up or across, whatever the palmtop sends.
                    if e.name.is_empty()
                        || e.name == b"."
                        || e.name == b".."
                        || e.name.iter().any(|&b| matches!(b, b'\\' | b'/' | b':'))
                    {
                        continue;
                    }
                    let p = path::join(&r.at, &e.name);
                    r.count += 1;
                    if !e.is_directory()
                        && e.attributes & attr::READ_ONLY != 0
                        && r.read_only.is_none()
                    {
                        r.read_only = Some(path::display(p.get(root_len + 1..).unwrap_or(&p)));
                    }
                    if e.is_directory() {
                        r.dirs.push(p.clone());
                        r.to_list.push(p);
                    } else {
                        r.files.push(p);
                    }
                }
            }
            Ok(_) => {}
            Err(JobError::Failed(Failure::Fs(FsError::ACCESS_DENIED)))
                if r.phase == Phase::Dirs =>
            {
                return Some(Err(PalmtopError::HiddenInside(at)));
            }
            Err(e) => return Some(Err(job_error(e, &at))),
        }
        if self.stopping {
            return Some(Err(PalmtopError::Stopped));
        }
        if r.phase == Phase::Listing {
            if let Some(next) = r.to_list.pop() {
                r.at = next;
                let job = Job::List(pattern(&r.at));
                return self.remove_send(now, r, job);
            }
            if let Some(file) = r.read_only.take() {
                return Some(Err(PalmtopError::ReadOnlyInside { dir: r.name, file }));
            }
            r.phase = Phase::Files;
        }
        if r.phase == Phase::Files {
            if let Some(file) = r.files.pop() {
                r.at = file.clone();
                return self.remove_send(now, r, Job::Delete(file));
            }
            if r.dirs.is_empty() {
                // A file's removal is over.
                return Some(Ok(PalmtopOutcome::Removed {
                    name: r.name,
                    count: r.count,
                }));
            }
            r.phase = Phase::Dirs;
        }
        match r.dirs.pop() {
            Some(dir) => {
                r.at = dir.clone();
                self.remove_send(now, r, Job::RemoveDir(dir))
            }
            None => Some(Ok(PalmtopOutcome::Removed {
                name: r.name,
                count: r.count,
            })),
        }
    }

    fn remove_send(
        &mut self,
        now: Instant,
        r: Box<Remove>,
        job: Job,
    ) -> Option<Result<PalmtopOutcome>> {
        match self.send(now, Running::Remove(r), job) {
            Ok(()) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

/// The checks of a request before anything is sent: the names it makes.
fn check(request: &PalmtopRequest) -> Result<()> {
    let made = match request {
        PalmtopRequest::Put { name, .. } | PalmtopRequest::MakeDir { name, .. } => name,
        PalmtopRequest::Rename { to, .. } => to,
        _ => return Ok(()),
    };
    dos_name(made)
        .map(|_| ())
        .map_err(|problem| PalmtopError::BadName {
            name: made.clone(),
            problem,
        })
}

fn unexpected(name: &str) -> PalmtopError {
    PalmtopError::Status {
        name: name.to_string(),
        failure: "an unexpected reply".into(),
    }
}

/// A failed job (the session open) as the device's error about `name`.
fn job_error(e: JobError, name: &str) -> PalmtopError {
    match e {
        JobError::Exists => PalmtopError::Exists(name.to_string()),
        JobError::TooLarge { .. } => PalmtopError::TooLarge(name.to_string()),
        JobError::Cancelled => PalmtopError::Stopped,
        JobError::Failed(Failure::Fs(FsError::NOT_FOUND)) => {
            PalmtopError::NotFound(name.to_string())
        }
        JobError::Failed(Failure::Fs(FsError::EXISTS)) => PalmtopError::Exists(name.to_string()),
        JobError::Failed(Failure::Fs(error)) => PalmtopError::Fs {
            name: name.to_string(),
            error,
        },
        JobError::Failed(Failure::Status(code)) => PalmtopError::Status {
            name: name.to_string(),
            failure: Failure::Status(code).to_string(),
        },
        JobError::Link(_) => PalmtopError::NotAnswering,
        other => PalmtopError::Status {
            name: name.to_string(),
            failure: other.to_string(),
        },
    }
}

#[cfg(test)]
#[path = "palmtop_tests.rs"]
mod tests;
