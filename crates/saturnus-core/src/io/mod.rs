//! Memory-mapped I/O of the HP48: the 64-nibble HDW register window, the two
//! hardware timers, the UART and the keyboard matrix; and the 42S's Lewis
//! display and register block.
//!
//! wiki: hardware/io-ram, wiki: hardware/timers, wiki: hardware/uart,
//! wiki: hardware/keyboard
//!
//! Public: only [`Key`], the keys hosts press through
//! [`Machine::key_down`](crate::Machine::key_down).

pub(crate) mod keyboard;
pub(crate) mod keyboard42;
pub(crate) mod keyboard49;
pub(crate) mod keyboard_aplet;
pub(crate) mod lewis;
pub(crate) mod registers;
pub(crate) mod timers;
pub(crate) mod uart;

pub use keyboard::Key;
pub(crate) use keyboard::{Keyboard, Layout};
pub(crate) use lewis::LewisIo;
pub(crate) use registers::IoRegisters;
pub(crate) use timers::Timers;
pub(crate) use uart::Uart;
