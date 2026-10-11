//! Recorded serial traffic with its timing, and its replay.
//!
//! A trace is text, one line per step: the time in milliseconds since the
//! recording began, a direction, and the bytes in `kermit-proto`'s trace
//! escapes (`\xNN`, `\r`, `\n`, `\\`):
//!
//! ```text
//! # request connect
//!        0 = 9600 none
//!     4950 < \x01#\x20N3\r
//!     4952 > \x01...
//! ```
//!
//! `=` is an [`Action::Open`] (speed and parity), `>` an [`Action::Write`],
//! `<` bytes read; `#` lines are notes (`just tx-traces` marks each request
//! with `# request ...`). A trace holds serial bytes only.
//!
//! [`Replay`] plays the calculator of a trace to a [`Device`](crate::Device)
//! on a fake clock: each action must be the trace's next `=` or `>`, and
//! each `<` arrives as long after the step before it as it did when
//! recorded.

use std::collections::VecDeque;
use std::fmt::{self, Write as _};

use kermit_proto::trace::{escape, unescape};
use satx_core::time::{Duration, Instant};

use crate::device::{Action, LinkSettings, Parity};

/// One step of a [`Trace`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// A note (`# ...`), e.g. `request get PRG`.
    Note(String),
    /// The host opened the port (`=`).
    Open(LinkSettings),
    /// The host wrote these bytes (`>`).
    Out(Vec<u8>),
    /// The host read these bytes (`<`).
    In(Vec<u8>),
}

/// A recorded session: steps with their times since the start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trace {
    /// The steps and when each happened; a note has the time of the step
    /// before it.
    pub steps: Vec<(Duration, Step)>,
}

impl Trace {
    /// An empty trace.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a step at `at`.
    pub fn push(&mut self, at: Duration, step: Step) {
        self.steps.push((at, step));
    }

    /// Append a note.
    pub fn note(&mut self, text: &str) {
        let at = self.steps.last().map_or(Duration::ZERO, |(at, _)| *at);
        self.steps.push((at, Step::Note(text.to_string())));
    }

    /// Parse the text form.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut trace = Trace::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim_end();
            let bad = |what: &str| format!("line {}: {what}", n + 1);
            if line.is_empty() {
                continue;
            }
            if let Some(note) = line.strip_prefix('#') {
                trace.note(note.strip_prefix(' ').unwrap_or(note));
                continue;
            }
            let line = line.trim_start();
            let (ms, rest) = line.split_once(' ').ok_or_else(|| bad("no direction"))?;
            let ms: u64 = ms.parse().map_err(|_| bad("bad time"))?;
            let at = Duration::from_millis(ms);
            let (dir, bytes) = rest
                .split_at_checked(1)
                .ok_or_else(|| bad("no direction"))?;
            let bytes = bytes.strip_prefix(' ').unwrap_or(bytes);
            let step = match dir {
                "=" => {
                    let (baud, parity) =
                        bytes.split_once(' ').ok_or_else(|| bad("bad settings"))?;
                    Step::Open(LinkSettings {
                        baud: baud.parse().map_err(|_| bad("bad speed"))?,
                        parity: Parity::from_name(parity).ok_or_else(|| bad("bad parity"))?,
                    })
                }
                ">" => Step::Out(unescape(bytes).map_err(|e| bad(&e))?),
                "<" => Step::In(unescape(bytes).map_err(|e| bad(&e))?),
                _ => return Err(bad("expected '=', '>' or '<'")),
            };
            trace.push(at, step);
        }
        Ok(trace)
    }

    /// The steps before the first note that starts with `prefix` (the
    /// whole trace when there is none).
    pub fn before_note(&self, prefix: &str) -> Trace {
        let end = self
            .steps
            .iter()
            .position(|(_, s)| matches!(s, Step::Note(n) if n.starts_with(prefix)))
            .unwrap_or(self.steps.len());
        Trace {
            steps: self.steps[..end].to_vec(),
        }
    }

    /// Every byte the host read, in order.
    pub fn input(&self) -> Vec<u8> {
        self.steps
            .iter()
            .filter_map(|(_, s)| match s {
                Step::In(bytes) => Some(bytes.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect()
    }
}

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        for (at, step) in &self.steps {
            let ms = at.as_millis();
            let _ = match step {
                Step::Note(text) => writeln!(out, "# {text}"),
                Step::Open(s) => writeln!(out, "{ms:>8} = {} {}", s.baud, s.parity.name()),
                Step::Out(bytes) => writeln!(out, "{ms:>8} > {}", escape(bytes)),
                Step::In(bytes) => writeln!(out, "{ms:>8} < {}", escape(bytes)),
            };
        }
        f.write_str(&out)
    }
}

/// A [`Trace`] played back as the calculator, on the caller's clock.
#[derive(Debug)]
pub struct Replay {
    steps: VecDeque<(Duration, Step)>,
    /// The trace time and the replay time of the last action matched (or
    /// of the start).
    anchor: (Duration, Instant),
}

impl Replay {
    /// Play `trace` from `start`.
    pub fn new(trace: &Trace, start: Instant) -> Self {
        let steps = trace
            .steps
            .iter()
            .filter(|(_, s)| !matches!(s, Step::Note(_)))
            .cloned()
            .collect();
        Replay {
            steps,
            anchor: (Duration::ZERO, start),
        }
    }

    /// The device did `action` at `now`: it must be the trace's next step
    /// (bytes still to be read before it in the trace are a mismatch too).
    pub fn action(&mut self, now: Instant, action: &Action) -> Result<(), String> {
        let matches = match (self.steps.front(), action) {
            (Some((_, Step::Open(a))), Action::Open(b)) => a == b,
            (Some((_, Step::Out(a))), Action::Write(b)) => a == b,
            _ => false,
        };
        if !matches {
            let expected = match self.steps.front() {
                None => "the end of the trace".to_string(),
                Some((_, Step::In(bytes))) => format!("input {} first", escape(bytes)),
                Some((_, Step::Out(bytes))) => format!("write {}", escape(bytes)),
                Some((_, Step::Open(s))) => format!("open at {s}"),
                Some((_, Step::Note(n))) => format!("note {n}"),
            };
            let got = match action {
                Action::Open(s) => format!("open at {s}"),
                Action::Write(bytes) => format!("write {}", escape(bytes)),
            };
            return Err(format!("trace mismatch: {got}, expected {expected}"));
        }
        if let Some((at, _)) = self.steps.pop_front() {
            self.anchor = (at, now);
        }
        Ok(())
    }

    /// When the next bytes arrive; `None` when the trace waits for an
    /// action or is over.
    pub fn next_input(&self) -> Option<Instant> {
        match self.steps.front() {
            Some((at, Step::In(_))) => Some(self.anchor.1 + at.saturating_sub(self.anchor.0)),
            _ => None,
        }
    }

    /// The bytes due at `now`.
    pub fn take_input(&mut self, now: Instant) -> Option<Vec<u8>> {
        if self.next_input().is_some_and(|due| due <= now)
            && let Some((_, Step::In(bytes))) = self.steps.pop_front()
        {
            return Some(bytes);
        }
        None
    }

    /// The steps not played yet.
    pub fn remaining(&self) -> usize {
        self.steps.len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const TEXT: &str = "# request connect
       0 = 9600 none
    4950 < \\x01# N3\\r
    5000 > \\x01-
# request list
    5100 < \\x01% Y
";

    #[test]
    fn text_round_trips() {
        let trace = Trace::parse(TEXT).unwrap();
        assert_eq!(trace.steps.len(), 6);
        assert_eq!(trace.steps[1].1, Step::Open(LinkSettings::DEFAULT));
        assert_eq!(trace.steps[2].1, Step::In(b"\x01# N3\r".to_vec()));
        assert_eq!(Trace::parse(&trace.to_string()).unwrap(), trace);
        assert_eq!(trace.before_note("request list").steps.len(), 4);
        assert_eq!(trace.input(), b"\x01# N3\r\x01% Y");
        assert!(Trace::parse("12 ? x").is_err());
        assert!(Trace::parse("x > y").is_err());
    }

    #[test]
    fn replay_keeps_the_recorded_gaps() {
        let trace = Trace::parse(TEXT).unwrap();
        let t0 = Instant::now();
        let mut replay = Replay::new(&trace, t0);
        assert_eq!(replay.next_input(), None);
        assert!(replay.action(t0, &Action::Write(b"x".to_vec())).is_err());
        replay
            .action(t0, &Action::Open(LinkSettings::DEFAULT))
            .unwrap();
        let due = replay.next_input().unwrap();
        assert_eq!(due, t0 + Duration::from_millis(4950));
        assert_eq!(replay.take_input(t0), None);
        assert_eq!(replay.take_input(due).unwrap(), b"\x01# N3\r");
        // The write comes later than recorded: the next input keeps its
        // gap to it.
        let late = due + Duration::from_secs(1);
        replay
            .action(late, &Action::Write(b"\x01-".to_vec()))
            .unwrap();
        assert_eq!(replay.next_input(), Some(late + Duration::from_millis(100)));
        assert_eq!(replay.remaining(), 1);
    }
}
