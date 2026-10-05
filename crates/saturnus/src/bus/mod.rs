//! System bus: address decoding between the Saturn CPU and the memory
//! modules.
//!
//! The [`MemoryController`] models the per-chip controllers of the Clarke
//! (HP48 SX) and Yorke (HP48 GX, HP49G) chips: it decides which chip select line
//! answers a given address. Memory contents live in [`crate::modules`].

pub mod controller;

pub use controller::{Chip, MemoryController, Select};
