//! Running a write: its steps in turns, a stop, the 49G's algebraic mode.

use super::*;

/// The step running now.
#[derive(Debug)]
pub(super) enum Running {
    Typing(Box<Job>),
    WaitNak {
        cap: u64,
    },
    Exchange(Box<Exchange>, Step),
    /// Ending a server with ON.
    EndServer {
        phase: EndPhase,
        until: u64,
    },
    /// After ON: does the server answer an I packet (it still runs)? At
    /// most until `until` (a cycle count, [`PROBE_CAP_MS`]).
    Probe {
        x: Box<Exchange>,
        until: u64,
    },
    /// Cancelling a line typed in part.
    CancelLine {
        phase: CancelPhase,
        until: u64,
    },
    Settle(SettleState),
    /// Presses of the backspace key still to make; the current one is
    /// down until `up` (a cycle count), then the CPU is waited for until
    /// `next`.
    Drop {
        left: usize,
        up: u64,
        next: u64,
    },
    /// The pause after a dropping press, until `until` (a cycle count);
    /// `left` presses with the one made.
    DropGap {
        left: usize,
        until: u64,
    },
}

/// Where [`Running::EndServer`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EndPhase {
    /// ON is down.
    Hold,
    /// The calculator returns to its stack; then [`Running::Probe`].
    Settle,
}

/// Where [`Running::CancelLine`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CancelPhase {
    /// The calculator reads the last key.
    Idle,
    /// ON is down.
    Hold,
    /// The line closes.
    Gap,
}

/// Waiting for the stack after `G F` (or after ON).
#[derive(Debug)]
pub(super) struct SettleState {
    /// Run plainly until this cycle count.
    pub(super) until: u64,
    /// Then until the screen is stable, at most until this one.
    pub(super) cap: u64,
    pub(super) last: Option<saturnus_core::Lcd>,
    pub(super) since: u64,
}

impl SettleState {
    pub(super) fn new(m: &Machine, first: Duration) -> SettleState {
        let until = m.cycles() + cycles(m, first);
        SettleState {
            until,
            cap: until + ms_cycles(m, IDLE_CAP_MS),
            last: None,
            since: until,
        }
    }
}

/// What a write made on the calculator and has not removed yet, noted as
/// it is made, so a failure or a stop removes only what this write made
/// and an error names what it could not.
#[derive(Debug, Default)]
pub(super) struct Leftovers {
    /// The backup's temporary name ([`Op::Backup`]).
    pub(super) backup: Option<String>,
    /// Its `ARCHIVE` made `:0:N`.
    pub(super) archive: bool,
    /// `'N' STO` made the variable `N` in HOME.
    pub(super) variable: bool,
    /// A replacing store ([`Op::Store`]): the temporary name, the
    /// variable's, and its directory as the calculator writes it.
    pub(super) replace: Option<(String, String, String)>,
    /// The temporary variable holds the new object.
    pub(super) temp: bool,
}

impl Leftovers {
    /// The port copy's purge.
    pub(super) fn unarchive(n: &str) -> String {
        format!("IFERR :0:{n} PURGE THEN END")
    }

    /// The backup variable's purge.
    pub(super) fn unstore(n: &str) -> String {
        format!("IFERR {} PGDIR THEN END", quoted(n))
    }

    /// Host command `text` is sent: what it may make counts as made from
    /// now on, so a stop in the middle of it still removes it (`N` is free
    /// in HOME; a port 0 name taken before makes ARCHIVE fail at once,
    /// [`Leftovers::failed`]).
    pub(super) fn started(&mut self, text: &str) {
        if let Some(n) = &self.backup {
            if text == format!(":0:{n} ARCHIVE") {
                self.archive = true;
            } else if text == format!("{} STO", quoted(n)) {
                self.variable = true;
            }
        }
    }

    /// Host command `text` failed: the ARCHIVE made nothing.
    pub(super) fn failed(&mut self, text: &str) {
        if let Some(n) = &self.backup
            && text == format!(":0:{n} ARCHIVE")
        {
            self.archive = false;
        }
    }

    /// Host command `text` went through: note what it removed.
    pub(super) fn done(&mut self, text: &str) {
        if let Some(n) = &self.backup {
            if text == Self::unarchive(n) {
                self.archive = false;
            } else if text == Self::unstore(n) {
                self.variable = false;
            }
        }
        if let Some((t, _, _)) = &self.replace
            && (text == format!("{} PURGE", quoted(t)) || text == format!("{} PGDIR", quoted(t)))
        {
            self.temp = false;
        }
    }

    /// The lines a stop types once the server has ended, to remove what
    /// the backup made (nothing it did not make).
    pub(super) fn typed_cleanup(&self) -> Vec<String> {
        let Some(n) = &self.backup else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        if self.archive {
            lines.push(Self::unarchive(n));
        }
        if self.variable {
            lines.push(Self::unstore(n));
        }
        lines
    }

    /// What an error adds: the new object kept under the temporary name.
    pub(super) fn note(&self) -> Option<String> {
        let (t, name, dir) = self.replace.as_ref()?;
        self.temp
            .then(|| format!("the new {name} is kept as {t} in {{ {dir} }}"))
    }
}

/// A write in progress: step it until [`Transfer::step`] says done.
#[derive(Debug)]
pub struct Transfer {
    pub(super) steps: VecDeque<Step>,
    pub(super) running: Option<Running>,
    pub(super) wire: Wire,
    /// The server runs (typed and not ended).
    pub(super) server: bool,
    /// `SERVER` is being typed: once its ENTER is in, the server may run
    /// before its first NAK says so.
    pub(super) entering: bool,
    /// The number of stack levels before the first command.
    pub(super) baseline: Option<usize>,
    /// The first error; the cleanup steps still run.
    pub(super) error: Option<String>,
    pub(super) result: TransferResult,
    pub(super) fetched: Option<Vec<u8>>,
    /// The file sent is `storeText`'s string or `storeScreen`'s GROB,
    /// which must keep its temporary name.
    pub(super) temp_variable: bool,
    /// Objects a [`Step::Push`] left on the stack.
    pub(super) pushed: usize,
    pub(super) started: u64,
    pub(super) done: bool,
    /// The 49G was in algebraic mode, with this many stack levels: -95 is
    /// set again after the server, also on a stop.
    pub(super) algebraic: Option<usize>,
    /// The keys typed now set -95 again.
    pub(super) restoring: bool,
    /// Why -95 could not be set again (reported with any other error).
    pub(super) restore_error: Option<String>,
    /// Stopped ([`Transfer::stop`]): only the cleanup runs now.
    pub(super) stopped: bool,
    /// Lines typed and run so far ([`Step::Type`] and the like).
    pub(super) lines_run: usize,
    /// The next `G F` is the probe's: a server that answered it.
    pub(super) probe_finish: bool,
    /// Probes made so far ([`END_ROUNDS`]).
    pub(super) probes: usize,
    /// The probe running is [`Step::Ask`]'s, which ends a server that
    /// answers only when this is true.
    pub(super) asking: Option<bool>,
    /// What it made and has not removed yet.
    pub(super) leftovers: Leftovers,
}

/// The error of a write that was stopped (the host says why).
pub const STOPPED: &str = "stopped";
/// Emulated time per turn of [`crate::Emulator::stop_transfer_now`].
pub(super) const STOP_TURN_MS: f64 = 50.0;

/// What a 49G write types to clear algebraic mode, and to set it again.
pub(super) const TO_RPN: &str = "CF(-95)";
pub(super) const TO_ALGEBRAIC: &str = "-95 SF";

impl Transfer {
    /// Run at most `budget` cycles; true once the write is done (its
    /// result is then ready, [`Transfer::finish`]).
    pub fn step(&mut self, m: &mut Machine, budget: u64) -> crate::Result<bool> {
        let stop = m.cycles().saturating_add(budget);
        while !self.done {
            if m.cycles() >= stop {
                return Ok(false);
            }
            let Some(running) = self.running.take() else {
                match self.steps.pop_front() {
                    Some(step) => self.begin(m, step)?,
                    None => self.done = true,
                }
                continue;
            };
            self.running = self.advance(m, running, stop)?;
        }
        Ok(true)
    }

    /// Run `running` until it ends or `stop`; what still runs.
    pub(super) fn advance(
        &mut self,
        m: &mut Machine,
        running: Running,
        stop: u64,
    ) -> crate::Result<Option<Running>> {
        match running {
            Running::Typing(mut job) => match job.step(m, stop - m.cycles()) {
                Ok(false) => Ok(Some(Running::Typing(job))),
                Ok(true) => {
                    self.lines_run += 1;
                    let o = job.outcome();
                    let refused = if let Some(e) = &o.error {
                        Some(format!("the calculator refused it: {e}"))
                    } else if o.closed == Some(false) {
                        Some("the command line stayed open".to_string())
                    } else {
                        None
                    };
                    if std::mem::take(&mut self.restoring) {
                        self.restored(m, refused);
                    } else if let Some(e) = refused {
                        self.fail(e);
                    }
                    Ok(None)
                }
                Err(e) => {
                    if std::mem::take(&mut self.restoring) {
                        self.restored(m, Some(e.to_string()));
                    } else {
                        self.fail(e.to_string());
                    }
                    Ok(None)
                }
            },
            Running::WaitNak { cap } => {
                if self.wire.nak_step(m)? {
                    self.server = true;
                    return Ok(None);
                }
                if m.cycles() < cap {
                    return Ok(Some(Running::WaitNak { cap }));
                }
                self.wire.discard(m);
                // It may run all the same, on another port: ON.
                self.server = true;
                Ok(self.abort(
                    m,
                    format!(
                        "no Kermit server answered within {} s of emulated time after SERVER",
                        SERVER_CAP.as_secs()
                    ),
                ))
            }
            Running::Exchange(mut x, step) => {
                let Some(outcome) = x.step(&mut self.wire, m, stop)? else {
                    return Ok(Some(Running::Exchange(x, step)));
                };
                match outcome {
                    Ok(t) => {
                        self.finished(step, t);
                        Ok(None)
                    }
                    // A server that answered the probe gets ON and the
                    // probe again; it counts as ended either way.
                    Err(e) if matches!(step, Step::Finish) => {
                        if std::mem::take(&mut self.probe_finish) {
                            if self.probes < END_ROUNDS {
                                return Ok(Some(self.press_on(m)));
                            }
                            let e = match e {
                                Failure::Remote(e) | Failure::Link(e) => e,
                            };
                            self.fail(format!("the server could not be ended: {e}"));
                        }
                        self.server = false;
                        Ok(None)
                    }
                    // The server refused it and still runs: the cleanup,
                    // G F and the wait for the stack follow.
                    Err(Failure::Remote(e)) => {
                        self.fail(e);
                        Ok(None)
                    }
                    Err(Failure::Link(e)) => Ok(self.abort(m, e)),
                }
            }
            Running::EndServer { phase, until } => {
                if m.cycles() < until {
                    m.run_cycles((until.min(stop) - m.cycles()).max(1))?;
                    return Ok(Some(Running::EndServer { phase, until }));
                }
                Ok(Some(if phase == EndPhase::Hold {
                    let _ = m.key_up(Key::On);
                    Running::EndServer {
                        phase: EndPhase::Settle,
                        until: m.cycles() + cycles(m, FINISH_SETTLE),
                    }
                } else {
                    self.probe(m)?
                }))
            }
            Running::Probe { mut x, until } => {
                let outcome = x.step(&mut self.wire, m, stop.min(until))?;
                if outcome.is_none() && m.cycles() < until {
                    return Ok(Some(Running::Probe { x, until }));
                }
                self.probed(m, x.answered);
                Ok(None)
            }
            Running::CancelLine { phase, until } => {
                if phase == CancelPhase::Idle && m.is_shutdown() || m.cycles() >= until {
                    return Ok(match phase {
                        CancelPhase::Idle if cmdline::command_line(m).is_ok_and(|l| l.active) => {
                            m.key_down(Key::On)?;
                            Some(Running::CancelLine {
                                phase: CancelPhase::Hold,
                                until: m.cycles() + ms_cycles(m, ON_HOLD_MS),
                            })
                        }
                        CancelPhase::Hold => {
                            m.key_up(Key::On)?;
                            Some(Running::CancelLine {
                                phase: CancelPhase::Gap,
                                until: m.cycles() + ms_cycles(m, DROP_GAP_MS * 5),
                            })
                        }
                        _ => None,
                    });
                }
                let n = if phase == CancelPhase::Idle {
                    ms_cycles(m, 1)
                } else {
                    until.min(stop).saturating_sub(m.cycles())
                };
                m.run_cycles(n.max(1))?;
                Ok(Some(Running::CancelLine { phase, until }))
            }
            Running::Drop { left, up, next } => {
                let now = m.cycles();
                if now < up {
                    m.run_cycles((up.min(stop) - now).max(1))?;
                    if m.cycles() >= up {
                        m.key_up(Key::Backspace)?;
                    }
                    return Ok(Some(Running::Drop { left, up, next }));
                }
                if now < next && !m.is_shutdown() {
                    m.run_cycles(ms_cycles(m, 1))?;
                    return Ok(Some(Running::Drop { left, up, next }));
                }
                // The gap after the press, within the turn's budget.
                Ok(Some(Running::DropGap {
                    left,
                    until: now + ms_cycles(m, DROP_GAP_MS),
                }))
            }
            Running::DropGap { left, until } => {
                if m.cycles() < until {
                    m.run_cycles((until.min(stop) - m.cycles()).max(1))?;
                    return Ok(Some(Running::DropGap { left, until }));
                }
                if left > 1 {
                    return Ok(Some(self.press_drop(m, left - 1)?));
                }
                Ok(None)
            }
            Running::Settle(mut s) => {
                if m.cycles() < s.until {
                    m.run_cycles((s.until.min(stop) - m.cycles()).max(1))?;
                    return Ok(Some(Running::Settle(s)));
                }
                m.run_cycles(ms_cycles(m, 5))?;
                let lcd = m.lcd();
                let now = m.cycles();
                let mut settled = now >= s.cap;
                if s.last.as_ref() != Some(&lcd) {
                    s.last = Some(lcd);
                    s.since = now;
                } else if m.is_shutdown() && now - s.since >= ms_cycles(m, STABLE_MS) {
                    settled = true;
                }
                if !settled {
                    return Ok(Some(Running::Settle(s)));
                }
                // The server's bytes are no one's.
                self.wire.discard(m);
                Ok(None)
            }
        }
    }

    /// Press the backspace key, the first of `left` presses.
    pub(super) fn press_drop(&mut self, m: &mut Machine, left: usize) -> crate::Result<Running> {
        m.key_down(Key::Backspace)?;
        let up = m.cycles() + ms_cycles(m, crate::typing::HOLD_MS);
        Ok(Running::Drop {
            left,
            up,
            next: up + ms_cycles(m, crate::typing::KEY_CAP_MS),
        })
    }

    /// Start `step`.
    pub(super) fn begin(&mut self, m: &mut Machine, step: Step) -> crate::Result<()> {
        let failed = self.error.is_some() || self.result.error.is_some();
        if failed && !step.cleanup() {
            return Ok(());
        }
        let step = match step {
            Step::Unarchive => match &self.leftovers.backup {
                Some(n) if self.leftovers.archive => Step::Host {
                    text: Leftovers::unarchive(n),
                    cleanup: true,
                },
                _ => return Ok(()),
            },
            step => step,
        };
        let command = match &step {
            Step::Type(text) | Step::TypeCleanup(text) => {
                self.entering = text == "SERVER";
                match Job::new(m, Verb::Run, text) {
                    Ok(job) => self.running = Some(Running::Typing(Box::new(job))),
                    Err(e) => self.fail(e.to_string()),
                }
                return Ok(());
            }
            Step::WaitNak => {
                self.entering = false;
                let cap = m.cycles() + cycles(m, SERVER_CAP);
                self.running = Some(Running::WaitNak { cap });
                return Ok(());
            }
            Step::Settle => {
                self.running = Some(Running::Settle(SettleState::new(m, FINISH_SETTLE)));
                return Ok(());
            }
            Step::Algebraic => {
                if rpn_now(m) {
                    match Job::new(m, Verb::Run, TO_ALGEBRAIC) {
                        Ok(job) => {
                            self.restoring = true;
                            self.running = Some(Running::Typing(Box::new(job)));
                        }
                        Err(e) => self.restored(m, Some(e.to_string())),
                    }
                }
                return Ok(());
            }
            Step::CancelLine => {
                self.running = Some(Running::CancelLine {
                    phase: CancelPhase::Idle,
                    until: m.cycles() + ms_cycles(m, 1_000),
                });
                return Ok(());
            }
            Step::EndServer => {
                self.running = Some(self.press_on(m));
                return Ok(());
            }
            Step::Ask { end } => {
                let mut c = config();
                c.timeout = ASK_TIMEOUT;
                c.retries = PROBE_RETRIES;
                let x = Exchange::with(&mut self.wire, m, Command::Info, c)?;
                self.asking = Some(*end);
                self.running = Some(Running::Probe {
                    x: Box::new(x),
                    until: m.cycles() + ms_cycles(m, ASK_CAP_MS),
                });
                return Ok(());
            }
            Step::DropTo(depth) => {
                let now = UserMemory::of(m)
                    .and_then(|u| u.stack_addresses())
                    .map_err(|e| format!("{e:#}"))?
                    .len();
                // What a push left is the write's, not an echo.
                let depth = depth + self.pushed;
                if now > depth {
                    self.running = Some(self.press_drop(m, now - depth)?);
                }
                return Ok(());
            }
            Step::Free { .. } => Command::Host(b"MEM".to_vec()),
            Step::Unarchive => return Ok(()),
            Step::Host { text, .. } | Step::Push { text } | Step::Compile { text } => {
                self.leftovers.started(text);
                let bytes = charset::encode_command(text)
                    .map_err(|c| format!("{c:?} is not in the HP character set"))?;
                Command::Host(bytes)
            }
            Step::Send { name, data } => Command::Send(vec![OutgoingFile {
                name: charset::encode(name)
                    .map_err(|c| format!("{c:?} is not in the HP character set"))?,
                data: data.clone(),
            }]),
            Step::Get { name, .. } => Command::Get(
                charset::encode(name)
                    .map_err(|c| format!("{c:?} is not in the HP character set"))?,
            ),
            Step::Finish => {
                if !self.server {
                    return Ok(());
                }
                Command::Finish
            }
        };
        let x = Exchange::new(&mut self.wire, m, command)?;
        self.running = Some(Running::Exchange(Box::new(x), step));
        Ok(())
    }

    /// A transaction of `step` completed.
    pub(super) fn finished(&mut self, step: Step, t: Transcript) {
        let push = matches!(step, Step::Push { .. });
        match step {
            Step::Host { text, .. } | Step::Push { text } => {
                let reply = tfile::parse_stack(&charset::decode(&t.text));
                let levels = reply.levels.len();
                let Some(error) = reply.error else {
                    self.leftovers.done(&text);
                    self.baseline.get_or_insert(levels);
                    if push {
                        self.pushed += 1;
                    }
                    return;
                };
                self.leftovers.failed(&text);
                if self.error.is_none() {
                    let what = if text.is_empty() {
                        "reading the stack"
                    } else {
                        &text
                    };
                    self.error = Some(format!("{what}: {error}"));
                }
                // What the failed command left behind goes first.
                if let Some(extra) = self.baseline.and_then(|b| levels.checked_sub(b))
                    && extra > 0
                {
                    self.steps.push_front(Step::Host {
                        text: format!("{extra} DROPN"),
                        cleanup: true,
                    });
                }
            }
            Step::Compile { .. } => {
                let reply = tfile::parse_stack(&charset::decode(&t.text));
                let levels = reply.levels.len();
                let extra = self.baseline.and_then(|b| levels.checked_sub(b));
                let refusal = match (&reply.error, extra) {
                    (Some(e), _) => Some(e.clone()),
                    (None, Some(2)) => match reply.level(1).and_then(reply_count) {
                        Some(1) => None,
                        Some(0) => Some("the text holds no object".to_string()),
                        Some(_) => Some("the text holds more than one object".to_string()),
                        None => Some("the calculator's reply could not be read".to_string()),
                    },
                    _ => Some("the calculator's reply could not be read".to_string()),
                };
                if let Some(why) = refusal {
                    self.result.error = Some(why);
                    // What the compile left goes first.
                    if let Some(extra) = extra
                        && extra > 0
                    {
                        self.steps.push_front(Step::Host {
                            text: format!("{extra} DROPN"),
                            cleanup: true,
                        });
                    }
                }
            }
            Step::Send { name, .. } if self.temp_variable => {
                let stored = t.stored.into_iter().next();
                if stored.as_deref() != Some(name.as_str()) {
                    self.fail(format!(
                        "the calculator stored the file as {}, not {name}",
                        stored.unwrap_or_default()
                    ));
                } else if self
                    .leftovers
                    .replace
                    .as_ref()
                    .is_some_and(|(t, _, _)| *t == name)
                {
                    self.leftovers.temp = true;
                }
            }
            Step::Send { .. } => self.result.name = t.stored.into_iter().next(),
            Step::Free { needed, refusal } => {
                let reply = tfile::parse_stack(&charset::decode(&t.text));
                let levels = reply.levels.len();
                // What it left goes, whatever it says.
                if let Some(extra) = self.baseline.and_then(|b| levels.checked_sub(b))
                    && extra > 0
                {
                    self.steps.push_front(Step::Host {
                        text: format!("{extra} DROPN"),
                        cleanup: true,
                    });
                }
                let free = match (&reply.error, reply.level(1).and_then(parse_number)) {
                    (Some(e), _) => return self.fail(format!("MEM: {e}")),
                    (None, None) => {
                        return self.fail("MEM: the calculator's reply could not be read".into());
                    }
                    (None, Some(free)) => free,
                };
                self.result.free = Some(free);
                if free < needed {
                    self.fail(format!(
                        "{refusal}{needed} bytes free and the calculator has {free}"
                    ));
                }
            }
            Step::Get { name, text } => match t.files.into_iter().next() {
                Some(file) => {
                    let file = if text { file } else { trim_fetched(file) };
                    self.result.size = Some(file.len());
                    self.fetched = Some(file);
                }
                None => self.fail(format!("GET {name}: no file came back")),
            },
            Step::Finish => {
                self.server = false;
                self.probe_finish = false;
            }
            _ => {}
        }
    }

    /// Record `error`; the cleanup steps still run.
    pub(super) fn fail(&mut self, error: String) {
        self.error.get_or_insert(error);
    }

    /// The keys that set -95 again are done (`refused`: why they failed):
    /// -95 must read set now, or the 49G stays in RPN mode, which is said
    /// with whatever else the write reports.
    pub(super) fn restored(&mut self, m: &Machine, refused: Option<String>) {
        let why = refused.or_else(|| rpn_now(m).then(|| "flag -95 is still clear".to_string()));
        if let Some(why) = why {
            self.restore_error
                .get_or_insert(format!("the 49G stays in RPN mode: {why}"));
        }
    }

    /// The machine failed: let go of every key, run nothing more.
    pub(super) fn abandon(mut self, m: &mut Machine) {
        match self.running.take() {
            Some(Running::Typing(mut job)) => job.stop(m),
            Some(Running::Drop { .. }) => {
                let _ = m.key_up(Key::Backspace);
            }
            Some(Running::EndServer { .. } | Running::CancelLine { .. }) => {
                let _ = m.key_up(Key::On);
            }
            _ => {}
        }
        // A given-up write owns no inbound bytes (a probe's or a packet's).
        m.serial_clear_inbound();
        self.wire.discard(m);
    }

    /// Press ON to end a server.
    pub(super) fn press_on(&mut self, m: &mut Machine) -> Running {
        let _ = m.key_down(Key::On);
        Running::EndServer {
            phase: EndPhase::Hold,
            until: m.cycles() + ms_cycles(m, ON_HOLD_MS),
        }
    }

    /// Ask the server, after ON, whether it still runs: an I packet, which
    /// a running server answers at once. Its silence cannot tell: an idle
    /// server NAKs once and then stays silent while it runs (wiki:
    /// protocols/server-commands).
    pub(super) fn probe(&mut self, m: &mut Machine) -> crate::Result<Running> {
        self.probes += 1;
        let x = self.probe_exchange(m)?;
        Ok(Running::Probe {
            x: Box::new(x),
            until: m.cycles() + ms_cycles(m, PROBE_CAP_MS),
        })
    }

    /// The probe's I exchange.
    pub(super) fn probe_exchange(&mut self, m: &mut Machine) -> crate::Result<Exchange> {
        let mut c = config();
        c.timeout = PROBE_TIMEOUT;
        c.retries = PROBE_RETRIES;
        Exchange::with(&mut self.wire, m, Command::Info, c)
    }

    /// The probe is over; `answered`: the calculator sent a packet. A
    /// server that answered is back in its command wait and is ended with
    /// `G F` and the wait for the stack; one that did not has ended (the
    /// I packet's bytes still on their way to it are dropped).
    pub(super) fn probed(&mut self, m: &mut Machine, answered: bool) {
        if let Some(end) = self.asking.take() {
            // Step::Ask: a server that answers is in its command wait.
            self.result.server = Some(answered);
            if answered && end {
                // A `G F` that fails gets ON and the probe, as after ON.
                self.server = true;
                self.probe_finish = true;
                self.steps.push_front(Step::Settle);
                self.steps.push_front(Step::Finish);
            } else if !answered {
                m.serial_clear_inbound();
            }
            self.wire.discard(m);
            return;
        }
        if answered {
            self.server = true;
            self.probe_finish = true;
            self.steps.push_front(Step::Settle);
            self.steps.push_front(Step::Finish);
        } else {
            self.server = false;
            m.serial_clear_inbound();
        }
        self.wire.discard(m);
    }

    /// The server no longer answers as it should: record `error`, drop the
    /// plan but what follows the server, and end it with ON.
    pub(super) fn abort(&mut self, m: &mut Machine, error: String) -> Option<Running> {
        self.error.get_or_insert(error);
        self.steps.retain(Step::after_server);
        if !self.server {
            return None;
        }
        Some(self.press_on(m))
    }

    /// Stop where it is (the host gave up): every key is let go at once,
    /// and the rest becomes cleanup that runs in turns like any step
    /// ([`Transfer::step`] until done): a line typed in part cancelled, a
    /// running server ended with ON (and `G F` if it still answers), the 49G
    /// back in algebraic mode. The result is then the error [`STOPPED`],
    /// or why algebraic mode could not be set again.
    pub fn stop(&mut self, m: &mut Machine) {
        if self.stopped || self.done {
            self.done = true;
            return;
        }
        self.stopped = true;
        self.error.get_or_insert_with(|| STOPPED.to_string());
        let running = self.running.take();
        let waiting = matches!(running, Some(Running::WaitNak { .. }));
        // `probeServer`'s question leaves a server running: a stop in it
        // only drops its bytes; `endServer`'s ends one as after ON.
        let asking_only = self.asking == Some(false);
        let ending = matches!(
            running,
            Some(Running::EndServer { .. } | Running::Probe { .. })
        ) && !asking_only;
        if asking_only {
            m.serial_clear_inbound();
            self.wire.discard(m);
        }
        let typing = matches!(running, Some(Running::Typing(_)));
        match running {
            Some(Running::Typing(mut job)) => job.stop(m),
            Some(Running::Drop { .. }) => {
                let _ = m.key_up(Key::Backspace);
            }
            Some(Running::EndServer { .. } | Running::CancelLine { .. }) => {
                let _ = m.key_up(Key::On);
            }
            _ => {}
        }
        self.restoring = false;
        self.asking = None;
        self.steps.clear();
        if typing {
            self.steps.push_back(Step::CancelLine);
        }
        if self.server || waiting || ending || std::mem::take(&mut self.entering) {
            self.steps.push_back(Step::EndServer);
        }
        // What a backup made goes, typed once the server has ended (its
        // host commands cannot run in a server a stop interrupted).
        // What a command left on the stack (MEM's number, a recalled
        // archive, a failed command's arguments) goes first: the stack had
        // `baseline` levels; a recalled object would keep the port's copy
        // in use.
        if let Some(depth) = self.baseline.or(self.algebraic) {
            self.steps.push_back(Step::DropTo(depth));
        }
        for line in self.leftovers.typed_cleanup() {
            self.steps.push_back(Step::TypeCleanup(line));
        }
        if let Some(depth) = self.algebraic {
            self.steps.push_back(Step::Settle);
            self.steps.push_back(Step::Algebraic);
            self.steps.push_back(Step::DropTo(depth));
        }
    }

    /// The result of a write that is done: the reply and, for a fetch, the
    /// file.
    pub fn finish(self, m: &Machine) -> crate::Result<(TransferResult, Option<Vec<u8>>)> {
        // The new object kept under its temporary name is said too; after
        // a stop it is all that is said.
        let note = self.leftovers.note();
        let error = match (self.error, note) {
            (Some(e), Some(n)) if e == STOPPED => Some(n),
            (Some(e), Some(n)) => Some(format!("{e}; {n}")),
            (e, _) => e,
        };
        // A failed return to algebraic mode is said with the rest.
        match (error, self.restore_error) {
            (Some(e), Some(r)) if e == STOPPED => return Err(r.into()),
            (Some(e), Some(r)) => return Err(format!("{e}; {r}").into()),
            (Some(e), None) | (None, Some(e)) => return Err(e.into()),
            (None, None) => {}
        }
        let mut result = self.result;
        result.emulated_ms =
            (m.cycles() - self.started) as f64 * 1000.0 / f64::from(m.model().clock_hz());
        Ok((result, self.fetched))
    }
}
