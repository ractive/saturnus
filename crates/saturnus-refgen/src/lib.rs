//! `saturnus-refgen`: the command reference data, generated on the
//! emulator from the ROM itself.
//!
//! - [`names`]: the command names of the ROM's libraries.
//! - [`catalog`]: the names per model (`data/commands/<model>.json`).
//! - [`reference`]: our descriptions and the inputs to run.
//! - [`examples`]: those inputs run on each model.
//! - [`private_dir`]: where the generators keep machine states.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod catalog;
pub mod examples;
pub mod names;
pub mod private_dir;
pub mod reference;
