//! One Kermit transaction on the emulated wire: `kermit-proto`'s client
//! stepped in turns. The one Kermit client of the emulated calculator:
//! the engine's hidden writes step it in turns, `saturnus-kermit` (the
//! reference generator, the ROM-gated Kermit tests) runs it to its end.

use super::*;

/// Whether `input`, what the calculator sent during a probe, is a server's
/// answer: a Kermit packet (its ACK, a NAK, an E). A calculator back at its
/// stack sends nothing.
pub(super) fn is_answer(input: &[u8]) -> bool {
    input.contains(&kermit_proto::codec::SOH)
}

/// What one Kermit transaction produced.
#[derive(Debug, Default)]
pub struct Transcript {
    /// The files received (GET), the discarded ones left out.
    pub files: Vec<Vec<u8>>,
    /// The names the calculator stored sent files under (SEND).
    pub stored: Vec<String>,
    /// The server's text (host command and directory replies), raw HP
    /// bytes.
    pub text: Vec<u8>,
}

/// Where a transaction's driver is.
#[derive(Debug)]
pub(super) enum Phase {
    /// Hand out packets and events, then read.
    Pump,
    /// The wire runs a packet's turnaround or a read ([`Wire::step`]).
    Wire,
}

/// One Kermit transaction in progress.
#[derive(Debug)]
pub struct Exchange {
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

/// Why a Kermit transaction failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The calculator refused it with an E packet: its server is alive
    /// and waits for the next command.
    Remote(String),
    /// A timeout or a protocol error: the server cannot be trusted to
    /// answer any more.
    Link(String),
}

/// `kermit-proto`'s defaults with a shorter timeout, fewer retries and no
/// linger (the next command comes after the turnaround anyway).
pub fn config() -> Config {
    let mut c = Config::default();
    c.timeout = TIMEOUT;
    c.retries = RETRIES;
    c.linger = Duration::ZERO;
    c
}

pub(super) fn ms_cycles(m: &Machine, ms: u64) -> u64 {
    ms * u64::from(m.model().clock_hz()) / 1000
}

impl Failure {
    /// The message.
    pub fn message(&self) -> &str {
        match self {
            Failure::Remote(e) | Failure::Link(e) => e,
        }
    }
}

impl Exchange {
    /// Start `command` on `wire` with [`config`]; input the calculator
    /// sent before (the idle server's NAKs) is dropped first.
    pub fn new<L: Line + ?Sized>(
        wire: &mut Wire,
        m: &mut L,
        command: Command,
    ) -> crate::Result<Exchange> {
        Self::with(wire, m, command, config())
    }

    /// [`Exchange::new`] with `config`.
    pub fn with<L: Line + ?Sized>(
        wire: &mut Wire,
        m: &mut L,
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

    /// Run until `stop` (a cycle count); the transcript once done. Fails
    /// when the machine or the wire does (the read cap).
    pub fn step<L: Line + ?Sized>(
        &mut self,
        wire: &mut Wire,
        m: &mut L,
        stop: u64,
    ) -> crate::Result<Option<Result<Transcript, Failure>>> {
        loop {
            match self.phase {
                Phase::Pump => {
                    let now = wire.now();
                    while let Some(p) = self.client.poll_output(now) {
                        self.out.push_back(p);
                    }
                    self.events();
                    if let Some(packet) = self.out.pop_front() {
                        if !wire.start_write(m, packet) {
                            self.phase = Phase::Wire;
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
                    wire.start_read(m, deadline);
                    self.phase = Phase::Wire;
                }
                Phase::Wire => {
                    let budget = stop.saturating_sub(m.cycles());
                    match wire.step(m, budget)? {
                        WireStep::Pending => return Ok(None),
                        WireStep::Sent => {}
                        WireStep::Quiet(input) | WireStep::Deadline(input) => {
                            let now = wire.now();
                            if !input.is_empty() {
                                self.answered |= is_answer(&input);
                                self.client.handle_input(now, &input);
                            }
                            self.client.handle_timeout(now);
                        }
                    }
                    self.phase = Phase::Pump;
                }
            }
        }
    }

    fn events(&mut self) {
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
