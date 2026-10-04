//! Memory modules that sit behind the chip select lines: ROM and RAM.
//!
//! Modules only store nibbles; which module answers an address is decided by
//! [`crate::bus::MemoryController`].

pub mod ram;
pub mod rom;

pub use ram::Ram;
pub use rom::Rom;
