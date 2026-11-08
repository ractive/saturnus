//! Status words and file-system result codes, with their meanings (wiki:
//! protocols/filer-hplx, "Errors").

use std::fmt;

/// Every reply's data starts with a status word: 0 when the server ran the
/// request, otherwise one of these protocol-level refusals (with no
/// further data).
pub mod status {
    /// The server accepted and ran the request.
    pub const OK: u16 = 0;
    /// Not a valid CONNECT while no session is open; sent in a CONNECT
    /// reply (command 0x40, counter 0). Also asks the client to fall back
    /// to revision 1.
    pub const NOT_CONNECTED: u16 = 0xFF3B;
    /// The counter is neither the last one nor the next one; not run.
    pub const OUT_OF_SEQUENCE: u16 = 0xFF3C;
    /// All 8 file slots in use at an open or create (the server leaves the
    /// session without replying).
    pub const NO_SLOT: u16 = 0xFF3D;
    /// Unknown command.
    pub const UNKNOWN_COMMAND: u16 = 0xFF3E;
    /// HP's client: the transmit queue made no progress for 5 s.
    pub const TRANSMIT_STUCK: u16 = 0xFF38;
    /// HP's client: no reply, or no complete valid frame, after all
    /// attempts.
    pub const NO_REPLY: u16 = 0xFF39;
    /// HP's client: the reply's counter differs from the request's, or its
    /// direction byte is not 0x81.
    pub const BAD_REPLY: u16 = 0xFF3A;
}

/// What a status word means, or `None` for a code the notes do not list.
pub fn status_meaning(code: u16) -> Option<&'static str> {
    Some(match code {
        status::OK => "ok",
        status::NOT_CONNECTED => "no session open on the server",
        status::OUT_OF_SEQUENCE => "packet counter out of sequence",
        status::NO_SLOT => "all 8 file slots in use",
        status::UNKNOWN_COMMAND => "unknown command",
        status::TRANSMIT_STUCK => "the transmit queue made no progress",
        status::NO_REPLY => "no reply in time",
        status::BAD_REPLY => "reply with the wrong counter or direction",
        _ => return None,
    })
}

/// A file-system result code, the second word of every reply except
/// CONNECT and DISCONNECT. The System Manager's file error codes, 200 to
/// 222; their meanings come from matching the DOS errors behind each code
/// with the order of the system's texts (inferred on the wiki).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FsError(pub u16);

impl FsError {
    /// Invalid file or path name.
    pub const INVALID_NAME: FsError = FsError(200);
    /// Disk full.
    pub const DISK_FULL: FsError = FsError(203);
    /// Internal software error (the catch-all).
    pub const INTERNAL: FsError = FsError(207);
    /// File does not exist.
    pub const NOT_FOUND: FsError = FsError(208);
    /// Directory (path) does not exist, or invalid drive.
    pub const PATH_NOT_FOUND: FsError = FsError(209);
    /// Too many open files.
    pub const TOO_MANY_OPEN: FsError = FsError(210);
    /// Access denied: a read-only file, a non-empty directory, a file
    /// created over an existing directory.
    pub const ACCESS_DENIED: FsError = FsError(211);
    /// Cannot delete the current directory.
    pub const CURRENT_DIR: FsError = FsError(212);
    /// Cannot move a file to another drive.
    pub const OTHER_DRIVE: FsError = FsError(213);
    /// Disk write-protected.
    pub const WRITE_PROTECTED: FsError = FsError(214);
    /// Drive not ready.
    pub const NOT_READY: FsError = FsError(215);
    /// File already exists (create, 0x03, on an existing name).
    pub const EXISTS: FsError = FsError(217);
    /// Cannot create directory.
    pub const CANNOT_MKDIR: FsError = FsError(218);

    /// The code.
    pub fn code(self) -> u16 {
        self.0
    }

    /// Its meaning, or `None` for a code outside 200-222.
    pub fn meaning(self) -> Option<&'static str> {
        Some(match self.0 {
            200 => "invalid file or path name",
            201 => "cannot find system directory",
            202 => "too many alarms",
            203 => "disk full",
            204 => "print queue error",
            205 => "sharing or network error",
            206 => "I/O device error",
            207 => "internal software error",
            208 => "file does not exist",
            209 => "directory does not exist, or invalid drive",
            210 => "too many open files",
            211 => "access denied",
            212 => "cannot delete the current directory",
            213 => "cannot move a file to another drive",
            214 => "disk write-protected",
            215 => "drive not ready",
            216 => "printer out of paper",
            217 => "file already exists",
            218 => "cannot create directory",
            219 => "critical-error handler failure",
            220 => "too many redirections",
            221 => "duplicate redirection",
            222 => "insufficient memory",
            _ => return None,
        })
    }
}

impl fmt::Debug for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FsError({self})")
    }
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.meaning() {
            Some(m) => write!(f, "{m} ({})", self.0),
            None => write!(f, "file error {}", self.0),
        }
    }
}

impl std::error::Error for FsError {}

/// Why one request failed, the session still open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// A non-zero status word (see [`status`]).
    Status(u16),
    /// A non-zero result word.
    Fs(FsError),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Status(code) => match status_meaning(*code) {
                Some(m) => write!(f, "{m} (status {code:#06x})"),
                None => write!(f, "status {code:#06x}"),
            },
            Failure::Fs(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Failure {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meanings() {
        assert_eq!(FsError(208).to_string(), "file does not exist (208)");
        assert_eq!(FsError(199).to_string(), "file error 199");
        assert!((200..=222).all(|c| FsError(c).meaning().is_some()));
        assert_eq!(
            Failure::Status(0xFF3E).to_string(),
            "unknown command (status 0xff3e)"
        );
        assert_eq!(Failure::Status(0x1234).to_string(), "status 0x1234");
    }
}
