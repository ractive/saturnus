//! The commands: their codes, the request data and the reply layouts
//! (wiki: protocols/filer-hplx, "Data conventions" and "Commands"). All
//! numbers are little-endian.
//!
//! Names are DOS paths as bytes (`C:\`, `C:\*.*`, `C:\ABCD.XYZ`), sent as
//! a two-byte length, the characters, and a two-byte parameter word not
//! counted in the length.

use std::fmt;

use crate::error::{Failure, FsError, status};

/// A command code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Code {
    /// 0x01: close the open file.
    Close,
    /// 0x03: create a file; 217 if it exists.
    Create,
    /// 0x04: delete a file.
    Delete,
    /// 0x05: create or overwrite a file.
    Replace,
    /// 0x06: a drive's current directory.
    CurrentDir,
    /// 0x07: the current drive.
    CurrentDrive,
    /// 0x08: what a name is (nothing, a file, a directory).
    TestName,
    /// 0x09: the next directory entry of a listing.
    FindNext,
    /// 0x0A: make a directory.
    MakeDir,
    /// 0x0B: open a file for reading.
    Open,
    /// 0x0C: read a block.
    Read,
    /// 0x0D: rename or move within a drive.
    Rename,
    /// 0x0E: remove an empty directory.
    RemoveDir,
    /// 0x0F: set the file position.
    Seek,
    /// 0x10: change directory.
    ChangeDir,
    /// 0x11: select a drive.
    SetDrive,
    /// 0x12: start a listing.
    FindFirst,
    /// 0x13: the file position.
    Tell,
    /// 0x14: volume label and free space of the current drive.
    VolumeInfo,
    /// 0x15: write a block.
    Write,
    /// 0x16: drop the most recent listing context.
    FindClose,
    /// 0x17: the open file's time and date.
    GetTime,
    /// 0x18: set the open file's time and date.
    SetTime,
    /// 0x19: a file's attribute byte.
    GetAttr,
    /// 0x1A: set a file's attribute byte.
    SetAttr,
    /// 0x1B: the drive letters.
    Drives,
    /// 0x40: open a session.
    Connect,
    /// 0x41: end the session.
    Disconnect,
}

impl Code {
    /// Every code, in numeric order.
    pub const ALL: [Code; 28] = [
        Code::Close,
        Code::Create,
        Code::Delete,
        Code::Replace,
        Code::CurrentDir,
        Code::CurrentDrive,
        Code::TestName,
        Code::FindNext,
        Code::MakeDir,
        Code::Open,
        Code::Read,
        Code::Rename,
        Code::RemoveDir,
        Code::Seek,
        Code::ChangeDir,
        Code::SetDrive,
        Code::FindFirst,
        Code::Tell,
        Code::VolumeInfo,
        Code::Write,
        Code::FindClose,
        Code::GetTime,
        Code::SetTime,
        Code::GetAttr,
        Code::SetAttr,
        Code::Drives,
        Code::Connect,
        Code::Disconnect,
    ];

    /// The byte on the wire.
    pub fn byte(self) -> u8 {
        match self {
            Code::Close => 0x01,
            Code::Create => 0x03,
            Code::Delete => 0x04,
            Code::Replace => 0x05,
            Code::CurrentDir => 0x06,
            Code::CurrentDrive => 0x07,
            Code::TestName => 0x08,
            Code::FindNext => 0x09,
            Code::MakeDir => 0x0A,
            Code::Open => 0x0B,
            Code::Read => 0x0C,
            Code::Rename => 0x0D,
            Code::RemoveDir => 0x0E,
            Code::Seek => 0x0F,
            Code::ChangeDir => 0x10,
            Code::SetDrive => 0x11,
            Code::FindFirst => 0x12,
            Code::Tell => 0x13,
            Code::VolumeInfo => 0x14,
            Code::Write => 0x15,
            Code::FindClose => 0x16,
            Code::GetTime => 0x17,
            Code::SetTime => 0x18,
            Code::GetAttr => 0x19,
            Code::SetAttr => 0x1A,
            Code::Drives => 0x1B,
            Code::Connect => 0x40,
            Code::Disconnect => 0x41,
        }
    }

    /// The code of a byte; `None` for 0x02 (not implemented) and unknown
    /// bytes.
    pub fn from_byte(b: u8) -> Option<Code> {
        Code::ALL.into_iter().find(|c| c.byte() == b)
    }
}

/// DOS file attribute bits.
pub mod attr {
    /// Read-only.
    pub const READ_ONLY: u8 = 0x01;
    /// Hidden.
    pub const HIDDEN: u8 = 0x02;
    /// System.
    pub const SYSTEM: u8 = 0x04;
    /// Volume label.
    pub const VOLUME: u8 = 0x08;
    /// Directory.
    pub const DIRECTORY: u8 = 0x10;
    /// Archive (changed since the last backup).
    pub const ARCHIVE: u8 = 0x20;
}

/// A DOS time and date, as two words (time first): hours in bits 15-11,
/// minutes 10-5, seconds divided by two in 4-0; years since 1980 in bits
/// 15-9, month 8-5, day 4-0. Local time on the palmtop.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DosDateTime {
    /// The time word.
    pub time: u16,
    /// The date word.
    pub date: u16,
}

impl DosDateTime {
    /// From its parts; `None` outside 1980-2107 or for an impossible field.
    /// Odd seconds round down (DOS keeps two-second steps).
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Self> {
        if !(1980..=2107).contains(&year)
            || !(1..=12).contains(&month)
            || !(1..=31).contains(&day)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        Some(DosDateTime {
            time: u16::from(hour) << 11 | u16::from(minute) << 5 | u16::from(second / 2),
            date: (year - 1980) << 9 | u16::from(month) << 5 | u16::from(day),
        })
    }

    /// The year (1980-2107).
    pub fn year(self) -> u16 {
        1980 + (self.date >> 9)
    }
    /// The month (1-12 when valid).
    pub fn month(self) -> u8 {
        ((self.date >> 5) & 0x0F) as u8
    }
    /// The day (1-31 when valid).
    pub fn day(self) -> u8 {
        (self.date & 0x1F) as u8
    }
    /// The hour.
    pub fn hour(self) -> u8 {
        (self.time >> 11) as u8
    }
    /// The minute.
    pub fn minute(self) -> u8 {
        ((self.time >> 5) & 0x3F) as u8
    }
    /// The second (even).
    pub fn second(self) -> u8 {
        ((self.time & 0x1F) * 2) as u8
    }

    fn bytes(self) -> [u8; 4] {
        let [t0, t1] = self.time.to_le_bytes();
        let [d0, d1] = self.date.to_le_bytes();
        [t0, t1, d0, d1]
    }
}

impl fmt::Display for DosDateTime {
    /// `2004-12-14 13:49:36`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}

impl fmt::Debug for DosDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DosDateTime({self})")
    }
}

/// One directory entry: the DOS find record (22 bytes on the wire).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    /// The attribute byte ([`attr`]).
    pub attributes: u8,
    /// Time and date of the last change.
    pub modified: DosDateTime,
    /// Size in bytes (0 for a directory).
    pub size: u32,
    /// The 8.3 name, without the path.
    pub name: Vec<u8>,
}

/// The length of a [`DirEntry`] on the wire.
pub const ENTRY_LEN: usize = 22;
/// The name field of a [`DirEntry`]: 8.3 and the closing zero.
const ENTRY_NAME_LEN: usize = 13;

impl DirEntry {
    /// Whether the entry is a directory.
    pub fn is_directory(&self) -> bool {
        self.attributes & attr::DIRECTORY != 0
    }

    /// The name as text (bytes 0x80-0xFF as Latin-1).
    pub fn name_lossy(&self) -> String {
        self.name.iter().map(|&b| char::from(b)).collect()
    }

    /// The 22 bytes on the wire (the name field zero-filled).
    pub fn encode(&self) -> [u8; ENTRY_LEN] {
        let mut out = [0u8; ENTRY_LEN];
        out[0] = self.attributes;
        out[1..5].copy_from_slice(&self.modified.bytes());
        out[5..9].copy_from_slice(&self.size.to_le_bytes());
        let n = self.name.len().min(ENTRY_NAME_LEN - 1);
        out[9..9 + n].copy_from_slice(&self.name[..n]);
        out
    }

    /// Parse the 22 bytes; the name ends at the first zero (the bytes after
    /// it are not cleared by the server).
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < ENTRY_LEN {
            return None;
        }
        let field = &bytes[9..9 + ENTRY_NAME_LEN];
        let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
        Some(DirEntry {
            attributes: bytes[0],
            modified: DosDateTime {
                time: u16::from_le_bytes([bytes[1], bytes[2]]),
                date: u16::from_le_bytes([bytes[3], bytes[4]]),
            },
            size: u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]),
            name: field[..end].to_vec(),
        })
    }
}

/// What a name is (the reply to [`Request::TestName`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameKind {
    /// Nothing there.
    Nothing,
    /// A file.
    File,
    /// A directory.
    Directory,
}

/// One request, with its data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Request {
    /// Open a session (counter 0).
    Connect,
    /// End the session (counter 0, no reply awaited).
    Disconnect,
    /// The drive letters.
    Drives,
    /// The current drive.
    CurrentDrive,
    /// Select a drive (its letter, `b'C'`).
    SetDrive(u8),
    /// Volume label and free space of the current drive.
    VolumeInfo,
    /// A drive's current directory.
    CurrentDir(u8),
    /// Change directory.
    ChangeDir(Vec<u8>),
    /// Make a directory.
    MakeDir(Vec<u8>),
    /// Remove an empty directory.
    RemoveDir(Vec<u8>),
    /// What a name is.
    TestName(Vec<u8>),
    /// Start a listing of a DOS pattern (`C:\*.*`).
    FindFirst {
        /// Path and pattern.
        pattern: Vec<u8>,
        /// The search attribute (HP's PC server ignores it).
        attributes: u16,
    },
    /// The next entry of the listing.
    FindNext,
    /// Drop the most recent listing context.
    FindClose,
    /// Open a file for reading.
    Open {
        /// The file.
        name: Vec<u8>,
        /// The open mode (0).
        mode: u16,
    },
    /// Read up to `len` (at most 0x800) bytes.
    Read {
        /// The file handle.
        handle: u16,
        /// How many bytes.
        len: u16,
    },
    /// Create a file; [`FsError::EXISTS`] if it exists.
    Create {
        /// The file.
        name: Vec<u8>,
        /// Its attribute (0).
        attributes: u16,
    },
    /// Create or overwrite a file.
    Replace {
        /// The file.
        name: Vec<u8>,
        /// Its attribute (0).
        attributes: u16,
    },
    /// Write a block (at most 0x800 bytes).
    Write {
        /// The file handle.
        handle: u16,
        /// The bytes.
        data: Vec<u8>,
    },
    /// The open file's time and date.
    GetTime(u16),
    /// Set the open file's time and date.
    SetTime {
        /// The file handle.
        handle: u16,
        /// The new time and date.
        modified: DosDateTime,
    },
    /// The file position.
    Tell(u16),
    /// Set the file position.
    Seek {
        /// The file handle.
        handle: u16,
        /// The position.
        position: u32,
    },
    /// Close the open file.
    Close(u16),
    /// Rename or move within a drive.
    Rename {
        /// The old name.
        from: Vec<u8>,
        /// The new name.
        to: Vec<u8>,
    },
    /// Delete a file.
    Delete(Vec<u8>),
    /// A file's attribute byte.
    GetAttr(Vec<u8>),
    /// Set a file's attribute byte.
    SetAttr {
        /// The file.
        name: Vec<u8>,
        /// The new attribute.
        attributes: u8,
    },
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_name(out: &mut Vec<u8>, name: &[u8], word: u16) {
    // A name never comes close to 64 KiB; the frame limit refuses one
    // longer than a body anyway.
    put_u16(out, u16::try_from(name.len()).unwrap_or(u16::MAX));
    out.extend_from_slice(name);
    put_u16(out, word);
}

/// A little-endian reader over request or reply data.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.bytes.len() < n {
            return None;
        }
        let (head, rest) = self.bytes.split_at(n);
        self.bytes = rest;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn counted(&mut self) -> Option<&'a [u8]> {
        let n = self.u16()?;
        self.take(usize::from(n))
    }
    /// A name and its parameter word.
    fn name(&mut self) -> Option<(Vec<u8>, u16)> {
        let name = self.counted()?.to_vec();
        // HP's client always sends the word; be lenient when it is missing.
        let word = self.u16().unwrap_or(0);
        Some((name, word))
    }
}

impl Request {
    /// Its command code.
    pub fn code(&self) -> Code {
        match self {
            Request::Connect => Code::Connect,
            Request::Disconnect => Code::Disconnect,
            Request::Drives => Code::Drives,
            Request::CurrentDrive => Code::CurrentDrive,
            Request::SetDrive(_) => Code::SetDrive,
            Request::VolumeInfo => Code::VolumeInfo,
            Request::CurrentDir(_) => Code::CurrentDir,
            Request::ChangeDir(_) => Code::ChangeDir,
            Request::MakeDir(_) => Code::MakeDir,
            Request::RemoveDir(_) => Code::RemoveDir,
            Request::TestName(_) => Code::TestName,
            Request::FindFirst { .. } => Code::FindFirst,
            Request::FindNext => Code::FindNext,
            Request::FindClose => Code::FindClose,
            Request::Open { .. } => Code::Open,
            Request::Read { .. } => Code::Read,
            Request::Create { .. } => Code::Create,
            Request::Replace { .. } => Code::Replace,
            Request::Write { .. } => Code::Write,
            Request::GetTime(_) => Code::GetTime,
            Request::SetTime { .. } => Code::SetTime,
            Request::Tell(_) => Code::Tell,
            Request::Seek { .. } => Code::Seek,
            Request::Close(_) => Code::Close,
            Request::Rename { .. } => Code::Rename,
            Request::Delete(_) => Code::Delete,
            Request::GetAttr(_) => Code::GetAttr,
            Request::SetAttr { .. } => Code::SetAttr,
        }
    }

    /// The request data.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Request::Connect
            | Request::Disconnect
            | Request::Drives
            | Request::CurrentDrive
            | Request::VolumeInfo
            | Request::FindNext
            | Request::FindClose => {}
            Request::SetDrive(letter) | Request::CurrentDir(letter) => {
                out.extend_from_slice(&[*letter, 0]);
            }
            Request::ChangeDir(name)
            | Request::MakeDir(name)
            | Request::RemoveDir(name)
            | Request::TestName(name)
            | Request::Delete(name)
            | Request::GetAttr(name) => put_name(&mut out, name, 0),
            Request::FindFirst {
                pattern,
                attributes,
            } => put_name(&mut out, pattern, *attributes),
            Request::Open { name, mode } => put_name(&mut out, name, *mode),
            Request::Create { name, attributes } | Request::Replace { name, attributes } => {
                put_name(&mut out, name, *attributes);
            }
            Request::Read { handle, len } => {
                put_u16(&mut out, *handle);
                put_u16(&mut out, *len);
            }
            Request::Write { handle, data } => {
                put_u16(&mut out, *handle);
                put_u16(&mut out, u16::try_from(data.len()).unwrap_or(u16::MAX));
                out.extend_from_slice(data);
            }
            Request::GetTime(handle) | Request::Tell(handle) | Request::Close(handle) => {
                put_u16(&mut out, *handle);
            }
            Request::SetTime { handle, modified } => {
                put_u16(&mut out, *handle);
                out.extend_from_slice(&modified.bytes());
            }
            Request::Seek { handle, position } => {
                put_u16(&mut out, *handle);
                put_u16(&mut out, 0);
                out.extend_from_slice(&position.to_le_bytes());
            }
            Request::Rename { from, to } => {
                put_name(&mut out, from, 0);
                put_name(&mut out, to, 0);
            }
            Request::SetAttr { name, attributes } => {
                put_name(&mut out, name, u16::from(*attributes));
            }
        }
        out
    }

    /// Parse a request's data (the server's side); `None` when it is too
    /// short for its command.
    pub fn decode(code: Code, data: &[u8]) -> Option<Request> {
        let mut r = Reader::new(data);
        Some(match code {
            Code::Connect => Request::Connect,
            Code::Disconnect => Request::Disconnect,
            Code::Drives => Request::Drives,
            Code::CurrentDrive => Request::CurrentDrive,
            Code::VolumeInfo => Request::VolumeInfo,
            Code::FindNext => Request::FindNext,
            Code::FindClose => Request::FindClose,
            Code::SetDrive => Request::SetDrive(r.u8()?),
            Code::CurrentDir => Request::CurrentDir(r.u8()?),
            Code::ChangeDir => Request::ChangeDir(r.name()?.0),
            Code::MakeDir => Request::MakeDir(r.name()?.0),
            Code::RemoveDir => Request::RemoveDir(r.name()?.0),
            Code::TestName => Request::TestName(r.name()?.0),
            Code::Delete => Request::Delete(r.name()?.0),
            Code::GetAttr => Request::GetAttr(r.name()?.0),
            Code::FindFirst => {
                let (pattern, attributes) = r.name()?;
                Request::FindFirst {
                    pattern,
                    attributes,
                }
            }
            Code::Open => {
                let (name, mode) = r.name()?;
                Request::Open { name, mode }
            }
            Code::Create => {
                let (name, attributes) = r.name()?;
                Request::Create { name, attributes }
            }
            Code::Replace => {
                let (name, attributes) = r.name()?;
                Request::Replace { name, attributes }
            }
            Code::Read => Request::Read {
                handle: r.u16()?,
                len: r.u16()?,
            },
            Code::Write => {
                let handle = r.u16()?;
                let data = r.counted()?.to_vec();
                Request::Write { handle, data }
            }
            Code::GetTime => Request::GetTime(r.u16()?),
            Code::Tell => Request::Tell(r.u16()?),
            Code::Close => Request::Close(r.u16()?),
            Code::SetTime => Request::SetTime {
                handle: r.u16()?,
                modified: DosDateTime {
                    time: r.u16()?,
                    date: r.u16()?,
                },
            },
            Code::Seek => {
                let handle = r.u16()?;
                r.u16()?;
                Request::Seek {
                    handle,
                    position: r.u32()?,
                }
            }
            Code::Rename => {
                let from = r.name()?.0;
                let to = r.name()?.0;
                Request::Rename { from, to }
            }
            Code::SetAttr => {
                let (name, word) = r.name()?;
                Request::SetAttr {
                    name,
                    attributes: word.to_le_bytes()[0],
                }
            }
        })
    }
}

/// What a successful reply carries.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Response {
    /// A session is open; the revision is the one the client asked with.
    Connected,
    /// The session ended.
    Disconnected,
    /// Done, nothing more to say (also the reply to 0x15, whose value word
    /// has no defined meaning).
    Done,
    /// The drive letters.
    Drives(Vec<u8>),
    /// The current drive's letter.
    Drive(u8),
    /// Volume label (11 bytes) and free space in bytes.
    Volume {
        /// The label, blank-padded.
        label: Vec<u8>,
        /// Free bytes.
        free: u32,
    },
    /// A drive's current directory (`C:\`).
    Directory(Vec<u8>),
    /// What a name is.
    Kind(NameKind),
    /// The next listing entry; `None` when the listing has ended.
    Entry(Option<DirEntry>),
    /// The handle of the file just opened or created.
    Handle(u16),
    /// A block read; empty at the end of the file.
    Data(Vec<u8>),
    /// The open file's time and date.
    Time(DosDateTime),
    /// The file position.
    Position(u32),
    /// A file's attribute byte.
    Attributes(u8),
}

/// A reply whose data does not fit its command's layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Malformed {
    /// The command it answers.
    pub code: Code,
    /// The reply data.
    pub data: Vec<u8>,
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "malformed reply to {:?} ({} bytes: {})",
            self.code,
            self.data.len(),
            crate::codec::hex(&self.data)
        )
    }
}

impl std::error::Error for Malformed {}

impl Response {
    /// Parse the reply data (status word first) to a request with `code`.
    pub fn decode(code: Code, data: &[u8]) -> Result<Result<Response, Failure>, Malformed> {
        let malformed = || Malformed {
            code,
            data: data.to_vec(),
        };
        let mut r = Reader::new(data);
        // Missing words read as zero: BSD Filer answers 0x03 with no data
        // at all and 0x15 with the two words alone. A layout with a count
        // (a block, an entry, a list) must be complete.
        let status = r.u16().unwrap_or(status::OK);
        if status != status::OK {
            return Ok(Err(Failure::Status(status)));
        }
        match code {
            // The status word alone (BSD Filer sends six bytes for 0x41).
            Code::Connect => return Ok(Ok(Response::Connected)),
            Code::Disconnect => return Ok(Ok(Response::Disconnected)),
            _ => {}
        }
        let result = r.u16().unwrap_or(0);
        if result != 0 {
            return Ok(Err(Failure::Fs(FsError(result))));
        }
        let response = match code {
            Code::Connect | Code::Disconnect => return Err(malformed()),
            Code::Close
            | Code::Delete
            | Code::MakeDir
            | Code::Rename
            | Code::RemoveDir
            | Code::Seek
            | Code::ChangeDir
            | Code::SetDrive
            | Code::FindFirst
            | Code::Write
            | Code::FindClose
            | Code::SetTime
            | Code::SetAttr => Some(Response::Done),
            Code::Drives => (|| {
                let n = usize::from(r.u16()?);
                let field = r.take(n)?;
                let letters = field.iter().copied().take_while(|&b| b != 0).collect();
                Some(Response::Drives(letters))
            })(),
            Code::CurrentDrive => r.u8().map(Response::Drive),
            Code::VolumeInfo => (|| {
                let label = r.counted()?.to_vec();
                let free = r.u32()?;
                Some(Response::Volume { label, free })
            })(),
            Code::CurrentDir => r.counted().map(|d| Response::Directory(d.to_vec())),
            Code::TestName => r.u16().and_then(|k| match k {
                0 => Some(Response::Kind(NameKind::Nothing)),
                1 => Some(Response::Kind(NameKind::File)),
                2 => Some(Response::Kind(NameKind::Directory)),
                _ => None,
            }),
            Code::FindNext => (|| {
                let entry = r.counted()?;
                let found = r.u32()?;
                if found == 0 {
                    Some(Response::Entry(None))
                } else {
                    DirEntry::decode(entry).map(|e| Response::Entry(Some(e)))
                }
            })(),
            Code::Open | Code::Create | Code::Replace => {
                Some(Response::Handle(r.u16().unwrap_or(0)))
            }
            Code::Read => r.counted().map(|d| Response::Data(d.to_vec())),
            Code::GetTime => (|| {
                Some(Response::Time(DosDateTime {
                    time: r.u16()?,
                    date: r.u16()?,
                }))
            })(),
            Code::Tell => r.u32().map(Response::Position),
            Code::GetAttr => r.u8().map(Response::Attributes),
        };
        response.map(Ok).ok_or_else(malformed)
    }

    /// The reply data for a successful request: the status word, the
    /// result word (except for CONNECT and DISCONNECT) and the values.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![0, 0];
        if matches!(self, Response::Connected | Response::Disconnected) {
            return out;
        }
        put_u16(&mut out, 0);
        match self {
            Response::Connected | Response::Disconnected => {}
            Response::Done => put_u16(&mut out, 0),
            Response::Drives(letters) => {
                put_u16(
                    &mut out,
                    u16::try_from(letters.len() + 1).unwrap_or(u16::MAX),
                );
                out.extend_from_slice(letters);
                out.extend_from_slice(&[0, 0, 0, 0, 0]);
            }
            Response::Drive(letter) => out.extend_from_slice(&[*letter, 0]),
            Response::Volume { label, free } => {
                put_u16(&mut out, u16::try_from(label.len()).unwrap_or(u16::MAX));
                out.extend_from_slice(label);
                out.extend_from_slice(&free.to_le_bytes());
            }
            Response::Directory(dir) => {
                put_u16(&mut out, u16::try_from(dir.len()).unwrap_or(u16::MAX));
                out.extend_from_slice(dir);
                out.extend_from_slice(&[0, 0, 0, 0]);
            }
            Response::Kind(kind) => put_u16(
                &mut out,
                match kind {
                    NameKind::Nothing => 0,
                    NameKind::File => 1,
                    NameKind::Directory => 2,
                },
            ),
            Response::Entry(entry) => {
                put_u16(&mut out, ENTRY_LEN as u16);
                match entry {
                    Some(e) => {
                        out.extend_from_slice(&e.encode());
                        out.extend_from_slice(&1u32.to_le_bytes());
                    }
                    None => out.extend_from_slice(&[0; ENTRY_LEN + 4]),
                }
            }
            Response::Handle(h) => put_u16(&mut out, *h),
            Response::Data(data) => {
                put_u16(&mut out, u16::try_from(data.len()).unwrap_or(u16::MAX));
                out.extend_from_slice(data);
            }
            Response::Time(t) => out.extend_from_slice(&t.bytes()),
            Response::Position(p) => out.extend_from_slice(&p.to_le_bytes()),
            Response::Attributes(a) => out.extend_from_slice(&[*a, 0]),
        }
        out
    }
}

/// The reply data for a failed request: a non-zero status word alone, or
/// status 0, the result code and a zero word.
pub fn encode_failure(failure: Failure) -> Vec<u8> {
    match failure {
        Failure::Status(code) => code.to_le_bytes().to_vec(),
        Failure::Fs(FsError(code)) => {
            let mut out = vec![0, 0];
            put_u16(&mut out, code);
            put_u16(&mut out, 0);
            out
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        for c in Code::ALL {
            assert_eq!(Code::from_byte(c.byte()), Some(c));
        }
        assert_eq!(Code::from_byte(0x02), None);
        assert_eq!(Code::from_byte(0x1C), None);
    }

    /// The recorded time and date (wiki: "Data conventions").
    #[test]
    fn dos_time() {
        let t = DosDateTime {
            time: 0x6E32,
            date: 0x318E,
        };
        assert_eq!(t.to_string(), "2004-12-14 13:49:36");
        assert_eq!(DosDateTime::new(2004, 12, 14, 13, 49, 37), Some(t));
        assert_eq!(DosDateTime::new(1979, 1, 1, 0, 0, 0), None);
        assert_eq!(DosDateTime::new(2000, 13, 1, 0, 0, 0), None);
    }

    #[test]
    fn requests_round_trip() {
        let name = b"C:\\ABCD.XYZ".to_vec();
        let all = [
            Request::Connect,
            Request::Disconnect,
            Request::Drives,
            Request::CurrentDrive,
            Request::SetDrive(b'C'),
            Request::VolumeInfo,
            Request::CurrentDir(b'C'),
            Request::ChangeDir(b"C:\\".to_vec()),
            Request::MakeDir(name.clone()),
            Request::RemoveDir(name.clone()),
            Request::TestName(name.clone()),
            Request::FindFirst {
                pattern: b"C:\\*.*".to_vec(),
                attributes: 0x10,
            },
            Request::FindNext,
            Request::FindClose,
            Request::Open {
                name: name.clone(),
                mode: 0,
            },
            Request::Read {
                handle: 0,
                len: 0x800,
            },
            Request::Create {
                name: name.clone(),
                attributes: 0,
            },
            Request::Replace {
                name: name.clone(),
                attributes: 0,
            },
            Request::Write {
                handle: 1,
                data: vec![0x10, 0x20],
            },
            Request::GetTime(0),
            Request::SetTime {
                handle: 0,
                modified: DosDateTime {
                    time: 0x6E32,
                    date: 0x318E,
                },
            },
            Request::Tell(0),
            Request::Seek {
                handle: 0,
                position: 0x0102_0304,
            },
            Request::Close(0),
            Request::Rename {
                from: name.clone(),
                to: b"C:\\B".to_vec(),
            },
            Request::Delete(name.clone()),
            Request::GetAttr(name.clone()),
            Request::SetAttr {
                name: name.clone(),
                attributes: attr::READ_ONLY,
            },
        ];
        assert_eq!(all.len(), Code::ALL.len());
        for r in all {
            assert_eq!(
                Request::decode(r.code(), &r.encode()),
                Some(r.clone()),
                "{r:?}"
            );
        }
    }

    #[test]
    fn request_layouts() {
        assert_eq!(
            Request::FindFirst {
                pattern: b"C:\\*.*".to_vec(),
                attributes: 0
            }
            .encode(),
            [6, 0, b'C', b':', b'\\', b'*', b'.', b'*', 0, 0]
        );
        assert_eq!(
            Request::Rename {
                from: b"A".to_vec(),
                to: b"B".to_vec()
            }
            .encode(),
            [1, 0, b'A', 0, 0, 1, 0, b'B', 0, 0]
        );
        assert_eq!(
            Request::Seek {
                handle: 2,
                position: 5
            }
            .encode(),
            [2, 0, 0, 0, 5, 0, 0, 0]
        );
        assert_eq!(Request::SetDrive(b'A').encode(), [b'A', 0]);
    }

    /// Reply data recorded from BSD Filer v0.3.
    #[test]
    fn oracle_replies() {
        let decode = |c, d: &[u8]| Response::decode(c, d).unwrap().unwrap();
        assert_eq!(decode(Code::Connect, &[0, 0]), Response::Connected);
        assert_eq!(
            decode(Code::Disconnect, &[0, 0, 0, 0, 0, 0]),
            Response::Disconnected
        );
        assert_eq!(
            decode(Code::Drives, &[0, 0, 0, 0, 2, 0, b'C', 0, 0, 0, 0, 0]),
            Response::Drives(b"C".to_vec())
        );
        let volume = [
            0, 0, 0, 0, 0x0b, 0, b'B', b'S', b'D', b'F', b'I', b'L', b'E', b'R', b' ', b' ', b' ',
            0x00, 0x40, 0x32, 0x30,
        ];
        assert_eq!(
            decode(Code::VolumeInfo, &volume),
            Response::Volume {
                label: b"BSDFILER   ".to_vec(),
                free: 0x3032_4000
            }
        );
        assert_eq!(
            decode(
                Code::CurrentDir,
                &[0, 0, 0, 0, 3, 0, b'C', b':', b'\\', 0, 0, 0, 0]
            ),
            Response::Directory(b"C:\\".to_vec())
        );
        let entry = [
            0, 0, 0, 0, 0x16, 0, 0x10, 0xc4, 0xa9, 0x68, 0x5d, 0, 0x10, 0, 0, b'S', b'U', b'B', 0,
            b'Y', b'.', b'D', b'A', b'T', 0, 0, 0, 0, 1, 0, 0, 0,
        ];
        let Response::Entry(Some(e)) = decode(Code::FindNext, &entry) else {
            panic!("no entry")
        };
        assert_eq!(e.name, b"SUB");
        assert!(e.is_directory());
        // BSD Filer gives a directory its Unix size.
        assert_eq!(e.size, 4096);
        assert_eq!(e.modified.to_string(), "2026-11-08 21:14:08");
        let mut end = entry;
        end[28] = 0;
        assert_eq!(decode(Code::FindNext, &end), Response::Entry(None));
    }

    #[test]
    fn responses_round_trip() {
        let all = [
            (Code::Close, Response::Done),
            (Code::Drives, Response::Drives(b"ACD".to_vec())),
            (Code::CurrentDrive, Response::Drive(b'C')),
            (
                Code::VolumeInfo,
                Response::Volume {
                    label: b"           ".to_vec(),
                    free: 12345,
                },
            ),
            (Code::CurrentDir, Response::Directory(b"C:\\_DAT".to_vec())),
            (Code::TestName, Response::Kind(NameKind::Directory)),
            (
                Code::FindNext,
                Response::Entry(Some(DirEntry {
                    attributes: attr::ARCHIVE,
                    modified: DosDateTime {
                        time: 0x6E32,
                        date: 0x318E,
                    },
                    size: 41,
                    name: b"ABCD.XYZ".to_vec(),
                })),
            ),
            (Code::FindNext, Response::Entry(None)),
            (Code::Open, Response::Handle(3)),
            (Code::Read, Response::Data(vec![1, 2, 0x10])),
            (Code::GetTime, Response::Time(DosDateTime::default())),
            (Code::Tell, Response::Position(77)),
            (Code::GetAttr, Response::Attributes(0x21)),
        ];
        for (code, r) in all {
            assert_eq!(
                Response::decode(code, &r.encode()),
                Ok(Ok(r.clone())),
                "{r:?}"
            );
        }
        // The drive list's layout from the wiki: count 4, A C D 0, four zeros.
        assert_eq!(
            Response::Drives(b"ACD".to_vec()).encode(),
            [0, 0, 0, 0, 4, 0, b'A', b'C', b'D', 0, 0, 0, 0, 0]
        );
        assert_eq!(Response::Done.encode(), [0; 6]);
    }

    #[test]
    fn failures() {
        assert_eq!(
            Response::decode(Code::Open, &encode_failure(Failure::Fs(FsError::NOT_FOUND))),
            Ok(Err(Failure::Fs(FsError::NOT_FOUND)))
        );
        assert_eq!(
            Response::decode(Code::Open, &[0x3e, 0xff]),
            Ok(Err(Failure::Status(0xFF3E)))
        );
        assert!(Response::decode(Code::Read, &[0, 0, 0, 0, 5, 0, 1]).is_err());
        assert!(Response::decode(Code::FindNext, &[0, 0, 0, 0]).is_err());
        // BSD Filer: no data for 0x03, the two words alone for 0x15.
        assert_eq!(
            Response::decode(Code::Create, &[]),
            Ok(Ok(Response::Handle(0)))
        );
        assert_eq!(
            Response::decode(Code::Write, &[0, 0, 0, 0]),
            Ok(Ok(Response::Done))
        );
    }
}
