//! `saturnus-kermit`: a Kermit host for tests and generators. It owns one
//! emulated calculator, drives its serial port in process on emulated time
//! only (the same exchange always leaves the same machine state) and talks
//! to the ROM's Kermit server through `kermit-proto`; the objects on the
//! wire come from `saturnus-objects`' `transfer` module.
//!
//! - [`emulator`]: the calculator: boot, key scripts, server start and stop.
//! - [`link`]: the serial link and the client's emulated clock.
//! - [`kermit`]: host commands, listings, GET, SEND and `G F`.
//! - [`reply`]: the server's stack and listing text parsed.
//! - [`semantic`]: `eval`, the typed stack and variables.
//!
//! Not published: the ROM-gated Kermit tests and `saturnus-refgen` use it.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod emulator;
pub mod kermit;
pub mod link;
pub mod reply;
pub mod semantic;

pub use emulator::{Emulator, KeyReport, has_server};
pub use kermit::TransferMode;
pub use reply::StackReply;
