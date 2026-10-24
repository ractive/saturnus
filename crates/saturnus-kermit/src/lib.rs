//! `saturnus-kermit`: a Kermit host for tests and generators. It owns one
//! emulated calculator, drives its serial port in process on emulated time
//! only (the same exchange always leaves the same machine state) and talks
//! to the ROM's Kermit server through the Kermit client the products run
//! (`saturnus_host::transfer::Exchange`, the engine's hidden writes); the
//! objects on the wire come from `saturnus-objects`' `transfer` module.
//!
//! - [`emulator`]: the calculator: boot, key scripts, server start and stop.
//! - [`link`]: the serial link, the products' wire and Kermit client.
//! - [`kermit`]: the operations on the link: host commands, listings,
//!   the current directory, GET, SEND and `G F`.
//! - [`semantic`]: `eval`, the typed stack and variables.
//!
//! Not published: the ROM-gated Kermit tests and `saturnus-refgen` use it.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod emulator;
pub mod kermit;
pub mod link;
pub mod semantic;

pub use emulator::{Emulator, KeyReport, has_server};
pub use kermit::TransferMode;
pub use satx_core::reply::StackReply;
