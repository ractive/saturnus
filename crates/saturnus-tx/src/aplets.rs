//! The 38G, 39G and 40G "disk drive": a folder served to the calculator,
//! which drives every transfer as a Kermit client (wiki:
//! protocols/aplet-transfer).
//!
//! - [`Directory`]: the directory files `HP38DIR.CUR` and `HP38DIR.000`
//!   (`HP39DIR.…` on the 39G and 40G), parsed and written.
//! - [`Header`]: what a calculator file says about itself (family, kind
//!   letter, title).
//! - [`listing`]: the directory file the calculator gets, from the folder's
//!   files and the copy it wrote there last.
//! - [`ApletServer`]: the folder served over a link: `kermit-proto`'s
//!   [`Server`] answering the calculator's requests, the directory files
//!   made from the folder, received files handed to the host to store, and
//!   a binary aplet sent by XModem ("HP39/40 (Wire)", 39G and 40G).
//!
//! No I/O and no clock, as the rest of the crate: the host owns the folder
//! and the link.

use std::collections::VecDeque;
use std::fmt;

use kermit_proto::{OutgoingFile, Server, ServerConfig, ServerEvent};
use satx_core::Model;
use satx_core::time::{Duration, Instant};
use satx_core::xmodem_proto::{self, BlockSize, Check};

/// The two families of directory files and aplets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    /// The 38G: `HP38DIR.CUR`, `HP38Bin`, `HP38Asc`.
    Hp38,
    /// The 39G and 40G: `HP39DIR.CUR`, `HP39Bin`, `HP39Asc`.
    Hp39,
}

impl Family {
    /// `HP38` or `HP39`.
    pub fn prefix(self) -> &'static str {
        match self {
            Family::Hp38 => "HP38",
            Family::Hp39 => "HP39",
        }
    }

    /// The model the family stands for (the 39G and 40G are one).
    pub fn model(self) -> Model {
        match self {
            Family::Hp38 => Model::Hp38G,
            Family::Hp39 => Model::Hp39G,
        }
    }

    /// The family of `model`, if it has one.
    pub fn of(model: Model) -> Option<Family> {
        match model {
            Model::Hp38G => Some(Family::Hp38),
            Model::Hp39G => Some(Family::Hp39),
            _ => None,
        }
    }

    /// The pointer to the current directory, `HP38DIR.CUR`.
    pub fn cur_name(self) -> String {
        format!("{}DIR.CUR", self.prefix())
    }

    /// The first directory file, `HP38DIR.000`.
    pub fn dir_name(self) -> String {
        format!("{}DIR.000", self.prefix())
    }

    fn from_prefix(bytes: &[u8]) -> Option<Family> {
        match bytes.get(..4)? {
            b"HP38" => Some(Family::Hp38),
            b"HP39" => Some(Family::Hp39),
            _ => None,
        }
    }
}

/// Whether `name` is a directory file (`HP38DIR.` or `HP39DIR.` and
/// anything after, any case), and of which family.
pub fn directory_file(name: &str) -> Option<Family> {
    match Model::from_directory_file(name.as_bytes())? {
        Model::Hp38G => Some(Family::Hp38),
        Model::Hp39G => Some(Family::Hp39),
        _ => None,
    }
}

/// What the first bytes of a calculator file say: `HP38Bin` or `HP39Bin`
/// (binary) or `…Asc` (text), a kind letter (`B` an aplet, `E` a list,
/// `A` a directory file), and for a binary file the title (one length
/// byte, then the title).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// The calculator family.
    pub family: Family,
    /// `true` for `…Bin`, `false` for `…Asc`.
    pub binary: bool,
    /// The kind letter, the 8th byte.
    pub kind: u8,
    /// The title of a binary file; `None` for text or when cut short.
    pub title: Option<String>,
}

impl Header {
    /// Read the header at the start of `bytes`; `None` when it is not one.
    pub fn read(bytes: &[u8]) -> Option<Header> {
        let family = Family::from_prefix(bytes)?;
        let binary = match bytes.get(4..7)? {
            b"Bin" => true,
            b"Asc" => false,
            _ => return None,
        };
        let kind = *bytes.get(7)?;
        if !kind.is_ascii_uppercase() {
            return None;
        }
        let title = if binary {
            let len = usize::from(*bytes.get(8)?);
            bytes
                .get(9..9 + len)
                .filter(|t| t.iter().all(|&b| b >= 0x20))
                .map(latin1)
        } else {
            None
        };
        Some(Header {
            family,
            binary,
            kind,
            title,
        })
    }

    /// An aplet the 39G or 40G takes by XModem: a binary aplet (`HP39BinB`).
    pub fn is_wire_aplet(&self) -> bool {
        self.family == Family::Hp39 && self.binary && self.kind == b'B'
    }
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

fn latin1_bytes(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

/// One entry of a directory file: a kind letter, a file name and a title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The kind letter (the 8th byte of the file it names).
    pub kind: u8,
    /// The file name, e.g. `FUNCTION.000`.
    pub file: String,
    /// The title the calculator lists, e.g. `Function`.
    pub title: String,
}

/// A directory file: `HP38AscA ` (its family and kind `A`), then entries
/// with nothing between them, each a kind letter, a space, and the file
/// name and the title, each counted (`12 FUNCTION.000`, `8 Function`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directory {
    /// Its family.
    pub family: Family,
    /// The entries, in their order (the calculator puts new ones first).
    pub entries: Vec<Entry>,
}

/// Why a directory file did not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirError(pub String);

impl fmt::Display for DirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not a directory file: {}", self.0)
    }
}

impl std::error::Error for DirError {}

impl Directory {
    /// An empty directory of `family`.
    pub fn new(family: Family) -> Self {
        Directory {
            family,
            entries: Vec::new(),
        }
    }

    /// The `HP38DIR.CUR` the calculator writes when it initialises a disk
    /// drive: one entry, kind `A`, naming `HP38DIR.000` titled `Root`.
    pub fn current(family: Family) -> Self {
        Directory {
            family,
            entries: vec![Entry {
                kind: b'A',
                file: family.dir_name(),
                title: "Root".to_string(),
            }],
        }
    }

    /// Parse a directory file.
    pub fn parse(bytes: &[u8]) -> Result<Directory, DirError> {
        let header = Header::read(bytes).ok_or_else(|| DirError("no header".into()))?;
        if header.binary || header.kind != b'A' {
            return Err(DirError("not an HP38AscA or HP39AscA header".into()));
        }
        let mut rest = bytes.get(8..).unwrap_or_default();
        rest = rest.strip_prefix(b" ").unwrap_or(rest);
        let mut entries = Vec::new();
        while !rest.is_empty() {
            let kind = rest[0];
            if !kind.is_ascii_uppercase() || rest.get(1) != Some(&b' ') {
                return Err(DirError(format!(
                    "bad entry at {}",
                    bytes.len() - rest.len()
                )));
            }
            let (file, after) = counted(&rest[2..])?;
            let (title, after) = counted(after)?;
            entries.push(Entry {
                kind,
                file: latin1(file),
                title: latin1(title),
            });
            rest = after;
        }
        Ok(Directory {
            family: header.family,
            entries,
        })
    }

    /// The file's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = format!("{}AscA ", self.family.prefix()).into_bytes();
        for e in &self.entries {
            out.push(e.kind);
            out.push(b' ');
            for text in [&e.file, &e.title] {
                let bytes = latin1_bytes(text);
                out.extend_from_slice(format!("{} ", bytes.len()).as_bytes());
                out.extend_from_slice(&bytes);
            }
        }
        out
    }
}

/// A counted string: decimal digits, a space, that many bytes.
fn counted(bytes: &[u8]) -> Result<(&[u8], &[u8]), DirError> {
    let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || bytes.get(digits) != Some(&b' ') {
        return Err(DirError("a count is missing".into()));
    }
    let len: usize = std::str::from_utf8(&bytes[..digits])
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| DirError("a count is out of range".into()))?;
    let start = digits + 1;
    let text = bytes
        .get(start..start + len)
        .ok_or_else(|| DirError("a string runs past the end".into()))?;
    Ok((text, &bytes[start + len..]))
}

/// A file in the served folder, as the host gives it: its name and its
/// first bytes (at least 264, the header and the longest title), or all of
/// it for a directory file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderFile {
    /// The file name in the folder.
    pub name: String,
    /// Its first bytes (all of a directory file).
    pub head: Vec<u8>,
}

/// How many bytes of a file [`FolderFile::head`] needs (the 8-byte header,
/// the length byte and a title of up to 255 bytes).
pub const HEAD_LEN: usize = 264;

/// Whether the calculator can ask for `name`: an 8.3 name of printable
/// ASCII without spaces (the names it makes itself are like `FUNCTION.000`).
pub fn servable_name(name: &str) -> bool {
    let (stem, ext) = name.split_once('.').unwrap_or((name, ""));
    !stem.is_empty()
        && stem.len() <= 8
        && ext.len() <= 3
        && !ext.contains('.')
        && name.bytes().all(|b| b.is_ascii_graphic())
}

/// The directory file `family`'s calculator gets: the entries of `stored`
/// (the copy it wrote, if it parses) whose files are in the folder, then the
/// folder's other files of the family with a calculator header and a
/// servable name, by name; their title is the file's own, or the name
/// without its extension.
pub fn listing(family: Family, files: &[FolderFile], stored: Option<&[u8]>) -> Directory {
    let find = |name: &str| files.iter().find(|f| f.name.eq_ignore_ascii_case(name));
    let mut dir = Directory::new(family);
    if let Some(old) = stored.and_then(|s| Directory::parse(s).ok()) {
        for e in old.entries {
            let listed = dir
                .entries
                .iter()
                .any(|d| d.file.eq_ignore_ascii_case(&e.file));
            if !listed && directory_file(&e.file).is_none() && find(&e.file).is_some() {
                dir.entries.push(e);
            }
        }
    }
    let mut more: Vec<Entry> = files
        .iter()
        .filter(|f| directory_file(&f.name).is_none() && servable_name(&f.name))
        .filter(|f| {
            !dir.entries
                .iter()
                .any(|e| e.file.eq_ignore_ascii_case(&f.name))
        })
        .filter_map(|f| {
            let h = Header::read(&f.head)?;
            (h.family == family).then(|| Entry {
                kind: h.kind,
                file: f.name.clone(),
                title: h
                    .title
                    .unwrap_or_else(|| f.name.split('.').next().unwrap_or(&f.name).to_string()),
            })
        })
        .collect();
    more.sort_by(|a, b| a.file.cmp(&b.file));
    dir.entries.extend(more);
    dir
}

/// Tunables of an [`ApletServer`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ApletOptions {
    /// The Kermit server's.
    pub kermit: ServerConfig,
    /// How long an XModem send waits for the calculator's RECEIVE (default
    /// 120 s: the user walks to the calculator after the click).
    pub wire_start: Duration,
}

impl Default for ApletOptions {
    fn default() -> Self {
        ApletOptions {
            kermit: ServerConfig::default(),
            wire_start: Duration::from_secs(120),
        }
    }
}

/// What happened while serving.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApletEvent {
    /// The calculator's model, the first time a directory file tells it.
    Model(Model),
    /// The calculator asked for a file that is not a directory file: the
    /// host answers with [`ApletServer::provide`] (its bytes, or `None`).
    NeedFile {
        /// The folder's file (its own spelling).
        name: String,
    },
    /// A file goes to the calculator: progress.
    Sending {
        /// The name the calculator asked for.
        name: String,
        /// Bytes acknowledged.
        done: u64,
        /// Its size.
        total: u64,
    },
    /// A file reached the calculator.
    Sent {
        /// The name.
        name: String,
        /// It was a directory file.
        directory: bool,
    },
    /// The calculator asked for a file the folder does not have.
    NotFound {
        /// The name it asked for.
        name: String,
    },
    /// A file comes from the calculator: progress.
    Receiving {
        /// Its name.
        name: String,
        /// Bytes so far.
        done: u64,
    },
    /// A file came from the calculator: the host stores it in the folder
    /// under `name`, replacing a file of that name in any case.
    Received {
        /// The calculator's name for it.
        name: String,
        /// Its bytes.
        data: Vec<u8>,
        /// It is a directory file the calculator wrote.
        directory: bool,
    },
    /// The calculator stopped sending a file part way.
    Discarded {
        /// Its name.
        name: String,
    },
    /// An error message from the calculator (`Transfer Failed`).
    CalculatorSays(String),
    /// A transfer failed on this side (no answer, a broken packet).
    Failed(String),
    /// The XModem send began: the calculator's RECEIVE is waiting.
    WireStarted,
    /// XModem send progress.
    WireProgress {
        /// Bytes acknowledged.
        done: u64,
        /// The file's size.
        total: u64,
    },
    /// The XModem send ended: every block and the end acknowledged.
    WireDone,
    /// The XModem send failed or was cancelled.
    WireFailed(String),
}

/// Why the server refused a call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApletError {
    /// A transfer runs: an XModem send waits until it ends.
    Busy,
    /// [`ApletServer::provide`] without a request waiting for it.
    NoRequest,
}

impl fmt::Display for ApletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApletError::Busy => f.write_str("a transfer is running"),
            ApletError::NoRequest => f.write_str("the calculator is not waiting for a file"),
        }
    }
}

impl std::error::Error for ApletError {}

/// A folder served to a 38G, 39G or 40G over one link. Feed it the link's
/// bytes and the time, write what [`ApletServer::poll_output`] gives, and
/// act on its [`ApletEvent`]s: store what it received, answer
/// [`ApletEvent::NeedFile`].
#[derive(Debug)]
pub struct ApletServer {
    options: ApletOptions,
    kermit: Server,
    wire: Option<xmodem_proto::Transfer>,
    files: Vec<FolderFile>,
    model: Option<Model>,
    /// The request waiting for the host: the name the calculator asked for.
    asked: Option<String>,
    /// The file being sent or received, and whether it is a directory file.
    current: Option<(String, bool)>,
    events: VecDeque<ApletEvent>,
}

impl ApletServer {
    /// Serve a folder holding `files` (see [`FolderFile`]).
    pub fn new(options: ApletOptions, files: Vec<FolderFile>) -> Self {
        let kermit = Server::new(options.kermit.clone());
        ApletServer {
            options,
            kermit,
            wire: None,
            files,
            model: None,
            asked: None,
            current: None,
            events: VecDeque::new(),
        }
    }

    /// The folder changed on the host's side: its files now.
    pub fn set_files(&mut self, files: Vec<FolderFile>) {
        self.files = files;
    }

    /// The folder's files as the server knows them.
    pub fn files(&self) -> &[FolderFile] {
        &self.files
    }

    /// The calculator's model, once a directory file told it.
    pub fn model(&self) -> Option<Model> {
        self.model
    }

    /// A transfer runs (Kermit or XModem), or a request waits for the host.
    pub fn is_busy(&self) -> bool {
        !self.kermit.is_idle() || self.wire.is_some()
    }

    /// An XModem send waits or runs.
    pub fn is_wire(&self) -> bool {
        self.wire.is_some()
    }

    /// Feed bytes read from the link.
    pub fn handle_input(&mut self, now: Instant, bytes: &[u8]) {
        if let Some(wire) = self.wire.as_mut() {
            wire.handle_input(now, bytes);
            self.drain_wire();
        } else {
            self.kermit.handle_input(now, bytes);
            self.drain_kermit(now);
        }
    }

    /// Call when [`ApletServer::next_timeout`] has passed.
    pub fn handle_timeout(&mut self, now: Instant) {
        if let Some(wire) = self.wire.as_mut() {
            wire.handle_timeout(now);
            self.drain_wire();
        } else {
            self.kermit.handle_timeout(now);
            self.drain_kermit(now);
        }
    }

    /// Next bytes to write, each as one unit.
    pub fn poll_output(&mut self, now: Instant) -> Option<Vec<u8>> {
        if let Some(out) = self.kermit.poll_output(now) {
            return Some(out);
        }
        let out = self.wire.as_mut()?.poll_output(now);
        // A finished send is dropped once its last bytes are out.
        if out.is_none() && self.wire.as_ref().is_some_and(|w| w.is_idle()) {
            self.wire = None;
        }
        out
    }

    /// Next event.
    pub fn poll_event(&mut self) -> Option<ApletEvent> {
        self.events.pop_front()
    }

    /// When to call [`ApletServer::handle_timeout`]; `None` while the
    /// server only waits for the calculator.
    pub fn next_timeout(&self) -> Option<Instant> {
        let k = self.kermit.next_timeout();
        let w = self.wire.as_ref().and_then(|w| w.next_timeout());
        match (k, w) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Answer [`ApletEvent::NeedFile`]: the file's bytes, or `None` when it
    /// cannot be read (the calculator is told it is not there).
    pub fn provide(&mut self, now: Instant, data: Option<Vec<u8>>) -> Result<(), ApletError> {
        let name = self.asked.take().ok_or(ApletError::NoRequest)?;
        match data {
            Some(data) => self.serve(now, name, data, false),
            None => self.refuse(now, name),
        }
        Ok(())
    }

    /// Send `data`, a binary aplet, by XModem: the calculator then chooses
    /// RECEIVE, "HP39/40 (Wire)". Refused while a transfer runs.
    pub fn send_wire(&mut self, now: Instant, data: Vec<u8>) -> Result<(), ApletError> {
        if self.is_busy() {
            return Err(ApletError::Busy);
        }
        let mut config = xmodem_proto::Config::default();
        config.check = Check::Checksum;
        config.block_size = BlockSize::B128;
        config.pad = 0;
        config.start_timeout = self.options.wire_start;
        let mut wire = xmodem_proto::Transfer::new(config);
        wire.start(now, xmodem_proto::Command::Send(data))
            .map_err(|_| ApletError::Busy)?;
        self.wire = Some(wire);
        Ok(())
    }

    /// Stop what runs: an XModem send is cancelled (CANs), a Kermit
    /// transfer ends with an E packet.
    pub fn cancel(&mut self, now: Instant) {
        if let Some(wire) = self.wire.as_mut() {
            wire.cancel(now);
            self.drain_wire();
        }
        self.asked = None;
        self.kermit.cancel(now);
        self.drain_kermit(now);
    }

    fn serve(&mut self, now: Instant, name: String, data: Vec<u8>, directory: bool) {
        let file = OutgoingFile {
            name: name.clone().into_bytes(),
            data,
        };
        if self.kermit.serve(now, file).is_ok() {
            self.current = Some((name, directory));
        }
    }

    fn refuse(&mut self, now: Instant, name: String) {
        if self.kermit.refuse(now, b"File not found").is_ok() {
            self.events.push_back(ApletEvent::NotFound { name });
        }
    }

    fn find(&self, name: &str) -> Option<&FolderFile> {
        self.files
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// The directory file the calculator's `HP38DIR.CUR` points to.
    fn current_dir_name(&self, family: Family) -> String {
        self.find(&family.cur_name())
            .and_then(|f| Directory::parse(&f.head).ok())
            .and_then(|d| d.entries.into_iter().next())
            .map_or_else(|| family.dir_name(), |e| e.file)
    }

    fn on_request(&mut self, now: Instant, name: String) {
        if let Some(family) = directory_file(&name) {
            let model = family.model();
            if self.model != Some(model) {
                self.model = Some(model);
                self.events.push_back(ApletEvent::Model(model));
            }
            if name.eq_ignore_ascii_case(&family.cur_name()) {
                let data = self
                    .find(&name)
                    .filter(|f| Directory::parse(&f.head).is_ok_and(|d| d.entries.len() == 1))
                    .map_or_else(|| Directory::current(family).encode(), |f| f.head.clone());
                return self.serve(now, name, data, true);
            }
            if name.eq_ignore_ascii_case(&self.current_dir_name(family)) {
                let stored = self.find(&name).map(|f| f.head.clone());
                let data = listing(family, &self.files, stored.as_deref()).encode();
                return self.serve(now, name, data, true);
            }
        }
        match self.find(&name).map(|f| f.name.clone()) {
            Some(file) => {
                self.asked = Some(name);
                self.events.push_back(ApletEvent::NeedFile { name: file });
            }
            None => self.refuse(now, name),
        }
    }

    fn store(&mut self, name: &str, data: &[u8]) {
        let head = if directory_file(name).is_some() {
            data.to_vec()
        } else {
            data.get(..HEAD_LEN).unwrap_or(data).to_vec()
        };
        match self
            .files
            .iter_mut()
            .find(|f| f.name.eq_ignore_ascii_case(name))
        {
            Some(f) => {
                f.name = name.to_string();
                f.head = head;
            }
            None => self.files.push(FolderFile {
                name: name.to_string(),
                head,
            }),
        }
    }

    fn drain_kermit(&mut self, now: Instant) {
        while let Some(e) = self.kermit.poll_event() {
            match e {
                ServerEvent::Request { name } => self.on_request(now, latin1(&name)),
                ServerEvent::FileStart { name, sending } => {
                    if !sending {
                        let name = latin1(&name);
                        let family = directory_file(&name);
                        if let Some(family) = family
                            && self.model.is_none()
                        {
                            let model = family.model();
                            self.model = Some(model);
                            self.events.push_back(ApletEvent::Model(model));
                        }
                        self.current = Some((name, family.is_some()));
                    }
                }
                ServerEvent::Progress { done, total } => {
                    if let Some((name, _)) = &self.current {
                        let name = name.clone();
                        self.events.push_back(match total {
                            Some(total) => ApletEvent::Sending { name, done, total },
                            None => ApletEvent::Receiving { name, done },
                        });
                    }
                }
                ServerEvent::Received { name, data } => {
                    let name = latin1(&name);
                    let directory = directory_file(&name).is_some();
                    if self.model.is_none()
                        && let Some(h) = Header::read(&data)
                    {
                        let model = h.family.model();
                        self.model = Some(model);
                        self.events.push_back(ApletEvent::Model(model));
                    }
                    self.store(&name, &data);
                    self.current = None;
                    self.events.push_back(ApletEvent::Received {
                        name,
                        data,
                        directory,
                    });
                }
                ServerEvent::Discarded { name } => {
                    self.current = None;
                    self.events.push_back(ApletEvent::Discarded {
                        name: latin1(&name),
                    });
                }
                ServerEvent::Sent { .. } => {
                    if let Some((name, directory)) = self.current.take() {
                        self.events.push_back(ApletEvent::Sent { name, directory });
                    }
                }
                ServerEvent::Remote(text) => {
                    self.current = None;
                    self.asked = None;
                    let text = latin1(&text).trim_end().to_string();
                    self.events.push_back(ApletEvent::CalculatorSays(text));
                }
                ServerEvent::Error(err) => {
                    self.current = None;
                    self.asked = None;
                    self.events.push_back(ApletEvent::Failed(err.to_string()));
                }
                ServerEvent::Done => {}
                _ => {}
            }
        }
    }

    fn drain_wire(&mut self) {
        let Some(wire) = self.wire.as_mut() else {
            return;
        };
        while let Some(e) = wire.poll_event() {
            self.events.push_back(match e {
                xmodem_proto::Event::Started { .. } => ApletEvent::WireStarted,
                xmodem_proto::Event::Progress { bytes, total } => ApletEvent::WireProgress {
                    done: bytes,
                    total: total.unwrap_or(bytes),
                },
                xmodem_proto::Event::Done => ApletEvent::WireDone,
                xmodem_proto::Event::Error(err) => ApletEvent::WireFailed(err.to_string()),
                _ => continue,
            });
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const CUR: &[u8] = b"HP38AscA A 11 HP38DIR.0004 Root";
    const TWO: &[u8] = b"HP38AscA B 12 PARAMETR.00010 ParametricB 12 FUNCTION.0008 Function";

    fn file(name: &str, head: &[u8]) -> FolderFile {
        FolderFile {
            name: name.to_string(),
            head: head.to_vec(),
        }
    }

    fn aplet(family: &str, title: &str) -> Vec<u8> {
        let mut v = format!("{family}BinB").into_bytes();
        v.push(title.len() as u8);
        v.extend_from_slice(title.as_bytes());
        v.extend_from_slice(&[0x96, 0x2a, 0xf0]);
        v
    }

    #[test]
    fn directory_files_round_trip() {
        let cur = Directory::parse(CUR).unwrap();
        assert_eq!(cur, Directory::current(Family::Hp38));
        assert_eq!(cur.encode(), CUR);
        let two = Directory::parse(TWO).unwrap();
        assert_eq!(two.entries.len(), 2);
        assert_eq!(two.entries[0].file, "PARAMETR.000");
        assert_eq!(two.entries[0].title, "Parametric");
        assert_eq!(two.entries[1].kind, b'B');
        assert_eq!(two.encode(), TWO);
        let empty = Directory::parse(b"HP39AscA ").unwrap();
        assert_eq!(empty, Directory::new(Family::Hp39));
        assert_eq!(empty.encode(), b"HP39AscA ");
        // The 39G's list entry, kind E.
        let list = b"HP39AscA E 12 L1000000.0002 L1B 12 XYZ00000.0003 XYZ";
        assert_eq!(Directory::parse(list).unwrap().encode(), list);
    }

    #[test]
    fn bad_directory_files() {
        for bad in [
            &b"HP38BinB"[..],
            b"HP38AscA B 12 FUNCTION.000",
            b"HP38AscA B x FUNCTION.0008 Function",
            b"HP38AscA b 3 A.B1 C",
            b"hello",
        ] {
            assert!(Directory::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn headers() {
        let h = Header::read(&aplet("HP39", "XYZ")).unwrap();
        assert_eq!(
            h,
            Header {
                family: Family::Hp39,
                binary: true,
                kind: b'B',
                title: Some("XYZ".into())
            }
        );
        assert!(h.is_wire_aplet());
        assert!(!Header::read(&aplet("HP38", "XYZ")).unwrap().is_wire_aplet());
        assert_eq!(Header::read(CUR).unwrap().title, None);
        assert_eq!(Header::read(b"HP38BinB\x09Fun").unwrap().title, None);
        assert_eq!(Header::read(b"HPHP48-R"), None);
        assert_eq!(Header::read(b"HP39Bi"), None);
    }

    #[test]
    fn names() {
        assert!(servable_name("FUNCTION.000"));
        assert!(servable_name("game.apt"));
        assert!(servable_name("README"));
        assert!(!servable_name("my game.000"));
        assert!(!servable_name("TOOLONGNAME.000"));
        assert!(!servable_name("A.B.C"));
        assert!(!servable_name(".000"));
        assert_eq!(directory_file("hp39dir.cur"), Some(Family::Hp39));
        assert_eq!(directory_file("HP38DIR.000"), Some(Family::Hp38));
        assert_eq!(directory_file("HP40DIR.CUR"), None);
    }

    #[test]
    fn listing_merges_the_stored_copy_and_the_folder() {
        let files = vec![
            file("HP38DIR.CUR", CUR),
            file("HP38DIR.000", TWO),
            // PARAMETR.000 is gone from the folder.
            file("function.000", &aplet("HP38", "Function")),
            file("ZEBRA.000", &aplet("HP38", "Zebra")),
            file("APPLE.000", &aplet("HP38", "Apple")),
            file("OTHER.000", &aplet("HP39", "Other")),
            file("NOTES.TXT", b"hello"),
            file("long name.000", &aplet("HP38", "Long")),
        ];
        let dir = listing(Family::Hp38, &files, Some(TWO));
        let got: Vec<(&str, &str)> = dir
            .entries
            .iter()
            .map(|e| (e.file.as_str(), e.title.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("FUNCTION.000", "Function"),
                ("APPLE.000", "Apple"),
                ("ZEBRA.000", "Zebra")
            ]
        );
        let none = listing(Family::Hp39, &files, None);
        assert_eq!(none.entries.len(), 1);
        assert_eq!(none.entries[0].title, "Other");
        // A text file of the family, titled by its name.
        let text = listing(Family::Hp39, &[file("PROG.000", b"HP39AscP rest")], None);
        assert_eq!(text.entries[0].title, "PROG");
        assert_eq!(text.entries[0].kind, b'P');
    }

    /// Replay a recorded session (`just tx-aplet-traces`) on its own
    /// clock: every write must be the trace's next `>`; the folder starts
    /// empty and keeps what arrives; at the note `wire` the list goes out
    /// by XModem. Returns the events and the folder.
    fn replay(text: &str) -> (Vec<ApletEvent>, BTreeMap<String, Vec<u8>>) {
        use crate::trace::{Step, Trace};
        let trace = Trace::parse(text).unwrap();
        let base = Instant::now();
        let mut server = ApletServer::new(ApletOptions::default(), Vec::new());
        let mut folder: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut events = Vec::new();
        for (i, (at, step)) in trace.steps.iter().enumerate() {
            let now = base + *at;
            if server.next_timeout().is_some_and(|t| t <= now) {
                server.handle_timeout(now);
            }
            match step {
                Step::Note(n) if n.starts_with("wire") => {
                    let list = folder.get("L1000000.000").cloned().unwrap();
                    server.send_wire(now, list).unwrap();
                }
                Step::Note(_) | Step::Open(_) => {}
                Step::In(bytes) => server.handle_input(now, bytes),
                Step::Out(bytes) => {
                    let got = server.poll_output(now);
                    assert_eq!(
                        got.as_deref().map(kermit_proto::trace::escape),
                        Some(kermit_proto::trace::escape(bytes)),
                        "step {i}"
                    );
                }
            }
            while let Some(e) = server.poll_event() {
                match &e {
                    ApletEvent::NeedFile { name } => {
                        server.provide(now, folder.get(name).cloned()).unwrap();
                    }
                    ApletEvent::Received { name, data, .. } => {
                        folder.insert(name.clone(), data.clone());
                    }
                    _ => {}
                }
                events.push(e);
            }
        }
        assert_eq!(server.poll_output(base + Duration::from_secs(86_400)), None);
        (events, folder)
    }

    fn sent(events: &[ApletEvent]) -> Vec<&str> {
        events
            .iter()
            .filter_map(|e| match e {
                ApletEvent::Sent { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn g38_session() {
        let (events, folder) = replay(include_str!("../traces/38g-aplets.trace"));
        assert_eq!(events[0], ApletEvent::Model(Model::Hp38G));
        assert_eq!(
            sent(&events),
            [
                "HP38DIR.CUR",
                "HP38DIR.000",
                "HP38DIR.CUR",
                "HP38DIR.000",
                "L1000000.000"
            ]
        );
        let dir = Directory::parse(&folder["HP38DIR.000"]).unwrap();
        assert_eq!(
            dir.entries,
            [Entry {
                kind: b'E',
                file: "L1000000.000".into(),
                title: "L1".into()
            }]
        );
        assert!(folder["L1000000.000"].starts_with(b"HP38BinE\x02L1"));
        assert!(!events.iter().any(|e| matches!(
            e,
            ApletEvent::Failed(_) | ApletEvent::CalculatorSays(_) | ApletEvent::NotFound { .. }
        )));
    }

    #[test]
    fn g39_session_and_the_wire() {
        let (events, folder) = replay(include_str!("../traces/39g-aplets.trace"));
        assert_eq!(events[0], ApletEvent::Model(Model::Hp39G));
        assert_eq!(sent(&events).len(), 5);
        assert!(folder["L1000000.000"].starts_with(b"HP39BinE\x02L1"));
        let wire: Vec<&ApletEvent> = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    ApletEvent::WireStarted | ApletEvent::WireDone | ApletEvent::WireFailed(_)
                )
            })
            .collect();
        assert_eq!(wire, [&ApletEvent::WireStarted, &ApletEvent::WireDone]);
    }

    #[test]
    fn requests_answered() {
        let now = Instant::now();
        let files = vec![
            file("HP39DIR.CUR", b"HP39AscA A 11 HP39DIR.0014 Root"),
            file("GAME.000", &aplet("HP39", "Game")),
        ];
        let mut s = ApletServer::new(ApletOptions::default(), files);
        let wire = |seq: u8, kind: u8, data: &[u8]| {
            kermit_proto::codec::Packet::new(seq, kind, data.to_vec())
                .encode(kermit_proto::BlockCheck::Type1, &Default::default())
                .unwrap()
        };
        // The stored HP39DIR.CUR names HP39DIR.001: that is the listing.
        s.handle_input(now, &wire(0, b'R', b"HP39DIR.001"));
        assert_eq!(s.poll_event(), Some(ApletEvent::Model(Model::Hp39G)));
        assert!(s.is_busy());
        assert!(s.poll_output(now).unwrap().starts_with(b"\x01"));
        s.cancel(now);
        while s.poll_event().is_some() {}
        while s.poll_output(now).is_some() {}
        // A file in the folder, any case: the host is asked for it.
        s.handle_input(now, &wire(0, b'R', b"game.000"));
        assert_eq!(
            s.poll_event(),
            Some(ApletEvent::NeedFile {
                name: "GAME.000".into()
            })
        );
        assert_eq!(s.send_wire(now, vec![1]), Err(ApletError::Busy));
        s.provide(now, None).unwrap();
        assert_eq!(
            s.poll_event(),
            Some(ApletEvent::NotFound {
                name: "game.000".into()
            })
        );
        assert_eq!(s.provide(now, None), Err(ApletError::NoRequest));
        // Not there at all.
        s.handle_input(now, &wire(0, b'E', b"Transfer Failed#M#J"));
        assert_eq!(
            s.poll_event(),
            Some(ApletEvent::CalculatorSays("Transfer Failed".into()))
        );
        s.handle_input(now, &wire(0, b'R', b"NOPE.000"));
        assert_eq!(
            s.poll_event(),
            Some(ApletEvent::NotFound {
                name: "NOPE.000".into()
            })
        );
        assert!(!s.is_busy());
        assert!(s.send_wire(now, vec![1]).is_ok());
        assert!(s.is_wire());
    }
}
