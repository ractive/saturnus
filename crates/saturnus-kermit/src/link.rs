//! The serial link between the Kermit client and the emulated machine, in
//! process and single-threaded, on emulated time only: the products' wire,
//! [`saturnus_host::wire::Wire`], blocked on.
//!
//! The machine runs only while the link waits for it; nothing advances it
//! in the background. The Kermit client's clock is the link's
//! [`Link::now`]: a fixed start plus the emulated time the calculator spent
//! idle while a reply was awaited, so the same exchange always leaves the
//! same machine state and no wall-clock time is involved. The machine
//! runs through [`Session::run`], so the session's limits and trace hold. The rules (the
//! quiet line, busy time, the turnaround before a packet that opens a
//! transaction, the read cap) are the wire's; see [`saturnus_host::wire`].

use std::time::Duration;

use anyhow::Result;
use kermit_proto::time::Instant;
use saturnus_core::Machine;
use saturnus_drive::session::Session;
use saturnus_host::wire::{self, Line, Wire};

/// The session as the wire's [`Line`]: its runs go through
/// [`Session::run`], so the session's limits (abort, wall-clock deadline)
/// and trace hold on the wire too, and its error (a halt with the trace)
/// is kept to be returned as it is.
struct Port<'a> {
    session: &'a mut Session,
    error: Option<anyhow::Error>,
}

impl Port<'_> {
    /// The wire's result, with the session's own error in place of the
    /// wire's copy of it.
    fn result<T>(self, r: saturnus_host::Result<T>) -> Result<T> {
        match (r, self.error) {
            (Ok(v), _) => Ok(v),
            (Err(_), Some(e)) => Err(e),
            (Err(e), None) => Err(e.into()),
        }
    }
}

impl Line for Port<'_> {
    fn cycles(&self) -> u64 {
        self.session.machine.cycles()
    }
    fn clock_hz(&self) -> u32 {
        self.session.machine.model().clock_hz()
    }
    fn run_cycles(&mut self, n: u64) -> saturnus_host::Result<()> {
        self.session.run(n).map_err(|e| {
            let text = format!("{e:#}");
            self.error = Some(e);
            saturnus_host::Error::from(text)
        })
    }
    fn serial_drain(&mut self) -> Vec<u8> {
        self.session.machine.serial_drain()
    }
    fn serial_push(&mut self, bytes: &[u8]) {
        self.session.machine.serial_push(bytes);
    }
    fn serial_pending(&self) -> usize {
        Machine::serial_pending(&self.session.machine)
    }
    fn is_shutdown(&self) -> bool {
        self.session.machine.is_shutdown()
    }
}

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
        let mut port = Port {
            session: &mut self.session,
            error: None,
        };
        let r = self.wire.run(&mut port, n);
        port.result(r)
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
        let mut port = Port {
            session: &mut self.session,
            error: None,
        };
        let r = self.wire.wait_for_nak(&mut port, cap);
        port.result(r)
    }

    /// Queue `packet` for the calculator, after the wire's
    /// [`TURNAROUND`](wire::TURNAROUND) when it opens a transaction.
    pub fn write_packet(&mut self, packet: &[u8]) -> Result<()> {
        let mut port = Port {
            session: &mut self.session,
            error: None,
        };
        let r = self.wire.write_packet(&mut port, packet);
        port.result(r)
    }

    /// What the calculator sends until its output goes quiet, or what came
    /// in once the clock reaches `deadline`. Fails past the read cap.
    pub fn read(&mut self, deadline: Instant) -> Result<Vec<u8>> {
        let mut port = Port {
            session: &mut self.session,
            error: None,
        };
        let r = self.wire.read(&mut port, deadline);
        port.result(r)
    }
}
