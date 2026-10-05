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

    // Another token: 401, and the message carries neither token.
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
    assert!(err.contains("401"), "{err}");
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
    assert!(String::from_utf8_lossy(&out.stderr).contains("start `saturnus run`"));

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
    let out = bin()
        .args(["run", "--rom"])
        .arg(&rom)
        .args(["--screen"])
        .arg(&screen)
        .args(["--control", "4899"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}
