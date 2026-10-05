//! The MCP tool surface over one [`Emulator`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use hptx_core::TransferMode;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::emulator::{Emulator, KeyReport, parse_model};
use saturnus_drive::session::Limits;

/// Default PNG scale of `screen`.
const DEFAULT_SCALE: u32 = 1;
/// Longest wall-clock time one tool call may keep the emulator running;
/// the emulated-time budget (`keys::MAX_BUDGET_MS`) is the first line of
/// defence, this one catches a slow host.
const CALL_WALL_LIMIT: Duration = Duration::from_secs(15 * 60);

/// The run limits for one tool call, starting now.
fn call_limits() -> Limits {
    Limits {
        abort: None,
        deadline: Some(Instant::now() + CALL_WALL_LIMIT),
    }
}

/// What the session lock guards.
#[derive(Debug, Default)]
pub struct State {
    /// The calculator, once booted.
    pub emulator: Option<Emulator>,
    /// Why the boot requested on the command line failed.
    pub startup_error: Option<String>,
}

impl State {
    fn emulator(&mut self) -> Result<&mut Emulator> {
        match (&mut self.emulator, &self.startup_error) {
            (Some(e), _) => Ok(e),
            (None, Some(err)) => bail!("no calculator: the startup boot failed ({err}); call boot"),
            (None, None) => bail!("no calculator yet: call boot with a model and a ROM path"),
        }
    }
}

/// The MCP server: one session lock around one emulator.
#[derive(Debug, Clone)]
pub struct SaturnusMcp {
    state: Arc<Mutex<State>>,
    tool_router: ToolRouter<Self>,
}

/// A calculator model.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
pub enum ModelArg {
    /// HP 48SX.
    #[serde(rename = "48sx")]
    Hp48sx,
    /// HP 48GX.
    #[serde(rename = "48gx")]
    Hp48gx,
    /// HP 49G.
    #[serde(rename = "49g")]
    Hp49g,
    /// HP 38G (no Kermit server).
    #[serde(rename = "38g")]
    Hp38g,
    /// HP 39G (no Kermit server).
    #[serde(rename = "39g")]
    Hp39g,
    /// HP 40G, the 39G ROM on 40G hardware (no Kermit server).
    #[serde(rename = "40g")]
    Hp40g,
}

impl ModelArg {
    fn name(self) -> &'static str {
        match self {
            ModelArg::Hp48sx => "48sx",
            ModelArg::Hp48gx => "48gx",
            ModelArg::Hp49g => "49g",
            ModelArg::Hp38g => "38g",
            ModelArg::Hp39g => "39g",
            ModelArg::Hp40g => "40g",
        }
    }
}

/// Arguments of `boot`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BootArgs {
    /// Calculator model.
    pub model: ModelArg,
    /// Path of the packed ROM image on the server's machine.
    pub rom_path: String,
    /// Also start the Kermit server (needed by read_stack, run_command,
    /// send_object, receive_object). Not available on the 38G, 39G or 40G.
    #[serde(default)]
    pub autostart: bool,
}

/// Arguments of `press_keys`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PressKeysArgs {
    /// Key script. One action per line: `press KEY [HOLD_MS]`, a bare key
    /// name (a press), `down KEY`, `up KEY`, `wait MS`, `wait-idle [CAP_MS]`;
    /// `#` starts a comment. A line may also list several keys separated by
    /// spaces, pressed in order. Key names: `0`-`9`, `a`-`f` (softkeys),
    /// `enter`, `plus` (`+`), `minus` (`-`), `multiply` (`*`), `divide`
    /// (`/`), `point` (`.`), `space`, `neg`, `eex`, `del`, `backspace`,
    /// `alpha`, `leftshift`, `rightshift`, `on`, `up`, `down`, `left`,
    /// `right`, `nxt`, `sto`, `eval`, `var`, `sin`, `cos`, `tan`, `sqrt`,
    /// `power`, `inv`; 48 only: `mth`, `prg`, `cst`, `quote`; 49G only:
    /// `apps`, `mode`, `tool`, `hist`, `cat`, `eqw`, `symb`, `x`. The 38G,
    /// 39G and 40G use their own labels: `plot`, `symb`, `num`, `home`,
    /// `math`, `xt` (X,T,θ), `lparen`, `rparen`, `shift`, `comma`, `neg`
    /// ((-)), `power` (x^y), `del`, `alpha` (A...Z), `sin`, `cos`, `tan`,
    /// `sqrt`, `var` and `lib` (38G); `aplet`, `views`, `vars`, `ddx`,
    /// `ln`, `log`, `square` (39G/40G).
    /// Example: `6 enter 7 * enter`.
    pub script: String,
}

/// Arguments of `type_text`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TypeTextArgs {
    /// Text to type: letters A-Z and a-z (via alpha mode; not on the 39G
    /// or 40G), digits, `.`, `+`, `-`, `*`, `/`, space (not on the 38G,
    /// 39G or 40G) and newline for ENTER. Operators act like their keys
    /// (in RPN they execute).
    pub text: String,
}

/// Screen formats.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScreenFormat {
    /// A PNG image (default).
    #[default]
    Png,
    /// 64 lines of 131 characters, `#` dark and `.` light.
    Text,
}

/// Arguments of `screen`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ScreenArgs {
    /// `png` (default) or `text`.
    #[serde(default)]
    pub format: ScreenFormat,
    /// PNG only: each LCD pixel becomes a SCALE x SCALE block, 1-8
    /// (default 1, a 131x64 image; 4 is easier to read).
    pub scale: Option<u32>,
}

/// Arguments of `read_stack`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ReadStackArgs {
    /// Return at most this many levels, from level 1 up (default: all).
    pub levels: Option<usize>,
}

/// Arguments of `run_command`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    /// RPL command line to execute on the calculator, e.g. `6 7 *`. Unicode
    /// or ASCII trigraphs such as `\->` are accepted. At most about 77
    /// encoded bytes (one Kermit packet).
    pub command: String,
}

/// Kermit transfer modes.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ModeArg {
    /// Flag -35 set: objects travel as HP binary files (`HPHP48-x` /
    /// `HPHP49-x` header); a non-binary file is stored as a string.
    Binary,
    /// Flag -35 clear: objects travel as text (`%%HP:` header), which the
    /// calculator compiles on receipt.
    Ascii,
}

impl From<ModeArg> for TransferMode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::Binary => TransferMode::Binary,
            ModeArg::Ascii => TransferMode::Ascii,
        }
    }
}

/// Arguments of `send_object`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendObjectArgs {
    /// Variable name to store under, in the current directory.
    pub name: String,
    /// The object as base64 bytes (e.g. an HP binary file). Give this or
    /// `text`.
    pub bytes_base64: Option<String>,
    /// The object as text (sent as UTF-8; HP characters as ASCII
    /// trigraphs). Give this or `bytes_base64`.
    pub text: Option<String>,
    /// Transfer mode; default `ascii` for `text`, `binary` for
    /// `bytes_base64`.
    pub mode: Option<ModeArg>,
}

/// Arguments of `receive_object`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReceiveObjectArgs {
    /// Variable name in the current directory.
    pub name: String,
    /// Transfer mode, default `ascii` (returned as text); `binary` returns
    /// base64.
    pub mode: Option<ModeArg>,
}

/// Arguments of `load_state`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PathArgs {
    /// File path on the server's machine.
    pub path: String,
}

/// Arguments of `save_state`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveStateArgs {
    /// File path on the server's machine. The session's ROM is never
    /// overwritten; an existing file that is not a saturnus state needs
    /// `overwrite`.
    pub path: String,
    /// Replace an existing file that is not a saturnus state (default false).
    pub overwrite: Option<bool>,
}

fn text(s: impl Into<String>) -> ContentBlock {
    ContentBlock::text(s)
}

/// A tool's outcome as an MCP result: errors become tool errors carrying
/// the whole message chain, never protocol errors.
fn finish(result: Result<Vec<ContentBlock>>) -> std::result::Result<CallToolResult, ErrorData> {
    Ok(match result {
        Ok(content) => CallToolResult::success(content),
        Err(e) => CallToolResult::error(vec![text(format!("{e:#}"))]),
    })
}

fn key_report(headline: &str, r: &KeyReport) -> String {
    let mut s = format!(
        "{headline}\nelapsed: {} ms emulated, {} key presses\nannunciators: {}\n",
        r.elapsed_ms, r.presses, r.annunciators
    );
    for w in &r.warnings {
        s.push_str(&format!("warning: {w}\n"));
    }
    s.push_str("screen:\n");
    s.push_str(&r.screen);
    s
}

/// The stack as the calculator shows it, highest level first.
fn stack_text(levels: &[String]) -> String {
    if levels.is_empty() {
        return "Empty Stack".into();
    }
    let mut lines: Vec<String> = levels
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{}: {v}", i + 1))
        .collect();
    lines.reverse();
    lines.join("\n")
}

/// Decode the `send_object` payload and pick the mode.
pub fn object_payload(args: &SendObjectArgs) -> Result<(Vec<u8>, TransferMode)> {
    match (&args.bytes_base64, &args.text) {
        (Some(b), None) => {
            let data = BASE64
                .decode(b.trim())
                .context("bytes_base64 is not valid base64")?;
            Ok((data, args.mode.unwrap_or(ModeArg::Binary).into()))
        }
        (None, Some(t)) => Ok((
            t.as_bytes().to_vec(),
            args.mode.unwrap_or(ModeArg::Ascii).into(),
        )),
        _ => bail!("give exactly one of bytes_base64 and text"),
    }
}

impl SaturnusMcp {
    /// A server with no calculator yet.
    pub fn new() -> Self {
        Self::with_state(State::default())
    }

    /// A server around `state` (e.g. booted from the command line).
    pub fn with_state(state: State) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
            tool_router: Self::tool_router(),
        }
    }

    /// The session lock, for a startup boot that must run before any tool.
    pub fn state(&self) -> Arc<Mutex<State>> {
        Arc::clone(&self.state)
    }

    /// Run `f` on a blocking thread under the session lock.
    async fn with_state_locked<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut State) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let mut guard = Arc::clone(&self.state).lock_owned().await;
        tokio::task::spawn_blocking(move || f(&mut guard))
            .await
            .context("the tool's worker thread failed")?
    }

    /// Run `f` on the booted emulator.
    async fn with_emulator<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Emulator) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.with_state_locked(move |s| {
            let emu = s.emulator()?;
            emu.set_limits(call_limits())?;
            f(emu)
        })
        .await
    }
}

impl Default for SaturnusMcp {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl SaturnusMcp {
    #[tool(
        description = "Build a calculator from a ROM image and boot it: run to the first prompt and \
        answer it (NO at \"Try To Recover Memory?\", then OK on the 49G; OK on the 38G, 39G and 40G), so \
        the stack (38G, 39G, 40G: HOME) shows. Replaces any running calculator. With autostart, also start the Kermit \
        server (48SX, 48GX, 49G). Returns the emulated time taken and the screen as text."
    )]
    async fn boot(
        &self,
        Parameters(args): Parameters<BootArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let model_name = args.model.name();
        let result = self
            .with_state_locked(move |state| {
                let model = parse_model(model_name)?;
                let path = PathBuf::from(&args.rom_path);
                // The old calculator stays if the new one fails to build.
                let (emu, report) = Emulator::boot(model, &path, args.autostart, call_limits())?;
                state.emulator = Some(emu);
                state.startup_error = None;
                let server = if args.autostart {
                    "; Kermit server running"
                } else {
                    ""
                };
                Ok(vec![text(key_report(
                    &format!("booted {model_name} from {}{server}", path.display()),
                    &report,
                ))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Run a key script in emulated time (each press holds the key 60 ms, then waits \
        until the calculator is idle). Refused while the Kermit server runs (call stop_server). Returns \
        the emulated milliseconds taken, the annunciators and the screen as text (64 lines of 131 \
        '#'/'.'). Keys the model lacks are refused before anything runs."
    )]
    async fn press_keys(
        &self,
        Parameters(args): Parameters<PressKeysArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| {
                let report = emu.press_keys(&args.script)?;
                Ok(vec![text(key_report("keys done", &report))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Type text on the calculator's keyboard, model-aware: letters through alpha \
        mode (48SX/48GX/49G only), digits, '.', '+', '-', '*', '/', space and newline (ENTER). Other \
        characters (quotes, brackets, '=', ...) are refused: use press_keys with shifted keys, or \
        run_command. Operators act like their keys (in RPN they execute at once). Refused while the \
        Kermit server runs. Returns like press_keys."
    )]
    async fn type_text(
        &self,
        Parameters(args): Parameters<TypeTextArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| {
                let report = emu.type_text(&args.text)?;
                Ok(vec![text(key_report("typed", &report))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "The 131x64 LCD: a PNG image (default; scale 1-8 enlarges it) or text (64 lines \
        of 131 characters, '#' dark, '.' light), plus the lit annunciators. Does not run the calculator."
    )]
    async fn screen(
        &self,
        Parameters(args): Parameters<ScreenArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| match args.format {
                ScreenFormat::Png => {
                    let (png, ann) = emu.screen_png(args.scale.unwrap_or(DEFAULT_SCALE))?;
                    Ok(vec![
                        ContentBlock::image(BASE64.encode(png), "image/png"),
                        text(format!("annunciators: {ann}")),
                    ])
                }
                ScreenFormat::Text => {
                    if args.scale.is_some() {
                        bail!("scale applies to the png format only");
                    }
                    let (screen, ann) = emu.screen_text()?;
                    Ok(vec![text(format!("annunciators: {ann}\n{screen}"))])
                }
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Read the stack over Kermit (needs the server: boot with autostart or call \
        start_server). Returns the levels as display text, highest first (\"2: ...\", \"1: ...\"), or \
        \"Empty Stack\"; the stack is not changed."
    )]
    async fn read_stack(
        &self,
        Parameters(args): Parameters<ReadStackArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| {
                let levels = emu.read_stack(args.levels)?;
                Ok(vec![text(stack_text(&levels))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Execute an RPL command line on the calculator over Kermit (needs the server) and \
        return the whole stack afterwards, highest level first. Results stay on the stack. A \
        calculator error (e.g. \"Too Few Arguments\") is a tool error that includes the stack."
    )]
    async fn run_command(
        &self,
        Parameters(args): Parameters<RunCommandArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| {
                let reply = emu.run_command(&args.command)?;
                let stack = stack_text(&reply.levels);
                match reply.error {
                    Some(e) => bail!("calculator error: {e}\nstack:\n{stack}"),
                    None => Ok(vec![text(stack)]),
                }
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Store an object as a variable in the calculator's current directory over Kermit \
        (needs the server). Give text (default mode ascii: the calculator compiles it, e.g. \
        \"\\<< 1 2 + \\>>\") or bytes_base64 (default mode binary: an HP binary object file, or any \
        bytes, which become a string). Returns the name the calculator stored it under (it adds a \
        suffix such as .1 when the name exists)."
    )]
    async fn send_object(
        &self,
        Parameters(args): Parameters<SendObjectArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| {
                let (data, mode) = object_payload(&args)?;
                let stored = emu.send_object(&args.name, &data, mode)?;
                Ok(vec![text(format!("stored as {stored}"))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Fetch a variable from the calculator's current directory over Kermit (needs the \
        server). Mode ascii (default) returns the object as text with its %%HP: header, HP characters \
        as ASCII trigraphs; binary returns the HP binary file as base64."
    )]
    async fn receive_object(
        &self,
        Parameters(args): Parameters<ReceiveObjectArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| {
                let mode = args.mode.unwrap_or(ModeArg::Ascii);
                let data = emu.receive_object(&args.name, mode.into())?;
                Ok(vec![text(match mode {
                    ModeArg::Ascii => hptx_core::charset::to_trigraphs(&data),
                    ModeArg::Binary => BASE64.encode(&data),
                })])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Start the calculator's Kermit server: types ALPHA ALPHA S E R V E R ENTER and \
        waits for the server's first NAK. The stack must be showing with an empty command line. Not on \
        the 38G, 39G or 40G. While it runs, the keyboard tools are refused."
    )]
    async fn start_server(&self) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(|emu| {
                let report = emu.start_server()?;
                Ok(vec![text(key_report("Kermit server running", &report))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "End the Kermit server (Kermit FINISH) and wait until the calculator shows the \
        stack again, so the keyboard tools work."
    )]
    async fn stop_server(&self) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(|emu| {
                let (acknowledged, report) = emu.stop_server()?;
                let headline = if acknowledged {
                    "Kermit server stopped"
                } else {
                    "Kermit server stopped (FINISH was not acknowledged; the calculator may have left \
                     server mode already)"
                };
                Ok(vec![text(key_report(headline, &report))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Save the whole machine state (RAM, CPU, I/O) to a file on the server's machine. \
        Load it later with load_state on the same model and ROM. Save with the Kermit server stopped. \
        Never overwrites the session's ROM; an existing file that is not a saturnus state needs \
        overwrite: true."
    )]
    async fn save_state(
        &self,
        Parameters(args): Parameters<SaveStateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| {
                let path = PathBuf::from(&args.path);
                let n = emu.save_state(&path, args.overwrite.unwrap_or(false))?;
                let note = if emu.server_running() {
                    " (the Kermit server was running; after load_state it counts as stopped)"
                } else {
                    ""
                };
                Ok(vec![text(format!(
                    "saved {n} bytes to {}{note}",
                    path.display()
                ))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Restore a machine state saved by save_state (same model and ROM as the running \
        calculator). The Kermit server counts as stopped afterwards."
    )]
    async fn load_state(
        &self,
        Parameters(args): Parameters<PathArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(move |emu| {
                let path = PathBuf::from(&args.path);
                emu.load_state(&path)?;
                Ok(vec![text(format!("loaded {}", path.display()))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Hardware reset of the calculator (RAM is kept, like the reset hole), then run \
        until idle. The ROM's first screen (usually \"Try To Recover Memory?\") is left to answer with \
        press_keys. The Kermit server counts as stopped."
    )]
    async fn reset(&self) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_emulator(|emu| {
                let report = emu.reset()?;
                Ok(vec![text(key_report("reset", &report))])
            })
            .await;
        finish(result)
    }

    #[tool(
        description = "Session facts as JSON: model, ROM path, CPU cycles, emulated milliseconds, whether \
        the Kermit server runs, keys pressed so far, annunciators."
    )]
    async fn status(&self) -> Result<CallToolResult, ErrorData> {
        let result = self
            .with_state_locked(|state| {
                let value = match &state.emulator {
                    Some(emu) => emu.status()?,
                    None => serde_json::json!({
                        "model": null,
                        "startup_error": state.startup_error,
                    }),
                };
                Ok(vec![text(serde_json::to_string_pretty(&value)?)])
            })
            .await;
        finish(result)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SaturnusMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("saturnus-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "An emulated HP 48SX/48GX/49G/38G/39G/40G calculator. boot it (or it was booted from the \
                 command line), drive it with press_keys or type_text, look with screen. For the stack \
                 and object transfers, start the Kermit server (boot with autostart, or start_server \
                 with the stack showing), then use read_stack, run_command, send_object, \
                 receive_object; stop_server before pressing keys again. Time only passes while a tool \
                 runs.",
            )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn args<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Result<T, serde_json::Error> {
        serde_json::from_value(v)
    }

    #[test]
    fn boot_args() {
        let a: BootArgs = args(serde_json::json!({"model": "49g", "rom_path": "/r"})).unwrap();
        assert!(matches!(a.model, ModelArg::Hp49g));
        assert!(!a.autostart);
        assert!(args::<BootArgs>(serde_json::json!({"model": "50g", "rom_path": "/r"})).is_err());
        assert!(args::<BootArgs>(serde_json::json!({"model": "48sx"})).is_err());
    }

    #[test]
    fn screen_args_default_to_png() {
        let a: ScreenArgs = args(serde_json::json!({})).unwrap();
        assert!(matches!(a.format, ScreenFormat::Png));
        let a: ScreenArgs = args(serde_json::json!({"format": "text"})).unwrap();
        assert!(matches!(a.format, ScreenFormat::Text));
        assert!(args::<ScreenArgs>(serde_json::json!({"format": "gif"})).is_err());
    }

    #[test]
    fn send_object_payload_and_mode() {
        let a: SendObjectArgs = args(serde_json::json!({"name": "X", "text": "42"})).unwrap();
        assert_eq!(
            object_payload(&a).unwrap(),
            (b"42".to_vec(), TransferMode::Ascii)
        );
        let a: SendObjectArgs =
            args(serde_json::json!({"name": "X", "bytes_base64": "AAEC"})).unwrap();
        assert_eq!(
            object_payload(&a).unwrap(),
            (vec![0, 1, 2], TransferMode::Binary)
        );
        let a: SendObjectArgs =
            args(serde_json::json!({"name": "X", "text": "A", "mode": "binary"})).unwrap();
        assert_eq!(object_payload(&a).unwrap().1, TransferMode::Binary);
        let both: SendObjectArgs =
            args(serde_json::json!({"name": "X", "text": "A", "bytes_base64": "AA=="})).unwrap();
        assert!(object_payload(&both).is_err());
        let neither: SendObjectArgs = args(serde_json::json!({"name": "X"})).unwrap();
        assert!(object_payload(&neither).is_err());
        let bad: SendObjectArgs =
            args(serde_json::json!({"name": "X", "bytes_base64": "!!"})).unwrap();
        assert!(object_payload(&bad).is_err());
    }

    #[test]
    fn stack_text_lists_highest_level_first() {
        assert_eq!(stack_text(&[]), "Empty Stack");
        assert_eq!(stack_text(&["42".into(), "'A'".into()]), "2: 'A'\n1: 42");
    }

    #[test]
    fn every_tool_has_a_description_and_object_schema() {
        let tools = SaturnusMcp::new().tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        for expected in [
            "boot",
            "press_keys",
            "type_text",
            "screen",
            "read_stack",
            "run_command",
            "send_object",
            "receive_object",
            "start_server",
            "stop_server",
            "save_state",
            "load_state",
            "reset",
            "status",
        ] {
            assert!(names.contains(&expected), "missing {expected}: {names:?}");
        }
        for t in &tools {
            assert!(
                t.description.as_deref().is_some_and(|d| !d.is_empty()),
                "{}",
                t.name
            );
            assert_eq!(
                t.input_schema.get("type"),
                Some(&serde_json::json!("object"))
            );
        }
    }

    #[tokio::test]
    async fn tools_without_a_calculator_are_tool_errors() {
        let server = SaturnusMcp::new();
        let r = server
            .press_keys(Parameters(PressKeysArgs { script: "1".into() }))
            .await
            .unwrap();
        assert_eq!(r.is_error, Some(true));
        let r = server
            .boot(Parameters(BootArgs {
                model: ModelArg::Hp48sx,
                rom_path: "/nonexistent/rom".into(),
                autostart: false,
            }))
            .await
            .unwrap();
        assert_eq!(r.is_error, Some(true));
        let msg = r.content[0].as_text().unwrap().text.clone();
        assert!(msg.contains("cannot read ROM"), "{msg}");
        let r = server
            .boot(Parameters(BootArgs {
                model: ModelArg::Hp38g,
                rom_path: "/nonexistent/rom".into(),
                autostart: true,
            }))
            .await
            .unwrap();
        let msg = r.content[0].as_text().unwrap().text.clone();
        assert!(msg.contains("38G has no Kermit server"), "{msg}");
        let r = server.status().await.unwrap();
        assert_eq!(r.is_error, Some(false));
    }
}
