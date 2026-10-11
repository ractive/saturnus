//! The app embeds exactly the transfer page's files: the set `build.rs`
//! assembled in `$OUT_DIR/app-site`, without the app's own additions (the
//! page again as `index.html`, the wasm packages), equals what
//! `web/site.sh --list transfer` selects, so the two rules cannot drift
//! apart. Skipped where `sh` cannot be run (Windows without Git Bash); CI's
//! tauri job runs it on Linux.
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

/// What the app adds to the page's files: the page as `index.html` and,
/// once built, the two wasm packages.
fn added(name: &str) -> bool {
    name == "index.html" || name.starts_with("pkg/") || name.starts_with("tx/pkg/")
}

#[test]
fn the_app_embeds_the_files_site_sh_lists_for_the_transfer_page() {
    let site = PathBuf::from(env!("OUT_DIR")).join("app-site");
    let embedded = files_under(&site);
    assert!(
        embedded.iter().any(|f| f == "tx.html"),
        "no tx.html in {embedded:?}"
    );
    assert_eq!(
        std::fs::read(site.join("index.html")).unwrap(),
        std::fs::read(site.join("tx.html")).unwrap(),
        "index.html is the transfer page"
    );
    // The installable page's parts are the web site's only: the app has no
    // network and registers no service worker; the emulator's page is not
    // in it either.
    assert!(
        !embedded.iter().any(|f| f == "sw.js"
            || f.ends_with(".webmanifest")
            || f.starts_with("tx/pwa/")
            || f == "app.js"),
        "the app embeds a service worker, a manifest or the emulator's page: {embedded:?}"
    );
    let wasm: Vec<&String> = embedded.iter().filter(|f| f.contains("pkg/")).collect();
    assert!(
        wasm.is_empty() || wasm.len() == 4,
        "both wasm packages or none: {wasm:?}"
    );

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/site.sh");
    let listed = match Command::new("sh")
        .arg(&script)
        .args(["--list", "transfer"])
        .output()
    {
        Ok(out) => out,
        Err(e) => {
            eprintln!("cannot run sh ({e}): frontend check skipped");
            return;
        }
    };
    assert!(
        listed.status.success(),
        "web/site.sh --list transfer failed"
    );
    let mut listed: Vec<String> = String::from_utf8(listed.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    listed.sort();
    let page: Vec<String> = embedded.into_iter().filter(|f| !added(f)).collect();
    assert_eq!(
        page, listed,
        "build.rs and web/site.sh --list transfer select different page files"
    );
}
