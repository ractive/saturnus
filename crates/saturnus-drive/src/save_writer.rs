//! The auto-saves of a native host, written off the machine thread
//! (decision log, 2026-10-31). The 49G's state is about 2.6 MB and
//! [`StateDir::save`](crate::files::StateDir) writes it whole and fsynced;
//! on the machine thread a key pressed meanwhile waited for the write.
//!
//! The machine thread hands each [`Saved`] to a [`SaveWriter`] and goes
//! on; the writer's own thread puts it into the [`StateStore`]. Only the
//! newest state of a model waits: a save that comes while another is
//! being written replaces the one queued for its model, so a busy disk
//! never piles states up. Saves land in the order they were given (one
//! that replaced a queued save takes its place at the end).
//!
//! Whatever reads or clears a slot (a boot, which restores the kept state
//! or, for a fresh start, deletes it) goes through
//! [`SaveWriter::with_store`], which first waits until everything queued
//! has landed: an older save is never written over the slot after it was
//! read or deleted. [`SaveWriter::flush`] waits the same way (the page
//! hidden, the window closing, the machine thread ending), and dropping the
//! writer writes what is queued and joins its thread.
//!
//! A wait never outlasts the writer's thread: a write or a report that
//! panics is caught (logged, and the thread goes on), the end of a write
//! wakes the waiters whatever happened in it, and should the thread end
//! anyway, whatever was still queued is logged as lost and every wait
//! returns.

use std::io::Write as _;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

use saturnus_host::protocol::Saved;

use crate::files::StateStore;

/// What becomes of each save, called on the writer's thread once it was
/// written (or failed) and before [`SaveWriter::flush`] returns for it:
/// the runner's `autoSaved` event, or the error on stderr.
pub type Report = Box<dyn Fn(&Saved, Result<(), String>) + Send + 'static>;

#[derive(Default)]
struct Queue {
    /// The newest unwritten state of each model, in the order given.
    queued: Vec<Saved>,
    /// A save was taken off `queued` and is being written.
    writing: bool,
    /// No more saves come: the thread writes what is queued and ends.
    closed: bool,
    /// The thread has ended: nothing more lands, nobody waits for it.
    gone: bool,
}

impl Queue {
    fn idle(&self) -> bool {
        self.gone || (self.queued.is_empty() && !self.writing)
    }
}

/// A line on stderr that cannot panic (`eprintln!` does when stderr is
/// closed): the writer's thread must outlive its own logging.
fn log(msg: std::fmt::Arguments<'_>) {
    let _ = writeln!(std::io::stderr(), "{msg}");
}

struct Shared {
    queue: Mutex<Queue>,
    /// A save was queued, a write ended, or the writer closed.
    changed: Condvar,
    store: Mutex<Box<dyn StateStore>>,
}

/// A lock that a panic elsewhere left poisoned is still the data: the
/// queue and the store hold nothing a half-done step could break.
fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Shared {
    fn wait<'a>(&self, q: MutexGuard<'a, Queue>) -> MutexGuard<'a, Queue> {
        self.changed
            .wait(q)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Wait until nothing is queued or being written (or the thread is
    /// gone); returns the queue still locked, so nothing new is taken
    /// meanwhile.
    fn idle(&self) -> MutexGuard<'_, Queue> {
        let mut q = lock(&self.queue);
        while !q.idle() {
            q = self.wait(q);
        }
        q
    }
}

/// The writer thread of the auto-saves (see the module's notes). Used from
/// the machine thread; the store is the writer's from here on, reached
/// through [`SaveWriter::with_store`].
pub struct SaveWriter {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SaveWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let q = lock(&self.shared.queue);
        f.debug_struct("SaveWriter")
            .field("queued", &q.queued.len())
            .field("writing", &q.writing)
            .finish_non_exhaustive()
    }
}

impl SaveWriter {
    /// Start the thread that writes into `store`; `report` hears of each
    /// save.
    pub fn spawn(store: Box<dyn StateStore>, report: Report) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            changed: Condvar::new(),
            store: Mutex::new(store),
        });
        let thread = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("saturnus-save".into())
                .spawn(move || work(&shared, &report))?
        };
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Queue `saved` and return at once. A save of the same model that is
    /// queued, not yet being written, is replaced: only the newest lands.
    pub fn save(&self, saved: Saved) {
        let mut q = lock(&self.shared.queue);
        if q.gone {
            log(format_args!(
                "saturnus: the {} state was not kept: the state writer has stopped",
                saved.model
            ));
            return;
        }
        q.queued.retain(|s| s.model != saved.model);
        q.queued.push(saved);
        self.shared.changed.notify_all();
    }

    /// Wait until every save given so far has landed (or failed, and was
    /// reported).
    pub fn flush(&self) {
        drop(self.shared.idle());
    }

    /// Run `f` on the store once every save given so far has landed, with
    /// no write beside it: how a boot reads or clears a slot.
    pub fn with_store<R>(&self, f: impl FnOnce(&dyn StateStore) -> R) -> R {
        let _queue = self.shared.idle();
        let store = lock(&self.shared.store);
        f(store.as_ref())
    }
}

impl Drop for SaveWriter {
    /// The last saves still land: the thread writes what is queued, then
    /// ends, and is joined.
    fn drop(&mut self) {
        lock(&self.shared.queue).closed = true;
        self.shared.changed.notify_all();
        if let Some(t) = self.thread.take()
            && t.join().is_err()
        {
            log(format_args!(
                "saturnus: the state writer stopped with a panic"
            ));
        }
    }
}

/// Ends a write whatever happened in it: `writing` is cleared and the
/// waiters are woken, also when the write or its report panicked.
struct Writing<'a>(&'a Shared);

impl Drop for Writing<'_> {
    fn drop(&mut self) {
        lock(&self.0.queue).writing = false;
        self.0.changed.notify_all();
    }
}

/// Marks the thread gone when [`work`] ends, by returning or by a panic
/// nothing caught: what is still queued is logged as lost, and the
/// waiters return instead of waiting for a thread that is not there.
struct Gone<'a>(&'a Shared);

impl Drop for Gone<'_> {
    fn drop(&mut self) {
        let mut q = lock(&self.0.queue);
        q.gone = true;
        for s in q.queued.drain(..) {
            log(format_args!(
                "saturnus: the {} state was not kept: the state writer has stopped",
                s.model
            ));
        }
        self.0.changed.notify_all();
    }
}

/// The writer's thread: take the oldest queued save, write it, report it,
/// until closed with nothing queued.
fn work(shared: &Shared, report: &Report) {
    let _gone = Gone(shared);
    let mut q = lock(&shared.queue);
    loop {
        if !q.queued.is_empty() {
            let saved = q.queued.remove(0);
            q.writing = true;
            drop(q);
            {
                let _writing = Writing(shared);
                // A store that panics is a failed write, reported as one,
                // and a report that panics is logged; the writer goes on.
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    lock(&shared.store).save(saved.model, &saved.state)
                }))
                .unwrap_or_else(|_| Err("the store panicked".to_string()));
                if std::panic::catch_unwind(AssertUnwindSafe(|| report(&saved, result))).is_err() {
                    log(format_args!(
                        "saturnus: the report of the {} state's save panicked",
                        saved.model
                    ));
                }
            }
            q = lock(&shared.queue);
        } else if q.closed {
            return;
        } else {
            q = shared.wait(q);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError, channel};
    use std::time::Duration;

    /// Longest a gated write waits to be released: a test that forgets to
    /// release fails instead of hanging.
    const GATE_LIMIT: Duration = Duration::from_secs(30);

    /// A store whose writes wait at a gate: each `save` says it started
    /// (`entered`), then waits for one release, or for good once the
    /// test drops the gate's sender. `log` has every write as it landed,
    /// and every load and clear.
    pub(crate) struct Gated {
        pub log: Arc<Mutex<Vec<String>>>,
        slots: Mutex<HashMap<String, Vec<u8>>>,
        entered: Sender<String>,
        gate: Mutex<Receiver<()>>,
    }

    /// The test's ends of a [`Gated`] store.
    pub(crate) struct Gate {
        pub log: Arc<Mutex<Vec<String>>>,
        pub entered: Receiver<String>,
        pub release: Option<Sender<()>>,
    }

    impl Gate {
        /// The writes so far, as they landed.
        pub fn log(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }

        /// Wait until a write has started; its `model state`.
        pub fn started(&self) -> String {
            self.entered.recv_timeout(GATE_LIMIT).unwrap()
        }

        /// Let every write through from now on.
        pub fn open(&mut self) {
            self.release = None;
        }
    }

    pub(crate) fn gated() -> (Gated, Gate) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let (entered, started) = channel();
        let (release, gate) = channel();
        (
            Gated {
                log: Arc::clone(&log),
                slots: Mutex::default(),
                entered,
                gate: Mutex::new(gate),
            },
            Gate {
                log,
                entered: started,
                release: Some(release),
            },
        )
    }

    impl StateStore for Gated {
        fn load(&self, model: &str) -> Result<Option<Vec<u8>>, String> {
            self.log.lock().unwrap().push(format!("load {model}"));
            Ok(self.slots.lock().unwrap().get(model).cloned())
        }

        fn save(&self, model: &str, state: &[u8]) -> Result<(), String> {
            let state = String::from_utf8_lossy(state).into_owned();
            let _ = self.entered.send(format!("{model} {state}"));
            match self.gate.lock().unwrap().recv_timeout(GATE_LIMIT) {
                Ok(()) | Err(RecvTimeoutError::Disconnected) => {}
                Err(RecvTimeoutError::Timeout) => panic!("the gate was never opened"),
            }
            if state == "bad" {
                return Err("disk full".to_string());
            }
            self.log
                .lock()
                .unwrap()
                .push(format!("save {model} {state}"));
            self.slots
                .lock()
                .unwrap()
                .insert(model.to_string(), state.into_bytes());
            Ok(())
        }

        fn clear(&self, model: &str) -> Result<(), String> {
            self.log.lock().unwrap().push(format!("clear {model}"));
            self.slots.lock().unwrap().remove(model);
            Ok(())
        }
    }

    pub(crate) fn saved(model: &'static str, state: &str) -> Saved {
        Saved {
            model,
            rom_name: String::new(),
            cycles: 0,
            state: state.as_bytes().to_vec(),
        }
    }

    type Reports = Arc<Mutex<Vec<String>>>;

    fn writer(store: Gated) -> (SaveWriter, Reports) {
        let reports = Reports::default();
        let heard = Arc::clone(&reports);
        let w = SaveWriter::spawn(
            Box::new(store),
            Box::new(move |s, r| {
                let state = String::from_utf8_lossy(&s.state);
                heard.lock().unwrap().push(match r {
                    Ok(()) => format!("{} {state}", s.model),
                    Err(e) => format!("{} {state}: {e}", s.model),
                });
            }),
        )
        .unwrap();
        (w, reports)
    }

    /// Saves that come while one is being written do not pile up: only
    /// the newest of each model lands after it, in the order given.
    #[test]
    fn the_newest_save_wins() {
        let (store, mut gate) = gated();
        let (w, reports) = writer(store);
        w.save(saved("48sx", "1"));
        assert_eq!(gate.started(), "48sx 1");
        w.save(saved("48sx", "2"));
        w.save(saved("49g", "a"));
        w.save(saved("48sx", "3"));
        w.save(saved("48sx", "4"));
        gate.open();
        w.flush();
        assert_eq!(gate.log(), ["save 48sx 1", "save 49g a", "save 48sx 4"]);
        assert_eq!(*reports.lock().unwrap(), ["48sx 1", "49g a", "48sx 4"]);
    }

    /// A boot reads or clears the slot only once what is queued has
    /// landed: the old machine's save never comes back after a fresh
    /// start's delete, and a restore reads the newest state.
    #[test]
    fn a_boot_waits_for_the_queued_saves() {
        let (store, mut gate) = gated();
        let (w, _) = writer(store);
        w.save(saved("48sx", "old"));
        assert_eq!(gate.started(), "48sx old");
        w.save(saved("48sx", "older-queued"));
        w.save(saved("48sx", "newest"));
        std::thread::scope(|s| {
            let (done, booted) = channel();
            let w = &w;
            s.spawn(move || {
                let kept = w.with_store(|st| st.load("48sx"));
                done.send(kept).unwrap();
            });
            // The write is held at the gate: the load cannot have run.
            assert_eq!(booted.try_recv(), Err(TryRecvError::Empty));
            gate.open();
            let kept = booted.recv_timeout(GATE_LIMIT).unwrap().unwrap();
            assert_eq!(kept.as_deref(), Some(&b"newest"[..]));
        });
        assert_eq!(
            gate.log(),
            ["save 48sx old", "save 48sx newest", "load 48sx"]
        );
        // A fresh start: the delete comes after the queued save.
        let (store, mut gate) = gated();
        let (w, _) = writer(store);
        w.save(saved("48sx", "before"));
        assert_eq!(gate.started(), "48sx before");
        w.save(saved("48sx", "queued"));
        std::thread::scope(|s| {
            let fresh = s.spawn(|| w.with_store(|st| st.clear("48sx")));
            gate.open();
            fresh.join().unwrap().unwrap();
        });
        w.flush();
        assert_eq!(
            gate.log(),
            ["save 48sx before", "save 48sx queued", "clear 48sx"]
        );
        assert_eq!(w.with_store(|st| st.load("48sx")).unwrap(), None);
    }

    /// Dropping the writer (the machine thread ends, the app quits) still
    /// writes what is queued, then joins the thread.
    #[test]
    fn the_last_save_lands_on_shutdown() {
        let (store, mut gate) = gated();
        let (w, reports) = writer(store);
        w.save(saved("49g", "1"));
        assert_eq!(gate.started(), "49g 1");
        w.save(saved("49g", "2"));
        w.save(saved("49g", "last"));
        let (done, dropped) = channel();
        let quit = std::thread::spawn(move || {
            drop(w);
            done.send(()).unwrap();
        });
        assert_eq!(dropped.try_recv(), Err(TryRecvError::Empty), "it waits");
        gate.open();
        quit.join().unwrap();
        dropped.try_recv().unwrap();
        assert_eq!(gate.log(), ["save 49g 1", "save 49g last"]);
        assert_eq!(*reports.lock().unwrap(), ["49g 1", "49g last"]);
    }

    /// A report that panics (a sink that broke, stderr closed) does not
    /// stop the writer: the flush returns, and the next save lands and is
    /// reported.
    #[test]
    fn a_report_that_panics_does_not_stop_the_writer() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let (store, mut gate) = gated();
        gate.open();
        let reports = Reports::default();
        let heard = Arc::clone(&reports);
        let first = AtomicBool::new(true);
        let w = Arc::new(
            SaveWriter::spawn(
                Box::new(store),
                Box::new(move |s, _| {
                    assert!(!first.swap(false, Ordering::SeqCst), "the sink broke");
                    let state = String::from_utf8_lossy(&s.state);
                    heard.lock().unwrap().push(format!("{} {state}", s.model));
                }),
            )
            .unwrap(),
        );
        // A flush that hung would fail here, not hang the test run.
        let flushed = |w: &Arc<SaveWriter>| {
            let (done, flushed) = channel();
            let w = Arc::clone(w);
            std::thread::spawn(move || {
                w.flush();
                done.send(()).unwrap();
            });
            flushed.recv_timeout(GATE_LIMIT).unwrap();
        };
        w.save(saved("48sx", "1"));
        flushed(&w);
        w.save(saved("48sx", "2"));
        flushed(&w);
        assert_eq!(gate.log(), ["save 48sx 1", "save 48sx 2"]);
        assert_eq!(*reports.lock().unwrap(), ["48sx 2"]);
        let kept = w.with_store(|st| st.load("48sx")).unwrap();
        assert_eq!(kept.as_deref(), Some(&b"2"[..]));
    }

    /// A failed write is reported, and the next save is written.
    #[test]
    fn a_failed_write_is_reported() {
        let (store, mut gate) = gated();
        gate.open();
        let (w, reports) = writer(store);
        w.save(saved("48gx", "bad"));
        w.flush();
        w.save(saved("48gx", "good"));
        w.flush();
        assert_eq!(
            *reports.lock().unwrap(),
            ["48gx bad: disk full", "48gx good"]
        );
        assert_eq!(gate.log(), ["save 48gx good"]);
    }
}
