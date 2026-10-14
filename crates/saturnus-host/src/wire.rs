//! The emulated serial wire: the calculator's serial port driven on
//! emulated time only, the one implementation every in-process Kermit
//! host uses (the hidden writes of [`crate::transfer`], which the emulator
//! page, the desktop app and `saturnus run` step in turns, and
//! `saturnus-kermit`, which the reference generator and the ROM-gated
//! Kermit tests block on). Wasm-safe: no threads, no I/O, no wall clock
//! (the client's clock is a fixed start plus emulated time).
//!
//! The rules, one set for all of them:
//!
//! - **The clock.** A Kermit client's clock is [`Wire::now`]: a fixed start
//!   plus the emulated time the calculator idled while a reply was
//!   awaited, so the same exchange always leaves the same machine state.
//! - **A read** ([`Wire::start_read`], then [`Wire::step`]) runs the
//!   machine in [`STEP`]s until it has transmitted something and the line
//!   has gone [`QUIET`] (bytes still on their way in mean more is coming
//!   back), or until the client's deadline. While nothing has come back
//!   and the CPU is busy (not in SHUTDN at the end of a step), the step
//!   does not advance the clock: the calculator answers a host command
//!   only when it is done, and an idle server sits in SHUTDN between bytes.
//!   A long computation then returns its reply instead of timing the
//!   client out; at most [`MAX_BUSY`] of such time per read.
//! - **A write** ([`Wire::start_write`], then [`Wire::step`]) runs the
//!   machine for [`TURNAROUND`] before a packet that
//!   [opens a transaction](opens_transaction) (a command right after the
//!   final ACK of the one before is lost), then queues the packet, which
//!   the emulated UART receives at line rate.
//! - **The read cap** ([`Wire::set_read_cap`]) limits one command's
//!   computing time, busy or not.
//!
//! [`Wire::step`] is resumable: it runs at most a budget of cycles and
//! says [`WireStep::Pending`] when that ran out first, so a host stepping
//! the machine in turns calls it once per turn; a blocking caller loops it
//! ([`Wire::read`], [`Wire::write_packet`]). Wiki: protocols/kermit,
//! protocols/server-commands.

use std::collections::VecDeque;

use kermit_proto::codec::SOH;
use kermit_proto::time::{Duration, Instant};
use saturnus_core::Machine;

/// Emulated pause before a packet that opens a Kermit transaction: a
/// command right after the final ACK of the one before is lost (100 ms was
/// enough on the 48SX, 48GX and 49G).
pub const TURNAROUND: Duration = Duration::from_millis(200);
/// Shortest read wait a client gets, whatever its deadline says.
pub const MIN_WAIT: Duration = Duration::from_millis(10);
/// Read wait while a transaction runs but the client has no deadline.
pub const IDLE_WAIT: Duration = Duration::from_millis(100);
/// Reply timeout per packet on the wire's clock (idle emulated time): a
/// calculator that left server mode fails a command in seconds.
pub const TIMEOUT: Duration = Duration::from_secs(6);
/// Retransmissions per packet.
pub const RETRIES: u32 = 3;
/// Longest emulated time one read keeps running a busy calculator.
pub const MAX_BUSY: Duration = Duration::from_secs(600);
/// Emulated time run per step while reading.
pub const STEP: Duration = Duration::from_millis(1);
/// Emulated quiet time after the last transmitted byte that ends a read: a
/// byte takes about 1 ms at 9600 baud and the calculator sends a packet
/// back to back.
pub const QUIET: Duration = Duration::from_millis(4);
/// Emulated time per step while waiting for the idle server's first NAK.
pub const NAK_STEP: Duration = Duration::from_millis(10);
/// Longest emulated wait for the idle server's NAK after `SERVER`: it NAKs
/// once, about 5 s after `SERVER` (or its timeout after its last packet),
/// and then stays silent (wiki: protocols/server-commands).
pub const SERVER_CAP: Duration = Duration::from_secs(15);
/// Emulated time after `G F` before the calculator takes keys again (the
/// 48SX loses keys typed within 2 s of FINISH).
pub const FINISH_SETTLE: Duration = Duration::from_millis(2_500);
/// The error of a read stopped by [`Wire::set_read_cap`].
pub const READ_CAP_MESSAGE: &str = "emulated time limit reached while waiting for the calculator";

/// What the wire needs of the calculator: its cycle clock and its serial
/// port. [`Machine`] is one; the tests use a fake.
pub trait Line {
    /// CPU cycles run so far.
    fn cycles(&self) -> u64;
    /// CPU cycles per second.
    fn clock_hz(&self) -> u32;
    /// Run `n` cycles.
    fn run_cycles(&mut self, n: u64) -> crate::Result<()>;
    /// What the calculator transmitted since the last call.
    fn serial_drain(&mut self) -> Vec<u8>;
    /// Queue bytes for the calculator to receive at line rate.
    fn serial_push(&mut self, bytes: &[u8]);
    /// Bytes queued for the calculator and not received yet.
    fn serial_pending(&self) -> usize;
    /// Whether the CPU sleeps (SHUTDN).
    fn is_shutdown(&self) -> bool;
}

impl Line for Machine {
    fn cycles(&self) -> u64 {
        Machine::cycles(self)
    }
    fn clock_hz(&self) -> u32 {
        self.model().clock_hz()
    }
    fn run_cycles(&mut self, n: u64) -> crate::Result<()> {
        Ok(Machine::run_cycles(self, n)?)
    }
    fn serial_drain(&mut self) -> Vec<u8> {
        Machine::serial_drain(self)
    }
    fn serial_push(&mut self, bytes: &[u8]) {
        Machine::serial_push(self, bytes);
    }
    fn serial_pending(&self) -> usize {
        Machine::serial_pending(self)
    }
    fn is_shutdown(&self) -> bool {
        Machine::is_shutdown(self)
    }
}

/// `d` of emulated time in `m`'s CPU cycles.
pub fn cycles<L: Line + ?Sized>(m: &L, d: Duration) -> u64 {
    let c = d.as_nanos() * u128::from(m.clock_hz()) / 1_000_000_000;
    u64::try_from(c).unwrap_or(u64::MAX)
}

/// Whether `packet` (SOH, LEN, SEQ, TYPE, ...) is a client's first packet
/// of a transaction: send-init, receive-init, info, generic or host
/// command.
pub fn opens_transaction(packet: &[u8]) -> bool {
    let start = packet.iter().position(|&b| b == SOH);
    start
        .and_then(|i| packet.get(i + 3))
        .is_some_and(|t| matches!(t, b'S' | b'R' | b'I' | b'G' | b'C'))
}

/// Whether `rx` holds the start of a packet other than a NAK (SOH, LEN,
/// SEQ, TYPE): the calculator has started to answer.
fn reply_started(rx: &[u8]) -> bool {
    rx.iter()
        .enumerate()
        .any(|(i, &b)| b == SOH && rx.get(i + 3).is_some_and(|&t| t != b'N'))
}

/// Whether `rx` holds a Kermit NAK (SOH, LEN, SEQ, `N`).
fn holds_nak(rx: &[u8]) -> bool {
    rx.windows(4).any(|w| w[0] == SOH && w[3] == b'N')
}

/// What [`Wire::step`] came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireStep {
    /// The read ended: the calculator sent these bytes and the line went
    /// quiet.
    Quiet(Vec<u8>),
    /// The read ended at its deadline, with what came in so far (often
    /// nothing).
    Deadline(Vec<u8>),
    /// The write's packet is on the line (after its turnaround).
    Sent,
    /// The budget ran out first: step again.
    Pending,
}

/// What the wire is doing.
#[derive(Clone, Debug)]
enum Job {
    /// Nothing: [`Wire::step`] has nothing to run.
    Idle,
    /// Run until `until` (a cycle count), then put `packet` on the line.
    Write { until: u64, packet: Vec<u8> },
    /// Collect the reply until the line is quiet or `deadline`.
    Read {
        deadline: Instant,
        busy: Duration,
        last_byte: u64,
    },
}

/// The state of [`Wire::set_read_cap`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadCap {
    /// No limit.
    Off,
    /// A limit of this much emulated time, no packet written yet.
    Pending(Duration),
    /// A limit of this much emulated time, starting once the written
    /// packet has been received.
    Sent(Duration),
    /// The limit ends at this cycle count.
    Armed(u64),
}

/// The serial wire to one emulated calculator: the bytes it sent that no
/// client read yet, the client's clock, and the read or write in progress.
#[derive(Debug)]
pub struct Wire {
    /// Transmitted bytes not handed to the client yet.
    rx: VecDeque<u8>,
    /// The client's clock at zero idle time.
    start: Instant,
    /// Idle emulated time spent waiting for replies.
    idle: Duration,
    job: Job,
    read_cap: ReadCap,
    /// Whether a read stopped at the read cap since the last
    /// [`Wire::take_read_cap_hit`].
    read_cap_hit: bool,
}

impl Default for Wire {
    fn default() -> Self {
        Wire::new()
    }
}

impl Wire {
    /// A wire with nothing received, its clock at a fixed start.
    pub fn new() -> Wire {
        Wire {
            rx: VecDeque::new(),
            start: Instant::now(),
            idle: Duration::ZERO,
            job: Job::Idle,
            read_cap: ReadCap::Off,
            read_cap_hit: false,
        }
    }

    /// The Kermit client's clock.
    pub fn now(&self) -> Instant {
        self.start + self.idle
    }

    /// Run `n` cycles (at least one) and collect what the calculator
    /// transmitted; returns whether it transmitted anything.
    pub fn run<L: Line + ?Sized>(&mut self, m: &mut L, n: u64) -> crate::Result<bool> {
        m.run_cycles(n.max(1))?;
        let out = m.serial_drain();
        self.rx.extend(&out);
        Ok(!out.is_empty())
    }

    /// Drop everything the calculator sent so far, collected or not (the
    /// idle server's NAKs while no exchange ran), and any read or write in
    /// progress.
    pub fn discard<L: Line + ?Sized>(&mut self, m: &mut L) {
        self.rx.clear();
        m.serial_drain();
        self.job = Job::Idle;
    }

    /// One [`NAK_STEP`] of the wait for the idle server's NAK: true once
    /// the calculator has sent one (the bytes collected are dropped then).
    pub fn nak_step<L: Line + ?Sized>(&mut self, m: &mut L) -> crate::Result<bool> {
        self.run(m, cycles(m, NAK_STEP))?;
        if holds_nak(self.rx.make_contiguous()) {
            self.rx.clear();
            return Ok(true);
        }
        Ok(false)
    }

    /// Run until the calculator has sent a Kermit NAK, at most `cap` of
    /// emulated time: proof that the server runs. The collected bytes are
    /// dropped.
    pub fn wait_for_nak<L: Line + ?Sized>(
        &mut self,
        m: &mut L,
        cap: Duration,
    ) -> crate::Result<bool> {
        let end = m.cycles().saturating_add(cycles(m, cap));
        while m.cycles() < end {
            if self.nak_step(m)? {
                return Ok(true);
            }
        }
        self.rx.clear();
        Ok(false)
    }

    /// Limit the next command's computing time to `d` of emulated time,
    /// busy or not: the limit starts once the calculator has received the
    /// whole command packet (turnaround and line time do not count) and
    /// ends with the first reply packet that is not a NAK (sending the
    /// reply does not count). A read past it fails with
    /// [`READ_CAP_MESSAGE`]. `None` lifts it.
    pub fn set_read_cap(&mut self, d: Option<Duration>) {
        self.read_cap = d.map_or(ReadCap::Off, ReadCap::Pending);
    }

    /// Whether a read stopped at the cap since the last call; resets it.
    pub fn take_read_cap_hit(&mut self) -> bool {
        std::mem::take(&mut self.read_cap_hit)
    }

    /// Advance the read cap: arm it once the inbound queue is empty after
    /// a write, lift it once a reply packet has started. Returns whether
    /// it is exceeded.
    fn read_cap_exceeded<L: Line + ?Sized>(&mut self, m: &L) -> bool {
        let now = m.cycles();
        match self.read_cap {
            ReadCap::Sent(d) if m.serial_pending() == 0 => {
                self.read_cap = ReadCap::Armed(now.saturating_add(cycles(m, d)));
            }
            ReadCap::Armed(_) if reply_started(self.rx.make_contiguous()) => {
                self.read_cap = ReadCap::Off;
            }
            ReadCap::Armed(end) if now >= end => {
                self.read_cap_hit = true;
                return true;
            }
            _ => {}
        }
        false
    }

    /// Put `packet` on the line: at once, or (a packet that
    /// [opens a transaction](opens_transaction)) after the [`TURNAROUND`],
    /// which [`Wire::step`] runs and ends with [`WireStep::Sent`]. Returns
    /// whether it went out at once.
    pub fn start_write<L: Line + ?Sized>(&mut self, m: &mut L, packet: Vec<u8>) -> bool {
        if opens_transaction(&packet) {
            let until = m.cycles().saturating_add(cycles(m, TURNAROUND));
            self.job = Job::Write { until, packet };
            false
        } else {
            self.push(m, &packet);
            true
        }
    }

    fn push<L: Line + ?Sized>(&mut self, m: &mut L, packet: &[u8]) {
        m.serial_push(packet);
        if let ReadCap::Pending(d) = self.read_cap {
            self.read_cap = ReadCap::Sent(d);
        }
    }

    /// Start a read that ends when the line goes quiet after the
    /// calculator sent something, or once the clock reaches `deadline`;
    /// [`Wire::step`] runs it.
    pub fn start_read<L: Line + ?Sized>(&mut self, m: &L, deadline: Instant) {
        self.job = Job::Read {
            deadline,
            busy: Duration::ZERO,
            last_byte: m.cycles(),
        };
    }

    /// Run the read or write in progress for at most `budget` cycles (a
    /// read's step that starts before the budget runs out ends after it).
    /// Fails past the read cap, or when nothing is in progress.
    pub fn step<L: Line + ?Sized>(&mut self, m: &mut L, budget: u64) -> crate::Result<WireStep> {
        let stop = m.cycles().saturating_add(budget);
        loop {
            match &mut self.job {
                Job::Idle => return Err("the wire has no read or write in progress".into()),
                Job::Write { until, packet } => {
                    if m.cycles() >= *until {
                        let packet = std::mem::take(packet);
                        self.job = Job::Idle;
                        self.push(m, &packet);
                        return Ok(WireStep::Sent);
                    }
                    if m.cycles() >= stop {
                        return Ok(WireStep::Pending);
                    }
                    let n = (*until).min(stop) - m.cycles();
                    self.run(m, n)?;
                }
                Job::Read {
                    deadline,
                    last_byte,
                    ..
                } => {
                    let (deadline, last) = (*deadline, *last_byte);
                    if self.now() >= deadline {
                        self.job = Job::Idle;
                        return Ok(WireStep::Deadline(self.rx.drain(..).collect()));
                    }
                    if self.read_cap_exceeded(m) {
                        self.job = Job::Idle;
                        return Err(READ_CAP_MESSAGE.into());
                    }
                    // Input still on its way in means more is coming back;
                    // otherwise stop once the output has gone quiet.
                    if !self.rx.is_empty()
                        && m.cycles() - last >= cycles(m, QUIET)
                        && m.serial_pending() == 0
                    {
                        self.job = Job::Idle;
                        return Ok(WireStep::Quiet(self.rx.drain(..).collect()));
                    }
                    if m.cycles() >= stop {
                        return Ok(WireStep::Pending);
                    }
                    let sent = self.run(m, cycles(m, STEP))?;
                    let silent = self.rx.is_empty() && !m.is_shutdown();
                    if let Job::Read {
                        busy, last_byte, ..
                    } = &mut self.job
                    {
                        if sent {
                            *last_byte = m.cycles();
                        } else if silent && *busy < MAX_BUSY {
                            // Still computing the reply: not idle time.
                            *busy += STEP;
                            continue;
                        }
                    }
                    self.idle += STEP;
                }
            }
        }
    }

    /// Run the read or write in progress to its end.
    fn finish<L: Line + ?Sized>(&mut self, m: &mut L) -> crate::Result<WireStep> {
        loop {
            match self.step(m, u64::MAX)? {
                WireStep::Pending => {}
                done => return Ok(done),
            }
        }
    }

    /// Queue `packet` for the calculator, after the [`TURNAROUND`] when it
    /// opens a transaction (a blocking [`Wire::start_write`]).
    pub fn write_packet<L: Line + ?Sized>(
        &mut self,
        m: &mut L,
        packet: &[u8],
    ) -> crate::Result<()> {
        if !self.start_write(m, packet.to_vec()) {
            self.finish(m)?;
        }
        Ok(())
    }

    /// What the calculator sends until its output goes quiet, or what came
    /// in once the clock reaches `deadline` (a blocking
    /// [`Wire::start_read`]). Fails past the read cap.
    pub fn read<L: Line + ?Sized>(
        &mut self,
        m: &mut L,
        deadline: Instant,
    ) -> crate::Result<Vec<u8>> {
        self.start_read(m, deadline);
        match self.finish(m)? {
            WireStep::Quiet(bytes) | WireStep::Deadline(bytes) => Ok(bytes),
            WireStep::Sent | WireStep::Pending => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1000 cycles per emulated ms.
    const HZ: u32 = 1_000_000;
    const MS: u64 = 1_000;

    /// A calculator on a script: it sends `script`'s bytes at their cycle
    /// counts, computes (awake) until `busy_until`, and takes one pushed
    /// byte per ms off the line.
    #[derive(Default)]
    struct Fake {
        cycles: u64,
        script: Vec<(u64, u8)>,
        out: Vec<u8>,
        received: Vec<u8>,
        inbound: VecDeque<u8>,
        busy_until: u64,
    }

    impl Fake {
        /// `bytes` sent back to back from `at_ms`, one per ms.
        fn sends(mut self, at_ms: u64, bytes: &[u8]) -> Fake {
            for (i, &b) in bytes.iter().enumerate() {
                self.script.push(((at_ms + i as u64) * MS, b));
            }
            self
        }
    }

    impl Line for Fake {
        fn cycles(&self) -> u64 {
            self.cycles
        }
        fn clock_hz(&self) -> u32 {
            HZ
        }
        fn run_cycles(&mut self, n: u64) -> crate::Result<()> {
            let from = self.cycles;
            self.cycles += n;
            let to = self.cycles;
            for &(at, b) in &self.script {
                if at >= from && at < to {
                    self.out.push(b);
                }
            }
            for _ in (from / MS)..(to / MS) {
                if let Some(b) = self.inbound.pop_front() {
                    self.received.push(b);
                }
            }
            Ok(())
        }
        fn serial_drain(&mut self) -> Vec<u8> {
            std::mem::take(&mut self.out)
        }
        fn serial_push(&mut self, bytes: &[u8]) {
            self.inbound.extend(bytes);
        }
        fn serial_pending(&self) -> usize {
            self.inbound.len()
        }
        fn is_shutdown(&self) -> bool {
            self.cycles >= self.busy_until
        }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn transaction_openers() {
        // SOH, LEN, SEQ, TYPE; a host command and an ACK.
        assert!(opens_transaction(b"\x01& C1 2 +X\r"));
        assert!(opens_transaction(b"\x01#\x20S~"));
        assert!(!opens_transaction(b"\x01#!Y5\r"));
        assert!(!opens_transaction(b""));
    }

    #[test]
    fn replies_start_with_a_packet_other_than_a_nak() {
        assert!(reply_started(b"\r\x01+ S~*"));
        assert!(!reply_started(b"\x01# N3\r"));
        assert!(!reply_started(b"\x01+ "));
        assert!(!reply_started(b""));
    }

    #[test]
    fn a_read_ends_once_the_line_is_quiet() {
        let mut m = Fake::default().sends(5, b"\x01# Y");
        let mut w = Wire::new();
        let start = w.now();
        let got = w.read(&mut m, start + ms(1_000)).unwrap();
        assert_eq!(got, b"\x01# Y");
        // The last byte came at 8 ms, then QUIET.
        assert_eq!(m.cycles / MS, 8 + 1 + 4);
        assert_eq!(w.now() - start, ms(13), "an idle calculator's time counts");
    }

    #[test]
    fn busy_time_is_not_idle_time() {
        let mut m = Fake {
            busy_until: 300 * MS,
            ..Fake::default()
        }
        .sends(400, b"\x01# Y");
        let mut w = Wire::new();
        let start = w.now();
        // The reply comes after the deadline in emulated time, but the
        // calculator computed for 300 ms of it: those do not count.
        let got = w.read(&mut m, start + ms(200)).unwrap();
        assert_eq!(got, b"\x01# Y");
        // The last byte at 403 ms, quiet at 408 ms; the steps up to 299 ms
        // ended with the CPU awake.
        assert_eq!(m.cycles / MS, 408);
        assert_eq!(w.now() - start, ms(408 - 299));
    }

    #[test]
    fn a_read_ends_at_its_deadline() {
        let mut m = Fake::default();
        let mut w = Wire::new();
        let start = w.now();
        assert_eq!(w.read(&mut m, start + ms(20)).unwrap(), b"");
        assert_eq!(m.cycles, 20 * MS);
        assert_eq!(w.now() - start, ms(20));
        // A deadline already passed returns at once.
        w.start_read(&m, start);
        assert_eq!(
            w.step(&mut m, u64::MAX).unwrap(),
            WireStep::Deadline(vec![])
        );
        assert_eq!(m.cycles, 20 * MS);
    }

    #[test]
    fn a_read_runs_in_turns() {
        let mut blocking = Fake::default().sends(5, b"\x01# Y");
        let mut stepped = Fake::default().sends(5, b"\x01# Y");
        let (mut a, mut b) = (Wire::new(), Wire::new());
        let deadline = |w: &Wire| w.now() + ms(1_000);
        let want = a.read(&mut blocking, deadline(&a)).unwrap();
        b.start_read(&stepped, deadline(&b));
        let mut turns = 0;
        let got = loop {
            turns += 1;
            match b.step(&mut stepped, 2 * MS).unwrap() {
                WireStep::Pending => {}
                WireStep::Quiet(bytes) => break bytes,
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(got, want);
        assert_eq!(stepped.cycles, blocking.cycles);
        assert!(turns > 5, "{turns} turns");
    }

    #[test]
    fn an_opening_packet_waits_for_the_turnaround() {
        let mut m = Fake::default();
        let mut w = Wire::new();
        // An ACK goes out at once.
        assert!(w.start_write(&mut m, b"\x01#!Y5\r".to_vec()));
        assert_eq!(m.serial_pending(), 6);
        assert_eq!(m.cycles, 0);
        m.inbound.clear();
        // A host command after the turnaround, in turns.
        assert!(!w.start_write(&mut m, b"\x01& C1 2 +X\r".to_vec()));
        assert_eq!(w.step(&mut m, 150 * MS).unwrap(), WireStep::Pending);
        assert_eq!(m.serial_pending(), 0);
        assert_eq!(w.step(&mut m, 150 * MS).unwrap(), WireStep::Sent);
        assert_eq!(m.cycles, 200 * MS);
        assert_eq!(m.serial_pending(), 11);
        // Nothing in progress now.
        assert!(w.step(&mut m, MS).is_err());
    }

    #[test]
    fn the_read_cap_stops_a_long_computation() {
        let mut m = Fake {
            busy_until: u64::MAX,
            ..Fake::default()
        };
        let mut w = Wire::new();
        w.set_read_cap(Some(ms(50)));
        w.write_packet(&mut m, b"\x01#!Y5\r").unwrap();
        let start = w.now();
        let e = w.read(&mut m, start + ms(10)).unwrap_err();
        assert_eq!(e.to_string(), READ_CAP_MESSAGE);
        assert!(w.take_read_cap_hit());
        assert!(!w.take_read_cap_hit());
        // Six bytes on the line, then the cap's 50 ms of busy time.
        assert_eq!(m.cycles / MS, 6 + 50);
        assert_eq!(w.now(), start, "busy time");
    }

    #[test]
    fn the_read_cap_ends_with_the_reply() {
        let mut m = Fake::default().sends(60, b"\x01# N3\r\x01# Y");
        let mut w = Wire::new();
        w.set_read_cap(Some(ms(70)));
        w.write_packet(&mut m, b"\x01#!Y5\r").unwrap();
        let start = w.now();
        // Armed once the six bytes are in (6 ms): it ends at 76 ms. A NAK
        // does not lift it; the Y packet does, its type in at 69 ms.
        let got = w.read(&mut m, start + ms(1_000)).unwrap();
        assert_eq!(got, b"\x01# N3\r\x01# Y");
        assert!(!w.take_read_cap_hit());
    }

    #[test]
    fn the_idle_server_naks() {
        let mut m = Fake::default().sends(5_000, b"\x01# N3\r");
        let mut w = Wire::new();
        assert!(w.wait_for_nak(&mut m, SERVER_CAP).unwrap());
        assert_eq!(m.cycles, 5_010 * MS);
        let mut quiet = Fake::default();
        assert!(!w.wait_for_nak(&mut quiet, ms(100)).unwrap());
        assert_eq!(quiet.cycles, 100 * MS);
    }
}
