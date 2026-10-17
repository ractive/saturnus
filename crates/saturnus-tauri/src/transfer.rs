//! What the page's Files view needs of the computer (iteration 51; the
//! commands of the former saturnus tx app, iteration 40): its protocol
//! stays in the page's wasm (kb/docs/saturnus-tx.md, "Crates and folders");
//! this side only moves bytes and files.
//!
//! - **Serial** ([`serial`]): `serial_ports` (the adapters' names),
//!   `serial_open` (a port at the page's settings; its bytes and its loss
//!   come back on the channel the page passes), `serial_write` (the bytes
//!   as the request's raw body), `serial_close`. Ports are closed when the
//!   page (re)loads.
//! - **Files** ([`files`]): the folder pane is the computer's file
//!   system, opening at the home folder (`folder_home`), with
//!   `folder_list`, `folder_stat`, `folder_read`, `folder_create`,
//!   `folder_write`, `folder_mkdir`, `folder_remove`, `folder_rename`, and
//!   `folder_pick` (Tauri's folder dialog, "Open a folder…").
//!   `save_file` saves a download through a save dialog (the webview's own
//!   downloads do not reach the disk).
//! - **Remembered folders** ([`places`]): `folder_recent`, `folder_used`
//!   and `folder_recent_clear`, the pane's "Recent folders", in
//!   `folders.json` in the app's config folder.
//!
//! Scope (owner, 2026-10-11 and 2026-10-14): the folder pane browses
//! everything the user's account may, as a file manager does:
//! `folder_list` and `folder_stat` are not confined. Changes and reads of
//! a file's bytes are: `folder_read`, `folder_create`, `folder_write`,
//! `folder_mkdir`, `folder_remove` and `folder_rename` refuse a path that
//! is not under a root ([`files::Roots`]), the home folder or a folder
//! the user picked in the app's dialog ("Open a folder…", remembered in
//! `folders.json` across runs; a mounted volume is picked once like any
//! other). The page cannot add a root: the dialog is opened here, and a
//! folder the page reports with `folder_used` is listed, never a root. A
//! recursive remove of the home folder or a folder directly in it is
//! refused whatever the roots. The aplet server's writes carry an
//! `x-serve` header and are refused directly in the home folder: that
//! guards against a bug in the page, not against a hostile page, which
//! would leave the header out. What else stays tight: no Tauri plugin but
//! the dialog (opened from Rust only; the page has no dialog, fs, shell or
//! opener permission), the serial bytes come on a channel, the page is
//! the embedded files only under a strict CSP, every path is a list of
//! plain names checked here ([`files::check_name`]: no `..`, no
//! separators), only plain files and folders are listed or read, and a
//! read is capped.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody, Request, Response};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::{files, places, serial};

/// The open serial ports, shared with the commands.
#[derive(Debug, Default)]
pub struct Serial(pub Arc<serial::Ports>);

/// Run blocking `f` off the main thread.
pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("the app's worker failed: {e}"))?
}

/// The raw body of `request`, or an error naming the command.
fn raw_body(request: &Request<'_>, what: &str) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(_) => Err(format!("{what}: the bytes must come as a raw body")),
    }
}

/// Header `name` of `request`, percent-decoded (the page sends
/// `encodeURIComponent` of the text: headers are ASCII only).
fn header(request: &Request<'_>, name: &str) -> Result<String, String> {
    let value = request
        .headers()
        .get(name)
        .ok_or_else(|| format!("no {name} header"))?
        .to_str()
        .map_err(|_| format!("the {name} header is not ASCII"))?;
    percent_decode(value).ok_or_else(|| format!("the {name} header is not percent-encoded UTF-8"))
}

/// `%XX` escapes of `s` decoded, the result UTF-8; `None` when it is not.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            // Exactly two hex digits: `from_str_radix` alone would also
            // take a sign (`%+1`).
            let (hi, lo) = (hex_digit(*b.get(i + 1)?)?, hex_digit(*b.get(i + 2)?)?);
            out.push(hi << 4 | lo);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The value of the ASCII hex digit `c`.
fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// A path header: the JSON list of names, percent-encoded.
fn path_header(request: &Request<'_>) -> Result<Vec<String>, String> {
    serde_json::from_str(&header(request, "x-path")?).map_err(|e| format!("bad x-path header: {e}"))
}

// ------------------------------------------------------------ serial

#[tauri::command]
pub async fn serial_ports() -> Result<Vec<serial::Port>, String> {
    blocking(serial::ports).await
}

/// What the channel carries besides the bytes (raw): `{"lost": why}`.
#[derive(Serialize)]
struct Lost<'a> {
    lost: &'a str,
}

#[tauri::command]
pub async fn serial_open(
    state: State<'_, Serial>,
    name: String,
    baud: u32,
    parity: String,
    data_bits: u8,
    on_event: Channel<InvokeResponseBody>,
) -> Result<u32, String> {
    let settings = serial::Settings::parse(baud, &parity, data_bits)?;
    let ports = Arc::clone(&state.0);
    // The page this open is for: a reload meanwhile refuses it.
    let generation = ports.generation();
    blocking(move || {
        ports.open_for(generation, &name, settings, move |event| {
            let body = match event {
                serial::Event::Data(bytes) => InvokeResponseBody::Raw(bytes),
                serial::Event::Lost(why) => match serde_json::to_string(&Lost { lost: &why }) {
                    Ok(json) => InvokeResponseBody::Json(json),
                    Err(_) => return,
                },
            };
            // The page went away: the next load closes the port.
            let _ = on_event.send(body);
        })
    })
    .await
}

#[tauri::command]
pub async fn serial_write(state: State<'_, Serial>, request: Request<'_>) -> Result<(), String> {
    let id: u32 = header(&request, "x-port")?
        .parse()
        .map_err(|_| "bad x-port header".to_string())?;
    let bytes = raw_body(&request, "serial_write")?;
    let ports = Arc::clone(&state.0);
    blocking(move || ports.write(id, &bytes)).await
}

#[tauri::command]
pub async fn serial_close(state: State<'_, Serial>, id: u32) -> Result<(), String> {
    let ports = Arc::clone(&state.0);
    blocking(move || {
        ports.close(id);
        Ok(())
    })
    .await
}

// ------------------------------------------------------------ files

/// A file-system call off the main thread, its error as the page reads it.
async fn fs_call<T: Send + 'static>(
    f: impl FnOnce() -> files::Result<T> + Send + 'static,
) -> Result<T, String> {
    blocking(move || f().map_err(|e| e.to_page())).await
}

#[tauri::command]
pub async fn folder_home(app: AppHandle) -> Result<Vec<String>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| format!("OsError: no home folder: {e}"))?;
    files::split(&home).map_err(|e| e.to_page())
}

/// The remembered folders and the roots (the scope note), shared with the
/// folder commands.
#[derive(Debug)]
pub struct Folders(pub Arc<Mutex<places::Places>>);

/// Run `f` with the places, off the main thread.
async fn with_places<T: Send + 'static>(
    state: &State<'_, Folders>,
    f: impl FnOnce(&mut places::Places) -> files::Result<T> + Send + 'static,
) -> Result<T, String> {
    let places = Arc::clone(&state.0);
    fs_call(move || {
        let mut p = places
            .lock()
            .map_err(|_| files::FsError::new("OsError", "the app's list of folders is lost"))?;
        f(&mut p)
    })
    .await
}

/// Run `act` once `check` passes on the roots, off the main thread (the
/// list is not held while `act` runs: a long read blocks no other).
async fn checked<T: Send + 'static>(
    state: &State<'_, Folders>,
    check: impl Fn(&files::Roots) -> files::Result<()> + Send + 'static,
    act: impl FnOnce() -> files::Result<T> + Send + 'static,
) -> Result<T, String> {
    with_places(state, move |p| p.check(check)).await?;
    fs_call(act).await
}

/// Whether the request is a write of the aplet server (`x-serve: 1`).
fn served(request: &Request<'_>) -> bool {
    request
        .headers()
        .get("x-serve")
        .is_some_and(|v| v.as_bytes() == b"1")
}

/// The folder chosen in Tauri's dialog, as names; `None` when cancelled.
/// It becomes a root, remembered for the next runs.
#[tauri::command]
pub async fn folder_pick(
    app: AppHandle,
    state: State<'_, Folders>,
) -> Result<Option<Vec<String>>, String> {
    let chosen = blocking(move || {
        Ok(app
            .dialog()
            .file()
            .set_title("Open a folder")
            .blocking_pick_folder())
    })
    .await?;
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let path = chosen
        .into_path()
        .map_err(|e| format!("OsError: that folder cannot be opened: {e}"))?;
    with_places(&state, move |p| p.pick(&path).map(Some)).await
}

#[tauri::command]
pub async fn folder_list(path: Vec<String>) -> Result<Vec<files::Entry>, String> {
    fs_call(move || files::list(&path)).await
}

#[tauri::command]
pub async fn folder_stat(path: Vec<String>) -> Result<files::Entry, String> {
    fs_call(move || files::stat(&path)).await
}

#[tauri::command]
pub async fn folder_read(
    state: State<'_, Folders>,
    path: Vec<String>,
    start: Option<u64>,
    end: Option<u64>,
) -> Result<Response, String> {
    let at = files::resolve(&path).map_err(|e| e.to_page())?;
    checked(
        &state,
        move |r| r.check(&at),
        move || files::read(&path, start.unwrap_or(0), end),
    )
    .await
    .map(Response::new)
}

#[tauri::command]
pub async fn folder_create(
    state: State<'_, Folders>,
    request: Request<'_>,
    path: Vec<String>,
) -> Result<(), String> {
    let at = files::resolve(&path).map_err(|e| e.to_page())?;
    let serve = served(&request);
    checked(
        &state,
        move |r| {
            if serve {
                r.check_served(&at)
            } else {
                r.check(&at)
            }
        },
        move || files::create(&path),
    )
    .await
}

#[tauri::command]
pub async fn folder_write(state: State<'_, Folders>, request: Request<'_>) -> Result<(), String> {
    let path = path_header(&request)?;
    let bytes = raw_body(&request, "folder_write")?;
    let at = files::resolve(&path).map_err(|e| e.to_page())?;
    let serve = served(&request);
    checked(
        &state,
        move |r| {
            if serve {
                r.check_served(&at)
            } else {
                r.check(&at)
            }
        },
        move || files::write(&path, &bytes),
    )
    .await
}

#[tauri::command]
pub async fn folder_mkdir(state: State<'_, Folders>, path: Vec<String>) -> Result<(), String> {
    let at = files::resolve(&path).map_err(|e| e.to_page())?;
    checked(&state, move |r| r.check(&at), move || files::mkdir(&path)).await
}

#[tauri::command]
pub async fn folder_remove(
    state: State<'_, Folders>,
    path: Vec<String>,
    recursive: bool,
) -> Result<(), String> {
    if path.is_empty() || (cfg!(windows) && path.len() == 1) {
        // The top or a drive: files::remove says so.
        return fs_call(move || files::remove(&path, recursive)).await;
    }
    let at = files::resolve(&path).map_err(|e| e.to_page())?;
    checked(
        &state,
        move |r| r.check_remove(&at, recursive),
        move || files::remove(&path, recursive),
    )
    .await
}

#[tauri::command]
pub async fn folder_rename(
    state: State<'_, Folders>,
    dir: Vec<String>,
    from: String,
    to: String,
) -> Result<(), String> {
    let at = files::resolve(&dir).map_err(|e| e.to_page())?;
    checked(
        &state,
        move |r| r.check(&at),
        move || files::rename(&dir, &from, &to),
    )
    .await
}

/// The pane's "Recent folders": `[{names, picked, used}]`, newest first,
/// those that are there.
#[tauri::command]
pub async fn folder_recent(state: State<'_, Folders>) -> Result<Vec<places::Place>, String> {
    with_places(&state, |p| Ok(p.recent())).await
}

/// The page reports a folder its pane showed: to the top of the list.
#[tauri::command]
pub async fn folder_used(state: State<'_, Folders>, path: Vec<String>) -> Result<(), String> {
    with_places(&state, move |p| p.used(path)).await
}

/// "Clear recent folders": every remembered folder forgotten.
#[tauri::command]
pub async fn folder_recent_clear(state: State<'_, Folders>) -> Result<(), String> {
    with_places(&state, |p| {
        p.clear();
        Ok(())
    })
    .await
}

/// Debug builds' settings folder (`SATURNUS_SETTINGS_DIR`, or under the
/// self-test hook the temp directory, as the ROM library's), so tests
/// never touch the user's.
fn debug_settings_dir() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("SATURNUS_SETTINGS_DIR") {
        return Some(PathBuf::from(dir));
    } else if std::env::var_os("SATURNUS_SELFTEST").is_some() {
        return Some(std::env::temp_dir().join("saturnus-selftest-settings"));
    }
    None
}

/// The places of this run: the home folder and `folders.json` in the
/// app's config folder.
pub fn load_places(app: &AppHandle) -> Result<places::Places, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| format!("no home folder: {e}"))?;
    let file = match debug_settings_dir() {
        Some(dir) => Some(dir),
        None => app.path().app_config_dir().ok(),
    }
    .map(|d| d.join(places::FOLDERS_FILE));
    Ok(places::Places::load(file, &home))
}

/// Save the request's bytes where the user says, offering the `x-name`
/// header's name. Resolves to whether they were saved.
#[tauri::command]
pub async fn save_file(app: AppHandle, request: Request<'_>) -> Result<bool, String> {
    let name = crate::offered_name(&header(&request, "x-name")?, "download");
    let bytes = raw_body(&request, "save_file")?;
    blocking(move || {
        let Some(chosen) = app
            .dialog()
            .file()
            .set_title("Save")
            .set_file_name(&name)
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = chosen
            .into_path()
            .map_err(|e| format!("that place cannot be written: {e}"))?;
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(true)
    })
    .await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn headers_are_percent_decoded() {
        assert_eq!(
            percent_decode("%5B%22Users%22%2C%22%C3%A4%22%5D").unwrap(),
            "[\"Users\",\"ä\"]"
        );
        assert_eq!(percent_decode("plain").unwrap(), "plain");
        assert!(percent_decode("%4").is_none());
        assert!(percent_decode("%zz").is_none());
        assert!(percent_decode("%ff").is_none(), "not UTF-8");
        assert!(percent_decode("%+1").is_none(), "a sign is not a hex digit");
        assert!(percent_decode("%-1").is_none());
        assert!(percent_decode("%4a%4A").as_deref() == Some("JJ"));
    }
}
