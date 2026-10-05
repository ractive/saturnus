//! A `saturnus run` process serving on ephemeral ports, and `saturnus ctl`
//! against it, for the integration tests. The token file lives in a
//! temporary directory, never in the user's real location.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// The `saturnus` binary under test.
pub fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_saturnus"));
    // Neither the user's settings nor their token file.
    c.env_remove("SATURNUS_CONTROL")
        .env_remove("SATURNUS_TOKEN_FILE");
    c
}

/// A temporary directory removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("saturnus-it-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A serving `saturnus run`.
pub struct Instance {
    child: Option<Child>,
    /// The control API's port.
    pub control: u16,
    /// The serial bridge's TCP port, if it listens.
    pub serial: Option<u16>,
    /// The token file it uses.
    pub token_file: PathBuf,
    pub dir: TempDir,
}

impl Instance {
    /// Start `run --model MODEL --rom ROM` on ephemeral ports with `extra`
    /// arguments; waits for its startup lines.
    pub fn start(model: &str, rom: &Path, extra: &[&str]) -> Self {
        let dir = TempDir::new("run");
        let token_file = dir.0.join("tokens").join("control-token");
        let mut child = bin()
            .args(["run", "--model", model, "--rom"])
            .arg(rom)
            .args(["--control", "127.0.0.1:0", "--token-file"])
            .arg(&token_file)
            .args(extra)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut serial = None;
        let control = loop {
            let line = rx
                .recv_timeout(Duration::from_secs(60))
                .expect("no startup lines from saturnus run");
            if let Some(rest) = line.strip_prefix("serial bridged on tcp:") {
                serial = Some(rest.rsplit(':').next().unwrap().parse().unwrap());
            }
            if let Some(rest) = line.strip_prefix("control API on http://127.0.0.1:") {
                break rest.split(' ').next().unwrap().parse().unwrap();
            }
        };
        Self {
            child: Some(child),
            control,
            serial,
            token_file,
            dir,
        }
    }

    /// `saturnus ctl ARGS` against this instance.
    pub fn ctl(&self, args: &[&str]) -> Output {
        bin()
            .arg("ctl")
            .args(["--control", &self.control.to_string(), "--token-file"])
            .arg(&self.token_file)
            .args(args)
            .output()
            .unwrap()
    }

    /// `ctl ARGS`, which must succeed; its stdout.
    pub fn ctl_ok(&self, args: &[&str]) -> String {
        let out = self.ctl(args);
        assert!(
            out.status.success(),
            "ctl {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// The token, read as a client would (for raw requests).
    pub fn token(&self) -> String {
        std::fs::read_to_string(&self.token_file)
            .unwrap()
            .trim()
            .to_string()
    }

    /// Stop it as Ctrl-C would (SIGINT; on Windows a kill); whether it
    /// exited successfully (always true on Windows).
    pub fn stop(mut self) -> bool {
        let mut child = self.child.take().unwrap();
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-INT", &child.id().to_string()])
                .status();
            for _ in 0..200 {
                if let Some(status) = child.try_wait().unwrap() {
                    return status.success();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = child.kill();
            let _ = child.wait();
            false
        }
        #[cfg(not(unix))]
        {
            let _ = child.kill();
            let _ = child.wait();
            true
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Width and height from a PNG's IHDR chunk.
pub fn png_size(png: &[u8]) -> (u32, u32) {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[12..16], b"IHDR");
    let be = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    (be(&png[16..20]), be(&png[20..24]))
}
