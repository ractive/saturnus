#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! Core library of the saturnus emulator: Saturn CPU, bus, modules and
//! per-model machine wiring. No I/O and no threads: this crate must build
//! for `wasm32`.
//!
//! The public API is the [`Machine`] and what it takes and returns:
//! [`Model`], [`io::Key`], [`Port`], the display ([`Lcd`],
//! [`Framebuffer`], [`Annunciators`]), [`Error`] and [`Halt`], and a
//! disassembler for traces ([`disasm`]). The CPU, the bus and the I/O
//! chips are internal; the `internals` feature exposes them, without any
//! stability promise, for this crate's own ROM-gated tests and its `boot`
//! example.
//!
//! Hardware facts are cited as `wiki: <page>` referring to the calculator
//! wiki (see `kb/docs/knowledge-sources.md`).

mod bus;
mod cpu;
pub mod disasm;
mod error;
pub mod io;
mod machine;
mod modules;
mod state;

pub use cpu::ADDR_MASK;
pub use error::Error;
pub use machine::{
    Annunciators, CARD_MAX_BYTES, CARD_MIN_BYTES, Framebuffer, Halt, LCD_HEIGHT, LCD_HEIGHT_42S,
    LCD_WIDTH, Lcd, Machine, Model, NEW_CARD_BYTES, Port,
};

/// The CPU, the bus and the display renderer for this crate's ROM-gated
/// tests and the `boot` example (feature `internals`). Not part of the
/// public API: anything here may change in any release.
#[cfg(feature = "internals")]
#[doc(hidden)]
pub mod internals {
    pub use crate::bus::{Chip, Select};
    pub use crate::cpu::instr::{DatSize, Instruction, Ptr, Reg};
    pub use crate::cpu::{Decoded, decode, disassemble};
    use crate::machine::{Lcd, Machine};

    /// The display rendered from `m`'s display registers, reading the
    /// bitmaps through `peek`.
    pub fn render_lcd(m: &Machine, peek: impl Fn(u32) -> u8) -> Lcd {
        Lcd::render(&m.hw.io, peek)
    }

    /// The address ranges the display reads its rows from.
    pub fn lcd_row_spans(m: &Machine) -> impl Iterator<Item = (u32, u32)> {
        Lcd::row_spans(&m.hw.io)
    }
}

// The README (the crate's page on crates.io) compiles as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
