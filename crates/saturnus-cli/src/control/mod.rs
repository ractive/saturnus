//! The control API of `saturnus run`: HTTP/1.1 + JSON on 127.0.0.1, guarded
//! by a per-user token file and Host and Origin checks ([`server`]), and
//! its client `saturnus ctl` ([`client`]). Request and response bodies are
//! the command and reply shapes of `web/protocol.md`; the HTTP mapping is
//! written down there. Security rules: kb/docs/control-api-security.md.

pub mod client;
pub mod server;

pub use saturnus_drive::loopback::{http, token};

use std::net::{Ipv4Addr, SocketAddrV4};

use anyhow::{Context, Result, bail};

/// The control API's default port. Outside 4848-4852 (the saturnng
/// oracle containers of satx's tests) and 4860-4889 (other bridges and tests on the owner's
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

/// How to choose another port, for a busy port's error.
pub const PORT_HINT: &str = "choose another port (for a second instance: --control PORT or SATURNUS_CONTROL, --serial tcp:PORT)";

/// An error for a failed bind of `what` on `addr`: a busy port names the
/// process listening there where the platform tells us.
pub fn bind_error(what: &str, addr: &str, e: std::io::Error) -> anyhow::Error {
    saturnus_drive::loopback::bind_error(what, addr, e, PORT_HINT)
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
}
