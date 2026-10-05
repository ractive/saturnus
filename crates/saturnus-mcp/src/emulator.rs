//! One emulated calculator and everything the tools do with it. All of it
//! is synchronous; the server runs it on a blocking thread under its
//! session lock, so a key script and a Kermit exchange never overlap.

use std::path::{Path, PathBuf};
use std::sync::MutexGuard;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use hptx_core::reply::StackReply;
use hptx_core::{Calculator, Options, Session as KermitSession, TransferMode};
use saturnus::{Machine, Model};
use saturnus_drive::autostart::{BOOT_CAP_MS, autostart_script, boot_script};
use saturnus_drive::rom;
use saturnus_drive::screen;
use saturnus_drive::script::{Action, Line};
use saturnus_drive::session::{Limits, Session};

use crate::keys;
use crate::link::{Core, MachineTransport, SharedCore};

/// Longest emulated wait for the idle server's first NAK after SERVER (it
/// NAKs about every 5 s while idle).
const SERVER_CAP: Duration = Duration::from_secs(15);
/// Emulated time after `G F` before keys are accepted again: on the 48SX,
/// SERVER typed within 2 s after FINISH loses keys (hptx calculator
/// quirks).
const FINISH_SETTLE_MS: u64 = 2_500;
/// Kermit reply timeout per packet (wall-clock; the link runs emulated
/// time while it waits). Shorter than hptx's 20 s default so a calculator
/// that left server mode fails a tool in seconds, not minutes.
const KERMIT_TIMEOUT: Duration = Duration::from_secs(6);
/// Kermit retransmissions per packet.
const KERMIT_RETRIES: u32 = 3;

/// What a key script or a boot did, for the tool reply.
#[derive(Clone, Debug)]
pub struct KeyReport {
    /// Emulated milliseconds the script took.
    pub elapsed_ms: u64,
    /// Key presses in the script.
    pub presses: u64,
    /// `wait-idle` caps that were reached.
    pub warnings: Vec<String>,
    /// The LCD as 64 lines of 131 `#`/`.`.
    pub screen: String,
    /// The lit annunciators, `-` for none.
    pub annunciators: String,
    /// Whether the Kermit server was stopped first (keystroke tools).
    pub left_server: bool,
}

/// The emulated calculator of the MCP session.
pub struct Emulator {
    core: SharedCore,
    model: Model,
    rom_path: PathBuf,
    /// The Kermit client while the calculator's server runs.
    calc: Option<Calculator>,
    keys_pressed: u64,
}

impl std::fmt::Debug for Emulator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Emulator")
            .field("model", &self.model)
            .field("rom_path", &self.rom_path)
            .field("server_running", &self.calc.is_some())
            .field("keys_pressed", &self.keys_pressed)
            .finish_non_exhaustive()
    }
}

/// The first bytes of a saturnus state file (the core's `state` format).
pub const STATE_MAGIC: &[u8; 8] = b"SATURNUS";

/// Whether `path` starts with [`STATE_MAGIC`].
fn is_state_file(path: &Path) -> Result<bool> {
    use std::io::Read;
    let mut head = [0u8; STATE_MAGIC.len()];
    let mut f =
        std::fs::File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    let n = f
        .read(&mut head)
        .with_context(|| format!("cannot read {}", path.display()))?;
    Ok(n == head.len() && &head == STATE_MAGIC)
}

/// Whether `a` and `b` name the same existing file.
fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// A model name as the tools and the command line take it.
pub fn parse_model(name: &str) -> Result<Model> {
    match name.to_ascii_lowercase().as_str() {
        "48sx" => Ok(Model::Hp48sx),
        "48gx" => Ok(Model::Hp48gx),
        "49g" => Ok(Model::Hp49g),
        "38g" => Ok(Model::Hp38g),
        "39g" => Ok(Model::Hp39g),
        "40g" => Ok(Model::Hp40g),
        "42s" => Ok(Model::Hp42s),
        _ => bail!("unknown model {name:?}: expected 48sx, 48gx, 49g, 38g, 39g, 40g or 42s"),
    }
}

impl Emulator {
    /// Build `model` from the ROM at `rom_path`, boot to the first prompt
    /// and answer it (NO at "Try To Recover Memory?", then OK on the 49G;
    /// OK on the 38G, 39G and 40G), so the stack (aplet models: HOME)
    /// shows; nothing to answer on the 42S. With `autostart`, also start
    /// the Kermit server (not on the 38G, 39G, 40G or 42S, which have
    /// none).
    pub fn boot(
        model: Model,
        rom_path: &Path,
        autostart: bool,
        limits: Limits,
    ) -> Result<(Self, KeyReport)> {
        if autostart && !crate::semantic::has_server(model) {
            bail!(
                "the {} has no Kermit server; boot it without autostart",
                model.name().to_uppercase()
            );
        }
        let image = rom::load(model, rom_path)?;
        let machine = Machine::new(model, &image).context("cannot build the machine")?;
        let mut emu = Emulator::from_machine(machine, rom_path);
        emu.set_limits(limits)?;
        let mut report = emu.run_lines(&boot_script(model))?;
        if autostart {
            let server = emu.start_server()?;
            report.elapsed_ms += server.elapsed_ms;
            report.presses += server.presses;
            report.warnings.extend(server.warnings);
            report.screen = server.screen;
            report.annunciators = server.annunciators;
        }
        Ok((emu, report))
    }

    /// Wrap a machine as is, without booting it.
    pub(crate) fn from_machine(machine: Machine, rom_path: &Path) -> Self {
        let model = machine.model();
        let mut session = Session::new(machine, 0, false);
        session.set_echo_warnings(false);
        Emulator {
            core: Core::new(session),
            model,
            rom_path: rom_path.to_path_buf(),
            calc: None,
            keys_pressed: 0,
        }
    }

    /// Bound everything the machine runs from now on, keys and Kermit
    /// alike (the server sets these per tool call).
    pub fn set_limits(&self, limits: Limits) -> Result<()> {
        self.core()?.session.set_limits(limits);
        Ok(())
    }

    /// The emulated model.
    pub fn model(&self) -> Model {
        self.model
    }

    /// Whether the Kermit server runs (started by us and not stopped).
    pub fn server_running(&self) -> bool {
        self.calc.is_some()
    }

    /// The machine and the link (a tool's lock is already held). Never
    /// poisoned: see [`crate::link::lock`].
    pub(crate) fn core(&self) -> Result<MutexGuard<'_, Core>> {
        Ok(crate::link::lock(&self.core))
    }

    /// Run parsed script lines, checked against the model's keyboard first.
    pub(crate) fn run_lines(&mut self, lines: &[Line]) -> Result<KeyReport> {
        let mut core = self.core()?;
        core.session.check_keys(lines)?;
        let start = core.session.machine.cycles();
        let mut presses = 0;
        for line in lines {
            core.session.apply(line)?;
            if matches!(line.action, Action::Press { .. } | Action::Down(_)) {
                presses += 1;
            }
        }
        // The server is not reading the port: what it sent is stale.
        core.discard_input();
        let elapsed_ms = (core.session.machine.cycles() - start) / core.session.cycles_per_ms();
        let warnings = core.session.take_warnings();
        let fb = core.session.machine.framebuffer();
        drop(core);
        self.keys_pressed += presses;
        Ok(KeyReport {
            elapsed_ms,
            presses,
            warnings,
            screen: fb.to_text(),
            annunciators: fb.annunciator_line(),
            left_server: false,
        })
    }

    /// The Kermit server owns the keyboard: stop it before keys. Returns
    /// whether it ran.
    fn leave_server_for_keys(&mut self) -> Result<bool> {
        if self.calc.is_none() {
            return Ok(false);
        }
        self.stop_server()
            .context("cannot leave Kermit server mode before pressing keys")?;
        Ok(true)
    }

    /// The server counts as stopped (it was interrupted).
    pub(crate) fn forget_server(&mut self) {
        self.calc = None;
    }

    /// Run a `press_keys` script (see [`keys::parse_script`]), leaving
    /// Kermit server mode first if needed.
    pub fn press_keys(&mut self, script: &str) -> Result<KeyReport> {
        let lines = keys::parse_script(script)?;
        keys::check_budget(&lines)?;
        let left = self.leave_server_for_keys()?;
        let mut report = self.run_lines(&lines)?;
        report.left_server = left;
        Ok(report)
    }

    /// Type `text` (see [`keys::type_keys`]), leaving Kermit server mode
    /// first if needed.
    pub fn type_text(&mut self, text: &str) -> Result<KeyReport> {
        let lines = keys::typing_lines(&keys::type_keys(self.model, text)?);
        keys::check_budget(&lines).context("text too long for one call")?;
        let left = self.leave_server_for_keys()?;
        let mut report = self.run_lines(&lines)?;
        report.left_server = left;
        // Each key is a down and an up line; count it once.
        self.keys_pressed -= report.presses / 2;
        report.presses /= 2;
        Ok(report)
    }

    /// The LCD as text (64 lines of 131 `#`/`.`) and the annunciator line.
    pub fn screen_text(&self) -> Result<(String, String)> {
        let fb = self.core()?.session.machine.framebuffer();
        Ok((fb.to_text(), fb.annunciator_line()))
    }

    /// The LCD as a 1-bit PNG, each pixel `scale` x `scale`, and the
    /// annunciator line.
    pub fn screen_png(&self, scale: u32) -> Result<(Vec<u8>, String)> {
        let fb = self.core()?.session.machine.framebuffer();
        Ok((screen::png_bytes(&fb.pixels, scale)?, fb.annunciator_line()))
    }

    /// Type ALPHA ALPHA S E R V E R ENTER and wait for the server's first
    /// NAK. The calculator must show the stack with an empty command line.
    pub fn start_server(&mut self) -> Result<KeyReport> {
        if self.calc.is_some() {
            bail!("the Kermit server is already running");
        }
        let script = autostart_script(self.model, false)?;
        let mut report = self.run_lines(&script)?;
        let mut core = self.core()?;
        let start = core.session.machine.cycles();
        let up = core.wait_for_nak(SERVER_CAP)?;
        report.elapsed_ms += (core.session.machine.cycles() - start) / core.session.cycles_per_ms();
        let fb = core.session.machine.framebuffer();
        drop(core);
        report.screen = fb.to_text();
        report.annunciators = fb.annunciator_line();
        if !up {
            bail!(
                "no Kermit NAK within {} s of emulated time after SERVER: was the stack showing \
                 with an empty command line? Check with screen",
                SERVER_CAP.as_secs()
            );
        }
        let mut options = Options::default();
        options.kermit.timeout = KERMIT_TIMEOUT;
        options.kermit.retries = KERMIT_RETRIES;
        options.drain = Duration::ZERO;
        // The pause between transactions runs as emulated time in the link
        // (`link::TURNAROUND`), not as a wall-clock sleep.
        options.turnaround = Duration::ZERO;
        let transport = MachineTransport::new(self.core.clone());
        let session = KermitSession::new(Box::new(transport), options)
            .context("cannot open the Kermit session")?;
        self.calc = Some(Calculator::new(session));
        Ok(report)
    }

    /// End server mode with Kermit `G F`, give the ROM time to return to
    /// the stack and wait until idle. Returns whether the calculator
    /// acknowledged `G F`; it counts as stopped either way.
    pub fn stop_server(&mut self) -> Result<(bool, KeyReport)> {
        let Some(mut calc) = self.calc.take() else {
            bail!("the Kermit server is not running");
        };
        self.core()?.discard_input();
        let acknowledged = calc.finish().is_ok();
        drop(calc);
        let report = self.run_lines(&[
            Line {
                number: 1,
                action: Action::Wait {
                    ms: FINISH_SETTLE_MS,
                },
            },
            Line {
                number: 2,
                action: Action::WaitIdle {
                    cap_ms: saturnus_drive::script::DEFAULT_IDLE_CAP_MS,
                },
            },
        ])?;
        Ok((acknowledged, report))
    }

    /// The Kermit client, with stale input dropped.
    pub(crate) fn kermit(&mut self) -> Result<&mut Calculator> {
        if self.calc.is_none() {
            bail!(
                "the Kermit server is not running: call start_server (with the stack showing), \
                 or boot with autostart"
            );
        }
        self.core()?.discard_input();
        self.calc
            .as_mut()
            .context("the Kermit server is not running")
    }

    /// The stack, level 1 first; at most `levels` levels.
    pub fn read_stack(&mut self, levels: Option<usize>) -> Result<Vec<String>> {
        // An empty host command runs nothing and returns the stack.
        let reply = self.kermit()?.run("").context("reading the stack")?;
        if let Some(e) = reply.error {
            bail!("calculator error while reading the stack: {e}");
        }
        let mut values = reply.levels;
        if let Some(n) = levels {
            values.truncate(n);
        }
        Ok(values)
    }

    /// Run a host command; the reply holds the stack afterwards and the
    /// calculator's error text, if any.
    pub fn run_command(&mut self, command: &str) -> Result<StackReply> {
        Ok(self.kermit()?.run(command)?)
    }

    /// Store `data` as variable `name` in the current directory; returns the
    /// name the calculator stored it under.
    pub fn send_object(&mut self, name: &str, data: &[u8], mode: TransferMode) -> Result<String> {
        Ok(self.kermit()?.put(name, data, mode)?)
    }

    /// Fetch variable `name` from the current directory.
    pub fn receive_object(&mut self, name: &str, mode: TransferMode) -> Result<Vec<u8>> {
        Ok(self.kermit()?.get(name, mode)?)
    }

    /// Write the machine state to `path`, through a temporary file in the
    /// same directory and a rename. Refuses the ROM the session booted
    /// from, and an existing file that is not a saturnus state unless
    /// `overwrite`.
    pub fn save_state(&self, path: &Path, overwrite: bool) -> Result<usize> {
        if same_file(path, &self.rom_path) {
            bail!(
                "{} is the ROM of this session; refusing to overwrite it",
                path.display()
            );
        }
        if path.exists() && !overwrite && !is_state_file(path)? {
            bail!(
                "{} exists and is not a saturnus state; pass overwrite: true to replace it",
                path.display()
            );
        }
        let data = self.core()?.session.machine.save_state();
        let name = path
            .file_name()
            .with_context(|| format!("{} names no file", path.display()))?;
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(format!(".tmp-{}", std::process::id()));
        let tmp = path.with_file_name(tmp_name);
        std::fs::write(&tmp, &data)
            .with_context(|| format!("cannot write state {}", tmp.display()))?;
        if let Err(e) = std::fs::rename(&tmp, path) {
            // Best effort: the rename error is the one to report.
            let _ = std::fs::remove_file(&tmp);
            return Err(e).with_context(|| format!("cannot write state {}", path.display()));
        }
        Ok(data.len())
    }

    /// Restore the machine state from `path` (saved from the same ROM). On
    /// success the Kermit server counts as stopped; save states with it
    /// stopped. A refused state changes nothing.
    pub fn load_state(&mut self, path: &Path) -> Result<()> {
        let data =
            std::fs::read(path).with_context(|| format!("cannot read state {}", path.display()))?;
        let mut core = self.core()?;
        core.session
            .machine
            .load_state(&data)
            .with_context(|| format!("cannot load state {}", path.display()))?;
        core.discard_input();
        drop(core);
        self.calc = None;
        Ok(())
    }

    /// Hardware reset (RAM kept), then run until idle; the ROM's first
    /// screen is left for the caller to answer. The server counts as
    /// stopped.
    pub fn reset(&mut self) -> Result<KeyReport> {
        self.calc = None;
        self.core()?.session.machine.reset();
        self.run_lines(&[Line {
            number: 1,
            action: Action::WaitIdle {
                cap_ms: BOOT_CAP_MS,
            },
        }])
    }

    /// Run `f` on the machine (timing studies: the core's executed-instruction
    /// profile, emulated time).
    pub fn with_machine<R>(&self, f: impl FnOnce(&mut Machine) -> R) -> Result<R> {
        Ok(f(&mut self.core()?.session.machine))
    }

    /// Model, ROM, emulated time and session facts as JSON.
    pub fn status(&self) -> Result<serde_json::Value> {
        let core = self.core()?;
        let m = &core.session.machine;
        let cycles = m.cycles();
        let fb = m.framebuffer();
        Ok(serde_json::json!({
            "model": self.model.name(),
            "rom_path": self.rom_path.display().to_string(),
            "cycles": cycles,
            "clock_hz": self.model.clock_hz(),
            "emulated_ms": cycles / core.session.cycles_per_ms(),
            "server_running": self.calc.is_some(),
            "mode": if self.calc.is_some() { "server" } else { "keyboard" },
            "keys_pressed": self.keys_pressed,
            "cpu_shutdown": m.is_shutdown(),
            "display_on": m.hw.io.display_on(),
            "annunciators": fb.annunciator_line(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A session around a blank 48SX ROM image (no boot), with the ROM
    /// written to `dir` so the save/load paths can be checked.
    fn blank_session(dir: &Path) -> Emulator {
        let rom_path = dir.join("blank.rom");
        let image = vec![0u8; Model::Hp48sx.rom_bytes()];
        std::fs::write(&rom_path, &image).unwrap();
        let machine = Machine::new(Model::Hp48sx, &image).unwrap();
        Emulator::from_machine(machine, &rom_path)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("saturnus-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_state_refuses_the_rom_and_foreign_files_unless_overwrite() {
        let dir = scratch("save");
        let emu = blank_session(&dir);
        let e = emu.save_state(&dir.join("blank.rom"), true).unwrap_err();
        assert!(e.to_string().contains("ROM of this session"), "{e}");
        assert_eq!(
            std::fs::metadata(dir.join("blank.rom")).unwrap().len(),
            262_144
        );

        let doc = dir.join("notes.txt");
        std::fs::write(&doc, b"keep me").unwrap();
        let e = emu.save_state(&doc, false).unwrap_err();
        assert!(e.to_string().contains("not a saturnus state"), "{e}");
        assert_eq!(std::fs::read(&doc).unwrap(), b"keep me");
        emu.save_state(&doc, true).unwrap();
        assert!(std::fs::read(&doc).unwrap().starts_with(STATE_MAGIC));

        // A state file may be replaced without the flag, and nothing else
        // is left behind in the directory.
        let state = dir.join("a.state");
        emu.save_state(&state, false).unwrap();
        emu.save_state(&state, false).unwrap();
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(names.iter().all(|n| !n.contains(".tmp-")), "{names:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_load_changes_nothing() {
        let dir = scratch("load");
        let mut emu = blank_session(&dir);
        let before = emu.core().unwrap().session.machine.save_state();
        let bad = dir.join("bad.state");
        std::fs::write(&bad, b"not a state").unwrap();
        let e = emu.load_state(&bad).unwrap_err();
        assert!(e.to_string().contains("cannot load state"), "{e}");
        assert_eq!(emu.core().unwrap().session.machine.save_state(), before);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
