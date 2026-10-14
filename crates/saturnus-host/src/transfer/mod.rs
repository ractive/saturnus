//! The hidden Kermit transaction: what changes the calculator's user
//! memory from outside (`storeFile`, `fetchFile`, `purge`, `rename`,
//! `createDir`, `changeDir`, `setFlag`, `storeText`, `copy`, `move`,
//! `storeScreen`, `backup` in `web/protocol.md`)
//! goes through
//! the ROM's own Kermit server, so its memory manager stays consistent;
//! RAM is never written. `storeText` (the palette's editor) sends its text
//! as a string variable and has the calculator compile it with a host
//! command ([`Op::StoreText`]); `storeScreen` sends the screen as a GROB
//! under a temporary name and recalls it ([`Op::StoreScreen`]).
//!
//! A [`Transfer`] types `SERVER` on the command line (the typing engine,
//! [`crate::typing`]), waits for the idle server's first NAK, runs its
//! Kermit transactions (`kermit-proto`'s client over the emulated serial
//! port, on emulated time only), ends the server with `G F` and waits until
//! the calculator shows its stack again. It is stepped in emulated time
//! like a send, so a host runs it in turns at the speed the machine allows
//! (a calculator waiting for a packet sleeps, which costs nearly nothing),
//! with the screen held: what the server draws is never shown.
//!
//! The stack is left as it was: every transaction starts with an empty host
//! command, whose reply counts the levels, and a command that failed has
//! its leftover arguments dropped before the server ends. Flag -35 (the
//! transfer format) is set as each transfer needs and put back; the
//! current directory is put back after a store, fetch, purge or rename in
//! another directory. As on a real calculator, the server keeps its I/O
//! parameters in `IOPAR` in HOME, creating it on first use (purged, it
//! comes back with the server's end).
//!
//! The 49G in algebraic mode (flag -95 set) cannot enter the server as it
//! is: a server entered from it leaves the stack packed in a list. Its
//! writes clear -95 by keys first (`CF(-95)`, its echo dropped), run the
//! server in RPN mode and set -95 again by keys once the server has ended
//! (`-95 SF`), on every path: success, the calculator's refusal, a dead
//! server ended with ON, a stop. A flag alone is set or cleared by keys
//! (`SF(n)`, `CF(n)`) without the server. Wiki: protocols/kermit,
//! protocols/server-commands.

mod emulator;
mod exchange;
mod ops;
mod plan;
mod run;
#[cfg(test)]
mod tests;

use exchange::*;
use ops::*;
use run::*;

pub use ops::{
    BACKUP_MARGIN, BACKUP_NO_ROOM, BACKUP_VARIABLE, Op, REPLACE_VARIABLE, SCREEN_VARIABLE,
    TEXT_VARIABLE, Target, may_serve, store_bytes, trim_fetched,
};
pub use run::{STOPPED, Transfer};

use std::collections::VecDeque;

use kermit_proto::time::Duration;
use kermit_proto::{Client, Command, Config, Event, OutgoingFile};
use saturnus_core::io::Key;
use saturnus_core::{Machine, Model};
use saturnus_objects::transfer::{self as tfile, is_plain_name};
use saturnus_objects::{
    Flags, Graphic, ObjectType, UserMemory, Variable, charset, cmdline, prolog,
};

use crate::typing::{Job, Verb};
use crate::wire::{
    FINISH_SETTLE, IDLE_WAIT, MIN_WAIT, RETRIES, SERVER_CAP, TIMEOUT, Wire, WireStep, cycles,
};

/// Largest file `storeFile` takes (the 49G's RAM is 512 KiB, a 48's at
/// most 256 KiB with cards; a transfer of this size takes minutes of
/// emulated time at 9600 baud).
pub const MAX_FILE_BYTES: usize = 512 * 1024;
/// How long the screen must stay unchanged, with the CPU asleep, to count
/// as back at the stack.
pub(super) const STABLE_MS: u64 = 300;
/// Longest wait for that, after [`FINISH_SETTLE`].
pub(super) const IDLE_CAP_MS: u64 = 10_000;
/// How long ON is held to stop a server that no longer answers.
pub(super) const ON_HOLD_MS: u64 = 100;
/// Reply timeout of the I packet that asks, after ON, whether the server
/// still runs. A server in its command wait answers within milliseconds;
/// silence cannot be read from the line otherwise, since an idle server
/// NAKs only once and then stays silent while it runs (wiki:
/// protocols/server-commands).
pub(super) const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// Resends of that I packet (one: a packet the calculator lost while it
/// came back from ON gets a second chance).
pub(super) const PROBE_RETRIES: u32 = 1;
/// Longest emulated time the probe runs, whatever the link's clock says (a
/// calculator that stays awake does not count as idle on it): two sends of
/// [`PROBE_TIMEOUT`], the turnaround, and slack.
pub(super) const PROBE_CAP_MS: u64 = 15_000;
/// Reply timeout of the I packet that asks, with no ON before it, whether
/// a server runs ([`Op::Server`]): a server in its command wait answers
/// within milliseconds.
pub(super) const ASK_TIMEOUT: Duration = Duration::from_secs(2);
/// Longest emulated time that question runs: two sends of
/// [`ASK_TIMEOUT`], the turnaround, and slack.
pub(super) const ASK_CAP_MS: u64 = 6_000;
/// Rounds of ON and the probe at most when a server that answers the probe
/// then fails `G F` (ON landed mid-transaction, say, and the I packet drew
/// a NAK or a resent S).
pub(super) const END_ROUNDS: usize = 2;
/// Pause after a dropping key press (the ROM sees a release at its next
/// 1/16 s poll).
pub(super) const DROP_GAP_MS: u64 = 100;

/// A finished write's reply (`web/protocol.md`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferResult {
    /// Emulated ms it took.
    pub emulated_ms: f64,
    /// `storeFile`: the name the calculator stored the file under (it adds
    /// `.1` when the name is taken and flag -36 is clear); `fetchFile`: the
    /// variable fetched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `fetchFile`: the file's size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
    /// Done by keys rather than the Kermit server (the 49G in algebraic
    /// mode).
    pub keys: bool,
    /// [`Op::Server`]: whether the Kermit server ran (it answered the I
    /// packet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<bool>,
    /// `storeText`: why the calculator did not compile the text (its own
    /// message, as `Invalid Syntax`, or that the text is not one object);
    /// nothing was stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// `backup`: the bytes free before it (`MEM`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free: Option<f64>,
    /// `backup`: the bytes it needed free.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needed: Option<f64>,
}

/// One planned step.
#[derive(Clone, Debug)]
pub(super) enum Step {
    /// Type the text and ENTER (`SERVER`, or a key fallback).
    Type(String),
    /// [`Step::Type`] as cleanup, after an error or a stop too (what a
    /// stopped backup made, removed once its server has ended).
    TypeCleanup(String),
    /// Wait for the idle server's first NAK.
    WaitNak,
    /// A host command; `cleanup` steps run after an error too.
    Host { text: String, cleanup: bool },
    /// SEND a file.
    Send { name: String, data: Vec<u8> },
    /// GET a variable, in binary or (`text`) in ASCII mode.
    Get { name: String, text: bool },
    /// `MEM`: refused unless `needed` bytes are free ([`Op::Backup`], a
    /// store replacing a variable with a directory), with `refusal` and
    /// the numbers; the number it leaves is dropped.
    Free { needed: f64, refusal: String },
    /// Purge the backup's copy in port 0 (`:0:N`), but only once its
    /// `ARCHIVE` made it ([`Leftovers::archive`]): a name taken in port 0
    /// before is never this write's to purge. Runs after an error too.
    Unarchive,
    /// A host command that leaves one object more on the stack (the
    /// recalled screen): counted, so the 49G's return to algebraic mode
    /// keeps it.
    Push { text: String },
    /// A host command that compiles the text sent as a string: its reply
    /// must show two levels more than the baseline, the list the text was
    /// wrapped in and its size (at most 2), 1.
    Compile { text: String },
    /// End the server.
    Finish,
    /// Wait until the calculator shows its stack again.
    Settle,
    /// Drop (the backspace key) what the stack holds beyond this many
    /// levels: the echo algebraic mode leaves.
    DropTo(usize),
    /// Set -95 again by keys (`-95 SF`, typed in RPN mode) if it is clear:
    /// the 49G was in algebraic mode when the write began.
    Algebraic,
    /// After a stop: a line typed in part is cancelled with ON.
    CancelLine,
    /// [`Op::Server`]: an I packet, no ON before it; a server that answers
    /// it is ended (`G F`, the wait for the stack) when `end`.
    Ask { end: bool },
    /// End a server that no longer answers, or one a stop interrupted:
    /// ON, then an I packet; a server that answers it still runs and is
    /// ended with `G F` ([`Running::Probe`]). Should that `G F` fail, ON
    /// and the probe come again ([`END_ROUNDS`] in all), as the old loop
    /// pressed ON again while the server still NAKed; then the error says
    /// the server could not be ended.
    EndServer,
}

impl Step {
    /// Whether the step runs after an error.
    pub(super) fn cleanup(&self) -> bool {
        match self {
            Step::Host { cleanup, .. } => *cleanup,
            Step::Finish
            | Step::Settle
            | Step::DropTo(_)
            | Step::Algebraic
            | Step::CancelLine
            | Step::Unarchive
            | Step::TypeCleanup(_)
            | Step::EndServer => true,
            _ => false,
        }
    }

    /// Whether the step runs after the server, so it stays when a dead
    /// server is ended with ON.
    pub(super) fn after_server(&self) -> bool {
        matches!(self, Step::Algebraic | Step::DropTo(_))
    }
}
