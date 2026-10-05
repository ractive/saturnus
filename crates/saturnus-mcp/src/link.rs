//! The serial link between `hptx-core`'s Kermit client and the owned
//! machine, in-process and single-threaded.
//!
//! The machine only runs while a tool works on it; nothing advances it in
//! the background. Time model, after hptx's own in-process transport
//! (`hptx-core` `transport/saturnus.rs`, not used here because its
//! `saturnus` feature pins an older saturnus):
//!
//! - [`Transport::read`] runs the machine in 1 ms emulated steps until it
//!   has transmitted something and the line has gone quiet for
//!   [`QUIET`], or until `timeout` worth of emulated time has passed. On a
//!   timeout without data it then sleeps out the rest of `timeout` in
//!   wall-clock time, so the client's wall-clock deadlines see the idle
//!   period the calculator saw instead of a burst of emulated timeouts.
//! - While nothing has come back yet and the CPU is busy (not in SHUTDN
//!   at the end of a step), that step does not count toward `timeout`:
//!   the calculator answers a host command only when it is done, and an
//!   idle server sits in SHUTDN between bytes (over 99% of 1 ms samples on
//!   the 48SX, 48GX and both 49G ROMs). A long computation then returns
//!   its reply instead of timing out the client after a few retries and
//!   leaving the reply for the next command to read. At most [`MAX_BUSY`]
//!   of emulated time per read; the session's wall-clock limits still
//!   apply.
//! - [`Transport::write_packet`] first runs the machine for the wall time
//!   the client spent outside the transport, at most [`MAX_CATCH_UP`]:
//!   the client's turnaround pause between transactions is time the
//!   calculator needs before the next command (calculator quirk: a command
//!   right after the final ACK is lost). Then it queues the packet, which
//!   the emulated UART receives at line rate.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use hptx_core::transport::Transport;
use saturnus_drive::session::Session;

/// Longest wall-clock gap between transport calls replayed as emulated
/// time before a write.
pub const MAX_CATCH_UP: Duration = Duration::from_secs(2);
/// Longest emulated time one read keeps running a busy calculator beyond
/// its timeout (as the key tools' cap: 10 minutes).
pub const MAX_BUSY: Duration = Duration::from_secs(600);
/// Emulated time run per step while reading.
const STEP: Duration = Duration::from_millis(1);
/// Emulated quiet time after the last transmitted byte that ends a read: a
/// byte takes about 1 ms at 9600 baud and the calculator sends a packet
/// back to back.
const QUIET: Duration = Duration::from_millis(4);

/// The emulated calculator plus the bytes it sent that nobody read yet.
#[derive(Debug)]
pub struct Core {
    /// The machine and its scripted-session bookkeeping.
    pub session: Session,
    /// Transmitted bytes not handed to the Kermit client yet.
    rx: VecDeque<u8>,
}

/// The core, shared between the tools (keys, screen) and the transport.
/// The server's session lock already serialises every tool, so this lock
/// is never contended; it only lets both own the machine.
pub type SharedCore = Arc<Mutex<Core>>;

impl Core {
    /// Wrap `session`.
    pub fn new(session: Session) -> SharedCore {
        Arc::new(Mutex::new(Core {
            session,
            rx: VecDeque::new(),
        }))
    }

    /// Emulated time `d` in CPU cycles.
    pub fn cycles_in(&self, d: Duration) -> u64 {
        let hz = u128::from(self.session.machine.model().clock_hz());
        u64::try_from(d.as_nanos() * hz / 1_000_000_000).unwrap_or(u64::MAX)
    }

    /// Run `n` cycles and collect what the calculator transmitted; returns
    /// whether it transmitted anything.
    pub fn run(&mut self, n: u64) -> io::Result<bool> {
        self.session
            .run(n)
            .map_err(|e| io::Error::other(format!("{e:#}")))?;
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
    pub fn wait_for_nak(&mut self, cap: Duration) -> io::Result<bool> {
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

    fn take(&mut self, buf: &mut [u8]) -> usize {
        let n = buf.len().min(self.rx.len());
        for (dst, src) in buf.iter_mut().zip(self.rx.drain(..n)) {
            *dst = src;
        }
        n
    }
}

/// Lock the core for the transport.
fn lock(core: &SharedCore) -> io::Result<MutexGuard<'_, Core>> {
    core.lock()
        .map_err(|_| io::Error::other("emulator state poisoned by an earlier panic"))
}

/// `hptx-core`'s view of the owned machine's serial port.
#[derive(Debug)]
pub struct MachineTransport {
    core: SharedCore,
    /// When the client last returned from a transport call.
    last_call: Instant,
}

impl MachineTransport {
    /// A transport on `core`.
    pub fn new(core: SharedCore) -> Self {
        Self {
            core,
            last_call: Instant::now(),
        }
    }
}

impl Transport for MachineTransport {
    fn write_packet(&mut self, packet: &[u8]) -> io::Result<()> {
        let mut core = lock(&self.core)?;
        let gap = core.cycles_in(self.last_call.elapsed().min(MAX_CATCH_UP));
        core.run(gap)?;
        core.session.machine.serial_push(packet);
        drop(core);
        self.last_call = Instant::now();
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let start = Instant::now();
        let mut core = lock(&self.core)?;
        let step = core.cycles_in(STEP);
        let quiet = core.cycles_in(QUIET);
        let begin = core.session.machine.cycles();
        let mut end = begin.saturating_add(core.cycles_in(timeout));
        let busy_end = end.saturating_add(core.cycles_in(MAX_BUSY));
        let mut last_byte = begin;
        while core.rx.len() < buf.len() && core.session.machine.cycles() < end {
            // Input still on its way in means more is coming back;
            // otherwise stop once the output has gone quiet.
            if !core.rx.is_empty()
                && core.session.machine.cycles() - last_byte >= quiet
                && core.session.machine.serial_pending() == 0
            {
                break;
            }
            if core.run(step)? {
                last_byte = core.session.machine.cycles();
            } else if core.rx.is_empty() && !core.session.machine.is_shutdown() {
                // Still computing the reply: this step is not idle time.
                end = end.saturating_add(step).min(busy_end);
            }
        }
        let n = core.take(buf);
        drop(core);
        if n == 0 {
            std::thread::sleep(timeout.saturating_sub(start.elapsed()));
        }
        self.last_call = Instant::now();
        Ok(n)
    }
}
