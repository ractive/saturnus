//! Wall-clock pacing: [`Pacer`] keeps the emulated cycle count at
//! `clock_hz` cycles per real second (times a speed factor), sleeping when
//! ahead and catching up (boundedly) when behind. Used by the CLI's serial
//! bridge, whose Kermit timeouts are wall-clock on both ends, and by the
//! Tauri app's machine thread.

use std::time::{Duration, Instant};

/// Emulated time is re-anchored when it falls further behind wall-clock
/// time than this (a slow host, a debugger stop): catching up a long lag
/// would run the calculator in a burst that no real one ever does.
pub const MAX_LAG: Duration = Duration::from_millis(200);
/// Longest stretch of emulated time one [`Pacer::budget`] hands out.
pub const SLICE: Duration = Duration::from_millis(1);

/// Maps wall-clock time to a target cycle count.
#[derive(Clone, Copy, Debug)]
pub struct Pacer {
    /// Emulated cycles per wall second: the clock times the speed factor.
    rate_hz: u64,
    start: Instant,
    start_cycles: u64,
    /// Times the anchor was moved because the machine fell behind.
    pub rebases: u64,
}

impl Pacer {
    /// Start pacing now, at `cycles`, in real time.
    pub fn new(clock_hz: u32, now: Instant, cycles: u64) -> Self {
        Self::with_speed(clock_hz, 1, now, cycles)
    }

    /// Start pacing now, at `cycles`, running `speed` times real time (the
    /// calculator's clock runs fast too).
    pub fn with_speed(clock_hz: u32, speed: u32, now: Instant, cycles: u64) -> Self {
        Self {
            rate_hz: u64::from(clock_hz) * u64::from(speed.max(1)),
            start: now,
            start_cycles: cycles,
            rebases: 0,
        }
    }

    /// Cycles corresponding to `d` of wall time.
    pub fn cycles_in(&self, d: Duration) -> u64 {
        let c = d.as_nanos() * u128::from(self.rate_hz) / 1_000_000_000;
        u64::try_from(c).unwrap_or(u64::MAX)
    }

    /// The wall-clock instant at which the machine should be at `cycles`:
    /// how far the wall time is accounted for.
    pub fn instant_of(&self, cycles: u64) -> Instant {
        let ahead = cycles.saturating_sub(self.start_cycles);
        let nanos = u128::from(ahead) * 1_000_000_000 / u128::from(self.rate_hz.max(1));
        let d = Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX));
        self.start.checked_add(d).unwrap_or(self.start)
    }

    /// Follow the wall clock from `now`, with the machine at `cycles`
    /// (after a pause or a sleep the caller accounted for itself).
    pub fn rebase(&mut self, now: Instant, cycles: u64) {
        self.start = now;
        self.start_cycles = cycles;
    }

    /// How many cycles to run at `now` with the machine at `cycles`, at
    /// most one [`SLICE`]. Zero means the machine is ahead: sleep.
    pub fn budget(&mut self, now: Instant, cycles: u64) -> u64 {
        let target = self
            .start_cycles
            .saturating_add(self.cycles_in(now.saturating_duration_since(self.start)));
        let behind = target.saturating_sub(cycles);
        if behind > self.cycles_in(MAX_LAG) {
            self.rebase(now, cycles);
            self.rebases += 1;
            return self.cycles_in(SLICE);
        }
        behind.min(self.cycles_in(SLICE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacer_follows_wall_clock_and_rebases_long_lags() {
        let t0 = Instant::now();
        let mut p = Pacer::new(2_000_000, t0, 1000);
        // Nothing elapsed: ahead, sleep.
        assert_eq!(p.budget(t0, 1000), 0);
        // 0.5 ms behind: run exactly that.
        assert_eq!(p.budget(t0 + Duration::from_micros(500), 1000), 1000);
        // 50 ms behind: at most one slice per call.
        assert_eq!(p.budget(t0 + Duration::from_millis(50), 1000), 2000);
        // Ahead of the clock: sleep.
        assert_eq!(p.budget(t0 + Duration::from_millis(1), 10_000), 0);
        assert_eq!(p.rebases, 0);
        // A second behind: re-anchor, then follow the clock from there.
        let t1 = t0 + Duration::from_secs(1);
        assert_eq!(p.budget(t1, 1000), 2000);
        assert_eq!(p.rebases, 1);
        assert_eq!(p.budget(t1 + Duration::from_micros(250), 1000), 500);
    }

    #[test]
    fn speed_scales_the_rate() {
        let t0 = Instant::now();
        let mut p = Pacer::with_speed(2_000_000, 4, t0, 0);
        assert_eq!(p.budget(t0 + Duration::from_micros(100), 0), 800);
        // The slice is wall time: four times the cycles at 4x.
        assert_eq!(p.budget(t0 + Duration::from_millis(10), 0), 8000);
        assert_eq!(p.instant_of(8000), t0 + Duration::from_millis(1));
        p.rebase(t0 + Duration::from_secs(5), 123);
        assert_eq!(p.budget(t0 + Duration::from_secs(5), 123), 0);
    }
}
