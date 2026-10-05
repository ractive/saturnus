//! `saturnus-mcp`: a Model Context Protocol server that owns one emulated
//! calculator and lets an agent press keys, type, look at the screen, read
//! the stack and move objects over Kermit (through `hptx-core`).
//!
//! - [`server`]: the tools and their argument schemas.
//! - [`emulator`]: the calculator session behind them.
//! - [`link`]: the in-process serial link `hptx-core` talks Kermit over.
//! - [`keys`]: key scripts and the `type_text` character map.
//! - [`memory`]: the HOME tree, stack and flags read straight from RAM.
//! - [`object`]: typed objects, decoded from and encoded to HP binary files.
//! - [`semantic`]: `eval`, the typed stack and variables over the server.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod emulator;
pub mod keys;
pub mod link;
pub mod memory;
pub mod object;
pub mod semantic;
pub mod server;
