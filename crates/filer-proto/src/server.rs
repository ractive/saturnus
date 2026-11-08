//! The server state machine, for tests: it answers a [`crate::Client`] the
//! way the notes describe the palmtop's Filer in server mode (wiki:
//! protocols/filer-hplx). The files live behind [`FileSystem`];
//! [`MemoryFs`](crate::MemoryFs) keeps them in memory.
//!
//! - A valid CONNECT (counter 0, direction 0x01) opens a session with all
//!   8 file slots free; inside a session it resets it and takes the
//!   client's revision byte. While no session is open every other frame
//!   gets a CONNECT reply carrying 0xFF3B.
//! - A request with the counter one more than the last runs; the same
//!   counter as the last gets the previous reply again without running
//!   the command; any other counter gets 0xFF3C with the last good
//!   counter.
//! - 0x02 gets no reply (the counter still advances); unknown codes, and
//!   0x40 or 0x41 with a non-zero counter, get 0xFF3E.
//! - An open or create with all 8 slots in use, or a frame whose body does
//!   not start with SOH, ends the session without a reply.
//! - Listings use a ring of 10 contexts: 0x12 moves to the next one, 0x09
//!   reads from the current one (the first 0x09 runs the search), 0x16
//!   steps back.

use std::collections::VecDeque;

use crate::codec::{DLE, DeframeError, Deframer, Frame, MAX_BLOCK, REQUEST, REVISION};
use crate::command::{
    Code, DirEntry, DosDateTime, NameKind, Request, Response, attr, encode_failure,
};
use crate::error::{Failure, FsError, status};

/// The files a [`Server`] serves. Paths are canonical: upper case, the
/// drive letter, a colon and a backslash-separated absolute path (`C:\`,
/// `C:\SUB\FILE.TXT`).
pub trait FileSystem {
    /// The drive letters, upper case.
    fn drives(&self) -> Vec<u8>;
    /// Volume label (11 bytes) and free bytes of a drive.
    fn volume(&self, drive: u8) -> Result<(Vec<u8>, u32), FsError>;
    /// What is at `path`.
    fn kind(&self, path: &[u8]) -> NameKind;
    /// The entries of the directory `dir`, as DOS lists them (with `.` and
    /// `..` below the root).
    fn list(&self, dir: &[u8]) -> Result<Vec<DirEntry>, FsError>;
    /// A file's contents and time.
    fn read(&self, path: &[u8]) -> Result<(Vec<u8>, DosDateTime), FsError>;
    /// Create an empty file; with `overwrite` an existing one is emptied,
    /// without it [`FsError::EXISTS`].
    fn create(&mut self, path: &[u8], overwrite: bool) -> Result<(), FsError>;
    /// Store a file's contents, with `modified` as its time (`None`: now).
    fn store(
        &mut self,
        path: &[u8],
        data: &[u8],
        modified: Option<DosDateTime>,
    ) -> Result<(), FsError>;
    /// Delete a file.
    fn delete(&mut self, path: &[u8]) -> Result<(), FsError>;
    /// Make a directory.
    fn make_dir(&mut self, path: &[u8]) -> Result<(), FsError>;
    /// Remove an empty directory.
    fn remove_dir(&mut self, path: &[u8]) -> Result<(), FsError>;
    /// Rename or move, within one drive.
    fn rename(&mut self, from: &[u8], to: &[u8]) -> Result<(), FsError>;
    /// A file's attribute byte.
    fn attributes(&self, path: &[u8]) -> Result<u8, FsError>;
    /// Set a file's attribute byte.
    fn set_attributes(&mut self, path: &[u8], attributes: u8) -> Result<(), FsError>;
}

/// Tunables of a [`Server`].
///
/// Non-exhaustive: start from [`ServerConfig::default`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ServerConfig {
    /// The revision the server speaks while idle (default 2). With 1, a
    /// CONNECT in revision 2 is refused with 0xFF3B, as an older peer
    /// would.
    pub revision: u8,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig { revision: REVISION }
    }
}

/// File slots per session.
pub const SLOTS: usize = 8;
/// Listing contexts in the ring.
pub const LISTINGS: usize = 10;

#[derive(Debug)]
struct Slot {
    path: Vec<u8>,
    data: Vec<u8>,
    position: usize,
    writing: bool,
    modified: Option<DosDateTime>,
}

#[derive(Debug, Default, Clone)]
struct Listing {
    /// Directory and pattern of the last 0x12 on this context.
    dir: Vec<u8>,
    pattern: Vec<u8>,
    /// The search result, run by the first 0x09.
    entries: Option<Result<Vec<DirEntry>, FsError>>,
    next: usize,
}

#[derive(Debug)]
struct Session {
    revision: u8,
    /// The counter of the last request run.
    counter: u8,
    /// The last reply, sent again for a repeated counter.
    last_reply: Option<Vec<u8>>,
    slots: [Option<Slot>; SLOTS],
    listings: Vec<Listing>,
    listing: usize,
    drive: u8,
    /// The current directory per drive (`C:\SUB`), when not the root.
    cwd: Vec<(u8, Vec<u8>)>,
}

impl Session {
    fn new(revision: u8, drive: u8) -> Self {
        Session {
            revision,
            counter: 0,
            last_reply: None,
            slots: Default::default(),
            listings: vec![Listing::default(); LISTINGS],
            listing: 0,
            drive,
            cwd: Vec::new(),
        }
    }

    fn cwd(&self, drive: u8) -> Vec<u8> {
        self.cwd
            .iter()
            .find(|(d, _)| *d == drive)
            .map_or_else(|| vec![drive, b':', b'\\'], |(_, p)| p.clone())
    }
}

/// A Filer server over a [`FileSystem`].
#[derive(Debug)]
pub struct Server<F> {
    fs: F,
    config: ServerConfig,
    session: Option<Session>,
    deframer: Deframer,
    out: VecDeque<Vec<u8>>,
    drop_replies: u32,
    executed: u64,
}

impl<F: FileSystem> Server<F> {
    /// A server, idle (no session).
    pub fn new(fs: F, config: ServerConfig) -> Self {
        Server {
            fs,
            config,
            session: None,
            deframer: Deframer::new(),
            out: VecDeque::new(),
            drop_replies: 0,
            executed: 0,
        }
    }

    /// The files.
    pub fn fs(&self) -> &F {
        &self.fs
    }

    /// The files, to change between requests.
    pub fn fs_mut(&mut self) -> &mut F {
        &mut self.fs
    }

    /// Whether a session is open.
    pub fn in_session(&self) -> bool {
        self.session.is_some()
    }

    /// Requests run so far (repeated counters not counted).
    pub fn executed(&self) -> u64 {
        self.executed
    }

    /// Lose the next `n` replies on the way (they still count as sent, so
    /// a retransmission gets them again): for tests of the client's
    /// retries.
    pub fn drop_replies(&mut self, n: u32) {
        self.drop_replies = n;
    }

    /// End the session as Ctrl+Break on the palmtop would: no reply, open
    /// files closed.
    pub fn break_session(&mut self) {
        self.end_session();
    }

    /// Feed bytes read from the link.
    pub fn handle_input(&mut self, bytes: &[u8]) {
        self.deframer.push(bytes);
        while let Some(frame) = self.deframer.next_frame() {
            match frame {
                Ok(frame) => self.on_frame(frame),
                Err(DeframeError::NoSoh(_)) => self.end_session(),
                Err(_) => {}
            }
        }
    }

    /// The next reply to write.
    pub fn poll_output(&mut self) -> Option<Vec<u8>> {
        self.out.pop_front()
    }

    fn send(&mut self, frame: Vec<u8>) {
        if self.drop_replies > 0 {
            self.drop_replies -= 1;
        } else {
            self.out.push_back(frame);
        }
    }

    fn end_session(&mut self) {
        if let Some(mut s) = self.session.take() {
            close_all(&mut self.fs, &mut s);
        }
    }

    fn connect_reply(&mut self, status_word: u16, revision: u8) {
        let frame = Frame::reply(
            Code::Connect.byte(),
            0,
            revision,
            status_word.to_le_bytes().to_vec(),
        );
        self.send(frame.encode());
    }

    fn on_frame(&mut self, frame: Frame) {
        if frame.direction != REQUEST {
            return;
        }
        let is_connect = frame.command == Code::Connect.byte() && frame.counter == 0;
        let Some(session) = self.session.as_mut() else {
            if is_connect && (self.config.revision != 1 || frame.revision == 1) {
                let revision = self.config.revision;
                self.session = Some(Session::new(revision, self.first_drive()));
                self.connect_reply(status::OK, revision);
            } else {
                self.connect_reply(status::NOT_CONNECTED, self.config.revision);
            }
            return;
        };
        if is_connect {
            // Files left open are not closed.
            let revision = if frame.revision == 1 { 1 } else { REVISION };
            let drive = session.drive;
            self.session = Some(Session::new(revision, drive));
            self.connect_reply(status::OK, revision);
            return;
        }
        let revision = session.revision;
        if frame.command == Code::Disconnect.byte() && frame.counter == 0 {
            let reply = Frame::reply(frame.command, 0, revision, vec![0, 0]).encode();
            self.end_session();
            self.send(reply);
            return;
        }
        if frame.counter == session.counter {
            if let Some(reply) = session.last_reply.clone() {
                self.send(reply);
            }
            return;
        }
        if frame.counter != session.counter.wrapping_add(1) {
            let last = session.counter;
            let reply = Frame::reply(
                frame.command,
                last,
                revision,
                status::OUT_OF_SEQUENCE.to_le_bytes().to_vec(),
            )
            .encode();
            self.send(reply);
            return;
        }
        session.counter = frame.counter;
        if frame.command == 0x02 {
            session.last_reply = None;
            return;
        }
        let data = match Code::from_byte(frame.command) {
            None | Some(Code::Connect | Code::Disconnect) => {
                status::UNKNOWN_COMMAND.to_le_bytes().to_vec()
            }
            Some(code) => match Request::decode(code, &frame.data) {
                None => encode_failure(Failure::Fs(FsError::INTERNAL)),
                Some(request) => {
                    self.executed += 1;
                    match self.run(&request) {
                        Ok(Some(response)) => response.encode(),
                        Ok(None) => return self.end_session(),
                        Err(failure) => encode_failure(failure),
                    }
                }
            },
        };
        let reply = Frame::reply(frame.command, frame.counter, revision, data).encode();
        if let Some(session) = self.session.as_mut() {
            session.last_reply = Some(reply.clone());
        }
        self.send(reply);
    }

    fn first_drive(&self) -> u8 {
        let drives = self.fs.drives();
        if drives.contains(&b'C') {
            b'C'
        } else {
            drives.first().copied().unwrap_or(b'C')
        }
    }

    /// Run a request in the session. `Ok(None)`: end the session without a
    /// reply.
    fn run(&mut self, request: &Request) -> Result<Option<Response>, Failure> {
        let fs_err = Failure::Fs;
        let Some(s) = self.session.as_mut() else {
            return Ok(None);
        };
        let fs = &mut self.fs;
        let drives = fs.drives();
        let resolve = |s: &Session, name: &[u8]| resolve(s, &drives, name);
        let response = match request {
            Request::Connect | Request::Disconnect => {
                return Err(Failure::Status(status::UNKNOWN_COMMAND));
            }
            Request::Drives => Response::Drives(drives.clone()),
            Request::CurrentDrive => Response::Drive(s.drive),
            Request::SetDrive(letter) => {
                let letter = letter.to_ascii_uppercase();
                if !drives.contains(&letter) {
                    return Err(fs_err(FsError::PATH_NOT_FOUND));
                }
                s.drive = letter;
                Response::Done
            }
            Request::VolumeInfo => {
                let (label, free) = fs.volume(s.drive).map_err(fs_err)?;
                Response::Volume { label, free }
            }
            Request::CurrentDir(letter) => {
                let letter = letter.to_ascii_uppercase();
                if !drives.contains(&letter) {
                    return Err(fs_err(FsError::PATH_NOT_FOUND));
                }
                Response::Directory(s.cwd(letter))
            }
            Request::ChangeDir(name) => {
                let path = resolve(s, name)?;
                if fs.kind(&path) != NameKind::Directory {
                    return Err(fs_err(FsError::PATH_NOT_FOUND));
                }
                let drive = path[0];
                s.cwd.retain(|(d, _)| *d != drive);
                s.cwd.push((drive, path));
                Response::Done
            }
            Request::MakeDir(name) => {
                fs.make_dir(&resolve(s, name)?).map_err(fs_err)?;
                Response::Done
            }
            Request::RemoveDir(name) => {
                let path = resolve(s, name)?;
                if s.cwd(path[0]) == path && path.len() > 3 {
                    return Err(fs_err(FsError::CURRENT_DIR));
                }
                fs.remove_dir(&path).map_err(fs_err)?;
                Response::Done
            }
            Request::TestName(name) => Response::Kind(fs.kind(&resolve(s, name)?)),
            Request::FindFirst { pattern, .. } => {
                let path = resolve(s, pattern)?;
                let (dir, pattern) = split_last(&path);
                s.listing = (s.listing + 1) % LISTINGS;
                s.listings[s.listing] = Listing {
                    dir,
                    pattern,
                    entries: None,
                    next: 0,
                };
                Response::Done
            }
            Request::FindNext => {
                let listing = &mut s.listings[s.listing];
                if listing.entries.is_none() {
                    let found = if listing.pattern.is_empty() {
                        Err(FsError::NOT_FOUND)
                    } else {
                        fs.list(&listing.dir).map(|entries| {
                            entries
                                .into_iter()
                                .filter(|e| {
                                    e.attributes & (attr::HIDDEN | attr::SYSTEM | attr::VOLUME) == 0
                                        && matches(&listing.pattern, &e.name)
                                })
                                .collect::<Vec<_>>()
                        })
                    };
                    // DOS's "no more files" on the first search is an empty
                    // listing, not an error; a missing directory is.
                    listing.entries = Some(found);
                }
                match &listing.entries {
                    Some(Err(e)) => return Err(fs_err(*e)),
                    Some(Ok(entries)) => {
                        let entry = entries.get(listing.next).cloned();
                        if entry.is_some() {
                            listing.next += 1;
                        }
                        Response::Entry(entry)
                    }
                    None => Response::Entry(None),
                }
            }
            Request::FindClose => {
                s.listing = (s.listing + LISTINGS - 1) % LISTINGS;
                Response::Done
            }
            Request::Open { name, .. } => {
                let path = resolve(s, name)?;
                match fs.kind(&path) {
                    NameKind::File => {}
                    NameKind::Directory => return Err(fs_err(FsError::ACCESS_DENIED)),
                    NameKind::Nothing => return Err(fs_err(FsError::NOT_FOUND)),
                }
                let (data, modified) = fs.read(&path).map_err(fs_err)?;
                let Some(handle) = free_slot(s) else {
                    return Ok(None);
                };
                s.slots[handle] = Some(Slot {
                    path,
                    data,
                    position: 0,
                    writing: false,
                    modified: Some(modified),
                });
                Response::Handle(handle as u16)
            }
            Request::Create { name, .. } | Request::Replace { name, .. } => {
                let overwrite = matches!(request, Request::Replace { .. });
                let path = resolve(s, name)?;
                if free_slot(s).is_none() {
                    return Ok(None);
                }
                fs.create(&path, overwrite).map_err(fs_err)?;
                let Some(handle) = free_slot(s) else {
                    return Ok(None);
                };
                s.slots[handle] = Some(Slot {
                    path,
                    data: Vec::new(),
                    position: 0,
                    writing: true,
                    modified: None,
                });
                Response::Handle(handle as u16)
            }
            Request::Read { handle, len } => {
                let slot = slot(s, *handle)?;
                let start = slot.position.min(slot.data.len());
                let end = (start + usize::from(*len).min(MAX_BLOCK)).min(slot.data.len());
                slot.position = end;
                Response::Data(slot.data[start..end].to_vec())
            }
            Request::Write { handle, data } => {
                let slot = slot(s, *handle)?;
                if !slot.writing {
                    return Err(fs_err(FsError::ACCESS_DENIED));
                }
                let end = slot.position + data.len();
                if slot.data.len() < end {
                    slot.data.resize(end, 0);
                }
                slot.data[slot.position..end].copy_from_slice(data);
                slot.position = end;
                Response::Done
            }
            Request::GetTime(handle) => {
                let slot = slot(s, *handle)?;
                Response::Time(slot.modified.unwrap_or_default())
            }
            Request::SetTime { handle, modified } => {
                slot(s, *handle)?.modified = Some(*modified);
                Response::Done
            }
            Request::Tell(handle) => {
                let slot = slot(s, *handle)?;
                Response::Position(u32::try_from(slot.position).unwrap_or(u32::MAX))
            }
            Request::Seek { handle, position } => {
                slot(s, *handle)?.position = *position as usize;
                Response::Done
            }
            Request::Close(handle) => {
                slot(s, *handle)?;
                let Some(slot) = s.slots[usize::from(*handle)].take() else {
                    return Err(fs_err(FsError::INTERNAL));
                };
                if slot.writing {
                    fs.store(&slot.path, &slot.data, slot.modified)
                        .map_err(fs_err)?;
                }
                Response::Done
            }
            Request::Rename { from, to } => {
                let from = resolve(s, from)?;
                let to = resolve(s, to)?;
                if from[0] != to[0] {
                    return Err(fs_err(FsError::OTHER_DRIVE));
                }
                fs.rename(&from, &to).map_err(fs_err)?;
                Response::Done
            }
            Request::Delete(name) => {
                fs.delete(&resolve(s, name)?).map_err(fs_err)?;
                Response::Done
            }
            Request::GetAttr(name) => {
                Response::Attributes(fs.attributes(&resolve(s, name)?).map_err(fs_err)?)
            }
            Request::SetAttr { name, attributes } => {
                fs.set_attributes(&resolve(s, name)?, *attributes)
                    .map_err(fs_err)?;
                Response::Done
            }
        };
        Ok(Some(response))
    }
}

fn free_slot(s: &Session) -> Option<usize> {
    s.slots.iter().position(Option::is_none)
}

fn slot(s: &mut Session, handle: u16) -> Result<&mut Slot, Failure> {
    s.slots
        .get_mut(usize::from(handle))
        .and_then(Option::as_mut)
        .ok_or(Failure::Fs(FsError::INTERNAL))
}

fn close_all<F: FileSystem>(fs: &mut F, s: &mut Session) {
    for slot in s.slots.iter_mut().filter_map(Option::take) {
        if slot.writing {
            let _ = fs.store(&slot.path, &slot.data, slot.modified);
        }
    }
}

/// Bytes a DOS name may not hold (wiki: the PC server's error 200).
fn bad_name_byte(b: u8) -> bool {
    b < 0x20 || b" \"/[]|<>+=;,".contains(&b) || b == DLE
}

/// The canonical path of a name in the session: drive, absolute, upper
/// case, `.` and `..` resolved.
fn resolve(s: &Session, drives: &[u8], name: &[u8]) -> Result<Vec<u8>, Failure> {
    let invalid = Failure::Fs(FsError::INVALID_NAME);
    let name = name.to_ascii_uppercase();
    let (drive, rest) = match name.as_slice() {
        [d, b':', rest @ ..] if d.is_ascii_alphabetic() => (*d, rest),
        _ => (s.drive, name.as_slice()),
    };
    if !drives.contains(&drive) {
        return Err(Failure::Fs(FsError::PATH_NOT_FOUND));
    }
    if rest.iter().any(|&b| bad_name_byte(b) || b == b':') {
        return Err(invalid);
    }
    let mut parts: Vec<Vec<u8>> = Vec::new();
    if rest.first() != Some(&b'\\') {
        let cwd = s.cwd(drive);
        parts.extend(
            cwd[3..]
                .split(|&b| b == b'\\')
                .filter(|p| !p.is_empty())
                .map(<[u8]>::to_vec),
        );
    }
    for part in rest.split(|&b| b == b'\\') {
        match part {
            b"" | b"." => {}
            b".." => {
                parts.pop();
            }
            p => parts.push(p.to_vec()),
        }
    }
    let mut path = vec![drive, b':', b'\\'];
    path.extend(parts.join(&b'\\'));
    Ok(path)
}

/// `C:\A\B` to (`C:\A`, `B`); `C:\` to (`C:\`, ``).
fn split_last(path: &[u8]) -> (Vec<u8>, Vec<u8>) {
    match path.iter().rposition(|&b| b == b'\\') {
        Some(i) if i >= 3 => (path[..i].to_vec(), path[i + 1..].to_vec()),
        Some(i) => (path[..=i].to_vec(), path[i + 1..].to_vec()),
        None => (path.to_vec(), Vec::new()),
    }
}

/// DOS wildcards: `*` any run, `?` one character; `*.*` matches every
/// name, with or without an extension.
pub(crate) fn matches(pattern: &[u8], name: &[u8]) -> bool {
    if pattern == b"*.*" {
        return true;
    }
    let name = name.to_ascii_uppercase();
    glob(&pattern.to_ascii_uppercase(), &name)
}

fn glob(p: &[u8], n: &[u8]) -> bool {
    match (p.first(), n.first()) {
        (None, None) => true,
        (Some(b'*'), _) => glob(&p[1..], n) || (!n.is_empty() && glob(p, &n[1..])),
        (Some(b'?'), Some(_)) => glob(&p[1..], &n[1..]),
        (Some(a), Some(b)) if a == b => glob(&p[1..], &n[1..]),
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::MemoryFs;

    fn request(code: u8, counter: u8, data: &[u8]) -> Vec<u8> {
        Frame::request(code, counter, 2, data.to_vec()).encode()
    }

    fn exchange(s: &mut Server<MemoryFs>, wire: &[u8]) -> Option<Frame> {
        s.handle_input(wire);
        let out = s.poll_output()?;
        let mut d = Deframer::new();
        d.push(&out);
        Some(d.next_frame().unwrap().unwrap())
    }

    fn server() -> Server<MemoryFs> {
        let mut fs = MemoryFs::new(b"ACD");
        fs.insert_file(b"C:\\HELLO.TXT", b"hi".to_vec());
        Server::new(fs, ServerConfig::default())
    }

    #[test]
    fn idle_server_refuses_everything_but_connect() {
        let mut s = server();
        let f = exchange(&mut s, &request(0x1B, 1, &[])).unwrap();
        assert_eq!((f.command, f.counter, f.data), (0x40, 0, vec![0x3B, 0xFF]));
        let f = exchange(&mut s, &request(0x40, 3, &[])).unwrap();
        assert_eq!(f.data, [0x3B, 0xFF]);
        let f = exchange(&mut s, &request(0x40, 0, &[])).unwrap();
        assert_eq!((f.command, f.direction, f.data), (0x40, 0x81, vec![0, 0]));
        assert!(s.in_session());
    }

    #[test]
    fn counters() {
        let mut s = server();
        exchange(&mut s, &request(0x40, 0, &[])).unwrap();
        let f = exchange(&mut s, &request(0x1B, 1, &[])).unwrap();
        assert_eq!(f.data, Response::Drives(b"ACD".to_vec()).encode());
        assert_eq!(s.executed(), 1);
        // The same counter again: the previous reply, not run again.
        let again = exchange(&mut s, &request(0x1B, 1, &[])).unwrap();
        assert_eq!(again, f);
        assert_eq!(s.executed(), 1);
        // A skipped counter: 0xFF3C with the last good one.
        let f = exchange(&mut s, &request(0x1B, 5, &[])).unwrap();
        assert_eq!((f.counter, f.data), (1, vec![0x3C, 0xFF]));
        // Unknown and unimplemented commands.
        let f = exchange(&mut s, &request(0x1C, 2, &[])).unwrap();
        assert_eq!(f.data, [0x3E, 0xFF]);
        assert!(exchange(&mut s, &request(0x02, 3, &[])).is_none());
        let f = exchange(&mut s, &request(0x07, 4, &[])).unwrap();
        assert_eq!(f.data, Response::Drive(b'C').encode());
        let f = exchange(&mut s, &request(0x41, 0, &[])).unwrap();
        assert_eq!(f.data, [0, 0]);
        assert!(!s.in_session());
    }

    #[test]
    fn all_slots_in_use_ends_the_session() {
        let mut s = server();
        exchange(&mut s, &request(0x40, 0, &[])).unwrap();
        let open = Request::Open {
            name: b"C:\\HELLO.TXT".to_vec(),
            mode: 0,
        }
        .encode();
        for i in 0..8u8 {
            let f = exchange(&mut s, &request(0x0B, i + 1, &open)).unwrap();
            assert_eq!(f.data, Response::Handle(u16::from(i)).encode());
        }
        assert!(exchange(&mut s, &request(0x0B, 9, &open)).is_none());
        assert!(!s.in_session());
    }

    #[test]
    fn paths_resolve() {
        let s = Session::new(2, b'C');
        let drives = b"ACD";
        let r = |n: &[u8]| resolve(&s, drives, n);
        assert_eq!(r(b"c:\\abcd.xyz").unwrap(), b"C:\\ABCD.XYZ");
        assert_eq!(r(b"SUB\\..\\X").unwrap(), b"C:\\X");
        assert_eq!(r(b"A:").unwrap(), b"A:\\");
        assert_eq!(r(b"E:\\X"), Err(Failure::Fs(FsError::PATH_NOT_FOUND)));
        assert_eq!(r(b"C:\\A B"), Err(Failure::Fs(FsError::INVALID_NAME)));
        assert_eq!(split_last(b"C:\\A\\B"), (b"C:\\A".to_vec(), b"B".to_vec()));
        assert_eq!(split_last(b"C:\\*.*"), (b"C:\\".to_vec(), b"*.*".to_vec()));
        assert!(matches(b"*.*", b"README"));
        assert!(matches(b"*.TXT", b"hello.txt"));
        assert!(!matches(b"*.TXT", b"HELLO.BIN"));
        assert!(matches(b"H?LLO.*", b"HELLO.TXT"));
    }
}
