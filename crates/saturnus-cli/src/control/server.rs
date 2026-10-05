//! The control API's HTTP server: one thread accepts on 127.0.0.1, each
//! connection gets a thread of its own (at most [`MAX_CONNECTIONS`]), and
//! a handler talks to the machine thread only through the protocol's
//! command channel, so a slow or stalled client never holds the machine
//! or the serial bridge.
//!
//! Every request passes, in this order: a bounded head read (size and
//! time), the `Host` check (421), the `Origin` check (403), `OPTIONS`
//! refused (405), the bearer token (401, no detail), the route (404), the
//! method (405: `GET` never changes anything), the body cap (413) and a
//! bounded body read (408). No CORS header is ever sent. The server never
//! takes a file path: snapshots travel as bytes.

use std::net::{SocketAddrV4, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use saturnus_drive::runner::{self, MAX_STATE_FILE, Request};
use serde_json::{Value, json};

use super::http::{self, Head, ReadError, Response};
use super::token::Token;

/// Most connections served at once; more are answered 503 at once.
pub const MAX_CONNECTIONS: usize = 8;
/// Wall time a client has to send its request head.
pub const HEAD_TIMEOUT: Duration = Duration::from_secs(5);
/// Wall time a client has to send its body.
pub const BODY_TIMEOUT: Duration = Duration::from_secs(20);
/// Wall time a client has to take the response.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// Longest a handler waits for the machine thread: a key script may run
/// [`runner::SCRIPT_WALL_LIMIT`], and commands queue behind each other.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(90);
/// Largest JSON body: a `poke` of [`runner::MAX_MEM_NIBBLES`] and a key
/// script of 64 KiB fit.
pub const MAX_JSON_BODY: usize = 256 * 1024;
/// Largest snapshot body: the largest state file (`MAX_STATE_FILE`, the
/// 49G's 2.6 MB with room to spare).
pub const MAX_SNAPSHOT_BODY: usize = MAX_STATE_FILE as usize;

/// What the server needs: the bound port, the token and the machine.
#[derive(Debug, Clone)]
pub struct Config {
    /// The bound port; `Host` and `Origin` must name it.
    pub port: u16,
    /// The bearer token.
    pub token: Token,
    /// The machine thread's command channel.
    pub tx: Sender<Request>,
}

/// The API's endpoints, `/v1/<name>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Endpoint {
    Screen,
    Keys,
    Type,
    Mem,
    Snapshot,
    Info,
    Cycles,
    Model,
    Stack,
    Tree,
    Flags,
}

impl Endpoint {
    fn from_path(path: &str) -> Option<Self> {
        Some(match path.strip_prefix("/v1/")? {
            "screen" => Self::Screen,
            "keys" => Self::Keys,
            "type" => Self::Type,
            "mem" => Self::Mem,
            "snapshot" => Self::Snapshot,
            "info" => Self::Info,
            "cycles" => Self::Cycles,
            "model" => Self::Model,
            "stack" => Self::Stack,
            "tree" => Self::Tree,
            "flags" => Self::Flags,
            _ => return None,
        })
    }

    /// The methods it takes; `GET` only where nothing changes.
    fn methods(self) -> &'static [&'static str] {
        match self {
            Self::Keys | Self::Type => &["POST"],
            Self::Mem => &["GET", "POST"],
            Self::Snapshot => &["GET", "PUT", "POST"],
            _ => &["GET"],
        }
    }

    /// The protocol commands a JSON `POST` may carry.
    fn commands(self) -> &'static [&'static str] {
        match self {
            Self::Keys => &[
                "keyScript",
                "keyDown",
                "keyUp",
                "keyUpAll",
                "typeKeys",
                "releaseAll",
            ],
            Self::Type => &["typeText"],
            Self::Mem => &["poke"],
            _ => &[],
        }
    }
}

/// Serve `listener` on a thread of its own, for as long as the process
/// lives.
pub fn spawn(listener: TcpListener, cfg: Config) -> std::io::Result<()> {
    let cfg = Arc::new(cfg);
    std::thread::Builder::new()
        .name("saturnus-control".into())
        .spawn(move || accept(&listener, &cfg))?;
    Ok(())
}

/// Counts a connection while it lives.
struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn accept(listener: &TcpListener, cfg: &Arc<Config>) {
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::SeqCst);
            // Never wait for this client: a short write timeout, then gone.
            let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
            let _ = error(503, "too many connections").write_to(&mut stream);
            let _ = stream.shutdown(std::net::Shutdown::Write);
            continue;
        }
        let slot = Slot(Arc::clone(&active));
        let cfg = Arc::clone(cfg);
        let spawned = std::thread::Builder::new()
            .name("saturnus-control-conn".into())
            .spawn(move || {
                let _slot = slot;
                serve(stream, &cfg);
            });
        // Out of threads: the connection (and its slot) is dropped.
        drop(spawned);
    }
}

fn serve(mut stream: TcpStream, cfg: &Config) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    if let Some(resp) = respond(&mut stream, cfg) {
        let _ = resp.write_to(&mut stream);
    }
    linger(&mut stream);
}

/// Most a closing connection reads and drops after its response, and for
/// how long: closing a socket with unread bytes (a refused request's body)
/// resets it, and the client may lose the response.
const LINGER_BYTES: usize = 1024 * 1024;
const LINGER_TIME: Duration = Duration::from_secs(1);

/// Finish the response (FIN), then drain what the client still sends, a
/// bounded amount for a bounded time, before closing.
fn linger(stream: &mut TcpStream) {
    use std::io::Read as _;
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let deadline = Instant::now() + LINGER_TIME;
    let mut buf = [0u8; 16 * 1024];
    let mut total = 0;
    while total < LINGER_BYTES {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }
}

/// The reply shape of `web/protocol.md` for a failure.
fn error(status: u16, message: &str) -> Response {
    json_response(
        status,
        &json!({"type": "reply", "ok": false, "error": message}),
    )
}

fn json_response(status: u16, v: &Value) -> Response {
    Response {
        status,
        content_type: "application/json",
        headers: Vec::new(),
        body: v.to_string().into_bytes(),
    }
}

fn not_allowed(allow: &[&str], message: &str) -> Response {
    let mut r = error(405, message);
    r.headers.push(("Allow", allow.join(", ")));
    r
}

/// The checks that need only the head, in order. `Ok` is the endpoint.
fn check(head: &Head, cfg: &Config) -> Result<Endpoint, Response> {
    let bad = |m: String| error(400, &m);
    // Host: the bound address, or localhost, with the bound port.
    let host = head.header("host").map_err(bad)?;
    let ours = [
        format!("127.0.0.1:{}", cfg.port),
        format!("localhost:{}", cfg.port),
    ];
    if !host.is_some_and(|h| ours.iter().any(|o| o.eq_ignore_ascii_case(h))) {
        return Err(error(
            421,
            &format!("Host must be {} or {}", ours[0], ours[1]),
        ));
    }
    // A browser names the page's origin; only the API's own is accepted.
    if let Some(origin) = head.header("origin").map_err(bad)? {
        let own = ours
            .iter()
            .any(|o| origin.eq_ignore_ascii_case(&format!("http://{o}")));
        if !own {
            return Err(error(403, "cross-origin requests are refused"));
        }
    }
    let method = head.first.as_str();
    if method == "OPTIONS" {
        return Err(not_allowed(
            &["GET", "POST", "PUT"],
            "OPTIONS is refused (no CORS)",
        ));
    }
    let presented = head.header("authorization").ok().flatten().and_then(|a| {
        let (scheme, cred) = a.split_once(' ')?;
        scheme.eq_ignore_ascii_case("bearer").then_some(cred.trim())
    });
    if !presented.is_some_and(|p| cfg.token.matches(p.as_bytes())) {
        // No detail: not which part was wrong, nothing about the token.
        return Err(Response {
            status: 401,
            content_type: "application/json",
            headers: vec![("WWW-Authenticate", "Bearer".to_string())],
            body: b"{}".to_vec(),
        });
    }
    let path = head.second.split('?').next().unwrap_or_default();
    let ep = Endpoint::from_path(path).ok_or_else(|| error(404, "no such endpoint"))?;
    if !ep.methods().contains(&method) {
        return Err(not_allowed(
            ep.methods(),
            &format!("{method} is not allowed on {path}"),
        ));
    }
    Ok(ep)
}

/// Handle one request; `None` when the client is gone (nothing to send).
fn respond(stream: &mut TcpStream, cfg: &Config) -> Option<Response> {
    let (head, rest) = match http::read_head(stream, Instant::now() + HEAD_TIMEOUT) {
        Ok(h) => h,
        Err(ReadError::Timeout) => return Some(error(408, "request head not received in time")),
        Err(ReadError::TooLarge) => return Some(error(431, "request head too large")),
        Err(ReadError::Bad(m)) => return Some(error(400, &m)),
        Err(ReadError::Closed | ReadError::Io(_)) => return None,
    };
    if !matches!(head.third.as_str(), "HTTP/1.1" | "HTTP/1.0") {
        return Some(error(400, "HTTP/1.1 expected"));
    }
    let ep = match check(&head, cfg) {
        Ok(ep) => ep,
        Err(r) => return Some(r),
    };
    let len = match head.content_length() {
        Ok(n) => n,
        Err(m) => return Some(error(400, &m)),
    };
    let cap = if ep == Endpoint::Snapshot {
        MAX_SNAPSHOT_BODY
    } else {
        MAX_JSON_BODY
    };
    if len > cap {
        return Some(error(
            413,
            &format!("body of {len} bytes is over the cap of {cap}"),
        ));
    }
    let body = match http::read_body(stream, rest, len, Instant::now() + BODY_TIMEOUT) {
        Ok(b) => b,
        Err(ReadError::Timeout) => return Some(error(408, "request body not received in time")),
        Err(ReadError::Bad(m)) => return Some(error(400, &m)),
        Err(_) => return None,
    };
    Some(dispatch(ep, &head, &body, cfg).unwrap_or_else(|r| r))
}

/// The query parameter `name` of the request target.
fn query<'a>(head: &'a Head, name: &str) -> Option<&'a str> {
    head.second.split_once('?').and_then(|(_, q)| {
        q.split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v)
    })
}

/// A number in a query: decimal, or hex after `0x` or `#` (`%23`).
fn query_number(head: &Head, name: &str) -> Result<u64, Response> {
    let v = query(head, name).ok_or_else(|| error(400, &format!("missing ?{name}=")))?;
    let hex = v
        .strip_prefix("0x")
        .or_else(|| v.strip_prefix("0X"))
        .or_else(|| v.strip_prefix('#'))
        .or_else(|| v.strip_prefix("%23"));
    match hex {
        Some(h) => u64::from_str_radix(h, 16),
        None => v.parse(),
    }
    .map_err(|_| error(400, &format!("bad number {v:?} for {name}")))
}

fn media_type(head: &Head) -> Result<String, Response> {
    let ct = head.header("content-type").map_err(|m| error(400, &m))?;
    Ok(ct
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase())
}

/// Send `msg` to the machine thread and wait for its reply.
fn call(cfg: &Config, mut msg: Value) -> Result<Value, Response> {
    msg["v"] = json!(runner::PROTOCOL);
    let (reply, answer) = channel();
    cfg.tx
        .send(Request {
            msg,
            file: None,
            reply: Some(reply),
        })
        .map_err(|_| error(503, "the machine thread has stopped"))?;
    match answer.recv_timeout(REPLY_TIMEOUT) {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(m)) => Err(error(422, &m)),
        Err(_) => Err(error(504, "the machine did not answer in time")),
    }
}

fn ok(result: Value) -> Response {
    json_response(200, &json!({"type": "reply", "ok": true, "result": result}))
}

/// A *bytes* field of a result (base64) as raw bytes.
fn bytes_of(result: &Value, field: &str) -> Result<Vec<u8>, Response> {
    let s = result
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| error(500, &format!("the reply has no {field}")))?;
    runner::base64_decode(s).map_err(|e| error(500, &e))
}

fn dispatch(ep: Endpoint, head: &Head, body: &[u8], cfg: &Config) -> Result<Response, Response> {
    let method = head.first.as_str();
    let get = |cmd: &str| call(cfg, json!({"cmd": cmd})).map(ok);
    match (ep, method) {
        (Endpoint::Screen, _) => {
            let accept = head.header("accept").map_err(|m| error(400, &m))?;
            let png = query(head, "format") == Some("png")
                || accept.is_some_and(|a| a.to_ascii_lowercase().contains("image/png"));
            if !png {
                return get("screen");
            }
            let scale = match query(head, "scale") {
                Some(_) => query_number(head, "scale")?,
                None => 1,
            };
            let r = call(cfg, json!({"cmd": "screen", "png": true, "scale": scale}))?;
            Ok(Response {
                status: 200,
                content_type: "image/png",
                headers: Vec::new(),
                body: bytes_of(&r, "png")?,
            })
        }
        (Endpoint::Info, _) => get("info"),
        (Endpoint::Cycles, _) => get("stats"),
        (Endpoint::Model, _) => get("model"),
        (Endpoint::Stack, _) => get("stack"),
        (Endpoint::Tree, _) => get("memoryTree"),
        (Endpoint::Flags, _) => get("flags"),
        (Endpoint::Mem, "GET") => {
            let address = query_number(head, "address")?;
            let length = query_number(head, "length")?;
            call(
                cfg,
                json!({"cmd": "peek", "address": address, "length": length}),
            )
            .map(ok)
        }
        (Endpoint::Snapshot, "GET") => {
            let r = call(cfg, json!({"cmd": "saveState"}))?;
            Ok(Response {
                status: 200,
                content_type: "application/octet-stream",
                headers: Vec::new(),
                body: bytes_of(&r, "state")?,
            })
        }
        (Endpoint::Snapshot, _) => {
            if media_type(head)? != "application/octet-stream" {
                return Err(error(415, "send the state as application/octet-stream"));
            }
            let state = saturnus_web::host::base64(body);
            call(cfg, json!({"cmd": "loadState", "state": state})).map(ok)
        }
        _ => {
            if media_type(head)? != "application/json" {
                return Err(error(415, "send a protocol command as application/json"));
            }
            let mut msg: Value =
                serde_json::from_slice(body).map_err(|e| error(400, &format!("bad JSON: {e}")))?;
            let Some(obj) = msg.as_object_mut() else {
                return Err(error(400, "the body must be a JSON object"));
            };
            match obj.get("v") {
                None => {}
                Some(v) if v.as_u64() == Some(runner::PROTOCOL) => {}
                Some(v) => {
                    return Err(error(
                        400,
                        &format!("protocol version {v} not supported (this host speaks 1)"),
                    ));
                }
            }
            // Replies are the HTTP response; an `id` means nothing here.
            obj.remove("id");
            let cmd = obj.get("cmd").and_then(Value::as_str).unwrap_or_default();
            if !ep.commands().contains(&cmd) {
                return Err(error(
                    400,
                    &format!(
                        "{:?} is not a command of {}; expected one of {}",
                        cmd,
                        head.second.split('?').next().unwrap_or_default(),
                        ep.commands().join(", ")
                    ),
                ));
            }
            call(cfg, msg).map(ok)
        }
    }
}

/// Bind the control API on `addr` (always 127.0.0.1).
pub fn bind(addr: SocketAddrV4) -> anyhow::Result<TcpListener> {
    if *addr.ip() != std::net::Ipv4Addr::LOCALHOST {
        anyhow::bail!("the control API listens on 127.0.0.1 only");
    }
    TcpListener::bind(addr).map_err(|e| super::bind_error("control API", &addr.to_string(), e))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use saturnus_drive::runner::Runner;
    use saturnus_web::Emulator;

    use super::super::token::{self, tests::TempDir};
    use super::*;

    struct NoSink;
    impl runner::Sink for NoSink {
        fn event(&self, _: Value) {}
    }

    /// A server on an ephemeral port in front of a 48SX on a ROM of zeros
    /// (it runs nonsense or halts; memory, info and states still work).
    struct Fixture {
        port: u16,
        token: String,
        _dir: TempDir,
    }

    fn fixture() -> Fixture {
        let dir = TempDir::new("server");
        let t = token::load_or_create(&dir.0.join("control-token")).unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut r = Runner::for_host(NoSink, "http");
            let emu = Emulator::new_inner("48sx", &vec![0u8; 256 * 1024]).unwrap();
            r.start(emu, "zeros");
            r.run(&rx);
        });
        let l = bind(SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let bearer = t.bearer();
        spawn(l, Config { port, token: t, tx }).unwrap();
        Fixture {
            port,
            token: bearer[7..].to_string(),
            _dir: dir,
        }
    }

    /// Send raw bytes, return the status and the body; status 0 when the
    /// server closed without a response.
    fn raw(port: u16, request: &[u8]) -> (u16, Vec<u8>, String) {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        // A refused connection may be closed before the request is sent.
        let _ = s.write_all(request);
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        let Some(end) = out.windows(4).position(|w| w == b"\r\n\r\n") else {
            return (0, Vec::new(), String::new());
        };
        let head = String::from_utf8_lossy(&out[..end]).to_string();
        let status = head[9..12].parse().unwrap();
        (status, out[end + 4..].to_vec(), head)
    }

    impl Fixture {
        fn req(
            &self,
            method: &str,
            path: &str,
            extra: &str,
            body: &[u8],
        ) -> (u16, Vec<u8>, String) {
            let mut r = format!(
                "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\n{extra}Content-Length: {}\r\n\r\n",
                self.port,
                self.token,
                body.len()
            )
            .into_bytes();
            r.extend_from_slice(body);
            raw(self.port, &r)
        }

        fn json(&self, method: &str, path: &str, body: &Value) -> (u16, Value) {
            let (s, b, _) = self.req(
                method,
                path,
                "Content-Type: application/json\r\n",
                body.to_string().as_bytes(),
            );
            (s, serde_json::from_slice(&b).unwrap())
        }
    }

    #[test]
    fn info_mem_and_snapshot_round_trip() {
        let f = fixture();
        let (s, b, head) = f.req("GET", "/v1/info", "", b"");
        assert_eq!(s, 200, "{}", String::from_utf8_lossy(&b));
        assert!(
            !head.to_ascii_lowercase().contains("access-control"),
            "{head}"
        );
        let v: Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["result"]["model"], "48sx");
        assert_eq!(v["result"]["host"], "http");

        // A zero ROM configures no RAM: the write goes nowhere, but both
        // directions are served.
        let (s, v) = f.json(
            "POST",
            "/v1/mem",
            &json!({"cmd": "poke", "address": 0x80000, "nibbles": "A5"}),
        );
        assert_eq!(s, 200, "{v}");
        let (s, b, _) = f.req("GET", "/v1/mem?address=0x80000&length=2", "", b"");
        assert_eq!(s, 200);
        let v: Value = serde_json::from_slice(&b).unwrap();
        assert_eq!(v["result"]["nibbles"].as_str().unwrap().len(), 2);

        let (s, state, head) = f.req("GET", "/v1/snapshot", "", b"");
        assert_eq!(s, 200);
        assert!(head.contains("application/octet-stream"), "{head}");
        // The 48SX state holds its 32 KiB RAM; the ROM is bound by hash.
        assert!(state.len() > 32 * 1024, "{}", state.len());
        let (s, b, _) = f.req(
            "PUT",
            "/v1/snapshot",
            "Content-Type: application/octet-stream\r\n",
            &state,
        );
        assert_eq!(s, 200, "{}", String::from_utf8_lossy(&b));
        let (s, b, _) = f.req(
            "PUT",
            "/v1/snapshot",
            "Content-Type: application/octet-stream\r\n",
            &state[..100],
        );
        assert_eq!(s, 422, "{}", String::from_utf8_lossy(&b));

        let (s, png, head) = f.req("GET", "/v1/screen", "Accept: image/png\r\n", b"");
        assert_eq!(s, 200);
        assert!(head.contains("image/png"));
        assert_eq!(&png[1..4], b"PNG");
        let (s, v) = f.json("GET", "/v1/screen", &json!({}));
        assert_eq!(s, 200);
        assert_eq!(v["result"]["rows"].as_array().unwrap().len(), 64);
        let (s, _, _) = f.req("GET", "/v1/cycles", "", b"");
        assert_eq!(s, 200);
        let (s, v) = f.json("GET", "/v1/model", &json!({}));
        assert_eq!((s, v["result"]["width"].as_u64()), (200, Some(131)));
    }

    #[test]
    fn missing_or_wrong_tokens_get_401_without_detail() {
        let f = fixture();
        let host = format!("Host: 127.0.0.1:{}\r\n", f.port);
        let (s, b, head) = raw(
            f.port,
            format!("GET /v1/info HTTP/1.1\r\n{host}\r\n").as_bytes(),
        );
        assert_eq!(s, 401);
        assert_eq!(b, b"{}");
        assert!(head.contains("WWW-Authenticate: Bearer"));
        let wrong = "0".repeat(64);
        for auth in [
            format!("Bearer {wrong}"),
            format!("Basic {}", f.token),
            "Bearer".to_string(),
            format!("Bearer {}", &f.token[..63]),
            format!("Bearer {}0", f.token),
        ] {
            let (s, b, _) = raw(
                f.port,
                format!("GET /v1/info HTTP/1.1\r\n{host}Authorization: {auth}\r\n\r\n").as_bytes(),
            );
            assert_eq!(s, 401, "{auth}");
            assert!(!String::from_utf8_lossy(&b).contains(&f.token));
        }
        // Even an unknown path does not tell more than 401.
        let (s, _, _) = raw(
            f.port,
            format!("GET /v1/nope HTTP/1.1\r\n{host}\r\n").as_bytes(),
        );
        assert_eq!(s, 401);
    }

    #[test]
    fn foreign_hosts_and_origins_are_refused_with_a_valid_token() {
        let f = fixture();
        let auth = format!("Authorization: Bearer {}\r\n", f.token);
        let host = format!("Host: 127.0.0.1:{}\r\n", f.port);
        for bad_host in [
            "Host: evil.example\r\n".to_string(),
            format!("Host: evil.example:{}\r\n", f.port),
            "Host: 127.0.0.1\r\n".to_string(),
            format!("Host: 127.0.0.1:{}\r\n", f.port.wrapping_add(1)),
            format!("Host: 0.0.0.0:{}\r\n", f.port),
            String::new(),
        ] {
            let (s, _, _) = raw(
                f.port,
                format!("GET /v1/info HTTP/1.1\r\n{bad_host}{auth}\r\n").as_bytes(),
            );
            assert_eq!(s, 421, "{bad_host:?}");
        }
        let (s, _, _) = raw(
            f.port,
            format!(
                "GET /v1/info HTTP/1.1\r\nHost: localhost:{}\r\n{auth}\r\n",
                f.port
            )
            .as_bytes(),
        );
        assert_eq!(s, 200);
        for origin in [
            "http://evil.example".to_string(),
            "null".to_string(),
            format!("http://127.0.0.1:{}.evil.example", f.port),
            format!("https://127.0.0.1:{}", f.port),
            "http://127.0.0.1".to_string(),
            "http://localhost:1".to_string(),
        ] {
            let (s, _, head) = raw(
                f.port,
                format!("POST /v1/keys HTTP/1.1\r\n{host}{auth}Origin: {origin}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}").as_bytes(),
            );
            assert_eq!(s, 403, "{origin}");
            assert!(!head.to_ascii_lowercase().contains("access-control"));
        }
        let (s, _, _) = raw(
            f.port,
            format!(
                "GET /v1/info HTTP/1.1\r\n{host}{auth}Origin: http://127.0.0.1:{}\r\n\r\n",
                f.port
            )
            .as_bytes(),
        );
        assert_eq!(s, 200, "the API's own origin");
    }

    #[test]
    fn options_and_wrong_methods_are_refused() {
        let f = fixture();
        let (s, _, head) = raw(
            f.port,
            format!(
                "OPTIONS /v1/keys HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:{}\r\nAccess-Control-Request-Method: POST\r\n\r\n",
                f.port, f.port
            )
            .as_bytes(),
        );
        assert_eq!(s, 405);
        assert!(
            !head.to_ascii_lowercase().contains("access-control"),
            "{head}"
        );
        for (method, path) in [
            ("GET", "/v1/keys"),
            ("GET", "/v1/type"),
            ("POST", "/v1/info"),
            ("POST", "/v1/screen"),
            ("DELETE", "/v1/snapshot"),
        ] {
            let (s, _, head) = f.req(method, path, "", b"");
            assert_eq!(s, 405, "{method} {path}");
            assert!(head.contains("Allow: "), "{head}");
        }
        let (s, _, _) = f.req("GET", "/v2/info", "", b"");
        assert_eq!(s, 404);
        // A command of another endpoint, or none, is refused.
        let (s, v) = f.json(
            "POST",
            "/v1/keys",
            &json!({"cmd": "poke", "address": 0, "nibbles": "0"}),
        );
        assert_eq!(s, 400, "{v}");
        let (s, _) = f.json("POST", "/v1/keys", &json!({"cmd": "boot", "model": "48sx"}));
        assert_eq!(s, 400);
        let (s, _, _) = f.req("POST", "/v1/keys", "Content-Type: text/plain\r\n", b"{}");
        assert_eq!(s, 415);
    }

    #[test]
    fn oversized_bodies_and_heads_are_refused() {
        let f = fixture();
        // Refused from the Content-Length alone, before any body is read.
        let (s, _, _) = raw(
            f.port,
            format!(
                "POST /v1/keys HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                f.port,
                f.token,
                MAX_JSON_BODY + 1
            )
            .as_bytes(),
        );
        assert_eq!(s, 413);
        let (s, _, _) = raw(
            f.port,
            format!(
                "PUT /v1/snapshot HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
                f.port,
                f.token,
                MAX_SNAPSHOT_BODY + 1
            )
            .as_bytes(),
        );
        assert_eq!(s, 413);
        let (s, _, _) = raw(
            f.port,
            format!(
                "POST /v1/keys HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nTransfer-Encoding: chunked\r\nAuthorization: Bearer {}\r\n\r\n",
                f.port, f.token
            )
            .as_bytes(),
        );
        assert_eq!(s, 400);
        let mut big = b"GET /v1/info HTTP/1.1\r\nX: ".to_vec();
        big.extend(std::iter::repeat_n(b'a', http::MAX_HEAD + 10));
        big.extend_from_slice(b"\r\n\r\n");
        let (s, _, _) = raw(f.port, &big);
        assert_eq!(s, 431);
    }

    #[test]
    fn memory_access_is_bounded() {
        let f = fixture();
        let max = runner::MAX_MEM_NIBBLES;
        for q in [
            "address=0x100000&length=1".to_string(),
            "address=0xFFFFF&length=2".to_string(),
            format!("address=0&length={}", max + 1),
            "address=0&length=0".to_string(),
            "address=-1&length=1".to_string(),
        ] {
            let (s, b, _) = f.req("GET", &format!("/v1/mem?{q}"), "", b"");
            assert!(
                s == 422 || s == 400,
                "{q}: {s} {}",
                String::from_utf8_lossy(&b)
            );
        }
        let (s, b, _) = f.req("GET", "/v1/mem?address=0xFFFFF&length=1", "", b"");
        assert_eq!(s, 200, "{}", String::from_utf8_lossy(&b));
        let (s, v) = f.json(
            "POST",
            "/v1/mem",
            &json!({"cmd": "poke", "address": 0xFFFFF, "nibbles": "12"}),
        );
        assert_eq!(s, 422, "{v}");
        let (s, v) = f.json(
            "POST",
            "/v1/mem",
            &json!({"cmd": "poke", "address": 0, "nibbles": "1".repeat(max + 1)}),
        );
        assert_eq!(s, 422, "{v}");
        let (s, v) = f.json(
            "POST",
            "/v1/mem",
            &json!({"cmd": "poke", "address": 0, "nibbles": "xyz"}),
        );
        assert_eq!(s, 422, "{v}");
        // No file path is ever taken.
        let (s, v) = f.json(
            "POST",
            "/v1/mem",
            &json!({"cmd": "poke", "address": 0, "nibbles": "0", "path": "/etc/hosts"}),
        );
        assert_eq!(s, 422, "{v}");
        assert!(v["error"].as_str().unwrap().contains("not accepted"));
    }

    /// A client that sends nothing, or half a body, ties up only its own
    /// connection: others are served meanwhile, and it is dropped after
    /// the timeout. Too many connections get 503 at once.
    #[test]
    fn stalled_clients_do_not_block_others() {
        let f = fixture();
        let mut stalled = Vec::new();
        for _ in 0..MAX_CONNECTIONS - 1 {
            stalled.push(TcpStream::connect(("127.0.0.1", f.port)).unwrap());
        }
        std::thread::sleep(Duration::from_millis(100));
        let t = Instant::now();
        let (s, _, _) = f.req("GET", "/v1/info", "", b"");
        assert_eq!(s, 200);
        assert!(t.elapsed() < Duration::from_secs(2));
        // All slots taken now: one more is refused at once.
        let mut more = vec![TcpStream::connect(("127.0.0.1", f.port)).unwrap()];
        std::thread::sleep(Duration::from_millis(100));
        let (s, _, _) = f.req("GET", "/v1/info", "", b"");
        // 503, or (some platforms) a reset before the client read it.
        assert!(s == 503 || s == 0, "{s}");
        more.clear();
        // The stalled ones time out on their head.
        let mut first = stalled.remove(0);
        first.set_read_timeout(Some(HEAD_TIMEOUT * 2)).unwrap();
        let mut out = Vec::new();
        let _ = first.read_to_end(&mut out);
        assert!(String::from_utf8_lossy(&out).starts_with("HTTP/1.1 408"));
    }
}
