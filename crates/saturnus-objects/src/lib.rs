#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! HP 48/49 RPL objects for saturnus hosts (the MCP server, the web page,
//! a desktop shell), without any transfer or I/O stack: builds for
//! `wasm32` like the core.
//!
//! - [`object`]: the typed object model ([`Object`], JSON through serde)
//!   and the exact decoder of the calculator's nibble format.
//! - [`ram`]: a read-only view of a paused machine's user memory: the
//!   HOME tree with each variable's type, size and checksum, the current
//!   directory, the data stack and the flags, straight from RAM (no Kermit
//!   server, no mode switch), plus a change counter for cheap polling.
//! - [`names`]: command names read at run time from the loaded ROM's own
//!   library tables ([`NameTable`], built once per ROM).
//! - [`decompile`]: the calculator's text for programs, algebraics, units
//!   and data objects in the display mode the flags select.
//! - [`prolog`]: object types by prolog and the size walk.
//! - [`cmdline`]: the command line being edited (text, cursor, open or
//!   not) and the editor's entry mode, alpha and shifts, from RAM.

pub mod charset;
pub mod cmdline;
pub mod decompile;
pub mod menus;
pub mod names;
pub mod object;
pub mod prolog;
pub mod ram;

pub use cmdline::{CommandLine, Editor, EditorLayout, command_line};
pub use decompile::{NumberFormat, Settings, described, display, has_text, text};
pub use names::{CommandInfo, NameStats, NameTable, UnitMarkers};
pub use object::{
    ArrayItem, Base, Integer, MAX_CLONED_NIBBLES, MAX_DECODED_NIBBLES, MAX_DECODED_OBJECTS, Memory,
    NoMemory, Object, Reader, Real, decode, decode_at,
};
pub use prolog::ObjectType;
pub use ram::{
    Flags, Layout, UserMemory, Variable, change_counter, current_path, flags, memory_tree,
    stack_objects,
};

// The README (the crate's page on crates.io) compiles as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
