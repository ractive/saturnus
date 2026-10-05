//! Memory modules that sit behind the chip select lines: ROM, RAM and
//! the NCE1 device.
//!
//! Modules only store nibbles; which module answers an address is decided by
//! [`crate::bus::MemoryController`].

pub mod flash;
pub mod nce1;
pub mod ram;
pub mod rom;

pub use flash::Flash;
pub use nce1::Nce1;
pub use ram::Ram;
pub use rom::Rom;
