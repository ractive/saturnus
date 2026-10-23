//! The desktop app's MCP endpoint (iteration 58): MCP over Streamable
//! HTTP at `http://127.0.0.1:PORT/mcp` (default [`DEFAULT_PORT`]), so AI
//! apps on this computer (Claude Code, Codex, VS Code, Cursor) use the
//! calculators without the internet.
//!
//! A thin endpoint: the tools are the web app's (`web/ai-tools.js`), run in
//! the window by `web/ai-local.js`. This side speaks the transport and
//! JSON-RPC and hands `tools/list` and `tools/call` to the window
//! ([`Window`], in the app `crate::mcp_app`), which answers them, asks the
//! user before a consequential tool and logs the calls.
//!
//! Two eras, told apart by the request body:
//! - The stateless revision ([`MODERN`], 2026-07-28): every request
//!   carries the `_meta` envelope (`io.modelcontextprotocol/protocolVersion`
//!   and `clientCapabilities`) and the `MCP-Protocol-Version`, `Mcp-Method`
//!   and (for `tools/call`) `Mcp-Name` headers, which must agree with the
//!   body. `server/discover`, `tools/list`, `tools/call`,
//!   `subscriptions/listen` (no notifications offered) and `ping`. Checked
//!   as tab-mcp's page server checks them (`tab-mcp-client` `mcp.ts`),
//!   with its relay's allowance for Claude Code's 2025-11-25 header on a
//!   2026-07-28 body.
//! - The legacy handshake ([`LEGACY`]) for clients that still start with
//!   `initialize`: answered with the version asked for when it is one of
//!   those, else the newest; an `Mcp-Session-Id` that only names the client
//!   for the log (never checked: every request stands alone), then
//!   `tools/list`, `tools/call` and `ping`, JSON answers, `GET` and
//!   `DELETE` 405.
//!
//! The control API's rules hold unchanged (kb:
//! docs/control-api-security, `saturnus_drive::loopback::guard`): bound
//! to 127.0.0.1, `Host` and `Origin` checked, `OPTIONS` refused and no
//! CORS header, the per-user bearer token (the control API's
//! `control-token`) compared in constant time on every request, bounded
//! heads and bodies, at most [`MAX_CONNECTIONS`] requests served at once.
//! The token is never logged and never in an answer.

use std::collections::VecDeque;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use saturnus_drive::loopback::guard::{self, BODY_TIMEOUT, Gate, HEAD_TIMEOUT, Pending, Refusal};
use saturnus_drive::loopback::http::{self, Head, ReadError, Response};
use saturnus_drive::loopback::token::Token;
use serde_json::{Map, Value, json};

/// The endpoint's default port: after the control API's 4840 and the
/// serial bridge's 4841; 4848-4852 and 4860-4889 are taken by the satx
/// oracle and the bridges.
pub const DEFAULT_PORT: u16 = 4842;
/// The one path served.
pub const PATH: &str = "/mcp";
/// The stateless revision.
pub const MODERN: &str = "2026-07-28";
/// The revisions of the legacy handshake, oldest first.
pub const LEGACY: [&str; 3] = ["2025-03-26", "2025-06-18", "2025-11-25"];
/// Claude Code sends this `MCP-Protocol-Version` with 2026-07-28 bodies
/// (claude-code #93290); tab-mcp's relay accepts the pair, and so do we.
const CLAUDE_MISMATCHED_HEADER: &str = "2025-11-25";
/// Most authenticated requests served at once; more are answered 503.
pub const MAX_CONNECTIONS: usize = 8;
/// Largest request body: a `store_variable` of a long program fits.
pub const MAX_BODY: usize = 256 * 1024;
/// How often a waiting request looks whether its client is still there.
pub const CLIENT_CHECK: Duration = Duration::from_millis(200);
/// Legacy session ids remembered (for the clients' names), newest last.
const MAX_SESSIONS: usize = 64;

const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
const META_CLIENT_INFO: &str = "io.modelcontextprotocol/clientInfo";
const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";
const META_SUBSCRIPTION: &str = "io.modelcontextprotocol/subscriptionId";
const HEADER_MISMATCH: i64 = -32020;
const UNSUPPORTED_VERSION: i64 = -32022;
/// The error of a call the window could not answer.
const NOT_ANSWERED: i64 = -32001;

/// Why the window did not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// No window is ready (it is loading, reloading or closed).
    NotReady,
    /// The window did not answer within the call's time limit.
    Timeout,
    /// The client left, or the server was turned off.
    Gone,
    /// The window answered with an error.
    Failed(String),
}

impl CallError {
    fn message(&self) -> String {
        match self {
            Self::NotReady => "The saturnus app is not ready: its window is loading or closed. Try again in a moment.".into(),
            Self::Timeout => "The saturnus app did not answer in time.".into(),
            Self::Gone => "The call was cancelled.".into(),
            Self::Failed(m) => m.clone(),
        }
    }
}

/// The window that runs the tools, as the endpoint sees it.
pub trait Window: Send + Sync + 'static {
    /// The server's instructions, while the window is ready.
    fn instructions(&self) -> Option<String>;
    /// Run `method` (`tools/list` or `tools/call`) with its JSON-RPC
    /// `params` for `client`; `gone` says whether the caller left and the
    /// call should be cancelled. The window's result, or why there is none.
    fn call(
        &self,
        method: &str,
        params: Value,
        client: &str,
        gone: &dyn Fn() -> bool,
    ) -> Result<Value, CallError>;
    /// An AI app called `client` made an authenticated request.
    fn seen(&self, _client: &str) {}
}

/// What the endpoint needs: the bound port, the token and the window.
pub struct Config {
    /// The bound port; `Host` and `Origin` must name it.
    pub port: u16,
    /// The bearer token.
    pub token: Token,
    /// Runs the tools.
    pub window: Arc<dyn Window>,
    /// The app's version, for `serverInfo`.
    pub version: String,
    /// Legacy session ids and the clients' names they stand for.
    sessions: Mutex<VecDeque<(String, String)>>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("port", &self.port)
            .field("token", &self.token)
            .finish_non_exhaustive()
    }
}

impl Config {
    /// The endpoint on `port` with `token`, running tools in `window`.
    pub fn new(port: u16, token: Token, window: Arc<dyn Window>, version: &str) -> Self {
        Self {
            port,
            token,
            window,
            version: version.to_string(),
            sessions: Mutex::new(VecDeque::new()),
        }
    }

    fn remember(&self, session: &str, client: &str) {
        if let Ok(mut s) = self.sessions.lock() {
            s.retain(|(id, _)| id != session);
            while s.len() >= MAX_SESSIONS {
                s.pop_front();
            }
            s.push_back((session.to_string(), client.to_string()));
        }
    }

    fn session_client(&self, session: &str) -> Option<String> {
        let s = self.sessions.lock().ok()?;
        s.iter()
            .find(|(id, _)| id == session)
            .map(|(_, c)| c.clone())
    }
}

/// Serve `listener` until `stop` is set; the accepting thread's handle.
pub fn spawn(
    listener: std::net::TcpListener,
    cfg: Arc<Config>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    guard::spawn(
        listener,
        "saturnus-mcp",
        Gate::new(MAX_CONNECTIONS),
        Some(stop),
        move |stream, gate, pending| respond(stream, &cfg, gate, pending),
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

/// A JSON-RPC error in an HTTP response of `status`.
fn rpc_error(status: u16, id: &Value, code: i64, message: &str, data: Option<Value>) -> Response {
    let mut error = json!({"code": code, "message": message});
    if let Some(d) = data {
        error["data"] = d;
    }
    json_response(status, &json!({"jsonrpc": "2.0", "id": id, "error": error}))
}

/// A refusal before the body is read: a JSON-RPC error without an id.
fn refused(status: u16, message: &str) -> Response {
    rpc_error(status, &Value::Null, -32000, message, None)
}

fn not_allowed(message: &str) -> Response {
    let mut r = refused(405, message);
    r.headers.push(("Allow", "POST".to_string()));
    r
}

/// The checks of the head, in order: `Host` and `Origin`, no `OPTIONS`,
/// the token (before anything about the path is said), the path, the
/// method.
fn check(head: &Head, cfg: &Config) -> Result<(), Response> {
    guard::host_and_origin(head, cfg.port)
        .map_err(|Refusal { status, message }| refused(status, &message))?;
    let method = head.first.as_str();
    if method == "OPTIONS" {
        return Err(not_allowed("OPTIONS is refused (no CORS)"));
    }
    if !guard::bearer_matches(head, &cfg.token) {
        // No detail: not which part was wrong, nothing about the token.
        return Err(guard::unauthorized());
    }
    let path = head.second.split('?').next().unwrap_or_default();
    if path != PATH {
        return Err(refused(404, &format!("no such endpoint: MCP is at {PATH}")));
    }
    if method != "POST" {
        return Err(not_allowed(&format!(
            "{method} is not allowed: this endpoint takes POST only (no event stream)"
        )));
    }
    Ok(())
}

/// Handle one request; `None` when the client is gone.
fn respond(
    stream: &mut TcpStream,
    cfg: &Config,
    gate: &Gate,
    pending: Pending<'_>,
) -> Option<Response> {
    let (head, rest) = match http::read_head(stream, Instant::now() + HEAD_TIMEOUT) {
        Ok(h) => h,
        Err(ReadError::Timeout) => return Some(refused(408, "request head not received in time")),
        Err(ReadError::TooLarge) => return Some(refused(431, "request head too large")),
        Err(ReadError::Bad(m)) => return Some(refused(400, &m)),
        Err(ReadError::Closed | ReadError::Io(_)) => return None,
    };
    if !matches!(head.third.as_str(), "HTTP/1.1" | "HTTP/1.0") {
        return Some(refused(400, "HTTP/1.1 expected"));
    }
    let checked = check(&head, cfg);
    drop(pending);
    if let Err(r) = checked {
        return Some(r);
    }
    let Some(_slot) = gate.admit() else {
        return Some(refused(503, "too many requests in progress; try again"));
    };
    let content_type = match head.header("content-type") {
        Ok(ct) => ct.unwrap_or_default(),
        Err(m) => return Some(refused(400, &m)),
    };
    let media = content_type.split(';').next().unwrap_or_default().trim();
    if !media.eq_ignore_ascii_case("application/json") {
        return Some(refused(
            415,
            "Unsupported Media Type: Content-Type must be application/json",
        ));
    }
    let len = match head.content_length() {
        Ok(n) => n,
        Err(m) => return Some(refused(400, &m)),
    };
    if len > MAX_BODY {
        return Some(refused(
            413,
            &format!("body of {len} bytes is over the cap of {MAX_BODY}"),
        ));
    }
    let body = match http::read_body(stream, rest, len, Instant::now() + BODY_TIMEOUT) {
        Ok(b) => b,
        Err(ReadError::Timeout) => return Some(refused(408, "request body not received in time")),
        Err(ReadError::Bad(m)) => return Some(refused(400, &m)),
        Err(_) => return None,
    };
    let peer: &TcpStream = stream;
    Some(handle(&head, &body, cfg, &|| client_gone(peer)))
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

/// Which era a request is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Era {
    Modern,
    Legacy,
}

/// A request that passed the transport's checks.
struct Call<'a> {
    id: Value,
    method: &'a str,
    params: Map<String, Value>,
    era: Era,
}

/// The JSON-RPC part of one request (its body already read and capped).
pub(crate) fn handle(head: &Head, body: &[u8], cfg: &Config, gone: &dyn Fn() -> bool) -> Response {
    let Ok(msg) = serde_json::from_slice::<Value>(body) else {
        return rpc_error(
            400,
            &Value::Null,
            -32700,
            "Parse error: the request body is not valid JSON",
            None,
        );
    };
    let call = match parse(head, &msg) {
        Ok(Some(c)) => c,
        // A notification: nothing to answer.
        Ok(None) => {
            return Response {
                status: 202,
                content_type: "application/json",
                headers: Vec::new(),
                body: Vec::new(),
            };
        }
        Err(r) => return r,
    };
    let client = client_name(head, &call, cfg);
    cfg.window.seen(&client);
    match call.era {
        Era::Modern => modern(&call, cfg, &client, gone),
        Era::Legacy => legacy(&call, cfg, &client, gone),
    }
}

/// The client's name for the log: its `clientInfo`, the name its legacy
/// session stands for, its `User-Agent`'s product, else "AI app".
fn client_name(head: &Head, call: &Call<'_>, cfg: &Config) -> String {
    let info = match call.era {
        Era::Modern => call
            .params
            .get("_meta")
            .and_then(|m| m.get(META_CLIENT_INFO)),
        Era::Legacy => call.params.get("clientInfo"),
    };
    let named = info
        .and_then(|i| i.get("name"))
        .and_then(Value::as_str)
        .map(clean_name)
        .filter(|n| !n.is_empty());
    if let Some(n) = named {
        return n;
    }
    if let Some(n) = head
        .header("mcp-session-id")
        .ok()
        .flatten()
        .and_then(|s| cfg.session_client(s))
    {
        return n;
    }
    head.header("user-agent")
        .ok()
        .flatten()
        .and_then(|ua| ua.split([' ', '/']).next())
        .map(clean_name)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "AI app".to_string())
}

/// A client's self-declared name, shown in the app: printable, at most 64
/// characters.
fn clean_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect::<String>()
        .trim()
        .to_string()
}

/// The transport's checks (tab-mcp's, `mcp.ts` `check`): the JSON-RPC
/// shape, the era, the envelope and the headers. `None` for a
/// notification.
fn parse<'a>(head: &Head, msg: &'a Value) -> Result<Option<Call<'a>>, Response> {
    let invalid = || {
        rpc_error(
            400,
            &Value::Null,
            -32600,
            "Bad Request: the request body is not a valid JSON-RPC message",
            None,
        )
    };
    if msg.is_array() {
        return Err(rpc_error(
            400,
            &Value::Null,
            -32600,
            "Bad Request: JSON-RPC batches are not supported by this endpoint",
            None,
        ));
    }
    let Some(obj) = msg.as_object() else {
        return Err(invalid());
    };
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(invalid());
    }
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return Err(if obj.contains_key("result") || obj.contains_key("error") {
            rpc_error(
                400,
                &Value::Null,
                -32600,
                "Bad Request: JSON-RPC responses cannot be posted to this endpoint",
                None,
            )
        } else {
            invalid()
        });
    };
    let header = |name: &str| {
        head.header(name)
            .map_err(|m| rpc_error(400, &Value::Null, -32600, &m, None))
    };
    let header_version = header("mcp-protocol-version")?;
    let header_method = header("mcp-method")?;
    let params = obj
        .get("params")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let meta = params.get("_meta").and_then(Value::as_object);
    let claim = meta.is_some_and(|m| m.contains_key(META_VERSION));
    let claimed = meta
        .and_then(|m| m.get(META_VERSION))
        .and_then(Value::as_str);
    let header_modern = header_version.is_some_and(|v| v >= MODERN);

    let Some(raw_id) = obj.get("id") else {
        // A notification: nothing to answer, but the headers must agree.
        if let (Some(c), Some(h)) = (claimed, header_version)
            && c != h
            && !(c == MODERN && h == CLAUDE_MISMATCHED_HEADER)
        {
            return Err(mismatch(
                &Value::Null,
                h,
                &format!(
                    "the notification envelope names protocol version {c} but the MCP-Protocol-Version header names {h}"
                ),
            ));
        }
        if (claim || header_modern)
            && let Some(h) = header_method
            && h != method
        {
            return Err(mismatch(
                &Value::Null,
                h,
                &format!(
                    "the notification body names method {method} but the Mcp-Method header names {h}"
                ),
            ));
        }
        return Ok(None);
    };
    if !(raw_id.is_string() || raw_id.is_number()) {
        return Err(invalid());
    }
    let id = raw_id.clone();
    let legacy_initialize = method == "initialize" && claimed.is_none_or(|c| c < MODERN);
    if !claim || legacy_initialize {
        // The legacy era: no envelope.
        if header_modern {
            if method == "initialize" {
                return Err(mismatch(
                    &id,
                    header_version.unwrap_or_default(),
                    "an initialize request (legacy handshake) was sent with a modern MCP-Protocol-Version header",
                ));
            }
            let missing = if meta.is_none() {
                "_meta"
            } else {
                META_VERSION
            };
            return Err(rpc_error(
                400,
                &id,
                -32602,
                &format!(
                    "Invalid params: the MCP-Protocol-Version header names protocol revision {}, but the request is missing the required per-request envelope key(s): {missing}",
                    header_version.unwrap_or_default()
                ),
                Some(json!({"envelope": {"missing": [missing]}})),
            ));
        }
        if let Some(h) = header_version
            && !LEGACY.contains(&h)
        {
            return Err(unsupported(&id, Some(h)));
        }
        return Ok(Some(Call {
            id,
            method,
            params,
            era: Era::Legacy,
        }));
    }
    // The stateless era: the envelope, and headers that agree with it.
    let meta = meta.cloned().unwrap_or_default();
    if let Some((key, problem)) = envelope_issue(&meta) {
        return Err(rpc_error(
            400,
            &id,
            -32602,
            &format!("Invalid _meta envelope for protocol revision {MODERN}: {key}: {problem}"),
            Some(json!({"envelope": {"key": key, "problem": problem}})),
        ));
    }
    let claimed = claimed.unwrap_or_default();
    if let Some(h) = header_version
        && h != claimed
        && !(claimed == MODERN && h == CLAUDE_MISMATCHED_HEADER)
    {
        return Err(mismatch(
            &id,
            h,
            &format!(
                "the body envelope names protocol version {claimed} but the MCP-Protocol-Version header names {h}"
            ),
        ));
    }
    if let Some(h) = header_method
        && h != method
    {
        return Err(mismatch(
            &id,
            h,
            &format!("the body names method {method} but the Mcp-Method header names {h}"),
        ));
    }
    if claimed != MODERN {
        return Err(unsupported(&id, Some(claimed)));
    }
    if header_version.is_none() {
        return Err(mismatch(
            &id,
            "(missing)",
            &format!(
                "the body envelope names protocol version {claimed} but the required MCP-Protocol-Version header is absent"
            ),
        ));
    }
    if header_method.is_none() {
        return Err(mismatch(
            &id,
            "(missing)",
            &format!("the body names method {method} but the required Mcp-Method header is absent"),
        ));
    }
    if method == "tools/call"
        && let Some(name) = params.get("name").and_then(Value::as_str)
    {
        let Some(h) = header("mcp-name")? else {
            return Err(mismatch(
                &id,
                "(missing)",
                &format!(
                    "the body carries params.name=\"{name}\" but the required Mcp-Name header is absent"
                ),
            ));
        };
        let Some(decoded) = decode_header_value(h) else {
            return Err(mismatch(
                &id,
                h,
                "the Mcp-Name header carries an invalid Base64 sentinel value",
            ));
        };
        if decoded != name {
            return Err(mismatch(
                &id,
                h,
                &format!(
                    "the body carries params.name=\"{name}\" but the Mcp-Name header names \"{decoded}\""
                ),
            ));
        }
    }
    Ok(Some(Call {
        id,
        method,
        params,
        era: Era::Modern,
    }))
}

fn mismatch(id: &Value, header: &str, body: &str) -> Response {
    rpc_error(
        400,
        id,
        HEADER_MISMATCH,
        &format!("Bad Request: the request headers and body disagree: {body}"),
        Some(json!({"mismatch": {"header": header, "body": body}})),
    )
}

fn unsupported(id: &Value, requested: Option<&str>) -> Response {
    let supported: Vec<&str> = LEGACY.iter().copied().chain([MODERN]).collect();
    match requested {
        None => rpc_error(
            400,
            id,
            UNSUPPORTED_VERSION,
            "Unsupported protocol version: the request did not name a protocol version",
            Some(json!({"supported": supported})),
        ),
        Some(r) => rpc_error(
            400,
            id,
            UNSUPPORTED_VERSION,
            &format!("Unsupported protocol version: {r}"),
            Some(json!({"supported": supported, "requested": r})),
        ),
    }
}

/// The first problem of a 2026-07-28 `_meta` envelope.
fn envelope_issue(meta: &Map<String, Value>) -> Option<(&'static str, &'static str)> {
    if !meta.contains_key(META_VERSION) {
        return Some((META_VERSION, "missing"));
    }
    if !meta.contains_key(META_CAPABILITIES) {
        return Some((META_CAPABILITIES, "missing"));
    }
    if !meta[META_VERSION].is_string() {
        return Some((META_VERSION, "expected a protocol version string"));
    }
    if !meta[META_CAPABILITIES].is_object() {
        return Some((META_CAPABILITIES, "expected an object"));
    }
    if let Some(info) = meta.get(META_CLIENT_INFO) {
        let ok = info.get("name").is_some_and(Value::is_string)
            && info.get("version").is_some_and(Value::is_string);
        if !ok {
            return Some((META_CLIENT_INFO, "expected {name, version}"));
        }
    }
    None
}

/// The `=?base64?…?=` sentinel of SEP-2243 headers decoded; the value as
/// it is without one; `None` if malformed.
fn decode_header_value(value: &str) -> Option<String> {
    let Some(b64) = value
        .strip_prefix("=?base64?")
        .and_then(|v| v.strip_suffix("?="))
    else {
        return Some(value.to_string());
    };
    let bytes = saturnus_host::host::base64_decode(b64).ok()?;
    String::from_utf8(bytes).ok()
}

fn server_info(cfg: &Config) -> Value {
    json!({"name": "saturnus", "title": "saturnus calculators", "version": cfg.version})
}

/// A result of the stateless era: `resultType` and the server's info added.
fn modern_result(id: &Value, cfg: &Config, mut value: Value) -> Response {
    if let Some(o) = value.as_object_mut() {
        o.insert("resultType".into(), json!("complete"));
        let meta = o.entry("_meta").or_insert_with(|| json!({}));
        if let Some(m) = meta.as_object_mut() {
            m.insert(META_SERVER_INFO.into(), server_info(cfg));
        }
    }
    json_response(200, &json!({"jsonrpc": "2.0", "id": id, "result": value}))
}

/// A call the window did not answer, as a JSON-RPC error (the AI app sees
/// why; HTTP 200, so no client takes it for a lost session).
fn not_answered(id: &Value, e: &CallError) -> Response {
    rpc_error(200, id, NOT_ANSWERED, &e.message(), None)
}

/// `tools/list` or `tools/call` through the window; a `tools/call` that
/// fails is an `isError` result, so the AI app reads why.
fn through_window(
    call: &Call<'_>,
    cfg: &Config,
    client: &str,
    gone: &dyn Fn() -> bool,
) -> Result<Value, CallError> {
    let mut params = call.params.clone();
    params.remove("_meta");
    let r = cfg
        .window
        .call(call.method, Value::Object(params), client, gone);
    match r {
        Err(e) if call.method == "tools/call" && e != CallError::Gone => Ok(json!({
            "content": [{"type": "text", "text": e.message()}],
            "isError": true,
        })),
        r => r,
    }
}

fn modern(call: &Call<'_>, cfg: &Config, client: &str, gone: &dyn Fn() -> bool) -> Response {
    let id = &call.id;
    match call.method {
        "server/discover" => {
            let mut discover = json!({
                "supportedVersions": [MODERN],
                "capabilities": {"tools": {"listChanged": false}},
                "ttlMs": 0,
                "cacheScope": "private",
            });
            discover["instructions"] = json!(instructions(cfg));
            modern_result(id, cfg, discover)
        }
        "ping" => modern_result(id, cfg, json!({})),
        "subscriptions/listen" => {
            // The tool list never changes while the app runs: no
            // notification is offered, so the stream ends at once.
            let ack = json!({
                "jsonrpc": "2.0",
                "method": "notifications/subscriptions/acknowledged",
                "params": {"notifications": {}, "_meta": {META_SUBSCRIPTION: id}},
            });
            let done = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"resultType": "complete", "_meta": {META_SUBSCRIPTION: id, META_SERVER_INFO: server_info(cfg)}},
            });
            Response {
                status: 200,
                content_type: "text/event-stream",
                headers: Vec::new(),
                body: format!("event: message\ndata: {ack}\n\nevent: message\ndata: {done}\n\n")
                    .into_bytes(),
            }
        }
        "tools/list" | "tools/call" => match through_window(call, cfg, client, gone) {
            Ok(mut v) => {
                if call.method == "tools/list"
                    && let Some(o) = v.as_object_mut()
                {
                    o.insert("ttlMs".into(), json!(0));
                    o.insert("cacheScope".into(), json!("private"));
                }
                modern_result(id, cfg, v)
            }
            Err(e) => not_answered(id, &e),
        },
        _ => rpc_error(404, id, -32601, "Method not found", None),
    }
}

/// The server's instructions: the window's, or what to do while it is not
/// ready.
fn instructions(cfg: &Config) -> String {
    cfg.window
        .instructions()
        .unwrap_or_else(|| CallError::NotReady.message())
}

fn legacy(call: &Call<'_>, cfg: &Config, client: &str, gone: &dyn Fn() -> bool) -> Response {
    let id = &call.id;
    let ok =
        |result: Value| json_response(200, &json!({"jsonrpc": "2.0", "id": id, "result": result}));
    match call.method {
        "initialize" => {
            let asked = call.params.get("protocolVersion").and_then(Value::as_str);
            let version = asked
                .filter(|a| LEGACY.contains(a))
                .unwrap_or(LEGACY[LEGACY.len() - 1]);
            let mut r = ok(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": server_info(cfg),
                "instructions": instructions(cfg),
            }));
            // Names the client for the log only; never required.
            if let Ok(session) = saturnus_drive::loopback::token::random_hex() {
                cfg.remember(&session, client);
                r.headers.push(("Mcp-Session-Id", session));
            }
            r
        }
        "ping" => ok(json!({})),
        "tools/list" | "tools/call" => match through_window(call, cfg, client, gone) {
            Ok(v) => ok(v),
            Err(e) => not_answered(id, &e),
        },
        // HTTP 200: a 404 would tell a legacy client its session is gone.
        _ => rpc_error(200, id, -32601, "Method not found", None),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::sync::atomic::{AtomicBool, Ordering};

    use saturnus_drive::loopback::token::{self, testing::TempDir};

    use super::*;

    /// A window that lists one tool and echoes calls; `slow` makes a call
    /// wait until its client leaves.
    #[derive(Default)]
    struct FakeWindow {
        not_ready: AtomicBool,
        slow: AtomicBool,
        cancelled: AtomicBool,
        seen: Mutex<Vec<String>>,
    }

    impl Window for FakeWindow {
        fn instructions(&self) -> Option<String> {
            (!self.not_ready.load(Ordering::SeqCst)).then(|| "Use the calculators.".to_string())
        }

        fn call(
            &self,
            method: &str,
            params: Value,
            client: &str,
            gone: &dyn Fn() -> bool,
        ) -> Result<Value, CallError> {
            if self.not_ready.load(Ordering::SeqCst) {
                return Err(CallError::NotReady);
            }
            if self.slow.load(Ordering::SeqCst) {
                let end = Instant::now() + Duration::from_secs(30);
                while Instant::now() < end {
                    if gone() {
                        self.cancelled.store(true, Ordering::SeqCst);
                        return Err(CallError::Gone);
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                return Err(CallError::Timeout);
            }
            Ok(match method {
                "tools/list" => {
                    json!({"tools": [{"name": "read_display", "description": "d", "inputSchema": {"type": "object"}}]})
                }
                _ => {
                    json!({"content": [{"type": "text", "text": format!("{} for {client}", params["name"])}]})
                }
            })
        }

        fn seen(&self, client: &str) {
            self.seen.lock().unwrap().push(client.to_string());
        }
    }

    struct Fixture {
        port: u16,
        token: String,
        window: Arc<FakeWindow>,
        stop: Arc<AtomicBool>,
        _dir: TempDir,
    }

    fn fixture() -> Fixture {
        let dir = TempDir::new("mcp");
        let t = token::load_or_create(&dir.0.join("control-token")).unwrap();
        let bearer = t.bearer();
        let l = saturnus_drive::loopback::bind(
            "MCP endpoint",
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            "x",
        )
        .unwrap();
        let port = l.local_addr().unwrap().port();
        let window = Arc::new(FakeWindow::default());
        let stop = Arc::new(AtomicBool::new(false));
        let w: Arc<dyn Window> = window.clone();
        spawn(
            l,
            Arc::new(Config::new(port, t, w, "0.0.0")),
            Arc::clone(&stop),
        )
        .unwrap();
        Fixture {
            port,
            token: bearer[7..].to_string(),
            window,
            stop,
            _dir: dir,
        }
    }

    /// Send raw bytes; the status, the head and the body (status 0 when
    /// the server closed without a response).
    fn raw(port: u16, request: &[u8]) -> (u16, String, Vec<u8>) {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let _ = s.write_all(request);
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        let Some(end) = out.windows(4).position(|w| w == b"\r\n\r\n") else {
            return (0, String::new(), Vec::new());
        };
        let head = String::from_utf8_lossy(&out[..end]).to_string();
        (head[9..12].parse().unwrap(), head, out[end + 4..].to_vec())
    }

    impl Fixture {
        fn post(&self, extra: &str, body: &Value) -> (u16, String, Value) {
            let body = body.to_string();
            let req = format!(
                "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\n{extra}Content-Length: {}\r\n\r\n{body}",
                self.port,
                self.token,
                body.len()
            );
            let (s, head, b) = raw(self.port, req.as_bytes());
            let v = if b.is_empty() || head.contains("text/event-stream") {
                json!(String::from_utf8_lossy(&b))
            } else {
                serde_json::from_slice(&b).unwrap()
            };
            (s, head, v)
        }

        /// A request of the stateless revision with its envelope and headers.
        fn modern(&self, method: &str, params: Value) -> (u16, String, Value) {
            let mut p = params;
            p["_meta"] = json!({META_VERSION: MODERN, META_CAPABILITIES: {}, META_CLIENT_INFO: {"name": "claude-code", "version": "2"}});
            let name = p
                .get("name")
                .and_then(Value::as_str)
                .map(|n| format!("Mcp-Name: {n}\r\n"))
                .unwrap_or_default();
            self.post(
                &format!("MCP-Protocol-Version: {MODERN}\r\nMcp-Method: {method}\r\n{name}"),
                &json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": p}),
            )
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
        }
    }

    /// The stateless revision: discover, list, call, ping, listen, with
    /// the envelope and headers checked.
    #[test]
    fn the_stateless_handshake() {
        let f = fixture();
        let (s, _, v) = f.modern("server/discover", json!({}));
        assert_eq!(s, 200, "{v}");
        assert_eq!(v["result"]["supportedVersions"], json!([MODERN]));
        assert_eq!(v["result"]["instructions"], "Use the calculators.");
        assert_eq!(v["result"]["resultType"], "complete");
        assert_eq!(v["result"]["_meta"][META_SERVER_INFO]["name"], "saturnus");
        let (s, _, v) = f.modern("tools/list", json!({}));
        assert_eq!(s, 200, "{v}");
        assert_eq!(v["result"]["tools"][0]["name"], "read_display");
        assert_eq!(v["result"]["ttlMs"], 0);
        let (s, _, v) = f.modern(
            "tools/call",
            json!({"name": "read_display", "arguments": {}}),
        );
        assert_eq!(s, 200, "{v}");
        assert_eq!(
            v["result"]["content"][0]["text"],
            "\"read_display\" for claude-code"
        );
        assert_eq!(f.window.seen.lock().unwrap().last().unwrap(), "claude-code");
        let (s, _, v) = f.modern("ping", json!({}));
        assert_eq!((s, &v["result"]["resultType"]), (200, &json!("complete")));
        let (s, head, v) = f.modern(
            "subscriptions/listen",
            json!({"notifications": {"toolsListChanged": true}}),
        );
        assert_eq!(s, 200);
        assert!(head.contains("text/event-stream"), "{head}");
        assert!(v.as_str().unwrap().contains("acknowledged"), "{v}");
        let (s, _, v) = f.modern("resources/list", json!({}));
        assert_eq!((s, &v["error"]["code"]), (404, &json!(-32601)));

        // Claude Code's mismatched header is accepted with a modern body.
        let body = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {"_meta": {META_VERSION: MODERN, META_CAPABILITIES: {}}}});
        let (s, _, v) = f.post(
            "MCP-Protocol-Version: 2025-11-25\r\nMcp-Method: tools/list\r\n",
            &body,
        );
        assert_eq!(s, 200, "{v}");
        // Headers that disagree with the body, or are missing, are refused.
        let (s, _, v) = f.post(
            "MCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/call\r\n",
            &body,
        );
        assert_eq!(
            (s, &v["error"]["code"]),
            (400, &json!(HEADER_MISMATCH)),
            "{v}"
        );
        let (s, _, v) = f.post("MCP-Protocol-Version: 2026-07-28\r\n", &body);
        assert_eq!(
            (s, &v["error"]["code"]),
            (400, &json!(HEADER_MISMATCH)),
            "{v}"
        );
        let (s, _, v) = f.post("Mcp-Method: tools/list\r\n", &body);
        assert_eq!(
            (s, &v["error"]["code"]),
            (400, &json!(HEADER_MISMATCH)),
            "{v}"
        );
        let call = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "a", "_meta": {META_VERSION: MODERN, META_CAPABILITIES: {}}}});
        let (s, _, v) = f.post(
            "MCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/call\r\nMcp-Name: b\r\n",
            &call,
        );
        assert_eq!(
            (s, &v["error"]["code"]),
            (400, &json!(HEADER_MISMATCH)),
            "{v}"
        );
        let (s, _, v) = f.post("MCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/call\r\nMcp-Name: =?base64?YQ==?=\r\n", &call);
        assert_eq!(s, 200, "the Base64 sentinel decodes: {v}");
        // A bad envelope, an unknown revision.
        let bad = json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list", "params": {"_meta": {META_VERSION: MODERN}}});
        let (s, _, v) = f.post(
            "MCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/list\r\n",
            &bad,
        );
        assert_eq!((s, &v["error"]["code"]), (400, &json!(-32602)), "{v}");
        let future = json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list", "params": {"_meta": {META_VERSION: "2099-01-01", META_CAPABILITIES: {}}}});
        let (s, _, v) = f.post(
            "MCP-Protocol-Version: 2099-01-01\r\nMcp-Method: tools/list\r\n",
            &future,
        );
        assert_eq!(
            (s, &v["error"]["code"]),
            (400, &json!(UNSUPPORTED_VERSION)),
            "{v}"
        );
        // A modern header on a body without the envelope.
        let bare = json!({"jsonrpc": "2.0", "id": 6, "method": "tools/list"});
        let (s, _, v) = f.post(
            "MCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/list\r\n",
            &bare,
        );
        assert_eq!((s, &v["error"]["code"]), (400, &json!(-32602)), "{v}");
    }

    /// The legacy handshake: initialize, the notification, list and call.
    #[test]
    fn the_legacy_handshake() {
        let f = fixture();
        let init = json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "cursor", "version": "1"}}});
        let (s, head, v) = f.post("", &init);
        assert_eq!(s, 200, "{v}");
        assert_eq!(v["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(v["result"]["serverInfo"]["name"], "saturnus");
        assert_eq!(v["result"]["instructions"], "Use the calculators.");
        assert!(v["result"]["capabilities"]["tools"].is_object());
        let session = head
            .lines()
            .find_map(|l| l.strip_prefix("Mcp-Session-Id: "))
            .unwrap()
            .to_string();
        // A version it does not know is answered with the newest.
        let mut newer = init.clone();
        newer["params"]["protocolVersion"] = json!("2030-01-01");
        let (_, _, v) = f.post("", &newer);
        assert_eq!(v["result"]["protocolVersion"], LEGACY[2]);
        let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        let (s, _, _) = f.post("MCP-Protocol-Version: 2025-06-18\r\n", &note);
        assert_eq!(s, 202);
        let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
        let (s, _, v) = f.post(
            &format!("MCP-Protocol-Version: 2025-06-18\r\nMcp-Session-Id: {session}\r\n"),
            &list,
        );
        assert_eq!(s, 200, "{v}");
        assert_eq!(v["result"]["tools"][0]["name"], "read_display");
        assert!(v["result"].get("resultType").is_none());
        let call = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "read_display", "arguments": {}}});
        let (s, _, v) = f.post(&format!("Mcp-Session-Id: {session}\r\n"), &call);
        assert_eq!(s, 200, "{v}");
        assert_eq!(
            v["result"]["content"][0]["text"], "\"read_display\" for cursor",
            "named by the session"
        );
        // Without a session the User-Agent names it; the session is never required.
        let (s, _, v) = f.post("User-Agent: codex_cli_rs/0.5\r\n", &call);
        assert_eq!(s, 200, "{v}");
        assert_eq!(
            v["result"]["content"][0]["text"],
            "\"read_display\" for codex_cli_rs"
        );
        let (s, _, v) = f.post("", &json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}));
        assert_eq!((s, &v["result"]), (200, &json!({})));
        let (s, _, v) = f.post(
            "",
            &json!({"jsonrpc": "2.0", "id": 4, "method": "prompts/list"}),
        );
        assert_eq!((s, &v["error"]["code"]), (200, &json!(-32601)), "{v}");
        // An unknown legacy header version, and an initialize with a modern header.
        let (s, _, v) = f.post("MCP-Protocol-Version: 2024-01-01\r\n", &list);
        assert_eq!(
            (s, &v["error"]["code"]),
            (400, &json!(UNSUPPORTED_VERSION)),
            "{v}"
        );
        let (s, _, v) = f.post("MCP-Protocol-Version: 2026-07-28\r\n", &init);
        assert_eq!(
            (s, &v["error"]["code"]),
            (400, &json!(HEADER_MISMATCH)),
            "{v}"
        );
        // Not JSON-RPC: batches, responses, bad JSON.
        let (s, _, v) = f.post("", &json!([list]));
        assert_eq!((s, &v["error"]["code"]), (400, &json!(-32600)));
        let (s, _, _) = f.post("", &json!({"jsonrpc": "2.0", "id": 9, "result": {}}));
        assert_eq!(s, 400);
        let (s, _, _) = raw(
            f.port,
            format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: 1\r\n\r\n{{", f.port, f.token).as_bytes(),
        );
        assert_eq!(s, 400);
    }

    /// A window that is not ready, or a call whose client leaves.
    #[test]
    fn a_window_that_does_not_answer() {
        let f = fixture();
        f.window.not_ready.store(true, Ordering::SeqCst);
        let (s, _, v) = f.modern("tools/list", json!({}));
        assert_eq!((s, &v["error"]["code"]), (200, &json!(NOT_ANSWERED)), "{v}");
        assert!(
            v["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not ready")
        );
        // A tool call says it in its result, for the AI app to read.
        let (s, _, v) = f.modern("tools/call", json!({"name": "read_display"}));
        assert_eq!((s, &v["result"]["isError"]), (200, &json!(true)), "{v}");
        // Discover still answers, with what to do.
        let (s, _, v) = f.modern("server/discover", json!({}));
        assert_eq!(s, 200);
        assert!(
            v["result"]["instructions"]
                .as_str()
                .unwrap()
                .contains("not ready")
        );
        f.window.not_ready.store(false, Ordering::SeqCst);
        f.window.slow.store(true, Ordering::SeqCst);
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string();
        let mut s = TcpStream::connect(("127.0.0.1", f.port)).unwrap();
        write!(s, "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", f.port, f.token, body.len()).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        drop(s);
        let end = Instant::now() + Duration::from_secs(10);
        while !f.window.cancelled.load(Ordering::SeqCst) {
            assert!(Instant::now() < end, "the call was not cancelled");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The control API's rules: Host, Origin, OPTIONS, no CORS, the token
    /// without detail, the path and the method, content type and caps.
    #[test]
    fn the_control_api_rules_hold() {
        let f = fixture();
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}).to_string();
        let auth = format!("Authorization: Bearer {}\r\n", f.token);
        let host = format!("Host: 127.0.0.1:{}\r\n", f.port);
        let req = |method: &str, path: &str, extra: &str| {
            raw(
                f.port,
                format!("{method} {path} HTTP/1.1\r\n{extra}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes(),
            )
        };
        let (s, head, _) = req("POST", "/mcp", &format!("{host}{auth}"));
        assert_eq!(s, 200);
        assert!(
            !head.to_ascii_lowercase().contains("access-control"),
            "{head}"
        );
        for bad_host in [
            "Host: evil.example\r\n".to_string(),
            format!("Host: evil.example:{}\r\n", f.port),
            format!("Host: 127.0.0.1:{}\r\n", f.port.wrapping_add(1)),
            String::new(),
        ] {
            assert_eq!(
                req("POST", "/mcp", &format!("{bad_host}{auth}")).0,
                421,
                "{bad_host:?}"
            );
        }
        assert_eq!(
            req(
                "POST",
                "/mcp",
                &format!("Host: localhost:{}\r\n{auth}", f.port)
            )
            .0,
            200
        );
        for origin in [
            "http://evil.example".to_string(),
            "null".to_string(),
            format!("https://127.0.0.1:{}", f.port),
            "http://localhost:1".to_string(),
        ] {
            let (s, head, _) = req("POST", "/mcp", &format!("{host}{auth}Origin: {origin}\r\n"));
            assert_eq!(s, 403, "{origin}");
            assert!(!head.to_ascii_lowercase().contains("access-control"));
        }
        assert_eq!(
            req(
                "POST",
                "/mcp",
                &format!("{host}{auth}Origin: http://127.0.0.1:{}\r\n", f.port)
            )
            .0,
            200
        );
        let (s, head, _) = req(
            "OPTIONS",
            "/mcp",
            &format!(
                "{host}Origin: http://127.0.0.1:{}\r\nAccess-Control-Request-Method: POST\r\n",
                f.port
            ),
        );
        assert_eq!(s, 405);
        assert!(
            !head.to_ascii_lowercase().contains("access-control"),
            "{head}"
        );
        // The token: missing or wrong is 401 with {} and nothing else, before the path.
        let (s, head, b) = req("POST", "/mcp", &host);
        assert_eq!((s, b.as_slice()), (401, b"{}".as_slice()));
        assert!(head.contains("WWW-Authenticate: Bearer"));
        for a in [
            format!("Bearer {}", "0".repeat(64)),
            format!("Basic {}", f.token),
            format!("Bearer {}", &f.token[..63]),
            format!("Bearer {}0", f.token),
        ] {
            let (s, _, b) = req("POST", "/mcp", &format!("{host}Authorization: {a}\r\n"));
            assert_eq!(s, 401, "{a}");
            assert!(!String::from_utf8_lossy(&b).contains(&f.token));
        }
        assert_eq!(req("POST", "/other", &host).0, 401);
        assert_eq!(req("POST", "/other", &format!("{host}{auth}")).0, 404);
        for m in ["GET", "DELETE", "PUT"] {
            let (s, head, _) = req(m, "/mcp", &format!("{host}{auth}"));
            assert_eq!(s, 405, "{m}");
            assert!(head.contains("Allow: POST"), "{head}");
        }
        let (s, _, _) = raw(f.port, format!("POST /mcp HTTP/1.1\r\n{host}{auth}Content-Type: text/plain\r\nContent-Length: 2\r\n\r\n{{}}").as_bytes());
        assert_eq!(s, 415);
        // Caps: the body from its Content-Length, no Transfer-Encoding, the head.
        let (s, _, _) = raw(f.port, format!("POST /mcp HTTP/1.1\r\n{host}{auth}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n", MAX_BODY + 1).as_bytes());
        assert_eq!(s, 413);
        let (s, _, _) = raw(f.port, format!("POST /mcp HTTP/1.1\r\n{host}{auth}Content-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n").as_bytes());
        assert_eq!(s, 400);
        let mut big = b"POST /mcp HTTP/1.1\r\nX: ".to_vec();
        big.extend(std::iter::repeat_n(b'a', http::MAX_HEAD + 10));
        big.extend_from_slice(b"\r\n\r\n");
        assert_eq!(raw(f.port, &big).0, 431);
        // The token never comes back in an answer.
        let (_, head, b) = req("POST", "/mcp", &format!("{host}{auth}"));
        assert!(!head.contains(&f.token) && !String::from_utf8_lossy(&b).contains(&f.token));
    }

    /// Turned off, the endpoint stops accepting at once.
    #[test]
    fn turned_off_it_stops_accepting() {
        let f = fixture();
        assert_eq!(f.modern("ping", json!({})).0, 200);
        f.stop.store(true, Ordering::SeqCst);
        let end = Instant::now() + Duration::from_secs(5);
        while TcpStream::connect(("127.0.0.1", f.port)).is_ok() {
            assert!(Instant::now() < end, "still accepting");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn names_and_header_values() {
        assert_eq!(clean_name(" Claude\u{7}Code "), "ClaudeCode");
        assert_eq!(clean_name(&"x".repeat(100)).len(), 64);
        assert_eq!(decode_header_value("plain").as_deref(), Some("plain"));
        assert_eq!(
            decode_header_value("=?base64?cnVuX3JwbA==?=").as_deref(),
            Some("run_rpl")
        );
        assert_eq!(decode_header_value("=?base64?!!?="), None);
    }
}
