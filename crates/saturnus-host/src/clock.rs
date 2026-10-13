//! Setting the calculator's clock (`setClock` in `web/protocol.md`) the
//! way its owner would: by its own commands and menus, typed or pressed,
//! never by writing the ROM's clock in RAM. Which models have a clock,
//! and how each sets it, from the manuals: wiki: hardware/clocks.
//!
//! - 48SX, 48GX, 49G: `H.MMSS →TIME` and the date `→DATE`, in the order
//!   flag -42 asks for (MM.DDYYYY clear, DD.MMYYYY set), with a comma for
//!   the point when flag -51 is set; typed as one line in RPN mode, or as
//!   `→TIME(…)` and `→DATE(…)` in the 49G's algebraic mode. Both commands
//!   take their argument, so the stack is as it was; whatever an error
//!   leaves is dropped ([`crate::transfer::Op::SetClock`]).
//! - 38G, 39G, 40G: in HOME, `H.MMSS STO▶ TIME` and `M.DDYYYY STO▶ DATE`
//!   (their manuals' DISPTIME entry); the two lines each store adds to the
//!   HOME history are deleted again (▲, DEL, DEL, ▼).
//! - 27S, 17BII, 17B, 19BII: the TIME application's SET menu: the date keyed in
//!   the calculator's own format and DATE, then the time in 24-hour form
//!   and TIME (12/24 pressed around it when the clock shows 12-hour time),
//!   then back out.
//!
//! The 42S has no clock. A calculator that is off, busy or has a command
//! line open is refused rather than typed into.

use saturnus_core::{Machine, Model};
use serde_json::Value;

use crate::Emulator;
use crate::transfer::Op;

/// Why a calculator with a command line open is refused.
pub const LINE_OPEN: &str = "Finish what you're typing first.";
/// Why a calculator that is off is refused.
pub const OFF: &str = "The calculator is off. Turn it on first.";
/// Why a busy calculator is refused.
pub const BUSY: &str = "The calculator is busy. Try again when it has finished.";

/// The earliest and latest year every clock takes: the 48's `→DATE`
/// takes January 1, 1991 to December 31, 2090 (48G AUR, `→DATE`).
pub const YEARS: std::ops::RangeInclusive<u16> = 1991..=2090;

/// A local date and time to set, to the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

fn days_in(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl ClockTime {
    /// A checked date and time.
    pub fn new(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> crate::Result<ClockTime> {
        if !YEARS.contains(&year) {
            return Err(format!(
                "the year must be between {} and {}",
                YEARS.start(),
                YEARS.end()
            )
            .into());
        }
        if !(1..=12).contains(&month) || day == 0 || day > days_in(year, month) {
            return Err(format!("{year}-{month:02}-{day:02} is not a date").into());
        }
        if hour > 23 || minute > 59 || second > 59 {
            return Err(format!("{hour:02}:{minute:02}:{second:02} is not a time").into());
        }
        Ok(ClockTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// From a command's `year`, `month`, `day`, `hour`, `minute` and
    /// `second` fields.
    pub fn from_json(msg: &Value) -> crate::Result<ClockTime> {
        let field = |k: &str| -> crate::Result<u16> {
            msg.get(k)
                .and_then(Value::as_u64)
                .and_then(|v| u16::try_from(v).ok())
                .ok_or_else(|| format!("missing number field {k:?}").into())
        };
        let small = |k: &str| -> crate::Result<u8> {
            u8::try_from(field(k)?).map_err(|_| format!("{k:?} is out of range").into())
        };
        ClockTime::new(
            field("year")?,
            small("month")?,
            small("day")?,
            small("hour")?,
            small("minute")?,
            small("second")?,
        )
    }

    /// This time `secs` seconds later (earlier if negative); the date
    /// rolls over either way.
    pub fn shifted(self, secs: i64) -> ClockTime {
        let day = days_from_civil(self.year, self.month, self.day);
        let total = day * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
            + secs;
        let (year, month, day) = civil_from_days(total.div_euclid(86_400));
        let rest = total.rem_euclid(86_400);
        ClockTime {
            year,
            month,
            day,
            hour: (rest / 3600) as u8,
            minute: (rest / 60 % 60) as u8,
            second: (rest % 60) as u8,
        }
    }

    /// The time as the calculators key it, H.MMSS in 24-hour form
    /// (`9.0830` for 9:08:30, `0.0005` five seconds after midnight).
    pub fn time_number(&self) -> String {
        format!("{}.{:02}{:02}", self.hour, self.minute, self.second)
    }

    /// The date as MM.DDYYYY (`day_first`: DD.MMYYYY), without the
    /// leading zero (`6.092026`).
    pub fn date_number(&self, day_first: bool) -> String {
        let (a, b) = if day_first {
            (self.day, self.month)
        } else {
            (self.month, self.day)
        };
        format!("{a}.{b:02}{}", self.year)
    }
}

/// Days since 1 January 1970 of a proleptic Gregorian date (the usual
/// era arithmetic: a 400-year era has 146 097 days).
fn days_from_civil(year: u16, month: u8, day: u8) -> i64 {
    let y = i64::from(year) - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day number from [`days_from_civil`].
fn civil_from_days(days: i64) -> (u16, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u8;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year.clamp(0, i64::from(u16::MAX)) as u16, month, day)
}

/// Whether `model` has a clock a user sets (wiki: hardware/clocks; the
/// 19BII: hardware/hp19bii "Clock"): not
/// the 42S, nor the 28S, whose reference manual has no time or date
/// command (only a random seed "based upon the system clock").
pub fn has_clock(model: Model) -> bool {
    matches!(
        model,
        Model::Hp48sx
            | Model::Hp48gx
            | Model::Hp49g
            | Model::Hp38g
            | Model::Hp39g
            | Model::Hp40g
            | Model::Hp27s
            | Model::Hp17bii
            | Model::Hp17b
            | Model::Hp19bii
    )
}

/// Whether `model` sets its clock by RPL commands typed on its command
/// line (and so through [`crate::transfer`]), rather than by keys.
pub fn by_command(model: Model) -> bool {
    matches!(model, Model::Hp48sx | Model::Hp48gx | Model::Hp49g)
}

/// The lines the 48 and 49 run to set `time` and `date`: one in RPN mode, one per
/// command in the 49G's algebraic mode. `day_first`: flag -42 set;
/// `comma`: flag -51 set (a comma is the fraction mark).
pub fn rpl_lines(
    time: &ClockTime,
    date: &ClockTime,
    day_first: bool,
    comma: bool,
    algebraic: bool,
) -> Vec<String> {
    let mark = |s: String| if comma { s.replace('.', ",") } else { s };
    let time = mark(time.time_number());
    let date = mark(date.date_number(day_first));
    if algebraic {
        vec![format!("→TIME({time})"), format!("→DATE({date})")]
    } else {
        vec![format!("{time} →TIME {date} →DATE")]
    }
}

/// One step of a key plan: a key's full press, or a letter typed through
/// alpha mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Key(&'static str),
    Letter(char),
}

/// The keys of a number: digits and the point key.
fn number_keys(text: &str) -> Vec<Press> {
    text.chars()
        .filter_map(|c| match c {
            '.' => Some(Press::Key("point")),
            '0'..='9' => DIGITS
                .get(usize::from(c as u8 - b'0'))
                .map(|&d| Press::Key(d)),
            _ => None,
        })
        .collect()
}

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// The nibble of the TIME SET modes on the Pioneers and the 19BII: bit 0
/// set for day.month.year, bit 2 set for the 24-hour clock. Found by
/// pressing M/D and 12/24 in saturnus and watching RAM (the owner's ROMs;
/// wiki: hardware/clocks, hardware/hp19bii "Clock").
fn pioneer_modes(model: Model) -> Option<u32> {
    match model {
        Model::Hp27s => Some(0x50025),
        Model::Hp17bii => Some(0x5002B),
        Model::Hp17b => Some(0x5002A),
        Model::Hp19bii => Some(0xC41E1),
        _ => None,
    }
}

/// Why an international 17B or 17BII is refused before its language is
/// chosen: its first screen after a memory loss asks for one, and takes
/// no other key.
pub const NO_LANGUAGE: &str = "Choose the calculator's language first.";

/// Whether a 17B or 17BII still asks for its language: an international
/// ROM (its language ROM ends "OK-17B-L" at #2FFE6, wiki:
/// hardware/clocks) whose nibble at #50092 (17BII) or #50091 (17B)
/// reads 0. That nibble is 0 only on the language prompt and 2 in every
/// menu after it (seen in saturnus on the owner's ROMs; what else it
/// holds is not known). Every 19BII image known is international and
/// asks after a memory loss; its nibble is #C402B, 0 only on that prompt
/// and 2 after it (seen the same way; wiki: hardware/hp19bii).
fn asks_language(m: &Machine) -> bool {
    if m.model() == Model::Hp19bii {
        return m.peek(0xC402B) & 0xF == 0;
    }
    let at = match m.model() {
        Model::Hp17bii => 0x50092,
        Model::Hp17b => 0x50091,
        _ => return false,
    };
    let mark: Vec<u8> = (0..8)
        .map(|i| (m.peek(0x2FFE6 + 2 * i) & 0xF) | ((m.peek(0x2FFE7 + 2 * i) & 0xF) << 4))
        .collect();
    mark == b"OK-17B-L" && m.peek(at) & 0xF == 0
}

/// The presses that set the clock on a model set by keys, cut where the
/// calculator takes the time and the date.
#[derive(Clone, Debug, PartialEq)]
struct Plan {
    keys: Vec<Press>,
    /// The presses up to and including the one that commits the time.
    time_at: usize,
    /// The same for the date.
    date_at: usize,
}

/// The presses that key `time` and `date`; on the aplet models the time
/// before the date if `time_first`.
fn key_plan(
    m: &Machine,
    time: &ClockTime,
    date: &ClockTime,
    time_first: bool,
) -> crate::Result<Plan> {
    let model = m.model();
    let mut keys = Vec::new();
    let (time_at, date_at);
    if let Some(modes) = pioneer_modes(model) {
        let mode = u32::from(m.peek(modes) & 0xF);
        if mode & 0b1010 != 0 {
            return Err("the clock's settings cannot be read on this ROM".into());
        }
        let day_first = mode & 1 != 0;
        let twelve = mode & 4 == 0;
        // Into TIME, then SET (the fourth menu key).
        if model == Model::Hp27s {
            keys.extend([Press::Key("shift"), Press::Key("divide")]);
        } else {
            // MAIN, then its fourth label, TIME.
            keys.extend([Press::Key("shift"), Press::Key("exit"), Press::Key("d")]);
        }
        keys.push(Press::Key("d"));
        keys.extend(number_keys(&date.date_number(day_first)));
        keys.push(Press::Key("a"));
        date_at = keys.len();
        if twelve {
            keys.push(Press::Key("e"));
        }
        keys.extend(number_keys(&time.time_number()));
        keys.push(Press::Key("b"));
        time_at = keys.len();
        if twelve {
            keys.push(Press::Key("e"));
        }
        if model == Model::Hp27s {
            keys.extend([Press::Key("exit"), Press::Key("exit")]);
        } else {
            keys.extend([Press::Key("shift"), Press::Key("exit")]);
        }
    } else {
        // 38G, 39G, 40G: HOME, then STO▶ (the first menu key) into TIME
        // and DATE, each store's two history lines deleted. How long the
        // 38G's queue waits for its ROM to go idle varies with where the
        // host's slices fall (its set took 6.9 to 8.4 s to the time's
        // commit with the date first, by pass length alone), so the
        // rehearsal cannot pin it down. At 1x only the commit counts:
        // the time first, little before it. Faster, the rest of the set
        // counts too (`keyed`): the time last, little after it.
        keys.push(Press::Key("home"));
        let store = |keys: &mut Vec<Press>, number: String, name: &str| {
            keys.extend(number_keys(&number));
            keys.push(Press::Key("a"));
            keys.extend(name.chars().map(Press::Letter));
            keys.push(Press::Key("enter"));
            keys.len()
        };
        let forget = |keys: &mut Vec<Press>| {
            keys.extend(["up", "del", "del", "down"].map(Press::Key));
        };
        if time_first {
            time_at = store(&mut keys, time.time_number(), "TIME");
            forget(&mut keys);
            date_at = store(&mut keys, date.date_number(false), "DATE");
        } else {
            date_at = store(&mut keys, date.date_number(false), "DATE");
            forget(&mut keys);
            time_at = store(&mut keys, time.time_number(), "TIME");
        }
        forget(&mut keys);
    }
    Ok(Plan {
        keys,
        time_at,
        date_at,
    })
}

/// The moment the clock is set, as the page read it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Now {
    /// The local date and time, to the second.
    pub time: ClockTime,
    /// Milliseconds past `time`'s second, 0 to 999.
    pub ms: f64,
    /// The host's clock ([`crate::protocol::Clock`]) when the request
    /// came: the time spent rehearsing counts from there.
    pub wall_ms: f64,
}

/// How fast the host runs the set against the wall clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    /// Emulated ms per wall ms the host aims for while the set runs: the
    /// speed (1, 2, 4), what "Max" allows, or infinity for a write, which
    /// runs as fast as the host can.
    pub speed: f64,
    /// The share of the wall time the host spends emulating (a pass's
    /// budget over its period, at most 1).
    pub share: f64,
    /// Emulated ms one of the host's passes runs while keys play (its
    /// period times the speed; at "Max" what a pass may run): the
    /// rehearsal runs its slices in the same steps, since where a slice
    /// ends decides when the queue sees the ROM idle.
    pub pass_ms: f64,
}

impl Pace {
    /// Real time on a host that keeps up, in the Worker's passes: the
    /// tests' pace at 1x.
    pub const REAL_TIME: Pace = Pace {
        speed: 1.0,
        share: 1.0,
        pass_ms: 1000.0 / 60.0,
    };
    /// As fast as the host can (a write, or "Max" on a fast host).
    pub const UNBOUNDED: Pace = Pace {
        speed: f64::INFINITY,
        share: 1.0,
        pass_ms: 1000.0,
    };

    /// Emulated ms per wall ms the set will run at: the speed aimed for,
    /// unless the host is slower than that (`emulated_ms` of the rehearsal
    /// took `wall_ms`, the host's speed with nothing else to do).
    fn rate(&self, emulated_ms: f64, wall_ms: f64) -> f64 {
        let host = if wall_ms > 0.0 {
            emulated_ms / wall_ms * self.share
        } else {
            f64::INFINITY
        };
        self.speed.min(host).max(f64::MIN_POSITIVE)
    }
}

/// When a set's commits fall, in emulated ms from its start, found by
/// playing it on a copy of the machine: the time taken, the date taken,
/// and the set done (the keys played, the CPU asleep).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub time_ms: f64,
    pub date_ms: f64,
    pub end_ms: f64,
}

/// What a set keyed, and why: for the host's log and the tests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lead {
    /// The rehearsal's timing of the set that was keyed.
    pub timing: Timing,
    /// Emulated ms per wall ms the set is taken to run at.
    pub rate: f64,
    /// Wall ms from the request to the set's start (the rehearsals).
    pub planning_ms: f64,
    /// The time keyed.
    pub time: ClockTime,
    /// The date keyed (its day; the 48's and 49's time of day too).
    pub date: ClockTime,
}

/// What to key for `now` given the rehearsal's `timing`, the set
/// starting `planning_ms` of wall time after `now` and running at `rate`
/// emulated ms per wall ms.
///
/// The calculator's clock runs in emulated time, and the host runs
/// emulated time at `rate` only while the set computes or keys are
/// queued; once the CPU sleeps again (the set done) it follows the wall
/// clock at 1x whatever the speed (`web/protocol.md`, "Pacing"). So the
/// time keyed, `K`, taken at emulated `c` from the set's start, reads
/// `K + (end - c)` when the set is done, at wall time
/// `now + planning + end / rate`, and runs with the wall clock from there.
/// For it to read the wall clock then: `K = now + planning + c / rate -
/// (end - c) * (1 - 1 / rate)`. At 1x that is `now + planning + c`, the
/// time when it is taken; at Max and for a write (a rate far above 1) it
/// is about `now + planning - (end - c)`, as the rest of the set flies by.
/// A key commits a whole second; rounding to the nearest leaves the clock
/// within half a second of the wall clock.
///
/// The date is that of the clock when the date is taken: the time keyed
/// plus the emulated ms between the two, if the date comes after it (the
/// 38G's STO▶ DATE); if before (the Pioneers'), keying the time leaves the
/// date as it was, the date of the time keyed. Not covered: a Pioneer
/// whose own clock passes midnight in the second or two between its DATE
/// and its TIME, which then shows the next day.
fn keyed(now: &Now, timing: &Timing, rate: f64, planning_ms: f64) -> (ClockTime, ClockTime) {
    let c = timing.time_ms;
    let after = (timing.end_ms - c).max(0.0);
    let lead = planning_ms + c / rate - after * (1.0 - 1.0 / rate);
    let at = |ms: f64| now.time.shifted(((now.ms + ms) / 1000.0).round() as i64);
    let between = (timing.date_ms - c).max(0.0);
    (at(lead), at(lead + between))
}

/// Why a set is refused whose rehearsal never ended.
pub const NOT_SETTLED: &str =
    "The clock could not be set: the calculator did not finish the keys. Try again.";

/// Most emulated time a rehearsal may run, in ms: far above any set.
const REHEARSAL_CAP_MS: f64 = 120_000.0;

/// Queue `keys` on `e` (no pump).
fn queue_presses(e: &mut Emulator, keys: &[Press]) -> crate::Result<()> {
    for p in keys {
        match *p {
            Press::Key(k) => e.queue().type_keys(&[k]),
            Press::Letter(c) => {
                if !e.queue().type_letter(c) {
                    return Err(format!("cannot type {c:?} on this model").into());
                }
            }
        }
    }
    Ok(())
}

/// Presses `keys` takes: a letter two (alpha, then its key: the plans
/// type their letters with alpha off, each letter after a key that used
/// it up).
fn presses(keys: &[Press]) -> u64 {
    keys.iter()
        .map(|p| match p {
            Press::Key(_) => 1,
            Press::Letter(_) => 2,
        })
        .sum()
}

/// Play `plan` on a copy of `on` as the host plays queued keys, the whole
/// plan queued at once: each commit is taken somewhere between its key
/// going down and the CPU asleep after it (or the next key down,
/// whichever comes first); the middle of the two counts. And when the
/// set is done: the keys played, the CPU asleep.
fn rehearse_keys(on: &Emulator, plan: &Plan, pass_ms: f64) -> crate::Result<Timing> {
    let mut e = on.rehearsal();
    queue_presses(&mut e, &plan.keys)?;
    let marks = [
        presses(&plan.keys[..plan.time_at]),
        presses(&plan.keys[..plan.date_at]),
    ];
    let mut down = [None::<f64>; 2];
    let mut taken = [None::<f64>; 2];
    let start = e.emulated_ms();
    let base = e.queue().presses_started();
    e.pump();
    // What is left of the current pass.
    let mut pass = pass_ms;
    loop {
        let ran = e.emulated_ms() - start;
        let n = e.queue().presses_started() - base;
        let asleep = e.idle_ms().is_some();
        for i in 0..2 {
            // Taken checked first: the key went down at the end of an
            // earlier slice, and the CPU has run since.
            if down[i].is_some() && taken[i].is_none() && (n > marks[i] || asleep) {
                taken[i] = Some(ran);
            }
            if down[i].is_none() && n >= marks[i] {
                down[i] = Some(ran);
            }
        }
        if !e.keys_busy() && asleep {
            let commit = |i: usize| match (down[i], taken[i]) {
                (Some(d), Some(t)) => (d + t) / 2.0,
                _ => ran,
            };
            return Ok(Timing {
                time_ms: commit(0),
                date_ms: commit(1),
                end_ms: ran,
            });
        }
        if ran > REHEARSAL_CAP_MS {
            return Err(NOT_SETTLED.into());
        }
        if pass <= 0.0 {
            pass = pass_ms;
        }
        pass -= e.run_slice(pass, true)?;
    }
}

/// Run the 48's or 49's clock write on a copy of `on`: when each of its
/// lines has run (the time's `→TIME` with or before the date's `→DATE`),
/// and when the write is done.
fn rehearse_write(on: &Emulator, time: &ClockTime, date: &ClockTime) -> crate::Result<Timing> {
    let mut e = on.rehearsal();
    e.start_transfer(Op::SetClock {
        time: *time,
        date: *date,
    })?;
    let start = e.emulated_ms();
    let mut lines = Vec::new();
    loop {
        let done = e.transfer_step(crate::host::SLICE_MS)?;
        let ran = e.emulated_ms() - start;
        while lines.len() < e.transfer_lines_run() {
            lines.push(ran);
        }
        if done {
            let (r, _) = e.transfer_result()?;
            if let Some(err) = r.error {
                return Err(err.into());
            }
            let time_ms = lines.first().copied().unwrap_or(ran);
            return Ok(Timing {
                time_ms,
                date_ms: lines.get(1).copied().unwrap_or(time_ms),
                end_ms: ran,
            });
        }
        if ran > REHEARSAL_CAP_MS {
            return Err(NOT_SETTLED.into());
        }
    }
}

/// Rehearse the set for `now`, then plan it again with the time that
/// rehearsal asks for, until a plan asks for itself: its own keys
/// rehearsed. Not merely as many keys: on the 38G one digit for another
/// moved a set's end by over a second (the queue's waits for the ROM to
/// idle fall differently). Usually two rehearsals, the first on the time
/// requested; after three, the last plan rehearsed, which keys a time at
/// most a second or so off what its rehearsal asks for. What to key and
/// why; the planning time counts to the end.
fn plan_lead<P: PartialEq>(
    now: &Now,
    pace: &Pace,
    clock: &dyn crate::protocol::Clock,
    plan: impl Fn(&ClockTime, &ClockTime) -> crate::Result<P>,
    rehearse: impl Fn(&P) -> crate::Result<Timing>,
) -> crate::Result<(P, Lead)> {
    let (mut time, mut date) = (now.time, now.time);
    let mut p = plan(&time, &date)?;
    let mut round = 0;
    loop {
        let before = clock.now_ms();
        let timing = rehearse(&p)?;
        let rate = pace.rate(timing.end_ms, clock.now_ms() - before);
        let planning_ms = (clock.now_ms() - now.wall_ms).max(0.0);
        let (t, d) = keyed(now, &timing, rate, planning_ms);
        let next = plan(&t, &d)?;
        round += 1;
        if next == p || round == 3 {
            if next == p {
                (time, date) = (t, d);
            }
            for t in [time, date] {
                if !YEARS.contains(&t.year) {
                    return Err(format!(
                        "the year must be between {} and {}",
                        YEARS.start(),
                        YEARS.end()
                    )
                    .into());
                }
            }
            return Ok((
                p,
                Lead {
                    timing,
                    rate,
                    planning_ms,
                    time,
                    date,
                },
            ));
        }
        (time, date, p) = (t, d, next);
    }
}

impl Emulator {
    /// Why the clock cannot be set now, or `None`.
    pub fn clock_refusal(&self) -> Option<String> {
        let m = self.machine();
        let model = m.model();
        if !has_clock(model) {
            return Some(format!("The {} has no clock.", model.name().to_uppercase()));
        }
        if self.typing() || self.transferring() || self.keys_busy() {
            return Some(BUSY.into());
        }
        if !m.display_on() {
            return Some(OFF.into());
        }
        if asks_language(m) {
            return Some(NO_LANGUAGE.into());
        }
        // The Pioneers' ((•)) is the alarm, not busy.
        if pioneer_modes(model).is_none() && m.framebuffer().annunciators.busy {
            return Some(BUSY.into());
        }
        if by_command(model) && self.command_line().is_ok_and(|l| l.active) {
            return Some(LINE_OPEN.into());
        }
        // The key plans start from a plain keyboard: a pending shift or
        // alpha would turn their keys into others.
        let a = m.framebuffer().annunciators;
        if !by_command(model) && (a.left_shift || a.right_shift || a.alpha) {
            return Some(LINE_OPEN.into());
        }
        None
    }

    /// Set the clock to `now` on a model set by keys: queues the
    /// presses (the host runs them as any queued keys) with the time
    /// keyed so that the clock reads the wall clock once they are done at
    /// `pace` (rehearsed on a copy of the machine first, see `keyed`).
    /// The 48 and 49 go through [`Emulator::clock_write`] instead.
    pub fn set_clock_by_keys(
        &mut self,
        now: &Now,
        pace: &Pace,
        clock: &dyn crate::protocol::Clock,
    ) -> crate::Result<Lead> {
        if let Some(why) = self.clock_refusal() {
            return Err(why.into());
        }
        if by_command(self.model()) {
            return Err("the 48 and 49 set their clock by a write".into());
        }
        let this: &Emulator = self;
        let m = self.machine();
        let (plan, lead) = plan_lead(
            now,
            pace,
            clock,
            |time, date| key_plan(m, time, date, pace.speed <= 1.0),
            |p| rehearse_keys(this, p, pace.pass_ms.max(1.0)),
        )?;
        self.release_keys();
        if let Err(e) = queue_presses(self, &plan.keys) {
            self.release_keys();
            return Err(e);
        }
        self.pump();
        Ok(lead)
    }

    /// The write that sets the clock to `now` on a 48 or 49, its time
    /// and date keyed so that the clock reads the wall clock once it is
    /// done (rehearsed on a copy of the machine first, see `keyed`; a
    /// write runs as fast as the host can, [`Pace::UNBOUNDED`] on a fast
    /// host).
    pub fn clock_write(
        &self,
        now: &Now,
        pace: &Pace,
        clock: &dyn crate::protocol::Clock,
    ) -> crate::Result<(Op, Lead)> {
        if let Some(why) = self.clock_refusal() {
            return Err(why.into());
        }
        if !by_command(self.model()) {
            return Err("this model sets its clock by keys".into());
        }
        let this: &Emulator = self;
        plan_lead(
            now,
            pace,
            clock,
            |time, date| {
                Ok(Op::SetClock {
                    time: *time,
                    date: *date,
                })
            },
            |op| match op {
                Op::SetClock { time, date } => rehearse_write(this, time, date),
                _ => Err("not a clock write".into()),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_keyed_as_the_manuals_show() {
        let t = ClockTime::new(2026, 6, 9, 21, 8, 30).unwrap();
        assert_eq!(t.time_number(), "21.0830");
        assert_eq!(t.date_number(false), "6.092026");
        assert_eq!(t.date_number(true), "9.062026");
        let midnight = ClockTime::new(2026, 10, 13, 0, 0, 5).unwrap();
        assert_eq!(midnight.time_number(), "0.0005");
        assert_eq!(
            rpl_lines(&t, &t, false, false, false),
            ["21.0830 →TIME 6.092026 →DATE"]
        );
        assert_eq!(
            rpl_lines(&t, &t, true, true, true),
            ["→TIME(21,0830)", "→DATE(9,062026)"]
        );
        assert_eq!(
            number_keys("6.09"),
            [
                Press::Key("6"),
                Press::Key("point"),
                Press::Key("0"),
                Press::Key("9")
            ]
        );
    }

    #[test]
    fn dates_are_checked_and_roll_over() {
        assert!(ClockTime::new(2026, 2, 29, 0, 0, 0).is_err());
        assert!(ClockTime::new(2024, 2, 29, 0, 0, 0).is_ok());
        assert!(ClockTime::new(1990, 1, 1, 0, 0, 0).is_err());
        assert!(ClockTime::new(2026, 1, 1, 24, 0, 0).is_err());
        let t = ClockTime::new(2026, 12, 31, 23, 59, 50).unwrap();
        assert_eq!(t.shifted(15), ClockTime::new(2027, 1, 1, 0, 0, 5).unwrap());
        assert_eq!(t.shifted(15).shifted(-15), t);
        let march = ClockTime::new(2024, 3, 1, 0, 0, 2).unwrap();
        assert_eq!(
            march.shifted(-3),
            ClockTime::new(2024, 2, 29, 23, 59, 59).unwrap()
        );
        assert_eq!(march.shifted(86_400 * 366).year, 2025);
        let json = serde_json::json!({"year": 2026, "month": 10, "day": 13, "hour": 9, "minute": 5, "second": 0});
        assert_eq!(
            ClockTime::from_json(&json).unwrap(),
            ClockTime::new(2026, 10, 13, 9, 5, 0).unwrap()
        );
        assert!(ClockTime::from_json(&serde_json::json!({"year": 2026})).is_err());
    }

    /// The time keyed for a set rehearsed to take the time 10 s in and
    /// end 4 s later: at 1x when it is taken, at Max that much behind;
    /// the 38G's date, taken 3 s after the time, of that moment.
    #[test]
    fn the_time_keyed_reads_the_wall_clock_when_the_set_is_done() {
        let now = Now {
            time: ClockTime::new(2026, 10, 13, 23, 59, 50).unwrap(),
            ms: 600.0,
            wall_ms: 0.0,
        };
        let timing = Timing {
            time_ms: 10_000.0,
            date_ms: 13_000.0,
            end_ms: 14_000.0,
        };
        let (time, date) = keyed(&now, &timing, 1.0, 0.0);
        assert_eq!(time, ClockTime::new(2026, 10, 14, 0, 0, 1).unwrap());
        assert_eq!(date.day, 14);
        // 2x: taken 5 s of wall time in; the 4 s after run in 2.
        let (time, _) = keyed(&now, &timing, 2.0, 0.0);
        assert_eq!(time, ClockTime::new(2026, 10, 13, 23, 59, 54).unwrap());
        // Max: 4 s behind, the date of the moment 3 s later.
        let (time, date) = keyed(&now, &timing, f64::INFINITY, 400.0);
        assert_eq!(time, ClockTime::new(2026, 10, 13, 23, 59, 47).unwrap());
        assert_eq!(date.day, 13);
        // The Pioneers take the date first: the date of the time keyed.
        let first = Timing {
            date_ms: 5_000.0,
            ..timing
        };
        let (time, date) = keyed(&now, &first, 1.0, 0.0);
        assert_eq!(date, time);
        // The host's own speed caps the speed aimed for.
        let max = Pace {
            speed: 60.0,
            share: 0.5,
            pass_ms: 1000.0,
        };
        assert_eq!(max.rate(1000.0, 0.0), 60.0);
        assert_eq!(max.rate(1000.0, 50.0), 10.0);
        assert_eq!(Pace::UNBOUNDED.rate(1000.0, 10.0), 100.0);
    }

    #[test]
    fn only_the_42s_and_28s_have_no_clock() {
        assert!(has_clock(Model::Hp19bii));
        for m in Model::ALL {
            let none = matches!(m, Model::Hp42s | Model::Hp28s);
            assert_eq!(has_clock(m), !none, "{}", m.name());
        }
    }
}
