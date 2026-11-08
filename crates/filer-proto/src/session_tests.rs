//! [`Session`] against [`Server`] over [`MemoryFs`], on a fake clock.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::time::{Duration, Instant};
use crate::{
    Config, DosDateTime, FsError, Job, JobError, MemoryFs, NameKind, Output, Server, ServerConfig,
    Session, SessionEvent, client, codec, error::Failure,
};

/// Every byte value, 5000 bytes: DLE doubling and three blocks.
fn bytes() -> Vec<u8> {
    (0..5000u32).map(|i| (i % 256) as u8).collect()
}

struct Rig {
    session: Session,
    server: Server<MemoryFs>,
    now: Instant,
    /// Frames from the client, counted.
    sent: Vec<Vec<u8>>,
    progress: Vec<(u64, Option<u64>)>,
}

impl Rig {
    fn new() -> Self {
        let mut fs = MemoryFs::new(b"ACD");
        fs.set_read_only(b'D');
        fs.insert_file(b"C:\\HELLO.TXT", b"Hello\r\n".to_vec());
        fs.insert_file(b"C:\\BYTES.BIN", bytes());
        fs.insert_file(b"C:\\SUB\\NOTE.TXT", b"note".to_vec());
        let mut rig = Rig {
            session: Session::new(Config::default()),
            server: Server::new(fs, ServerConfig::default()),
            now: Instant::now(),
            sent: Vec::new(),
            progress: Vec::new(),
        };
        assert_eq!(rig.run(Job::Connect), Ok(Output::Connected { revision: 2 }));
        rig
    }

    /// Run a job to its end; the server answers at once.
    fn run(&mut self, job: Job) -> Result<Output, JobError> {
        self.session.start(self.now, job).unwrap();
        for _ in 0..10_000 {
            while let Some(frame) = self.session.poll_output(self.now) {
                self.sent.push(frame.clone());
                self.server.handle_input(&frame);
            }
            while let Some(reply) = self.server.poll_output() {
                self.session.handle_input(self.now, &reply);
            }
            while let Some(event) = self.session.poll_event() {
                match event {
                    SessionEvent::Finished(result) => return result,
                    SessionEvent::Progress { done, total } => self.progress.push((done, total)),
                }
            }
            // Only a frame handed out has a deadline: a queued one waits.
            if let Some(t) = self.session.next_timeout() {
                self.now = t;
                self.session.handle_timeout(t);
            }
        }
        panic!("job did not finish");
    }

    /// Run a job and cancel it once it has reported `after` progress
    /// events.
    fn run_cancelled(&mut self, job: Job, after: usize) -> Result<Output, JobError> {
        self.session.start(self.now, job).unwrap();
        let mut seen = 0;
        for _ in 0..10_000 {
            while let Some(frame) = self.session.poll_output(self.now) {
                self.sent.push(frame.clone());
                self.server.handle_input(&frame);
            }
            while let Some(reply) = self.server.poll_output() {
                self.session.handle_input(self.now, &reply);
            }
            while let Some(event) = self.session.poll_event() {
                match event {
                    SessionEvent::Finished(result) => return result,
                    SessionEvent::Progress { .. } => {
                        seen += 1;
                        if seen == after {
                            self.session.cancel();
                        }
                    }
                }
            }
            if let Some(t) = self.session.next_timeout() {
                self.now = t;
                self.session.handle_timeout(t);
            }
        }
        panic!("job did not finish");
    }
}

fn names(output: Output) -> Vec<String> {
    let Output::Listing(entries) = output else {
        panic!("{output:?}")
    };
    entries.iter().map(|e| e.name_lossy()).collect()
}

#[test]
fn list_and_drives() {
    let mut r = Rig::new();
    assert_eq!(
        names(r.run(Job::List(b"C:\\*.*".to_vec())).unwrap()),
        ["BYTES.BIN", "HELLO.TXT", "SUB"]
    );
    // `.` and `..` of a subdirectory are left out.
    assert_eq!(
        names(r.run(Job::List(b"C:\\SUB\\*.*".to_vec())).unwrap()),
        ["NOTE.TXT"]
    );
    assert_eq!(
        names(r.run(Job::List(b"C:\\*.TXT".to_vec())).unwrap()),
        ["HELLO.TXT"]
    );
    assert_eq!(
        r.run(Job::List(b"C:\\NONE\\*.*".to_vec())),
        Err(JobError::Failed(Failure::Fs(FsError::PATH_NOT_FOUND)))
    );
    let Output::Drives(drives) = r.run(Job::Drives { volumes: true }).unwrap() else {
        panic!()
    };
    let letters: Vec<u8> = drives.iter().map(|d| d.letter).collect();
    assert_eq!(letters, b"ACD");
    let (label, free) = drives[1].volume.clone().unwrap().unwrap();
    assert_eq!(label, b"           ");
    assert!(free < 1 << 20);
    let Output::Drives(drives) = r.run(Job::Drives { volumes: false }).unwrap() else {
        panic!()
    };
    assert!(drives.iter().all(|d| d.volume.is_none()));
}

#[test]
fn get_reads_in_blocks_until_an_empty_one() {
    let mut r = Rig::new();
    let Output::File { data, modified } = r.run(Job::Get(b"C:\\BYTES.BIN".to_vec())).unwrap()
    else {
        panic!()
    };
    assert_eq!(data, bytes());
    assert_eq!(modified, Some(r.server.fs().now));
    // The size came from the listing lookup.
    assert_eq!(
        r.progress,
        [(2048, Some(5000)), (4096, Some(5000)), (5000, Some(5000))]
    );
    let codes: Vec<u8> = r.sent.iter().map(|f| f[6]).collect();
    assert!(
        codes.ends_with(&[0x0B, 0x12, 0x09, 0x16, 0x0C, 0x0C, 0x0C, 0x0C, 0x17, 0x01]),
        "{codes:02x?}"
    );
    assert_eq!(
        r.run(Job::Get(b"C:\\NONE.TXT".to_vec())),
        Err(JobError::Failed(Failure::Fs(FsError::NOT_FOUND)))
    );
}

#[test]
fn put_create_overwrite_and_time() {
    let mut r = Rig::new();
    let t = DosDateTime::new(2004, 12, 14, 13, 49, 36).unwrap();
    let put = |path: &[u8], data: &[u8], overwrite| Job::Put {
        path: path.to_vec(),
        data: data.to_vec(),
        modified: Some(t),
        overwrite,
    };
    assert_eq!(
        r.run(put(b"C:\\NEW.BIN", &bytes(), false)),
        Ok(Output::Done)
    );
    assert_eq!(r.server.fs().file(b"C:\\NEW.BIN"), Some(&bytes()[..]));
    assert_eq!(r.server.fs().modified(b"C:\\NEW.BIN"), Some(t));
    assert_eq!(
        r.run(put(b"C:\\HELLO.TXT", b"x", false)),
        Err(JobError::Exists)
    );
    assert_eq!(
        r.server.fs().file(b"C:\\HELLO.TXT"),
        Some(&b"Hello\r\n"[..])
    );
    assert_eq!(r.run(put(b"C:\\HELLO.TXT", b"x", true)), Ok(Output::Done));
    assert_eq!(r.server.fs().file(b"C:\\HELLO.TXT"), Some(&b"x"[..]));
    // An empty file.
    assert_eq!(r.run(put(b"C:\\EMPTY", b"", false)), Ok(Output::Done));
    assert_eq!(r.server.fs().file(b"C:\\EMPTY"), Some(&b""[..]));
    // The ROM drive.
    assert_eq!(
        r.run(put(b"D:\\X", b"x", false)),
        Err(JobError::Failed(Failure::Fs(FsError::WRITE_PROTECTED)))
    );
}

#[test]
fn a_failed_write_deletes_the_partial_file() {
    let mut r = Rig::new();
    r.server.fs_mut().capacity = 6000;
    let job = Job::Put {
        path: b"C:\\BIG.BIN".to_vec(),
        data: vec![0x10; 4000],
        modified: None,
        overwrite: false,
    };
    assert_eq!(
        r.run(job),
        Err(JobError::Failed(Failure::Fs(FsError::DISK_FULL)))
    );
    assert_eq!(r.server.fs().file(b"C:\\BIG.BIN"), None);
    assert_eq!(r.sent.last().unwrap()[6], 0x04);
}

#[test]
fn rename_delete_and_directories() {
    let mut r = Rig::new();
    let p = |s: &[u8]| s.to_vec();
    assert_eq!(
        r.run(Job::Rename {
            from: p(b"C:\\HELLO.TXT"),
            to: p(b"C:\\BYTES.BIN")
        }),
        Err(JobError::Exists)
    );
    assert_eq!(
        r.run(Job::Rename {
            from: p(b"C:\\HELLO.TXT"),
            to: p(b"C:\\SUB\\HI.TXT")
        }),
        Ok(Output::Done)
    );
    assert_eq!(
        r.run(Job::Kind(p(b"C:\\SUB\\HI.TXT"))),
        Ok(Output::Kind(NameKind::File))
    );
    assert_eq!(
        r.run(Job::RemoveDir(p(b"C:\\SUB"))),
        Err(JobError::Failed(Failure::Fs(FsError::ACCESS_DENIED)))
    );
    assert_eq!(r.run(Job::Delete(p(b"C:\\SUB\\HI.TXT"))), Ok(Output::Done));
    assert_eq!(
        r.run(Job::Delete(p(b"C:\\SUB\\NOTE.TXT"))),
        Ok(Output::Done)
    );
    assert_eq!(
        r.run(Job::Delete(p(b"C:\\SUB\\NOTE.TXT"))),
        Err(JobError::Failed(Failure::Fs(FsError::NOT_FOUND)))
    );
    assert_eq!(r.run(Job::RemoveDir(p(b"C:\\SUB"))), Ok(Output::Done));
    assert_eq!(r.run(Job::MakeDir(p(b"C:\\NEWDIR"))), Ok(Output::Done));
    assert_eq!(
        r.run(Job::MakeDir(p(b"C:\\NEWDIR"))),
        Err(JobError::Failed(Failure::Fs(FsError::CANNOT_MKDIR)))
    );
    assert_eq!(
        r.run(Job::Kind(p(b"C:\\NEWDIR"))),
        Ok(Output::Kind(NameKind::Directory))
    );
}

/// The acceptance's "a lost reply is resent with the same counter": the
/// write runs once, its reply is lost, the client resends the identical
/// frame after 5.9 s and the server answers with the reply it kept.
#[test]
fn a_lost_reply_is_resent_and_not_run_twice() {
    let mut r = Rig::new();
    r.session
        .start(
            r.now,
            Job::Put {
                path: b"C:\\LOST.BIN".to_vec(),
                data: vec![1, 2, 3],
                modified: None,
                overwrite: true,
            },
        )
        .unwrap();
    // The create (no probe with overwrite).
    let create = r.session.poll_output(r.now).unwrap();
    r.server.handle_input(&create);
    let reply = r.server.poll_output().unwrap();
    r.session.handle_input(r.now, &reply);
    // The write; its reply is lost.
    let write = r.session.poll_output(r.now).unwrap();
    assert_eq!(write[6], 0x15);
    r.server.drop_replies(1);
    r.server.handle_input(&write);
    let executed = r.server.executed();
    assert!(r.server.poll_output().is_none());
    let deadline = r.session.next_timeout().unwrap();
    assert!(deadline - r.now >= Duration::from_millis(5900));
    r.now = deadline;
    r.session.handle_timeout(r.now);
    let again = r.session.poll_output(r.now).unwrap();
    assert_eq!(again, write);
    r.server.handle_input(&again);
    assert_eq!(r.server.executed(), executed);
    let reply = r.server.poll_output().unwrap();
    r.session.handle_input(r.now, &reply);
    // Finish it.
    loop {
        while let Some(frame) = r.session.poll_output(r.now) {
            r.server.handle_input(&frame);
        }
        while let Some(reply) = r.server.poll_output() {
            r.session.handle_input(r.now, &reply);
        }
        if let Some(SessionEvent::Finished(result)) = r.session.poll_event() {
            assert_eq!(result, Ok(Output::Done));
            break;
        }
    }
    assert_eq!(r.server.fs().file(b"C:\\LOST.BIN"), Some(&[1, 2, 3][..]));
}

#[test]
fn a_server_that_left_the_session_ends_it() {
    let mut r = Rig::new();
    r.server.break_session();
    assert_eq!(
        r.run(Job::Kind(b"C:\\".to_vec())),
        Err(JobError::Link(client::Error::NotConnected))
    );
    assert!(!r.session.is_open());
    // A new connect works.
    assert_eq!(r.run(Job::Connect), Ok(Output::Connected { revision: 2 }));
}

#[test]
fn a_silent_server_fails_after_three_sends() {
    let mut r = Rig::new();
    r.server.drop_replies(u32::MAX);
    let before = r.sent.len();
    assert!(matches!(
        r.run(Job::Kind(b"C:\\".to_vec())),
        Err(JobError::Link(client::Error::NoReply { sends: 3, .. }))
    ));
    // Three sends of the request, then one DISCONNECT.
    let after: Vec<u8> = r.sent[before..].iter().map(|f| f[6]).collect();
    assert_eq!(after, [0x08, 0x08, 0x08, 0x41]);
}

#[test]
fn revision_one_server() {
    let mut fs = MemoryFs::new(b"C");
    fs.insert_file(b"C:\\A", vec![]);
    let mut r = Rig::new();
    r.server = Server::new(
        fs,
        ServerConfig {
            revision: 1,
            ..ServerConfig::default()
        },
    );
    assert_eq!(r.run(Job::Connect), Ok(Output::Connected { revision: 1 }));
    // The drive list is not asked for in revision 1.
    let before = r.sent.len();
    let Output::Drives(d) = r.run(Job::Drives { volumes: false }).unwrap() else {
        panic!()
    };
    assert_eq!(d[0].letter, b'C');
    assert_eq!(r.sent.len(), before);
    // Frames go out in revision 1.
    r.run(Job::Kind(b"C:\\A".to_vec())).unwrap();
    assert_eq!(r.sent.last().unwrap()[9], 1);
    let _ = codec::REVISION;
}

#[test]
fn a_listing_of_only_dots_gives_up() {
    // A server that answers every 0x09 with `.`: the entries dropped from
    // the listing still count towards its limit.
    use crate::{Code, DirEntry, Response, attr};
    let mut session = Session::new(Config::default());
    let now = Instant::now();
    let dot = Response::Entry(Some(DirEntry {
        attributes: attr::DIRECTORY,
        modified: DosDateTime::default(),
        size: 0,
        name: b".".to_vec(),
    }))
    .encode();
    let mut deframer = codec::Deframer::new();
    let mut finished = Vec::new();
    session.start(now, Job::Connect).unwrap();
    let mut started = false;
    for _ in 0..100_000 {
        while let Some(wire) = session.poll_output(now) {
            deframer.push(&wire);
            while let Some(frame) = deframer.next_frame() {
                let frame = frame.unwrap();
                let data = match Code::from_byte(frame.command) {
                    Some(Code::Connect) => vec![0, 0],
                    Some(Code::FindNext) => dot.clone(),
                    Some(Code::Disconnect) => continue,
                    _ => Response::Done.encode(),
                };
                let reply = codec::Frame::reply(frame.command, frame.counter, 2, data).encode();
                session.handle_input(now, &reply);
            }
        }
        while let Some(event) = session.poll_event() {
            if let SessionEvent::Finished(result) = event {
                finished.push(result);
            }
        }
        match finished.len() {
            1 if !started => {
                assert_eq!(finished[0], Ok(Output::Connected { revision: 2 }));
                session.start(now, Job::List(b"C:\\*.*".to_vec())).unwrap();
                started = true;
            }
            2 => break,
            _ => {}
        }
    }
    assert_eq!(
        finished.get(1),
        Some(&Err(JobError::Failed(Failure::Fs(FsError::INTERNAL))))
    );
}

#[test]
fn a_cancelled_get_closes_its_file_and_the_session_goes_on() {
    let mut r = Rig::new();
    assert_eq!(
        r.run_cancelled(Job::Get(b"C:\\BYTES.BIN".to_vec()), 1),
        Err(JobError::Cancelled)
    );
    // The last request was the close of handle 0.
    assert_eq!(r.sent.last().unwrap()[6], 0x01);
    assert!(r.session.is_open());
    assert!(!r.session.is_cancelling());
    let Output::File { data, .. } = r.run(Job::Get(b"C:\\BYTES.BIN".to_vec())).unwrap() else {
        panic!()
    };
    assert_eq!(data, bytes());
}

#[test]
fn a_cancelled_put_deletes_the_part_written() {
    let mut r = Rig::new();
    let job = Job::Put {
        path: b"C:\\NEW.BIN".to_vec(),
        data: bytes(),
        modified: None,
        overwrite: true,
    };
    assert_eq!(r.run_cancelled(job, 1), Err(JobError::Cancelled));
    assert_eq!(r.server.fs().file(b"C:\\NEW.BIN"), None);
    assert_eq!(r.sent.last().unwrap()[6], 0x04);
    // A listing cancelled before it starts ends at its first reply.
    r.session.start(r.now, Job::List(b"C:\\*.*".to_vec())).unwrap();
    r.session.cancel();
    let frame = r.session.poll_output(r.now).unwrap();
    r.server.handle_input(&frame);
    let reply = r.server.poll_output().unwrap();
    r.session.handle_input(r.now, &reply);
    assert_eq!(
        r.session.poll_event(),
        Some(SessionEvent::Finished(Err(JobError::Cancelled)))
    );
    assert!(r.session.is_open());
}
