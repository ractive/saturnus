//! The saturnus desktop app: the page in `web/` as its front end, the
//! emulator core linked natively and run on its own thread ([`runner`]).
//!
//! The page talks to it through one Tauri command, `command`, which
//! carries a message of the front-end protocol (`web/protocol.md`), and
//! listens to the `saturnus` event, which carries the protocol's events.
//! Where the page's protocol leaves a file to the host (the ROM of
//! `boot`, the file of `saveState` and `loadState`), this side opens a
//! native file dialog and hands the chosen path to the machine thread
//! beside the message; the page can never name a file (a message that
//! tries is refused), and files read are size-capped.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod order;
pub mod runner;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};

use order::{Sequencer, Slot};
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

/// The machine thread's command channel, behind the page's order.
#[derive(Debug)]
struct Machine {
    tx: Sender<Request>,
    order: Mutex<Sequencer<Request>>,
}

/// The file a command needs, chosen by the host.
#[derive(Clone, Copy, Debug)]
enum Need {
    Rom,
    SaveState,
    LoadState,
}

fn need(cmd: &str) -> Option<Need> {
    match cmd {
        "boot" => Some(Need::Rom),
        "saveState" => Some(Need::SaveState),
        "loadState" => Some(Need::LoadState),
        _ => None,
    }
}

/// A path chosen in a dialog, `None` if cancelled.
fn chosen(path: Option<tauri_plugin_dialog::FilePath>) -> Result<Option<PathBuf>, String> {
    path.map(|p| p.into_path().map_err(|e| e.to_string()))
        .transpose()
}

/// Debug builds under the self-test hook name the files themselves
/// (`SATURNUS_SELFTEST` is the ROM; states go to the temp directory), as
/// a dialog cannot be scripted. Release builds have no such path.
#[cfg(debug_assertions)]
fn selftest_file(need: Need) -> Option<PathBuf> {
    let rom = std::env::var_os("SATURNUS_SELFTEST")?;
    Some(match need {
        Need::Rom => PathBuf::from(rom),
        Need::SaveState | Need::LoadState => std::env::temp_dir().join("saturnus-selftest.state"),
    })
}

#[cfg(not(debug_assertions))]
fn selftest_file(_: Need) -> Option<PathBuf> {
    None
}

/// Ask the user for the file (blocks: never on the main thread); `None`
/// if cancelled. The page has no say in the path.
fn ask_for_file(app: &AppHandle, need: Need) -> Result<Option<PathBuf>, String> {
    if let Some(p) = selftest_file(need) {
        return Ok(Some(p));
    }
    let dialog = app.dialog().file();
    chosen(match need {
        Need::Rom => dialog.set_title("Choose a ROM image").blocking_pick_file(),
        Need::SaveState => dialog
            .set_title("Save the calculator's state")
            .set_file_name("saturnus.state")
            .add_filter("saturnus state", &["state"])
            .blocking_save_file(),
        Need::LoadState => dialog
            .set_title("Load a saved state")
            .add_filter("saturnus state", &["state"])
            .blocking_pick_file(),
    })
}

/// One protocol command; resolves to its result (`null` when a dialog was
/// cancelled). The message carries the page's `session` and `seq`, so
/// commands reach the machine thread in the order the page sent them
/// ([`order`]); a command that needs a file holds the later ones back
/// while its dialog is open.
#[tauri::command]
async fn command(app: AppHandle, machine: State<'_, Machine>, msg: Value) -> Result<Value, String> {
    let seq = msg
        .get("seq")
        .and_then(Value::as_u64)
        .ok_or("missing \"seq\"")?;
    let session = msg
        .get("session")
        .and_then(Value::as_str)
        .ok_or("missing \"session\"")?
        .to_string();
    let cmd = msg.get("cmd").and_then(Value::as_str).unwrap_or_default();
    // Refused here too, before any dialog opens (the runner refuses them).
    let file = if let Some(f) = ["romPath", "path"].iter().find(|f| msg.get(**f).is_some()) {
        Err(format!(
            "{f:?} is not accepted: this host chooses files in its own dialogs"
        ))
    } else if let Some(n) = need(cmd) {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || ask_for_file(&app, n))
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r)
            .map(|p| p.map(Some))
    } else {
        Ok(Some(None))
    };
    let (slot, answer) = match file {
        // Cancelled, or refused: the turn passes without a command.
        Ok(None) => (Slot::Skip, None),
        Err(e) => {
            admit(&machine, &session, seq, Slot::Skip)?;
            return Err(e);
        }
        Ok(Some(file)) => {
            let (reply, answer) = channel();
            let req = Request {
                msg,
                file,
                reply: Some(reply),
            };
            (Slot::Send(req), Some(answer))
        }
    };
    admit(&machine, &session, seq, slot)?;
    let Some(answer) = answer else {
        return Ok(Value::Null);
    };
    tauri::async_runtime::spawn_blocking(move || {
        answer
            .recv()
            .map_err(|_| "the machine thread has stopped".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Pass `slot` through the sequencer and send what is due, in order,
/// under the lock (so two admissions cannot interleave their sends).
fn admit(machine: &Machine, session: &str, seq: u64, slot: Slot<Request>) -> Result<(), String> {
    let mut order = machine
        .order
        .lock()
        .map_err(|_| "the command order is broken".to_string())?;
    for req in order.admit(session, seq, slot)? {
        machine
            .tx
            .send(req)
            .map_err(|_| "the machine thread has stopped".to_string())?;
    }
    Ok(())
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
    // The ROM is read from the variable by `selftest_file`, not here.
    if std::env::var_os("SATURNUS_SELFTEST").is_none() {
        return;
    }
    let secs: u64 = std::env::var("SATURNUS_SELFTEST_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let script = include_str!("selftest.js").replace("__SECS__", &secs.to_string());
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
            app.manage(Machine {
                tx,
                order: Mutex::new(Sequencer::default()),
            });
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
