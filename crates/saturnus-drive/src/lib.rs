//! Host-side helpers that drive a [`saturnus::Machine`] in emulated time,
//! shared by the `saturnus` CLI and the Kermit test host: key scripts
//! ([`script`]), the scripted session with its idle wait ([`session`]),
//! the per-model boot and Kermit server start ([`autostart`]), ROM loading
//! ([`rom`]) and its download from hpcalc.org ([`fetch`]), screen dumps
//! ([`screen`]), the native hosts' files ([`files`]) and the machine thread that drives the front-end protocol's
//! state machine (`saturnus_host::protocol`) for the Tauri app and the
//! CLI's control API ([`runner`]).
//!
//! Unlike the core crate this one does file I/O, so it is not meant for
//! `wasm32`.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod autostart;
pub mod fetch;
pub mod files;
pub mod rom;
pub mod runner;
pub mod screen;
pub mod script;
pub mod session;

// The README (the crate's page on crates.io) compiles as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
