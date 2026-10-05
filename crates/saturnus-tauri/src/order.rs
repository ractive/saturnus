//! Commands reach the machine thread in the order the page sent them.
//!
//! Tauri runs each async command as its own task, and two tasks may start
//! in either order, so a `keyDown` and the `keyUp` right after it could
//! swap and leave a key held. The page numbers its messages (`seq`, from
//! 0) within a `session` (an id it draws on every page load);
//! [`Sequencer`] parks a message until all earlier
//! ones have gone through, then releases it and every parked successor in
//! order. A command that first shows a dialog holds back the later ones
//! until the user has answered.
//!
//! A reload starts a new session. The sessions it replaced are retired:
//! a late message from one (a task that sat in a dialog across the
//! reload) is refused and changes nothing. What the old session had
//! parked is dropped on the reset; a parked item holds its reply channel,
//! so whoever waits for it gets an error instead of waiting forever.
//! Within a live session every number the page draws is admitted exactly
//! once (sent, skipped after a cancelled dialog, or skipped after an
//! error), so a gap closes as soon as its command has finished.

use std::collections::{BTreeMap, VecDeque};

/// Retired sessions remembered (a reload each; older ones are long gone).
const RETIRED: usize = 64;

/// What a sequence number stands for once its turn comes.
#[derive(Debug)]
pub enum Slot<T> {
    /// Deliver this.
    Send(T),
    /// Nothing to deliver (a dialog was cancelled); the turn just passes.
    Skip,
}

/// Releases numbered items in number order.
#[derive(Debug)]
pub struct Sequencer<T> {
    session: String,
    retired: VecDeque<String>,
    next: u64,
    parked: BTreeMap<u64, Slot<T>>,
}

impl<T> Default for Sequencer<T> {
    fn default() -> Self {
        Self {
            session: String::new(),
            retired: VecDeque::new(),
            next: 0,
            parked: BTreeMap::new(),
        }
    }
}

impl<T> Sequencer<T> {
    /// Admit item `seq` of `session`; returns the items now due, in
    /// order. A new session (a reloaded page) starts over at 0, retiring
    /// the current one and dropping what it had parked; a retired session
    /// is refused. A number already seen is refused.
    pub fn admit(&mut self, session: &str, seq: u64, slot: Slot<T>) -> Result<Vec<T>, String> {
        if session != self.session {
            if self.retired.iter().any(|r| r == session) {
                return Err("stale message from a page that has been reloaded".to_string());
            }
            let old = std::mem::replace(&mut self.session, session.to_string());
            if !old.is_empty() {
                self.retired.push_back(old);
                if self.retired.len() > RETIRED {
                    self.retired.pop_front();
                }
            }
            self.next = 0;
            // Dropping parked items closes their reply channels.
            self.parked.clear();
        }
        if seq < self.next || self.parked.contains_key(&seq) {
            return Err(format!(
                "message {seq} out of order (expected {} or later)",
                self.next
            ));
        }
        self.parked.insert(seq, slot);
        let mut due = Vec::new();
        while let Some(s) = self.parked.remove(&self.next) {
            self.next += 1;
            if let Slot::Send(t) = s {
                due.push(t);
            }
        }
        Ok(due)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};

    #[test]
    fn releases_in_order_and_skips() {
        let mut s = Sequencer::default();
        assert_eq!(s.admit("p1", 0, Slot::Send("a")).unwrap(), ["a"]);
        assert!(s.admit("p1", 2, Slot::Send("c")).unwrap().is_empty());
        assert!(s.admit("p1", 3, Slot::Send("d")).unwrap().is_empty());
        assert_eq!(s.admit("p1", 1, Slot::Skip).unwrap(), ["c", "d"]);
        assert!(s.admit("p1", 2, Slot::Send("again")).is_err());
        assert!(s.admit("p1", 5, Slot::Send("f")).unwrap().is_empty());
        assert!(
            s.admit("p1", 5, Slot::Send("f2")).is_err(),
            "a parked number"
        );
        // A reloaded page is a new session: its 1 waits for its 0.
        assert!(s.admit("p2", 1, Slot::Send("next")).unwrap().is_empty());
        assert_eq!(
            s.admit("p2", 0, Slot::Send("new")).unwrap(),
            ["new", "next"]
        );
        // The old page is retired: refused, and nothing changes.
        assert!(s.admit("p1", 6, Slot::Send("late")).is_err());
        assert_eq!(s.admit("p2", 2, Slot::Send("more")).unwrap(), ["more"]);
    }

    /// The reviewed scenario: a command of the old page sits in its
    /// dialog while the page reloads; the new page runs commands 0..=3;
    /// then the old command finishes. It is refused, and the new page's
    /// next commands still go through (nothing parks forever).
    #[test]
    fn a_late_command_from_a_reloaded_page_is_stale() {
        let mut s = Sequencer::default();
        for n in 0..5 {
            assert_eq!(s.admit("old", n, Slot::Send(n)).unwrap(), [n]);
        }
        // "old" 5 is in its dialog; "old" 6 arrived and parks behind it.
        assert!(s.admit("old", 6, Slot::Send(6)).unwrap().is_empty());
        // Reload.
        for n in 0..=3 {
            assert_eq!(s.admit("new", n, Slot::Send(100 + n)).unwrap(), [100 + n]);
        }
        assert!(s.admit("old", 5, Slot::Send(5)).is_err(), "stale");
        assert_eq!(s.admit("new", 4, Slot::Send(104)).unwrap(), [104]);
        assert_eq!(s.admit("new", 5, Slot::Send(105)).unwrap(), [105]);
    }

    /// What a superseded session left parked is dropped, and its waiting
    /// caller hears so at once.
    #[test]
    fn a_reload_releases_what_was_parked() {
        let mut s = Sequencer::default();
        let (tx, rx) = channel::<()>();
        assert!(s.admit("old", 1, Slot::Send(tx)).unwrap().is_empty());
        assert!(s.admit("new", 0, Slot::Skip).unwrap().is_empty());
        assert!(rx.recv().is_err(), "the parked item's channel is closed");
    }

    /// Many commands admitted from racing threads, as Tauri's tasks do,
    /// reach the receiving end (the machine thread's channel) in the
    /// page's order.
    #[test]
    fn racing_senders_keep_the_page_order() {
        const N: u64 = 2000;
        let seq = Arc::new(Mutex::new(Sequencer::default()));
        let (tx, rx) = channel();
        let threads: Vec<_> = (0..8)
            .map(|t| {
                let seq = Arc::clone(&seq);
                let tx = tx.clone();
                std::thread::spawn(move || {
                    // Each thread takes every 8th number, highest first,
                    // so arrivals are as far from the page order as can be.
                    let mine: Vec<u64> = (0..N).filter(|n| n % 8 == t).rev().collect();
                    for n in mine {
                        let slot = if n % 97 == 5 {
                            Slot::Skip
                        } else {
                            Slot::Send(n)
                        };
                        // Number 0 comes last, below.
                        if n == 0 {
                            continue;
                        }
                        let due = seq.lock().unwrap().admit("page", n, slot).unwrap();
                        for d in due {
                            tx.send(d).unwrap();
                        }
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        // Everything waits for number 0.
        assert!(rx.try_recv().is_err());
        for d in seq.lock().unwrap().admit("page", 0, Slot::Send(0)).unwrap() {
            tx.send(d).unwrap();
        }
        drop(tx);
        let got: Vec<u64> = rx.iter().collect();
        let want: Vec<u64> = (0..N).filter(|n| n % 97 != 5).collect();
        assert_eq!(got, want);
    }
}
