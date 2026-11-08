//! Sans-IO Filer protocol of the HP 100LX and 200LX palmtops: the link
//! the built-in Filer speaks in server mode, over the serial cable.
//!
//! The crate never touches a port, a clock or a file. The caller owns all
//! three: it reads and writes the link, supplies the current [`Instant`]
//! ([`time::Instant`]) and keeps the files. The state machines are in
//! between, with the seam of `kermit-proto` and `xmodem-proto`.
//!
//! Every fact comes from the calculator wiki's pages
//! `protocols/filer-hplx` and `hardware/hp100lx-200lx`
//! (<https://github.com/ractive/calculator-knowledgebase>), checked
//! against BSD Filer v0.3 as a black box. HP never published the
//! protocol.
//!
//! # Layers
//!
//! - [`codec`]: the frame (three SYN, DLE STX, SOH, command, counter,
//!   direction, revision, data with DLE doubling, DLE ETX, CRC-16/ARC low
//!   byte first and never doubled) and a [`Deframer`] that resynchronises.
//! - [`Request`], [`Response`] and [`Code`]: the commands, the request data
//!   and the reply layouts; [`FsError`] and [`error::status`] the error
//!   codes with their meanings.
//! - [`Client`]: one request at a time with HP's timing (2 s for CONNECT,
//!   5.9 s for everything else, three sends of the same frame, CONNECT up
//!   to six times, the counter advanced only on a matching reply).
//! - [`Session`]: what a file manager does ([`Job`]: list, get, put,
//!   rename, delete, make and remove directories, the drives) as the
//!   request sequences HP's client sends.
//! - [`Server`] over a [`FileSystem`] ([`MemoryFs`] in memory): the
//!   palmtop's side, for tests, with the server's counter rules (a
//!   repeated counter gets the previous reply without running the command
//!   again).
//! - [`path`]: DOS paths as a user types them to the canonical bytes.
//!
//! # The seam
//!
//! - [`Session::start`] begins a job ([`Job::Connect`] first).
//! - [`Session::handle_input`] feeds bytes read from the link, in any
//!   chunking.
//! - [`Session::handle_timeout`] tells the machine time has passed; call it
//!   when [`Session::next_timeout`] is reached (it does nothing early).
//! - [`Session::poll_output`] hands out the next frame. Write it whole: the
//!   palmtop's 8250 UART has no flow control.
//! - [`Session::poll_event`] yields [`SessionEvent`]s: progress, and one
//!   `Finished` per job.
//! - [`Session::disconnect`] queues one DISCONNECT, not awaited.
//!
//! Every call that can queue output takes `now`, and the deadline of a
//! frame starts when [`Session::poll_output`] hands it out.
//!
//! # Time and WebAssembly
//!
//! The state machines never read a clock: every call that needs the time
//! takes `now` from the caller. The type is [`time::Instant`], which is
//! `std::time::Instant` on every target except `wasm32`, where it is
//! `web_time::Instant` from the [`web-time`](https://docs.rs/web-time)
//! crate (`std::time::Instant::now()` panics on `wasm32-unknown-unknown`).
//!
//! # Driver loop
//!
//! ```no_run
//! use std::io::{ErrorKind, Read, Write};
//! use std::net::TcpStream;
//! use std::time::{Duration, Instant};
//! use filer_proto::{Config, Job, Output, Session, SessionEvent};
//!
//! fn run(link: &mut TcpStream, session: &mut Session, job: Job) -> std::io::Result<Output> {
//!     session.start(Instant::now(), job).map_err(std::io::Error::other)?;
//!     let mut buf = [0u8; 4096];
//!     loop {
//!         let now = Instant::now();
//!         while let Some(frame) = session.poll_output(now) {
//!             link.write_all(&frame)?;
//!         }
//!         while let Some(event) = session.poll_event() {
//!             if let SessionEvent::Finished(result) = event {
//!                 return result.map_err(std::io::Error::other);
//!             }
//!         }
//!         let wait = session
//!             .next_timeout()
//!             .map_or(Duration::from_millis(100), |t| t.saturating_duration_since(now));
//!         link.set_read_timeout(Some(wait.max(Duration::from_millis(1))))?;
//!         match link.read(&mut buf) {
//!             Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
//!             Ok(n) => session.handle_input(Instant::now(), &buf[..n]),
//!             Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
//!                 session.handle_timeout(Instant::now())
//!             }
//!             Err(e) => return Err(e),
//!         }
//!     }
//! }
//!
//! fn main() -> std::io::Result<()> {
//!     let mut link = TcpStream::connect("localhost:4853")?;
//!     let mut session = Session::new(Config::default());
//!     run(&mut link, &mut session, Job::Connect)?;
//!     let listing = run(&mut link, &mut session, Job::List(b"C:\\*.*".to_vec()))?;
//!     println!("{listing:?}");
//!     session.disconnect(Instant::now());
//!     while let Some(frame) = session.poll_output(Instant::now()) {
//!         link.write_all(&frame)?;
//!     }
//!     Ok(())
//! }
//! ```
//!
//! [`Instant`]: time::Instant

pub mod client;
pub mod codec;
pub mod command;
pub mod error;
mod memfs;
pub mod path;
pub mod server;
pub mod session;
#[cfg(test)]
mod session_tests;

/// The clock types of the seam: `std::time` everywhere except `wasm32`,
/// where they come from `web-time` (see the crate docs, "Time and
/// WebAssembly").
pub mod time {
    #[cfg(not(target_arch = "wasm32"))]
    pub use std::time::{Duration, Instant};
    #[cfg(target_arch = "wasm32")]
    pub use web_time::{Duration, Instant};
}

pub use client::{Client, Config, Error, Event, StartError};
pub use codec::{Deframer, Frame};
pub use command::{Code, DirEntry, DosDateTime, NameKind, Request, Response, attr};
pub use error::{Failure, FsError};
pub use memfs::MemoryFs;
pub use server::{FileSystem, Server, ServerConfig};
pub use session::{DriveInfo, Job, JobError, Output, Session, SessionEvent};
