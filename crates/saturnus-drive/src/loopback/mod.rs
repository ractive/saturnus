//! HTTP servers on 127.0.0.1 under one set of rules (kb:
//! docs/control-api-security): the CLI's control API (`saturnus run
//! --serve`) and the desktop app's MCP endpoint. A per-user bearer token
//! ([`token`]), the bounded HTTP/1.1 of [`http`], and the checks and
//! connection budgets every request passes ([`guard`]). Binding is to the
//! IPv4 loopback address only ([`bind`]); a busy port names its listener
//! ([`bind_error`]).

pub mod guard;
pub mod http;
pub mod token;

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::process::Command;

/// Bind `what` on `addr`, which must be 127.0.0.1; `hint` says how to
/// choose another port when this one is taken.
pub fn bind(what: &str, addr: SocketAddrV4, hint: &str) -> anyhow::Result<TcpListener> {
    if *addr.ip() != Ipv4Addr::LOCALHOST {
        anyhow::bail!("the {what} listens on 127.0.0.1 only");
    }
    TcpListener::bind(addr).map_err(|e| bind_error(what, &addr.to_string(), e, hint))
}

/// An error for a failed bind of `what` on `addr`: a busy port names the
/// process listening there where the platform tells us, then `hint` (how
/// to choose another port).
pub fn bind_error(what: &str, addr: &str, e: std::io::Error, hint: &str) -> anyhow::Error {
    if e.kind() != std::io::ErrorKind::AddrInUse {
        return anyhow::Error::new(e).context(format!("cannot listen on {addr} for the {what}"));
    }
    let port = addr
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse::<u16>().ok());
    let who = port
        .and_then(who_listens)
        .unwrap_or_else(|| "another program (its process ID is not available)".to_string());
    anyhow::anyhow!("cannot listen on {addr} for the {what}: the port is in use by {who}; {hint}")
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

    /// A busy port is refused with a message naming the listener, or
    /// saying that it cannot be named, and the hint; another address than
    /// 127.0.0.1 is refused.
    #[test]
    fn a_busy_port_names_its_listener() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let e = std::net::TcpListener::bind(&addr).unwrap_err();
        let msg = bind_error("control API", &addr, e, "choose another").to_string();
        assert!(msg.contains("in use by"), "{msg}");
        assert!(msg.ends_with("; choose another"), "{msg}");
        let named = msg.contains(&format!("pid {}", std::process::id()));
        assert!(named || msg.contains("another program"), "{msg}");
        let port = l.local_addr().unwrap().port();
        let e = bind(
            "MCP endpoint",
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, port),
            "x",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("in use by"), "{e}");
        let e = bind(
            "MCP endpoint",
            SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0),
            "x",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("127.0.0.1 only"), "{e}");
    }
}
