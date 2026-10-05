//! Memory-mapped I/O of the HP48: the 64-nibble HDW register window, the two
//! hardware timers, the UART and the keyboard matrix.
//!
//! wiki: hardware/io-ram, wiki: hardware/timers, wiki: hardware/uart,
//! wiki: hardware/keyboard

pub mod keyboard;
pub mod keyboard49;
pub mod keyboard_aplet;
pub mod registers;
pub mod timers;
pub mod uart;

pub use keyboard::{Key, KeyPos, Keyboard, Layout};
pub use registers::IoRegisters;
pub use timers::Timers;
pub use uart::Uart;
