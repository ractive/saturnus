//! `saturnus-refgen`: the command reference data, generated on the
//! emulator from the ROM itself.
//!
//! - [`names`]: the command names of the ROM's libraries.
//! - [`catalog`]: the names per model (`crates/saturnus-cli/data/commands/<model>.json`).
//! - [`menus`]: each command's menus, from the ROM's menu definitions.
//! - [`reference`]: our descriptions and the inputs to run.
//! - [`examples`]: those inputs run on each model.
//! - [`keyed`]: the key-driven models' reference (`keys-<model>.json`),
//!   checked and its examples run on the emulator, reading the screen
//!   with [`lcdtext`] (the Pioneers, the 19BII) or [`apletext`] (HOME on
//!   the 38G, 39G and 40G).
//! - [`hp28`]: the 28S, driven by keys and read off its display.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod apletext;
pub mod catalog;
pub mod examples;
pub mod hp28;
pub mod keyed;
pub mod lcdtext;
pub mod menus;
pub mod names;
pub mod reference;
