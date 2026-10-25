//! The desktop app's side of AI access (iteration 58): the setting "Let
//! AI apps on this computer use the calculators" (off by default, kept in
//! `ai-access.json` in the app's config folder), the MCP endpoint it
//! starts and stops ([`crate::mcp`]), and the bridge to the window that
//! runs the tools (`web/ai-local.js`).
//!
//! The bridge: the endpoint's `tools/list` and `tools/call` go to the
//! window as the event `mcp-call` (`{id, method, params, client}`); the
//! window answers with the command `mcp_reply(id, result, error)`. A call
//! not answered within its time limit (the window says each tool's in
//! `mcp_attach`), or whose AI app left, is withdrawn: the event
//! `mcp-cancel` (`{id}`) tells the window, which closes its confirmation.
//! Until the window has attached (`mcp_attach`, once its tools are ready),
//! and from the moment it starts loading again, calls are answered "the
//! app is not ready". Turning the setting off stops the endpoint at once
//! and ends the calls in progress.
//!
//! `mcp_status` and `mcp_set` give the page the state, and while the
//! endpoint runs its address and the `Authorization` header the AI apps
//! need (the control API's token): the page puts it into the install links
//! and commands, the AI app keeps it in its config (owner, 2026-10-23,
//! option b). It is never logged, never in an event. The event
//! `mcp-status` (no payload) says the state changed: an AI app was seen
//! for the first time in a while, or the endpoint started or stopped.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use saturnus_drive::loopback::{self, token};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, State};

use crate::mcp::{self, CLIENT_CHECK, CallError, Client, Config, Window};

/// The settings file, in the app's config folder.
pub const SETTINGS_FILE: &str = "ai-access.json";
/// The event a call goes to the window on.
pub const CALL_EVENT: &str = "mcp-call";
/// The event that withdraws a call.
pub const CANCEL_EVENT: &str = "mcp-cancel";
/// The event that says the state changed.
pub const STATUS_EVENT: &str = "mcp-status";
/// The window the tools run in.
const WINDOW: &str = "main";
/// Time limit of `tools/list`.
const LIST_LIMIT: Duration = Duration::from_secs(15);
/// Time limit of a tool the window named no limit for.
const DEFAULT_LIMIT: Duration = Duration::from_secs(120);
/// The bounds of a tool's time limit.
const MIN_LIMIT: Duration = Duration::from_secs(1);
const MAX_LIMIT: Duration = Duration::from_secs(600);
/// An AI app counts as connected for this long after its last request.
pub const APP_FRESH: Duration = Duration::from_secs(10 * 60);
/// Longest instructions the window may set.
const MAX_INSTRUCTIONS: usize = 64 * 1024;
/// What the calls in progress are told when the setting goes off.
pub const TURNED_OFF: &str = "AI access was turned off in the saturnus app.";
/// What they are told when the endpoint moves to another port.
pub const MOVED: &str = "AI access moved to another port in the saturnus app.";
/// Lowest port the setting takes (below are the system's).
const MIN_PORT: u16 = 1024;

/// The setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Whether the endpoint runs.
    pub enabled: bool,
    /// Its port.
    pub port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: mcp::DEFAULT_PORT,
        }
    }
}

/// What the window said when it attached.
#[derive(Debug)]
struct Ready {
    instructions: String,
    /// Each tool's time limit.
    limits: HashMap<String, Duration>,
}

/// The calls waiting for the window and what it said when it attached.
#[derive(Debug, Default)]
struct Calls {
    ready: Option<Ready>,
    pending: HashMap<u64, Sender<Result<Value, String>>>,
    next: u64,
}

/// Where the bridge sends its events: the app's window (a fake in tests).
pub trait Events: Send + Sync + 'static {
    /// Send `payload` on `event` to the tools' window; `false` if it is not
    /// there.
    fn emit(&self, event: &str, payload: Value) -> bool;
}

/// The app's window.
#[derive(Debug)]
struct AppEvents(AppHandle);

impl Events for AppEvents {
    fn emit(&self, event: &str, payload: Value) -> bool {
        self.0.emit_to(WINDOW, event, payload).is_ok()
    }
}

/// The bridge between the endpoint and the window.
pub struct Bridge {
    events: Box<dyn Events>,
    calls: Mutex<Calls>,
    /// The AI apps seen, with when.
    apps: Mutex<Vec<(String, Instant)>>,
}

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge").finish_non_exhaustive()
    }
}

impl Bridge {
    /// A bridge sending its events to `events`.
    pub fn new(events: Box<dyn Events>) -> Self {
        Self {
            events,
            calls: Mutex::new(Calls::default()),
            apps: Mutex::new(Vec::new()),
        }
    }

    /// The window is ready: its instructions and each tool's time limit
    /// in ms.
    pub fn attach(&self, instructions: String, limits: &HashMap<String, u64>) {
        let mut instructions = instructions;
        if instructions.len() > MAX_INSTRUCTIONS {
            let mut end = MAX_INSTRUCTIONS;
            while !instructions.is_char_boundary(end) {
                end -= 1;
            }
            instructions.truncate(end);
        }
        let limits = limits
            .iter()
            .map(|(k, ms)| {
                (
                    k.clone(),
                    Duration::from_millis(*ms).clamp(MIN_LIMIT, MAX_LIMIT),
                )
            })
            .collect();
        if let Ok(mut c) = self.calls.lock() {
            c.ready = Some(Ready {
                instructions,
                limits,
            });
        }
    }

    /// The window is loading again or gone: the calls waiting end ("not
    /// ready").
    pub fn detach(&self) {
        if let Ok(mut c) = self.calls.lock() {
            c.ready = None;
            // Dropping the senders ends each wait.
            c.pending.clear();
        }
    }

    /// The window's answer to call `id`.
    pub fn reply(&self, id: u64, answer: Result<Value, String>) {
        let tx = self
            .calls
            .lock()
            .ok()
            .and_then(|mut c| c.pending.remove(&id));
        if let Some(tx) = tx {
            let _ = tx.send(answer);
        }
    }

    /// End every call waiting with `why` (the setting was turned off).
    fn end_all(&self, why: &str) {
        let ended: Vec<(u64, Sender<Result<Value, String>>)> = match self.calls.lock() {
            Ok(mut c) => c.pending.drain().collect(),
            Err(_) => return,
        };
        for (id, tx) in ended {
            let _ = tx.send(Err(why.to_string()));
            self.events.emit(CANCEL_EVENT, json!({"id": id}));
        }
    }

    /// Withdraw call `id`: the window stops waiting for the user.
    fn withdraw(&self, id: u64) {
        if let Ok(mut c) = self.calls.lock() {
            c.pending.remove(&id);
        }
        self.events.emit(CANCEL_EVENT, json!({"id": id}));
    }

    /// The AI apps seen in the last [`APP_FRESH`], newest first, with
    /// seconds since.
    fn fresh_apps(&self) -> Vec<Value> {
        let Ok(apps) = self.apps.lock() else {
            return Vec::new();
        };
        let mut fresh: Vec<&(String, Instant)> = apps
            .iter()
            .filter(|(_, t)| t.elapsed() < APP_FRESH)
            .collect();
        fresh.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
        fresh
            .iter()
            .map(|(n, t)| json!({"name": n, "secondsAgo": t.elapsed().as_secs()}))
            .collect()
    }

    /// Forget the AI apps (the endpoint stopped).
    fn forget_apps(&self) {
        if let Ok(mut a) = self.apps.lock() {
            a.clear();
        }
    }
}

impl Window for Bridge {
    fn instructions(&self) -> Option<String> {
        let c = self.calls.lock().ok()?;
        c.ready.as_ref().map(|r| r.instructions.clone())
    }

    fn call(
        &self,
        method: &str,
        params: Value,
        client: &Client,
        gone: &dyn Fn() -> bool,
    ) -> Result<Value, CallError> {
        // The caller left, or the server was turned off, before the call
        // got here: nothing goes to the window.
        if gone() {
            return Err(CallError::Gone);
        }
        let (id, rx, limit) = {
            let mut c = self.calls.lock().map_err(|_| CallError::NotReady)?;
            let ready = c.ready.as_ref().ok_or(CallError::NotReady)?;
            let limit = if method == "tools/list" {
                LIST_LIMIT
            } else {
                params
                    .get("name")
                    .and_then(Value::as_str)
                    .and_then(|n| ready.limits.get(n).copied())
                    .unwrap_or(DEFAULT_LIMIT)
            };
            c.next += 1;
            let id = c.next;
            let (tx, rx) = channel();
            c.pending.insert(id, tx);
            (id, rx, limit)
        };
        let sent = self.events.emit(
            CALL_EVENT,
            json!({"id": id, "method": method, "params": params, "client": client.name, "clientStable": client.stable}),
        );
        if !sent {
            self.withdraw(id);
            return Err(CallError::NotReady);
        }
        let deadline = Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left.min(CLIENT_CHECK)) {
                Ok(answer) => return answer.map_err(CallError::Failed),
                // Dropped by `detach`: the window is loading again.
                Err(RecvTimeoutError::Disconnected) => return Err(CallError::NotReady),
                Err(RecvTimeoutError::Timeout) => {}
            }
            if left.is_zero() {
                self.withdraw(id);
                return Err(CallError::Timeout);
            }
            if gone() {
                self.withdraw(id);
                return Err(CallError::Gone);
            }
        }
    }

    fn seen(&self, client: &str) {
        let news = {
            let Ok(mut apps) = self.apps.lock() else {
                return;
            };
            let now = Instant::now();
            match apps.iter_mut().find(|(n, _)| n == client) {
                Some((_, t)) => {
                    let stale = t.elapsed() >= APP_FRESH;
                    *t = now;
                    stale
                }
                None => {
                    // A bounded list: the oldest goes.
                    if apps.len() >= 32 {
                        apps.sort_by_key(|(_, t)| *t);
                        apps.remove(0);
                    }
                    apps.push((client.to_string(), now));
                    true
                }
            }
        };
        if news {
            self.events.emit(STATUS_EVENT, Value::Null);
        }
    }
}

/// The endpoint while it runs.
#[derive(Debug)]
struct Running {
    port: u16,
    stop: Arc<AtomicBool>,
    authorization: String,
}

/// The setting, the endpoint and the bridge: the app's state.
#[derive(Debug)]
pub struct Local {
    bridge: Arc<Bridge>,
    file: Option<PathBuf>,
    /// The token file; `None`: the control API's (`token::resolve`).
    token_file: Option<PathBuf>,
    version: String,
    state: Mutex<LocalState>,
    /// Held while the endpoint is started or stopped (reading the token,
    /// binding, naming a busy port's user), so `state` is held only for
    /// a moment and the status is never kept waiting.
    applying: Mutex<()>,
}

#[derive(Debug, Default)]
struct LocalState {
    settings: Settings,
    running: Option<Running>,
    error: Option<String>,
}

impl Local {
    /// The state with the settings in `file` (none: defaults, not kept).
    pub fn new(bridge: Arc<Bridge>, file: Option<PathBuf>, version: &str) -> Self {
        let settings = file
            .as_deref()
            .and_then(|f| std::fs::read(f).ok())
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .unwrap_or_default();
        Self {
            bridge,
            file,
            token_file: None,
            version: version.to_string(),
            state: Mutex::new(LocalState {
                settings,
                ..LocalState::default()
            }),
            applying: Mutex::new(()),
        }
    }

    /// The bridge.
    pub fn bridge(&self) -> &Arc<Bridge> {
        &self.bridge
    }

    /// Start the endpoint if the setting is on (the app starts).
    pub fn start(&self) {
        self.apply();
    }

    /// Change the setting: kept, then applied (turning it off stops the
    /// endpoint at once). The new status, with the header.
    pub fn set(&self, enabled: bool, port: Option<u16>) -> Result<Value, String> {
        {
            let mut s = self.lock()?;
            let port = port.unwrap_or(s.settings.port);
            if port < MIN_PORT {
                return Err(format!("Choose a port from {MIN_PORT} to 65535."));
            }
            s.settings = Settings { enabled, port };
            if let Some(f) = &self.file {
                let saved = serde_json::to_vec_pretty(&s.settings)
                    .map_err(|e| e.to_string())
                    .and_then(|b| {
                        if let Some(dir) = f.parent() {
                            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                        }
                        std::fs::write(f, b).map_err(|e| e.to_string())
                    });
                if let Err(e) = saved {
                    eprintln!("saturnus: the AI access setting is not kept: {e}");
                }
            }
        }
        self.apply();
        let s = self.lock()?;
        Ok(status(&s, &self.bridge, true))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, LocalState>, String> {
        self.state
            .lock()
            .map_err(|_| "AI access is not available. Restart the app.".to_string())
    }

    /// The status; with the header while the endpoint runs.
    pub fn status(&self, with_header: bool) -> Value {
        match self.state.lock() {
            Ok(s) => status(&s, &self.bridge, with_header),
            Err(_) => {
                json!({"enabled": false, "running": false, "error": "AI access is not available. Restart the app."})
            }
        }
    }

    /// Make the endpoint match the setting. The slow part (the token
    /// file, the bind, naming a busy port's user) runs without `state`
    /// held; `applying` keeps two changes from interleaving.
    fn apply(&self) {
        let Ok(_one) = self.applying.lock() else {
            return;
        };
        let (want, old) = {
            let Ok(mut s) = self.state.lock() else {
                return;
            };
            let want = s.settings.enabled.then_some(s.settings.port);
            if s.running.is_some() && s.running.as_ref().map(|r| r.port) == want {
                return;
            }
            s.error = None;
            (want, s.running.take())
        };
        if let Some(r) = old {
            r.stop.store(true, Ordering::SeqCst);
            // Moved to another port, or turned off: the calls in progress
            // end, and the AI apps must be set up again for a new port, so
            // the list of apps starts afresh either way.
            self.bridge
                .end_all(if want.is_some() { MOVED } else { TURNED_OFF });
            self.bridge.forget_apps();
        }
        let launched = want.map(|port| self.launch(port));
        if let Ok(mut s) = self.state.lock() {
            match launched {
                Some(Ok(r)) => s.running = Some(r),
                Some(Err(e)) => s.error = Some(e),
                None => {}
            }
        }
        self.bridge.events.emit(STATUS_EVENT, Value::Null);
    }

    /// Start the endpoint on `port`.
    fn launch(&self, port: u16) -> Result<Running, String> {
        let path = match &self.token_file {
            Some(p) => Ok(p.clone()),
            None => token::resolve(None),
        };
        let t = path
            .and_then(|p| token::load_or_create(&p))
            .map_err(|e| format!("AI access could not start: {e}"))?;
        let authorization = t.bearer();
        let listener = std::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
            .map_err(|e| bind_message(port, &e))?;
        let stop = Arc::new(AtomicBool::new(false));
        let window: Arc<dyn Window> = self.bridge.clone();
        let cfg = Arc::new(Config::new(
            port,
            t,
            window,
            &self.version,
            Arc::clone(&stop),
        ));
        mcp::spawn(listener, cfg).map_err(|e| format!("AI access could not start: {e}"))?;
        Ok(Running {
            port,
            stop,
            authorization,
        })
    }
}

/// What a failed bind of `port` says: a busy port names its user.
fn bind_message(port: u16, e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::AddrInUse {
        let who = loopback::who_listens(port).unwrap_or_else(|| "another program".to_string());
        return format!("Port {port} is in use by {who}. Choose another port.");
    }
    format!("AI access could not use port {port}: {e}")
}

fn status(s: &LocalState, bridge: &Bridge, with_header: bool) -> Value {
    let running = s.running.as_ref();
    let mut v = json!({
        "enabled": s.settings.enabled,
        "port": s.settings.port,
        "defaultPort": mcp::DEFAULT_PORT,
        "running": running.is_some(),
        "address": running.map(|r| format!("http://127.0.0.1:{}{}", r.port, mcp::PATH)),
        "error": s.error,
        "apps": if running.is_some() { bridge.fresh_apps() } else { Vec::new() },
    });
    if with_header && let Some(r) = running {
        v["authorization"] = json!(r.authorization);
    }
    v
}

/// The window's tools are ready: its instructions and each tool's time
/// limit (ms).
#[tauri::command]
pub fn mcp_attach(local: State<'_, Local>, instructions: String, limits: HashMap<String, u64>) {
    local.bridge.attach(instructions, &limits);
}

/// The window's answer to call `id`: its result, or an error.
#[tauri::command]
pub fn mcp_reply(local: State<'_, Local>, id: u64, result: Option<Value>, error: Option<String>) {
    let answer = match (result, error) {
        (_, Some(e)) => Err(e),
        (Some(r), None) => Ok(r),
        (None, None) => Err("The app gave no answer.".to_string()),
    };
    local.bridge.reply(id, answer);
}

/// The state of AI access, with the address and header while it runs.
#[tauri::command]
pub fn mcp_status(local: State<'_, Local>) -> Value {
    local.status(true)
}

/// Turn AI access on or off, on `port` (unchanged when left out).
#[tauri::command]
pub async fn mcp_set(app: AppHandle, enabled: bool, port: Option<u16>) -> Result<Value, String> {
    // Starting reads the token file and binds: off the main thread.
    crate::transfer::blocking(move || {
        let local = tauri::Manager::state::<Local>(&app);
        local.set(enabled, port)
    })
    .await
}

/// The Claude Desktop extension's name in the embedded page (built there
/// by the crate's `build.rs` from `web/mcpb/`, iteration 59).
pub const MCPB_FILE: &str = "saturnus.mcpb";

/// "Add to Claude Desktop": the extension from the embedded page, written
/// to the app's cache folder and opened with the system, so Claude
/// Desktop shows its install dialog. It holds no token (the forwarder in
/// it reads the token file), so it is the same file for every user. The
/// file's path, for the page's note (open it by hand if Claude Desktop
/// does not).
#[tauri::command]
pub async fn mcpb_open(app: AppHandle) -> Result<String, String> {
    use tauri::Manager as _;
    let bytes = app
        .asset_resolver()
        .get(MCPB_FILE.to_string())
        .map(|a| a.bytes)
        .filter(|b| b.starts_with(b"PK\x03\x04"))
        .ok_or("This copy of saturnus came without the Claude Desktop extension. Download it from the saturnus docs (AI apps) instead.")?;
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cannot find the app's cache folder ({e})"))?;
    crate::transfer::blocking(move || {
        let path = write_mcpb(&dir, &bytes)?;
        open_with_system(&path)?;
        Ok(path.display().to_string())
    })
    .await
}

/// Write the extension's `bytes` as [`MCPB_FILE`] in `dir` (created if
/// missing), through a temporary file so a half-written one is never
/// opened; its path.
fn write_mcpb(dir: &std::path::Path, bytes: &[u8]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(MCPB_FILE);
    let part = dir.join(format!("{MCPB_FILE}.part"));
    std::fs::write(&part, bytes).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// How long `xdg-open` may take to say whether it opened the file.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const OPENER_WAIT: Duration = Duration::from_secs(10);

/// Open `path` with the system's handler for its type (Claude Desktop
/// for `.mcpb`). macOS `open` and `xdg-open` say by their exit status
/// when no app took the file. Windows: `explorer.exe` with the path as
/// one argument (std quotes it, spaces and all; no shell, so `&` or `^`
/// in a user name mean nothing); its exit status says nothing, so the
/// page's note names the path to open by hand.
fn open_with_system(path: &std::path::Path) -> Result<(), String> {
    let not_opened = || {
        format!(
            "Claude Desktop did not open the extension. Install Claude Desktop, then try again, or open {} with it.",
            path.display()
        )
    };
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg(path)
            .status()
            .map_err(|_| not_opened())?;
        if status.success() {
            Ok(())
        } else {
            Err(not_opened())
        }
    }
    #[cfg(target_os = "windows")]
    {
        let mut child = std::process::Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map_err(|_| not_opened())?;
        // Waited for on its own thread: no zombie, and the dialog is not held.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let mut child = std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|_| not_opened())?;
        let end = Instant::now() + OPENER_WAIT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) | Err(_) => return Err(not_opened()),
                Ok(None) if Instant::now() >= end => {
                    // Still running (some openers stay with the app): it
                    // started something; reaped on its own thread.
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    return Ok(());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }
}

/// The install links "Add to Cursor" and "Add to VS Code" may open
/// (web/shared/mcp-install.js), with the app each opens: the webview
/// does not hand other schemes to the system.
const INSTALL_LINKS: [(&str, &str); 3] = [
    ("vscode:mcp/install?", "VS Code"),
    ("vscode-insiders:mcp/install?", "VS Code Insiders"),
    ("cursor://anysphere.cursor-deeplink/mcp/install?", "Cursor"),
];

/// The app `url` opens: one of [`INSTALL_LINKS`]' prefixes followed by a
/// percent-encoded query (printable ASCII, no spaces or quotes, so no
/// opener reads it as more than one argument); else none.
fn install_app(url: &str) -> Option<&'static str> {
    let (prefix, app) = INSTALL_LINKS.iter().find(|(p, _)| url.starts_with(p))?;
    let query = &url[prefix.len()..];
    let plain = query.bytes().all(|b| b.is_ascii_graphic() && b != b'"');
    (!query.is_empty() && plain).then_some(*app)
}

/// "Add to Cursor", "Add to VS Code": open the page's install link `url`
/// with the system, which hands it to that app. Only [`INSTALL_LINKS`];
/// the link carries the access token, so no error names it.
#[tauri::command]
pub async fn mcp_install_open(url: String) -> Result<(), String> {
    let app = install_app(&url).ok_or("not an install link of Cursor or VS Code")?;
    crate::transfer::blocking(move || open_link(&url, app)).await
}

/// Open the install link `url` for `app` with the platform's opener:
/// macOS `open` says by its exit status when no app takes the scheme;
/// `rundll32` and `xdg-open` are waited for on their own thread.
fn open_link(url: &str, app: &str) -> Result<(), String> {
    let not_opened = || format!("{app} did not open. Install {app}, then try again.");
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg(url)
            .status()
            .map_err(|_| not_opened())?;
        if status.success() {
            Ok(())
        } else {
            Err(not_opened())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = std::process::Command::new("rundll32");
            c.arg("url.dll,FileProtocolHandler");
            c
        };
        #[cfg(not(target_os = "windows"))]
        let mut cmd = std::process::Command::new("xdg-open");
        let mut child = cmd.arg(url).spawn().map_err(|_| not_opened())?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

/// The app's state of AI access; [`start_in_background`] starts its
/// endpoint once it is managed.
pub fn setup(app: &AppHandle, file: Option<PathBuf>) -> Local {
    let bridge = Arc::new(Bridge::new(Box::new(AppEvents(app.clone()))));
    Local::new(bridge, file, &app.package_info().version.to_string())
}

/// Start the endpoint if the setting is on, off the main thread (it reads
/// the token file and binds).
pub fn start_in_background(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(local) = tauri::Manager::try_state::<Local>(&app) {
            local.start();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(name: &str) -> Client {
        Client {
            name: name.to_string(),
            stable: true,
        }
    }

    /// Records the events.
    #[derive(Default)]
    struct Recorder(Mutex<Vec<(String, Value)>>);

    impl Events for Arc<Recorder> {
        fn emit(&self, event: &str, payload: Value) -> bool {
            self.0.lock().unwrap().push((event.to_string(), payload));
            true
        }
    }

    fn bridge() -> (Arc<Bridge>, Arc<Recorder>) {
        let rec = Arc::new(Recorder::default());
        (Arc::new(Bridge::new(Box::new(Arc::clone(&rec)))), rec)
    }

    /// The id of the last `mcp-call`, waited for.
    fn last_call(rec: &Recorder) -> Value {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some((_, p)) = rec
                .0
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|(e, _)| e == CALL_EVENT)
            {
                return p.clone();
            }
            assert!(Instant::now() < end, "no call reached the window");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn calls_go_to_the_window_and_come_back() {
        let (b, rec) = bridge();
        let never = || false;
        // Not attached: not ready, nothing sent.
        assert_eq!(
            b.call("tools/list", json!({}), &client("x"), &never),
            Err(CallError::NotReady)
        );
        assert!(b.instructions().is_none());
        b.attach(
            "Use them.".into(),
            &HashMap::from([("run_rpl".to_string(), 50)]),
        );
        assert_eq!(b.instructions().as_deref(), Some("Use them."));
        let b2 = Arc::clone(&b);
        let t = std::thread::spawn(move || {
            b2.call(
                "tools/call",
                json!({"name": "read_display"}),
                &client("claude-code"),
                &|| false,
            )
        });
        let call = last_call(&rec);
        assert_eq!(call["method"], "tools/call");
        assert_eq!(call["client"], "claude-code");
        assert_eq!(call["clientStable"], true);
        b.reply(call["id"].as_u64().unwrap(), Ok(json!({"content": []})));
        assert_eq!(t.join().unwrap(), Ok(json!({"content": []})));
        // An error from the window.
        let b2 = Arc::clone(&b);
        let t =
            std::thread::spawn(move || b2.call("tools/list", json!({}), &client("x"), &|| false));
        let id = loop {
            let c = last_call(&rec);
            if c["method"] == "tools/list" {
                break c["id"].as_u64().unwrap();
            }
        };
        b.reply(id, Err("broken".into()));
        assert_eq!(t.join().unwrap(), Err(CallError::Failed("broken".into())));
    }

    #[test]
    fn time_limits_leaving_clients_and_reloads() {
        let (b, rec) = bridge();
        // run_rpl's limit is clamped to the minimum of 1 s.
        b.attach(String::new(), &HashMap::from([("run_rpl".to_string(), 10)]));
        let started = Instant::now();
        let r = b.call(
            "tools/call",
            json!({"name": "run_rpl"}),
            &client("x"),
            &|| false,
        );
        assert_eq!(r, Err(CallError::Timeout));
        assert!(started.elapsed() >= MIN_LIMIT);
        let cancelled = |rec: &Recorder| {
            rec.0
                .lock()
                .unwrap()
                .iter()
                .filter(|(e, _)| e == CANCEL_EVENT)
                .count()
        };
        assert_eq!(cancelled(&rec), 1, "the window is told");
        // The client left (or the server was turned off) before the call
        // got here: nothing goes to the window.
        let calls = |rec: &Recorder| {
            rec.0
                .lock()
                .unwrap()
                .iter()
                .filter(|(e, _)| e == CALL_EVENT)
                .count()
        };
        let before = calls(&rec);
        let r = b.call(
            "tools/call",
            json!({"name": "read_display"}),
            &client("x"),
            &|| true,
        );
        assert_eq!(r, Err(CallError::Gone));
        assert_eq!(calls(&rec), before, "no mcp-call");
        // It leaves while the call waits: withdrawn, the window told.
        let left = AtomicBool::new(false);
        let b2 = Arc::clone(&b);
        let gone_later = std::thread::scope(|sc| {
            let h = sc.spawn(|| {
                b2.call(
                    "tools/call",
                    json!({"name": "read_display"}),
                    &client("x"),
                    &|| left.load(Ordering::SeqCst),
                )
            });
            while calls(&rec) == before {
                std::thread::sleep(Duration::from_millis(5));
            }
            left.store(true, Ordering::SeqCst);
            h.join().unwrap()
        });
        assert_eq!(gone_later, Err(CallError::Gone));
        assert_eq!(cancelled(&rec), 2);
        // The window reloads while a call waits.
        let b2 = Arc::clone(&b);
        let t = std::thread::spawn(move || {
            b2.call(
                "tools/call",
                json!({"name": "read_display"}),
                &client("x"),
                &|| false,
            )
        });
        let _ = last_call(&rec);
        std::thread::sleep(Duration::from_millis(20));
        b.detach();
        assert_eq!(t.join().unwrap(), Err(CallError::NotReady));
        assert!(b.instructions().is_none());
        // A late reply for a call that ended is dropped.
        b.reply(999, Ok(json!({})));
    }

    #[test]
    fn apps_are_seen_and_named_once() {
        let (b, rec) = bridge();
        b.seen("cursor");
        b.seen("cursor");
        b.seen("claude-code");
        let status_events = rec
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|(e, _)| e == STATUS_EVENT)
            .count();
        assert_eq!(status_events, 2, "only news is told");
        let apps = b.fresh_apps();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0]["name"], "claude-code");
    }

    /// The extension is written whole into a folder made for it, and
    /// again over an older copy; no part file is left.
    #[test]
    fn the_extension_is_written_for_claude_desktop() {
        let dir = saturnus_drive::loopback::token::testing::TempDir::new("mcpb");
        let cache = dir.0.join("cache").join("ch.ractive.saturnus");
        let path = write_mcpb(&cache, b"PK\x03\x04old").unwrap();
        assert_eq!(path, cache.join(MCPB_FILE));
        let path = write_mcpb(&cache, b"PK\x03\x04new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"PK\x03\x04new");
        assert!(!cache.join(format!("{MCPB_FILE}.part")).exists());
    }

    /// Only Cursor's and VS Code's install links open, with a plain query.
    #[test]
    fn only_install_links_open() {
        assert_eq!(
            install_app("vscode:mcp/install?%7B%22name%22%7D"),
            Some("VS Code")
        );
        assert_eq!(
            install_app("vscode-insiders:mcp/install?%7B%7D"),
            Some("VS Code Insiders")
        );
        assert_eq!(
            install_app(
                "cursor://anysphere.cursor-deeplink/mcp/install?name=saturnus&config=e30%3D"
            ),
            Some("Cursor")
        );
        for url in [
            "vscode:mcp/install?",
            "vscode:mcp/install?a b",
            "vscode:mcp/install?{\"name\"}",
            "vscode:mcp/install?a\nb",
            "vscode:mcp/install?\u{e9}",
            "vscode://file/etc/passwd",
            "vscode:extension/x.y",
            "VSCODE:mcp/install?x",
            " vscode:mcp/install?x",
            "cursor://anysphere.cursor-deeplink/mcp/installx?x",
            "cursor://evil/mcp/install?x",
            "https://example.com/vscode:mcp/install?x",
            "file:///Applications/Calculator.app",
            "",
        ] {
            assert_eq!(install_app(url), None, "{url:?}");
        }
    }

    /// The page's copy of the extension (built by build.rs when Node is
    /// there) is a zip with the manifest.
    #[test]
    fn the_embedded_page_carries_the_extension() {
        let built = PathBuf::from(env!("OUT_DIR"))
            .join("app-site")
            .join(MCPB_FILE);
        let Ok(bytes) = std::fs::read(&built) else {
            eprintln!("no {MCPB_FILE} (Node missing at build time): skipped");
            return;
        };
        assert!(bytes.starts_with(b"PK\x03\x04"));
        assert!(bytes.windows(13).any(|w| w == b"manifest.json"));
    }

    /// The setting: off by default, kept, turning it on starts the
    /// endpoint with its header, off stops it and ends the calls; a busy
    /// port says who has it; a port below 1024 is refused.
    #[test]
    fn the_setting_starts_and_stops_the_endpoint() {
        let dir = saturnus_drive::loopback::token::testing::TempDir::new("mcp-app");
        let file = dir.0.join(SETTINGS_FILE);
        let (b, _) = bridge();
        let mut local = Local::new(Arc::clone(&b), Some(file.clone()), "0.0.0");
        // The token file of this test only.
        local.token_file = Some(dir.0.join("control-token"));
        let s = local.status(true);
        assert_eq!(
            (s["enabled"].as_bool(), s["running"].as_bool()),
            (Some(false), Some(false))
        );
        assert_eq!(s["port"], mcp::DEFAULT_PORT);
        assert!(s.get("authorization").is_none());
        assert!(local.set(true, Some(80)).unwrap_err().contains("1024"));

        // A free port, found by binding one and letting it go.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let on = local.set(true, Some(port)).unwrap();
        assert_eq!(on["running"], true, "{on}");
        assert_eq!(on["address"], format!("http://127.0.0.1:{port}/mcp"));
        let auth = on["authorization"].as_str().unwrap().to_string();
        assert!(auth.starts_with("Bearer ") && auth.len() == 71);
        assert!(local.status(false).get("authorization").is_none());
        let kept: Settings = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(
            kept,
            Settings {
                enabled: true,
                port
            }
        );
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());

        // A call waiting when it is turned off ends at once.
        b.attach(String::new(), &HashMap::new());
        let b2 = Arc::clone(&b);
        let t = std::thread::spawn(move || {
            b2.call("tools/call", json!({"name": "x"}), &client("x"), &|| false)
        });
        std::thread::sleep(Duration::from_millis(50));
        let off = local.set(false, None).unwrap();
        assert_eq!(off["running"], false);
        assert!(matches!(t.join().unwrap(), Err(CallError::Failed(m)) if m == TURNED_OFF));
        let end = Instant::now() + Duration::from_secs(5);
        while std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            assert!(Instant::now() < end, "still accepting");
            std::thread::sleep(Duration::from_millis(20));
        }
        // Read back on the next start.
        let again = Local::new(Arc::clone(&b), Some(file), "0.0.0");
        assert_eq!(again.status(false)["enabled"], false);

        // Moved to another port: the calls in progress are told so, the
        // old port stops, the new one serves.
        local.set(true, Some(port)).unwrap();
        let other = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let b2 = Arc::clone(&b);
        let t = std::thread::spawn(move || {
            b2.call("tools/call", json!({"name": "x"}), &client("x"), &|| false)
        });
        std::thread::sleep(Duration::from_millis(50));
        let moved = local.set(true, Some(other)).unwrap();
        assert_eq!(moved["address"], format!("http://127.0.0.1:{other}/mcp"));
        assert!(matches!(t.join().unwrap(), Err(CallError::Failed(m)) if m == MOVED));
        local.set(false, None).unwrap();

        // A busy port.
        let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let busy = held.local_addr().unwrap().port();
        let s = local.set(true, Some(busy)).unwrap();
        assert_eq!(s["running"], false);
        let e = s["error"].as_str().unwrap();
        assert!(
            e.contains(&format!("Port {busy} is in use by")) && e.ends_with("Choose another port."),
            "{e}"
        );
        local.set(false, None).unwrap();
    }

    /// While the endpoint starts or stops (the token file, the bind,
    /// naming a busy port's user), the status still answers at once.
    #[test]
    fn the_status_does_not_wait_for_a_start() {
        let (b, _) = bridge();
        let local = Arc::new(Local::new(b, None, "0.0.0"));
        let starting = local.applying.lock().unwrap();
        let l2 = Arc::clone(&local);
        let (tx, rx) = channel();
        std::thread::spawn(move || tx.send(l2.status(true)).unwrap());
        let s = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the status waited");
        assert_eq!(s["running"], false);
        drop(starting);
    }
}
