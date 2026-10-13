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

    /// This time `secs` seconds later (the date rolls over).
    pub fn plus_seconds(self, secs: u32) -> ClockTime {
        let mut t = self;
        let total =
            u32::from(t.hour) * 3600 + u32::from(t.minute) * 60 + u32::from(t.second) + secs;
        let mut days = total / 86_400;
        let rest = total % 86_400;
        t.hour = (rest / 3600) as u8;
        t.minute = (rest / 60 % 60) as u8;
        t.second = (rest % 60) as u8;
        while days > 0 {
            days -= 1;
            if t.day < days_in(t.year, t.month) {
                t.day += 1;
            } else {
                t.day = 1;
                if t.month < 12 {
                    t.month += 1;
                } else {
                    t.month = 1;
                    t.year += 1;
                }
            }
        }
        t
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

/// The lines the 48 and 49 run to set `t`: one in RPN mode, one per
/// command in the 49G's algebraic mode. `day_first`: flag -42 set;
/// `comma`: flag -51 set (a comma is the fraction mark).
pub fn rpl_lines(t: &ClockTime, day_first: bool, comma: bool, algebraic: bool) -> Vec<String> {
    let mark = |s: String| if comma { s.replace('.', ",") } else { s };
    let time = mark(t.time_number());
    let date = mark(t.date_number(day_first));
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

/// Emulated ms one press of a key plan takes, about (a letter is two:
/// alpha and its key). Measured on the owner's ROMs: a whole plan's
/// time over its presses.
fn press_ms(model: Model) -> f64 {
    match model {
        Model::Hp38g => 240.0,
        Model::Hp39g | Model::Hp40g => 230.0,
        _ => 200.0,
    }
}

/// Presses `keys` takes (a letter two).
fn presses(keys: &[Press]) -> usize {
    keys.iter()
        .map(|p| match p {
            Press::Key(_) => 1,
            Press::Letter(_) => 2,
        })
        .sum()
}

/// The presses that set `t` on a model that is set by keys, the time
/// keyed ahead by about how long the presses before it is taken last.
fn key_plan(m: &Machine, t: &ClockTime) -> crate::Result<Vec<Press>> {
    let model = m.model();
    let plan = |t: &ClockTime| -> crate::Result<(Vec<Press>, usize)> {
        let mut keys = Vec::new();
        let at;
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
            keys.extend(number_keys(&t.date_number(day_first)));
            keys.push(Press::Key("a"));
            if twelve {
                keys.push(Press::Key("e"));
            }
            keys.extend(number_keys(&t.time_number()));
            keys.push(Press::Key("b"));
            at = presses(&keys);
            if twelve {
                keys.push(Press::Key("e"));
            }
            if model == Model::Hp27s {
                keys.extend([Press::Key("exit"), Press::Key("exit")]);
            } else {
                keys.extend([Press::Key("shift"), Press::Key("exit")]);
            }
        } else {
            // 38G, 39G, 40G: HOME, then STO▶ (the first menu key) into
            // TIME and DATE, each store's two history lines deleted.
            keys.push(Press::Key("home"));
            let store = |keys: &mut Vec<Press>, number: String, name: &str| {
                keys.extend(number_keys(&number));
                keys.push(Press::Key("a"));
                keys.extend(name.chars().map(Press::Letter));
                keys.push(Press::Key("enter"));
                presses(keys)
            };
            let forget = |keys: &mut Vec<Press>| {
                keys.extend(["up", "del", "del", "down"].map(Press::Key));
            };
            at = store(&mut keys, t.time_number(), "TIME");
            forget(&mut keys);
            store(&mut keys, t.date_number(false), "DATE");
            forget(&mut keys);
        }
        Ok((keys, at))
    };
    // Plan once to count the presses before the time is taken, then key
    // the time that will be current by then.
    let (_, at) = plan(t)?;
    let ahead = (at as f64 * press_ms(model) / 1000.0).round() as u32;
    Ok(plan(&t.plus_seconds(ahead))?.0)
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

    /// Set the clock to `t` on a model set by keys: queues the presses
    /// (the host runs them as any queued keys). The 48 and 49 go through
    /// [`crate::transfer::Op::SetClock`] instead.
    pub fn set_clock_by_keys(&mut self, t: &ClockTime) -> crate::Result<()> {
        if let Some(why) = self.clock_refusal() {
            return Err(why.into());
        }
        if by_command(self.model()) {
            return Err("the 48 and 49 set their clock by a write".into());
        }
        let keys = key_plan(self.machine(), t)?;
        self.release_keys();
        for p in keys {
            match p {
                Press::Key(k) => self.queue().type_keys(&[k]),
                Press::Letter(c) => {
                    if !self.queue().type_letter(c) {
                        self.release_keys();
                        return Err(format!("cannot type {c:?} on this model").into());
                    }
                }
            }
        }
        self.pump();
        Ok(())
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
            rpl_lines(&t, false, false, false),
            ["21.0830 →TIME 6.092026 →DATE"]
        );
        assert_eq!(
            rpl_lines(&t, true, true, true),
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
        assert_eq!(
            t.plus_seconds(15),
            ClockTime::new(2027, 1, 1, 0, 0, 5).unwrap()
        );
        let json = serde_json::json!({"year": 2026, "month": 10, "day": 13, "hour": 9, "minute": 5, "second": 0});
        assert_eq!(
            ClockTime::from_json(&json).unwrap(),
            ClockTime::new(2026, 10, 13, 9, 5, 0).unwrap()
        );
        assert!(ClockTime::from_json(&serde_json::json!({"year": 2026})).is_err());
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
