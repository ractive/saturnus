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

/// Symbols accepted as key names by [`parse_keys`].
const SYMBOLS: [(&str, Key); 5] = [
    ("+", Key::Plus),
    ("-", Key::Minus),
    ("*", Key::Multiply),
    ("/", Key::Divide),
    (".", Key::Point),
];

/// The script commands; any other first word is a key.
const COMMANDS: [&str; 5] = ["press", "down", "up", "wait", "wait-idle"];

/// Largest script [`parse_keys`] takes, in bytes.
pub const MAX_SCRIPT_BYTES: usize = 64 * 1024;
/// Largest script [`parse_keys`] takes, in lines.
pub const MAX_SCRIPT_LINES: usize = 2_000;
/// Most emulated time one script may ask for, in milliseconds, summed over
/// holds, waits and idle caps (a press counts its hold and its idle cap).
pub const MAX_BUDGET_MS: u64 = 10 * 60 * 1000;

/// A key by script name or symbol.
fn key_named(word: &str) -> Option<Key> {
    SYMBOLS
        .iter()
        .find(|(s, _)| *s == word)
        .map(|&(_, k)| k)
        .or_else(|| Key::from_name(word))
}

/// Parse a script for a remote caller (the control API): the format of
/// [`parse`], where in addition a line may hold several key names
/// separated by spaces (each a `press` with the default hold, in order)
/// and `+ - * / .` name the plus, minus, multiply, divide and point keys,
/// so `2 ENTER 3 +` is a script. Scripts over [`MAX_SCRIPT_BYTES`],
/// [`MAX_SCRIPT_LINES`] or [`MAX_BUDGET_MS`] of emulated time are refused.
pub fn parse_keys(text: &str) -> Result<Vec<Line>> {
    if text.len() > MAX_SCRIPT_BYTES {
        bail!(
            "key script is {} bytes, at most {MAX_SCRIPT_BYTES} are accepted",
            text.len()
        );
    }
    let count = text.lines().count();
    if count > MAX_SCRIPT_LINES {
        bail!("key script has {count} lines, at most {MAX_SCRIPT_LINES} are accepted");
    }
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let number = i + 1;
        let body = raw.split('#').next().unwrap_or("");
        let words: Vec<&str> = body.split_whitespace().collect();
        let Some(first) = words.first() else {
            continue;
        };
        let is_command = COMMANDS.iter().any(|c| c.eq_ignore_ascii_case(first));
        if !is_command && words.iter().all(|w| key_named(w).is_some()) {
            for w in &words {
                let key =
                    key_named(w).with_context(|| format!("key script line {number}: {w:?}"))?;
                out.push(Line {
                    number,
                    action: Action::Press {
                        key,
                        hold_ms: DEFAULT_HOLD_MS,
                    },
                });
            }
            continue;
        }
        let action = parse_line(body.trim())
            .with_context(|| format!("key script line {number}: {raw:?}"))?;
        out.push(Line { number, action });
    }
    let budget = budget_ms(&out);
    if budget > MAX_BUDGET_MS {
        bail!(
            "key script asks for {budget} ms of emulated time, at most {MAX_BUDGET_MS} are accepted"
        );
    }
    Ok(out)
}

/// The most emulated time `lines` can run, in milliseconds: holds, waits
/// and idle caps (a press waits for idle after its hold).
pub fn budget_ms(lines: &[Line]) -> u64 {
    lines
        .iter()
        .map(|l| match l.action {
            Action::Press { hold_ms, .. } => hold_ms.saturating_add(DEFAULT_IDLE_CAP_MS),
            Action::Wait { ms } => ms,
            Action::WaitIdle { cap_ms } => cap_ms,
            Action::Down(_) | Action::Up(_) => 0,
        })
        .fold(0, u64::saturating_add)
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
    fn remote_scripts_take_several_keys_and_symbols_per_line() {
        let lines = parse_keys("2 ENTER 3 +\nwait-idle 500\npress on 100").unwrap();
        let keys: Vec<Action> = lines.iter().map(|l| l.action).collect();
        let press = |key| Action::Press {
            key,
            hold_ms: DEFAULT_HOLD_MS,
        };
        assert_eq!(
            keys,
            vec![
                press(Key::Two),
                press(Key::Enter),
                press(Key::Three),
                press(Key::Plus),
                Action::WaitIdle { cap_ms: 500 },
                Action::Press {
                    key: Key::On,
                    hold_ms: 100
                },
            ]
        );
        assert_eq!(lines[3].number, 1);
        assert!(parse_keys("2 frobnicate").is_err());
        assert!(parse_keys(&"f\n".repeat(MAX_SCRIPT_LINES + 1)).is_err());
        assert!(parse_keys(&"x".repeat(MAX_SCRIPT_BYTES + 1)).is_err());
        let e = parse_keys("wait 600001").unwrap_err().to_string();
        assert!(e.contains("emulated time"), "{e}");
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
