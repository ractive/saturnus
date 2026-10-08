//! `watchMemory` and the `memoryChanged` event: the user memory is looked
//! at on the machine's side, never by the page (`web/protocol.md`,
//! "Pacing"): only while a page watches, only when the machine ran since
//! the last look (or its memory changed without running: a new machine, a
//! loaded state, a poke), only while it is not computing, at most every
//! [`LOOK_MS`], and not within [`EVENT_MS`] of the last event. A look that
//! comes too early sets [`Watch::due`], so a sleeping machine still tells
//! of the last change.

use super::Clock;
use super::pacing::Core;

/// Shortest time between two `memoryChanged` events, in ms.
pub const EVENT_MS: f64 = 250.0;
/// Shortest time between two looks at the user memory, in ms.
pub const LOOK_MS: f64 = 100.0;

/// The user memory's change counter, or why it cannot be read; `None`
/// without a machine.
pub(crate) fn memory_state(core: Option<&impl Core>) -> Option<String> {
    core.map(|c| match c.memory_changes() {
        Ok(n) => format!("{n:016X}"),
        Err(e) => format!("error: {e}"),
    })
}

/// The memory as the page was last told.
#[derive(Clone, Debug, Default)]
pub(crate) struct Watch {
    /// A page asked for `memoryChanged` events.
    pub on: bool,
    /// The change counter (or read error) the page knows.
    last: Option<String>,
    /// When it was last read.
    at: Option<f64>,
    /// When the page was last told.
    told: Option<f64>,
    /// The machine's cycle count then: no cycles, no change.
    cycles: u64,
    /// Memory changed without cycles.
    pub force: bool,
    /// A look is owed at this time.
    pub due: Option<f64>,
    /// Looks so far and the wall ms they took, for `stats`.
    pub looks: u64,
    pub spent_ms: f64,
}

impl Watch {
    /// Start (or stop) watching: events are for what changes from here
    /// on, as the page reads after this.
    pub fn subscribe(&mut self, on: bool, core: Option<&impl Core>) {
        self.on = on;
        self.last = memory_state(core);
        self.at = None;
        self.told = None;
        self.force = false;
        self.due = None;
        if let Some(c) = core {
            self.cycles = c.cycles();
        }
    }

    /// Look if a look is due; `true` if the page must be told. `quiet`
    /// says the machine is not computing (its structures are whole).
    pub fn poll(&mut self, core: Option<&impl Core>, quiet: bool, clock: &dyn Clock) -> bool {
        let Some(core) = core else {
            return false;
        };
        if !self.on || self.due.is_some() || !quiet {
            return false;
        }
        let cycles = core.cycles();
        if cycles == self.cycles && !self.force {
            return false;
        }
        let now = clock.now_ms();
        let due = [
            self.at.map(|t| t + LOOK_MS),
            self.told.map(|t| t + EVENT_MS),
        ]
        .into_iter()
        .flatten()
        .fold(f64::NEG_INFINITY, f64::max);
        if due > now {
            self.due = Some(due);
            return false;
        }
        self.at = Some(now);
        self.cycles = cycles;
        self.force = false;
        let state = memory_state(Some(core));
        self.looks += 1;
        self.spent_ms += clock.now_ms() - now;
        if state != self.last {
            self.last = state;
            self.told = Some(now);
            return true;
        }
        false
    }
}
