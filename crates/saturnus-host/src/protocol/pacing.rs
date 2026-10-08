//! Pacing against the wall clock (`web/protocol.md`, "Pacing"): while the
//! CPU computes or keys are queued, passes run emulated time equal to the
//! elapsed wall time times the speed, dropping what a pass cannot fit into
//! its budget; while it sleeps in SHUTDN with nothing queued, the loop
//! asks to be woken at its next timer event, then runs all the time that
//! passed at 1x (at most [`MAX_BEHIND_MS`]), owing what does not fit the
//! wake's budget to the next passes.
//!
//! [`Loop`] holds that state; it runs a [`Core`] and reads a [`Clock`],
//! and its only output besides the machine's progress is the time it
//! wants to be called again ([`Loop::due`]).

use super::Clock;
use crate::{Emulator, Result};

/// Most emulated time a wake catches up, in ms: 12 hours. Idle time is
/// cheap (the core jumps over SHUTDN; only the ROM's timer ticks run: an
/// idle 48SX hour took 2 ms, 12 hours 9 ms), but owed time the ROM spends
/// awake (an alarm, a running program) is computed for real at the pass
/// budget; the bound keeps a computer that slept for days from paying that
/// off for minutes afterwards.
pub const MAX_BEHIND_MS: f64 = 12.0 * 3600.0 * 1000.0;

/// Longest delay a wake asks for, in ms (a browser timer's limit).
const MAX_DELAY_MS: f64 = 2_147_483_647.0;

/// How a host's pacing is tuned: the rules are the same everywhere, the
/// periods and budgets suit the host's timers. Defined here once for every
/// host ([`Pacing::WORKER`], [`Pacing::NATIVE`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pacing {
    /// Period of the passes while the CPU computes, in ms.
    pub pass_ms: f64,
    /// Wall time one pass may spend emulating at 1x, 2x and 4x.
    pub pass_budget_ms: f64,
    /// Wall time one pass may spend emulating at "Max".
    pub max_budget_ms: f64,
    /// Emulated ms one pass at "Max" may run at most.
    pub max_emulated_ms: f64,
    /// Longest stretch of wall time one pass makes up for.
    pub max_wall_ms: f64,
    /// Wall time one wake may spend catching up.
    pub wake_budget_ms: f64,
    /// The same while the page is hidden, where no frame waits for it.
    pub hidden_wake_budget_ms: f64,
    /// Shortest time between two `frame` events from passes (0: every
    /// pass may send one).
    pub frame_ms: f64,
    /// Wall time one turn of a send may take before the host serves
    /// other messages.
    pub typing_tick_ms: f64,
    /// Whether a hidden page stops the passes of a computing machine (as
    /// it stops animation frames); a native window keeps computing.
    pub pauses_when_hidden: bool,
    /// How early a host's timer may fire and still count as due, in ms (a
    /// browser truncates timer delays to whole milliseconds).
    pub timer_slack_ms: f64,
}

impl Pacing {
    /// The Web Worker's: passes on a ~60 Hz timer, as animation frames
    /// were, each its own frame.
    pub const WORKER: Pacing = Pacing {
        pass_ms: 1000.0 / 60.0,
        pass_budget_ms: 22.0,
        max_budget_ms: 11.0,
        max_emulated_ms: 1000.0,
        max_wall_ms: 100.0,
        wake_budget_ms: 22.0,
        hidden_wake_budget_ms: 200.0,
        frame_ms: 0.0,
        typing_tick_ms: 40.0,
        pauses_when_hidden: true,
        timer_slack_ms: 1.0,
    };

    /// The native machine thread's (the desktop app, `saturnus run`):
    /// passes every millisecond, so the serial bridge between them sees
    /// the UART at its pace, and frames at most every 16 ms.
    pub const NATIVE: Pacing = Pacing {
        pass_ms: 1.0,
        pass_budget_ms: 4.0,
        max_budget_ms: 11.0,
        max_emulated_ms: 1000.0,
        max_wall_ms: 100.0,
        wake_budget_ms: 22.0,
        hidden_wake_budget_ms: 22.0,
        frame_ms: 16.0,
        typing_tick_ms: 16.0,
        pauses_when_hidden: false,
        timer_slack_ms: 0.0,
    };
}

/// The speed setting (`setSpeed`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Speed {
    /// Real time.
    #[default]
    One,
    /// Twice real time.
    Two,
    /// Four times real time.
    Four,
    /// As fast as the host can while staying responsive.
    Max,
}

impl Speed {
    /// The protocol's name: `"1"`, `"2"`, `"4"` or `"max"`; anything else
    /// is real time.
    pub fn parse(s: &str) -> Speed {
        match s {
            "2" => Speed::Two,
            "4" => Speed::Four,
            "max" => Speed::Max,
            _ => Speed::One,
        }
    }

    /// The protocol's name.
    pub fn name(self) -> &'static str {
        match self {
            Speed::One => "1",
            Speed::Two => "2",
            Speed::Four => "4",
            Speed::Max => "max",
        }
    }

    /// Emulated time per wall time while computing (1 at "Max", whose
    /// passes are bounded by budget instead).
    fn factor(self) -> f64 {
        match self {
            Speed::One | Speed::Max => 1.0,
            Speed::Two => 2.0,
            Speed::Four => 4.0,
        }
    }
}

impl serde::Serialize for Speed {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

/// What the loop does (the `loop` field of `status` and `stats`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LoopState {
    /// Passes run (the CPU computes or keys are queued).
    Frame,
    /// The CPU sleeps until a timer event.
    Sleep,
    /// Nothing runs (no machine, paused, halted, hidden while computing,
    /// or a send drives the machine).
    Stopped,
}

/// The machine as the loop sees it.
pub(crate) trait Core {
    /// One slice of at most `left_ms` emulated ms, feeding the key queue
    /// if `keys`; the emulated ms run (more than zero).
    fn run_slice(&mut self, left_ms: f64, keys: bool) -> Result<f64>;
    /// Emulated ms the CPU sleeps before its next event; `None` while it
    /// computes.
    fn idle_ms(&self) -> Option<f64>;
    /// Keys are down or queued.
    fn keys_busy(&self) -> bool;
    /// CPU cycles since power-on.
    fn cycles(&self) -> u64;
    /// The user memory's change counter.
    fn memory_changes(&self) -> Result<u64>;
}

impl Core for Emulator {
    fn run_slice(&mut self, left_ms: f64, keys: bool) -> Result<f64> {
        Emulator::run_slice(self, left_ms, keys)
    }
    fn idle_ms(&self) -> Option<f64> {
        Emulator::idle_ms(self)
    }
    fn keys_busy(&self) -> bool {
        Emulator::keys_busy(self)
    }
    fn cycles(&self) -> u64 {
        self.machine().cycles()
    }
    fn memory_changes(&self) -> Result<u64> {
        Emulator::memory_changes(self)
    }
}

/// The CPU sleeps in SHUTDN with no keys queued.
pub(crate) fn asleep(core: &impl Core) -> bool {
    core.idle_ms().is_some() && !core.keys_busy()
}

/// The timer the loop waits for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Timer {
    /// The next pass, at this time.
    Pass(f64),
    /// The wake from a sleep, at this time.
    Wake(f64),
}

/// The run loop's state.
#[derive(Clone, Debug)]
pub(crate) struct Loop {
    pub pacing: Pacing,
    /// The machine runs (a ROM is booted, not paused, not halted).
    pub running: bool,
    pub speed: Speed,
    /// The page is hidden (only if [`Pacing::pauses_when_hidden`]).
    pub hidden: bool,
    timer: Option<Timer>,
    /// Wall clock up to which the passes accounted for emulated time.
    last_pass: Option<f64>,
    /// Wall clock up to which emulated time is accounted for while asleep.
    slept_at: f64,
    /// Emulated ms owed to wall time: what a wake could not catch up
    /// within its budget. Passes and further wakes run it off first.
    behind_ms: f64,
    /// Wall ms spent emulating.
    pub work_ms: f64,
    /// Passes run.
    pub ticks: u64,
    /// Wakes from a sleep.
    pub wakes: u64,
}

impl Loop {
    pub fn new(pacing: Pacing) -> Loop {
        Loop {
            pacing,
            running: false,
            speed: Speed::One,
            hidden: false,
            timer: None,
            last_pass: None,
            slept_at: 0.0,
            behind_ms: 0.0,
            work_ms: 0.0,
            ticks: 0,
            wakes: 0,
        }
    }

    /// When the loop wants [`Loop::fire`] called, on the host's clock.
    pub fn due(&self) -> Option<f64> {
        match self.timer {
            Some(Timer::Pass(t) | Timer::Wake(t)) => Some(t),
            None => None,
        }
    }

    pub fn state(&self) -> LoopState {
        match self.timer {
            Some(Timer::Pass(_)) => LoopState::Frame,
            Some(Timer::Wake(_)) => LoopState::Sleep,
            None => LoopState::Stopped,
        }
    }

    /// Passes are due (the loop does not stop between them).
    pub fn passing(&self) -> bool {
        matches!(self.timer, Some(Timer::Pass(_)))
    }

    /// The CPU sleeps on a wake timer.
    pub fn sleeping(&self) -> bool {
        matches!(self.timer, Some(Timer::Wake(_)))
    }

    /// Passes stop while the page is hidden.
    fn held(&self) -> bool {
        self.hidden && self.pacing.pauses_when_hidden
    }

    /// Emulated ms owed to the wall clock at `now`: unpaid, plus the
    /// current sleep.
    pub fn owed_ms(&self, now: f64) -> f64 {
        self.behind_ms
            + if self.sleeping() {
                now - self.slept_at
            } else {
                0.0
            }
    }

    /// Stop: no timer, nothing owed.
    pub fn stop(&mut self) {
        self.timer = None;
        self.last_pass = None;
        self.behind_ms = 0.0;
    }

    /// Start after [`Loop::stop`]: a pass at once, or (hidden) the sleep
    /// or nothing.
    pub fn start(&mut self, core: &impl Core, now: f64) {
        if !self.running {
            return;
        }
        if self.held() {
            self.schedule(core, now);
        } else {
            self.timer = Some(Timer::Pass(now));
        }
    }

    /// The page was hidden or shown; `idle` means the loop may start (no
    /// send drives the machine).
    pub fn set_hidden(&mut self, hidden: bool, core: Option<&impl Core>, idle: bool, now: f64) {
        if !self.pacing.pauses_when_hidden {
            return;
        }
        self.hidden = hidden;
        if let Some(core) = core
            && !hidden
            && self.running
            && self.timer.is_none()
            && idle
        {
            self.schedule(core, now);
        }
    }

    /// Run whichever timer is due at `now`; an error is a halted CPU.
    pub fn fire(&mut self, core: &mut impl Core, clock: &dyn Clock, now: f64) -> Result<()> {
        let slack = self.pacing.timer_slack_ms;
        match self.timer {
            Some(Timer::Pass(t)) if t <= now + slack => self.pass(core, clock),
            Some(Timer::Wake(t)) if t <= now + slack => self.wake(core, clock),
            _ => Ok(()),
        }
    }

    /// Run `ms` emulated ms in slices, feeding the keys if `keys`, within
    /// `budget_ms` of wall time, and only until the CPU sleeps with nothing
    /// queued if `until_sleep`; returns the emulated ms left unrun.
    fn run_slices(
        &mut self,
        core: &mut impl Core,
        clock: &dyn Clock,
        ms: f64,
        budget_ms: f64,
        keys: bool,
        until_sleep: bool,
    ) -> Result<f64> {
        let start = clock.now_ms();
        let mut left = ms;
        let result = (|| {
            // Checked before each slice: a pass that starts asleep runs
            // nothing.
            while left > 0.0 && !(until_sleep && asleep(core)) {
                left -= core.run_slice(left, keys)?;
                if clock.now_ms() - start > budget_ms {
                    break;
                }
            }
            Ok(())
        })();
        self.work_ms += clock.now_ms() - start;
        result.map(|()| left.max(0.0))
    }

    /// One pass while the CPU computes: time owed from a sleep first (wall
    /// time, at 1x), kept until paid; then the pass's own share at the
    /// speed until the CPU sleeps. What does not fit the budget is
    /// dropped; the wall time a sleep left over is the sleep's, at 1x.
    fn pass(&mut self, core: &mut impl Core, clock: &dyn Clock) -> Result<()> {
        self.timer = None;
        let t = clock.now_ms();
        let wall = self
            .last_pass
            .map_or(0.0, |l| (t - l).clamp(0.0, self.pacing.max_wall_ms));
        self.last_pass = Some(t);
        if !self.running {
            return Ok(());
        }
        self.ticks += 1;
        let max = self.speed == Speed::Max;
        let budget = if max {
            self.pacing.max_budget_ms
        } else {
            self.pacing.pass_budget_ms
        };
        self.behind_ms = self.run_slices(core, clock, self.behind_ms, budget, true, false)?;
        if self.behind_ms == 0.0 {
            let factor = self.speed.factor();
            let own = if max {
                self.pacing.max_emulated_ms
            } else {
                wall * factor
            };
            let rest = (budget - (clock.now_ms() - t)).max(0.0);
            let left = self.run_slices(core, clock, own, rest, true, true)?;
            if !max && asleep(core) {
                self.last_pass = Some(t - left / factor);
            }
        }
        self.schedule(core, clock.now_ms());
        Ok(())
    }

    /// Keep passing while the CPU computes or keys are queued; once it
    /// sleeps with nothing to do, set the wake timer for its next timer
    /// event instead, so an idle machine costs nothing. Time still owed
    /// brings the wake forward to now.
    pub fn schedule(&mut self, core: &impl Core, now: f64) {
        if !self.running {
            return;
        }
        match core.idle_ms() {
            Some(idle) if !core.keys_busy() => {
                // The last pass or wake accounted for wall time up to
                // `last_pass`.
                self.slept_at = self.last_pass.unwrap_or(now);
                self.last_pass = None;
                // Asleep, emulated time runs at 1x at any speed.
                let delay = if self.behind_ms > 0.0 {
                    0.0
                } else {
                    idle.min(MAX_DELAY_MS) + 1.0
                };
                self.timer = Some(Timer::Wake(now + delay));
            }
            _ => {
                if !self.passing() && !self.held() {
                    let due = self
                        .last_pass
                        .map_or(now, |l| (l + self.pacing.pass_ms).max(now));
                    self.timer = Some(Timer::Pass(due));
                }
            }
        }
    }

    /// Leave the sleep: run all the emulated time that passed meanwhile
    /// (cheap while the CPU sleeps), however late the timer fired. What
    /// does not fit the budget, as when the ROM wakes up and computes,
    /// stays owed for the next pass or wake. Nothing if not asleep.
    pub fn wake(&mut self, core: &mut impl Core, clock: &dyn Clock) -> Result<()> {
        if !self.running || !self.sleeping() {
            return Ok(());
        }
        self.timer = None;
        self.wakes += 1;
        let now = clock.now_ms();
        self.behind_ms = (self.behind_ms + (now - self.slept_at)).min(MAX_BEHIND_MS);
        let budget = if self.hidden {
            self.pacing.hidden_wake_budget_ms
        } else {
            self.pacing.wake_budget_ms
        };
        // A key that woke the machine goes down after the time that passed.
        self.behind_ms = self.run_slices(core, clock, self.behind_ms, budget, false, false)?;
        self.last_pass = Some(now);
        // The ROM may have woken for a timer event and redrawn (the
        // clock), then gone back to sleep: sleep on without a pass.
        self.schedule(core, clock.now_ms());
        Ok(())
    }
}
