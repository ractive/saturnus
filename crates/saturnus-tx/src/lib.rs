//! The device layer of Files, the app's view for moving files, in the
//! browser and in the desktop app (`kb/docs/saturnus-tx.md`): what a user does with a calculator in
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
//!   holds ([`names::FileKind`]); `satx_core::names`, re-exported, so the
//!   satx CLI names files the same way.
//! - [`xmodem`]: the fast transfer of the 48GX and 49G (XModem, the user
//!   pressing EVAL on the calculator for `XRECV` or `XSEND`).
//! - [`xserv`]: a 49G running XSERV, the XModem server: the requests over
//!   it, and the Kermit server nested in it for what XSERV cannot do.
//! - [`palmtop`]: an HP 100LX or 200LX palmtop, its Filer in server mode
//!   (`filer-proto`): the drives, listings, get, put, rename, removal of
//!   a file or a directory tree, new directories, 8.3 names, Stop.
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
pub use satx_core::names;
mod ops;
pub mod palmtop;
mod search;
pub mod trace;
pub mod xmodem;
pub mod xserv;

pub use device::{
    Action, Backup, Connection, Device, DeviceOptions, Entry, Event, File, Format, LinkSettings,
    Memory, Outcome, Parity, Pict, Request, Tree, TreeItem, TxError,
};
pub use satx_core::{Model, Progress};
pub use search::{hear, probe};
pub use xserv::Server;
