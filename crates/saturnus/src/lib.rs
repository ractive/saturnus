#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! Core library of the saturnus emulator: Saturn CPU, bus, modules and
//! per-model machine wiring. No I/O and no threads: this crate must build
//! for `wasm32`.
//!
//! Hardware facts are cited as `wiki: <page>` referring to the calculator
//! wiki (see `kb/docs/knowledge-sources.md`).

pub mod cpu;
