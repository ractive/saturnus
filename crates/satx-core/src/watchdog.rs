//! The no-progress watchdog of the Kermit and XModem transfer loops.
//!
//! The protocol machines end a transaction themselves when a packet runs
//! out of retries. The watchdog is the backstop above them: a peer that
//! keeps the line busy without moving the transfer on (a packet replayed
//! forever, noise at the right rate) ends once the client's own clock has
//! seen no progress for longer than the machine could need to give up on
//! one packet. Progress resets it, so a long transfer that moves on, or a
//! slow command that answers within the machine's own timeouts, never
//! trips it. Sans-I/O: it only compares the caller's clock.

use crate::time::{Duration, Instant};

/// When a transfer without progress is given up.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Watchdog {
    /// Time allowed between two signs of progress.
    limit: Duration,
    /// When the transfer is given up; `None`: never (a limit too large for
    /// the clock).
    deadline: Option<Instant>,
}

impl Watchdog {
    /// A watchdog started at `now` that allows `first` until the first
    /// sign of progress, then `limit` between two.
    pub(crate) fn new(now: Instant, first: Duration, limit: Duration) -> Self {
        Watchdog {
            limit,
            deadline: now.checked_add(first),
        }
    }

    /// The transfer moved on at `now`.
    pub(crate) fn progress(&mut self, now: Instant) {
        self.deadline = now.checked_add(self.limit);
    }

    /// When the transfer is given up without progress.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Whether `now` is past the deadline.
    pub(crate) fn expired(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|d| now >= d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_moves_the_deadline_and_silence_expires_it() {
        let t0 = Instant::now();
        let s = Duration::from_secs(1);
        let mut w = Watchdog::new(t0, 3 * s, 2 * s);
        assert_eq!(w.deadline(), Some(t0 + 3 * s));
        assert!(!w.expired(t0 + 2 * s));
        // Progress every second for a long time: never expires.
        for i in 1..100 {
            let now = t0 + i * s;
            assert!(!w.expired(now));
            w.progress(now);
        }
        let last = t0 + 99 * s;
        assert_eq!(w.deadline(), Some(last + 2 * s));
        assert!(!w.expired(last + 2 * s - Duration::from_millis(1)));
        assert!(w.expired(last + 2 * s));
    }

    #[test]
    fn a_limit_beyond_the_clock_never_expires() {
        let t0 = Instant::now();
        let w = Watchdog::new(t0, Duration::MAX, Duration::MAX);
        assert_eq!(w.deadline(), None);
        assert!(!w.expired(t0 + Duration::from_secs(1 << 20)));
    }
}
