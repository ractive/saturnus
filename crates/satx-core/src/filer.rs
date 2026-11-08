//! The HP 100LX/200LX palmtops: their Filer in server mode over a
//! [`Transport`], blocking, on the system clock ([`Palmtop`]). The
//! protocol is `filer-proto`'s; this module only drives its
//! [`Session`] (wiki: protocols/filer-hplx).
//!
//! On the palmtop: Filer, MENU, Communications, Remote Settings: Baud as
//! here (19200 by default), Interface Com1, Server Mode enabled, OK; then
//! leave Filer open and in front.

use std::fmt;

use filer_proto::{
    Config, DirEntry, DosDateTime, DriveInfo, Job, JobError, NameKind, Output, Session,
    SessionEvent,
};

use crate::time::{Duration, Instant};
use crate::transport::{self, Transport};
use crate::{Error, Result};

/// The default speed: the comp.sys.palmtops FAQ's safe limit for the
/// palmtop's 8250 UART (wiki: hardware/hp100lx-200lx).
pub const DEFAULT_BAUD: u32 = 19200;

/// How long stale input is read and dropped before CONNECT.
const DRAIN: Duration = Duration::from_millis(100);
/// The longest wait handed to one read while nothing is due.
const IDLE_READ: Duration = Duration::from_millis(100);

/// A progress callback: bytes so far, and all bytes when known.
pub type Progress = Box<dyn FnMut(u64, Option<u64>) + Send>;

/// A connected palmtop.
pub struct Palmtop {
    transport: Box<dyn Transport>,
    session: Session,
    progress: Option<Progress>,
    revision: u8,
}

impl fmt::Debug for Palmtop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Palmtop")
            .field("session", &self.session)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl Palmtop {
    /// Open `addr` (a serial device at `baud`, or `tcp://host:port`) and
    /// connect.
    pub fn open(addr: &str, baud: u32) -> Result<Self> {
        let transport = transport::open_at(addr, baud)?;
        let mut config = Config::default();
        config.baud = baud;
        Self::connect(transport, config)
    }

    /// Connect over an open link.
    pub fn connect(mut transport: Box<dyn Transport>, config: Config) -> Result<Self> {
        transport::drain(transport.as_mut(), DRAIN)?;
        let mut palmtop = Palmtop {
            transport,
            session: Session::new(config),
            progress: None,
            revision: 0,
        };
        if let Output::Connected { revision } = palmtop.run(Job::Connect)? {
            palmtop.revision = revision;
        }
        Ok(palmtop)
    }

    /// The session's protocol revision (2, or 1 for an older server).
    pub fn revision(&self) -> u8 {
        self.revision
    }

    /// Report the bytes of each get and put block.
    pub fn set_progress(&mut self, progress: Option<Progress>) {
        self.progress = progress;
    }

    /// Run one job to its end. A link that fails (a closed TCP
    /// connection, a serial error) ends the session.
    pub fn run(&mut self, job: Job) -> Result<Output> {
        self.session
            .start(Instant::now(), job)
            .map_err(|e| Error::Reply(format!("filer: {e}")))?;
        let result = self.drive();
        if result.is_err() && self.session.is_busy() {
            self.session.disconnect(Instant::now());
            while self.session.poll_output(Instant::now()).is_some() {}
        }
        result
    }

    fn drive(&mut self) -> Result<Output> {
        let mut buf = [0u8; 4096];
        loop {
            self.flush()?;
            while let Some(event) = self.session.poll_event() {
                match event {
                    SessionEvent::Progress { done, total } => {
                        if let Some(p) = self.progress.as_mut() {
                            p(done, total);
                        }
                    }
                    SessionEvent::Finished(result) => {
                        // A failed session queued its DISCONNECT.
                        self.flush()?;
                        return result.map_err(Error::Filer);
                    }
                }
            }
            let now = Instant::now();
            let wait = self
                .session
                .next_timeout()
                .map_or(IDLE_READ, |t| t.saturating_duration_since(now));
            let n = self.transport.read(&mut buf, wait)?;
            let now = Instant::now();
            if n > 0 {
                self.session.handle_input(now, &buf[..n]);
            }
            self.session.handle_timeout(now);
        }
    }

    fn flush(&mut self) -> Result<()> {
        while let Some(frame) = self.session.poll_output(Instant::now()) {
            self.transport.write_packet(&frame)?;
        }
        Ok(())
    }

    /// The drives; with `volumes` each one's label and free space too.
    pub fn drives(&mut self, volumes: bool) -> Result<Vec<DriveInfo>> {
        match self.run(Job::Drives { volumes })? {
            Output::Drives(d) => Ok(d),
            other => Err(unexpected(&other)),
        }
    }

    /// The entries matching a DOS pattern (`C:\*.*`), without `.` and `..`.
    pub fn list(&mut self, pattern: &[u8]) -> Result<Vec<DirEntry>> {
        match self.run(Job::List(pattern.to_vec()))? {
            Output::Listing(entries) => Ok(entries),
            other => Err(unexpected(&other)),
        }
    }

    /// What a name is.
    pub fn kind(&mut self, path: &[u8]) -> Result<NameKind> {
        match self.run(Job::Kind(path.to_vec()))? {
            Output::Kind(kind) => Ok(kind),
            other => Err(unexpected(&other)),
        }
    }

    /// A whole file and its time.
    pub fn get(&mut self, path: &[u8]) -> Result<(Vec<u8>, Option<DosDateTime>)> {
        match self.run(Job::Get(path.to_vec()))? {
            Output::File { data, modified } => Ok((data, modified)),
            other => Err(unexpected(&other)),
        }
    }

    /// Write a whole file; without `overwrite` an existing name is
    /// [`JobError::Exists`].
    pub fn put(
        &mut self,
        path: &[u8],
        data: &[u8],
        modified: Option<DosDateTime>,
        overwrite: bool,
    ) -> Result<()> {
        self.done(Job::Put {
            path: path.to_vec(),
            data: data.to_vec(),
            modified,
            overwrite,
        })
    }

    /// Delete a file.
    pub fn delete(&mut self, path: &[u8]) -> Result<()> {
        self.done(Job::Delete(path.to_vec()))
    }

    /// Make a directory.
    pub fn make_dir(&mut self, path: &[u8]) -> Result<()> {
        self.done(Job::MakeDir(path.to_vec()))
    }

    /// Remove an empty directory.
    pub fn remove_dir(&mut self, path: &[u8]) -> Result<()> {
        self.done(Job::RemoveDir(path.to_vec()))
    }

    /// Rename or move within a drive; refused when `to` exists.
    pub fn rename(&mut self, from: &[u8], to: &[u8]) -> Result<()> {
        self.done(Job::Rename {
            from: from.to_vec(),
            to: to.to_vec(),
        })
    }

    fn done(&mut self, job: Job) -> Result<()> {
        match self.run(job)? {
            Output::Done => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// End the session: one DISCONNECT, not awaited.
    pub fn close(mut self) -> Result<()> {
        self.disconnect()
    }

    fn disconnect(&mut self) -> Result<()> {
        if self.session.is_open() {
            self.session.disconnect(Instant::now());
            self.flush()?;
        }
        Ok(())
    }
}

impl Drop for Palmtop {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

fn unexpected(output: &Output) -> Error {
    Error::Reply(format!("filer: unexpected result {output:?}"))
}

/// The file-system error inside an [`Error`], when the palmtop refused a
/// request with one.
pub fn fs_error(error: &Error) -> Option<filer_proto::FsError> {
    match error {
        Error::Filer(e) => e.fs_error(),
        _ => None,
    }
}

/// Whether an [`Error`] says the name exists.
pub fn is_exists(error: &Error) -> bool {
    matches!(error, Error::Filer(JobError::Exists))
        || fs_error(error) == Some(filer_proto::FsError::EXISTS)
}
