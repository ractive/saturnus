//! The writes on [`crate::Emulator`].

use super::*;

impl crate::Emulator {
    /// Start the write `op`; every key is released first. Fails, running
    /// nothing, when the calculator cannot take it now.
    pub fn start_transfer(&mut self, op: Op) -> crate::Result<()> {
        if self.typing.is_some() || self.transfer.is_some() {
            return Err("a send or a transfer is already in progress"
                .to_string()
                .into());
        }
        self.release_keys();
        self.transfer = Some(Transfer::new(&self.machine, op)?);
        self.menu_dirty = true;
        Ok(())
    }

    /// The address of the variable `target` in `dir`, or of a stack level.
    fn target_address(
        &self,
        u: &UserMemory<'_>,
        dir: &[String],
        target: &Target,
    ) -> crate::Result<u32> {
        Ok(match target {
            Target::Variable(name) => {
                let tree = u.tree()?;
                let dir = components(dir)?;
                let var = directory(&tree, dir)?
                    .iter()
                    .find(|v| &v.name == name)
                    .ok_or_else(|| format!("no variable {name} in {{ {} }}", cd_command(dir)))?;
                if var.variables.is_some() {
                    return Err(format!("{name} is a directory, which has no text").into());
                }
                var.address
            }
            Target::Level(n) => {
                let levels = u.stack_addresses()?;
                *n.checked_sub(1)
                    .and_then(|i| levels.get(i))
                    .ok_or_else(|| {
                        format!("there is no level {n} (the stack has {})", levels.len())
                    })?
            }
        })
    }

    /// The text to edit of the variable `target` in `dir` or of a stack
    /// level (`editText`), written for compiling again
    /// ([`saturnus_objects::decompile::edit_text`]), and the object's
    /// identity ([`crate::Emulator::edit_identity`]).
    pub fn edit_text(&self, dir: &[String], target: &Target) -> crate::Result<(String, String)> {
        let names = self.names();
        let u = UserMemory::of(&self.machine)?.with_names(&names);
        let address = self.target_address(&u, dir, target)?;
        let text = u.edit_text_at(address)?;
        Ok((text, identity(&u, address)?))
    }

    /// The identity of the object `target` holds: its size in nibbles and
    /// its checksum (`"46:1A2B"`), the same in any display mode; what
    /// `storeText`'s `was` names.
    pub fn edit_identity(&self, dir: &[String], target: &Target) -> crate::Result<String> {
        let u = UserMemory::of(&self.machine)?;
        let address = self.target_address(&u, dir, target)?;
        identity(&u, address)
    }

    /// An error unless the object `target` holds is still `was` (its
    /// [`crate::Emulator::edit_identity`] when the editor opened it): a
    /// save never replaces what the editor did not show.
    pub fn check_unchanged(&self, dir: &[String], target: &Target, was: &str) -> crate::Result<()> {
        if self.edit_identity(dir, target).ok().as_deref() == Some(was) {
            return Ok(());
        }
        Err(match target {
            Target::Variable(n) => {
                format!("{n} changed on the calculator since it was opened: open it again")
            }
            Target::Level(n) => {
                format!("level {n} changed on the calculator since it was opened: open it again")
            }
        }
        .into())
    }

    /// Lines the write in progress has typed and run so far (0 without
    /// one): when the clock's `→TIME` and `→DATE` ran.
    pub fn transfer_lines_run(&self) -> usize {
        self.transfer.as_ref().map_or(0, |t| t.lines_run)
    }

    /// Whether a write is in progress.
    pub fn transferring(&self) -> bool {
        self.transfer.is_some()
    }

    /// Run the write for at most `ms` emulated ms; true once it is done
    /// (its result is then ready). An error of the machine ends it.
    pub fn transfer_step(&mut self, ms: f64) -> crate::Result<bool> {
        let budget = self.step_cycles(ms);
        let Some(t) = self.transfer.as_mut() else {
            return Err("no transfer in progress".to_string().into());
        };
        match t.step(&mut self.machine, budget) {
            Ok(done) => Ok(done),
            Err(e) => {
                // The machine failed (a halt): nothing more can run on it.
                if let Some(t) = self.transfer.take() {
                    t.abandon(&mut self.machine);
                }
                Err(e)
            }
        }
    }

    /// Stop the write where it is: its keys let go at once, then its
    /// cleanup (a running server ended with ON, the 49G's algebraic mode
    /// set again) runs in turns through [`crate::Emulator::transfer_step`] like
    /// the rest of the write, and [`crate::Emulator::transfer_result`] says how it
    /// ended ([`STOPPED`], or why the cleanup failed).
    pub fn stop_transfer(&mut self) {
        if let Some(t) = self.transfer.as_mut() {
            t.stop(&mut self.machine);
        }
    }

    /// Give the write up where it is, without its cleanup (its cleanup
    /// itself ran out of time): every key let go.
    pub fn abandon_transfer(&mut self) {
        if let Some(t) = self.transfer.take() {
            t.abandon(&mut self.machine);
        }
    }

    /// Stop the write and run its cleanup to the end at once (at most
    /// `cap_ms` of emulated time; for tests and hosts with no turns to
    /// give): the result, as [`crate::Emulator::transfer_result`].
    pub fn stop_transfer_now(
        &mut self,
        cap_ms: f64,
    ) -> crate::Result<(TransferResult, Option<Vec<u8>>)> {
        self.stop_transfer();
        let mut left = cap_ms;
        while self.transfer.is_some() && left > 0.0 {
            if self.transfer_step(STOP_TURN_MS)? {
                break;
            }
            left -= STOP_TURN_MS;
        }
        self.transfer_result()
    }

    /// The result of a finished write, and the file of a fetch.
    pub fn transfer_result(&mut self) -> crate::Result<(TransferResult, Option<Vec<u8>>)> {
        let t = self
            .transfer
            .take()
            .ok_or_else(|| "no transfer in progress".to_string())?;
        t.finish(&self.machine)
    }
}
