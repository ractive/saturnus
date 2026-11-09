//! [`Palmtop`] against `filer-proto`'s [`Server`] over [`MemoryFs`], on a
//! fake clock; and the recorded session of `traces/100lx-session.trace`
//! (the page's driver test replays it, `web/test/tx-driver.test.mjs`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use filer_proto::time::{Duration, Instant};
use filer_proto::{FsError, MemoryFs, Server, ServerConfig};

use super::*;
use crate::trace::{Step, Trace};

/// The bytes of the recorded put: DLE, SYN and ETX in it, for the
/// doubling.
pub(crate) const PUT_DATA: &[u8] = b"\x10\x16\x03 hello, palmtop\r\n";

fn fs() -> MemoryFs {
    let mut fs = MemoryFs::new(b"ACD");
    fs.set_read_only(b'D');
    fs.insert_file(b"C:\\HELLO.TXT", b"Hello\r\n".to_vec());
    fs.insert_file(
        b"C:\\BIG.BIN",
        (0..5000u32).map(|i| (i % 251) as u8).collect(),
    );
    fs.insert_file(b"C:\\SUB\\NOTE.TXT", b"note".to_vec());
    fs.insert_file(b"C:\\SUB\\DEEP\\A.TXT", b"a".to_vec());
    fs.insert_file(b"D:\\ROM.TXT", b"rom".to_vec());
    fs
}

struct Rig {
    palmtop: Palmtop,
    server: Server<MemoryFs>,
    start: Instant,
    now: Instant,
    /// The server answers nothing.
    silent: bool,
    trace: Trace,
    progress: Vec<(u64, Option<u64>)>,
    /// Frames the palmtop wrote, by command byte.
    commands: Vec<u8>,
}

impl Rig {
    fn new() -> Self {
        Self::with(Server::new(fs(), ServerConfig::default()))
    }

    fn with(server: Server<MemoryFs>) -> Self {
        let now = Instant::now();
        Rig {
            palmtop: Palmtop::new(),
            server,
            start: now,
            now,
            silent: false,
            trace: Trace::new(),
            progress: Vec::new(),
            commands: Vec::new(),
        }
    }

    fn connected() -> Self {
        let mut r = Self::new();
        assert_eq!(
            r.run(PalmtopRequest::Connect { baud: 19200 }),
            Ok(PalmtopOutcome::Connected(PalmtopConnection {
                baud: 19200,
                revision: 2
            }))
        );
        r
    }

    fn at(&self) -> Duration {
        self.now.saturating_duration_since(self.start)
    }

    /// Move the actions to the server and its replies back, until
    /// neither has more.
    fn exchange(&mut self) {
        loop {
            let before = self.trace.steps.len();
            self.exchange_once();
            if self.trace.steps.len() == before {
                break;
            }
        }
    }

    fn exchange_once(&mut self) {
        while let Some(action) = self.palmtop.poll_action() {
            match action {
                Action::Open(s) => self.trace.push(self.at(), Step::Open(s)),
                Action::Write(frame) => {
                    self.trace.push(self.at(), Step::Out(frame.clone()));
                    self.commands.push(frame[6]);
                    if !self.silent {
                        self.server.handle_input(&frame);
                    }
                }
            }
        }
        while let Some(reply) = self.server.poll_output() {
            self.trace.push(self.at(), Step::In(reply.clone()));
            self.palmtop.handle_input(self.now, &reply);
        }
        while let Some(PalmtopEvent::Progress { done, total }) = self.palmtop.poll_event() {
            self.progress.push((done, total));
        }
    }

    /// Run `request` to its end; `during` is called after each exchange.
    fn run_with(
        &mut self,
        request: PalmtopRequest,
        mut during: impl FnMut(&mut Rig),
    ) -> Result<PalmtopOutcome> {
        self.trace.note(&format!("request {}", request.describe()));
        self.palmtop.start(self.now, request).unwrap();
        for _ in 0..100_000 {
            loop {
                let before = self.trace.steps.len();
                self.exchange_once();
                during(self);
                if self.trace.steps.len() == before {
                    break;
                }
            }
            if let Some(result) = self.palmtop.poll_result() {
                return result;
            }
            match self.palmtop.next_timeout() {
                Some(t) => {
                    self.now = t.max(self.now);
                    self.palmtop.handle_timeout(self.now);
                }
                None => panic!("nothing due and no result"),
            }
        }
        panic!("the request did not end");
    }

    fn run(&mut self, request: PalmtopRequest) -> Result<PalmtopOutcome> {
        self.run_with(request, |_| {})
    }
}

fn dir(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

fn names(outcome: Result<PalmtopOutcome>) -> Vec<String> {
    match outcome {
        Ok(PalmtopOutcome::Listing { entries, .. }) => {
            entries.into_iter().map(|e| e.name).collect()
        }
        other => panic!("{other:?}"),
    }
}

fn put(d: &[&str], name: &str, data: &[u8], replace: bool) -> PalmtopRequest {
    PalmtopRequest::Put {
        dir: dir(d),
        name: name.into(),
        data: data.to_vec(),
        replace,
    }
}

#[test]
fn dos_names() {
    assert_eq!(dos_name("notes.txt"), Ok("NOTES.TXT".into()));
    assert_eq!(dos_name("A"), Ok("A".into()));
    assert_eq!(dos_name("_DAT"), Ok("_DAT".into()));
    assert_eq!(dos_name("12345678.ABC"), Ok("12345678.ABC".into()));
    assert_eq!(dos_name("{a}~!#$%"), Ok("{A}~!#$%".into()));
    assert_eq!(dos_name("123456789"), Err(NameProblem::TooLong));
    assert_eq!(dos_name("PROGRAMS.TEXT"), Err(NameProblem::TooLong));
    assert_eq!(dos_name("A.B.C"), Err(NameProblem::Dots));
    assert_eq!(dos_name(".HP"), Err(NameProblem::Dots));
    assert_eq!(dos_name("A."), Err(NameProblem::TooLong));
    assert_eq!(dos_name(""), Err(NameProblem::Empty));
    assert_eq!(dos_name("MY FILE"), Err(NameProblem::Char(' ')));
    assert_eq!(dos_name("A+B"), Err(NameProblem::Char('+')));
    assert_eq!(dos_name("é.TXT"), Err(NameProblem::Char('é')));
    assert_eq!(
        NameProblem::TooLong.to_string(),
        "the palmtop takes DOS names: up to 8 letters or digits, a dot and up to 3 more (like NOTES.TXT)"
    );
    assert_eq!(
        dos_path(&dir(&["c:", "SUB"]), Some("X.TXT")),
        Some(b"C:\\SUB\\X.TXT".to_vec())
    );
    assert_eq!(dos_path(&dir(&["C:"]), None), Some(b"C:\\".to_vec()));
    assert_eq!(dos_path(&dir(&["CC"]), None), None);
    for bad in [".", "..", "A:B", "*.*", "?"] {
        assert_eq!(dos_path(&dir(&["C:", bad]), None), None, "{bad}");
        assert_eq!(dos_path(&dir(&["C:"]), Some(bad)), None, "{bad}");
    }
    // DOS's device names, whatever the case and the extension.
    assert_eq!(dos_name("aux"), Err(NameProblem::Device("AUX".into())));
    assert_eq!(dos_name("Con.txt"), Err(NameProblem::Device("CON".into())));
    assert_eq!(dos_name("COM1.HP"), Err(NameProblem::Device("COM1".into())));
    assert_eq!(
        dos_name("clock$"),
        Err(NameProblem::Device("CLOCK$".into()))
    );
    assert_eq!(dos_name("COM5"), Ok("COM5".into()));
    assert_eq!(dos_name("CONS.TXT"), Ok("CONS.TXT".into()));
    assert_eq!(
        NameProblem::Device("AUX".into()).to_string(),
        "AUX is the name of a device on the palmtop (DOS keeps it), so no file can have it: choose another"
    );
    assert_eq!(dos_path(&dir(&["C:", "A\\B"]), None), None);
    // Latin-1 names read from the palmtop go back byte for byte.
    assert_eq!(
        dos_path(&dir(&["C:"]), Some("\u{e9}.TXT")),
        Some(b"C:\\\xe9.TXT".to_vec())
    );
}

#[test]
fn drives_listings_get_and_progress() {
    let mut r = Rig::connected();
    assert_eq!(
        names(r.run(PalmtopRequest::List { dir: vec![] })),
        ["A:", "C:", "D:"]
    );
    let Ok(PalmtopOutcome::Listing { entries, .. }) =
        r.run(PalmtopRequest::List { dir: dir(&["C:"]) })
    else {
        panic!()
    };
    let shown: Vec<_> = entries
        .iter()
        .map(|e| (e.name.as_str(), e.directory, e.size))
        .collect();
    assert_eq!(
        shown,
        [
            ("BIG.BIN", false, 5000),
            ("HELLO.TXT", false, 7),
            ("SUB", true, 0)
        ]
    );
    assert_eq!(entries[1].modified, DosDateTime::new(2026, 1, 1, 12, 0, 0));
    assert_eq!(
        names(r.run(PalmtopRequest::List {
            dir: dir(&["C:", "SUB"])
        })),
        ["DEEP", "NOTE.TXT"]
    );
    let got = r.run(PalmtopRequest::Get {
        dir: dir(&["C:"]),
        name: "BIG.BIN".into(),
    });
    let Ok(PalmtopOutcome::File {
        name,
        data,
        modified,
    }) = got
    else {
        panic!("{got:?}")
    };
    assert_eq!((name.as_str(), data.len()), ("BIG.BIN", 5000));
    assert_eq!(modified, DosDateTime::new(2026, 1, 1, 12, 0, 0));
    assert_eq!(
        r.progress,
        [(2048, Some(5000)), (4096, Some(5000)), (5000, Some(5000))]
    );
    assert_eq!(
        r.run(PalmtopRequest::Get {
            dir: dir(&["C:"]),
            name: "NONE.TXT".into()
        }),
        Err(PalmtopError::NotFound("NONE.TXT".into()))
    );
    let e = r
        .run(PalmtopRequest::List {
            dir: dir(&["C:", "NONE"]),
        })
        .unwrap_err();
    assert_eq!(
        e.to_string(),
        "C:\\NONE: that directory or drive is not there"
    );
}

#[test]
fn put_replace_rename_mkdir_and_errors() {
    let mut r = Rig::connected();
    assert_eq!(
        r.run(put(&["C:"], "new.txt", b"one", false)),
        Ok(PalmtopOutcome::Stored("NEW.TXT".into()))
    );
    assert_eq!(r.server.fs().file(b"C:\\NEW.TXT"), Some(&b"one"[..]));
    assert_eq!(
        r.run(put(&["C:"], "NEW.TXT", b"two", false)),
        Err(PalmtopError::Exists("NEW.TXT".into()))
    );
    assert_eq!(
        r.run(put(&["C:"], "NEW.TXT", b"two", true)),
        Ok(PalmtopOutcome::Stored("NEW.TXT".into()))
    );
    assert_eq!(r.server.fs().file(b"C:\\NEW.TXT"), Some(&b"two"[..]));
    // A long name is refused before anything is sent.
    let sent = r.commands.len();
    let e = r
        .run(put(&["C:"], "LONGNAME1.TXT", b"x", false))
        .unwrap_err();
    assert_eq!(r.commands.len(), sent);
    assert_eq!(
        e.to_string(),
        "LONGNAME1.TXT: the palmtop takes DOS names: up to 8 letters or digits, a dot and up to 3 more (like NOTES.TXT)"
    );
    assert_eq!(
        r.run(PalmtopRequest::Rename {
            dir: dir(&["C:"]),
            from: "NEW.TXT".into(),
            to: "hello.txt".into()
        }),
        Err(PalmtopError::Exists("HELLO.TXT".into()))
    );
    assert_eq!(
        r.run(PalmtopRequest::Rename {
            dir: dir(&["C:"]),
            from: "NEW.TXT".into(),
            to: "notes.txt".into()
        }),
        Ok(PalmtopOutcome::Renamed {
            from: "NEW.TXT".into(),
            to: "NOTES.TXT".into()
        })
    );
    assert_eq!(r.server.fs().file(b"C:\\NOTES.TXT"), Some(&b"two"[..]));
    assert_eq!(
        r.run(PalmtopRequest::MakeDir {
            dir: dir(&["C:"]),
            name: "docs".into()
        }),
        Ok(PalmtopOutcome::Created("DOCS".into()))
    );
    assert_eq!(
        r.run(PalmtopRequest::MakeDir {
            dir: dir(&["C:"]),
            name: "DOCS".into()
        }),
        Err(PalmtopError::Exists("DOCS".into()))
    );
    // The ROM disk.
    let e = r.run(put(&["D:"], "X.TXT", b"x", false)).unwrap_err();
    assert_eq!(
        e,
        PalmtopError::Fs {
            name: "X.TXT".into(),
            error: FsError::WRITE_PROTECTED
        }
    );
    assert_eq!(
        e.to_string(),
        "X.TXT: the drive is write-protected (D: is the palmtop's ROM disk)"
    );
    // The session goes on after every refusal.
    assert_eq!(
        names(r.run(PalmtopRequest::List { dir: dir(&["D:"]) })),
        ["ROM.TXT"]
    );
}

#[test]
fn remove_a_file_and_a_tree() {
    let mut r = Rig::connected();
    assert_eq!(
        r.run(PalmtopRequest::Remove {
            dir: dir(&["C:"]),
            name: "HELLO.TXT".into(),
            directory: false
        }),
        Ok(PalmtopOutcome::Removed {
            name: "HELLO.TXT".into(),
            count: 0
        })
    );
    assert_eq!(
        r.run(PalmtopRequest::Remove {
            dir: dir(&["C:"]),
            name: "SUB".into(),
            directory: true
        }),
        Ok(PalmtopOutcome::Removed {
            name: "SUB".into(),
            count: 3
        })
    );
    assert_eq!(
        names(r.run(PalmtopRequest::List { dir: dir(&["C:"]) })),
        ["BIG.BIN"]
    );
    assert_eq!(
        r.run(PalmtopRequest::Remove {
            dir: dir(&["C:"]),
            name: "SUB".into(),
            directory: true
        }),
        Err(PalmtopError::Fs {
            name: "SUB".into(),
            error: FsError::PATH_NOT_FOUND
        })
    );
}

#[test]
fn stop_closes_the_file_and_keeps_the_session() {
    let mut r = Rig::connected();
    let mut stopped = false;
    let got = r.run_with(
        PalmtopRequest::Get {
            dir: dir(&["C:"]),
            name: "BIG.BIN".into(),
        },
        |r| {
            if !r.progress.is_empty() && !stopped {
                stopped = true;
                r.palmtop.stop(r.now);
            }
        },
    );
    assert_eq!(got, Err(PalmtopError::Stopped));
    // The close of the file came last.
    assert_eq!(r.commands.last(), Some(&0x01));
    let big = vec![7u8; 6000];
    let mut stopped = false;
    r.progress.clear();
    let put = r.run_with(put(&["C:"], "PART.BIN", &big, false), |r| {
        if !r.progress.is_empty() && !stopped {
            stopped = true;
            r.palmtop.stop(r.now);
        }
    });
    assert_eq!(put, Err(PalmtopError::Stopped));
    assert_eq!(
        r.server.fs().file(b"C:\\PART.BIN"),
        None,
        "the part written is deleted"
    );
    let mut stopped = false;
    let removed = r.run_with(
        PalmtopRequest::Remove {
            dir: dir(&["C:"]),
            name: "SUB".into(),
            directory: true,
        },
        |r| {
            if !stopped {
                stopped = true;
                r.palmtop.stop(r.now);
            }
        },
    );
    assert_eq!(removed, Err(PalmtopError::Stopped));
    assert!(
        r.server.fs().file(b"C:\\SUB\\NOTE.TXT").is_some(),
        "stopped before any delete"
    );
    assert!(r.palmtop.connection().is_some());
    assert_eq!(
        names(r.run(PalmtopRequest::List { dir: dir(&["C:"]) })),
        ["BIG.BIN", "HELLO.TXT", "SUB"]
    );
}

#[test]
fn abort_reconnects_before_the_next_request() {
    let mut r = Rig::connected();
    r.palmtop
        .start(
            r.now,
            PalmtopRequest::Get {
                dir: dir(&["C:"]),
                name: "BIG.BIN".into(),
            },
        )
        .unwrap();
    r.exchange_once();
    r.palmtop.abort(r.now);
    assert!(!r.palmtop.is_busy());
    r.exchange();
    assert_eq!(r.commands.last(), Some(&0x41), "a DISCONNECT went out");
    assert_eq!(
        names(r.run(PalmtopRequest::List { dir: dir(&["C:"]) })),
        ["BIG.BIN", "HELLO.TXT", "SUB"]
    );
    let gone = r.commands.iter().rposition(|&c| c == 0x41).unwrap();
    assert_eq!(
        r.commands[gone + 1..gone + 3],
        [0x40, 0x12],
        "CONNECT first, then the listing"
    );
}

#[test]
fn a_silent_palmtop_is_not_answering_until_it_answers_a_connect() {
    let mut r = Rig::connected();
    r.silent = true;
    let started = r.now;
    assert_eq!(
        r.run(PalmtopRequest::List { dir: dir(&["C:"]) }),
        Err(PalmtopError::NotAnswering)
    );
    // Three sends, 5.9 s each.
    let waited = r.now.saturating_duration_since(started);
    assert!(
        waited > Duration::from_secs(17) && waited < Duration::from_secs(19),
        "{waited:?}"
    );
    assert!(r.palmtop.not_answering());
    assert_eq!(r.palmtop.connection(), None);
    assert_eq!(
        r.run(PalmtopRequest::List { dir: dir(&["C:"]) }),
        Err(PalmtopError::NotAnswering)
    );
    r.silent = false;
    // The server still thinks the old session is open: CONNECT resets it.
    assert!(matches!(
        r.run(PalmtopRequest::Connect { baud: 19200 }),
        Ok(PalmtopOutcome::Connected(_))
    ));
    assert!(!r.palmtop.not_answering());
    // One that left server mode answers a request with a CONNECT refusal.
    r.server.break_session();
    assert_eq!(
        r.run(PalmtopRequest::List { dir: dir(&["C:"]) }),
        Err(PalmtopError::NotAnswering)
    );
}

#[test]
fn nobody_there_is_no_answer_at_that_speed() {
    let mut r = Rig::new();
    r.silent = true;
    let e = r.run(PalmtopRequest::Connect { baud: 38400 }).unwrap_err();
    assert_eq!(e, PalmtopError::NoAnswer { baud: 38400 });
    assert!(e.to_string().contains("server mode on at 38400 baud"));
    assert!(!r.palmtop.not_answering());
    assert_eq!(
        r.run(PalmtopRequest::Connect { baud: 12345 }),
        Err(PalmtopError::BadSpeed(12345))
    );
    assert_eq!(
        r.run(PalmtopRequest::List { dir: vec![] }),
        Err(PalmtopError::NotConnected)
    );
}

#[test]
fn a_revision_one_filer_has_drive_c_only() {
    let mut config = ServerConfig::default();
    config.revision = 1;
    let mut r = Rig::with(Server::new(fs(), config));
    assert_eq!(
        r.run(PalmtopRequest::Connect { baud: 19200 }),
        Ok(PalmtopOutcome::Connected(PalmtopConnection {
            baud: 19200,
            revision: 1
        }))
    );
    assert_eq!(names(r.run(PalmtopRequest::List { dir: vec![] })), ["C:"]);
}

/// The session of `traces/100lx-session.trace`, which the page's driver
/// test (`web/test/tx-driver.test.mjs`) sends in this order.
pub(crate) fn session_requests() -> Vec<PalmtopRequest> {
    vec![
        PalmtopRequest::Connect { baud: 19200 },
        PalmtopRequest::List { dir: vec![] },
        PalmtopRequest::List { dir: dir(&["C:"]) },
        PalmtopRequest::Get {
            dir: dir(&["C:"]),
            name: "HELLO.TXT".into(),
        },
        put(&["C:"], "NEW.TXT", PUT_DATA, false),
        put(&["C:"], "NEW.TXT", PUT_DATA, false),
        put(&["C:"], "NEW.TXT", PUT_DATA, true),
        PalmtopRequest::Rename {
            dir: dir(&["C:"]),
            from: "NEW.TXT".into(),
            to: "NOTES.TXT".into(),
        },
        PalmtopRequest::MakeDir {
            dir: dir(&["C:"]),
            name: "DOCS".into(),
        },
        PalmtopRequest::Remove {
            dir: dir(&["C:"]),
            name: "SUB".into(),
            directory: true,
        },
        PalmtopRequest::List { dir: dir(&["C:"]) },
        PalmtopRequest::Disconnect,
    ]
}

#[test]
fn the_recorded_session_is_current() {
    let mut r = Rig::new();
    for request in session_requests() {
        let _ = r.run(request);
    }
    let mut text = String::from(
        "# saturnus tx trace: the palmtop session of crates/saturnus-tx/src/palmtop_tests.rs\n\
         # (session_requests) against filer-proto's server over its MemoryFs.\n\
         # Serial bytes only; times in ms since the start. SATURNUS_BLESS=1 rewrites it.\n",
    );
    text.push_str(&r.trace.to_string());
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/traces/100lx-session.trace");
    if std::env::var_os("SATURNUS_BLESS").is_some() {
        std::fs::write(file, &text).unwrap();
    }
    let kept = std::fs::read_to_string(file).unwrap_or_default();
    assert!(
        kept == text,
        "{file} is out of date: run the test with SATURNUS_BLESS=1"
    );
}

#[test]
fn a_tree_with_a_read_only_file_is_refused_before_anything_is_deleted() {
    use filer_proto::FileSystem;
    let mut r = Rig::connected();
    r.server
        .fs_mut()
        .set_attributes(b"C:\\SUB\\DEEP\\A.TXT", attr::READ_ONLY)
        .unwrap();
    let deletes = r.commands.iter().filter(|&&c| c == 0x04).count();
    let e = r
        .run(PalmtopRequest::Remove {
            dir: dir(&["C:"]),
            name: "SUB".into(),
            directory: true,
        })
        .unwrap_err();
    assert_eq!(
        e,
        PalmtopError::ReadOnlyInside {
            dir: "SUB".into(),
            file: "DEEP\\A.TXT".into()
        }
    );
    assert_eq!(
        e.to_string(),
        "SUB: DEEP\\A.TXT in it is read-only, so nothing was deleted; clear its read-only mark on the palmtop first"
    );
    assert_eq!(
        r.commands.iter().filter(|&&c| c == 0x04).count(),
        deletes,
        "nothing deleted"
    );
    assert!(r.server.fs().file(b"C:\\SUB\\NOTE.TXT").is_some());
}

#[test]
fn a_directory_with_a_hidden_file_says_so_at_its_removal() {
    use filer_proto::FileSystem;
    let mut r = Rig::connected();
    // Hidden files are not listed, so the walk cannot delete them.
    r.server
        .fs_mut()
        .set_attributes(b"C:\\SUB\\NOTE.TXT", attr::HIDDEN)
        .unwrap();
    let e = r
        .run(PalmtopRequest::Remove {
            dir: dir(&["C:"]),
            name: "SUB".into(),
            directory: true,
        })
        .unwrap_err();
    assert_eq!(e, PalmtopError::HiddenInside("SUB".into()));
    assert_eq!(
        e.to_string(),
        "SUB: the directory still holds files the palmtop does not list (hidden or system)"
    );
}
