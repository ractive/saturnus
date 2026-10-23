//! What every request to a loopback server passes before its own handler
//! sees it, and the connection budgets (kb: docs/control-api-security).
//!
//! One thread accepts; each connection gets a thread of its own (at most
//! [`MAX_THREADS`] alive). Until its head has passed the checks a
//! connection is *pending*: it has [`HEAD_TIMEOUT`] to send the head, and
//! when [`MAX_PENDING`] are pending the oldest is dropped for a new one, so
//! idle connections without the token cannot lock the token holder out.
//! An authenticated request then takes one of the server's slots
//! ([`Gate::admit`], 503 when none is free) until its response is ready.
//! One request per connection; after the response the server drains a
//! bounded amount for a bounded time before closing ([`linger`]).
//!
//! The checks, in this order: `Host` names 127.0.0.1 or localhost with
//! the bound port (421), an `Origin` is the server's own (403), no
//! `OPTIONS` (405, so no CORS preflight ever succeeds), and the bearer
//! token, compared in constant time ([`bearer_matches`], 401 with no
//! detail). No response ever carries a CORS header.

use std::collections::VecDeque;
use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::http::{Head, Response};
use super::token::Token;

/// Most connections still sending their head; a new one drops the oldest.
pub const MAX_PENDING: usize = 16;
/// Most connection threads alive (including closing ones); a connection
/// over this is closed at once.
pub const MAX_THREADS: usize = 64;
/// Wall time a client has to send its request head: local clients send
/// it at once, and until then the connection is unauthenticated.
pub const HEAD_TIMEOUT: Duration = Duration::from_secs(2);
/// Wall time a client has to send its body.
pub const BODY_TIMEOUT: Duration = Duration::from_secs(20);
/// Wall time a client has to take the response.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// How often a stoppable server's accept loop looks at its stop flag.
const STOP_POLL: Duration = Duration::from_millis(25);
/// Most a closing connection reads and drops after its response, and for
/// how long: closing a socket with unread bytes (a refused request's body)
/// resets it, and the client may lose the response.
const LINGER_BYTES: usize = 1024 * 1024;
const LINGER_TIME: Duration = Duration::from_secs(1);

/// The connection budgets of one server.
#[derive(Debug)]
pub struct Gate {
    /// Connections still sending their head, oldest first, with a handle
    /// to drop them.
    pending: Mutex<VecDeque<(u64, TcpStream)>>,
    next: AtomicU64,
    /// Authenticated requests in progress.
    served: AtomicUsize,
    /// Most authenticated requests in progress.
    max_served: usize,
    /// Connection threads alive.
    threads: AtomicUsize,
}

/// Decrements a counter on drop.
#[derive(Debug)]
pub struct Count<'a>(&'a AtomicUsize);

impl Drop for Count<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A connection's place among the pending ones, left on drop: a handler
/// drops it once the head has passed its checks.
#[derive(Debug)]
pub struct Pending<'a>(&'a Gate, u64);

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if let Ok(mut p) = self.0.pending.lock() {
            p.retain(|(id, _)| *id != self.1);
        }
    }
}

impl Gate {
    /// A gate that serves at most `max_served` authenticated requests at
    /// once.
    pub fn new(max_served: usize) -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
            next: AtomicU64::new(0),
            served: AtomicUsize::new(0),
            max_served,
            threads: AtomicUsize::new(0),
        }
    }

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
    pub fn admit(&self) -> Option<Count<'_>> {
        if self.served.fetch_add(1, Ordering::SeqCst) >= self.max_served {
            self.served.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Count(&self.served))
    }
}

/// Serve `listener` on a thread called `name`: each connection on a
/// thread of its own runs `handle` (the stream, the gate, its pending
/// place), whose response, if any, is written before the connection is
/// closed. With `stop`, the server stops accepting and lets the port go
/// within [`STOP_POLL`] once the flag is set; without, it serves as long
/// as the process lives. The join handle of the accepting thread.
pub fn spawn<H>(
    listener: TcpListener,
    name: &str,
    gate: Gate,
    stop: Option<Arc<AtomicBool>>,
    handle: H,
) -> std::io::Result<std::thread::JoinHandle<()>>
where
    H: Fn(&mut TcpStream, &Gate, Pending<'_>) -> Option<Response> + Send + Sync + 'static,
{
    if stop.is_some() {
        listener.set_nonblocking(true)?;
    }
    let conn_name = format!("{name}-conn");
    let gate = Arc::new(gate);
    let handle = Arc::new(handle);
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || accept(&listener, &conn_name, &gate, stop.as_deref(), &handle))
}

fn accept<H>(
    listener: &TcpListener,
    name: &str,
    gate: &Arc<Gate>,
    stop: Option<&AtomicBool>,
    handle: &Arc<H>,
) where
    H: Fn(&mut TcpStream, &Gate, Pending<'_>) -> Option<Response> + Send + Sync + 'static,
{
    loop {
        if stop.is_some_and(|s| s.load(Ordering::SeqCst)) {
            return;
        }
        let stream = match listener.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(STOP_POLL);
                continue;
            }
            Err(_) => continue,
        };
        // An accepted socket may inherit the listener's non-blocking mode.
        if stop.is_some() && stream.set_nonblocking(false).is_err() {
            continue;
        }
        if gate.threads.fetch_add(1, Ordering::SeqCst) >= MAX_THREADS {
            gate.threads.fetch_sub(1, Ordering::SeqCst);
            drop(stream);
            continue;
        }
        let id = gate.arrive(&stream);
        let (g, h) = (Arc::clone(gate), Arc::clone(handle));
        let spawned = std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let _thread = Count(&g.threads);
                serve(stream, &g, id, &*h);
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

fn serve<H>(mut stream: TcpStream, gate: &Gate, id: u64, handle: &H)
where
    H: Fn(&mut TcpStream, &Gate, Pending<'_>) -> Option<Response>,
{
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let pending = Pending(gate, id);
    if let Some(resp) = handle(&mut stream, gate, pending) {
        let _ = resp.write_to(&mut stream);
    }
    linger(&mut stream);
}

/// Finish the response (FIN), then drain what the client still sends, a
/// bounded amount for a bounded time, before closing.
pub fn linger(stream: &mut TcpStream) {
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

/// A request refused by a check: the status and what to say. Each server
/// puts the words in its own body shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// 400, 421, 403 or 405.
    pub status: u16,
    /// What was wrong; never anything about the token.
    pub message: String,
}

/// The `Host` and `Origin` checks for a server bound to `port`: `Host`
/// must be `127.0.0.1:PORT` or `localhost:PORT` (421; DNS rebinding sends
/// another name), an `Origin`, if any, `http://` and one of those (403;
/// cross-site browser requests always carry one). A header given twice
/// is 400.
pub fn host_and_origin(head: &Head, port: u16) -> Result<(), Refusal> {
    let bad = |message: String| Refusal {
        status: 400,
        message,
    };
    let host = head.header("host").map_err(bad)?;
    let ours = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    if !host.is_some_and(|h| ours.iter().any(|o| o.eq_ignore_ascii_case(h))) {
        return Err(Refusal {
            status: 421,
            message: format!("Host must be {} or {}", ours[0], ours[1]),
        });
    }
    if let Some(origin) = head.header("origin").map_err(bad)? {
        let own = ours
            .iter()
            .any(|o| origin.eq_ignore_ascii_case(&format!("http://{o}")));
        if !own {
            return Err(Refusal {
                status: 403,
                message: "cross-origin requests are refused".into(),
            });
        }
    }
    Ok(())
}

/// Whether the request carries `Authorization: Bearer <token>` with this
/// token (once; compared in constant time).
pub fn bearer_matches(head: &Head, token: &Token) -> bool {
    let presented = head.header("authorization").ok().flatten().and_then(|a| {
        let (scheme, cred) = a.split_once(' ')?;
        scheme.eq_ignore_ascii_case("bearer").then_some(cred.trim())
    });
    presented.is_some_and(|p| token.matches(p.as_bytes()))
}

/// The answer to a missing or wrong token: 401 with the body `{}`, a
/// `WWW-Authenticate: Bearer` and nothing else.
pub fn unauthorized() -> Response {
    Response {
        status: 401,
        content_type: "application/json",
        headers: vec![("WWW-Authenticate", "Bearer".to_string())],
        body: b"{}".to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::http::parse_head;
    use super::super::token::testing::token_of;
    use super::*;

    fn head(lines: &str) -> Head {
        parse_head(format!("GET /x HTTP/1.1\r\n{lines}").trim_end().as_bytes()).unwrap()
    }

    #[test]
    fn hosts_origins_and_tokens() {
        assert!(host_and_origin(&head("Host: 127.0.0.1:4842"), 4842).is_ok());
        assert!(host_and_origin(&head("Host: LOCALHOST:4842"), 4842).is_ok());
        for bad in ["Host: evil:4842", "Host: 127.0.0.1:4843", "X: 1"] {
            assert_eq!(host_and_origin(&head(bad), 4842).unwrap_err().status, 421);
        }
        let two = head("Host: 127.0.0.1:4842\r\nHost: 127.0.0.1:4842");
        assert_eq!(host_and_origin(&two, 4842).unwrap_err().status, 400);
        let own = head("Host: 127.0.0.1:4842\r\nOrigin: http://localhost:4842");
        assert!(host_and_origin(&own, 4842).is_ok());
        let foreign = head("Host: 127.0.0.1:4842\r\nOrigin: https://127.0.0.1:4842");
        assert_eq!(host_and_origin(&foreign, 4842).unwrap_err().status, 403);

        let t = token_of(&"ab".repeat(32));
        assert!(bearer_matches(
            &head(&format!("Authorization: Bearer {}", "ab".repeat(32))),
            &t
        ));
        assert!(bearer_matches(
            &head(&format!("Authorization: bearer  {}", "ab".repeat(32))),
            &t
        ));
        assert!(!bearer_matches(
            &head(&format!("Authorization: Basic {}", "ab".repeat(32))),
            &t
        ));
        assert!(!bearer_matches(&head("Authorization: Bearer ab"), &t));
        assert!(!bearer_matches(&head(""), &t));
        let r = unauthorized();
        assert_eq!((r.status, r.body.as_slice()), (401, b"{}".as_slice()));
    }

    /// A stoppable server stops accepting once told to stop.
    #[test]
    fn a_stopped_server_stops_accepting() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let joined = spawn(
            l,
            "test-guard",
            Gate::new(1),
            Some(Arc::clone(&stop)),
            |_, _, _| {
                Some(Response {
                    status: 200,
                    content_type: "text/plain",
                    headers: Vec::new(),
                    body: b"ok".to_vec(),
                })
            },
        )
        .unwrap();
        let mut s = TcpStream::connect(addr).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 200 OK"), "{out}");
        stop.store(true, Ordering::SeqCst);
        joined.join().unwrap();
        // Nobody listens any more: a new connection is refused.
        assert!(
            TcpStream::connect(addr).is_err(),
            "the server still accepts"
        );
    }
}
