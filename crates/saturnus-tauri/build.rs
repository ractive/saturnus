//! Assembles the app's page in `$OUT_DIR/app-site` from `web/`, with the
//! rule `web/site.sh --list` uses for the web app (iteration 50d: one
//! document, `index.html`, with the home page, Calculator and Files): every
//! page file at the top of `web/` (HTML, CSS, JS, JSON, SVG),
//! `components/*.js`, `tx/*.js` and `tx/*.css`, and the shared code
//! (`shared/*.js`, `shared/*.css`, `shared/*.svg`, `shared/components/*.js`)
//! and the vendored `vendor/tab-mcp-client/*.js` (the web app's AI access,
//! unused in the desktop app).
//! Not the installable page's files (`web/pwa/`: the app has no network
//! and registers no service worker), the tests, the docs or the build
//! scripts.
//!
//! The calculator runs natively, but Files' protocol, its aplet server and
//! its private calculators run in the page's wasm workers (iteration 51),
//! so the wasm package goes in too (`web/pkg/`, from `web/build.sh`).
//! `cargo tauri build` makes it first (`beforeBuildCommand`); a release
//! build by the Tauri CLI without it fails here, any other build (clippy,
//! the tests) only warns.
//!
//! Then points Tauri's code generation at that directory: `frontendDist`
//! in `tauri.conf.json` stays `../../web`, which the Tauri CLI checks for
//! and `cargo tauri dev` serves, and the embedded copy is set through
//! `TAURI_CONFIG`, merged into what the CLI may already have put there
//! (its dev server's URL). `tests/frontend.rs` checks the selection
//! against `web/site.sh --list`.

use std::error::Error;
use std::path::{Path, PathBuf};

/// The files of `web/<dir>` whose names end in one of `exts`, as paths
/// relative to `web`, into `files`.
fn files_in(web: &Path, dir: &str, exts: &[&str], files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(web.join(dir))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_file()
            && !name.starts_with('.')
            && exts.iter().any(|ext| name.ends_with(ext))
        {
            files.push(Path::new(dir).join(name));
        }
    }
    Ok(())
}

/// The app's page files of `web`, relative to it, sorted.
fn page_files(web: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    files_in(
        web,
        ".",
        &[".html", ".css", ".js", ".json", ".svg"],
        &mut files,
    )?;
    for f in &mut files {
        if let Ok(rel) = f.strip_prefix(".") {
            *f = rel.to_path_buf();
        }
    }
    files_in(web, "components", &[".js"], &mut files)?;
    files_in(web, "tx", &[".js", ".css"], &mut files)?;
    files_in(web, "shared", &[".js", ".css", ".svg"], &mut files)?;
    files_in(web, "shared/components", &[".js"], &mut files)?;
    files_in(web, "vendor/tab-mcp-client", &[".js"], &mut files)?;
    files.sort();
    Ok(files)
}

/// The wasm package the page loads, relative to `web`.
const WASM: [&str; 2] = ["pkg/saturnus_web.js", "pkg/saturnus_web_bg.wasm"];

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let web = manifest.join("../../web");
    let site = PathBuf::from(std::env::var("OUT_DIR")?).join("app-site");
    println!("cargo::rerun-if-changed={}", web.display());

    if site.exists() {
        std::fs::remove_dir_all(&site)?;
    }
    for dir in [
        "components",
        "tx",
        "pkg",
        "shared/components",
        "vendor/tab-mcp-client",
    ] {
        std::fs::create_dir_all(site.join(dir))?;
    }
    for file in page_files(&web)? {
        std::fs::copy(web.join(&file), site.join(&file))?;
    }

    // `tauri`'s build script says whether this is the CLI's production
    // build (its custom-protocol feature): there the page must work.
    let production = std::env::var("DEP_TAURI_DEV").is_ok_and(|v| v == "false");
    let missing: Vec<&str> = WASM
        .iter()
        .copied()
        .filter(|f| !web.join(f).is_file())
        .collect();
    if missing.is_empty() {
        for f in WASM {
            std::fs::copy(web.join(f), site.join(f))?;
        }
    } else if production {
        return Err(format!(
            "the page's wasm is missing ({}): run web/build.sh first",
            missing.join(", ")
        )
        .into());
    } else {
        println!(
            "cargo::warning=the page's wasm is not built ({}); Files' transfers and private calculators will not start. Run web/build.sh.",
            missing.join(", ")
        );
    }

    let mut config: serde_json::Value = match std::env::var("TAURI_CONFIG") {
        Ok(text) => serde_json::from_str(&text)?,
        Err(_) => serde_json::json!({}),
    };
    let site_text = site.to_str().ok_or("OUT_DIR is not valid UTF-8")?;
    config["build"]["frontendDist"] = site_text.into();
    println!("cargo::rerun-if-env-changed=TAURI_CONFIG");
    println!("cargo::rustc-env=TAURI_CONFIG={config}");

    tauri_build::build();
    Ok(())
}
