//! `satx xserv`: the XSERV command server of the 49g+/50g (and the 49G after
//! ROM 1.10), thin over [`satx_core::XservSession`].
//!
//! **Tested on a real 49G** (ROM 1.18, 2026-11-08): `mem`, `ls`, `get`,
//! `put` and `eval`; and on the emulated 49G (ROM 2.15), `quit` too; not
//! yet on a 49g+ or 50g. The framing follows the wiki's description of HP's
//! client code (wiki: protocols/xserv).

use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;
use satx_core::calc::validate_name;
use satx_core::names;
use satx_core::transport;
use satx_core::xserv::{self as core_xserv, DirRecord, XservCommand};
use satx_core::{Error, XmodemOptions, XservOptions, XservSession};
use serde_json::{Value, json};

use crate::commands::{Ctx, object_type_name_binary, refuse_existing_file, write_file};
use crate::error::Hinted;
use crate::output::{Hint, Outcome, shell_quote};
use crate::util;
use crate::xmodem::check_name;

/// Shown under every `xserv` subcommand's help.
const XSERV_HELP: &str = "Tested on a real 49G (ROM 1.18) and the emulated 49G (ROM 2.15); not \
yet on a 49g+ or 50g. Start XSERV on the calculator instead of SERVER: type XSERV and press ENTER.";

/// The `xserv` subcommands.
#[derive(Subcommand, Debug)]
pub enum XservCmd {
    /// List the current directory (command L); sizes as `satx ls` gives them.
    #[command(after_help = XSERV_HELP)]
    Ls,
    /// Download a variable (command G, then XModem).
    #[command(after_help = XSERV_HELP)]
    Get {
        /// Variable in the current directory.
        name: String,
        /// Output file [default: NAME.hp]; - for stdout.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Replace an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Upload a file as a variable (command P, then XModem).
    #[command(after_help = XSERV_HELP)]
    Put {
        /// File to upload; - for stdin (then --as is required).
        file: PathBuf,
        /// Variable name [default: the file name without extension, %XX read back].
        #[arg(long = "as", value_name = "NAME")]
        name: Option<String>,
        /// Print the bytes satx would send, open no link.
        #[arg(long)]
        dry_run: bool,
    },
    /// Execute RPL on the calculator (command E); results stay on its stack.
    #[command(after_help = XSERV_HELP)]
    Eval {
        /// RPL words, joined with spaces.
        #[arg(required = true, value_name = "RPL", allow_negative_numbers = true)]
        words: Vec<String>,
        /// Print the bytes satx would send, open no link.
        #[arg(long)]
        dry_run: bool,
    },
    /// Free memory in bytes (command M).
    #[command(after_help = XSERV_HELP)]
    Mem,
    /// End XSERV on the calculator (command Q).
    #[command(after_help = XSERV_HELP)]
    Quit,
}

/// Hex dump, `00 2a ff`.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The bytes `command` puts on the line: the command byte, then its packet.
fn wire_bytes(command: &XservCommand) -> Result<(u8, Option<Vec<u8>>)> {
    Ok((command.byte(), command.packet()?))
}

/// The listing as text and JSON.
fn listing_outcome(records: &[DirRecord], ctx: &Ctx) -> Outcome {
    let width = records
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let mut text = format!(
        "{} variable{}\n",
        records.len(),
        if records.len() == 1 { "" } else { "s" }
    );
    let mut results = Vec::new();
    for r in records {
        let size = r.size();
        let kind = r.kind();
        let pad = width - r.name.chars().count();
        let _ = writeln!(
            text,
            "  {}{} {:>8}  {:<16} #{:04X}h",
            r.name,
            " ".repeat(pad),
            util::number_text(size),
            kind,
            r.crc
        );
        results.push(json!({
            "name": r.name,
            "size": util::number(size),
            "nibbles": r.size_nibbles,
            "type": kind,
            "prolog": r.prolog,
            "crc": r.crc,
            "directory": r.is_directory(),
        }));
    }
    let hints = records
        .iter()
        .find(|r| !r.is_directory())
        .map(|r| {
            vec![Hint::cmd(
                format!("Download {}", r.name),
                ctx.cmd(&format!("xserv get {}", shell_quote(&r.name))),
            )]
        })
        .unwrap_or_default();
    Outcome {
        total: Some(records.len() as u64),
        results: Value::from(results),
        dir: None,
        hints,
        text,
    }
}

/// `--dry-run`: the bytes, no link.
fn dry_run_outcome(command: &XservCommand, what: String) -> Result<Outcome> {
    let (byte, packet) = wire_bytes(command)?;
    let packet = packet.unwrap_or_default();
    Ok(Outcome {
        results: json!({
            "command": char::from(byte).to_string(),
            "packet_hex": hex(&packet),
            "dry_run": true,
        }),
        total: None,
        dir: None,
        hints: Vec::new(),
        text: format!(
            "{what}\ncommand byte  {}\npacket        {}",
            char::from(byte),
            hex(&packet)
        ),
    })
}

/// The components of `dir` below HOME. Each is evaluated by the
/// calculator, so it must be a plain name ([`validate_name`]), as for
/// `Calculator::cd`.
fn dir_components(dir: &str) -> Result<Vec<String>> {
    let components = util::parse_dir(dir);
    for name in &components {
        validate_name(name)
            .with_context(|| format!("xserv: cd {}", util::dir_string(&components)))?;
    }
    Ok(components)
}

/// Change to `dir` from HOME like `Calculator::cd`: `HOME`, then per
/// component a listing that must show it as a directory before it is
/// evaluated (evaluating a variable would run it or push its value and
/// leave the directory unchanged).
fn xserv_cd(client: &mut XservSession, dir: &str) -> Result<()> {
    let components = dir_components(dir)?;
    let label = format!("xserv: cd {}", util::dir_string(&components));
    client.execute("HOME").with_context(|| label.clone())?;
    for name in &components {
        let records = client.list().with_context(|| label.clone())?;
        if !records.iter().any(|r| r.name == *name && r.is_directory()) {
            // XSERV cannot say where it was: HOME, not part of the way.
            let _ = client.execute("HOME");
            return Err(Error::Reply(format!("{name}: no such directory"))).context(label);
        }
        client.execute(name).with_context(|| label.clone())?;
    }
    Ok(())
}

impl Ctx {
    /// Open the link to a calculator running XSERV and change to `--dir`.
    fn xserv_open(&mut self) -> Result<XservSession> {
        let addr = crate::port::resolve(self.global.port.as_deref())?;
        self.link.addr = Some(addr.clone());
        let link = transport::open(&addr).with_context(|| format!("cannot open {addr}"))?;
        let mut client = XservSession::new(link, XservOptions::default());
        if let Some(dir) = self.global.dir.clone() {
            xserv_cd(&mut client, &dir)?;
        }
        Ok(client)
    }

    /// XModem options for an XSERV transfer: the 49G profile, the transfer
    /// starts at once.
    fn xserv_xmodem(&self) -> XmodemOptions {
        core_xserv::transfer_options(self.link.timeout)
    }

    pub(crate) fn xserv(&mut self, command: XservCmd) -> Result<Option<Outcome>> {
        let label = |cmd: &str| format!("xserv {cmd}");
        match command {
            XservCmd::Ls => {
                let mut client = self.xserv_open()?;
                let records = client.list().context(label("ls"))?;
                Ok(Some(listing_outcome(&records, self)))
            }
            XservCmd::Mem => {
                let mut client = self.xserv_open()?;
                let free = client.memory().context(label("mem"))?;
                Ok(Some(Outcome {
                    results: json!({"free_bytes": util::number(free)}),
                    total: None,
                    dir: None,
                    hints: Vec::new(),
                    text: format!("free      {} bytes", util::number_text(free)),
                }))
            }
            XservCmd::Quit => {
                let mut client = self.xserv_open()?;
                let ended = client.quit().context(label("quit"))?;
                let (text, hints) = if ended {
                    (
                        "XSERV has ended.",
                        vec![Hint::advice(
                            "Type XSERV (or SERVER for the Kermit server) on the calculator to connect again",
                        )],
                    )
                } else {
                    (
                        "XSERV still answers: this ROM does not know Q.",
                        vec![Hint::advice("Press ON on the calculator to end XSERV")],
                    )
                };
                Ok(Some(Outcome {
                    results: json!({"ended": ended}),
                    total: None,
                    dir: None,
                    hints,
                    text: text.to_string(),
                }))
            }
            XservCmd::Eval { words, dry_run } => {
                let rpl = words.join(" ");
                let cmd = XservCommand::Execute(rpl.clone());
                if dry_run {
                    return dry_run_outcome(&cmd, format!("Would execute {rpl}")).map(Some);
                }
                let mut client = self.xserv_open()?;
                client.execute(&rpl).context(label("eval"))?;
                Ok(Some(Outcome {
                    results: json!({"command": rpl}),
                    total: None,
                    dir: None,
                    hints: Vec::new(),
                    text: format!("Executed {rpl}; results stay on the calculator's stack."),
                }))
            }
            XservCmd::Get {
                name,
                output,
                force,
            } => self.xserv_get(&name, output.as_deref(), force),
            XservCmd::Put {
                file,
                name,
                dry_run,
            } => self.xserv_put(&file, name, dry_run).map(Some),
        }
    }

    fn xserv_get(
        &mut self,
        name: &str,
        output: Option<&Path>,
        force: bool,
    ) -> Result<Option<Outcome>> {
        validate_name(name).with_context(|| format!("xserv get {name}"))?;
        let to_stdout = output == Some(Path::new("-"));
        let file = output.map_or_else(
            || PathBuf::from(names::file_name(name, names::Format::Binary)),
            Path::to_path_buf,
        );
        if !to_stdout {
            refuse_existing_file(&file, force)?;
        }
        let options = self.xserv_xmodem();
        let mut client = self.xserv_open()?;
        let what = format!("xserv get {name}");
        let got = client
            .get(name, &options, &mut |_| {})
            .with_context(|| what.clone())?;
        if to_stdout {
            let mut out = std::io::stdout().lock();
            out.write_all(&got.data).context("writing to stdout")?;
            out.flush().context("writing to stdout")?;
            return Ok(None);
        }
        write_file(&file, &got.data, force)?;
        let kind = object_type_name_binary(&got.data);
        Ok(Some(Outcome {
            results: json!({
                "name": name,
                "file": file.display().to_string(),
                "bytes": got.data.len(),
                "received": got.received,
                "stripped": got.stripped,
                "check": check_name(got.check),
                "type": kind,
            }),
            total: None,
            dir: None,
            hints: Vec::new(),
            text: format!(
                "{name} -> {} ({} bytes, xserv, {})",
                file.display(),
                got.data.len(),
                check_name(got.check)
            ),
        }))
    }

    fn xserv_put(&mut self, file: &Path, name: Option<String>, dry_run: bool) -> Result<Outcome> {
        let from_stdin = file == Path::new("-");
        let name = match (name, from_stdin) {
            (Some(n), _) => n,
            (None, true) => {
                return Err(Hinted::new(
                    "xserv put -: no variable name",
                    "give one with --as NAME",
                )
                .into());
            }
            (None, false) => util::name_from_file(file)?,
        };
        validate_name(&name).with_context(|| format!("xserv put {name}"))?;
        let data = util::read_input(file)?;
        let label = file.display().to_string();
        let cmd = XservCommand::Put(name.clone());
        if dry_run {
            return dry_run_outcome(
                &cmd,
                format!(
                    "Would store {label} ({} bytes) as {name}, then send it by XModem",
                    data.len()
                ),
            );
        }
        let options = self.xserv_xmodem();
        let mut client = self.xserv_open()?;
        let what = format!("xserv put {name}");
        let report = client
            .put(&name, &data, &options, &mut |_| {})
            .with_context(|| what.clone())?;
        Ok(Outcome {
            results: json!({
                "file": label,
                "name": name,
                "bytes": report.bytes,
                "check": check_name(report.check),
            }),
            total: None,
            dir: None,
            hints: vec![Hint::cmd("List the directory", self.cmd("xserv ls"))],
            text: format!(
                "{label} -> {name} ({} bytes, xserv, {})",
                report.bytes,
                check_name(report.check)
            ),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use satx_core::transport::MemoryTransport;
    use satx_core::xmodem_proto::codec::ACK;
    use satx_core::xserv::encode_packet;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    type Log = Arc<Mutex<Vec<Vec<u8>>>>;

    /// Audit PR #18, #15: `--dir` components are checked like
    /// `Calculator::cd` before the calculator evaluates them.
    #[test]
    fn dir_components_are_names() {
        assert_eq!(dir_components("HOME/A/B").unwrap(), ["A", "B"]);
        assert!(dir_components("/").unwrap().is_empty());
        for bad in ["A/1 PURGE", "A/'X'", "{ HOME \u{ab}X\u{bb} }", "A/CLEAR;X"] {
            let err = dir_components(bad).unwrap_err();
            assert!(
                err.chain().any(|c| matches!(
                    c.downcast_ref::<satx_core::Error>(),
                    Some(satx_core::Error::Name(_))
                )),
                "{bad}: {err:?}"
            );
        }
    }

    /// A calculator that logs every write and answers with `reply`.
    fn client(
        mut reply: impl FnMut(&[u8]) -> Vec<Vec<u8>> + Send + 'static,
    ) -> (XservSession, Log) {
        let log: Log = Arc::default();
        let seen = log.clone();
        let transport = MemoryTransport::new(move |w| {
            seen.lock().unwrap().push(w.to_vec());
            reply(w)
        });
        let options = XservOptions {
            quiet: Duration::from_millis(1),
            answer: Duration::from_millis(30),
            busy: Duration::from_millis(200),
            timeout: Duration::from_millis(30),
        };
        (XservSession::new(Box::new(transport), options), log)
    }

    /// PR #20 review: each `--dir` component must be a directory in the
    /// listing before it is evaluated; a variable is refused and nothing
    /// after `HOME` is executed.
    #[test]
    fn cd_checks_each_component_is_a_directory() {
        let records = [
            &[3][..],
            b"SUB",
            &[0x96, 0x2A, 0x0D, 0, 0, 0, 0],
            &[3][..],
            b"VAR",
            &[0x2C, 0x2A, 0x0A, 0, 0, 0, 0],
        ]
        .concat();
        let listing = encode_packet(&records).unwrap();
        let server = move |w: &[u8]| {
            if w == b"L" {
                vec![listing.clone()]
            } else if w == b"M" {
                vec![encode_packet(b"1.").unwrap()]
            } else if w.len() > 1 {
                vec![vec![ACK]]
            } else {
                vec![]
            }
        };
        let executed = |log: &Log| -> Vec<Vec<u8>> {
            let log = log.lock().unwrap();
            log.windows(2)
                .filter(|w| w[0] == b"E")
                .map(|w| w[1].clone())
                .collect()
        };
        let (mut c, log) = client(server.clone());
        xserv_cd(&mut c, "HOME/SUB").unwrap();
        assert_eq!(
            executed(&log),
            [
                encode_packet(b"HOME").unwrap(),
                encode_packet(b"SUB").unwrap()
            ]
        );

        for dir in ["VAR", "NOSUCH", "SUB/VAR"] {
            let (mut c, log) = client(server.clone());
            let err = xserv_cd(&mut c, dir).unwrap_err();
            assert!(
                format!("{err:#}").contains("no such directory"),
                "{dir}: {err:#}"
            );
            let ran = executed(&log);
            assert!(
                !ran.contains(&encode_packet(b"VAR").unwrap())
                    && !ran.contains(&encode_packet(b"NOSUCH").unwrap()),
                "{dir}: {ran:?}"
            );
            assert_eq!(ran.last(), Some(&encode_packet(b"HOME").unwrap()), "{dir}");
        }
    }

    #[test]
    fn dry_run_shows_the_bytes() {
        let o = dry_run_outcome(&XservCommand::Put("AB".into()), "x".into()).unwrap();
        assert_eq!(o.results["command"], "P");
        assert_eq!(o.results["packet_hex"], "00 02 41 42 83");
    }
}
