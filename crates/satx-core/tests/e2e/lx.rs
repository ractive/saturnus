//! `satx_core::filer::Palmtop` end to end, over TCP (a module of the
//! e2e binary, `cargo test -p satx-core --test e2e lx::`):
//!
//! - always against `filer-proto`'s test server (`Server` over
//!   `MemoryFs`), in a thread of this process;
//! - also against the address in `FILER_E2E_ADDR`, BSD Filer v0.3 in
//!   Docker (`just lx-oracle-up`, `tcp://localhost:4853`): the scenarios
//!   it can run. It has no 0x05, 0x07, 0x0E, 0x19 or 0x1A (no answer at
//!   all), gives no error codes, renames deleted files to `.BAK`, keeps no
//!   time set with 0x18, and exits on a listing of or an open in a missing
//!   directory or file, so the error cases run in process only.
//!
//! Both serve the same files: `HELLO.TXT` (30 bytes, 2004-12-14
//! 13:49:36), `BYTES.BIN` (5000 bytes, every value), `EMPTY.DAT`,
//! `SUB\NOTE.TXT` (tools/filer-oracle/entrypoint.sh).

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use satx_core::Error;
use satx_core::filer::{self, DEFAULT_BAUD, Palmtop};
use satx_core::filer_proto::{
    DosDateTime, FsError, JobError, MemoryFs, NameKind, Server, ServerConfig,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
type Shared = Arc<Mutex<Server<MemoryFs>>>;

const HELLO: &[u8] = b"Hello from the filer oracle.\r\n";

fn bytes() -> Vec<u8> {
    (0..5000u32).map(|i| (i % 256) as u8).collect()
}

fn hello_time() -> DosDateTime {
    DosDateTime::new(2004, 12, 14, 13, 49, 36).unwrap()
}

/// The in-process server: one thread, one connection at a time, the
/// files kept across connections.
fn local() -> &'static (String, Shared) {
    static LOCAL: OnceLock<(String, Shared)> = OnceLock::new();
    LOCAL.get_or_init(|| {
        let mut fs = MemoryFs::new(b"ACD");
        fs.set_read_only(b'D');
        let now = fs.now;
        fs.now = hello_time();
        fs.insert_file(b"C:\\HELLO.TXT", HELLO.to_vec());
        fs.now = now;
        fs.insert_file(b"C:\\BYTES.BIN", bytes());
        fs.insert_file(b"C:\\EMPTY.DAT", Vec::new());
        fs.insert_file(b"C:\\SUB\\NOTE.TXT", b"in a subdirectory\r\n".to_vec());
        let server: Shared = Arc::new(Mutex::new(Server::new(fs, ServerConfig::default())));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("tcp://{}", listener.local_addr().unwrap());
        let shared = server.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(&stream, &shared);
            }
        });
        (addr, server)
    })
}

fn serve(mut stream: &TcpStream, server: &Shared) {
    stream
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => {
                let mut s = server.lock().unwrap();
                s.handle_input(&buf[..n]);
                while let Some(reply) = s.poll_output() {
                    if stream.write_all(&reply).is_err() {
                        return;
                    }
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return,
        }
    }
}

/// One link at a time per target.
static LOCK: Mutex<()> = Mutex::new(());

/// The targets: the in-process server, and the oracle when set.
fn targets() -> Vec<(String, bool)> {
    let mut t = vec![(local().0.clone(), false)];
    if let Some(addr) = std::env::var("FILER_E2E_ADDR")
        .ok()
        .filter(|a| !a.is_empty())
    {
        t.push((addr, true));
    }
    t
}

/// Run `body` on every target with a fresh connection.
fn each(body: impl Fn(&mut Palmtop, bool) -> TestResult) -> TestResult {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for (addr, oracle) in targets() {
        eprintln!("target {addr}");
        let mut p = Palmtop::open(&addr, DEFAULT_BAUD)?;
        body(&mut p, oracle).map_err(|e| format!("{addr}: {e}"))?;
        p.close()?;
    }
    Ok(())
}

/// A unique 8.3 base name per run (the oracle keeps its files).
fn unique(prefix: &str) -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros()
        % 100_000;
    format!("{prefix}{n:05}")
}

fn names(p: &mut Palmtop, pattern: &[u8]) -> Result<Vec<String>, Error> {
    Ok(p.list(pattern)?.iter().map(|e| e.name_lossy()).collect())
}

#[test]
fn drives_and_listing() -> TestResult {
    each(|p, _| {
        let drives = p.drives(true)?;
        let c = drives.iter().find(|d| d.letter == b'C').ok_or("no C:")?;
        let (label, _free) = c.volume.clone().ok_or("no volume")??;
        assert_eq!(label.len(), 11);
        let root = p.list(b"C:\\*.*")?;
        let hello = root
            .iter()
            .find(|e| e.name == b"HELLO.TXT")
            .ok_or("no HELLO.TXT")?;
        assert_eq!(hello.size, 30);
        assert_eq!(hello.modified, hello_time());
        assert!(!hello.is_directory());
        let bytes_bin = root.iter().find(|e| e.name == b"BYTES.BIN").unwrap();
        assert_eq!(bytes_bin.size, 5000);
        assert!(root.iter().any(|e| e.name == b"SUB" && e.is_directory()));
        assert!(!root.iter().any(|e| e.name == b"." || e.name == b".."));
        assert_eq!(names(p, b"C:\\SUB\\*.*")?, ["NOTE.TXT"]);
        assert_eq!(names(p, b"C:\\HELLO.*")?, ["HELLO.TXT"]);
        // Lower case is the same name.
        assert_eq!(names(p, b"c:\\hello.txt")?, ["HELLO.TXT"]);
        assert_eq!(p.kind(b"C:\\SUB")?, NameKind::Directory);
        assert_eq!(p.kind(b"C:\\")?, NameKind::Directory);
        Ok(())
    })
}

#[test]
fn get_byte_for_byte() -> TestResult {
    each(|p, _| {
        let (data, modified) = p.get(b"C:\\BYTES.BIN")?;
        assert_eq!(data, bytes());
        assert!(modified.is_some());
        let (data, modified) = p.get(b"C:\\HELLO.TXT")?;
        assert_eq!(data, HELLO);
        assert_eq!(modified, Some(hello_time()));
        assert_eq!(p.get(b"C:\\EMPTY.DAT")?.0, b"");
        assert_eq!(p.get(b"C:\\SUB\\NOTE.TXT")?.0, b"in a subdirectory\r\n");
        Ok(())
    })
}

#[test]
fn put_rename_delete() -> TestResult {
    each(|p, oracle| {
        let dir = unique("D");
        let file = unique("F");
        let dir_path = format!("C:\\{dir}");
        let path = format!("{dir_path}\\{file}.BIN");
        let data: Vec<u8> = (0..4500u32).map(|i| (i * 7 % 256) as u8).collect();
        p.make_dir(dir_path.as_bytes())?;
        assert_eq!(p.kind(dir_path.as_bytes())?, NameKind::Directory);
        let t = DosDateTime::new(2001, 2, 3, 4, 5, 6).unwrap();
        p.put(path.as_bytes(), &data, Some(t), false)?;
        let (back, modified) = p.get(path.as_bytes())?;
        assert_eq!(back, data);
        if !oracle {
            // BSD Filer keeps the time of the copy, not the one set.
            assert_eq!(modified, Some(t));
        }
        // Not replaced without overwrite.
        let err = p.put(path.as_bytes(), b"x", None, false).unwrap_err();
        assert!(filer::is_exists(&err), "{err}");
        assert_eq!(p.get(path.as_bytes())?.0, data);
        // Rename within the drive; onto an existing name is refused.
        let moved = format!("C:\\{file}.MOV");
        p.rename(path.as_bytes(), moved.as_bytes())?;
        assert_eq!(p.get(moved.as_bytes())?.0, data);
        let err = p.rename(moved.as_bytes(), b"C:\\HELLO.TXT").unwrap_err();
        assert!(filer::is_exists(&err), "{err}");
        assert_eq!(
            names(p, format!("{dir_path}\\*.*").as_bytes())?,
            Vec::<String>::new()
        );
        p.delete(moved.as_bytes())?;
        assert!(
            !names(p, b"C:\\*.*")?.contains(&format!("{file}.MOV")),
            "still listed"
        );
        if !oracle {
            p.remove_dir(dir_path.as_bytes())?;
            assert_eq!(p.kind(dir_path.as_bytes())?, NameKind::Nothing);
        }
        Ok(())
    })
}

/// What BSD Filer cannot show: the error codes, overwrite (0x05),
/// rmdir (0x0E), the ROM drive.
#[test]
fn errors_in_process() -> TestResult {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut p = Palmtop::open(&local().0, DEFAULT_BAUD)?;
    let code = |e: Error| filer::fs_error(&e);
    assert_eq!(
        code(p.get(b"C:\\NONE.TXT").unwrap_err()),
        Some(FsError::NOT_FOUND)
    );
    assert_eq!(
        code(p.list(b"C:\\NONE\\*.*").unwrap_err()),
        Some(FsError::PATH_NOT_FOUND)
    );
    assert_eq!(
        code(p.put(b"D:\\X.TXT", b"x", None, false).unwrap_err()),
        Some(FsError::WRITE_PROTECTED)
    );
    assert_eq!(
        code(p.remove_dir(b"C:\\SUB").unwrap_err()),
        Some(FsError::ACCESS_DENIED)
    );
    assert_eq!(
        code(p.delete(b"C:\\NONE.TXT").unwrap_err()),
        Some(FsError::NOT_FOUND)
    );
    p.put(b"C:\\OVER.TXT", b"one", None, false)?;
    p.put(b"C:\\OVER.TXT", b"two", None, true)?;
    assert_eq!(p.get(b"C:\\OVER.TXT")?.0, b"two");
    p.delete(b"C:\\OVER.TXT")?;
    // The text of an error names its meaning.
    let err = p.get(b"C:\\NONE.TXT").unwrap_err();
    assert_eq!(err.to_string(), "palmtop: file does not exist (208)");
    assert!(matches!(err, Error::Filer(JobError::Failed(_))));
    p.close()?;
    Ok(())
}

/// A lost reply over a real socket: the client resends the same frame
/// after 5.9 s and the server answers it from its kept reply.
#[test]
fn a_lost_reply_is_resent() -> TestResult {
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (addr, server) = local();
    let mut p = Palmtop::open(addr, DEFAULT_BAUD)?;
    let before = server.lock().unwrap().executed();
    server.lock().unwrap().drop_replies(1);
    let started = Instant::now();
    assert_eq!(p.kind(b"C:\\SUB")?, NameKind::Directory);
    let took = started.elapsed();
    assert!(took >= Duration::from_millis(5900), "{took:?}");
    assert!(took < Duration::from_secs(9), "{took:?}");
    assert_eq!(server.lock().unwrap().executed(), before + 1);
    p.close()?;
    Ok(())
}
