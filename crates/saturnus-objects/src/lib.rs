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

pub mod charset;
pub mod decompile;
pub mod names;
pub mod object;
pub mod prolog;
pub mod ram;

pub use decompile::{NumberFormat, Settings, described, display, has_text, text};
pub use names::{CommandInfo, NameStats, NameTable, UnitMarkers};
pub use object::{
    ArrayItem, Base, Integer, MAX_DECODED_OBJECTS, Memory, NoMemory, Object, Reader, Real, decode,
    decode_at,
};
pub use prolog::ObjectType;
pub use ram::{
    Flags, Layout, UserMemory, Variable, change_counter, current_path, flags, memory_tree,
    stack_objects,
};
