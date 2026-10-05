//! Memory-mapped I/O of the HP48: the 64-nibble HDW register window, the two
//! hardware timers, the UART and the keyboard matrix.
//!
//! wiki: hardware/io-ram, wiki: hardware/timers, wiki: hardware/uart,
//! wiki: hardware/keyboard

pub mod keyboard;
pub mod registers;
pub mod timers;
pub mod uart;

pub use keyboard::{Key, Keyboard};
pub use registers::IoRegisters;
pub use timers::Timers;
pub use uart::Uart;
