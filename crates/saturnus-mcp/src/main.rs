//! `saturnus-mcp`: the MCP server on stdin/stdout. See the README, "MCP
//! server".

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use rmcp::ServiceExt;
use saturnus_drive::session::Limits;
use saturnus_mcp::emulator::{Emulator, parse_model};
use saturnus_mcp::server::SaturnusMcp;

const USAGE: &str = "usage: saturnus-mcp [--model 48sx|48gx|49g|38g --rom PATH [--autostart]]

An MCP server on stdin/stdout owning one emulated calculator. With --rom it
boots that ROM at startup (model default 48sx; --autostart also starts the
Kermit server); otherwise the client calls the boot tool.";

/// What the command line asks for.
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    model: Option<String>,
    rom: Option<PathBuf>,
    autostart: bool,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Args>> {
    let mut out = Args::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--model" => out.model = Some(it.next().context("--model needs a value")?),
            "--rom" => out.rom = Some(PathBuf::from(it.next().context("--rom needs a path")?)),
            "--autostart" => out.autostart = true,
            "-h" | "--help" => return Ok(None),
            "-V" | "--version" => {
                println!("saturnus-mcp {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            other => bail!("unknown argument {other:?}\n{USAGE}"),
        }
    }
    if out.rom.is_none() && (out.model.is_some() || out.autostart) {
        bail!("--model and --autostart need --rom\n{USAGE}");
    }
    if let Some(m) = &out.model {
        parse_model(m)?;
    }
    Ok(Some(out))
}

#[tokio::main]
async fn main() -> Result<()> {
    let Some(args) = parse_args(std::env::args().skip(1))? else {
        println!("{USAGE}");
        return Ok(());
    };
    let server = SaturnusMcp::new();
    if let Some(rom) = args.rom {
        // Boot in the background so the client's initialize is answered at
        // once; the lock is taken before serving, so every tool waits for
        // the boot.
        let model = parse_model(args.model.as_deref().unwrap_or("48sx"))?;
        let mut guard = server.state().lock_owned().await;
        let autostart = args.autostart;
        tokio::task::spawn_blocking(move || {
            match Emulator::boot(model, &rom, autostart, Limits::default()) {
                Ok((emu, _)) => guard.emulator = Some(emu),
                Err(e) => {
                    let msg = format!("{e:#}");
                    eprintln!("saturnus-mcp: boot failed: {msg}");
                    guard.startup_error = Some(msg);
                }
            }
        });
    }
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .context("cannot start the MCP server on stdio")?;
    service.waiting().await.context("MCP server failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn parse(v: &[&str]) -> Result<Option<Args>> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn command_line() {
        assert_eq!(parse(&[]).unwrap(), Some(Args::default()));
        assert_eq!(
            parse(&["--model", "49g", "--rom", "/r", "--autostart"]).unwrap(),
            Some(Args {
                model: Some("49g".into()),
                rom: Some("/r".into()),
                autostart: true
            })
        );
        assert_eq!(parse(&["--help"]).unwrap(), None);
        assert!(parse(&["--model", "48sx"]).is_err());
        assert!(parse(&["--rom"]).is_err());
        assert!(parse(&["--model", "50g", "--rom", "/r"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
    }
}
