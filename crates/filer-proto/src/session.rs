//! What a file manager does, as sequences of requests over a [`Client`]:
//! list, get, put, rename, delete, make and remove directories, the drive
//! list. The sequences are those HP's own client sends (wiki:
//! protocols/filer-hplx, "HP's client sequences").
//!
//! [`Session`] has the same seam as [`Client`]: it never reads a clock or
//! touches a port. Start a [`Job`], drive the session, and take its
//! [`SessionEvent::Finished`].

use std::collections::VecDeque;
use std::fmt;

use crate::client::{self, Client, Config, Event, StartError};
use crate::codec::MAX_BLOCK;
use crate::command::{DirEntry, DosDateTime, NameKind, Request, Response, attr};
use crate::error::{Failure, FsError};
use crate::time::Instant;

/// The largest file [`Job::Get`] reads by default (16 MiB; the palmtops'
/// cards hold far less).
pub const MAX_FILE: usize = 16 << 20;
/// Listing entries after which [`Job::List`] gives up on a server that
/// never ends its listing.
const MAX_ENTRIES: usize = 65_536;

/// One thing to do.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Job {
    /// Open the session (CONNECT).
    Connect,
    /// The drive letters; with `volumes` also each drive's label and free
    /// space (select it, ask; the current drive changes).
    Drives {
        /// Ask each drive for its volume information.
        volumes: bool,
    },
    /// List a path with a DOS pattern (`C:\*.*`). `.` and `..` are left
    /// out.
    List(Vec<u8>),
    /// Read a whole file.
    Get(Vec<u8>),
    /// Write a whole file.
    Put {
        /// Where.
        path: Vec<u8>,
        /// The contents.
        data: Vec<u8>,
        /// The time to give it (`None`: the server's now).
        modified: Option<DosDateTime>,
        /// Replace an existing file (create, then on 217 create-or-replace).
        overwrite: bool,
    },
    /// Delete a file.
    Delete(Vec<u8>),
    /// Make a directory.
    MakeDir(Vec<u8>),
    /// Remove an empty directory.
    RemoveDir(Vec<u8>),
    /// Rename or move within a drive; refused when `to` exists.
    Rename {
        /// The old name.
        from: Vec<u8>,
        /// The new name.
        to: Vec<u8>,
    },
    /// What a name is.
    Kind(Vec<u8>),
    /// One request as it is.
    Raw(Request),
}

/// A drive and, when asked for, its volume information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriveInfo {
    /// The letter.
    pub letter: u8,
    /// Label and free bytes, or why the drive did not answer (215 for an
    /// empty card slot); `None` when not asked for.
    pub volume: Option<Result<(Vec<u8>, u32), Failure>>,
}

/// What a finished job gives.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Output {
    /// The session is open.
    Connected {
        /// Its revision.
        revision: u8,
    },
    /// The drives.
    Drives(Vec<DriveInfo>),
    /// The entries of a listing.
    Listing(Vec<DirEntry>),
    /// A file read.
    File {
        /// Its contents.
        data: Vec<u8>,
        /// Its time, when the server gave one.
        modified: Option<DosDateTime>,
    },
    /// What a name is.
    Kind(NameKind),
    /// Done.
    Done,
    /// The reply to a [`Job::Raw`] request.
    Response(Response),
}

/// Why a job failed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum JobError {
    /// The server refused a request (the session stays open).
    Failed(Failure),
    /// The name exists (a put without `overwrite`, a rename's target).
    Exists,
    /// The file is larger than the session's limit.
    TooLarge {
        /// The limit in bytes.
        limit: usize,
    },
    /// The link failed and the session is over.
    Link(client::Error),
}

impl JobError {
    /// The file-system code, when the server gave one.
    pub fn fs_error(&self) -> Option<FsError> {
        match self {
            JobError::Failed(Failure::Fs(e)) => Some(*e),
            _ => None,
        }
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobError::Failed(e) => e.fmt(f),
            JobError::Exists => f.write_str("the name exists"),
            JobError::TooLarge { limit } => write!(f, "file larger than {limit} bytes"),
            JobError::Link(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for JobError {}

/// What a [`Session`] reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    /// Bytes moved so far in a get or put; `total` when known.
    Progress {
        /// Bytes so far.
        done: u64,
        /// All bytes, when known (a get learns it from a listing).
        total: Option<u64>,
    },
    /// The job is over.
    Finished(Result<Output, JobError>),
}

type Reply = Result<Response, Failure>;

#[derive(Debug)]
enum GetStep {
    Open,
    FindFirst,
    FindNext,
    FindClose,
    Read,
    Time,
    Close,
}

#[derive(Debug)]
enum PutStep {
    /// 0x03 sent.
    Create,
    /// 0x05 sent after a 217.
    Replace,
    Write,
    Time,
    Close,
    Delete,
}

#[derive(Debug)]
enum Running {
    Connect,
    Single,
    Drives {
        volumes: bool,
        letters: Vec<u8>,
        index: usize,
        selected: bool,
        out: Vec<DriveInfo>,
    },
    List {
        entries: Vec<DirEntry>,
    },
    Get {
        path: Vec<u8>,
        step: GetStep,
        handle: u16,
        data: Vec<u8>,
        size: Option<u64>,
        modified: Option<DosDateTime>,
        error: Option<JobError>,
    },
    Put {
        path: Vec<u8>,
        data: Vec<u8>,
        modified: Option<DosDateTime>,
        overwrite: bool,
        step: PutStep,
        handle: u16,
        position: usize,
        error: Option<JobError>,
    },
    /// Does a name exist? A listing of the name itself (0x12, 0x09, 0x16),
    /// which every server answers for files too (BSD Filer's 0x08 says 0
    /// for a file, and its 0x03 overwrites without a 217). Found: the job
    /// fails with [`JobError::Exists`]; else `then` runs, from `request`.
    Probe {
        step: ProbeStep,
        /// The name looked for, without its path.
        name: Vec<u8>,
        found: bool,
        then: Box<Running>,
        request: Request,
    },
}

#[derive(Debug)]
enum ProbeStep {
    First,
    Next,
    Close,
}

/// `then` from `request`, unless `path` exists.
fn probe(path: Vec<u8>, then: Running, request: Request) -> (Running, Request) {
    (
        Running::Probe {
            step: ProbeStep::First,
            name: crate::path::file_name(&path).to_vec(),
            found: false,
            then: Box::new(then),
            request,
        },
        Request::FindFirst {
            pattern: path,
            attributes: u16::from(attr::DIRECTORY),
        },
    )
}

/// Jobs over one Filer session.
#[derive(Debug)]
pub struct Session {
    client: Client,
    running: Option<Running>,
    events: VecDeque<SessionEvent>,
    max_file: usize,
}

impl Session {
    /// A session, not connected yet ([`Job::Connect`] first).
    pub fn new(config: Config) -> Self {
        Session {
            client: Client::new(config),
            running: None,
            events: VecDeque::new(),
            max_file: MAX_FILE,
        }
    }

    /// The request-level client underneath.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Set the largest file a get reads.
    pub fn set_max_file(&mut self, bytes: usize) {
        self.max_file = bytes;
    }

    /// Whether the session is open.
    pub fn is_open(&self) -> bool {
        self.client.is_open()
    }

    /// Whether a job runs.
    pub fn is_busy(&self) -> bool {
        self.running.is_some()
    }

    /// Start a job; its [`SessionEvent::Finished`] follows.
    pub fn start(&mut self, now: Instant, job: Job) -> Result<(), StartError> {
        if self.running.is_some() {
            return Err(StartError::Busy);
        }
        if job == Job::Connect {
            self.client.connect(now)?;
            self.running = Some(Running::Connect);
            return Ok(());
        }
        if !self.client.is_open() {
            return Err(StartError::NotConnected);
        }
        let (running, first) = match job {
            Job::Connect => return Err(StartError::SessionCommand),
            Job::Drives { volumes } => {
                let running = Running::Drives {
                    volumes,
                    letters: Vec::new(),
                    index: 0,
                    selected: false,
                    out: Vec::new(),
                };
                if self.client.revision() == 1 {
                    // A revision-1 server is not asked; C: is assumed.
                    self.running = Some(running);
                    self.drives_step(now, None, Some(vec![b'C']));
                    return Ok(());
                }
                (running, Request::Drives)
            }
            Job::List(pattern) => (
                Running::List {
                    entries: Vec::new(),
                },
                Request::FindFirst {
                    pattern,
                    attributes: u16::from(attr::DIRECTORY),
                },
            ),
            Job::Get(path) => (
                Running::Get {
                    path: path.clone(),
                    step: GetStep::Open,
                    handle: 0,
                    data: Vec::new(),
                    size: None,
                    modified: None,
                    error: None,
                },
                Request::Open {
                    name: path,
                    mode: 0,
                },
            ),
            Job::Put {
                path,
                data,
                modified,
                overwrite,
            } => {
                let put = Running::Put {
                    path: path.clone(),
                    data,
                    modified,
                    overwrite,
                    step: PutStep::Create,
                    handle: 0,
                    position: 0,
                    error: None,
                };
                let create = Request::Create {
                    name: path.clone(),
                    attributes: 0,
                };
                if overwrite {
                    (put, create)
                } else {
                    probe(path, put, create)
                }
            }
            Job::Delete(path) => (Running::Single, Request::Delete(path)),
            Job::MakeDir(path) => (Running::Single, Request::MakeDir(path)),
            Job::RemoveDir(path) => (Running::Single, Request::RemoveDir(path)),
            Job::Kind(path) => (Running::Single, Request::TestName(path)),
            Job::Raw(request) => (Running::Single, request),
            Job::Rename { from, to } => {
                probe(to.clone(), Running::Single, Request::Rename { from, to })
            }
        };
        self.client.send(now, &first)?;
        self.running = Some(running);
        Ok(())
    }

    /// End the session (one DISCONNECT, not awaited). A running job is
    /// dropped without an event.
    pub fn disconnect(&mut self, now: Instant) {
        self.running = None;
        self.client.disconnect(now);
        while self.client.poll_event().is_some() {}
    }

    /// Feed bytes read from the link.
    pub fn handle_input(&mut self, now: Instant, bytes: &[u8]) {
        self.client.handle_input(now, bytes);
        self.pump(now);
    }

    /// Tell the session time has passed (at [`Session::next_timeout`]).
    pub fn handle_timeout(&mut self, now: Instant) {
        self.client.handle_timeout(now);
        self.pump(now);
    }

    /// The next frame to write, whole.
    pub fn poll_output(&mut self, now: Instant) -> Option<Vec<u8>> {
        self.client.poll_output(now)
    }

    /// The next event.
    pub fn poll_event(&mut self) -> Option<SessionEvent> {
        self.events.pop_front()
    }

    /// When the session wants [`Session::handle_timeout`].
    pub fn next_timeout(&self) -> Option<Instant> {
        self.client.next_timeout()
    }

    fn pump(&mut self, now: Instant) {
        while let Some(event) = self.client.poll_event() {
            match event {
                Event::Connected { revision } => {
                    if matches!(self.running, Some(Running::Connect)) {
                        self.finish(Ok(Output::Connected { revision }));
                    }
                }
                Event::Reply(reply) => self.advance(now, reply),
                Event::Disconnected => {}
                Event::Error(e) => {
                    if self.running.is_some() {
                        self.finish(Err(JobError::Link(e)));
                    }
                }
            }
        }
    }

    fn finish(&mut self, result: Result<Output, JobError>) {
        self.running = None;
        self.events.push_back(SessionEvent::Finished(result));
    }

    fn send(&mut self, now: Instant, request: Request) {
        if let Err(e) = self.client.send(now, &request) {
            // Not reachable while a job runs on an open session; report it
            // rather than hang.
            let _ = e;
            self.finish(Err(JobError::Link(client::Error::NotConnected)));
        }
    }

    fn advance(&mut self, now: Instant, reply: Reply) {
        let Some(running) = self.running.take() else {
            return;
        };
        match running {
            Running::Connect => self.running = Some(Running::Connect),
            Running::Single => self.finish(match reply {
                Ok(Response::Kind(kind)) => Ok(Output::Kind(kind)),
                Ok(Response::Done) => Ok(Output::Done),
                Ok(other) => Ok(Output::Response(other)),
                Err(f) => Err(JobError::Failed(f)),
            }),
            Running::Probe {
                step,
                name,
                mut found,
                then,
                request,
            } => {
                let step = match step {
                    ProbeStep::First if reply.is_err() => None,
                    ProbeStep::First => Some((ProbeStep::Next, Request::FindNext)),
                    // The entry must carry the name: BSD Filer answers a
                    // missing name with the directory's `.` entry.
                    ProbeStep::Next => {
                        found = matches!(
                            &reply,
                            Ok(Response::Entry(Some(e))) if e.name.eq_ignore_ascii_case(&name)
                        );
                        Some((ProbeStep::Close, Request::FindClose))
                    }
                    ProbeStep::Close => None,
                };
                match step {
                    Some((step, next)) => {
                        self.running = Some(Running::Probe {
                            step,
                            name,
                            found,
                            then,
                            request,
                        });
                        self.send(now, next);
                    }
                    None if found => self.finish(Err(JobError::Exists)),
                    None => {
                        self.running = Some(*then);
                        self.send(now, request);
                    }
                }
            }
            Running::List { mut entries } => match reply {
                Ok(Response::Entry(Some(e))) => {
                    if e.name != b"." && e.name != b".." {
                        entries.push(e);
                    }
                    if entries.len() > MAX_ENTRIES {
                        return self.finish(Err(JobError::Failed(Failure::Fs(FsError::INTERNAL))));
                    }
                    self.running = Some(Running::List { entries });
                    self.send(now, Request::FindNext);
                }
                Ok(Response::Entry(None)) => self.finish(Ok(Output::Listing(entries))),
                // The reply to 0x12.
                Ok(_) => {
                    self.running = Some(Running::List { entries });
                    self.send(now, Request::FindNext);
                }
                Err(f) => self.finish(Err(JobError::Failed(f))),
            },
            Running::Drives { .. } => {
                self.running = Some(running);
                self.drives_step(now, Some(reply), None);
            }
            Running::Get { .. } => {
                self.running = Some(running);
                self.get_step(now, reply);
            }
            Running::Put { .. } => {
                self.running = Some(running);
                self.put_step(now, reply);
            }
        }
    }

    fn drives_step(&mut self, now: Instant, reply: Option<Reply>, letters_now: Option<Vec<u8>>) {
        let Some(Running::Drives {
            volumes,
            mut letters,
            mut index,
            mut selected,
            mut out,
        }) = self.running.take()
        else {
            return;
        };
        if let Some(l) = letters_now {
            letters = l;
            out = letters
                .iter()
                .map(|&letter| DriveInfo {
                    letter,
                    volume: None,
                })
                .collect();
        } else if letters.is_empty() && out.is_empty() {
            // The reply to 0x1B.
            match reply {
                Some(Ok(Response::Drives(l))) => {
                    out = l
                        .iter()
                        .map(|&letter| DriveInfo {
                            letter,
                            volume: None,
                        })
                        .collect();
                    letters = l;
                }
                Some(Ok(_)) => {}
                Some(Err(f)) => return self.finish(Err(JobError::Failed(f))),
                None => {}
            }
        } else if let Some(reply) = reply {
            if selected {
                out[index].volume = Some(match reply {
                    Ok(Response::Volume { label, free }) => Ok((label, free)),
                    Ok(_) => Err(Failure::Fs(FsError::INTERNAL)),
                    Err(f) => Err(f),
                });
                index += 1;
                selected = false;
            } else {
                match reply {
                    Ok(_) => selected = true,
                    Err(f) => {
                        out[index].volume = Some(Err(f));
                        index += 1;
                    }
                }
            }
        }
        if !volumes || index >= letters.len() {
            return self.finish(Ok(Output::Drives(out)));
        }
        let request = if selected {
            Request::VolumeInfo
        } else {
            Request::SetDrive(letters[index])
        };
        self.running = Some(Running::Drives {
            volumes,
            letters,
            index,
            selected,
            out,
        });
        self.send(now, request);
    }

    fn get_step(&mut self, now: Instant, reply: Reply) {
        let max_file = self.max_file;
        let Some(Running::Get {
            path,
            step,
            mut handle,
            mut data,
            mut size,
            mut modified,
            mut error,
        }) = self.running.take()
        else {
            return;
        };
        let next = match step {
            GetStep::Open => match reply {
                Ok(Response::Handle(h)) => {
                    handle = h;
                    Some((
                        GetStep::FindFirst,
                        Request::FindFirst {
                            pattern: path.clone(),
                            attributes: 0,
                        },
                    ))
                }
                Ok(_) => return self.finish(Err(JobError::Failed(Failure::Fs(FsError::INTERNAL)))),
                Err(f) => return self.finish(Err(JobError::Failed(f))),
            },
            // The size, for the progress; a failure only loses that.
            GetStep::FindFirst => match reply {
                Ok(_) => Some((GetStep::FindNext, Request::FindNext)),
                Err(_) => Some((
                    GetStep::Read,
                    Request::Read {
                        handle,
                        len: MAX_BLOCK as u16,
                    },
                )),
            },
            GetStep::FindNext => {
                if let Ok(Response::Entry(Some(e))) = reply {
                    size = Some(u64::from(e.size));
                    modified = Some(e.modified);
                }
                Some((GetStep::FindClose, Request::FindClose))
            }
            GetStep::FindClose => Some((
                GetStep::Read,
                Request::Read {
                    handle,
                    len: MAX_BLOCK as u16,
                },
            )),
            GetStep::Read => match reply {
                Ok(Response::Data(block)) if block.is_empty() => {
                    Some((GetStep::Time, Request::GetTime(handle)))
                }
                Ok(Response::Data(block)) => {
                    data.extend_from_slice(&block);
                    self.events.push_back(SessionEvent::Progress {
                        done: data.len() as u64,
                        total: size,
                    });
                    if data.len() > max_file {
                        error = Some(JobError::TooLarge { limit: max_file });
                        Some((GetStep::Close, Request::Close(handle)))
                    } else {
                        Some((
                            GetStep::Read,
                            Request::Read {
                                handle,
                                len: MAX_BLOCK as u16,
                            },
                        ))
                    }
                }
                Ok(_) => {
                    error = Some(JobError::Failed(Failure::Fs(FsError::INTERNAL)));
                    Some((GetStep::Close, Request::Close(handle)))
                }
                Err(f) => {
                    error = Some(JobError::Failed(f));
                    Some((GetStep::Close, Request::Close(handle)))
                }
            },
            // The listing's time wins: BSD Filer's 0x17 gives no valid
            // time; HP's server should give the same in both.
            GetStep::Time => {
                if let (None, Ok(Response::Time(t))) = (modified, reply) {
                    modified = Some(t);
                }
                Some((GetStep::Close, Request::Close(handle)))
            }
            GetStep::Close => {
                let result = match (error, reply) {
                    (Some(e), _) => Err(e),
                    (None, Err(f)) => Err(JobError::Failed(f)),
                    (None, Ok(_)) => Ok(Output::File { data, modified }),
                };
                return self.finish(result);
            }
        };
        if let Some((step, request)) = next {
            self.running = Some(Running::Get {
                path,
                step,
                handle,
                data,
                size,
                modified,
                error,
            });
            self.send(now, request);
        }
    }

    fn put_step(&mut self, now: Instant, reply: Reply) {
        let Some(Running::Put {
            path,
            data,
            modified,
            overwrite,
            step,
            mut handle,
            mut position,
            mut error,
        }) = self.running.take()
        else {
            return;
        };
        let write = |handle: u16, position: usize, data: &[u8]| {
            let end = (position + MAX_BLOCK).min(data.len());
            Request::Write {
                handle,
                data: data[position..end].to_vec(),
            }
        };
        let after_open = |handle: u16, data: &[u8]| {
            if data.is_empty() {
                match modified {
                    Some(t) => (
                        PutStep::Time,
                        Request::SetTime {
                            handle,
                            modified: t,
                        },
                    ),
                    None => (PutStep::Close, Request::Close(handle)),
                }
            } else {
                (PutStep::Write, write(handle, 0, data))
            }
        };
        let (step, request) = match step {
            PutStep::Create | PutStep::Replace => match reply {
                Ok(Response::Handle(h)) => {
                    handle = h;
                    after_open(h, &data)
                }
                Err(Failure::Fs(FsError::EXISTS))
                    if overwrite && matches!(step, PutStep::Create) =>
                {
                    (
                        PutStep::Replace,
                        Request::Replace {
                            name: path.clone(),
                            attributes: 0,
                        },
                    )
                }
                Err(Failure::Fs(FsError::EXISTS)) => return self.finish(Err(JobError::Exists)),
                Ok(_) => return self.finish(Err(JobError::Failed(Failure::Fs(FsError::INTERNAL)))),
                Err(f) => return self.finish(Err(JobError::Failed(f))),
            },
            PutStep::Write => match reply {
                Ok(_) => {
                    position = (position + MAX_BLOCK).min(data.len());
                    self.events.push_back(SessionEvent::Progress {
                        done: position as u64,
                        total: Some(data.len() as u64),
                    });
                    if position < data.len() {
                        (PutStep::Write, write(handle, position, &data))
                    } else if let Some(t) = modified {
                        (
                            PutStep::Time,
                            Request::SetTime {
                                handle,
                                modified: t,
                            },
                        )
                    } else {
                        (PutStep::Close, Request::Close(handle))
                    }
                }
                Err(f) => {
                    error = Some(JobError::Failed(f));
                    (PutStep::Close, Request::Close(handle))
                }
            },
            // A server that cannot set the time still gets the file.
            PutStep::Time => (PutStep::Close, Request::Close(handle)),
            PutStep::Close => match (reply, error.take()) {
                (Ok(_), None) => return self.finish(Ok(Output::Done)),
                (Err(f), None) => {
                    error = Some(JobError::Failed(f));
                    (PutStep::Delete, Request::Delete(path.clone()))
                }
                (_, Some(e)) => {
                    error = Some(e);
                    (PutStep::Delete, Request::Delete(path.clone()))
                }
            },
            // The partial file is gone (or could not be deleted): report
            // the first failure.
            PutStep::Delete => {
                return self.finish(Err(
                    error.unwrap_or(JobError::Failed(Failure::Fs(FsError::INTERNAL)))
                ));
            }
        };
        self.running = Some(Running::Put {
            path,
            data,
            modified,
            overwrite,
            step,
            handle,
            position,
            error,
        });
        self.send(now, request);
    }
}
