//! The saturnus desktop app: the page in `web/` as its front end, the
//! emulator core linked natively and run on its own thread ([`runner`]).
//!
//! The page talks to it through one Tauri command, `command`, which
//! carries a message of the front-end protocol (`web/protocol.md`), and
//! listens to the `saturnus` event, which carries the protocol's events.
//! Where the page's protocol leaves a file to the host (the ROM of
//! `boot`, the file of `saveState` and `loadState`), this side opens a
//! native file dialog before handing the path to the machine thread.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod runner;

use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};

use runner::{Request, Sink};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// The event the machine thread's messages go out on.
pub const EVENT: &str = "saturnus";

/// Events to the app's windows.
#[derive(Debug)]
struct WindowSink(AppHandle);

impl Sink for WindowSink {
    fn event(&self, msg: Value) {
        // Fails only while the app shuts down.
        let _ = self.0.emit(EVENT, msg);
    }
}

/// The machine thread's command channel.
#[derive(Debug)]
struct Machine(Sender<Request>);

/// A path chosen in a dialog, `None` if cancelled.
fn chosen(path: Option<tauri_plugin_dialog::FilePath>) -> Result<Option<PathBuf>, String> {
    path.map(|p| p.into_path().map_err(|e| e.to_string()))
        .transpose()
}

/// Ask for the file a command needs, if the page did not name it; `None`
/// if the user cancelled.
fn ask_for_file(app: &AppHandle, msg: &mut Value) -> Result<Option<()>, String> {
    let cmd = msg.get("cmd").and_then(Value::as_str).unwrap_or_default();
    let (field, path) = match cmd {
        "boot" if msg.get("romPath").is_none() => (
            "romPath",
            chosen(
                app.dialog()
                    .file()
                    .set_title("Choose a ROM image")
                    .blocking_pick_file(),
            )?,
        ),
        "saveState" if msg.get("path").is_none() => (
            "path",
            chosen(
                app.dialog()
                    .file()
                    .set_title("Save the calculator's state")
                    .set_file_name("saturnus.state")
                    .add_filter("saturnus state", &["state"])
                    .blocking_save_file(),
            )?,
        ),
        "loadState" if msg.get("path").is_none() => (
            "path",
            chosen(
                app.dialog()
                    .file()
                    .set_title("Load a saved state")
                    .add_filter("saturnus state", &["state"])
                    .blocking_pick_file(),
            )?,
        ),
        _ => return Ok(Some(())),
    };
    let Some(path) = path else {
        return Ok(None);
    };
    if let Some(m) = msg.as_object_mut() {
        m.insert(field.into(), Value::String(path.display().to_string()));
    }
    Ok(Some(()))
}

/// One protocol command; resolves to its result (`null` when a dialog was
/// cancelled).
#[tauri::command]
async fn command(
    app: AppHandle,
    machine: State<'_, Machine>,
    mut msg: Value,
) -> Result<Value, String> {
    let tx = machine.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Dialogs block, so never on the main thread.
        if ask_for_file(&app, &mut msg)?.is_none() {
            return Ok(Value::Null);
        }
        let (reply, answer) = channel();
        tx.send(Request {
            msg,
            reply: Some(reply),
        })
        .map_err(|_| "the machine thread has stopped".to_string())?;
        answer
            .recv()
            .map_err(|_| "the machine thread has stopped".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Test hook of debug builds: a line from `selftest.js`; `done` quits.
#[cfg(debug_assertions)]
#[tauri::command]
fn selftest_log(app: AppHandle, line: String) {
    println!("selftest: {line}");
    if line == "done" {
        app.exit(0);
    }
}

/// Debug builds with `SATURNUS_SELFTEST=<48SX ROM>`: run `selftest.js` in
/// the window once the page has loaded (`SATURNUS_SELFTEST_SECS` per
/// pacing measurement, default 30).
#[cfg(debug_assertions)]
fn selftest(app: &tauri::App) {
    let Some(rom) = std::env::var_os("SATURNUS_SELFTEST") else {
        return;
    };
    let secs: u64 = std::env::var("SATURNUS_SELFTEST_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let state = std::env::temp_dir().join("saturnus-selftest.state");
    let quote = |p: &std::path::Path| Value::String(p.display().to_string()).to_string();
    let script = include_str!("selftest.js")
        .replace("__ROM__", &quote(std::path::Path::new(&rom)))
        .replace("__STATE__", &quote(&state))
        .replace("__SECS__", &secs.to_string());
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        if let Some(w) = handle.get_webview_window("main") {
            let _ = w.eval(script);
        }
    });
}

/// Opt out of App Nap: macOS throttles the timers and CPU of an app whose
/// window is hidden or in the background, which would slow the emulated
/// calculator below real time while it computes. Idle system sleep stays
/// allowed. The activity lasts as long as the process.
#[cfg(target_os = "macos")]
fn no_app_nap() {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
    let activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
        &NSString::from_str("emulating a calculator in real time"),
    );
    std::mem::forget(activity);
}

/// Run the app.
pub fn run() {
    #[cfg(target_os = "macos")]
    no_app_nap();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let tx = runner::spawn(WindowSink(app.handle().clone()))?;
            app.manage(Machine(tx));
            #[cfg(debug_assertions)]
            selftest(app);
            Ok(())
        });
    #[cfg(debug_assertions)]
    let builder = builder.invoke_handler(tauri::generate_handler![command, selftest_log]);
    #[cfg(not(debug_assertions))]
    let builder = builder.invoke_handler(tauri::generate_handler![command]);
    let result = builder.run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("saturnus: {e}");
        std::process::exit(1);
    }
}
