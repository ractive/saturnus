//! The control API of `saturnus run`: HTTP/1.1 + JSON on 127.0.0.1, guarded
//! by a per-user token file and Host and Origin checks ([`server`]), and
//! its client `saturnus ctl` ([`client`]). Request and response bodies are
//! the command and reply shapes of `web/protocol.md`; the HTTP mapping is
//! written down there. Security rules: kb/docs/control-api-security.md.

pub mod client;
pub mod http;
pub mod server;
pub mod token;

use std::net::{Ipv4Addr, SocketAddrV4};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The control API's default port. Outside 4848-4852 (the hptx test
/// containers) and 4860-4889 (other bridges and tests on the owner's
/// machine); see the decision log, iteration 17.
pub const DEFAULT_CONTROL_PORT: u16 = 4840;
/// The serial bridge's default port when `run` serves without `--serial`.
pub const DEFAULT_SERIAL_PORT: u16 = 4841;
/// Selects the control API's address for `run` and `ctl` (a second
/// instance): `PORT` or `127.0.0.1:PORT`.
pub const CONTROL_ENV: &str = "SATURNUS_CONTROL";

/// The port of a control address: `PORT`, `127.0.0.1:PORT`,
/// `localhost:PORT` (an `http://` prefix is allowed). Any other host is
/// refused: the API binds the IPv4 loopback address only.
pub fn parse_control(s: &str) -> Result<u16> {
    let t = s.strip_prefix("http://").unwrap_or(s).trim_end_matches('/');
    let (host, port) = t.rsplit_once(':').unwrap_or(("127.0.0.1", t));
    if host != "127.0.0.1" && !host.eq_ignore_ascii_case("localhost") {
        bail!(
            "the control API listens on 127.0.0.1 only; {s:?} names another address \
             (give PORT or 127.0.0.1:PORT)"
        );
    }
    port.parse()
        .with_context(|| format!("bad port in control address {s:?}"))
}

/// The control API's address: `flag`, else `$SATURNUS_CONTROL`, else the
/// default port, always on 127.0.0.1.
pub fn resolve_control(flag: Option<&str>) -> Result<SocketAddrV4> {
    let port = match flag {
        Some(f) => parse_control(f)?,
        None => match std::env::var(CONTROL_ENV) {
            Ok(v) if !v.is_empty() => {
                parse_control(&v).with_context(|| format!("in {CONTROL_ENV}"))?
            }
            _ => DEFAULT_CONTROL_PORT,
        },
    };
    Ok(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
}

/// An error for a failed bind of `what` on `addr`: a busy port names the
/// process listening there where the platform tells us.
pub fn bind_error(what: &str, addr: &str, e: std::io::Error) -> anyhow::Error {
    if e.kind() != std::io::ErrorKind::AddrInUse {
        return anyhow::Error::new(e).context(format!("cannot listen on {addr} for the {what}"));
    }
    let port = addr
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse::<u16>().ok());
    let who = port
        .and_then(who_listens)
        .unwrap_or_else(|| "another program (its process ID is not available)".to_string());
    anyhow::anyhow!(
        "cannot listen on {addr} for the {what}: the port is in use by {who}; \
         choose another port (for a second instance: --control PORT or {CONTROL_ENV}, \
         --serial tcp:PORT)"
    )
}

/// The process listening on TCP `port`, as "pid N (command)", if the
/// platform's tools tell us: `lsof` (macOS, Linux), `ss` (Linux), `netstat`
/// and `tasklist` (Windows).
pub fn who_listens(port: u16) -> Option<String> {
    #[cfg(unix)]
    {
        lsof(port).or_else(|| ss(port))
    }
    #[cfg(windows)]
    {
        netstat(port)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = port;
        None
    }
}

fn output(cmd: &mut Command) -> Option<String> {
    let out = cmd.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(unix)]
fn lsof(port: u16) -> Option<String> {
    let text = output(Command::new("lsof").args([
        "-nP",
        &format!("-iTCP:{port}"),
        "-sTCP:LISTEN",
        "-Fpc",
    ]))?;
    let mut pid = None;
    let mut name = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix('p') {
            if pid.is_some() {
                break;
            }
            pid = Some(p.to_string());
        } else if let Some(c) = line.strip_prefix('c') {
            name = Some(c.to_string());
        }
    }
    let pid = pid?;
    Some(match name {
        Some(n) => format!("pid {pid} ({n})"),
        None => format!("pid {pid}"),
    })
}

#[cfg(unix)]
fn ss(port: u16) -> Option<String> {
    let text = output(Command::new("ss").args(["-Hltnp", &format!("sport = :{port}")]))?;
    // users:(("saturnus",pid=1234,fd=3))
    let users = text
        .lines()
        .find_map(|l| l.split_once("users:((").map(|(_, u)| u))?;
    let name = users.split('"').nth(1).unwrap_or("?");
    let pid = users
        .split("pid=")
        .nth(1)?
        .split(|c: char| !c.is_ascii_digit())
        .next()?;
    Some(format!("pid {pid} ({name})"))
}

#[cfg(windows)]
fn netstat(port: u16) -> Option<String> {
    let text = output(Command::new("netstat").args(["-ano", "-p", "TCP"]))?;
    let suffix = format!(":{port}");
    let pid = text.lines().find_map(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        (f.len() >= 5 && f[1].ends_with(&suffix) && f[3].eq_ignore_ascii_case("LISTENING"))
            .then(|| f[4].to_string())
    })?;
    let name = output(Command::new("tasklist").args([
        "/FI",
        &format!("PID eq {pid}"),
        "/FO",
        "CSV",
        "/NH",
    ]))
    .and_then(|t| t.split('"').nth(1).map(str::to_string));
    Some(match name {
        Some(n) => format!("pid {pid} ({n})"),
        None => format!("pid {pid}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_addresses_are_loopback_only() {
        assert_eq!(parse_control("4890").unwrap(), 4890);
        assert_eq!(parse_control("127.0.0.1:4891").unwrap(), 4891);
        assert_eq!(parse_control("localhost:4892").unwrap(), 4892);
        assert_eq!(parse_control("http://127.0.0.1:4893/").unwrap(), 4893);
        assert_eq!(parse_control("127.0.0.1:0").unwrap(), 0);
        for bad in [
            "0.0.0.0:4840",
            "192.168.1.2:4840",
            "[::1]:4840",
            "::1:4840",
            "evil:1",
        ] {
            let e = parse_control(bad).unwrap_err().to_string();
            assert!(e.contains("127.0.0.1 only"), "{bad}: {e}");
        }
        assert!(parse_control("127.0.0.1:99999").is_err());
        assert!(parse_control("").is_err());
        assert_eq!(
            resolve_control(Some("4894")).unwrap(),
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 4894)
        );
    }

    /// A busy port is refused with a message naming the listener, or
    /// saying that it cannot be named.
    #[test]
    fn a_busy_port_names_its_listener() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let e = std::net::TcpListener::bind(&addr).unwrap_err();
        let msg = bind_error("control API", &addr, e).to_string();
        assert!(msg.contains("in use by"), "{msg}");
        let named = msg.contains(&format!("pid {}", std::process::id()));
        assert!(named || msg.contains("another program"), "{msg}");
    }
}
