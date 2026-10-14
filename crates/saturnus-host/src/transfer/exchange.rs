//! One Kermit transaction on the emulated wire: `kermit-proto`'s client
//! stepped in turns.

use super::*;

/// Whether `input`, what the calculator sent during a probe, is a server's
/// answer: a Kermit packet (its ACK, a NAK, an E). A calculator back at its
/// stack sends nothing.
pub(super) fn is_answer(input: &[u8]) -> bool {
    input.contains(&kermit_proto::codec::SOH)
}

/// What one Kermit transaction produced.
#[derive(Debug, Default)]
pub(super) struct Transcript {
    pub(super) files: Vec<Vec<u8>>,
    pub(super) stored: Vec<String>,
    pub(super) text: Vec<u8>,
}

/// Where a transaction's driver is.
#[derive(Debug)]
pub(super) enum Phase {
    /// Hand out packets and events, then read.
    Pump,
    /// Run until `until` (a cycle count), then put `packet` on the line.
    Turnaround { until: u64, packet: Vec<u8> },
    /// Collect the reply until the line is quiet or `deadline`.
    Read {
        deadline: Instant,
        busy: Duration,
        last_byte: u64,
    },
}

/// One Kermit transaction in progress.
#[derive(Debug)]
pub(super) struct Exchange {
    pub(super) client: Client,
    pub(super) sending: bool,
    pub(super) out: VecDeque<Vec<u8>>,
    pub(super) phase: Phase,
    pub(super) transcript: Transcript,
    pub(super) current: Option<Vec<u8>>,
    pub(super) outcome: Option<Result<(), Failure>>,
    /// The calculator sent a packet ([`is_answer`]).
    pub(super) answered: bool,
}

/// Whether `packet` (SOH, LEN, SEQ, TYPE, ...) opens a transaction.
pub(super) fn opens_transaction(packet: &[u8]) -> bool {
    let start = packet.iter().position(|&b| b == 0x01);
    start
        .and_then(|i| packet.get(i + 3))
        .is_some_and(|t| matches!(t, b'S' | b'R' | b'I' | b'G' | b'C'))
}

/// Why a Kermit transaction failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    /// The calculator refused it with an E packet: its server is alive
    /// and waits for the next command.
    Remote(String),
    /// A timeout or a protocol error: the server cannot be trusted to
    /// answer any more.
    Link(String),
}

/// `kermit-proto`'s defaults with a shorter timeout, fewer retries and no
/// linger (the next command comes after the turnaround anyway).
pub(super) fn config() -> Config {
    let mut c = Config::default();
    c.timeout = TIMEOUT;
    c.retries = RETRIES;
    c.linger = Duration::ZERO;
    c
}

pub(super) fn ms_cycles(m: &Machine, ms: u64) -> u64 {
    ms * u64::from(m.model().clock_hz()) / 1000
}

impl Exchange {
    pub(super) fn new(
        wire: &mut Wire,
        m: &mut Machine,
        command: Command,
    ) -> crate::Result<Exchange> {
        Self::with(wire, m, command, config())
    }

    pub(super) fn with(
        wire: &mut Wire,
        m: &mut Machine,
        command: Command,
        config: Config,
    ) -> crate::Result<Exchange> {
        wire.discard(m);
        let sending = matches!(command, Command::Send(_));
        let mut client = Client::new(config);
        client
            .start(wire.now(), command)
            .map_err(|e| format!("Kermit: {e}"))?;
        Ok(Exchange {
            client,
            sending,
            out: VecDeque::new(),
            phase: Phase::Pump,
            transcript: Transcript::default(),
            current: None,
            outcome: None,
            answered: false,
        })
    }

    /// Run until `stop` (a cycle count); the transcript once done.
    pub(super) fn step(
        &mut self,
        wire: &mut Wire,
        m: &mut Machine,
        stop: u64,
    ) -> crate::Result<Option<Result<Transcript, Failure>>> {
        loop {
            match &mut self.phase {
                Phase::Pump => {
                    let now = wire.now();
                    while let Some(p) = self.client.poll_output(now) {
                        self.out.push_back(p);
                    }
                    self.events();
                    if let Some(packet) = self.out.pop_front() {
                        if opens_transaction(&packet) {
                            let until = m.cycles() + ms_cycles(m, TURNAROUND_MS);
                            self.phase = Phase::Turnaround { until, packet };
                        } else {
                            m.serial_push(&packet);
                        }
                        continue;
                    }
                    let deadline = match (self.client.next_timeout(), &self.outcome) {
                        (None, Some(_)) => {
                            let outcome = self.outcome.take().unwrap_or(Ok(()));
                            let t = std::mem::take(&mut self.transcript);
                            return Ok(Some(outcome.map(|()| t)));
                        }
                        (Some(t), _) => t.max(now + MIN_WAIT),
                        (None, None) => now + IDLE_WAIT,
                    };
                    self.phase = Phase::Read {
                        deadline,
                        busy: Duration::ZERO,
                        last_byte: m.cycles(),
                    };
                }
                Phase::Turnaround { until, packet } => {
                    if m.cycles() < *until {
                        if m.cycles() >= stop {
                            return Ok(None);
                        }
                        let n = (*until).min(stop) - m.cycles();
                        wire.run(m, n)?;
                    } else {
                        m.serial_push(packet);
                        self.phase = Phase::Pump;
                    }
                }
                Phase::Read {
                    deadline,
                    busy,
                    last_byte,
                } => {
                    let quiet = !wire.rx.is_empty()
                        && m.cycles() - *last_byte >= ms_cycles(m, QUIET_MS)
                        && m.serial_pending() == 0;
                    if quiet || wire.now() >= *deadline {
                        let input: Vec<u8> = wire.rx.drain(..).collect();
                        let now = wire.now();
                        if !input.is_empty() {
                            self.answered |= is_answer(&input);
                            self.client.handle_input(now, &input);
                        }
                        self.client.handle_timeout(now);
                        self.phase = Phase::Pump;
                        continue;
                    }
                    if m.cycles() >= stop {
                        return Ok(None);
                    }
                    let step = Duration::from_millis(READ_STEP_MS);
                    if wire.run(m, ms_cycles(m, READ_STEP_MS))? {
                        *last_byte = m.cycles();
                    } else if wire.rx.is_empty() && !m.is_shutdown() && *busy < MAX_BUSY {
                        // Still computing the reply: not idle time.
                        *busy += step;
                        continue;
                    }
                    wire.idle += step;
                }
            }
        }
    }

    pub(super) fn events(&mut self) {
        while let Some(event) = self.client.poll_event() {
            match event {
                Event::FileStart { name } if self.sending => {
                    self.transcript.stored.push(charset::decode(&name));
                }
                Event::FileStart { .. } => self.current = Some(Vec::new()),
                Event::Data(data) => {
                    if let Some(file) = self.current.as_mut() {
                        file.extend_from_slice(&data);
                    }
                }
                Event::FileEnd { discarded } => {
                    if let Some(file) = self.current.take()
                        && !discarded
                    {
                        self.transcript.files.push(file);
                    }
                }
                Event::ServerText(text) => self.transcript.text.extend_from_slice(&text),
                Event::Done => self.outcome = Some(Ok(())),
                Event::Error(kermit_proto::Error::Remote(text)) => {
                    self.outcome = Some(Err(Failure::Remote(format!(
                        "calculator error: {}",
                        charset::decode(&text)
                    ))));
                }
                Event::Error(e) => self.outcome = Some(Err(Failure::Link(format!("Kermit: {e}")))),
                _ => {}
            }
        }
    }
}
