//! HP calculator layer on top of `kermit-proto` and `xmodem-proto`:
//! transports, the Kermit and XModem driver loops, server commands, the HP
//! object format and GROBs.
//!
//! # Two faces, one protocol core
//!
//! The protocol logic (Kermit session, calculator operations, XModem) never
//! reads a clock, sleeps or touches a port. It runs behind either face:
//!
//! - **Blocking**: [`Calculator`], [`Session`] and [`XmodemSession`] over a
//!   [`transport::Transport`] (serial port, TCP, the in-process saturnus
//!   emulator, memory), with the system clock. This is what the satx CLI
//!   uses.
//! - **Sans-I/O**: [`Machine`] is fed bytes and the caller's time and hands
//!   out bytes to write, progress events and the [`Reply`] of each
//!   [`Op`]. Nothing blocks, so it runs where bytes arrive by callback: a
//!   browser with Web Serial on `wasm32-unknown-unknown`, an event loop, a
//!   GUI thread. See [`machine`] for the calling convention and a host loop.
//!
//! # Features
//!
//! - `native` (default): serial ports (`serialport`) and TCP, opened by
//!   [`transport::open`].
//! - `saturnus` (opt-in; satx turns it on): the saturnus emulator
//!   in-process (`saturnus://[MODEL@]ROM` addresses), through
//!   `saturnus-core` and `saturnus-drive`.
//!
//! Without either the crate does no OS I/O and builds for
//! `wasm32-unknown-unknown`:
//! `satx-core = { version = "0.2", default-features = false }`. There the
//! blocking face ([`Calculator`], [`Session`], [`XmodemSession`],
//! `transport::MemoryTransport`, `transport::drain`) is left out: it
//! sleeps and blocks, which that target cannot.
//!
//! The clock type is [`time::Instant`]: `std::time::Instant` on native
//! targets, `web_time::Instant` on wasm32.
//!
//! # Modules
//!
//! - [`machine`]: the sans-I/O face ([`Machine`], [`Op`], [`Reply`],
//!   [`Progress`]).
//! - [`transport`]: blocking links ([`transport::open`]).
//! - [`session`]: drives a [`kermit_proto::Client`] ([`Session`]).
//! - [`calc`]: what a user does with a calculator: list, get, put, run host
//!   commands, PICT, backup and restore ([`Calculator`]).
//! - [`reply`]: parsers for the server's text replies (`G D` listings and the
//!   stack text returned for a `C` host command).
//! - [`object`]: `HPHP48-x` / `HPHP49-x` binary files, prologs, object
//!   length walk, `%%HP:` ASCII headers.
//! - [`names`]: variable names to file names and back (`NAME.hp`,
//!   `NAME.txt`, `%XX` for what a file system refuses), and what a file
//!   holds ([`names::FileKind`]).
//! - [`grob`]: GROB decoding and PNG export.
//! - [`charset`]: the HP character set and its ASCII trigraphs.
//! - [`xmodem`]: XModem transfers with `XRECV`/`XSEND` started on the
//!   calculator ([`XmodemSession`]).
//! - [`xserv`]: XSERV (49g+/50g) packet framing; unverified on hardware.
//!
//! Wiki references (calculator-knowledgebase,
//! <https://github.com/ractive/calculator-knowledgebase>): `protocols/server-commands`,
//! `protocols/hp-object-format`, `protocols/iopar`, `hardware/uart`,
//! `protocols/xmodem-hp`, `protocols/xserv`.

pub mod calc;
pub mod charset;
mod error;
pub mod grob;
mod link;
pub mod machine;
pub mod names;
pub mod object;
pub mod reply;
pub mod session;
pub mod transport;
pub mod xmodem;
pub mod xserv;

/// The clock types of the whole crate: those of
/// [`kermit_proto::time`] (`std::time` on native targets, `web_time` on
/// wasm32, where `std::time::Instant::now()` panics).
pub mod time {
    pub use kermit_proto::time::{Duration, Instant};
}

// The blocking face waits by sleeping or by blocking reads, which
// wasm32-unknown-unknown cannot do (`std::thread::sleep` panics there): it
// is left out of that build, where `Machine` is the face.
#[cfg(not(target_arch = "wasm32"))]
pub use calc::Calculator;
pub use calc::{Model, TransferMode};
pub use error::{Error, Result, SerialError};
pub use machine::{Machine, Op, Progress, Reply};
pub use session::Options;
#[cfg(not(target_arch = "wasm32"))]
pub use session::Session;
pub use xmodem::XmodemOptions;
#[cfg(not(target_arch = "wasm32"))]
pub use xmodem::XmodemSession;
pub use xmodem_proto;
