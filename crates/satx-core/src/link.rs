//! The seam between the protocol logic and whoever owns the I/O.
//!
//! The logic above `kermit-proto` and `xmodem-proto` (the Kermit session,
//! the calculator operations, the XModem session) is `async` code that only
//! talks to a [`Link`]: a mailbox of bytes fed in, packets queued out,
//! progress events and the caller's clock. It never reads a clock, sleeps or
//! touches a port. A read waits until bytes are fed or a deadline of the
//! fed clock passes; a pause waits for the clock alone.
//!
//! No executor and no waker are involved: the owner of the I/O polls the
//! future again after every feed (bytes, time or a link error), with a no-op
//! waker. Two owners exist:
//!
//! - [`drive`]: the blocking loop over a [`Transport`] and
//!   `Instant::now()` behind [`Session`](crate::Session),
//!   [`Calculator`](crate::Calculator) and
//!   [`XmodemSession`](crate::XmodemSession).
//! - [`Machine`](crate::machine::Machine): the sans-I/O face for a host
//!   whose bytes arrive by callback (a browser with Web Serial).

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

use crate::time::{Duration, Instant};
use crate::transport::Transport;

/// A progress event of the running operation, in the order the protocol
/// machines emitted them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// From the Kermit client (file start, data, progress, server text,
    /// `Done`, `Error`).
    Kermit(kermit_proto::Event),
    /// From the XModem machine.
    Xmodem(xmodem_proto::Event),
}

/// Input kept while nothing reads it (between operations of a
/// [`Machine`](crate::machine::Machine)); the oldest bytes go first. The idle
/// server sends one 6-byte NAK per server timeout and then stays silent
/// (saturnus and saturnng; real hardware unverified; wiki:
/// protocols/server-commands), so this is far more than idling leaves.
const MAX_IDLE_INPUT: usize = 64 * 1024;

#[derive(Debug)]
struct State {
    /// The caller's clock, as last fed.
    now: Instant,
    /// Bytes fed and not read yet.
    input: Vec<u8>,
    /// Packets to write, each with one write.
    output: VecDeque<Vec<u8>>,
    /// Progress events not taken yet.
    events: VecDeque<Progress>,
    /// Whether events are kept (a caller that never takes them would keep
    /// every file twice).
    keep_events: bool,
    /// When the pending wait ends without input; `None` while nothing waits
    /// on the clock.
    wake: Option<Instant>,
    /// The link failed: every later read or write of the operation fails
    /// the same way.
    error: Option<(io::ErrorKind, String)>,
    /// Whether [`Link::flush`] waits until the queued packets are written
    /// ([`drive`], whose writes fail or succeed at once); a
    /// [`Machine`](crate::machine::Machine)'s host writes later and reports
    /// a failure as a link error.
    write_through: bool,
}

/// The mailbox shared by the protocol logic and the I/O owner.
#[derive(Clone, Debug)]
pub(crate) struct Link(Arc<Mutex<State>>);

impl Link {
    pub(crate) fn new(now: Instant) -> Self {
        Link(Arc::new(Mutex::new(State {
            now,
            input: Vec::new(),
            output: VecDeque::new(),
            events: VecDeque::new(),
            keep_events: true,
            wake: None,
            error: None,
            write_through: false,
        })))
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // Nothing panics while holding the lock; a poisoned lock still holds
        // consistent data.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // ---- the protocol side ----

    /// The caller's clock as last fed.
    pub(crate) fn now(&self) -> Instant {
        self.state().now
    }

    /// Queue one packet for a single write. Fails once the link failed.
    pub(crate) fn write(&self, packet: Vec<u8>) -> io::Result<()> {
        let mut st = self.state();
        if let Some((kind, msg)) = &st.error {
            return Err(io::Error::new(*kind, msg.clone()));
        }
        st.output.push_back(packet);
        Ok(())
    }

    /// Wait until the queued packets are written (only under [`drive`]),
    /// then fail if the link has failed: the write of a transaction's last
    /// packet fails the transaction, as a direct write would.
    pub(crate) fn flush(&self) -> Flush<'_> {
        Flush { link: self }
    }

    /// Hand a progress event to the I/O owner.
    pub(crate) fn emit(&self, event: Progress) {
        let mut st = self.state();
        if st.keep_events {
            st.events.push_back(event);
        }
    }

    /// Wait for input until `deadline` (`None`: no deadline). Resolves to
    /// every byte fed so far, or to an empty buffer once the deadline has
    /// passed without input, or to the link's error.
    pub(crate) fn read(&self, deadline: Option<Instant>) -> Read<'_> {
        Read {
            link: self,
            deadline,
        }
    }

    /// Wait until the clock reaches `until`. Input keeps arriving meanwhile
    /// and is read later.
    pub(crate) fn sleep_until(&self, until: Instant) -> Sleep<'_> {
        Sleep { link: self, until }
    }

    /// Read and discard input for `period` from now: the idle server's NAK
    /// (one per server timeout) sits in the buffers until read. Returns the
    /// number of bytes discarded.
    pub(crate) async fn drain(&self, period: Duration) -> io::Result<usize> {
        let end = self.now() + period;
        let mut discarded = 0;
        while self.now() < end {
            discarded += self.read(Some(end)).await?.len();
        }
        Ok(discarded)
    }

    // ---- the I/O owner's side ----

    pub(crate) fn set_now(&self, now: Instant) {
        let mut st = self.state();
        // A clock never runs backwards for the protocol machines.
        if now > st.now {
            st.now = now;
        }
    }

    /// Feed bytes. Nothing is cut: during an operation every byte counts.
    pub(crate) fn push_input(&self, bytes: &[u8]) {
        self.state().input.extend_from_slice(bytes);
    }

    /// Keep at most the newest [`MAX_IDLE_INPUT`] bytes of unread input,
    /// while no operation reads it.
    pub(crate) fn cap_idle_input(&self) {
        let mut st = self.state();
        let excess = st.input.len().saturating_sub(MAX_IDLE_INPUT);
        if excess > 0 {
            st.input.drain(..excess);
        }
    }

    pub(crate) fn fail(&self, error: &io::Error) {
        let mut st = self.state();
        if st.error.is_none() {
            st.error = Some((error.kind(), error.to_string()));
        }
    }

    /// Forget the link's error: the next operation tries the link again.
    pub(crate) fn clear_error(&self) {
        self.state().error = None;
    }

    /// Bytes fed and not read yet.
    #[cfg(test)]
    pub(crate) fn unread_input(&self) -> usize {
        self.state().input.len()
    }

    pub(crate) fn take_output(&self) -> Option<Vec<u8>> {
        self.state().output.pop_front()
    }

    pub(crate) fn take_event(&self) -> Option<Progress> {
        self.state().events.pop_front()
    }

    pub(crate) fn set_keep_events(&self, keep: bool) {
        let mut st = self.state();
        st.keep_events = keep;
        if !keep {
            st.events.clear();
        }
    }

    /// When the pending wait ends without input (`None`: it waits for input
    /// only, or nothing waits).
    pub(crate) fn wake(&self) -> Option<Instant> {
        self.state().wake
    }

    /// Forget everything of an aborted operation: queued packets, events
    /// not taken, unread input (a partial packet would garble the next
    /// operation's first read) and the wait.
    pub(crate) fn reset(&self) {
        let mut st = self.state();
        st.output.clear();
        st.events.clear();
        st.input.clear();
        st.wake = None;
    }

    /// Poll `future` once with a no-op waker. The wake-up deadline is
    /// cleared first; a pending read or sleep sets it again.
    pub(crate) fn poll<F: Future + ?Sized>(&self, future: Pin<&mut F>) -> Poll<F::Output> {
        self.state().wake = None;
        let mut cx = Context::from_waker(Waker::noop());
        future.poll(&mut cx)
    }
}

/// See [`Link::read`].
pub(crate) struct Read<'a> {
    link: &'a Link,
    deadline: Option<Instant>,
}

impl Future for Read<'_> {
    type Output = io::Result<Vec<u8>>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut st = self.link.state();
        if !st.input.is_empty() {
            return Poll::Ready(Ok(std::mem::take(&mut st.input)));
        }
        if let Some((kind, msg)) = &st.error {
            return Poll::Ready(Err(io::Error::new(*kind, msg.clone())));
        }
        if self.deadline.is_some_and(|d| st.now >= d) {
            return Poll::Ready(Ok(Vec::new()));
        }
        st.wake = self.deadline;
        Poll::Pending
    }
}

/// See [`Link::flush`].
pub(crate) struct Flush<'a> {
    link: &'a Link,
}

impl Future for Flush<'_> {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let st = self.link.state();
        if st.write_through && !st.output.is_empty() {
            return Poll::Pending;
        }
        match &st.error {
            Some((kind, msg)) => Poll::Ready(Err(io::Error::new(*kind, msg.clone()))),
            None => Poll::Ready(Ok(())),
        }
    }
}

/// See [`Link::sleep_until`].
pub(crate) struct Sleep<'a> {
    link: &'a Link,
    until: Instant,
}

impl Future for Sleep<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let mut st = self.link.state();
        if st.now >= self.until {
            return Poll::Ready(());
        }
        st.wake = Some(self.until);
        Poll::Pending
    }
}

/// Read wait of [`drive`] while the protocol waits for input without a
/// deadline.
const IDLE_WAIT: Duration = Duration::from_millis(100);
/// Shortest timeout handed to [`Transport::read`].
const MIN_WAIT: Duration = Duration::from_millis(1);

/// Run `future` to completion over `transport` with the system clock: write
/// what it queues, read until its wake-up deadline and feed what arrives.
/// `progress` sees every event. A failing write or read is fed to the link
/// as its error, which the protocol logic sees on its next read, write or
/// flush; the next `drive` tries the transport again. After writing, the
/// future is polled again before anything is read, so a flush returns at
/// once. While the link has failed and the future still waits (a pause),
/// the wait is slept instead of read, as the pause was before the seam.
/// Packets still queued when the future completes are written; a failure
/// then is ignored (the result is decided).
pub(crate) fn drive<F: Future>(
    transport: &mut dyn Transport,
    link: &Link,
    future: F,
    progress: &mut dyn FnMut(&Progress),
) -> F::Output {
    link.clear_error();
    link.state().write_through = true;
    let mut future = std::pin::pin!(future);
    let mut buf = [0u8; 2048];
    loop {
        link.set_now(Instant::now());
        let polled = link.poll(future.as_mut());
        let mut wrote = false;
        while let Some(packet) = link.take_output() {
            wrote = true;
            if let Err(e) = transport.write_packet(&packet) {
                link.fail(&e);
            }
        }
        while let Some(event) = link.take_event() {
            progress(&event);
        }
        if let Poll::Ready(output) = polled {
            return output;
        }
        if wrote {
            continue;
        }
        let wait = link
            .wake()
            .map_or(IDLE_WAIT, |w| w.saturating_duration_since(Instant::now()));
        if link.state().error.is_some() {
            std::thread::sleep(wait);
            continue;
        }
        match transport.read(&mut buf, wait.max(MIN_WAIT)) {
            Ok(n) => link.push_input(buf.get(..n).unwrap_or_default()),
            Err(e) => link.fail(&e),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn read_waits_for_input_or_the_deadline() {
        let t0 = Instant::now();
        let link = Link::new(t0);
        let mut read = std::pin::pin!(link.read(Some(t0 + Duration::from_millis(10))));
        assert!(link.poll(read.as_mut()).is_pending());
        assert_eq!(link.wake(), Some(t0 + Duration::from_millis(10)));
        link.push_input(b"ab");
        assert!(matches!(link.poll(read.as_mut()), Poll::Ready(Ok(v)) if v == b"ab"));
        let mut read = std::pin::pin!(link.read(Some(t0 + Duration::from_millis(10))));
        link.set_now(t0 + Duration::from_millis(10));
        assert!(matches!(link.poll(read.as_mut()), Poll::Ready(Ok(v)) if v.is_empty()));
    }

    #[test]
    fn a_failed_link_fails_reads_after_the_input_and_writes() {
        let link = Link::new(Instant::now());
        link.push_input(b"x");
        link.fail(&io::Error::from(io::ErrorKind::UnexpectedEof));
        let mut read = std::pin::pin!(link.read(None));
        assert!(matches!(link.poll(read.as_mut()), Poll::Ready(Ok(v)) if v == b"x"));
        let mut read = std::pin::pin!(link.read(None));
        assert!(matches!(
            link.poll(read.as_mut()),
            Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::UnexpectedEof
        ));
        assert!(link.write(b"p".to_vec()).is_err());
    }

    #[test]
    fn the_clock_never_runs_backwards_and_idle_input_is_capped() {
        let t0 = Instant::now();
        let link = Link::new(t0 + Duration::from_secs(1));
        link.set_now(t0);
        assert_eq!(link.now(), t0 + Duration::from_secs(1));
        link.push_input(&vec![0; MAX_IDLE_INPUT]);
        link.push_input(b"new");
        link.cap_idle_input();
        let mut read = std::pin::pin!(link.read(None));
        let Poll::Ready(Ok(v)) = link.poll(read.as_mut()) else {
            panic!("no input");
        };
        assert_eq!(v.len(), MAX_IDLE_INPUT);
        assert!(v.ends_with(b"new"));
    }
}
