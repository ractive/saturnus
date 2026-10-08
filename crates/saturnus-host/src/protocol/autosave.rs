//! Auto-save (iteration 27): the calculator keeps its state across a
//! reload, as a real one keeps its memory when turned off. The engine
//! decides when; the host keeps the state ([`super::Output::Save`]): the
//! Worker in IndexedDB, the desktop app in its data folder.
//!
//! A change is what the page or the outside did to the machine: a key, a
//! send, a write, a state loaded, a reset, a poke, serial input. A machine
//! that only keeps time (the clock ticking, the cursor blinking) has not
//! changed: an idle calculator is never written, which matters for the
//! 49G, whose state carries its 2 MB flash.
//!
//! The state is saved [`DELAY_MS`] after the last change, at once when
//! the page is hidden (on a phone, the last moment before the system may
//! kill it), and only once the machine has settled: no send or write in
//! progress, the CPU asleep with no key down or queued (not computing),
//! not halted. A save that comes due while the machine is busy waits until
//! it settles.

/// Wall ms from the last change to its save.
pub const DELAY_MS: f64 = 5_000.0;

/// What is owed to the host's store.
#[derive(Clone, Debug, Default)]
pub(crate) struct AutoSave {
    /// The host keeps states ([`super::Engine::set_auto_save`]).
    pub on: bool,
    /// The machine changed at this time and has not been saved since.
    changed: Option<f64>,
    /// The page was hidden with a change unsaved: save once settled.
    urgent: bool,
    /// Saves handed to the host so far.
    pub saves: u64,
}

impl AutoSave {
    /// The machine changed at `now`.
    pub fn touch(&mut self, now: f64) {
        if self.on {
            self.changed = Some(now);
        }
    }

    /// The page was hidden: an unsaved change is saved as soon as the
    /// machine has settled.
    pub fn hide(&mut self) {
        if self.changed.is_some() {
            self.urgent = true;
        }
    }

    /// Nothing is owed: a new machine (booted or restored) is what the
    /// store has, or was never asked to keep.
    pub fn clear(&mut self) {
        self.changed = None;
        self.urgent = false;
    }

    /// A change is unsaved.
    pub fn owed(&self) -> bool {
        self.changed.is_some()
    }

    /// When the save is due, if one is owed.
    pub fn due(&self) -> Option<f64> {
        self.changed
            .map(|t| if self.urgent { t } else { t + DELAY_MS })
    }

    /// Whether to save now: one is owed, due by `now` (within `slack`),
    /// and the machine has `settled`. Saying yes clears what is owed.
    pub fn poll(&mut self, now: f64, slack: f64, settled: bool) -> bool {
        match self.due() {
            Some(due) if settled && due <= now + slack => {
                self.clear();
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> AutoSave {
        AutoSave {
            on: true,
            ..AutoSave::default()
        }
    }

    #[test]
    fn nothing_is_owed_without_a_change() {
        let mut a = on();
        for t in [0.0, 5_000.0, 60_000.0, 3_600_000.0] {
            assert!(!a.poll(t, 1.0, true), "{t}");
        }
        assert_eq!(a.due(), None);
        a.hide();
        assert!(!a.poll(3_600_001.0, 1.0, true), "hidden, unchanged");
    }

    #[test]
    fn a_change_is_saved_once_after_the_delay() {
        let mut a = on();
        a.touch(1_000.0);
        assert_eq!(a.due(), Some(1_000.0 + DELAY_MS));
        assert!(!a.poll(3_000.0, 1.0, true), "too early");
        assert!(a.poll(6_000.0, 1.0, true));
        assert!(!a.poll(6_001.0, 1.0, true), "once");
        assert!(!a.poll(60_000.0, 1.0, true), "nothing new");
    }

    #[test]
    fn each_change_moves_the_save_on() {
        let mut a = on();
        a.touch(0.0);
        a.touch(4_000.0);
        assert!(
            !a.poll(5_000.0, 1.0, true),
            "the delay counts from the last"
        );
        assert!(a.poll(9_000.0, 1.0, true));
        assert!(!a.poll(20_000.0, 1.0, true), "once");
    }

    #[test]
    fn a_busy_machine_is_saved_once_it_settles() {
        let mut a = on();
        a.touch(0.0);
        assert!(!a.poll(10_000.0, 1.0, false), "computing");
        assert!(!a.poll(20_000.0, 1.0, false), "still");
        assert!(a.poll(20_500.0, 1.0, true), "settled: saved at once");
    }

    #[test]
    fn hidden_saves_at_once_when_settled() {
        let mut a = on();
        a.touch(1_000.0);
        a.hide();
        assert_eq!(a.due(), Some(1_000.0));
        assert!(!a.poll(1_200.0, 1.0, false), "mid-transfer: waits");
        assert!(a.poll(1_300.0, 1.0, true), "settled: no delay");
        assert_eq!(a.due(), None);
        // The next change has the delay again.
        a.touch(2_000.0);
        assert_eq!(a.due(), Some(2_000.0 + DELAY_MS));
    }

    #[test]
    fn off_owes_nothing() {
        let mut a = AutoSave::default();
        a.touch(0.0);
        a.hide();
        assert!(!a.owed());
        assert!(!a.poll(10_000.0, 1.0, true));
    }
}
