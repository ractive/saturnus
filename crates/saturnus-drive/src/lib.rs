//! Host-side helpers that drive a [`saturnus::Machine`] in emulated time,
//! shared by the `saturnus` CLI and the `saturnus-mcp` server: key scripts
//! ([`script`]), the scripted session with its idle wait ([`session`]),
//! the per-model boot and Kermit server start ([`autostart`]), ROM loading
//! ([`rom`]), screen dumps ([`screen`]) and wall-clock pacing ([`pacer`],
//! also used by the Tauri app).
//!
//! Unlike the core crate this one does file I/O, so it is not meant for
//! `wasm32`.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod autostart;
pub mod pacer;
pub mod rom;
pub mod screen;
pub mod script;
pub mod session;
