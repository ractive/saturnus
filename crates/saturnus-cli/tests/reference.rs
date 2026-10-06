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
    // In the ROM's PRG OBJ menu; the owner's manual names the PRG key.
    assert_eq!(v["category"], "PRG OBJ");
    assert_eq!(v["category_source"]["source"], "rom");
    assert_eq!(v["categories"]["48sx"]["key"]["category"], "PRG");
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

#[test]
fn ambiguous_queries_list_the_candidates() {
    let out = saturnus_ref(&["Qr", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["candidates"], serde_json::json!(["QR", "qr"]));
    let out = saturnus_ref(&["\\.S", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["name"], "\u{222b}");
}

#[test]
fn help_texts_belong_to_their_subcommands() {
    let out = Command::new(env!("CARGO_BIN_EXE_saturnus"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8(out.stdout).unwrap();
    let line = |cmd: &str| {
        help.lines()
            .find(|l| l.trim_start().starts_with(&format!("{cmd} ")))
            .unwrap_or_else(|| panic!("no {cmd} in {help}"))
            .to_string()
    };
    assert!(line("ref").contains("Look up a built-in command"), "{help}");
    assert!(line("rom").contains("ROM image management"), "{help}");
    assert!(!line("ref").contains("ROM image"), "{help}");
}
