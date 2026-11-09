//! The HP 100LX/200LX palmtop for the page: [`TxPalmtop`], saturnus-tx's
//! [`Palmtop`] over a link the page holds (`web/tx/palmtop.js` through the
//! protocol Worker, which runs it in the same driver as [`TxDevice`]), the
//! name check [`palmtop_name_problem`], and [`TxFilerServer`],
//! `filer-proto`'s server over its in-memory files: the palmtop of the
//! page's tests (`web/test/tx-palmtop.test.mjs`), on a link of their own.
//!
//! The requests are [`parse_palmtop_request`]'s JSON, the results
//! [`palmtop_outcome_json`]'s and [`palmtop_error_json`]'s.
//!
//! [`TxDevice`]: crate::tx::TxDevice

use filer_proto::time::{Duration, Instant};
use filer_proto::{DosDateTime, FsError, MemoryFs, Server, ServerConfig, attr};
use js_sys::{Array, Reflect, Uint8Array};
use saturnus_tx::Action;
use saturnus_tx::palmtop::{
    self, Palmtop, PalmtopConnection, PalmtopEntry, PalmtopError, PalmtopEvent, PalmtopOutcome,
    PalmtopRequest,
};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::tx::{js_err, to_js};

fn dir_of(msg: &Value) -> Result<Vec<String>, String> {
    match msg.get("dir") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(parts)) => parts
            .iter()
            .map(|p| {
                p.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "a directory is a list of names".to_string())
            })
            .collect(),
        Some(_) => Err("a directory is a list of names".into()),
    }
}

fn text(msg: &Value, key: &str) -> Result<String, String> {
    msg.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{key} is missing"))
}

/// A request from the page's JSON: `{type: "connect", baud}`, `{type:
/// "list", dir}` (`[]`: the drives), `{type: "get", dir, name}`, `{type:
/// "put", dir, name, replace}` with the file as `data`, `{type: "rename",
/// dir, from, to}`, `{type: "remove", dir, name, directory}`, `{type:
/// "mkdir", dir, name}`, `{type: "disconnect"}`. A directory is a list of
/// names from the drive (`["C:", "_DAT"]`).
pub fn parse_palmtop_request(msg: &Value, data: Option<Vec<u8>>) -> Result<PalmtopRequest, String> {
    let kind = msg.get("type").and_then(Value::as_str).unwrap_or_default();
    Ok(match kind {
        "connect" => PalmtopRequest::Connect {
            baud: msg
                .get("baud")
                .and_then(Value::as_u64)
                .map_or(Ok(palmtop::DEFAULT_BAUD), u32::try_from)
                .map_err(|_| "a speed in baud".to_string())?,
        },
        "list" => PalmtopRequest::List { dir: dir_of(msg)? },
        "get" => PalmtopRequest::Get {
            dir: dir_of(msg)?,
            name: text(msg, "name")?,
        },
        "put" => PalmtopRequest::Put {
            dir: dir_of(msg)?,
            name: text(msg, "name")?,
            data: data.ok_or("a put needs the file")?,
            replace: msg.get("replace").and_then(Value::as_bool).unwrap_or(false),
        },
        "rename" => PalmtopRequest::Rename {
            dir: dir_of(msg)?,
            from: text(msg, "from")?,
            to: text(msg, "to")?,
        },
        "remove" => PalmtopRequest::Remove {
            dir: dir_of(msg)?,
            name: text(msg, "name")?,
            directory: msg
                .get("directory")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        "mkdir" => PalmtopRequest::MakeDir {
            dir: dir_of(msg)?,
            name: text(msg, "name")?,
        },
        "disconnect" => PalmtopRequest::Disconnect,
        other => return Err(format!("no request {other:?}")),
    })
}

/// A DOS time as `{year, month, day, hour, minute, second}`, or null.
fn time_json(t: Option<DosDateTime>) -> Value {
    match t {
        Some(t) if (1..=12).contains(&t.month()) && (1..=31).contains(&t.day()) => json!({
            "year": t.year(), "month": t.month(), "day": t.day(),
            "hour": t.hour(), "minute": t.minute(), "second": t.second(),
        }),
        _ => Value::Null,
    }
}

fn entry_json(e: &PalmtopEntry) -> Value {
    json!({
        "name": e.name,
        "directory": e.directory,
        "drive": e.drive,
        "size": e.size,
        "modified": time_json(e.modified),
        "attributes": e.attributes,
        "readOnly": e.read_only(),
        "hidden": e.attributes & attr::HIDDEN != 0,
        "system": e.attributes & attr::SYSTEM != 0,
    })
}

/// The connected palmtop as JSON: `{model: "palmtop", settings, revision}`.
pub fn palmtop_connection_json(c: PalmtopConnection) -> Value {
    json!({
        "model": "palmtop",
        "settings": {"baud": c.baud, "parity": "none", "dataBits": 8},
        "revision": c.revision,
    })
}

/// An outcome as JSON, and a file's bytes apart: `connected`
/// (`connection`), `listing` (`dir`, `entries`: `{name, directory,
/// drive, size, modified, attributes, readOnly, hidden, system}`), `file`
/// (`name`, `modified`; data), `stored` (`name`), `renamed` (`from`,
/// `to`), `removed` (`name`, `count`), `created` (`name`), `disconnected`.
pub fn palmtop_outcome_json(o: PalmtopOutcome) -> (Value, Option<Vec<u8>>) {
    match o {
        PalmtopOutcome::Connected(c) => (
            json!({"type": "connected", "connection": palmtop_connection_json(c)}),
            None,
        ),
        PalmtopOutcome::Listing { dir, entries } => (
            json!({"type": "listing", "dir": dir, "entries": entries.iter().map(entry_json).collect::<Vec<_>>()}),
            None,
        ),
        PalmtopOutcome::File {
            name,
            data,
            modified,
        } => (
            json!({"type": "file", "name": name, "modified": time_json(modified)}),
            Some(data),
        ),
        PalmtopOutcome::Stored(name) => (json!({"type": "stored", "name": name}), None),
        PalmtopOutcome::Renamed { from, to } => {
            (json!({"type": "renamed", "from": from, "to": to}), None)
        }
        PalmtopOutcome::Removed { name, count } => (
            json!({"type": "removed", "name": name, "count": count}),
            None,
        ),
        PalmtopOutcome::Created(name) => (json!({"type": "created", "name": name}), None),
        PalmtopOutcome::Disconnected => (json!({"type": "disconnected"}), None),
    }
}

/// An error as `{kind, message, name}`: `busy`, `notConnected`,
/// `alreadyConnected`, `badSpeed`, `noAnswer` (`baud`), `refused`,
/// `badName`, `exists`, `notFound`, `readOnly` (a write-protected drive),
/// `full`, `denied` (a read-only file, a directory not empty),
/// `notReady`, `palmtop` (another refusal; `code` the palmtop's),
/// `tooLarge`, `stopped`, `notAnswering`, `link`.
pub fn palmtop_error_json(e: &PalmtopError) -> Value {
    let (kind, name, code): (&str, Option<&str>, Option<u16>) = match e {
        PalmtopError::Busy => ("busy", None, None),
        PalmtopError::NotConnected => ("notConnected", None, None),
        PalmtopError::AlreadyConnected => ("alreadyConnected", None, None),
        PalmtopError::BadSpeed(_) => ("badSpeed", None, None),
        PalmtopError::NoAnswer { .. } => ("noAnswer", None, None),
        PalmtopError::Refused(_) => ("refused", None, None),
        PalmtopError::BadName { name, .. } => ("badName", Some(name), None),
        PalmtopError::Exists(n) => ("exists", Some(n), None),
        PalmtopError::NotFound(n) => ("notFound", Some(n), None),
        PalmtopError::Fs { name, error } => (
            match *error {
                FsError::WRITE_PROTECTED => "readOnly",
                FsError::DISK_FULL => "full",
                FsError::ACCESS_DENIED => "denied",
                FsError::NOT_READY => "notReady",
                FsError::PATH_NOT_FOUND => "notFound",
                _ => "palmtop",
            },
            Some(name),
            Some(error.code()),
        ),
        PalmtopError::Status { name, .. } => ("palmtop", Some(name), None),
        PalmtopError::TooLarge(n) => ("tooLarge", Some(n), None),
        PalmtopError::Stopped => ("stopped", None, None),
        PalmtopError::NotAnswering => ("notAnswering", None, None),
        PalmtopError::Link(_) => ("link", None, None),
        _ => ("palmtop", None, None),
    };
    let mut v = json!({"kind": kind, "message": e.to_string(), "name": name});
    if let Some(code) = code {
        v["code"] = json!(code);
    }
    if let PalmtopError::NoAnswer { baud } = e {
        v["baud"] = json!(baud);
    }
    v
}

/// Why the palmtop does not take `name` for a new file or directory (8.3
/// DOS names), or `undefined`.
#[wasm_bindgen]
pub fn palmtop_name_problem(name: &str) -> Option<String> {
    palmtop::dos_name(name).err().map(|p| {
        let text = p.to_string();
        let mut chars = text.chars();
        chars
            .next()
            .map(|c| c.to_uppercase().chain(chars).collect::<String>() + ".")
            .unwrap_or_default()
    })
}

/// saturnus-tx's [`Palmtop`] over a link the page holds, with
/// [`TxDevice`](crate::tx::TxDevice)'s calls, so the protocol Worker's
/// driver (`web/tx/driver.js`) runs either. Every call takes the page's
/// clock in milliseconds.
#[wasm_bindgen]
pub struct TxPalmtop {
    palmtop: Palmtop,
    base: Instant,
    base_ms: f64,
}

impl std::fmt::Debug for TxPalmtop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxPalmtop")
            .field("palmtop", &self.palmtop)
            .finish_non_exhaustive()
    }
}

impl TxPalmtop {
    fn at(&self, now_ms: f64) -> Instant {
        let ms = (now_ms - self.base_ms).max(0.0);
        self.base + Duration::from_micros((ms * 1000.0).round() as u64)
    }
}

#[wasm_bindgen]
impl TxPalmtop {
    /// A palmtop for one link, with the calculator device's arguments:
    /// the seed and the search's time are unused here; `silence_ms`, when
    /// given, is the wait for each reply in place of HP's 5.9 s, so a
    /// silent palmtop counts as not answering after three of them (tests).
    #[wasm_bindgen(constructor)]
    pub fn new(
        now_ms: f64,
        _seed: Option<f64>,
        _listen_ms: Option<f64>,
        silence_ms: Option<f64>,
    ) -> TxPalmtop {
        let palmtop = match silence_ms.filter(|ms| *ms > 0.0) {
            Some(ms) => Palmtop::with_reply_wait(Duration::from_millis(ms as u64)),
            None => Palmtop::new(),
        };
        TxPalmtop {
            palmtop,
            base: Instant::now(),
            base_ms: now_ms,
        }
    }

    /// Begin a request ([`parse_palmtop_request`]); `data` is a put's
    /// file. Throws for a malformed request or while one runs.
    pub fn start(&mut self, now_ms: f64, request: &str, data: JsValue) -> Result<(), JsValue> {
        let msg: Value = serde_json::from_str(request).map_err(js_err)?;
        let data = if data.is_instance_of::<Uint8Array>() {
            Some(Uint8Array::new(&data).to_vec())
        } else {
            None
        };
        let request = parse_palmtop_request(&msg, data).map_err(js_err)?;
        let now = self.at(now_ms);
        self.palmtop.start(now, request).map_err(js_err)
    }

    /// Bytes read from the link.
    pub fn input(&mut self, now_ms: f64, bytes: &[u8]) {
        let now = self.at(now_ms);
        self.palmtop.handle_input(now, bytes);
    }

    /// The time passed.
    pub fn timeout(&mut self, now_ms: f64) {
        let now = self.at(now_ms);
        self.palmtop.handle_timeout(now);
    }

    /// The link failed or closed.
    #[wasm_bindgen(js_name = linkError)]
    pub fn link_error(&mut self, now_ms: f64, message: &str) {
        let now = self.at(now_ms);
        self.palmtop.handle_link_error(now, message);
    }

    /// Stop the running request at the palmtop's next reply; its result
    /// (`stopped`) follows.
    pub fn stop(&mut self, now_ms: f64) {
        let now = self.at(now_ms);
        self.palmtop.stop(now);
    }

    /// Drop the running request at once (one DISCONNECT; the next request
    /// connects again first).
    pub fn abort(&mut self, now_ms: f64) {
        let now = self.at(now_ms);
        self.palmtop.abort(now);
    }

    /// When to call [`TxPalmtop::timeout`] at the latest, on the page's
    /// clock.
    pub fn deadline(&self) -> Option<f64> {
        let at = self.palmtop.next_timeout()?;
        let ms = at.saturating_duration_since(self.base).as_secs_f64() * 1000.0;
        Some(self.base_ms + ms)
    }

    /// Always false: the calculators' restore has no palmtop counterpart.
    pub fn restarted(&self) -> bool {
        false
    }

    /// Always false.
    #[wasm_bindgen(js_name = leftServer)]
    pub fn left_server(&self) -> bool {
        false
    }

    /// Whether the palmtop stopped answering since the last connect.
    #[wasm_bindgen(js_name = notAnswering)]
    pub fn not_answering(&self) -> bool {
        self.palmtop.not_answering()
    }

    /// Whether a request runs.
    #[wasm_bindgen(js_name = isBusy)]
    pub fn is_busy(&self) -> bool {
        self.palmtop.is_busy()
    }

    /// The connected palmtop ([`palmtop_connection_json`]) or `null`.
    pub fn connection(&self) -> Result<JsValue, JsValue> {
        match self.palmtop.connection() {
            Some(c) => to_js(&palmtop_connection_json(c)),
            None => Ok(JsValue::NULL),
        }
    }

    /// What the page does next, in order: `{type: "open", baud, parity,
    /// dataBits}`, `{type: "write", bytes}`, `{type: "progress", sent,
    /// total}` (`{type: "progress", done}` without a total), and last the
    /// result, `{type: "result", ok: true, outcome, data?}` or `{type:
    /// "result", ok: false, error}`.
    pub fn drain(&mut self) -> Result<Array, JsValue> {
        let out = Array::new();
        while let Some(action) = self.palmtop.poll_action() {
            match action {
                Action::Open(s) => out.push(&to_js(&json!({
                    "type": "open", "baud": s.baud, "parity": "none", "dataBits": 8,
                }))?),
                Action::Write(bytes) => {
                    let v = to_js(&json!({"type": "write"}))?;
                    Reflect::set(&v, &"bytes".into(), &Uint8Array::from(bytes.as_slice()))?;
                    out.push(&v)
                }
            };
        }
        while let Some(PalmtopEvent::Progress { done, total }) = self.palmtop.poll_event() {
            let v = match total {
                Some(total) => json!({"type": "progress", "sent": done, "total": total}),
                None => json!({"type": "progress", "done": done}),
            };
            out.push(&to_js(&v)?);
        }
        if let Some(result) = self.palmtop.poll_result() {
            let v = match result {
                Ok(outcome) => {
                    let (o, data) = palmtop_outcome_json(outcome);
                    let v = to_js(&json!({"type": "result", "ok": true, "outcome": o}))?;
                    if let Some(d) = data {
                        Reflect::set(&v, &"data".into(), &Uint8Array::from(d.as_slice()))?;
                    }
                    v
                }
                Err(e) => {
                    to_js(&json!({"type": "result", "ok": false, "error": palmtop_error_json(&e)}))?
                }
            };
            out.push(&v);
        }
        Ok(out)
    }
}

/// `filer-proto`'s server over its in-memory files: a palmtop in Filer
/// server mode for the page's tests, on a link of their own
/// (`web/test/tx-palmtop.test.mjs`). Drives `A:`, `C:` and `D:`, D:
/// write-protected as the palmtop's ROM disk.
#[wasm_bindgen]
pub struct TxFilerServer {
    server: Server<MemoryFs>,
}

impl std::fmt::Debug for TxFilerServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxFilerServer").finish_non_exhaustive()
    }
}

/// A DOS path of the server's files from the page's `C:\SUB\X.TXT`.
fn server_path(path: &str) -> Result<Vec<u8>, JsValue> {
    filer_proto::path::normalize(path, b'C').map_err(js_err)
}

#[wasm_bindgen]
impl TxFilerServer {
    /// A server holding `files` (`[{path: "C:\\SUB\\X.TXT", data}]`; no
    /// `data`: a directory).
    #[wasm_bindgen(constructor)]
    pub fn new(files: JsValue) -> Result<TxFilerServer, JsValue> {
        let mut fs = MemoryFs::new(b"ACD");
        fs.set_read_only(b'D');
        if Array::is_array(&files) {
            for item in Array::from(&files).iter() {
                let path = Reflect::get(&item, &"path".into())?
                    .as_string()
                    .ok_or_else(|| js_err("a file's path is a string"))?;
                let path = server_path(&path)?;
                let data = Reflect::get(&item, &"data".into())?;
                if data.is_instance_of::<Uint8Array>() {
                    fs.insert_file(&path, Uint8Array::new(&data).to_vec());
                } else {
                    fs.insert_dir(&path);
                }
            }
        }
        Ok(TxFilerServer {
            server: Server::new(fs, ServerConfig::default()),
        })
    }

    /// Bytes the page wrote.
    pub fn input(&mut self, bytes: &[u8]) {
        self.server.handle_input(bytes);
    }

    /// The replies to send back, each a `Uint8Array`.
    pub fn output(&mut self) -> Array {
        let out = Array::new();
        while let Some(frame) = self.server.poll_output() {
            out.push(&Uint8Array::from(frame.as_slice()));
        }
        out
    }

    /// Lose the next `n` replies (a lot: a palmtop gone silent; 0 answers
    /// again).
    #[wasm_bindgen(js_name = dropReplies)]
    pub fn drop_replies(&mut self, n: u32) {
        self.server.drop_replies(n);
    }

    /// A file's bytes, or `undefined`.
    pub fn file(&self, path: &str) -> Result<Option<Uint8Array>, JsValue> {
        let path = server_path(path)?;
        Ok(self.server.fs().file(&path).map(Uint8Array::from))
    }

    /// Every path the server holds, `C:\SUB\X.TXT`.
    pub fn paths(&self) -> Array {
        self.server
            .fs()
            .paths()
            .iter()
            .map(|p| JsValue::from(filer_proto::path::display(p)))
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_results() {
        let r = |v: Value| parse_palmtop_request(&v, Some(b"x".to_vec())).unwrap();
        assert_eq!(
            r(json!({"type": "connect"})),
            PalmtopRequest::Connect { baud: 19200 }
        );
        assert_eq!(
            r(json!({"type": "list", "dir": ["C:", "SUB"]})),
            PalmtopRequest::List {
                dir: vec!["C:".into(), "SUB".into()]
            }
        );
        assert_eq!(
            r(json!({"type": "put", "dir": ["C:"], "name": "A.TXT", "replace": true})),
            PalmtopRequest::Put {
                dir: vec!["C:".into()],
                name: "A.TXT".into(),
                data: b"x".to_vec(),
                replace: true
            }
        );
        assert!(
            parse_palmtop_request(&json!({"type": "put", "dir": [], "name": "A"}), None).is_err()
        );
        assert!(parse_palmtop_request(&json!({"type": "backup"}), None).is_err());
        let e = palmtop_error_json(&PalmtopError::Fs {
            name: "X.TXT".into(),
            error: FsError::WRITE_PROTECTED,
        });
        assert_eq!(e["kind"], "readOnly");
        assert_eq!(e["code"], 214);
        assert_eq!(
            palmtop_error_json(&PalmtopError::NoAnswer { baud: 38400 })["baud"],
            38400
        );
        let (o, data) = palmtop_outcome_json(PalmtopOutcome::File {
            name: "A.TXT".into(),
            data: vec![1],
            modified: DosDateTime::new(2026, 11, 9, 13, 5, 30),
        });
        assert_eq!(
            o["modified"],
            json!({"year": 2026, "month": 11, "day": 9, "hour": 13, "minute": 5, "second": 30})
        );
        assert_eq!(data, Some(vec![1]));
        assert_eq!(
            palmtop_name_problem("LONGNAME1.TXT").as_deref(),
            Some(
                "The palmtop takes DOS names: up to 8 letters or digits, a dot and up to 3 more (like NOTES.TXT)."
            )
        );
        assert_eq!(palmtop_name_problem("ok.txt"), None);
    }
}
