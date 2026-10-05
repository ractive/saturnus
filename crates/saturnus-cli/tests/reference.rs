//! `saturnus ref`: the embedded command reference, looked up from the
//! command line (no ROM needed).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

fn saturnus_ref(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saturnus"))
        .arg("ref")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn ascii_spellings_and_json() {
    let out = saturnus_ref(&["->list", "--model", "48sx", "--json"]);
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["name"], "\u{2192}LIST");
    assert_eq!(v["category"], "PRG OBJ");
    assert_eq!(v["stack_verified"], true);
    assert!(
        v["examples"]["48sx"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    );
    assert!(v["examples"].get("49g").is_none());
}

#[test]
fn text_and_unknown_names() {
    let out = saturnus_ref(&["off"]);
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with("OFF"), "{text}");
    assert!(text.contains("from the manuals, not run here"), "{text}");
    let out = saturnus_ref(&["NOSUCHCOMMAND"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no command"));
}
