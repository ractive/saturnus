//! The serial link between the Kermit client and the emulated machine, in
//! process and single-threaded, on emulated time only: the products' wire,
//! [`saturnus_host::wire::Wire`], blocked on.
//!
//! The machine runs only while the link waits for it; nothing advances it
//! in the background. The Kermit client's clock is the link's
//! [`Link::now`]: a fixed start plus the emulated time the calculator spent
//! idle while a reply was awaited, so the same exchange always leaves the
//! same machine state and no wall-clock time is involved. The rules (the
//! quiet line, busy time, the turnaround before a packet that opens a
//! transaction, the read cap) are the wire's; see [`saturnus_host::wire`].

use std::time::Duration;

use anyhow::Result;
use kermit_proto::time::Instant;
use saturnus_drive::session::Session;
use saturnus_host::wire::{self, Wire};

/// The emulated calculator and the wire to it.
#[derive(Debug)]
pub struct Link {
    /// The machine and its scripted-session bookkeeping.
    pub session: Session,
    wire: Wire,
}

impl Link {
    /// A link to the machine of `session`.
    pub fn new(session: Session) -> Self {
        Link {
            session,
            wire: Wire::new(),
        }
    }

    /// The Kermit client's clock.
    pub fn now(&self) -> Instant {
        self.wire.now()
    }

    /// Emulated time `d` in CPU cycles.
    pub fn cycles_in(&self, d: Duration) -> u64 {
        wire::cycles(&self.session.machine, d)
    }

    /// Limit the next command's computing time to `d` of emulated time
    /// ([`Wire::set_read_cap`]); a read past it fails with
    /// [`wire::READ_CAP_MESSAGE`]. `None` lifts it.
    pub fn set_read_cap(&mut self, d: Option<Duration>) {
        self.wire.set_read_cap(d);
    }

    /// Whether a read stopped at the cap since the last call; resets it.
    pub fn take_read_cap_hit(&mut self) -> bool {
        self.wire.take_read_cap_hit()
    }

    /// Run `n` cycles and collect what the calculator transmitted; returns
    /// whether it transmitted anything.
    pub fn run(&mut self, n: u64) -> Result<bool> {
        Ok(self.wire.run(&mut self.session.machine, n)?)
    }

    /// Drop everything the calculator sent so far, collected or not (the
    /// idle server's periodic NAKs while no exchange ran).
    pub fn discard_input(&mut self) {
        self.wire.discard(&mut self.session.machine);
    }

    /// Run until the calculator has sent a Kermit NAK, at most `cap` of
    /// emulated time: proof that the server runs. The collected bytes are
    /// dropped.
    pub fn wait_for_nak(&mut self, cap: Duration) -> Result<bool> {
        Ok(self.wire.wait_for_nak(&mut self.session.machine, cap)?)
    }

    /// Queue `packet` for the calculator, after the wire's
    /// [`TURNAROUND`](wire::TURNAROUND) when it opens a transaction.
    pub fn write_packet(&mut self, packet: &[u8]) -> Result<()> {
        Ok(self.wire.write_packet(&mut self.session.machine, packet)?)
    }

    /// What the calculator sends until its output goes quiet, or what came
    /// in once the clock reaches `deadline`. Fails past the read cap.
    pub fn read(&mut self, deadline: Instant) -> Result<Vec<u8>> {
        Ok(self.wire.read(&mut self.session.machine, deadline)?)
    }
}
