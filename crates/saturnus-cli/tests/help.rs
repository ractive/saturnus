//! A bare `saturnus` prints the short help on stdout and exits 0, as winget's
//! validator expects; bad arguments still fail (kb/decision-log.md,
//! 2026-11-01).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::{Command, Output};

fn saturnus(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saturnus"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn no_arguments_prints_the_short_help_and_succeeds() {
    let bare = saturnus(&[]);
    assert_eq!(bare.status.code(), Some(0), "{bare:?}");
    let stdout = String::from_utf8(bare.stdout.clone()).unwrap();
    assert!(stdout.contains("Usage: saturnus"), "{stdout}");
    assert!(bare.stderr.is_empty(), "{bare:?}");
    // Exactly what -h prints.
    let short = saturnus(&["-h"]);
    assert!(short.status.success(), "{short:?}");
    assert_eq!(stdout, String::from_utf8(short.stdout).unwrap());
}

#[test]
fn bad_arguments_still_fail() {
    for args in [&["no-such-command"][..], &["--no-such-flag"], &["rom"]] {
        let out = saturnus(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {out:?}");
        assert!(out.stdout.is_empty(), "{args:?}: {out:?}");
    }
}
