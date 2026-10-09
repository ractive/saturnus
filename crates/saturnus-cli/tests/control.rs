//! `saturnus run` serving the control API and `saturnus ctl` as separate
//! processes, without a ROM: a 48SX on a ROM of zeros (it runs nonsense or
//! halts; info, memory and snapshots still work). The ROM-gated end of
//! this is `e2e.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Instance, TempDir, bin, png_size};

fn zero_rom(dir: &TempDir) -> std::path::PathBuf {
    let p = dir.0.join("zeros.rom");
    std::fs::write(&p, vec![0u8; 256 * 1024]).unwrap();
    p
}

#[test]
fn ctl_talks_to_a_running_saturnus() {
    let roms = TempDir::new("zero-rom");
    let rom = zero_rom(&roms);
    let run = Instance::start("48sx", &rom, &["--serial", "tcp:127.0.0.1:0"]);
    assert!(run.serial.is_some());

    let info = run.ctl_ok(&["info"]);
    assert!(info.contains("model: 48sx"), "{info}");
    assert!(info.contains("romName: zeros.rom"), "{info}");
    assert!(info.contains("romRevision: null"), "{info}");
    let v: serde_json::Value = serde_json::from_str(&run.ctl_ok(&["info", "--json"])).unwrap();
    assert_eq!(
        v["serial"],
        format!("tcp:127.0.0.1:{}", run.serial.unwrap())
    );
    assert_eq!(v["control"], format!("http://127.0.0.1:{}", run.control));
    assert!(v["romSha256"].as_str().unwrap().len() == 64);

    let nibbles = run.ctl_ok(&["mem", "read", "#0", "8"]);
    assert_eq!(nibbles.trim().len(), 8, "{nibbles}");
    let out = run.ctl(&["mem", "read", "FFFFF", "2"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("past the end"), "{err}");

    // Snapshots go through files on the client's side only.
    let state = run.dir.0.join("s.state");
    run.ctl_ok(&["snapshot", "get", state.to_str().unwrap()]);
    assert!(std::fs::metadata(&state).unwrap().len() > 32 * 1024);
    run.ctl_ok(&["snapshot", "put", state.to_str().unwrap()]);
    std::fs::write(&state, b"not a state").unwrap();
    let out = run.ctl(&["snapshot", "put", state.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("422"));

    let png = run.dir.0.join("s.png");
    run.ctl_ok(&["screen", "--png", png.to_str().unwrap()]);
    assert_eq!(png_size(&std::fs::read(&png).unwrap()), (131, 64));
    assert_eq!(run.ctl_ok(&["screen"]).lines().count(), 64);
    assert!(run.ctl_ok(&["cycles"]).contains("cycles: "));
    assert!(run.ctl_ok(&["model"]).contains("display: 131x64"));

    // Another token: the server cannot prove it, so ctl refuses before
    // sending the token, and the message carries neither token.
    let other = TempDir::new("other-token");
    let other_file = other.0.join("control-token");
    std::fs::write(&other_file, "b".repeat(64)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&other_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let out = bin()
        .args(["ctl", "--control", &run.control.to_string(), "--token-file"])
        .arg(&other_file)
        .arg("info")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("did not prove"), "{err}");
    assert!(
        !err.contains(&"b".repeat(64)) && !err.contains(&run.token()),
        "{err}"
    );
    let out = bin()
        .args(["ctl", "--control", &run.control.to_string(), "--token-file"])
        .arg(other.0.join("missing"))
        .arg("info")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("start `saturnus run --serve`"));

    // A second instance on the same control port is refused, naming the
    // listener or saying it cannot.
    let out = bin()
        .args(["run", "--rom"])
        .arg(&rom)
        .args([
            "--no-serial",
            "--control",
            &run.control.to_string(),
            "--token-file",
        ])
        .arg(&run.token_file)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("is in use by"), "{err}");

    assert!(run.stop(), "run did not stop cleanly on SIGINT");
}

#[test]
fn run_refuses_other_addresses_and_keeps_batch_mode() {
    let roms = TempDir::new("zero-rom-batch");
    let rom = zero_rom(&roms);
    for addr in ["0.0.0.0:4899", "192.168.0.1:4899", "[::1]:4899"] {
        let out = bin()
            .args(["run", "--rom"])
            .arg(&rom)
            .args(["--no-serial", "--control", addr])
            .output()
            .unwrap();
        assert!(!out.status.success());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("127.0.0.1 only"), "{addr}: {err}");
    }
    // The serial bridge stays on loopback unless --serial-remote says so.
    let out = bin()
        .args(["run", "--rom"])
        .arg(&rom)
        .args(["--serial", "tcp:0.0.0.0:4899"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--serial-remote"), "{err}");
    // With --screen and no --serial, `run` stops after the script, as
    // before the control API.
    let screen = roms.0.join("out.txt");
    let out = bin()
        .args(["run", "--rom"])
        .arg(&rom)
        .args(["--cycles", "1000", "--screen"])
        .arg(&screen)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&screen).unwrap().lines().count(),
        64
    );
}

/// Run `saturnus ARGS` that must finish on its own within 30 s; its
/// output.
fn finishes(args: &[&std::ffi::OsStr]) -> std::process::Output {
    let mut child = bin()
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    for _ in 0..600 {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = child.kill();
    panic!("saturnus {args:?} did not finish on its own");
}

/// Every run without --serve, --serial or --control is a batch run, as
/// before the control API: it runs and stops, serving nothing.
#[test]
fn runs_without_serving_flags_finish_as_before() {
    let dir = TempDir::new("batch");
    let rom = zero_rom(&dir);
    let keys = dir.0.join("keys.txt");
    std::fs::write(&keys, "wait 10\n").unwrap();
    let card = dir.0.join("card.img");
    let os = |s: &str| std::ffi::OsString::from(s);
    let base = [os("run"), os("--rom"), rom.clone().into_os_string()];
    let cases: Vec<Vec<std::ffi::OsString>> = vec![
        vec![],
        vec![os("--cycles"), os("1000")],
        vec![os("--keys"), keys.clone().into_os_string()],
        vec![
            os("--cycles"),
            os("1000"),
            os("--card1"),
            card.clone().into_os_string(),
            os("--card-writeback"),
        ],
        vec![os("--cycles"), os("100"), os("--trace"), os("5")],
        vec![os("--model"), os("48sx"), os("-v")],
    ];
    for extra in cases {
        let args: Vec<std::ffi::OsString> = base.iter().cloned().chain(extra).collect();
        let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
        let out = finishes(&refs);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !stdout.contains("control API") && !stdout.contains("serial bridged"),
            "{stdout}"
        );
    }
    assert!(card.exists(), "the card was written back");
    // Serving-only flags need --serve.
    for flag in ["--no-serial", "--no-control"] {
        let out = finishes(&[
            "run".as_ref(),
            "--rom".as_ref(),
            rom.as_os_str(),
            flag.as_ref(),
        ]);
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("only works with --serve"));
    }
}

/// A failing output does not skip the others: with a --screen path that
/// cannot be written the RAM card is still written back and the state
/// saved, and the run fails naming the screen.
#[test]
fn a_failing_output_still_writes_the_card() {
    let dir = TempDir::new("outputs");
    let rom = zero_rom(&dir);
    let card = dir.0.join("card.img");
    let state = dir.0.join("calc.state");
    let screen = dir.0.join("missing").join("screen.png");
    let out = finishes(&[
        "run".as_ref(),
        "--rom".as_ref(),
        rom.as_os_str(),
        "--cycles".as_ref(),
        "1000".as_ref(),
        "--screen".as_ref(),
        screen.as_os_str(),
        "--save".as_ref(),
        state.as_os_str(),
        "--card1".as_ref(),
        card.as_os_str(),
        "--card-writeback".as_ref(),
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("screen.png"));
    std::fs::remove_file(&card).ok();
    assert!(state.exists(), "the state was saved");
    // Again, now with the card's creation out of the way: the write-back
    // itself must happen.
    std::fs::write(&card, vec![0u8; 128 * 1024]).unwrap();
    let before = std::fs::metadata(&card).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let out = finishes(&[
        "run".as_ref(),
        "--rom".as_ref(),
        rom.as_os_str(),
        "--cycles".as_ref(),
        "1000".as_ref(),
        "--screen".as_ref(),
        screen.as_os_str(),
        "--card1".as_ref(),
        card.as_os_str(),
        "--card-writeback".as_ref(),
    ]);
    assert!(!out.status.success());
    let after = std::fs::metadata(&card).unwrap().modified().unwrap();
    assert!(after > before, "the card was written back");
}

/// A serving run without a serial bridge (as on the 42S) still writes
/// --save, --screen and --annunciators when it stops.
#[test]
fn serving_without_serial_writes_outputs_on_stop() {
    let dir = TempDir::new("serve-out");
    for (model, size) in [("48sx", 256 * 1024), ("42s", 64 * 1024)] {
        let rom = dir.0.join(format!("{model}.rom"));
        std::fs::write(&rom, vec![0u8; size]).unwrap();
        let state = dir.0.join(format!("{model}.state"));
        let screen = dir.0.join(format!("{model}.txt"));
        let ann = dir.0.join(format!("{model}.ann"));
        let mut extra = vec!["--serve"];
        if model == "48sx" {
            extra.push("--no-serial");
        }
        let args: Vec<String> = extra
            .iter()
            .map(|s| s.to_string())
            .chain([
                "--save".into(),
                state.display().to_string(),
                "--screen".into(),
                screen.display().to_string(),
                "--annunciators".into(),
                ann.display().to_string(),
            ])
            .collect();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let run = Instance::start(model, &rom, &refs);
        assert!(run.serial.is_none(), "{model}");
        assert!(run.ctl_ok(&["info"]).contains(&format!("model: {model}")));
        let shown = run.ctl_ok(&["screen"]);
        let stopped = run.stop();
        // SIGINT lets it finish; on Windows the test can only kill it.
        if cfg!(unix) {
            assert!(stopped, "{model}");
            assert!(std::fs::metadata(&state).unwrap().len() > 0);
            assert_eq!(
                std::fs::read_to_string(&screen).unwrap().lines().count(),
                shown.lines().count()
            );
            assert!(ann.exists());
        }
    }
}

/// Another local user who binds the port first never gets the token:
/// `ctl` asks for the server's proof without the token and refuses one
/// that does not prove the token (none, a wrong one, a 401).
#[test]
fn ctl_does_not_send_the_token_to_an_impostor() {
    use std::io::{Read as _, Write as _};
    let dir = TempDir::new("impostor");
    let token = "ab".repeat(32);
    let token_file = dir.0.join("control-token");
    std::fs::write(&token_file, &token).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let wrong = format!(
        r#"{{"type":"reply","ok":true,"result":{{"proof":"{}"}}}}"#,
        "0".repeat(64)
    );
    for reply in [
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{wrong}",
            wrong.len()
        ),
        "HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\n{}".to_string(),
        "HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\n\r\n{}".to_string(),
    ] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for s in listener.incoming() {
                let Ok(mut s) = s else { return };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                log.lock().unwrap().extend_from_slice(&buf[..n]);
                let _ = s.write_all(reply.as_bytes());
            }
        });
        let out = finishes(&[
            "ctl".as_ref(),
            "--control".as_ref(),
            port.to_string().as_ref(),
            "--token-file".as_ref(),
            token_file.as_os_str(),
            "info".as_ref(),
        ]);
        assert!(!out.status.success());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("did not prove"), "{err}");
        let seen = String::from_utf8_lossy(&seen.lock().unwrap()).into_owned();
        assert!(seen.contains("GET /v1/hello?nonce="), "{seen}");
        assert!(
            !seen.contains(&token) && !seen.contains("Authorization"),
            "{seen}"
        );
    }
}
