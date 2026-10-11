//! The device layer of saturnus tx, the transfer page and desktop app
//! (`kb/docs/saturnus-tx.md`): what a user does with a calculator in
//! Kermit server mode, as whole operations over `satx-core`'s sans-I/O
//! [`Machine`](satx_core::Machine).
//!
//! - [`Device`]: one calculator on one link. Fed bytes, the time and link
//!   errors; hands out [`Action`]s (open the port with these settings,
//!   write these bytes), [`Event`]s and the [`Outcome`] of each
//!   [`Request`]: connect (the speed and parity search, the model,
//!   flag -35 kept and restored), list, get, put, disconnect (FINISH);
//!   make, rename and purge (a directory with its count), a directory tree
//!   got and put, the memory check and backup, restore, the graphics
//!   screen.
//! - [`aplets`]: the 38G, 39G and 40G, which drive every transfer
//!   themselves: a folder served as their "disk drive" (its directory
//!   files made from the folder), and binary aplets sent by XModem.
//! - [`copy`]: copying between two devices (a get, then a put), what is in
//!   use on the target, and the warning for objects of the other family
//!   (a 48 object going to a 49G or back).
//! - [`names`]: variable names to file names and back (`NAME.hp`,
//!   `NAME.txt`, `%XX` for what a file system refuses), and what a file
//!   holds ([`names::FileKind`]).
//! - [`trace`]: recorded serial traffic with its timing, and its replay
//!   against a [`Device`] on a fake clock (the unit tests; a fake link for
//!   the page).
//!
//! No I/O, no threads, no clock: the crate builds for
//! `wasm32-unknown-unknown`. The clock is the caller's
//! [`satx_core::time::Instant`].
//!
//! Wiki references (calculator-knowledgebase): `protocols/server-commands`
//! (the idle server's NAKs), `protocols/iopar` (speeds and parity), `protocols/aplet-transfer`,
//! `hardware/uart` (the frame), `protocols/kermit-hp`.

pub mod aplets;
pub mod copy;
pub mod device;
pub mod names;
mod ops;
mod search;
pub mod trace;

pub use device::{
    Action, Backup, Connection, Device, DeviceOptions, Entry, Event, File, Format, LinkSettings,
    Memory, Outcome, Parity, Pict, Request, Tree, TreeItem, TxError,
};
pub use satx_core::{Model, Progress};
pub use search::{hear, probe};
