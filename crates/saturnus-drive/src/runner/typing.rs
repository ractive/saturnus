//! The typing commands of the machine thread (`web/protocol.md`,
//! "Typing"): `commandLine`, and `insert`, `run`, `replace` (`typeText`
//! is `insert`), which type text into the command line by key presses
//! (`saturnus_host::typing`) at once in emulated time, as a key script
//! runs. A send of more than a few characters raises `busy` in the
//! `status` event and holds the frames until it is done; a shorter one
//! shows as it is typed. Bounded by [`SCRIPT_WALL_LIMIT`] of wall time and
//! stopped by a withdrawn ticket, like `keyScript`.

use std::time::Instant;

use serde_json::Value;

use super::{Runner, SCRIPT_WALL_LIMIT, Sink, json_of, str_field};
use crate::runner::Mode;

/// Emulated ms one step of a send runs between looks at the abort flag,
/// the deadline and the frames.
const STEP_MS: f64 = 20.0;

impl<S: Sink> Runner<S> {
    /// `{active, text, cursor}` of the command line, read from RAM.
    pub(super) fn command_line(&mut self) -> Result<Value, String> {
        json_of(&self.emu()?.command_line_inner()?)
    }

    /// Type `msg`'s `text` with `verb` and reply with the send's result.
    pub(super) fn send_text(&mut self, verb: &str, msg: &Value) -> Result<Value, String> {
        let text = str_field(msg, "text")?.to_string();
        if let Some(h) = &self.halted {
            return Err(format!("the CPU is halted: {h}"));
        }
        // A sleeping machine first catches up the time that passed.
        if matches!(self.mode, Mode::Sleep(_)) {
            self.wake();
        }
        let freezes = self.emu()?.start_typing_inner(verb, &text)?;
        if freezes {
            // The last frame stays up; the page shows its busy mark.
            self.busy = true;
            self.flush(true);
        }
        let started = Instant::now();
        let result = loop {
            if self
                .abort
                .as_ref()
                .is_some_and(|a| a.load(std::sync::atomic::Ordering::Relaxed))
            {
                break Err("cancelled".to_string());
            }
            if started.elapsed() > SCRIPT_WALL_LIMIT {
                break Err(format!(
                    "typing ran out of wall-clock time ({} s)",
                    SCRIPT_WALL_LIMIT.as_secs()
                ));
            }
            match self.emu()?.typing_step_inner(STEP_MS) {
                Ok(true) => break Ok(()),
                Ok(false) => {}
                Err(e) => break Err(e),
            }
            if !freezes {
                self.flush(false);
            }
        };
        self.work += started.elapsed();
        if freezes {
            // The status says the screen is live again before its frame.
            self.busy = false;
            self.send_status();
        }
        let reply = match result {
            Ok(()) => self.emu()?.typing_result_inner().and_then(|r| json_of(&r)),
            Err(e) => {
                self.emu()?.stop_typing_inner();
                Err(e)
            }
        };
        match &reply {
            Err(e) if e.contains("CPU halted") => self.halt(e.clone()),
            _ => self.set_running(self.running),
        }
        reply
    }
}
