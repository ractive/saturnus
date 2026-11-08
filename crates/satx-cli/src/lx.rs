//! `satx lx`: the HP 100LX/200LX palmtops' Filer in server mode, thin over
//! [`satx_core::filer::Palmtop`] (wiki: protocols/filer-hplx).
//!
//! Paths are DOS paths; `/` works as well as `\` so that a shell needs no
//! quoting, the drive defaults to C:, and names are upper case on the
//! palmtop. Hints write paths with `/`.

use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use satx_core::filer::{self, DEFAULT_BAUD, Palmtop};
use satx_core::filer_proto::{DirEntry, DosDateTime, attr, path as dos, session::MAX_FILE};
use serde_json::{Value, json};

use crate::commands::{Ctx, refuse_existing_file, write_file};
use crate::error::Hinted;
use crate::output::{Hint, Outcome, shell_quote};
use crate::util;

/// The palmtop's settings, under every `lx` subcommand's help.
pub const LX_HELP: &str = "\
On the palmtop: start Filer, then MENU, Communications, Remote Settings:
Baud the same as --baud (19200 by default), Interface Com1, Server Mode
enabled, OK (F10). Leave Filer open and in front. The cable is HP's 10-pin
serial cable for the PC (F1015A), straight into a USB-serial adapter.

Paths are DOS paths: C:/_DAT/PHONE.PDB or 'C:\\_DAT\\PHONE.PDB'; without a
drive C:, without a path the root. Names are 8.3 and upper case on the
palmtop. There is no keep-alive: the palmtop may switch itself off while
idle.";

/// The examples of `satx lx --help`.
const LX_EXAMPLES: &str = "\
The HP 100LX/200LX Filer protocol, not Kermit: --timeout and --retries do
not apply (HP's timing: 2 s for the connect, 5.9 s for each reply, three
sends). Tested against BSD Filer v0.3, a Unix emulation of the Filer
server; not yet on a real palmtop. Reports are welcome.

Examples:
  satx lx drives                          # A: C: D: with free space
  satx lx ls                              # C:\\
  satx lx ls C:/_DAT                      # a directory
  satx lx ls 'C:\\_DAT\\*.PDB'              # a pattern
  satx lx get C:/_DAT/PHONE.PDB           # to ./PHONE.PDB, with its date
  satx lx put notes.txt C:/_DAT           # to C:\\_DAT\\NOTES.TXT
  satx lx put notes.txt --overwrite       # replace C:\\NOTES.TXT
  satx lx rm C:/OLD.TXT --dry-run
  satx lx mv C:/A.TXT C:/_DAT             # move into a directory
  satx lx --baud 38400 ls                 # faster, if the palmtop holds up
  satx lx --port tcp://localhost:4853 ls  # the BSD Filer oracle";

/// `satx lx --help`'s text after the options.
pub fn after_help() -> String {
    format!("{LX_HELP}\n\n{LX_EXAMPLES}")
}

/// The speeds the palmtop's Remote Settings offer.
const SPEEDS: [u32; 8] = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115_200];

fn parse_baud(s: &str) -> Result<u32, String> {
    let baud: u32 = s.parse().map_err(|_| format!("{s} is not a number"))?;
    if SPEEDS.contains(&baud) {
        Ok(baud)
    } else {
        Err(format!(
            "the palmtop offers {}",
            SPEEDS.map(|b| b.to_string()).join(", ")
        ))
    }
}

/// `satx lx` and its options.
#[derive(Args, Debug)]
pub struct LxArgs {
    /// Line speed, as set in the palmtop's Remote Settings. 19200 is the
    /// safe limit of its 8250 UART; 38400 and up may lose bytes.
    #[arg(long, default_value_t = DEFAULT_BAUD, value_parser = parse_baud, global = true,
          value_name = "BAUD", help_heading = "Connection")]
    pub baud: u32,
    /// What to do.
    #[command(subcommand)]
    pub command: LxCmd,
}

/// The `lx` subcommands.
#[derive(Subcommand, Debug)]
pub enum LxCmd {
    /// The drives, with label and free space.
    #[command(after_help = LX_HELP)]
    Drives,
    /// List a directory, a file or a DOS pattern (C:/*.TXT) [default: C:/].
    #[command(after_help = LX_HELP)]
    Ls {
        /// Directory, file or pattern.
        path: Option<String>,
    },
    /// Download a file; the local copy keeps the palmtop's date.
    #[command(after_help = LX_HELP)]
    Get {
        /// The file on the palmtop.
        remote: String,
        /// Output file [default: its name in the working directory]; - for stdout.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Replace an existing local file.
        #[arg(long)]
        force: bool,
    },
    /// Upload a file, with its date.
    #[command(after_help = LX_HELP)]
    Put {
        /// The local file.
        file: PathBuf,
        /// Where on the palmtop: a directory (C:/_DAT) or a full path [default: C:/NAME].
        remote: Option<String>,
        /// Replace a file of that name.
        #[arg(long)]
        overwrite: bool,
        /// Connect and say what would happen; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete files (not directories: rmdir).
    #[command(after_help = LX_HELP)]
    Rm {
        /// Files on the palmtop; nothing is deleted if one is missing.
        #[arg(required = true, value_name = "PATH")]
        paths: Vec<String>,
        /// Show what would be deleted, delete nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Make a directory.
    #[command(after_help = LX_HELP)]
    Mkdir {
        /// The new directory.
        path: String,
    },
    /// Remove an empty directory.
    #[command(after_help = LX_HELP)]
    Rmdir {
        /// The directory.
        path: String,
    },
    /// Rename or move a file or directory within one drive (TO must not exist).
    #[command(after_help = LX_HELP)]
    Mv {
        /// The current path.
        from: String,
        /// The new path, or an existing directory to move into.
        to: String,
    },
}

/// A user's path to the canonical bytes.
fn canonical(input: &str) -> Result<Vec<u8>> {
    dos::normalize(input, b'C').map_err(|e| {
        Hinted::new(
            format!("{input}: {e}"),
            "DOS paths: C:/_DAT/PHONE.PDB or 'C:\\_DAT\\PHONE.PDB'",
        )
        .into()
    })
}

/// A path for a command line: `/` for `\`, quoted when needed.
fn arg(path: &[u8]) -> String {
    shell_quote(&dos::display(path).replace('\\', "/"))
}

fn no_wildcard(path: &[u8], what: &str) -> Result<()> {
    if dos::has_wildcard(path) {
        return Err(Hinted::new(
            format!("{what} {}: no wildcards here", dos::display(path)),
            "`satx lx ls` takes a pattern (C:/*.TXT); name one file at a time",
        )
        .into());
    }
    Ok(())
}

/// Whether `name` is a valid DOS 8.3 name.
fn valid_83(name: &str) -> bool {
    let ok = |s: &str, max: usize| {
        (1..=max).contains(&s.len())
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'()-@^_`{}~".contains(&b))
    };
    match name.split_once('.') {
        Some((base, ext)) => ok(base, 8) && ok(ext, 3),
        None => ok(name, 8),
    }
}

/// `2004-12-14T13:49:36`, or null for a zero date.
fn time_json(t: DosDateTime) -> Value {
    if t.date == 0 {
        Value::Null
    } else {
        Value::from(t.to_string().replace(' ', "T"))
    }
}

/// The attribute letters: R(ead-only) H(idden) S(ystem) A(rchive).
fn attr_letters(a: u8) -> String {
    [
        (attr::READ_ONLY, 'R'),
        (attr::HIDDEN, 'H'),
        (attr::SYSTEM, 'S'),
        (attr::ARCHIVE, 'A'),
    ]
    .iter()
    .filter(|(bit, _)| a & bit != 0)
    .map(|(_, c)| *c)
    .collect()
}

/// A local file's time as a DOS time, in the computer's time zone (the
/// palmtop keeps local time).
fn dos_time(t: SystemTime) -> Option<DosDateTime> {
    let zoned = jiff::Timestamp::try_from(t)
        .ok()?
        .to_zoned(jiff::tz::TimeZone::system());
    DosDateTime::new(
        u16::try_from(zoned.year()).ok()?,
        u8::try_from(zoned.month()).ok()?,
        u8::try_from(zoned.day()).ok()?,
        u8::try_from(zoned.hour()).ok()?,
        u8::try_from(zoned.minute()).ok()?,
        u8::try_from(zoned.second()).ok()?,
    )
}

/// A DOS time as a local `SystemTime`; `None` for an impossible one.
fn system_time(t: DosDateTime) -> Option<SystemTime> {
    let date = jiff::civil::Date::new(
        i16::try_from(t.year()).ok()?,
        i8::try_from(t.month()).ok()?,
        i8::try_from(t.day()).ok()?,
    )
    .ok()?;
    let time = jiff::civil::Time::new(
        i8::try_from(t.hour()).ok()?,
        i8::try_from(t.minute()).ok()?,
        i8::try_from(t.second()).ok()?,
        0,
    )
    .ok()?;
    let zoned = date
        .to_datetime(time)
        .to_zoned(jiff::tz::TimeZone::system())
        .ok()?;
    Some(SystemTime::from(zoned.timestamp()))
}

/// The entry of exactly `path` (a listing of the name itself), if any.
fn entry(p: &mut Palmtop, path: &[u8]) -> Result<Option<DirEntry>> {
    if path.len() <= 3 {
        return Ok(None);
    }
    let name = dos::file_name(path);
    Ok(p.list(path)?
        .into_iter()
        .find(|e| e.name.eq_ignore_ascii_case(name)))
}

fn entry_json(dir: &[u8], e: &DirEntry) -> Value {
    json!({
        "name": e.name_lossy(),
        "path": dos::display(&dos::join(dir, &e.name)),
        "size": if e.is_directory() { 0 } else { e.size },
        "directory": e.is_directory(),
        "modified": time_json(e.modified),
        "attributes": attr_letters(e.attributes),
    })
}

impl Ctx {
    /// Open the link to the palmtop and connect.
    fn lx_open(&mut self, baud: u32) -> Result<Palmtop> {
        if self.global.dir.is_some() {
            return Err(Hinted::new(
                "--dir is for calculators",
                "give `satx lx` full DOS paths instead (C:/_DAT)",
            )
            .into());
        }
        let addr = crate::port::resolve(self.global.port.as_deref())?;
        self.link.addr = Some(addr.clone());
        Palmtop::open(&addr, baud)
            .with_context(|| format!("cannot connect to the palmtop on {addr}"))
    }

    pub(crate) fn lx(&mut self, args: LxArgs) -> Result<Option<Outcome>> {
        let mut p = self.lx_open(args.baud)?;
        let outcome = self.lx_on(&mut p, args.command);
        if outcome.is_ok() {
            p.close().context("disconnecting")?;
        }
        outcome
    }

    /// Run one subcommand on a connected palmtop.
    pub(crate) fn lx_on(&self, p: &mut Palmtop, command: LxCmd) -> Result<Option<Outcome>> {
        Ok(Some(match command {
            LxCmd::Drives => self.lx_drives(p)?,
            LxCmd::Ls { path } => self.lx_ls(p, path.as_deref().unwrap_or("C:/"))?,
            LxCmd::Get {
                remote,
                output,
                force,
            } => return self.lx_get(p, &remote, output.as_deref(), force),
            LxCmd::Put {
                file,
                remote,
                overwrite,
                dry_run,
            } => self.lx_put(p, &file, remote.as_deref(), overwrite, dry_run)?,
            LxCmd::Rm { paths, dry_run } => self.lx_rm(p, &paths, dry_run)?,
            LxCmd::Mkdir { path } => {
                let path = canonical(&path)?;
                no_wildcard(&path, "mkdir")?;
                if entry(p, &path)?.is_some() {
                    return Err(Hinted::new(
                        format!("{} exists", dos::display(&path)),
                        "choose another name",
                    )
                    .into());
                }
                p.make_dir(&path)
                    .with_context(|| format!("lx mkdir {}", dos::display(&path)))?;
                done(
                    format!("made {}", dos::display(&path)),
                    json!({"path": dos::display(&path)}),
                    vec![Hint::cmd(
                        "List it",
                        self.cmd(&format!("lx ls {}", arg(&path))),
                    )],
                )
            }
            LxCmd::Rmdir { path } => {
                let path = canonical(&path)?;
                no_wildcard(&path, "rmdir")?;
                p.remove_dir(&path)
                    .with_context(|| format!("lx rmdir {}", dos::display(&path)))?;
                done(
                    format!("removed {}", dos::display(&path)),
                    json!({"path": dos::display(&path)}),
                    Vec::new(),
                )
            }
            LxCmd::Mv { from, to } => self.lx_mv(p, &from, &to)?,
        }))
    }

    fn lx_drives(&self, p: &mut Palmtop) -> Result<Outcome> {
        let drives = p.drives(true).context("lx drives")?;
        let mut text = String::new();
        let mut results = Vec::new();
        for d in &drives {
            let letter = char::from(d.letter);
            match &d.volume {
                Some(Ok((label, free))) => {
                    let label = dos::display(label).trim_end().to_string();
                    let _ = writeln!(text, "{letter}:  {free:>11} bytes free  {label}");
                    results
                        .push(json!({"drive": letter.to_string(), "label": label, "free": free}));
                }
                Some(Err(e)) => {
                    let _ = writeln!(text, "{letter}:  {e}");
                    results.push(json!({"drive": letter.to_string(), "error": e.to_string()}));
                }
                None => {
                    let _ = writeln!(text, "{letter}:");
                    results.push(json!({"drive": letter.to_string()}));
                }
            }
        }
        let hints = drives
            .iter()
            .find(|d| d.letter == b'C')
            .or(drives.first())
            .map(|d| {
                let root = [d.letter, b':', b'\\'];
                vec![Hint::cmd(
                    format!("List {}:", char::from(d.letter)),
                    self.cmd(&format!("lx ls {}", arg(&root))),
                )]
            })
            .unwrap_or_default();
        Ok(Outcome {
            total: Some(drives.len() as u64),
            results: Value::from(results),
            dir: None,
            hints,
            text,
        })
    }

    fn lx_ls(&self, p: &mut Palmtop, input: &str) -> Result<Outcome> {
        let path = canonical(input)?;
        let label = format!("lx ls {}", dos::display(&path));
        let name = dos::file_name(&path).to_vec();
        // A pattern as it is; a directory's contents; else the one file.
        let (dir, pattern, only) = if dos::has_wildcard(&name) {
            let dir = dos::parent(&path).unwrap_or(&path).to_vec();
            (dir, path.clone(), None)
        } else if path.len() <= 3
            || p.kind(&path).context(label.clone())? == satx_core::filer_proto::NameKind::Directory
        {
            (path.clone(), dos::join(&path, b"*.*"), None)
        } else {
            let dir = dos::parent(&path).unwrap_or(&path).to_vec();
            (dir, path.clone(), Some(name))
        };
        let mut entries = p.list(&pattern).context(label.clone())?;
        if let Some(name) = &only {
            entries.retain(|e| e.name.eq_ignore_ascii_case(name));
            if entries.is_empty() {
                return Err(Hinted::new(
                    format!("{}: no such file or directory", dos::display(&path)),
                    format!(
                        "`{}` lists the directory",
                        self.cmd(&format!("lx ls {}", arg(&dir)))
                    ),
                )
                .into());
            }
        }
        let width = entries
            .iter()
            .map(|e| e.name.len())
            .max()
            .unwrap_or(0)
            .max(12);
        let mut text = format!(
            "{}: {} entr{}\n",
            dos::display(&pattern),
            entries.len(),
            if entries.len() == 1 { "y" } else { "ies" }
        );
        for e in &entries {
            let size = if e.is_directory() {
                "<DIR>".to_string()
            } else {
                e.size.to_string()
            };
            let when = if e.modified.date == 0 {
                String::new()
            } else {
                e.modified.to_string()[..16].to_string()
            };
            let line = format!(
                "  {:<width$} {:>9}  {}  {}",
                e.name_lossy(),
                size,
                when,
                attr_letters(e.attributes)
            );
            let _ = writeln!(text, "{}", line.trim_end());
        }
        let mut hints = Vec::new();
        if let Some(f) = entries.iter().find(|e| !e.is_directory()) {
            hints.push(Hint::cmd(
                format!("Download {}", f.name_lossy()),
                self.cmd(&format!("lx get {}", arg(&dos::join(&dir, &f.name)))),
            ));
        }
        if let Some(d) = entries.iter().find(|e| e.is_directory()) {
            hints.push(Hint::cmd(
                format!("List {}", d.name_lossy()),
                self.cmd(&format!("lx ls {}", arg(&dos::join(&dir, &d.name)))),
            ));
        }
        Ok(Outcome {
            total: Some(entries.len() as u64),
            results: Value::from(
                entries
                    .iter()
                    .map(|e| entry_json(&dir, e))
                    .collect::<Vec<_>>(),
            ),
            dir: Some(dos::display(&dir)),
            hints,
            text,
        })
    }

    fn lx_get(
        &self,
        p: &mut Palmtop,
        remote: &str,
        output: Option<&Path>,
        force: bool,
    ) -> Result<Option<Outcome>> {
        let path = canonical(remote)?;
        no_wildcard(&path, "get")?;
        let name = dos::display(dos::file_name(&path));
        let to_stdout = output == Some(Path::new("-"));
        let file = output.map_or_else(|| PathBuf::from(&name), Path::to_path_buf);
        if name.is_empty() {
            return Err(Hinted::new(
                format!("{}: not a file", dos::display(&path)),
                "name a file: satx lx get C:/_DAT/PHONE.PDB",
            )
            .into());
        }
        if !to_stdout {
            refuse_existing_file(&file, force)?;
        }
        if self.format == crate::output::Format::Text && std::io::stderr().is_terminal() {
            p.set_progress(Some(progress_printer()));
        }
        let got = p.get(&path);
        p.set_progress(None);
        let (data, modified) = got.with_context(|| format!("lx get {}", dos::display(&path)))?;
        if to_stdout {
            let mut out = std::io::stdout().lock();
            out.write_all(&data).context("writing to stdout")?;
            out.flush().context("writing to stdout")?;
            return Ok(None);
        }
        write_file(&file, &data, force)?;
        // The date as on the palmtop; a file whose date cannot be set keeps
        // the time of the copy.
        if let Some(when) = modified.and_then(system_time)
            && let Ok(f) = std::fs::OpenOptions::new().write(true).open(&file)
        {
            let _ = f.set_modified(when);
        }
        Ok(Some(Outcome {
            results: json!({
                "path": dos::display(&path),
                "file": file.display().to_string(),
                "bytes": data.len(),
                "modified": modified.map_or(Value::Null, time_json),
            }),
            total: None,
            dir: None,
            hints: Vec::new(),
            text: format!(
                "{} -> {} ({} bytes)",
                dos::display(&path),
                file.display(),
                data.len()
            ),
        }))
    }

    fn lx_put(
        &self,
        p: &mut Palmtop,
        file: &Path,
        remote: Option<&str>,
        overwrite: bool,
        dry_run: bool,
    ) -> Result<Outcome> {
        let local_name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default();
        let into = |dir: &[u8]| -> Result<Vec<u8>> {
            if !valid_83(&local_name) {
                return Err(Hinted::new(
                    format!("{local_name:?} is not a DOS 8.3 name"),
                    "name the file on the palmtop: satx lx put FILE C:/NAME.EXT",
                )
                .into());
            }
            Ok(dos::join(dir, local_name.as_bytes()))
        };
        let target = match remote {
            None => into(b"C:\\")?,
            Some(r) => {
                let path = canonical(r)?;
                no_wildcard(&path, "put")?;
                let says_dir = r.ends_with('/') || r.ends_with('\\') || r.ends_with(':');
                if says_dir
                    || path.len() <= 3
                    || p.kind(&path).context("lx put")?
                        == satx_core::filer_proto::NameKind::Directory
                {
                    into(&path)?
                } else {
                    path
                }
            }
        };
        if !valid_83(&dos::display(dos::file_name(&target))) {
            return Err(Hinted::new(
                format!("{} is not a DOS 8.3 name", dos::display(&target)),
                "up to 8 characters, a dot, up to 3: letters, digits and !#$%&'()-@^_`{}~",
            )
            .into());
        }
        let label = format!("lx put {}", dos::display(&target));
        let data = util::read_input_limited(
            file,
            MAX_FILE as u64,
            "satx lx puts at most 16 MiB; the palmtop's cards hold less",
        )?;
        let modified = std::fs::metadata(file)
            .and_then(|m| m.modified())
            .ok()
            .and_then(dos_time);
        if dry_run {
            let existing = entry(p, &target).context(label.clone())?;
            let what = match (&existing, overwrite) {
                (None, _) => format!("would create {}", dos::display(&target)),
                (Some(e), _) if e.is_directory() => {
                    format!("{} is a directory: put would fail", dos::display(&target))
                }
                (Some(_), true) => format!("would replace {}", dos::display(&target)),
                (Some(_), false) => format!(
                    "{} exists: put refuses without --overwrite",
                    dos::display(&target)
                ),
            };
            return Ok(Outcome {
                results: json!({
                    "path": dos::display(&target),
                    "bytes": data.len(),
                    "exists": existing.is_some(),
                    "dry_run": true,
                }),
                total: None,
                dir: None,
                hints: Vec::new(),
                text: format!("{what} ({} bytes from {})", data.len(), file.display()),
            });
        }
        if self.format == crate::output::Format::Text && std::io::stderr().is_terminal() {
            p.set_progress(Some(progress_printer()));
        }
        let put = p.put(&target, &data, modified, overwrite);
        p.set_progress(None);
        if let Err(e) = &put
            && filer::is_exists(e)
        {
            return Err(Hinted::new(
                format!("{} exists on the palmtop", dos::display(&target)),
                "--overwrite replaces it",
            )
            .into());
        }
        put.context(label)?;
        let dir = dos::parent(&target).unwrap_or(&target).to_vec();
        Ok(Outcome {
            results: json!({
                "file": file.display().to_string(),
                "path": dos::display(&target),
                "bytes": data.len(),
            }),
            total: None,
            dir: None,
            hints: vec![Hint::cmd(
                "List the directory",
                self.cmd(&format!("lx ls {}", arg(&dir))),
            )],
            text: format!(
                "{} -> {} ({} bytes)",
                file.display(),
                dos::display(&target),
                data.len()
            ),
        })
    }

    fn lx_rm(&self, p: &mut Palmtop, inputs: &[String], dry_run: bool) -> Result<Outcome> {
        let mut paths = Vec::new();
        for input in inputs {
            let path = canonical(input)?;
            no_wildcard(&path, "rm")?;
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        // Every name first: nothing is deleted if one is missing.
        for path in &paths {
            match entry(p, path).context("lx rm")? {
                None => {
                    return Err(Hinted::new(
                        format!("{}: no such file; nothing deleted", dos::display(path)),
                        "`satx lx ls` lists a directory",
                    )
                    .into());
                }
                Some(e) if e.is_directory() => {
                    return Err(Hinted::new(
                        format!("{} is a directory; nothing deleted", dos::display(path)),
                        format!(
                            "empty it, then `{}`",
                            self.cmd(&format!("lx rmdir {}", arg(path)))
                        ),
                    )
                    .into());
                }
                Some(_) => {}
            }
        }
        let shown: Vec<String> = paths.iter().map(|p| dos::display(p)).collect();
        if !dry_run {
            for path in &paths {
                p.delete(path)
                    .with_context(|| format!("lx rm {}", dos::display(path)))?;
            }
        }
        Ok(Outcome {
            results: json!({"deleted": if dry_run { Vec::new() } else { shown.clone() },
                            "dry_run": dry_run, "paths": shown}),
            total: None,
            dir: None,
            hints: Vec::new(),
            text: format!(
                "{} {}",
                if dry_run { "would delete" } else { "deleted" },
                shown.join(", ")
            ),
        })
    }

    fn lx_mv(&self, p: &mut Palmtop, from: &str, to: &str) -> Result<Outcome> {
        let from = canonical(from)?;
        let mut to = canonical(to)?;
        no_wildcard(&from, "mv")?;
        no_wildcard(&to, "mv")?;
        if from[0] != to[0] {
            return Err(Hinted::new(
                "mv works within one drive",
                "get the file and put it on the other drive",
            )
            .into());
        }
        if to.len() <= 3
            || p.kind(&to).context("lx mv")? == satx_core::filer_proto::NameKind::Directory
        {
            to = dos::join(&to, dos::file_name(&from));
        }
        let label = format!("lx mv {} {}", dos::display(&from), dos::display(&to));
        if let Err(e) = p.rename(&from, &to) {
            if filer::is_exists(&e) {
                return Err(Hinted::new(
                    format!("{} exists", dos::display(&to)),
                    "choose another name, or delete it first",
                )
                .into());
            }
            return Err(e).context(label);
        }
        Ok(done(
            format!("{} -> {}", dos::display(&from), dos::display(&to)),
            json!({"from": dos::display(&from), "to": dos::display(&to)}),
            Vec::new(),
        ))
    }
}

use std::io::IsTerminal as _;

fn done(text: String, results: Value, hints: Vec<Hint>) -> Outcome {
    Outcome {
        results,
        total: None,
        dir: None,
        hints,
        text,
    }
}

/// Progress on stderr: `2048/5000 bytes`.
fn progress_printer() -> filer::Progress {
    Box::new(|done, total| {
        let mut err = std::io::stderr();
        let _ = match total {
            Some(t) => write!(err, "\r{done}/{t} bytes"),
            None => write!(err, "\r{done} bytes"),
        };
        if total == Some(done) {
            let _ = writeln!(err);
        }
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::testkit;
    use satx_core::filer_proto::{Config, MemoryFs, Server, ServerConfig};
    use satx_core::transport::MemoryTransport;
    use std::sync::{Arc, Mutex};

    type Shared = Arc<Mutex<Server<MemoryFs>>>;

    fn palmtop() -> (Palmtop, Shared) {
        let mut fs = MemoryFs::new(b"ACD");
        fs.set_read_only(b'D');
        fs.now = DosDateTime::new(2004, 12, 14, 13, 49, 36).unwrap();
        fs.insert_file(b"C:\\HELLO.TXT", b"Hello\r\n".to_vec());
        fs.insert_file(b"C:\\_DAT\\PHONE.PDB", (0..=255).collect());
        let server: Shared = Arc::new(Mutex::new(Server::new(fs, ServerConfig::default())));
        let s = server.clone();
        let transport = MemoryTransport::new(move |w| {
            let mut s = s.lock().unwrap();
            s.handle_input(w);
            std::iter::from_fn(|| s.poll_output()).collect()
        });
        let p = Palmtop::connect(Box::new(transport), Config::default()).unwrap();
        (p, server)
    }

    fn run(cmd: LxCmd) -> (Result<Option<Outcome>>, Shared) {
        let (mut p, server) = palmtop();
        let ctx = testkit::ctx();
        (ctx.lx_on(&mut p, cmd), server)
    }

    #[test]
    fn names_and_times() {
        assert!(valid_83("PHONE.PDB"));
        assert!(valid_83("A"));
        assert!(!valid_83("TOOLONGNAME.TXT"));
        assert!(!valid_83("A.B.C"));
        assert!(!valid_83("A B.TXT"));
        assert_eq!(attr_letters(attr::ARCHIVE | attr::READ_ONLY), "RA");
        let t = DosDateTime::new(2004, 12, 14, 13, 49, 36).unwrap();
        assert_eq!(time_json(t), "2004-12-14T13:49:36");
        assert_eq!(dos_time(system_time(t).unwrap()), Some(t));
        assert_eq!(arg(b"C:\\_DAT\\A.B"), "C:/_DAT/A.B");
        assert!(parse_baud("19200").is_ok());
        assert!(parse_baud("19201").is_err());
    }

    #[test]
    fn ls_root_dir_file_and_pattern() {
        let (o, _) = run(LxCmd::Ls { path: None });
        let o = o.unwrap().unwrap();
        assert_eq!(o.total, Some(2));
        assert_eq!(o.results[0]["name"], "HELLO.TXT");
        assert_eq!(o.results[0]["modified"], "2004-12-14T13:49:36");
        assert_eq!(o.results[1]["directory"], true);
        assert_eq!(o.hints[0].cmd, "satx lx get C:/HELLO.TXT");
        assert_eq!(o.hints[1].cmd, "satx lx ls C:/_DAT");
        assert!(o.text.contains("<DIR>"), "{}", o.text);
        let (o, _) = run(LxCmd::Ls {
            path: Some("c:/_dat".into()),
        });
        let o = o.unwrap().unwrap();
        assert_eq!(o.results[0]["path"], "C:\\_DAT\\PHONE.PDB");
        assert_eq!(o.results[0]["size"], 256);
        let (o, _) = run(LxCmd::Ls {
            path: Some("C:/*.TXT".into()),
        });
        assert_eq!(o.unwrap().unwrap().total, Some(1));
        let (o, _) = run(LxCmd::Ls {
            path: Some("C:/HELLO.TXT".into()),
        });
        assert_eq!(o.unwrap().unwrap().total, Some(1));
        let (o, _) = run(LxCmd::Ls {
            path: Some("C:/NONE.TXT".into()),
        });
        let err = o.unwrap_err();
        assert!(err.to_string().contains("no such file"), "{err}");
    }

    #[test]
    fn drives() {
        let (o, _) = run(LxCmd::Drives);
        let o = o.unwrap().unwrap();
        assert_eq!(o.total, Some(3));
        assert_eq!(o.results[1]["drive"], "C");
        assert!(o.results[1]["free"].as_u64().unwrap() > 0);
    }

    #[test]
    fn get_put_rm_mv_mkdir_rmdir() {
        let dir = std::env::temp_dir().join(format!("satx-lx-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("phone.pdb");
        let (o, server) = run(LxCmd::Get {
            remote: "C:/_DAT/PHONE.PDB".into(),
            output: Some(out.clone()),
            force: false,
        });
        let o = o.unwrap().unwrap();
        assert_eq!(o.results["bytes"], 256);
        assert_eq!(std::fs::read(&out).unwrap(), (0..=255).collect::<Vec<u8>>());
        // The date came along.
        let mtime = std::fs::metadata(&out).unwrap().modified().unwrap();
        assert_eq!(dos_time(mtime), DosDateTime::new(2004, 12, 14, 13, 49, 36));
        drop(server);

        let (mut p, server) = palmtop();
        let ctx = testkit::ctx();
        let put = |p: &mut Palmtop, remote: Option<&str>, overwrite| {
            ctx.lx_on(
                p,
                LxCmd::Put {
                    file: out.clone(),
                    remote: remote.map(Into::into),
                    overwrite,
                    dry_run: false,
                },
            )
        };
        let o = put(&mut p, None, false).unwrap().unwrap();
        assert_eq!(o.results["path"], "C:\\PHONE.PDB");
        assert_eq!(
            server.lock().unwrap().fs().modified(b"C:\\PHONE.PDB"),
            DosDateTime::new(2004, 12, 14, 13, 49, 36)
        );
        let err = put(&mut p, None, false).unwrap_err();
        assert!(err.to_string().contains("exists on the palmtop"), "{err}");
        put(&mut p, None, true).unwrap();
        let o = put(&mut p, Some("C:/_DAT/"), true).unwrap().unwrap();
        assert_eq!(o.results["path"], "C:\\_DAT\\PHONE.PDB");
        let o = put(&mut p, Some("C:/_DAT/COPY.PDB"), false)
            .unwrap()
            .unwrap();
        assert_eq!(o.results["path"], "C:\\_DAT\\COPY.PDB");
        let o = ctx
            .lx_on(
                &mut p,
                LxCmd::Put {
                    file: out.clone(),
                    remote: Some("C:/_DAT/COPY.PDB".into()),
                    overwrite: false,
                    dry_run: true,
                },
            )
            .unwrap()
            .unwrap();
        assert!(
            o.text.contains("put refuses without --overwrite"),
            "{}",
            o.text
        );
        // The ROM drive.
        let err = put(&mut p, Some("D:/X.PDB"), false).unwrap_err();
        let failure = crate::error::describe(&err, &Default::default());
        assert!(
            failure.error.contains("disk write-protected (214)"),
            "{failure:?}"
        );
        assert!(failure.hint.unwrap().contains("ROM"));

        // mv into a directory that has the name already; then a rename.
        ctx.lx_on(
            &mut p,
            LxCmd::Mv {
                from: "C:/PHONE.PDB".into(),
                to: "C:/_DAT".into(),
            },
        )
        .unwrap_err();
        ctx.lx_on(
            &mut p,
            LxCmd::Mv {
                from: "C:/PHONE.PDB".into(),
                to: "C:/P2.PDB".into(),
            },
        )
        .unwrap();
        assert!(server.lock().unwrap().fs().file(b"C:\\P2.PDB").is_some());

        // rm checks every name first.
        let err = ctx
            .lx_on(
                &mut p,
                LxCmd::Rm {
                    paths: vec!["C:/P2.PDB".into(), "C:/NONE".into()],
                    dry_run: false,
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("nothing deleted"), "{err}");
        assert!(server.lock().unwrap().fs().file(b"C:\\P2.PDB").is_some());
        ctx.lx_on(
            &mut p,
            LxCmd::Rm {
                paths: vec!["C:/P2.PDB".into()],
                dry_run: false,
            },
        )
        .unwrap();
        assert!(server.lock().unwrap().fs().file(b"C:\\P2.PDB").is_none());

        ctx.lx_on(
            &mut p,
            LxCmd::Mkdir {
                path: "C:/NEW".into(),
            },
        )
        .unwrap();
        assert!(
            ctx.lx_on(
                &mut p,
                LxCmd::Mkdir {
                    path: "C:/NEW".into()
                }
            )
            .is_err()
        );
        ctx.lx_on(
            &mut p,
            LxCmd::Rmdir {
                path: "C:/NEW".into(),
            },
        )
        .unwrap();
        let err = ctx
            .lx_on(
                &mut p,
                LxCmd::Rmdir {
                    path: "C:/_DAT".into(),
                },
            )
            .unwrap_err();
        let failure = crate::error::describe(&err, &Default::default());
        assert!(failure.error.contains("access denied (211)"), "{failure:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
