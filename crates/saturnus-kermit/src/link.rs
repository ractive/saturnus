//! The serial link between the Kermit client and the emulated machine, in
//! process and single-threaded, on emulated time only.
//!
//! The machine runs only while the link waits for it; nothing advances it
//! in the background. The Kermit client's clock is the link's
//! [`Link::now`]: a fixed start plus the emulated time the calculator spent
//! idle while a reply was awaited, so the same exchange always leaves the
//! same machine state and no wall-clock time is involved.
//!
//! - [`Link::read`] runs the machine in 1 ms emulated steps until it has
//!   transmitted something and the line has gone quiet for [`QUIET`], or
//!   until the client's deadline. While nothing has come back yet and the
//!   CPU is busy (not in SHUTDN at the end of a step), the step does not
//!   advance the clock: the calculator answers a host command only when it
//!   is done, and an idle server sits in SHUTDN between bytes (over 99% of
//!   1 ms samples on the 48SX, 48GX and both 49G ROMs). A long computation
//!   then returns its reply instead of timing the client out. At most
//!   [`MAX_BUSY`] of such time per read.
//! - [`Link::write_packet`] runs the machine for [`TURNAROUND`] before a
//!   packet that opens a transaction (calculator quirk: a command right
//!   after the final ACK is lost), then queues the packet, which the
//!   emulated UART receives at line rate.

use std::collections::VecDeque;
use std::time::Duration;

use anyhow::{Result, bail};
use kermit_proto::time::Instant;
use saturnus_drive::session::Session;

/// Emulated pause before a packet that opens a Kermit transaction (100 ms
/// was enough on all three models).
pub const TURNAROUND: Duration = Duration::from_millis(200);
/// Longest emulated time one read keeps running a busy calculator beyond
/// the client's deadline.
pub const MAX_BUSY: Duration = Duration::from_secs(600);
/// The error text of a read stopped by [`Link::set_read_cap`].
pub const READ_CAP_MESSAGE: &str = "emulated time limit reached while waiting for the calculator";
/// Emulated time run per step while reading.
const STEP: Duration = Duration::from_millis(1);
/// Emulated quiet time after the last transmitted byte that ends a read: a
/// byte takes about 1 ms at 9600 baud and the calculator sends a packet
/// back to back.
const QUIET: Duration = Duration::from_millis(4);

/// The emulated calculator and the bytes it sent that nobody read yet.
#[derive(Debug)]
pub struct Link {
    /// The machine and its scripted-session bookkeeping.
    pub session: Session,
    /// Transmitted bytes not handed to the client yet.
    rx: VecDeque<u8>,
    /// The client's clock at zero idle time.
    start: Instant,
    /// Idle emulated time spent waiting for replies.
    idle: Duration,
    /// The limit on a host command's computing time, see
    /// [`Link::set_read_cap`].
    read_cap: ReadCap,
    /// Whether a read stopped at `read_cap` since the last
    /// [`Link::take_read_cap_hit`].
    read_cap_hit: bool,
}

/// The state of [`Link::set_read_cap`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadCap {
    /// No limit.
    Off,
    /// A limit of this much emulated time, not started yet.
    Pending(Duration),
    /// The limit ends at this cycle count.
    Armed(u64),
}

impl Link {
    /// A link to the machine of `session`.
    pub fn new(session: Session) -> Self {
        Link {
            session,
            rx: VecDeque::new(),
            start: Instant::now(),
            idle: Duration::ZERO,
            read_cap: ReadCap::Off,
            read_cap_hit: false,
        }
    }

    /// The Kermit client's clock.
    pub fn now(&self) -> Instant {
        self.start + self.idle
    }

    /// Emulated time `d` in CPU cycles.
    pub fn cycles_in(&self, d: Duration) -> u64 {
        let hz = u128::from(self.session.machine.model().clock_hz());
        u64::try_from(d.as_nanos() * hz / 1_000_000_000).unwrap_or(u64::MAX)
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

    /// Advance the read cap: arm it once the inbound queue is empty, lift
    /// it once a reply packet has started. Returns whether it is exceeded.
    fn read_cap_exceeded(&mut self) -> bool {
        let now = self.session.machine.cycles();
        match self.read_cap {
            ReadCap::Pending(d) if self.session.machine.serial_pending() == 0 => {
                self.read_cap = ReadCap::Armed(now.saturating_add(self.cycles_in(d)));
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

    /// Run `n` cycles and collect what the calculator transmitted; returns
    /// whether it transmitted anything.
    pub fn run(&mut self, n: u64) -> Result<bool> {
        self.session.run(n)?;
        let out = self.session.machine.serial_drain();
        self.rx.extend(&out);
        Ok(!out.is_empty())
    }

    /// Drop everything the calculator sent so far, collected or not (the
    /// idle server's periodic NAKs while no exchange ran).
    pub fn discard_input(&mut self) {
        self.rx.clear();
        self.session.machine.serial_drain();
    }

    /// Run until the calculator has sent a Kermit NAK (SOH, LEN, SEQ, `N`),
    /// at most `cap` of emulated time: proof that the server runs. The
    /// collected bytes are dropped.
    pub fn wait_for_nak(&mut self, cap: Duration) -> Result<bool> {
        let step = self.cycles_in(Duration::from_millis(10));
        let end = self
            .session
            .machine
            .cycles()
            .saturating_add(self.cycles_in(cap));
        while self.session.machine.cycles() < end {
            self.run(step)?;
            let bytes = self.rx.make_contiguous();
            if bytes.windows(4).any(|w| w[0] == 0x01 && w[3] == b'N') {
                self.rx.clear();
                return Ok(true);
            }
        }
        self.rx.clear();
        Ok(false)
    }

    /// Queue `packet` for the calculator, after the [`TURNAROUND`] when it
    /// opens a transaction.
    pub fn write_packet(&mut self, packet: &[u8]) -> Result<()> {
        if opens_transaction(packet) {
            let n = self.cycles_in(TURNAROUND);
            self.run(n)?;
        }
        self.session.machine.serial_push(packet);
        Ok(())
    }

    /// What the calculator sends until its output goes quiet, or nothing
    /// once the clock reaches `deadline`. Fails past the read cap.
    pub fn read(&mut self, deadline: Instant) -> Result<Vec<u8>> {
        let step = self.cycles_in(STEP);
        let quiet = self.cycles_in(QUIET);
        let mut busy = Duration::ZERO;
        let mut last_byte = self.session.machine.cycles();
        while self.now() < deadline {
            if self.read_cap_exceeded() {
                bail!(READ_CAP_MESSAGE);
            }
            // Input still on its way in means more is coming back;
            // otherwise stop once the output has gone quiet.
            if !self.rx.is_empty()
                && self.session.machine.cycles() - last_byte >= quiet
                && self.session.machine.serial_pending() == 0
            {
                break;
            }
            if self.run(step)? {
                last_byte = self.session.machine.cycles();
            } else if self.rx.is_empty() && !self.session.machine.is_shutdown() && busy < MAX_BUSY {
                // Still computing the reply: this step is not idle time.
                busy += STEP;
                continue;
            }
            self.idle += STEP;
        }
        Ok(self.rx.drain(..).collect())
    }
}

/// Whether `rx` holds the start of a packet other than a NAK (SOH, LEN,
/// SEQ, TYPE): the calculator has started to answer.
fn reply_started(rx: &[u8]) -> bool {
    rx.iter()
        .enumerate()
        .any(|(i, &b)| b == 0x01 && rx.get(i + 3).is_some_and(|&t| t != b'N'))
}

/// Whether `packet` (SOH, LEN, SEQ, TYPE, ...) is a client's first packet
/// of a transaction: send-init, receive-init, info, generic or host
/// command.
fn opens_transaction(packet: &[u8]) -> bool {
    let start = packet.iter().position(|&b| b == 0x01);
    start
        .and_then(|i| packet.get(i + 3))
        .is_some_and(|t| matches!(t, b'S' | b'R' | b'I' | b'G' | b'C'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_start_with_a_packet_other_than_a_nak() {
        assert!(reply_started(b"\r\x01+ S~*"));
        assert!(!reply_started(b"\x01# N3\r"));
        assert!(!reply_started(b"\x01+ "));
        assert!(!reply_started(b""));
    }

    #[test]
    fn transaction_openers() {
        // SOH, LEN, SEQ, TYPE; a host command and an ACK.
        assert!(opens_transaction(b"\x01& C1 2 +X\r"));
        assert!(opens_transaction(b"\x01#\x20S~"));
        assert!(!opens_transaction(b"\x01#!Y5\r"));
        assert!(!opens_transaction(b""));
    }
}
