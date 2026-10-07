//! The app embeds exactly the web page's files: the set `build.rs`
//! assembled in `$OUT_DIR/app-site` equals what `web/site.sh --list`
//! selects, so the two rules cannot drift apart. Skipped where `sh` cannot
//! be run (Windows without Git Bash); CI's tauri job runs it on Linux.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The files under `dir`, relative to it with `/` separators, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap();
                let parts: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.push(parts.join("/"));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_app_embeds_the_page_files_site_sh_selects() {
    let embedded = files_under(&PathBuf::from(env!("OUT_DIR")).join("app-site"));
    assert!(
        embedded.iter().any(|f| f == "index.html"),
        "no index.html in {embedded:?}"
    );

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/site.sh");
    let listed = match Command::new("sh").arg(&script).arg("--list").output() {
        Ok(out) => out,
        Err(e) => {
            eprintln!("cannot run sh ({e}): frontend check skipped");
            return;
        }
    };
    assert!(listed.status.success(), "web/site.sh --list failed");
    let mut listed: Vec<String> = String::from_utf8(listed.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    listed.sort();
    assert_eq!(
        embedded, listed,
        "build.rs and web/site.sh select different page files"
    );
}
