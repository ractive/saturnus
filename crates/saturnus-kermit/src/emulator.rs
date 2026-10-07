//! One emulated calculator: booted from a ROM, driven by key scripts, and
//! talked to over Kermit once its ROM's server runs.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use saturnus::{Machine, Model};
use saturnus_drive::autostart::{autostart_script, boot_script};
use saturnus_drive::rom;
use saturnus_drive::script::{self, Action, DEFAULT_IDLE_CAP_MS, Line};
use saturnus_drive::session::{Limits, Session};

use crate::kermit::{Server, TransferMode};
use crate::link::Link;
use crate::reply::StackReply;

/// Longest emulated wait for the idle server's first NAK after SERVER (it
/// NAKs about every 5 s while idle).
const SERVER_CAP: Duration = Duration::from_secs(15);
/// Emulated time after `G F` before keys are accepted again: on the 48SX,
/// SERVER typed within 2 s after FINISH loses keys.
const FINISH_SETTLE_MS: u64 = 2_500;

/// What a key script did.
#[derive(Clone, Debug)]
pub struct KeyReport {
    /// Emulated milliseconds it took.
    pub elapsed_ms: u64,
    /// `wait-idle` caps that were reached.
    pub warnings: Vec<String>,
    /// Whether the Kermit server was stopped first.
    pub left_server: bool,
}

/// An emulated calculator and its Kermit link.
#[derive(Debug)]
pub struct Emulator {
    pub(crate) link: Link,
    model: Model,
    /// Whether the ROM's server runs (started here and not stopped).
    server: bool,
    /// The transfer mode last set or read while the server runs.
    mode: Option<TransferMode>,
}

/// Whether `model`'s ROM has a Kermit server.
pub fn has_server(model: Model) -> bool {
    matches!(model, Model::Hp48sx | Model::Hp48gx | Model::Hp49g)
}

impl Emulator {
    /// Build `model` from the ROM at `rom_path`, boot to the first prompt
    /// and answer it ([`boot_script`]), so the stack (aplet models: HOME)
    /// shows. With `autostart`, also start the Kermit server.
    pub fn boot(model: Model, rom_path: &Path, autostart: bool) -> Result<Self> {
        if autostart && !has_server(model) {
            bail!("the {} has no Kermit server", model.name().to_uppercase());
        }
        let image = rom::load(model, rom_path)?;
        let machine = Machine::new(model, &image).context("cannot build the machine")?;
        let mut emu = Emulator::from_machine(machine);
        emu.link.session.set_limits(Limits::default());
        emu.run_lines(&boot_script(model))?;
        if autostart {
            emu.start_server()?;
        }
        Ok(emu)
    }

    /// Wrap a machine as is, without booting it.
    pub fn from_machine(machine: Machine) -> Self {
        let model = machine.model();
        let mut session = Session::new(machine, 0, false);
        session.set_echo_warnings(false);
        Emulator {
            link: Link::new(session),
            model,
            server: false,
            mode: None,
        }
    }

    /// The emulated model.
    pub fn model(&self) -> Model {
        self.model
    }

    /// Whether the Kermit server runs.
    pub fn server_running(&self) -> bool {
        self.server
    }

    /// Run `f` on the machine.
    pub fn with_machine<R>(&mut self, f: impl FnOnce(&mut Machine) -> R) -> R {
        f(&mut self.link.session.machine)
    }

    /// The machine state (the core's state format).
    pub fn save_state(&self) -> Vec<u8> {
        self.link.session.machine.save_state()
    }

    /// Restore a state saved from the same ROM. The Kermit server counts
    /// as stopped: save states with it stopped.
    pub fn load_state(&mut self, data: &[u8]) -> Result<()> {
        self.link
            .session
            .machine
            .load_state(data)
            .context("cannot load the state")?;
        self.link.discard_input();
        self.server = false;
        Ok(())
    }

    /// Run script lines, checked against the model's keyboard first.
    pub(crate) fn run_lines(&mut self, lines: &[Line]) -> Result<KeyReport> {
        let s = &mut self.link.session;
        s.check_keys(lines)?;
        let start = s.machine.cycles();
        for line in lines {
            s.apply(line)?;
        }
        let elapsed_ms = (s.machine.cycles() - start) / s.cycles_per_ms();
        let warnings = s.take_warnings();
        // The server is not reading the port: what it sent is stale.
        self.link.discard_input();
        Ok(KeyReport {
            elapsed_ms,
            warnings,
            left_server: false,
        })
    }

    /// Run a key script (`saturnus_drive::script::parse_keys`: the CLI's
    /// format, several keys on a line, `+ - * / .` as keys), leaving
    /// Kermit server mode first: the server owns the keyboard.
    pub fn press_keys(&mut self, text: &str) -> Result<KeyReport> {
        let lines = script::parse_keys(text)?;
        let left = self.server;
        if left {
            self.stop_server()
                .context("cannot leave Kermit server mode before pressing keys")?;
        }
        let mut report = self.run_lines(&lines)?;
        report.left_server = left;
        Ok(report)
    }

    /// The LCD as lines of 131 `#`/`.` (64, or 16 on the 42S).
    pub fn screen_text(&self) -> String {
        self.link.session.machine.framebuffer().to_text()
    }

    /// Type ALPHA ALPHA S E R V E R ENTER and wait for the server's first
    /// NAK. The calculator must show the stack with an empty command line.
    pub fn start_server(&mut self) -> Result<()> {
        if !has_server(self.model) {
            bail!(
                "no Kermit server on this model: the {} has none",
                self.model.name().to_uppercase()
            );
        }
        if self.server {
            bail!("the Kermit server is already running");
        }
        self.run_lines(&autostart_script(self.model, false)?)?;
        if !self.link.wait_for_nak(SERVER_CAP)? {
            bail!(
                "no Kermit NAK within {} s of emulated time after SERVER: was the stack showing \
                 with an empty command line?",
                SERVER_CAP.as_secs()
            );
        }
        self.server = true;
        self.mode = None;
        Ok(())
    }

    /// End server mode with Kermit `G F`, give the ROM time to return to
    /// the stack and wait until idle. Returns whether the calculator
    /// acknowledged `G F`; it counts as stopped either way.
    pub fn stop_server(&mut self) -> Result<bool> {
        if !self.server {
            bail!("the Kermit server is not running");
        }
        let acknowledged = self.kermit()?.finish().is_ok();
        self.server = false;
        let settle = [
            Action::Wait {
                ms: FINISH_SETTLE_MS,
            },
            Action::WaitIdle {
                cap_ms: DEFAULT_IDLE_CAP_MS,
            },
        ];
        let lines: Vec<Line> = settle
            .into_iter()
            .enumerate()
            .map(|(i, action)| Line {
                number: i + 1,
                action,
            })
            .collect();
        self.run_lines(&lines)?;
        Ok(acknowledged)
    }

    /// The ROM's server, which must be running.
    pub fn kermit(&mut self) -> Result<Server<'_>> {
        if !self.server {
            bail!("the Kermit server is not running: start_server first (with the stack showing)");
        }
        Ok(Server::new(&mut self.link, &mut self.mode))
    }

    /// The server forgets it runs (it was interrupted).
    pub(crate) fn forget_server(&mut self) {
        self.server = false;
    }

    /// Run a host command; the reply holds the stack afterwards and the
    /// calculator's error text, if any.
    pub fn run_command(&mut self, command: &str) -> Result<StackReply> {
        self.kermit()?.run(command)
    }

    /// The stack as display text, level 1 first (an empty host command
    /// runs nothing and returns it).
    pub fn read_stack(&mut self) -> Result<Vec<String>> {
        let reply = self.run_command("")?;
        if let Some(e) = reply.error {
            bail!("calculator error while reading the stack: {e}");
        }
        Ok(reply.levels)
    }

    /// Store `data` as variable `name` in the current directory; returns
    /// the name the calculator stored it under.
    pub fn send_object(&mut self, name: &str, data: &[u8], mode: TransferMode) -> Result<String> {
        self.kermit()?.put(name, data, mode)
    }

    /// Fetch variable `name` from the current directory.
    pub fn receive_object(&mut self, name: &str, mode: TransferMode) -> Result<Vec<u8>> {
        self.kermit()?.get(name, mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(model: Model) -> Emulator {
        Emulator::from_machine(Machine::new(model, &vec![0u8; model.rom_bytes()]).unwrap())
    }

    #[test]
    fn models_without_a_server_are_refused() {
        let e = Emulator::boot(Model::Hp39g, Path::new("no-such.rom"), true).unwrap_err();
        assert!(e.to_string().contains("39G has no Kermit server"), "{e}");
        let e = blank(Model::Hp42s).start_server().unwrap_err();
        assert!(
            e.to_string().contains("no Kermit server on this model"),
            "{e}"
        );
        let e = blank(Model::Hp48sx).run_command("1").unwrap_err();
        assert!(e.to_string().contains("not running"), "{e}");
    }

    #[test]
    fn a_refused_state_changes_nothing() {
        let mut emu = blank(Model::Hp48sx);
        let before = emu.save_state();
        assert!(emu.load_state(b"not a state").is_err());
        assert_eq!(emu.save_state(), before);
    }
}
