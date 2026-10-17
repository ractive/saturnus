//! Assembles the app's front end in `$OUT_DIR/app-site` from `web/`, with
//! the rule `web/site.sh` uses for the web app (iteration 50d: one
//! document, `index.html`, with the home page, Calculator and Files): every
//! page file at the top of `web/` (HTML, CSS, JS, JSON, SVG),
//! `components/*.js`, Files' `tx/*.js` and `tx/*.css` (the document loads
//! them; the desktop app shows Calculator only, iteration 51 merges the
//! apps) and the shared code (`shared/*.js`, `shared/*.css`, `shared/*.svg` (the icon
//! sprite), `shared/components/*.js`).
//! Not the wasm package (the app runs the core natively), not the
//! `.htaccess`, the installable page's `web/pwa/` (service worker,
//! manifest, icons: the app has no network and registers no worker), the
//! tests, the docs or the build scripts. Then points Tauri's code
//! generation at that directory: `frontendDist` in `tauri.conf.json` stays
//! `../../web`, which the Tauri CLI checks for and `cargo tauri dev`
//! serves, and the embedded copy is set through `TAURI_CONFIG`, merged
//! into what the CLI may already have put there (its dev server's URL).
//! `tests/frontend.rs` checks that this selection equals
//! `web/site.sh --list`.

use std::error::Error;
use std::path::{Path, PathBuf};

/// Whether `name`, a file at the top of `web/`, is a page file of the app.
fn top_level_page_file(name: &str) -> bool {
    [".html", ".css", ".js", ".json", ".svg"]
        .iter()
        .any(|ext| name.ends_with(ext))
}

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

/// The page files of `web`, relative to it, sorted.
fn page_files(web: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(web)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_file() && !name.starts_with('.') && top_level_page_file(&name) {
            files.push(PathBuf::from(name));
        }
    }
    files_in(web, "components", &[".js"], &mut files)?;
    files_in(web, "tx", &[".js", ".css"], &mut files)?;
    files_in(web, "shared", &[".js", ".css", ".svg"], &mut files)?;
    files_in(web, "shared/components", &[".js"], &mut files)?;
    files.sort();
    Ok(files)
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let web = manifest.join("../../web");
    let site = PathBuf::from(std::env::var("OUT_DIR")?).join("app-site");
    println!("cargo::rerun-if-changed={}", web.display());

    if site.exists() {
        std::fs::remove_dir_all(&site)?;
    }
    std::fs::create_dir_all(site.join("components"))?;
    std::fs::create_dir_all(site.join("tx"))?;
    std::fs::create_dir_all(site.join("shared/components"))?;
    for file in page_files(&web)? {
        std::fs::copy(web.join(&file), site.join(&file))?;
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
