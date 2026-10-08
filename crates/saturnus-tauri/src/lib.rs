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
//!
//! The ROM slots (`romSlots`, `bootModel`, `chooseRom`, `forgetRom`,
//! `romSettings`) are answered here, over the remembered files of
//! [`roms`], each in its turn of the page's order; a boot they lead to
//! goes to the machine thread as a `boot` with the remembered file.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod order;
pub mod roms;

/// The machine thread, shared with the CLI's control API
/// (`saturnus_drive::runner`).
pub mod runner {
    pub use saturnus_drive::runner::*;
}

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use order::{Sequencer, Slot};
use roms::Library;
use runner::{Request, Sink};
use serde_json::{Value, json};
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
    order: Mutex<Sequencer<Due>>,
}

/// What the page's order releases: a request for the machine thread, or
/// a ROM command's turn (it is told on its channel).
#[derive(Debug)]
enum Due {
    Request(Request),
    Turn(Sender<()>),
}

/// The answer of a request sent to the machine thread.
fn answer(rx: &Receiver<Result<Value, String>>) -> Result<Value, String> {
    // Dropped unanswered: the page was reloaded while this waited behind
    // an older command, or the machine thread stopped.
    rx.recv()
        .map_err(|_| "command dropped (the page was reloaded or the machine stopped)".to_string())?
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
async fn command(
    app: AppHandle,
    machine: State<'_, Machine>,
    roms: State<'_, Roms>,
    msg: Value,
) -> Result<Value, String> {
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
    let refused = ["romPath", "path"]
        .iter()
        .find(|f| msg.get(**f).is_some())
        .map(|f| {
            format!("{f:?} is not accepted: a message never names a file, the host chooses them")
        });
    if refused.is_none() && ROM_COMMANDS.contains(&cmd) {
        return rom_command(app, &machine, &roms, msg, &session, seq).await;
    }
    let file = if let Some(e) = refused {
        Err(e)
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
    let (slot, pending) = match file {
        // Cancelled, or refused: the turn passes without a command.
        Ok(None) => (Slot::Skip, None),
        Err(e) => {
            admit(&machine, &session, seq, Slot::Skip)?;
            return Err(e);
        }
        Ok(Some(file)) => {
            let (reply, rx) = channel();
            let req = Request {
                msg,
                file,
                reply: Some(reply),
                ticket: None,
            };
            (Slot::Send(Due::Request(req)), Some(rx))
        }
    };
    admit(&machine, &session, seq, slot)?;
    let Some(rx) = pending else {
        return Ok(Value::Null);
    };
    tauri::async_runtime::spawn_blocking(move || answer(&rx))
        .await
        .map_err(|e| e.to_string())?
}

/// The remembered ROMs ([`roms`]), shared with the blocking tasks that
/// open dialogs and read files.
struct Roms(Arc<Mutex<Library>>);

/// The commands of the ROM slots, answered by this host over [`roms`].
const ROM_COMMANDS: [&str; 5] = [
    "romSlots",
    "bootModel",
    "chooseRom",
    "forgetRom",
    "romSettings",
];

fn lock(lib: &Mutex<Library>) -> Result<std::sync::MutexGuard<'_, Library>, String> {
    lib.lock()
        .map_err(|_| "the remembered ROMs are broken".to_string())
}

/// Ask for `model`'s ROM in a dialog that opens where the remembered ROMs
/// are; `None` if cancelled.
fn ask_for_rom(
    app: &AppHandle,
    model: saturnus::Model,
    folder: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let title = format!("Choose the {} ROM", model.name().to_uppercase());
    if let Some(p) = selftest_file(Need::Rom) {
        println!("selftest: dialog \"{title}\" answered by the hook");
        return Ok(Some(p));
    }
    let mut dialog = app.dialog().file().set_title(title);
    if let Some(dir) = folder {
        dialog = dialog.set_directory(dir);
    }
    chosen(dialog.blocking_pick_file())
}

/// The blocking part of a ROM command (dialogs, reading files); `None`
/// when a dialog was cancelled.
fn rom_work(
    app: &AppHandle,
    lib: &Mutex<Library>,
    msg: &Value,
) -> Result<Option<roms::Step>, String> {
    let none = roms::Step::default;
    match msg.get("cmd").and_then(Value::as_str).unwrap_or_default() {
        "romSettings" => {
            let on = msg
                .get("bootLast")
                .and_then(Value::as_bool)
                .ok_or("missing \"bootLast\"")?;
            lock(lib)?.set_boot_last(on);
            Ok(Some(none()))
        }
        "forgetRom" => {
            let model = match msg.get("model") {
                None | Some(Value::Null) => None,
                Some(_) => Some(roms::model_field(msg)?),
            };
            lock(lib)?.forget(model);
            Ok(Some(none()))
        }
        "bootModel" => {
            let model = roms::model_field(msg)?;
            let path = lock(lib)?.boot_file(model)?;
            Ok(Some(roms::Step {
                boot: Some((model, path)),
                notice: String::new(),
            }))
        }
        "chooseRom" => {
            let model = roms::model_field(msg)?;
            if let Some(id) = msg.get("offer") {
                let id = id.as_u64().ok_or("\"offer\" must be an offer's id")?;
                return lock(lib)?.take_offer(model, id).map(Some);
            }
            let folder = lock(lib)?.folder(model);
            let Some(path) = ask_for_rom(app, model, folder)? else {
                return Ok(None);
            };
            lock(lib)?.choose(model, &path).map(Some)
        }
        _ => Ok(Some(none())),
    }
}

/// A ROM command, in its turn of the page's order: the earlier commands
/// have gone to the machine thread and the later ones wait while it reads
/// and changes the library, opens its dialog and boots, so two ROM
/// commands never interleave. Resolves to the slots (`romSlots`'s result)
/// with `booted` (the boot's result or `null`) and `notice`, or to `null`
/// when the dialog was cancelled.
async fn rom_command(
    app: AppHandle,
    machine: &Machine,
    roms: &Roms,
    msg: Value,
    session: &str,
    seq: u64,
) -> Result<Value, String> {
    let (turn, my_turn) = channel();
    admit(machine, session, seq, Slot::Turn(Due::Turn(turn)))?;
    let lib = Arc::clone(&roms.0);
    let owned = session.to_string();
    let result = tauri::async_runtime::spawn_blocking(move || {
        // Dropped before its turn (the page was reloaded): the session is
        // gone and there is nothing to admit.
        my_turn
            .recv()
            .map_err(|_| "command dropped (the page was reloaded)".to_string())?;
        let machine = app.state::<Machine>();
        let turn = (&*machine, owned.as_str(), seq);
        Ok::<_, String>(rom_turn(&app, &lib, turn, &msg))
    })
    .await
    .unwrap_or_else(|e| Ok(Err(e.to_string())))?;
    // The turn ends whatever the work came to, so the later ones go on.
    admit(machine, session, seq, Slot::Skip)?;
    result
}

/// A ROM command's work once its turn has come (blocks: dialogs, files,
/// the boot's answer); `null` when the dialog was cancelled.
fn rom_turn(
    app: &AppHandle,
    lib: &Mutex<Library>,
    (machine, session, seq): (&Machine, &str, u64),
    msg: &Value,
) -> Result<Value, String> {
    let Some(step) = rom_work(app, lib, msg)? else {
        return Ok(Value::Null);
    };
    let booted = match step.boot {
        None => Ok(Value::Null),
        Some((model, path)) => {
            let (reply, rx) = channel();
            let boot = json!({
                "v": msg.get("v").cloned().unwrap_or(Value::Null),
                "cmd": "boot",
                "model": model.name(),
            });
            let req = Request {
                msg: boot,
                file: Some(path),
                reply: Some(reply),
                ticket: None,
            };
            send_in_turn(machine, session, seq, req)?;
            let r = answer(&rx);
            if r.is_ok() {
                lock(lib)?.booted(model);
            }
            r
        }
    };
    let cmd = msg.get("cmd").and_then(Value::as_str).unwrap_or_default();
    let slots = lock(lib)?.slots();
    rom_result(cmd, slots, booted, &step.notice)
}

/// A ROM command's result: the slots with `booted`, `notice` and
/// `bootError`. A `chooseRom` has already remembered its files and offers
/// when its boot fails, so the failure is told as `bootError` beside the
/// slots and the notice; a failed `bootModel` changed nothing and is the
/// command's error.
fn rom_result(
    cmd: &str,
    mut slots: Value,
    booted: Result<Value, String>,
    notice: &str,
) -> Result<Value, String> {
    let (booted, error) = match booted {
        Ok(b) => (b, Value::Null),
        Err(e) if cmd == "chooseRom" => (Value::Null, json!(e)),
        Err(e) => return Err(e),
    };
    slots["booted"] = booted;
    slots["notice"] = json!(notice);
    slots["bootError"] = error;
    Ok(slots)
}

/// The settings file of the remembered ROMs: `settings.json` in the
/// platform's config directory for the app (Tauri's `app_config_dir`).
/// Debug builds take another directory from `SATURNUS_SETTINGS_DIR`, or
/// under the self-test hook the temp directory, so self-tests never touch
/// the user's settings.
fn settings_file(app: &tauri::App) -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("SATURNUS_SETTINGS_DIR") {
        return Some(PathBuf::from(dir).join(roms::SETTINGS_FILE));
    } else if std::env::var_os("SATURNUS_SELFTEST").is_some() {
        return Some(std::env::temp_dir().join("saturnus-selftest-settings.json"));
    }
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join(roms::SETTINGS_FILE))
}

/// Pass `slot` through the sequencer and send what is due, in order,
/// under the lock (so two admissions cannot interleave their sends).
fn admit(machine: &Machine, session: &str, seq: u64, slot: Slot<Due>) -> Result<(), String> {
    let mut order = machine
        .order
        .lock()
        .map_err(|_| "the command order is broken".to_string())?;
    for due in order.admit(session, seq, slot)? {
        match due {
            Due::Request(req) => machine
                .tx
                .send(req)
                .map_err(|_| "the machine thread has stopped".to_string())?,
            // The turn's owner may have given up (a dropped task); the
            // number is then admitted again by nobody, as after a reload.
            Due::Turn(turn) => {
                let _ = turn.send(());
            }
        }
    }
    Ok(())
}

/// Send a ROM command's boot during its turn: the earlier commands are on
/// the channel already and the later ones wait, so it is in order. Checked
/// and sent under the order lock, so a page reloaded while the command sat
/// in its dialog never gets a boot it did not ask for.
fn send_in_turn(machine: &Machine, session: &str, seq: u64, req: Request) -> Result<(), String> {
    let order = machine
        .order
        .lock()
        .map_err(|_| "the command order is broken".to_string())?;
    if !order.holds(session, seq) {
        return Err("command dropped (the page was reloaded)".to_string());
    }
    machine
        .tx
        .send(req)
        .map_err(|_| "the machine thread has stopped".to_string())
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
    // `SATURNUS_SELFTEST_SCRIPT=roms` runs the remembered-ROM checks
    // instead, in the phase `SATURNUS_SELFTEST_PHASE` names.
    let script = if std::env::var("SATURNUS_SELFTEST_SCRIPT").as_deref() == Ok("roms") {
        let phase = std::env::var("SATURNUS_SELFTEST_PHASE").unwrap_or_default();
        let phase: String = phase.chars().filter(char::is_ascii_alphanumeric).collect();
        include_str!("selftest-roms.js").replace("__PHASE__", &phase)
    } else {
        include_str!("selftest.js").replace("__SECS__", &secs.to_string())
    };
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
            let lib = Library::open(settings_file(app));
            app.manage(Roms(Arc::new(Mutex::new(lib))));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A ROM command's boot is sent only while its turn holds: after a
    /// reload (a new session admitted) it is refused and never reaches the
    /// machine thread.
    #[test]
    fn a_boot_after_a_reload_is_not_sent() {
        let (tx, rx) = channel();
        let machine = Machine {
            tx,
            order: Mutex::new(Sequencer::default()),
        };
        let boot = || Request {
            msg: json!({"cmd": "boot", "model": "48sx"}),
            file: None,
            reply: None,
            ticket: None,
        };
        let (turn, my_turn) = channel();
        admit(&machine, "old", 0, Slot::Turn(Due::Turn(turn))).unwrap();
        my_turn.recv().unwrap();
        // The page reloads while the old command sits in its dialog.
        admit(&machine, "new", 0, Slot::Skip).unwrap();
        let e = send_in_turn(&machine, "old", 0, boot()).unwrap_err();
        assert!(e.contains("reloaded"), "{e}");
        assert!(rx.try_recv().is_err(), "no boot reached the machine");
        assert!(admit(&machine, "old", 0, Slot::Skip).is_err(), "stale");
        // A turn that holds sends it.
        let (turn, my_turn) = channel();
        admit(&machine, "new", 1, Slot::Turn(Due::Turn(turn))).unwrap();
        my_turn.recv().unwrap();
        send_in_turn(&machine, "new", 1, boot()).unwrap();
        assert_eq!(rx.try_recv().unwrap().msg["cmd"], "boot");
    }

    /// A failed boot after `chooseRom` keeps the slots and the notice and
    /// tells the error beside them; after `bootModel` it is the error.
    #[test]
    fn a_failed_boot_after_a_choice_keeps_the_notice() {
        let slots = json!({"slots": [], "offers": []});
        let r = rom_result(
            "chooseRom",
            slots.clone(),
            Err("bad ROM".into()),
            "Also found: x.",
        )
        .unwrap();
        assert_eq!(r["bootError"], "bad ROM");
        assert_eq!(r["notice"], "Also found: x.");
        assert_eq!(r["booted"], Value::Null);
        assert_eq!(r["slots"], json!([]));
        let ok = rom_result("chooseRom", slots.clone(), Ok(json!({"model": "48sx"})), "").unwrap();
        assert_eq!(
            (ok["booted"]["model"].as_str(), &ok["bootError"]),
            (Some("48sx"), &Value::Null)
        );
        assert_eq!(
            rom_result("bootModel", slots, Err("gone".into()), "").unwrap_err(),
            "gone"
        );
    }
}
