//! The palmtop device against BSD Filer in Docker (`just lx-oracle-up`,
//! `tools/filer-oracle/`), over TCP: connect, the drives, a listing, a put
//! read back byte for byte, a put to a name in use refused, a rename, a
//! delete. Runs only with `FILER_E2E_ADDR` set (`tcp://localhost:4853`);
//! skips otherwise. BSD Filer has no remove-directory (0x0E) and sends no
//! error codes (`tools/filer-oracle/README.md`), so neither is asked here;
//! the in-process server covers them (`src/palmtop_tests.rs`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use saturnus_tx::Action;
use saturnus_tx::palmtop::{Palmtop, PalmtopError, PalmtopOutcome, PalmtopRequest};

fn run(
    link: &mut TcpStream,
    p: &mut Palmtop,
    request: PalmtopRequest,
) -> Result<PalmtopOutcome, PalmtopError> {
    p.start(Instant::now(), request).unwrap();
    let mut buf = [0u8; 4096];
    let end = Instant::now() + Duration::from_secs(120);
    loop {
        while let Some(action) = p.poll_action() {
            if let Action::Write(bytes) = action {
                link.write_all(&bytes).unwrap();
            }
        }
        if let Some(result) = p.poll_result() {
            return result;
        }
        assert!(Instant::now() < end, "no result in 120 s");
        let now = Instant::now();
        let wait = p.next_timeout().map_or(Duration::from_millis(100), |t| {
            t.saturating_duration_since(now)
        });
        link.set_read_timeout(Some(wait.max(Duration::from_millis(1))))
            .unwrap();
        match link.read(&mut buf) {
            Ok(0) => panic!("the oracle closed the connection"),
            Ok(n) => p.handle_input(Instant::now(), &buf[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                p.handle_timeout(Instant::now());
            }
            Err(e) => panic!("{e}"),
        }
    }
}

fn names(outcome: Result<PalmtopOutcome, PalmtopError>) -> Vec<String> {
    match outcome {
        Ok(PalmtopOutcome::Listing { entries, .. }) => {
            entries.into_iter().map(|e| e.name).collect()
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn against_bsd_filer() {
    let Ok(addr) = std::env::var("FILER_E2E_ADDR") else {
        eprintln!("FILER_E2E_ADDR not set: skipped");
        return;
    };
    let host = addr.strip_prefix("tcp://").unwrap_or(&addr);
    let mut link = TcpStream::connect(host).unwrap();
    let mut p = Palmtop::new();
    let c = vec!["C:".to_string()];
    // A name of this run, so a second run starts clean.
    let stamp = format!("{:05}", std::process::id() % 100_000);
    let name = format!("T{stamp}.BIN");
    let renamed = format!("R{stamp}.BIN");
    assert!(matches!(
        run(&mut link, &mut p, PalmtopRequest::Connect { baud: 19200 }),
        Ok(PalmtopOutcome::Connected(_))
    ));
    assert!(
        names(run(&mut link, &mut p, PalmtopRequest::List { dir: vec![] }))
            .contains(&"C:".to_string())
    );
    let listing = names(run(
        &mut link,
        &mut p,
        PalmtopRequest::List { dir: c.clone() },
    ));
    assert!(listing.contains(&"HELLO.TXT".to_string()), "{listing:?}");
    let data: Vec<u8> = (0..4500u32).map(|i| (i % 256) as u8).collect();
    let put = |p: &mut Palmtop, link: &mut TcpStream, replace| {
        run(
            link,
            p,
            PalmtopRequest::Put {
                dir: c.clone(),
                name: name.to_lowercase(),
                data: data.clone(),
                replace,
            },
        )
    };
    assert_eq!(
        put(&mut p, &mut link, false),
        Ok(PalmtopOutcome::Stored(name.clone()))
    );
    assert_eq!(
        put(&mut p, &mut link, false),
        Err(PalmtopError::Exists(name.clone()))
    );
    let got = run(
        &mut link,
        &mut p,
        PalmtopRequest::Get {
            dir: c.clone(),
            name: name.clone(),
        },
    );
    let Ok(PalmtopOutcome::File { data: back, .. }) = got else {
        panic!("{got:?}")
    };
    assert_eq!(back, data, "byte for byte");
    assert_eq!(
        run(
            &mut link,
            &mut p,
            PalmtopRequest::Rename {
                dir: c.clone(),
                from: name.clone(),
                to: renamed.clone()
            }
        ),
        Ok(PalmtopOutcome::Renamed {
            from: name.clone(),
            to: renamed.clone()
        })
    );
    assert_eq!(
        run(
            &mut link,
            &mut p,
            PalmtopRequest::Remove {
                dir: c.clone(),
                name: renamed.clone(),
                directory: false
            }
        ),
        Ok(PalmtopOutcome::Removed {
            name: renamed.clone(),
            count: 0
        })
    );
    assert!(
        !names(run(
            &mut link,
            &mut p,
            PalmtopRequest::List { dir: c.clone() }
        ))
        .contains(&renamed)
    );
    assert_eq!(
        run(&mut link, &mut p, PalmtopRequest::Disconnect),
        Ok(PalmtopOutcome::Disconnected)
    );
    while let Some(Action::Write(bytes)) = p.poll_action() {
        link.write_all(&bytes).unwrap();
    }
}
