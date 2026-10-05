//! `saturnus-refgen`: the command reference data, generated on the
//! emulator from the ROM itself.
//!
//! - [`names`]: the command names of the ROM's libraries.
//! - [`menus`]: the built-in menus, crawled on the keyboard.
//! - [`catalog`]: both together, per model (`data/commands/<model>.json`).
//! - [`reference`]: our descriptions and the inputs to run.
//! - [`examples`]: those inputs run on each model.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod catalog;
pub mod examples;
pub mod menus;
pub mod names;
pub mod reference;
