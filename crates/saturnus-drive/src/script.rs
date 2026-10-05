//! Key scripts: a small line-based format that drives the keyboard in
//! emulated time. See `README.md` ("Key scripts") for the format.
//!
//! ```text
//! # boot, answer "Try To Recover Memory?" with NO
//! wait-idle 60000
//! press f
//! ```

use anyhow::{Context, Result, bail};
use saturnus::io::Key;

/// Default time a `press` holds its key, in emulated milliseconds. The ROM
/// accepts a key after about 10 ms of identical samples, so this leaves a
/// wide margin.
pub const DEFAULT_HOLD_MS: u64 = 60;
/// Default cap of a `wait-idle`, in emulated milliseconds.
pub const DEFAULT_IDLE_CAP_MS: u64 = 10_000;

/// One line of a key script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Press `key`, hold it `hold_ms`, release it, then wait for idle.
    Press { key: Key, hold_ms: u64 },
    /// Press `key` and keep it down.
    Down(Key),
    /// Release `key`.
    Up(Key),
    /// Run for a fixed time.
    Wait { ms: u64 },
    /// Run until the calculator is idle, at most `cap_ms`.
    WaitIdle { cap_ms: u64 },
}

/// A parsed action with the line it came from (1-based), for messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    /// Source line number.
    pub number: usize,
    /// The action on that line.
    pub action: Action,
}

/// Parse a whole script. Errors name the offending line.
pub fn parse(text: &str) -> Result<Vec<Line>> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let number = i + 1;
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let action =
            parse_line(line).with_context(|| format!("key script line {number}: {raw:?}"))?;
        out.push(Line { number, action });
    }
    Ok(out)
}

fn parse_line(line: &str) -> Result<Action> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let (cmd, args) = match words.split_first() {
        Some((c, a)) => (c.to_ascii_lowercase(), a),
        None => bail!("empty line"),
    };
    let arg_count = |max: usize| -> Result<()> {
        if args.len() > max {
            bail!("too many arguments to {cmd}");
        }
        Ok(())
    };
    let action = match cmd.as_str() {
        "press" => {
            arg_count(2)?;
            let key = key_arg(args.first().copied())?;
            let hold_ms = match args.get(1) {
                Some(v) => ms(v)?,
                None => DEFAULT_HOLD_MS,
            };
            Action::Press { key, hold_ms }
        }
        "down" => {
            arg_count(1)?;
            Action::Down(key_arg(args.first().copied())?)
        }
        "up" => {
            arg_count(1)?;
            Action::Up(key_arg(args.first().copied())?)
        }
        "wait" => {
            arg_count(1)?;
            let v = args.first().context("wait needs a time in ms")?;
            Action::Wait { ms: ms(v)? }
        }
        "wait-idle" => {
            arg_count(1)?;
            let cap_ms = match args.first() {
                Some(v) => ms(v)?,
                None => DEFAULT_IDLE_CAP_MS,
            };
            Action::WaitIdle { cap_ms }
        }
        _ => {
            // A bare key name is a `press` with the default hold.
            arg_count(0)?;
            let key = Key::from_name(&cmd)
                .with_context(|| format!("unknown action or key name {cmd:?}"))?;
            Action::Press {
                key,
                hold_ms: DEFAULT_HOLD_MS,
            }
        }
    };
    Ok(action)
}

fn key_arg(name: Option<&str>) -> Result<Key> {
    let name = name.context("missing key name")?;
    Key::from_name(name).with_context(|| format!("unknown key name {name:?}"))
}

/// A duration in milliseconds; `_` separators allowed, optional `ms` suffix.
fn ms(v: &str) -> Result<u64> {
    let digits = v.trim_end_matches("ms").replace('_', "");
    digits
        .parse()
        .with_context(|| format!("bad duration {v:?} (milliseconds expected)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_actions() {
        let s = "# comment\n\nwait-idle 60_000\nf\npress enter 100ms\ndown on\nup on\nwait 5 # trailing\nwait-idle\n";
        let lines = parse(s).unwrap();
        let actions: Vec<Action> = lines.iter().map(|l| l.action).collect();
        assert_eq!(
            actions,
            vec![
                Action::WaitIdle { cap_ms: 60_000 },
                Action::Press {
                    key: Key::F,
                    hold_ms: DEFAULT_HOLD_MS
                },
                Action::Press {
                    key: Key::Enter,
                    hold_ms: 100
                },
                Action::Down(Key::On),
                Action::Up(Key::On),
                Action::Wait { ms: 5 },
                Action::WaitIdle {
                    cap_ms: DEFAULT_IDLE_CAP_MS
                },
            ]
        );
        assert_eq!(lines[1].number, 4);
    }

    #[test]
    fn key_names_are_case_insensitive() {
        let lines = parse("ENTER\nPress LeftShift").unwrap();
        assert_eq!(
            lines[1].action,
            Action::Press {
                key: Key::LeftShift,
                hold_ms: DEFAULT_HOLD_MS
            }
        );
    }

    #[test]
    fn errors_name_the_line() {
        let e = parse("f\nfrobnicate\n").unwrap_err();
        assert!(format!("{e:#}").contains("line 2"), "{e:#}");
        assert!(parse("wait").is_err());
        assert!(parse("wait abc").is_err());
        assert!(parse("press f 10 20").is_err());
        assert!(parse("f g").is_err());
    }
}
