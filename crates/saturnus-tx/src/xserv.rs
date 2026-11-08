//! Files over XSERV, the XModem server of the 49G (`satx-core`'s
//! [`xserv`](satx_core::xserv)): the device's requests when the calculator
//! answered the connect's `M` probe.
//!
//! - **Listing, get, put, memory**: `L`, `G`, `P` and `M`. Every transfer is
//!   XModem (1k blocks, HP's CRC), binary: `G` sends the binary object
//!   whatever flag -35 says, and `P` of a file that is no binary object
//!   stores it as a string with the XModem padding in it (emulated 49G,
//!   2026-11-08), so text and other files go through the Kermit server
//!   (below).
//! - **Changes** (new directory, rename, purge) and directory changes are
//!   RPL run with `E`, which answers nothing: each change is checked in a
//!   fresh listing, which the XSERV client sends again until a calculator
//!   still running the RPL answers. An RPL error is not reported by XSERV
//!   (the calculator stays in XSERV), so the listing is the only proof.
//!   Each change runs as a string in `IFERR … STR→`, which drops what a
//!   failure leaves: else the `E` text stays on the stack (see
//!   `guarded`).
//! - **The current directory**: XSERV cannot say it, so the device keeps
//!   it and the directories it has seen in listings; it walks from HOME or
//!   from where it is, a listing per level it has not seen, and evaluates
//!   only names it has seen as directories (evaluating a variable would run
//!   it or push it).
//! - **A missing variable**: `G` of a name that is not there leaves the 49G
//!   in an error box ("Undefined Name") until a key is pressed, so a get
//!   lists first and refuses an unknown name.
//! - **The Kermit server, nested**: `SERVER` run through `E` starts it
//!   inside XSERV, and its FINISH returns to XSERV (real 49G, ROM 1.18, and
//!   the emulated one, ROM 2.15). The requests XSERV cannot do run there:
//!   text gets and puts, files stored as strings, PICT, backup and
//!   restore. Restore warm-starts the calculator, which ends
//!   both servers. Which server answers is kept (`Running`); while a
//!   nest starts or ends, and after a stop in a nested request, it is
//!   unknown, and the next request finds out first (`settle`).
//! - **An HP 48**: the XSrvr48 library answers as XSERV does; the connect
//!   tells it apart (`identify`) and refuses it.
//! - **Disconnect**: `Q` ends XSERV (ROM 2.15); a ROM that ignores it is
//!   told by `M` still answering.
//!
//! Wiki references (calculator-knowledgebase): `protocols/xserv`.

use std::collections::HashSet;

use satx_core::TransferMode;
use satx_core::calc::validate_name;
use satx_core::machine::{Op, Reply};
use satx_core::reply::{Entry, Listing};
use satx_core::time::Duration;
use satx_core::xserv::{DirRecord, transfer_options};

use crate::device::{Ctx, Request, Result, TxError, expect_done, unexpected};
use crate::names::FileKind;

/// How long the connect listens for XSERV's answer to `M` at each speed:
/// the emulated 49G answers within 0.15 s, the real one (ROM 1.18) at
/// once too; satx waits as long.
pub const PROBE_WAIT: Duration = Duration::from_millis(500);

/// How many times [`settle`] asks `M` before it takes the silence for the
/// Kermit server: XSERV drops an `M` that comes too soon after FINISH.
const SETTLE_PROBES: usize = 3;

/// How long the XModem of a `G` or `P` waits for its start: it starts at
/// once, as the command packet is ACKed.
const START_WAIT: Duration = Duration::from_secs(20);

/// The pause between `SERVER` run through `E` and the first Kermit packet:
/// `E` is ACKed before the RPL runs.
const KERMIT_START: Duration = Duration::from_secs(1);

/// The kind of server a connection talks to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Server {
    /// The Kermit server (`SERVER`).
    Kermit,
    /// XSERV, the XModem server of the 49G.
    Xserv,
}

impl Server {
    /// `kermit` or `xserv`.
    pub fn name(self) -> &'static str {
        match self {
            Server::Kermit => "kermit",
            Server::Xserv => "xserv",
        }
    }
}

/// Which server answers on an XSERV session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Running {
    /// XSERV.
    #[default]
    Xserv,
    /// The Kermit server nested in XSERV, in step, flag -35 set.
    Kermit,
    /// Not known: a nest was started or ended part way (an error, a
    /// stop). [`settle`] finds out before the next request.
    Unknown,
}

/// What an XSERV session keeps besides the current directory.
#[derive(Debug, Default)]
pub(crate) struct XservSession {
    /// The server that answers.
    pub(crate) running: Running,
    /// The transfer mode (flag -35) the first nested Kermit server found,
    /// put back before each FINISH; `None` while the flag was never read
    /// (and so never changed).
    pub(crate) user_mode: Option<TransferMode>,
    /// The directories seen in listings, as paths from HOME.
    pub(crate) dirs: HashSet<Vec<String>>,
}

impl XservSession {
    /// Forget `path` and everything below it.
    fn forget(&mut self, path: &[String]) {
        self.dirs.retain(|d| !d.starts_with(path));
    }

    /// `dir`'s listing: its directories are the ones below it now.
    fn learn(&mut self, dir: &[String], entries: &[Entry]) {
        let gone: Vec<Vec<String>> = self
            .dirs
            .iter()
            .filter(|d| d.len() == dir.len() + 1 && d.starts_with(dir))
            .filter(|d| {
                !entries
                    .iter()
                    .any(|e| e.is_directory() && d.last() == Some(&e.name))
            })
            .cloned()
            .collect();
        for d in gone {
            self.forget(&d);
        }
        for e in entries.iter().filter(|e| e.is_directory()) {
            self.dirs.insert(join(dir, &e.name));
        }
    }
}

fn join(dir: &[String], name: &str) -> Vec<String> {
    dir.iter().cloned().chain([name.to_string()]).collect()
}

/// Whether `request` needs the Kermit server on a calculator in XSERV.
pub(crate) fn needs_kermit(request: &Request) -> bool {
    let binary = |data: &[u8]| FileKind::of(data) == FileKind::Binary;
    match request {
        Request::Get { format, .. } | Request::GetTree { format, .. } => {
            *format == crate::Format::Text
        }
        Request::Put { data, .. } => !binary(data),
        Request::PutTree { items, .. } => items
            .iter()
            .any(|i| i.data.as_deref().is_some_and(|d| !binary(d))),
        Request::Backup | Request::Restore { .. } | Request::Pict => true,
        _ => false,
    }
}

/// The server that answers.
fn running(ctx: &Ctx) -> Result<Running> {
    ctx.xserv_session(|x| x.running)
}

fn set_running(ctx: &Ctx, running: Running) -> Result<()> {
    ctx.xserv_session(|x| x.running = running)
}

/// Start the Kermit server inside XSERV: `SERVER` through `E`, the sync,
/// binary transfers (flag -35 kept for [`leave_kermit`]). Unknown from
/// `SERVER` on until it is in step: an error or a stop part way leaves it
/// to [`settle`].
pub(crate) async fn enter_kermit(ctx: &Ctx) -> Result<()> {
    match running(ctx)? {
        Running::Kermit => return Ok(()),
        Running::Unknown => settle(ctx).await?,
        Running::Xserv => {}
    }
    set_running(ctx, Running::Unknown)?;
    execute(ctx, "SERVER".into()).await?;
    ctx.pause(KERMIT_START).await;
    expect_done(ctx.op(Op::Connect).await?)?;
    if ctx.xserv_session(|x| x.user_mode.is_none())? {
        let mode = match ctx.op(Op::TransferMode).await? {
            Reply::TransferMode(mode) => mode,
            other => return Err(unexpected("-35 FS?", &other)),
        };
        ctx.xserv_session(|x| x.user_mode = Some(mode))?;
    }
    expect_done(ctx.op(Op::SetTransferMode(TransferMode::Binary)).await?)?;
    set_running(ctx, Running::Kermit)
}

/// End the nested Kermit server (flag -35 back, FINISH): the calculator
/// is in XSERV again. A no-op when XSERV answers; an unknown server is
/// found out first ([`settle`]).
pub(crate) async fn leave_kermit(ctx: &Ctx) -> Result<()> {
    match running(ctx)? {
        Running::Xserv => Ok(()),
        Running::Unknown => settle(ctx).await,
        Running::Kermit => finish(ctx).await,
    }
}

/// Flag -35 back (when it was read), FINISH. Unknown until FINISH is
/// answered: a lost answer leaves it to [`settle`].
async fn finish(ctx: &Ctx) -> Result<()> {
    set_running(ctx, Running::Unknown)?;
    let restored = match ctx.xserv_session(|x| x.user_mode)? {
        Some(mode) => ctx
            .op(Op::SetTransferMode(mode))
            .await
            .and_then(expect_done),
        None => Ok(()),
    };
    expect_done(ctx.op(Op::Finish).await?)?;
    set_running(ctx, Running::Xserv)?;
    restored
}

/// Find out which server answers when that is not known: XSERV if it
/// answers `M` (asked [`SETTLE_PROBES`] times, as it answers about a
/// second after FINISH; the Kermit server ignores a lone `M`), else the
/// Kermit server, which is synced and ended with FINISH. The current
/// directory is forgotten.
pub(crate) async fn settle(ctx: &Ctx) -> Result<()> {
    ctx.session(|s| s.cwd = None)?;
    for _ in 0..SETTLE_PROBES {
        if let Reply::XservProbe(Some(_)) = ctx.op(Op::XservProbe(PROBE_WAIT)).await? {
            return set_running(ctx, Running::Xserv);
        }
    }
    expect_done(ctx.op(Op::Sync).await?)?;
    set_running(ctx, Running::Kermit)?;
    finish(ctx).await
}

/// The variable [`identify`] stores in HOME and purges again.
const ID_VAR: &str = "SATXID";

/// Whether the calculator in XSERV is a 49G (`false`: an HP 48 running
/// the XSrvr48 library, which answers alike). In HOME, `1. R→I` is stored
/// in a variable and listed: on the 49G it is an integer (prolog #2614),
/// on a 48, which has no `R→I`, the name `R→I` (the `1.` is dropped).
/// Without the variable in the listing (the RPL failed) it counts as a
/// 49G. The variable is purged; the current directory is HOME.
pub(crate) async fn identify(ctx: &Ctx) -> Result<bool> {
    let home = vec!["HOME".to_string()];
    ctx.session(|s| s.cwd = None)?;
    execute(
        ctx,
        format!("HOME DEPTH \\-> satxd \\<< 1. R\\->I '{ID_VAR}' STO DEPTH satxd - DROPN \\>>"),
    )
    .await?;
    let entries = list_here(ctx, &home).await?;
    let stored = entries.iter().find(|e| e.name == ID_VAR);
    if stored.is_some() {
        execute(ctx, guarded(&format!("'{ID_VAR}' PURGE"), &[ID_VAR])).await?;
    }
    ctx.session(|s| s.cwd = Some(home))?;
    Ok(stored.is_none_or(|e| e.kind == "Integer"))
}

/// Run `rpl` with `E`.
async fn execute(ctx: &Ctx, rpl: String) -> Result<()> {
    expect_done(ctx.op(Op::XservExecute(rpl)).await?)
}

/// The listing of the current directory, which is `here`.
async fn list_here(ctx: &Ctx, here: &[String]) -> Result<Vec<Entry>> {
    let records: Vec<DirRecord> = match ctx.op(Op::XservList).await? {
        Reply::XservListing(r) => r,
        other => return Err(unexpected("XSERV L", &other)),
    };
    let entries: Vec<Entry> = records.iter().map(DirRecord::entry).collect();
    ctx.xserv_session(|x| x.learn(here, &entries))?;
    Ok(entries)
}

/// Make `dir` (from HOME) the current directory.
pub(crate) async fn cd(ctx: &Ctx, dir: &[String]) -> Result<()> {
    let cwd = ctx.session(|s| s.cwd.clone())?;
    if cwd.as_deref() == Some(dir) {
        return Ok(());
    }
    let below = dir.get(1..).unwrap_or_default();
    for name in below {
        validate_name(name).map_err(|_| TxError::BadName(name.clone()))?;
    }
    // From where the calculator is when that is on the way, else HOME.
    let (mut at, mut pending) = match cwd {
        Some(c) if dir.starts_with(&c) => (c, String::new()),
        _ => (vec!["HOME".to_string()], "HOME".to_string()),
    };
    ctx.session(|s| s.cwd = None)?;
    while at.len() < dir.len() {
        let name = &dir[at.len()];
        let next = join(&at, name);
        if !ctx.xserv_session(|x| x.dirs.contains(&next))? {
            if !pending.is_empty() {
                execute(ctx, std::mem::take(&mut pending)).await?;
            }
            ctx.session(|s| s.cwd = Some(at.clone()))?;
            list_here(ctx, &at).await?;
            if !ctx.xserv_session(|x| x.dirs.contains(&next))? {
                return Err(TxError::NotFound(name.clone()));
            }
            ctx.session(|s| s.cwd = None)?;
        }
        if !pending.is_empty() {
            pending.push(' ');
        }
        pending.push_str(name);
        at = next;
    }
    if !pending.is_empty() {
        execute(ctx, pending).await?;
    }
    ctx.session(|s| s.cwd = Some(dir.to_vec()))
}

/// The listing of `dir`.
pub(crate) async fn list_dir(ctx: &Ctx, dir: &[String]) -> Result<Listing> {
    cd(ctx, dir).await?;
    let entries = list_here(ctx, dir).await?;
    Ok(Listing {
        path: Some(dir.to_vec()),
        free: None,
        entries,
    })
}

/// Get `name` of `dir` as a binary transfer file.
pub(crate) async fn get(ctx: &Ctx, dir: &[String], name: &str) -> Result<Vec<u8>> {
    let listing = list_dir(ctx, dir).await?;
    if !listing.entries.iter().any(|e| e.name == name) {
        return Err(TxError::NotFound(name.to_string()));
    }
    get_listed(ctx, name).await
}

/// Get `name`, which the caller has just seen in the listing of the
/// current directory, as a binary transfer file (no listing first).
pub(crate) async fn get_listed(ctx: &Ctx, name: &str) -> Result<Vec<u8>> {
    validate_name(name).map_err(|_| TxError::BadName(name.to_string()))?;
    let op = Op::XservGet {
        name: name.to_string(),
        options: transfer_options(START_WAIT),
    };
    match ctx.op(op).await? {
        Reply::XmodemReceived(got) => Ok(got.data),
        other => Err(unexpected("XSERV G", &other)),
    }
}

/// Put the binary transfer file `data` as `name` into the current
/// directory, which is `here`; the listing then says it is there.
pub(crate) async fn put(ctx: &Ctx, here: &[String], name: &str, data: Vec<u8>) -> Result<()> {
    let op = Op::XservPut {
        name: name.to_string(),
        data,
        options: transfer_options(START_WAIT),
    };
    match ctx.op(op).await? {
        Reply::XmodemSent(_) => {}
        other => return Err(unexpected("XSERV P", &other)),
    }
    let entries = list_here(ctx, here).await?;
    if entries.iter().any(|e| e.name == name) {
        Ok(())
    } else {
        Err(not_done(&format!("store {name}")))
    }
}

/// Why a change the listing does not show failed.
fn not_done(what: &str) -> TxError {
    TxError::Calc(satx_core::Error::Reply(format!(
        "the calculator did not {what}, and XSERV gives no reason: the name may be one of the calculator's commands, or the free memory too small"
    )))
}

/// Run `rpl` in `here` (the current directory) and check the listing with
/// `done`.
async fn change(
    ctx: &Ctx,
    here: &[String],
    rpl: String,
    what: &str,
    done: impl Fn(&[Entry]) -> bool,
) -> Result<()> {
    execute(ctx, rpl).await?;
    let entries = list_here(ctx, here).await?;
    if done(&entries) {
        Ok(())
    } else {
        Err(not_done(what))
    }
}

/// `rpl` (a change that leaves the stack as it found it) wrapped so that
/// an error leaves nothing behind either: XSERV keeps the text of an `E`
/// whose RPL fails on its stack, which the user finds there after XSERV
/// ends, and a failed `STO` puts its arguments back (emulated 49G, ROM
/// 2.15, 2026-11-08). The depth before is kept in a local variable named
/// unlike every name in `names`.
fn guarded(rpl: &str, names: &[&str]) -> String {
    let depth = ["satxd", "satxe", "satxf"]
        .into_iter()
        .find(|d| !names.contains(d))
        .unwrap_or("satxg");
    format!("DEPTH \\-> {depth} \\<< IFERR \"{rpl}\" STR\\-> THEN DEPTH {depth} - DROPN END \\>>")
}

fn quoted(name: &str) -> Result<String> {
    validate_name(name).map_err(|_| TxError::BadName(name.to_string()))?;
    Ok(format!("'{name}'"))
}

/// Make directory `name` in the current directory, `here`.
pub(crate) async fn mkdir(ctx: &Ctx, here: &[String], name: &str) -> Result<()> {
    let rpl = guarded(&format!("{} CRDIR", quoted(name)?), &[name]);
    ctx.xserv_session(|x| x.forget(&join(here, name)))?;
    change(ctx, here, rpl, &format!("make {name}"), |entries| {
        entries.iter().any(|e| e.name == name && e.is_directory())
    })
    .await
}

/// Purge `name` (a variable, or a directory with what is in it) from the
/// current directory, `here`.
pub(crate) async fn remove(ctx: &Ctx, here: &[String], name: &str, directory: bool) -> Result<()> {
    let purge = if directory { "PGDIR" } else { "PURGE" };
    let rpl = guarded(&format!("{} {purge}", quoted(name)?), &[name]);
    ctx.xserv_session(|x| x.forget(&join(here, name)))?;
    change(ctx, here, rpl, &format!("purge {name}"), |entries| {
        !entries.iter().any(|e| e.name == name)
    })
    .await
}

/// Rename `from` to `to` (not in use) in the current directory, `here`:
/// recalled, stored under the new name, purged, as satx-core's rename.
pub(crate) async fn rename(
    ctx: &Ctx,
    here: &[String],
    from: &str,
    to: &str,
    directory: bool,
) -> Result<()> {
    let purge = if directory { "PGDIR" } else { "PURGE" };
    let (f, t) = (quoted(from)?, quoted(to)?);
    let rpl = guarded(&format!("{f} RCL {t} STO {f} {purge}"), &[from, to]);
    ctx.xserv_session(|x| {
        x.forget(&join(here, from));
        x.forget(&join(here, to));
    })?;
    change(ctx, here, rpl, &format!("rename {from}"), |entries| {
        entries.iter().any(|e| e.name == to) && !entries.iter().any(|e| e.name == from)
    })
    .await
}

/// Replace `name` with the copy stored as `stored` in the current
/// directory, `here` (a put that replaces).
pub(crate) async fn swap_in(ctx: &Ctx, here: &[String], stored: &str, name: &str) -> Result<()> {
    let (s, n) = (quoted(stored)?, quoted(name)?);
    let rpl = guarded(&format!("{s} RCL {n} STO {s} PURGE"), &[stored, name]);
    change(ctx, here, rpl, &format!("replace {name}"), |entries| {
        entries.iter().any(|e| e.name == name) && !entries.iter().any(|e| e.name == stored)
    })
    .await
}

/// Purge `name` from `here`, best effort (a failed replace's copy).
pub(crate) async fn drop_copy(ctx: &Ctx, name: &str) {
    if let Ok(q) = quoted(name) {
        let _ = execute(ctx, guarded(&format!("{q} PURGE"), &[name])).await;
    }
}

/// The free memory in bytes.
pub(crate) async fn memory(ctx: &Ctx) -> Result<f64> {
    match ctx.op(Op::XservMemory).await? {
        Reply::Real(free) => Ok(free),
        other => Err(unexpected("XSERV M", &other)),
    }
}

/// End XSERV: whether it ended (else it still answers `M`).
pub(crate) async fn quit(ctx: &Ctx) -> Result<bool> {
    leave_kermit(ctx).await?;
    match ctx.op(Op::XservQuit).await? {
        Reply::XservQuit(ended) => Ok(ended),
        other => Err(unexpected("XSERV Q", &other)),
    }
}

/// [`Request::Sync`] on an XSERV session: whichever server answers
/// ([`settle`]), back to XSERV.
pub(crate) async fn resync(ctx: &Ctx) -> Result<()> {
    set_running(ctx, Running::Unknown)?;
    settle(ctx).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn path(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    fn dir(name: &str) -> Entry {
        Entry {
            name: name.into(),
            size: 10.0,
            kind: "Directory".into(),
            checksum: 0,
        }
    }

    #[test]
    fn a_listing_replaces_what_is_known_below_its_directory() {
        let mut x = XservSession::default();
        x.learn(&path(&["HOME"]), &[dir("A"), dir("B")]);
        x.learn(&path(&["HOME", "A"]), &[dir("C")]);
        assert!(x.dirs.contains(&path(&["HOME", "A", "C"])));
        // A is gone: so is everything below it; B stays.
        x.learn(&path(&["HOME"]), &[dir("B")]);
        assert_eq!(x.dirs, HashSet::from([path(&["HOME", "B"])]));
        x.forget(&path(&["HOME", "B"]));
        assert!(x.dirs.is_empty());
    }

    #[test]
    fn a_change_is_guarded_by_a_local_named_unlike_its_names() {
        assert_eq!(
            guarded("'A' PURGE", &["A"]),
            "DEPTH \\-> satxd \\<< IFERR \"'A' PURGE\" STR\\-> THEN DEPTH satxd - DROPN END \\>>"
        );
        assert!(guarded("x", &["satxd", "B"]).contains("\\-> satxe "));
    }

    #[test]
    fn what_needs_the_kermit_server() {
        let bin = b"HPHP49-C\x2c\x2a\x20\x00\x00".to_vec();
        let put = |data: Vec<u8>| Request::Put {
            dir: vec![],
            name: "X".into(),
            data,
            replace: false,
            as_string: true,
        };
        assert!(!needs_kermit(&put(bin.clone())));
        assert!(needs_kermit(&put(b"%%HP: T(3);\r\n1".to_vec())));
        assert!(needs_kermit(&put(b"hello".to_vec())));
        assert!(needs_kermit(&Request::Pict));
        assert!(needs_kermit(&Request::Backup));
        assert!(!needs_kermit(&Request::Memory));
        assert!(!needs_kermit(&Request::List { dir: vec![] }));
        let get = |format| Request::Get {
            dir: vec![],
            name: "X".into(),
            format,
        };
        assert!(needs_kermit(&get(crate::Format::Text)));
        assert!(!needs_kermit(&get(crate::Format::Binary)));
    }
}
