//! The emulated serial wire.

use std::collections::VecDeque;

use kermit_proto::time::{Duration, Instant};
use saturnus_core::Machine;

/// The serial link on emulated time: the client's clock is a fixed start
/// plus the emulated time the calculator idled while a reply was awaited,
/// so an exchange always leaves the same machine state.
#[derive(Debug)]
pub(crate) struct Wire {
    pub(crate) rx: VecDeque<u8>,
    pub(crate) start: Instant,
    pub(crate) idle: Duration,
}

impl Wire {
    pub(crate) fn now(&self) -> Instant {
        self.start + self.idle
    }

    /// Run `n` cycles; true if the calculator transmitted.
    pub(crate) fn run(&mut self, m: &mut Machine, n: u64) -> crate::Result<bool> {
        m.run_cycles(n.max(1))?;
        let out = m.serial_drain();
        self.rx.extend(&out);
        Ok(!out.is_empty())
    }

    /// Drop what the idle server sent (its NAK).
    pub(crate) fn discard(&mut self, m: &mut Machine) {
        self.rx.clear();
        m.serial_drain();
    }
}
