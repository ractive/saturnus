//! The control API's HTTP server: one thread accepts on 127.0.0.1, each
//! connection gets a thread of its own, and a handler talks to the machine
//! thread only through the protocol's command channel, so a slow or
//! stalled client never holds the machine or the serial bridge.
//!
//! Connections are budgeted in two stages. Until its head has passed the
//! checks a connection is *pending*: it has [`HEAD_TIMEOUT`] to send the
//! head, and when [`MAX_PENDING`] are pending the oldest is dropped for a
//! new one, so idle connections without the token cannot lock the token
//! holder out. An authenticated request then takes one of
//! [`MAX_CONNECTIONS`] slots (503 when none is free) until its response
//! is written. Commands wait in a bounded queue ([`QUEUE_DEPTH`], 503 when
//! full); a command whose caller timed out (504) or left is withdrawn and
//! never runs (`runner::Ticket`).
//!
//! Every request passes, in this order: a bounded head read (size and
//! time), the `Host` check (421), the `Origin` check (403), `OPTIONS`
//! refused (405), `GET /v1/hello` answered without the token (the
//! server's proof that it holds the token, which `saturnus ctl` checks
//! before it sends the token), the bearer token (401, no detail), the
//! route (404), the
//! method (405: `GET` never changes anything), the body cap (413) and a
//! bounded body read (408). No CORS header is ever sent. The server never
//! takes a file path: snapshots travel as bytes.

use std::collections::VecDeque;
use std::net::{SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, TrySendError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use saturnus_drive::runner::{self, MAX_STATE_FILE, Request, Ticket};
use serde_json::{Value, json};

use super::http::{self, Head, ReadError, Response};
use super::token::Token;

/// Most authenticated requests served at once; more are answered 503.
pub const MAX_CONNECTIONS: usize = 8;
/// Most connections still sending their head; a new one drops the oldest.
pub const MAX_PENDING: usize = 16;
/// Most connection threads alive (including closing ones); a connection
/// over this is closed at once.
pub const MAX_THREADS: usize = 64;
/// Commands waiting for the machine thread; more are answered 503.
pub const QUEUE_DEPTH: usize = 8;
/// Wall time a client has to send its request head: local clients send
/// it at once, and until then the connection is unauthenticated.
pub const HEAD_TIMEOUT: Duration = Duration::from_secs(2);
/// Wall time a client has to send its body.
pub const BODY_TIMEOUT: Duration = Duration::from_secs(20);
/// Wall time a client has to take the response.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// Longest a handler waits for the machine thread: a key script may run
/// [`runner::SCRIPT_WALL_LIMIT`], and commands queue behind each other.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(90);
/// How often a waiting handler looks whether its client is still there.
const CLIENT_CHECK: Duration = Duration::from_millis(200);
/// Longest a handler waits for a started command after withdrawing it (a
/// key script stops within one 50 ms slice; other commands are short).
const ABORT_WAIT: Duration = Duration::from_secs(10);
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
    /// The machine thread's command queue, bounded ([`QUEUE_DEPTH`]).
    pub tx: SyncSender<Request>,
    /// Longest a request waits for the machine ([`REPLY_TIMEOUT`]).
    pub reply_timeout: Duration,
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
    Cmdline,
    Object,
    /// Token-free: the proof for `?nonce=` (64 hex digits).
    Hello,
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
            "cmdline" => Self::Cmdline,
            "object" => Self::Object,
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
            Self::Type => &["typeText", "insert", "run", "replace"],
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

/// The connection budgets.
#[derive(Debug, Default)]
struct Gate {
    /// Connections still sending their head, oldest first, with a handle
    /// to drop them.
    pending: Mutex<VecDeque<(u64, TcpStream)>>,
    next: AtomicU64,
    /// Authenticated requests in progress.
    served: AtomicUsize,
    /// Connection threads alive.
    threads: AtomicUsize,
}

/// Decrements a counter on drop.
struct Count<'a>(&'a AtomicUsize);

impl Drop for Count<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A connection's place among the pending ones, left on drop.
struct Pending<'a>(&'a Gate, u64);

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if let Ok(mut p) = self.0.pending.lock() {
            p.retain(|(id, _)| *id != self.1);
        }
    }
}

impl Gate {
    /// Register a new connection as pending, dropping the oldest pending
    /// one when [`MAX_PENDING`] are; its id.
    fn arrive(&self, stream: &TcpStream) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut p) = self.pending.lock() {
            while p.len() >= MAX_PENDING {
                if let Some((_, old)) = p.pop_front() {
                    let _ = old.shutdown(std::net::Shutdown::Both);
                }
            }
            if let Ok(handle) = stream.try_clone() {
                p.push_back((id, handle));
            }
        }
        id
    }

    /// An authenticated request takes a slot, if one is free.
    fn admit(&self) -> Option<Count<'_>> {
        if self.served.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            self.served.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Count(&self.served))
    }
}

fn accept(listener: &TcpListener, cfg: &Arc<Config>) {
    let gate = Arc::new(Gate::default());
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if gate.threads.fetch_add(1, Ordering::SeqCst) >= MAX_THREADS {
            gate.threads.fetch_sub(1, Ordering::SeqCst);
            drop(stream);
            continue;
        }
        let id = gate.arrive(&stream);
        let (cfg, g) = (Arc::clone(cfg), Arc::clone(&gate));
        let spawned = std::thread::Builder::new()
            .name("saturnus-control-conn".into())
            .spawn(move || {
                let _thread = Count(&g.threads);
                serve(stream, &cfg, &g, id);
            });
        if spawned.is_err() {
            // Out of threads: the connection was dropped with the closure.
            gate.threads.fetch_sub(1, Ordering::SeqCst);
            if let Ok(mut p) = gate.pending.lock() {
                p.retain(|(i, _)| *i != id);
            }
        }
    }
}

fn serve(mut stream: TcpStream, cfg: &Config, gate: &Gate, id: u64) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let pending = Pending(gate, id);
    let resp = respond(&mut stream, cfg, gate, pending);
    if let Some(resp) = resp {
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
    let path = head.second.split('?').next().unwrap_or_default();
    if path == "/v1/hello" {
        return if method == "GET" {
            Ok(Endpoint::Hello)
        } else {
            Err(not_allowed(
                &["GET"],
                &format!("{method} is not allowed on {path}"),
            ))
        };
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
/// The connection stops being pending once its head has been checked, and
/// an authenticated request holds a slot until its response is ready.
fn respond(
    stream: &mut TcpStream,
    cfg: &Config,
    gate: &Gate,
    pending: Pending<'_>,
) -> Option<Response> {
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
    let checked = check(&head, cfg);
    drop(pending);
    let ep = match checked {
        Ok(Endpoint::Hello) => return Some(hello(&head, cfg)),
        Ok(ep) => ep,
        Err(r) => return Some(r),
    };
    let Some(_slot) = gate.admit() else {
        return Some(error(503, "too many requests in progress; try again"));
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
    Some(dispatch(ep, &head, &body, cfg, stream).unwrap_or_else(|r| r))
}

/// `GET /v1/hello?nonce=N`: the token's proof for `N` and the bound port.
/// It needs no slot and no machine; it tells nothing about the token.
fn hello(head: &Head, cfg: &Config) -> Response {
    match query(head, "nonce") {
        Some(n) if n.len() == 64 && n.bytes().all(|b| b.is_ascii_hexdigit()) => {
            ok(json!({"proof": cfg.token.proof(n, cfg.port)}))
        }
        _ => error(400, "?nonce= must be 64 hex digits"),
    }
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

/// Whether the client has closed its end (it gave up on the answer).
fn client_gone(stream: &TcpStream) -> bool {
    if stream.set_nonblocking(true).is_err() {
        return false;
    }
    let r = stream.peek(&mut [0u8; 1]);
    let _ = stream.set_nonblocking(false);
    match r {
        Ok(0) => true,
        Ok(_) => false,
        Err(e) => !matches!(
            e.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ),
    }
}

/// Send `msg` to the machine thread and wait for its reply. A command the
/// caller gives up on (after the timeout, or because it left) is
/// withdrawn: if the machine had not taken it, it never runs (504); if it
/// had, a key script or typed text is stopped and the reply says what
/// happened.
fn call(cfg: &Config, stream: &TcpStream, mut msg: Value) -> Result<Value, Response> {
    msg["v"] = json!(runner::PROTOCOL);
    let (reply, answer) = channel();
    let ticket = Ticket::new();
    let req = Request {
        msg,
        file: None,
        reply: Some(reply),
        ticket: Some(Arc::clone(&ticket)),
    };
    match cfg.tx.try_send(req) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            return Err(error(
                503,
                &format!("the machine's queue is full ({QUEUE_DEPTH} commands waiting); try again"),
            ));
        }
        Err(TrySendError::Disconnected(_)) => {
            return Err(error(503, "the machine thread has stopped"));
        }
    }
    let map = |r: Result<Value, String>| r.map_err(|m| error(422, &m));
    let deadline = Instant::now() + cfg.reply_timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match answer.recv_timeout(left.min(CLIENT_CHECK)) {
            Ok(r) => return map(r),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(error(503, "the machine thread has stopped"));
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        if !left.is_zero() && !client_gone(stream) {
            continue;
        }
        if ticket.cancel() {
            return Err(error(
                504,
                "the machine did not take the command in time; it did not run and will not",
            ));
        }
        // It had started: a script stops at its next slice.
        return match answer.recv_timeout(ABORT_WAIT) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(m)) => Err(error(
                504,
                &format!("stopped after the timeout ({m}); what it did before stays done"),
            )),
            Err(_) => Err(error(504, "the machine did not answer in time")),
        };
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
    runner::base64_decode(s).map_err(|e| error(500, &e.to_string()))
}

fn dispatch(
    ep: Endpoint,
    head: &Head,
    body: &[u8],
    cfg: &Config,
    stream: &TcpStream,
) -> Result<Response, Response> {
    let method = head.first.as_str();
    let send = |msg: Value| call(cfg, stream, msg);
    let get = |cmd: &str| send(json!({"cmd": cmd})).map(ok);
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
            let r = send(json!({"cmd": "screen", "png": true, "scale": scale}))?;
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
        (Endpoint::Cmdline, _) => get("commandLine"),
        (Endpoint::Object, _) => {
            let address = query_number(head, "address")?;
            send(json!({"cmd": "objectAt", "address": address})).map(ok)
        }
        (Endpoint::Mem, "GET") => {
            let address = query_number(head, "address")?;
            let length = query_number(head, "length")?;
            send(json!({"cmd": "peek", "address": address, "length": length})).map(ok)
        }
        (Endpoint::Snapshot, "GET") => {
            let r = send(json!({"cmd": "saveState"}))?;
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
            let state = saturnus_host::host::base64(body);
            send(json!({"cmd": "loadState", "state": state})).map(ok)
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
            send(msg).map(ok)
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
    use saturnus_host::Emulator;

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
        let (tx, rx) = std::sync::mpsc::sync_channel(QUEUE_DEPTH);
        std::thread::spawn(move || {
            let mut r = Runner::for_host(NoSink, "http");
            let emu = Emulator::new(saturnus::Model::Hp48sx, &vec![0u8; 256 * 1024]).unwrap();
            r.start(emu, "zeros");
            r.run(&rx);
        });
        serve_with(tx, REPLY_TIMEOUT)
    }

    /// A server in front of `tx`, whoever (if anyone) reads it.
    fn serve_with(tx: SyncSender<Request>, reply_timeout: Duration) -> Fixture {
        let dir = TempDir::new("server");
        let t = token::load_or_create(&dir.0.join("control-token")).unwrap();
        let l = bind(SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let bearer = t.bearer();
        let cfg = Config {
            port,
            token: t,
            tx,
            reply_timeout,
        };
        spawn(l, cfg).unwrap();
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
    fn hello_proves_the_token_without_it() {
        let f = fixture();
        let host = format!("Host: 127.0.0.1:{}\r\n", f.port);
        let nonce = "c".repeat(64);
        let (s, b, _) = raw(
            f.port,
            format!("GET /v1/hello?nonce={nonce} HTTP/1.1\r\n{host}\r\n").as_bytes(),
        );
        assert_eq!(s, 200);
        let v: Value = serde_json::from_slice(&b).unwrap();
        let token = super::super::token::tests::token_of(&f.token);
        assert_eq!(v["result"]["proof"], token.proof(&nonce, f.port));
        assert!(!String::from_utf8_lossy(&b).contains(&f.token));
        for target in ["/v1/hello", "/v1/hello?nonce=xyz"] {
            let (s, _, _) = raw(
                f.port,
                format!("GET {target} HTTP/1.1\r\n{host}\r\n").as_bytes(),
            );
            assert_eq!(s, 400, "{target}");
        }
        let (s, _, _) = raw(
            f.port,
            format!("POST /v1/hello?nonce={nonce} HTTP/1.1\r\n{host}\r\n").as_bytes(),
        );
        assert_eq!(s, 405);
        // The Host check still comes first.
        let (s, _, _) = raw(
            f.port,
            format!("GET /v1/hello?nonce={nonce} HTTP/1.1\r\nHost: evil.example\r\n\r\n")
                .as_bytes(),
        );
        assert_eq!(s, 421);
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
        // objectAt has a route with the runner's bounds.
        let (s, b, _) = f.req("GET", "/v1/object?address=0x100000", "", b"");
        assert_eq!(s, 422, "{}", String::from_utf8_lossy(&b));
        assert!(String::from_utf8_lossy(&b).contains("outside the address space"));
        let (s, _, _) = f.req("GET", "/v1/object", "", b"");
        assert_eq!(s, 400);
        let (s, b, _) = f.req("GET", "/v1/object?address=0x80000", "", b"");
        assert!(s == 200 || s == 422, "{s} {}", String::from_utf8_lossy(&b));
        let (s, _, _) = f.req("POST", "/v1/object?address=0", "", b"");
        assert_eq!(s, 405);
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

    /// Connections that never send a head cannot lock the token holder
    /// out: past MAX_PENDING the oldest is dropped, and a lone one is
    /// answered 408 after HEAD_TIMEOUT. No wall-clock bound: the server
    /// accepts in order, so the idle connections are pending before the
    /// request, and the order of events shows the rest.
    #[test]
    fn idle_unauthenticated_connections_do_not_block_others() {
        let f = fixture();
        let mut idle: Vec<TcpStream> = (0..3 * MAX_PENDING)
            .map(|_| TcpStream::connect(("127.0.0.1", f.port)).unwrap())
            .collect();
        let (s, _, _) = f.req("GET", "/v1/info", "", b"");
        assert_eq!(s, 200);
        // Answered before any idle connection's head timed out: the
        // newest has no answer yet.
        let mut last = idle.pop().unwrap();
        last.set_nonblocking(true).unwrap();
        let r = last.peek(&mut [0u8; 1]);
        assert!(
            r.as_ref()
                .is_err_and(|e| e.kind() == std::io::ErrorKind::WouldBlock),
            "{r:?}"
        );
        last.set_nonblocking(false).unwrap();
        // The oldest were dropped without an answer (a pending one would
        // get its 408 instead).
        let mut first = idle.remove(0);
        first.set_read_timeout(Some(HEAD_TIMEOUT * 3)).unwrap();
        let mut out = Vec::new();
        let r = first.read_to_end(&mut out);
        assert!(r.is_ok() && out.is_empty(), "{r:?} {out:?}");
        // The newest ones time out on their head.
        last.set_read_timeout(Some(HEAD_TIMEOUT * 3)).unwrap();
        let mut out = Vec::new();
        let _ = last.read_to_end(&mut out);
        assert!(String::from_utf8_lossy(&out).starts_with("HTTP/1.1 408"));
    }

    /// A command whose caller gave up (504) never runs, and the queue in
    /// front of the machine is bounded (503 when full).
    #[test]
    fn timed_out_commands_never_run_and_the_queue_is_bounded() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let f = serve_with(tx, Duration::from_millis(300));
        let poke = json!({"cmd": "poke", "address": 0, "nibbles": "1"});
        // Nobody reads the queue: the first command waits there and times out.
        let (s, v) = f.json("POST", "/v1/mem", &poke);
        assert_eq!(s, 504, "{v}");
        assert!(v["error"].as_str().unwrap().contains("did not run"), "{v}");
        // It still fills the one place in the queue.
        let (s, v) = f.json("POST", "/v1/mem", &poke);
        assert_eq!(s, 503, "{v}");
        assert!(
            v["error"].as_str().unwrap().contains("queue is full"),
            "{v}"
        );
        // The machine thread finds it withdrawn: it must not run.
        let req = rx.try_recv().unwrap();
        assert!(!req.ticket.unwrap().start());
    }

    /// A client that leaves while its command waits withdraws it too.
    #[test]
    fn a_client_that_leaves_withdraws_its_command() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let f = serve_with(tx, Duration::from_secs(60));
        let body = json!({"cmd": "keyScript", "script": "on"}).to_string();
        let mut s = TcpStream::connect(("127.0.0.1", f.port)).unwrap();
        write!(
            s,
            "POST /v1/keys HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            f.port,
            f.token,
            body.len()
        )
        .unwrap();
        let req = rx.recv_timeout(Duration::from_secs(30)).unwrap();
        drop(s);
        // The handler notices within its check interval, withdraws, and
        // returns, dropping its share of the ticket: wait for that (a
        // generous 30 s against a loaded machine, CLIENT_CHECK when idle).
        let ticket = req.ticket.unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while Arc::strong_count(&ticket) > 1 {
            assert!(Instant::now() < deadline, "the handler is still waiting");
            std::thread::sleep(CLIENT_CHECK / 4);
        }
        assert!(!ticket.start(), "the command was not withdrawn");
    }

    /// At most MAX_CONNECTIONS authenticated requests are in progress.
    /// The test plays the machine thread: once it holds every slot's
    /// command, one more request is refused; then it answers them.
    #[test]
    fn authenticated_requests_are_capped() {
        let (tx, rx) = std::sync::mpsc::sync_channel(4 * MAX_CONNECTIONS);
        let f = Arc::new(serve_with(tx, REPLY_TIMEOUT));
        let waiting: Vec<_> = (0..MAX_CONNECTIONS)
            .map(|_| {
                let f = Arc::clone(&f);
                std::thread::spawn(move || f.req("GET", "/v1/info", "", b"").0)
            })
            .collect();
        // A queued command holds its slot until it is answered.
        let held: Vec<Request> = (0..MAX_CONNECTIONS)
            .map(|_| rx.recv_timeout(Duration::from_secs(30)).unwrap())
            .collect();
        let (s, b, _) = f.req("GET", "/v1/info", "", b"");
        assert_eq!(s, 503, "{}", String::from_utf8_lossy(&b));
        for req in held {
            assert!(req.ticket.unwrap().start());
            req.reply.unwrap().send(Ok(json!({}))).unwrap();
        }
        for w in waiting {
            assert_eq!(w.join().unwrap(), 200);
        }
    }
}
