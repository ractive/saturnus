#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! WebAssembly bindings of saturnus tx for the transfer page
//! (`web/tx/`, `kb/docs/saturnus-tx.md`), built by `web/tx-build.sh` into
//! `web/tx/pkg`:
//!
//! - [`TxDevice`]: `saturnus-tx`'s [`Device`] over a link the page holds.
//!   The page (its protocol Worker, `web/tx/protocol-worker.js`) feeds it
//!   bytes, link errors and the time in milliseconds, and takes from
//!   [`TxDevice::drain`] what to do: open the port with these settings,
//!   write these bytes, the events and the request's result.
//! - The preview: [`describe_file`] (a binary transfer file as the typed
//!   object the shared `objects.js` shows), [`text_of`] (a `%%HP:` text
//!   file's body with the trigraphs as characters), and the file names
//!   ([`file_kind`], [`var_name`], [`file_name`]).
//!
//! The logic is in plain Rust functions over `serde_json` values, tested
//! natively; the wasm wrappers only convert.

use std::io;

use js_sys::{Array, Reflect, Uint8Array};
use saturnus_tx::device::ConnectOptions;
use saturnus_tx::names::{self, FileKind};
use saturnus_tx::{
    Action, Connection, Device, DeviceOptions, Event, Format, LinkSettings, Outcome, Parity,
    Progress, Request, TxError,
};
use satx_core::time::{Duration, Instant};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

pub mod preview;

pub use preview::{describe_bytes, text_body};

fn js_err(e: impl std::fmt::Display) -> JsValue {
    js_sys::Error::new(&e.to_string()).into()
}

/// A JSON value as a JS value.
fn to_js(v: &Value) -> Result<JsValue, JsValue> {
    js_sys::JSON::parse(&v.to_string())
}

// ------------------------------------------------------------ requests

/// The request a page message names: `{type: "connect", find, baud,
/// parity}`, `{type: "list", dir}`, `{type: "get", dir, name, format:
/// "binary" | "text"}`, `{type: "put", dir, name, replace, asString}`
/// (the file is `data`), `{type: "disconnect"}`.
pub fn parse_request(msg: &Value, data: Option<Vec<u8>>) -> Result<Request, String> {
    let kind = msg.get("type").and_then(Value::as_str).unwrap_or("");
    let dir = || -> Result<Vec<String>, String> {
        match msg.get("dir") {
            None | Some(Value::Null) => Ok(vec!["HOME".to_string()]),
            Some(Value::Array(parts)) => parts
                .iter()
                .map(|p| {
                    p.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| "a directory is a list of names".to_string())
                })
                .collect(),
            Some(_) => Err("a directory is a list of names".to_string()),
        }
    };
    let name = || -> Result<String, String> {
        msg.get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "a variable name is needed".to_string())
    };
    let flag = |key: &str| msg.get(key).and_then(Value::as_bool).unwrap_or(false);
    Ok(match kind {
        "connect" => {
            let baud = msg
                .get("baud")
                .and_then(Value::as_u64)
                .map_or(Ok(9600), |b| u32::try_from(b).map_err(|e| e.to_string()))?;
            if !saturnus_tx::device::SPEEDS.contains(&baud) {
                return Err(format!("{baud} baud is not a speed the calculators have"));
            }
            let parity = match msg.get("parity").and_then(Value::as_str) {
                None => Parity::None,
                Some(p) => Parity::from_name(p).ok_or_else(|| format!("no parity {p:?}"))?,
            };
            Request::Connect(ConnectOptions {
                find: msg.get("find").and_then(Value::as_bool).unwrap_or(true),
                settings: LinkSettings { baud, parity },
            })
        }
        "list" => Request::List { dir: dir()? },
        "get" => Request::Get {
            dir: dir()?,
            name: name()?,
            format: match msg.get("format").and_then(Value::as_str) {
                None | Some("binary") => Format::Binary,
                Some("text") => Format::Text,
                Some(f) => return Err(format!("no format {f:?}")),
            },
        },
        "put" => Request::Put {
            dir: dir()?,
            name: name()?,
            data: data.ok_or("a put needs the file's bytes")?,
            replace: flag("replace"),
            as_string: flag("asString"),
        },
        "disconnect" => Request::Disconnect,
        other => return Err(format!("no request {other:?}")),
    })
}

fn model_name(model: satx_core::Model) -> &'static str {
    match model {
        satx_core::Model::Hp48Sx => "48sx",
        satx_core::Model::Hp48Gx => "48gx",
        satx_core::Model::Hp49G => "49g",
        satx_core::Model::Unknown => "unknown",
    }
}

fn settings_json(s: LinkSettings) -> Value {
    json!({"baud": s.baud, "parity": s.parity.name(), "dataBits": s.data_bits()})
}

/// A connection as JSON: `{settings, model, version, userMode}`.
pub fn connection_json(c: &Connection) -> Value {
    json!({
        "settings": settings_json(c.settings),
        "model": model_name(c.model),
        "version": c.version,
        "userMode": match c.user_mode {
            satx_core::TransferMode::Binary => "binary",
            satx_core::TransferMode::Ascii => "text",
        },
    })
}

/// An outcome as JSON, and the file's bytes apart (they go to the page
/// as a `Uint8Array`, as `data`).
pub fn outcome_json(o: Outcome) -> (Value, Option<Vec<u8>>) {
    match o {
        Outcome::Connected(c) => (
            json!({"type": "connected", "connection": connection_json(&c)}),
            None,
        ),
        Outcome::Listing(l) => (
            json!({
                "type": "listing",
                "path": l.path,
                "free": l.free,
                "entries": l.entries.iter().map(|e| json!({
                    "name": e.name,
                    "kind": e.kind,
                    "size": e.size,
                    "checksum": e.checksum,
                    "directory": e.is_directory(),
                })).collect::<Vec<_>>(),
            }),
            None,
        ),
        Outcome::File(f) => (
            json!({
                "type": "file",
                "name": f.name,
                "fileName": f.file_name,
                "format": match f.format { Format::Binary => "binary", Format::Text => "text" },
            }),
            Some(f.data),
        ),
        Outcome::Stored(name) => (json!({"type": "stored", "name": name}), None),
        Outcome::Disconnected => (json!({"type": "disconnected"}), None),
        _ => (json!({"type": "other"}), None),
    }
}

/// An error as JSON: `{kind, message, name?}`, `kind` one of `busy`,
/// `notConnected`, `alreadyConnected`, `noAnswer`, `nameClash`,
/// `isDirectory`, `notAnObject`, `badName`, `calculator`.
pub fn error_json(e: &TxError) -> Value {
    let (kind, name) = match e {
        TxError::Busy => ("busy", None),
        TxError::NotConnected => ("notConnected", None),
        TxError::AlreadyConnected => ("alreadyConnected", None),
        TxError::NoAnswer { .. } => ("noAnswer", None),
        TxError::NameClash(n) => ("nameClash", Some(n.clone())),
        TxError::IsDirectory(n) => ("isDirectory", Some(n.clone())),
        TxError::NotAnObject => ("notAnObject", None),
        TxError::BadName(n) => ("badName", Some(n.clone())),
        _ => ("calculator", None),
    };
    json!({"kind": kind, "message": e.to_string(), "name": name})
}

/// An event as JSON, or `None` for one the page has no use for.
pub fn event_json(e: Event) -> Option<Value> {
    Some(match e {
        Event::Listening(s) => json!({"type": "listening", "settings": settings_json(s)}),
        Event::Found(s) => json!({"type": "found", "settings": settings_json(s)}),
        Event::Progress(Progress::Kermit(kermit_proto::Event::Data(d))) => {
            json!({"type": "progress", "received": d.len()})
        }
        Event::Progress(Progress::Kermit(kermit_proto::Event::Progress { sent, total })) => {
            json!({"type": "progress", "sent": sent, "total": total})
        }
        _ => return None,
    })
}

// ------------------------------------------------------------ the device

/// `saturnus-tx`'s [`Device`] over a link the page holds. Every call
/// takes the page's clock in milliseconds (`performance.now()`, or a
/// test's fake clock), which only has to grow.
#[wasm_bindgen]
pub struct TxDevice {
    device: Device,
    /// The device's clock at `base_ms` on the page's.
    base: Instant,
    base_ms: f64,
}

impl std::fmt::Debug for TxDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxDevice")
            .field("device", &self.device)
            .finish()
    }
}

impl TxDevice {
    fn at(&self, now_ms: f64) -> Instant {
        let ms = (now_ms - self.base_ms).max(0.0);
        self.base + Duration::from_micros((ms * 1000.0) as u64)
    }
}

#[wasm_bindgen]
impl TxDevice {
    /// A device for one link; `seed` should be random (the sync markers),
    /// `listen_ms` how long the speed search listens at each speed (the
    /// default 6000 when absent).
    #[wasm_bindgen(constructor)]
    pub fn new(now_ms: f64, seed: f64, listen_ms: Option<f64>) -> TxDevice {
        let mut options = DeviceOptions::default();
        if let Some(ms) = listen_ms.filter(|ms| *ms > 0.0) {
            options.listen = Duration::from_millis(ms as u64);
        }
        let base = Instant::now();
        TxDevice {
            device: Device::new(base, options, seed as u64),
            base,
            base_ms: now_ms,
        }
    }

    /// Begin a request (see [`parse_request`]); `data` is a put's file.
    /// Throws for a malformed request or while one runs.
    pub fn start(
        &mut self,
        now_ms: f64,
        request: &str,
        data: Option<Vec<u8>>,
    ) -> Result<(), JsValue> {
        let msg: Value = serde_json::from_str(request).map_err(js_err)?;
        let request = parse_request(&msg, data).map_err(js_err)?;
        let now = self.at(now_ms);
        self.device
            .start(now, request)
            .map_err(|_| js_err("another operation is still running"))
    }

    /// Bytes read from the link.
    pub fn input(&mut self, now_ms: f64, bytes: &[u8]) {
        let now = self.at(now_ms);
        self.device.handle_input(now, bytes);
    }

    /// The time passed (the page's timer for [`TxDevice::deadline`]).
    pub fn timeout(&mut self, now_ms: f64) {
        let now = self.at(now_ms);
        self.device.handle_timeout(now);
    }

    /// The link failed or closed: the running request fails with `message`.
    #[wasm_bindgen(js_name = linkError)]
    pub fn link_error(&mut self, now_ms: f64, message: &str) {
        let now = self.at(now_ms);
        let err = io::Error::new(io::ErrorKind::BrokenPipe, message.to_string());
        self.device.handle_link_error(now, &err);
    }

    /// Drop the running request without telling the calculator.
    pub fn abort(&mut self, now_ms: f64) {
        let now = self.at(now_ms);
        self.device.abort(now);
    }

    /// When to call [`TxDevice::timeout`] at the latest, on the page's
    /// clock; `undefined` while no request runs.
    pub fn deadline(&self) -> Option<f64> {
        let at = self.device.next_timeout()?;
        let ms = at.saturating_duration_since(self.base).as_secs_f64() * 1000.0;
        Some(self.base_ms + ms)
    }

    /// Whether a request runs.
    #[wasm_bindgen(js_name = isBusy)]
    pub fn is_busy(&self) -> bool {
        self.device.is_busy()
    }

    /// The connected calculator (`{settings, model, version, userMode}`)
    /// or `null`.
    pub fn connection(&self) -> Result<JsValue, JsValue> {
        match self.device.connection() {
            Some(c) => to_js(&connection_json(&c)),
            None => Ok(JsValue::NULL),
        }
    }

    /// What the page does next, in order: `{type: "open", baud, parity,
    /// dataBits}` (reopen the port; input read before it is not fed any
    /// more), `{type: "write", bytes}`, events (`listening`, `found`,
    /// `progress`), and last the result, `{type: "result", ok: true,
    /// outcome, data?}` or `{type: "result", ok: false, error}`.
    pub fn drain(&mut self) -> Result<Array, JsValue> {
        let out = Array::new();
        while let Some(action) = self.device.poll_action() {
            match action {
                Action::Open(s) => {
                    let mut v = settings_json(s);
                    v["type"] = json!("open");
                    out.push(&to_js(&v)?);
                }
                Action::Write(bytes) => {
                    let v = to_js(&json!({"type": "write"}))?;
                    Reflect::set(&v, &"bytes".into(), &Uint8Array::from(bytes.as_slice()))?;
                    out.push(&v);
                }
            }
        }
        while let Some(e) = self.device.poll_event() {
            if let Some(v) = event_json(e) {
                out.push(&to_js(&v)?);
            }
        }
        if let Some(result) = self.device.poll_result() {
            let v = match result {
                Ok(outcome) => {
                    let (o, data) = outcome_json(outcome);
                    let v = to_js(&json!({"type": "result", "ok": true, "outcome": o}))?;
                    if let Some(d) = data {
                        Reflect::set(&v, &"data".into(), &Uint8Array::from(d.as_slice()))?;
                    }
                    v
                }
                Err(e) => to_js(&json!({"type": "result", "ok": false, "error": error_json(&e)}))?,
            };
            out.push(&v);
        }
        Ok(out)
    }
}

// ------------------------------------------------------------ files

/// What a file holds: `"binary"` (a binary transfer file), `"text"` (a
/// `%%HP:` text file) or `"other"` (stored as a string).
#[wasm_bindgen]
pub fn file_kind(data: &[u8]) -> String {
    match FileKind::of(data) {
        FileKind::Binary => "binary",
        FileKind::Text => "text",
        FileKind::Other => "other",
    }
    .to_string()
}

/// The variable name a put takes from `file_name` (without its
/// extension, `%XX` read back); throws when there is none.
#[wasm_bindgen]
pub fn var_name(file_name: &str) -> Result<String, JsValue> {
    names::var_name(file_name).map_err(js_err)
}

/// The file name of variable `name` got as `format` (`"binary"`: `.hp`,
/// `"text"`: `.txt`).
#[wasm_bindgen]
pub fn file_name(name: &str, format: &str) -> String {
    let format = if format == "text" {
        Format::Text
    } else {
        Format::Binary
    };
    names::file_name(name, format)
}

/// Whether the calculator takes `name` as a variable name; `null` if it
/// does, else why not.
#[wasm_bindgen]
pub fn name_problem(name: &str) -> Option<String> {
    satx_core::calc::validate_name(name)
        .err()
        .map(|e| e.to_string())
}

/// A binary transfer file as the typed object of the front-end protocol
/// (the shape `web/shared/objects.js` previews: `type`, the fields, and
/// `text` where the calculator's text is known; a GROB's `graphic`).
/// Throws for a file that is not one.
#[wasm_bindgen]
pub fn describe_file(data: &[u8]) -> Result<JsValue, JsValue> {
    to_js(&describe_bytes(data).map_err(js_err)?)
}

/// The body of a `%%HP:` text file (or any text) with the trigraphs as
/// the characters they stand for.
#[wasm_bindgen]
pub fn text_of(data: &[u8]) -> String {
    text_body(data)
}

/// This build's version (the workspace's).
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_parse() {
        let r = parse_request(&json!({"type": "connect"}), None).unwrap();
        assert!(matches!(r, Request::Connect(o) if o.find && o.settings == LinkSettings::DEFAULT));
        let r = parse_request(
            &json!({"type": "connect", "find": false, "baud": 2400, "parity": "odd"}),
            None,
        )
        .unwrap();
        assert!(
            matches!(r, Request::Connect(o) if !o.find && o.settings.baud == 2400 && o.settings.parity == Parity::Odd)
        );
        assert!(parse_request(&json!({"type": "connect", "baud": 1234}), None).is_err());
        let r = parse_request(&json!({"type": "list", "dir": ["HOME", "D"]}), None).unwrap();
        assert!(matches!(r, Request::List { dir } if dir == ["HOME", "D"]));
        let r =
            parse_request(&json!({"type": "get", "name": "A", "format": "text"}), None).unwrap();
        assert!(
            matches!(r, Request::Get { name, format: Format::Text, dir } if name == "A" && dir == ["HOME"])
        );
        assert!(parse_request(&json!({"type": "get"}), None).is_err());
        assert!(parse_request(&json!({"type": "put", "name": "A"}), None).is_err());
        let r = parse_request(
            &json!({"type": "put", "name": "A", "replace": true}),
            Some(vec![1]),
        )
        .unwrap();
        assert!(matches!(
            r,
            Request::Put {
                replace: true,
                as_string: false,
                ..
            }
        ));
        assert!(parse_request(&json!({"type": "nope"}), None).is_err());
    }

    #[test]
    fn errors_name_their_kind() {
        let v = error_json(&TxError::NameClash("A".into()));
        assert_eq!(v["kind"], "nameClash");
        assert_eq!(v["name"], "A");
        assert_eq!(error_json(&TxError::NotAnObject)["kind"], "notAnObject");
    }

    #[test]
    fn file_names_round_trip() {
        assert_eq!(file_name("A/B", "binary"), "A%2FB.hp");
        assert_eq!(file_name("X", "text"), "X.txt");
        assert_eq!(names::var_name("A%2FB.hp").unwrap(), "A/B");
        assert!(name_problem("A").is_none());
        assert!(name_problem("1A").is_some());
        assert_eq!(file_kind(b"%%HP: T(3)A(R)F(.);\r\n1"), "text");
        assert_eq!(file_kind(b"hello"), "other");
    }
}
