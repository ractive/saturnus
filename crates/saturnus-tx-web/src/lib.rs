#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! WebAssembly bindings of saturnus tx for the transfer page
//! (`web/tx/`, `kb/docs/saturnus-tx.md`), built by `web/tx-build.sh` into
//! `web/tx/pkg`:
//!
//! - [`TxDevice`]: `saturnus-tx`'s [`Device`] over a link the page holds.
//!   The page (its protocol Worker, `web/tx/protocol-worker.js`) feeds it
//!   bytes, link errors and the time in milliseconds, and takes from
//!   [`TxDevice::drain`] what to do: open the port with these settings,
//!   write these bytes, the events and the request's result. The requests
//!   are [`parse_request`]'s; what replaces or removes something is its
//!   own request, sent after the page has asked (a put or tree put with
//!   `replace`, a purge with the directory's `count`, a restore).
//! - Copying between devices: [`copy_plan`] (the steps, the names in use,
//!   the warning for the other family) and [`header_check`] for a file.
//! - The preview: [`describe_file`] (a binary transfer file as the typed
//!   object the shared `objects.js` shows), [`text_of`] (a `%%HP:` text
//!   file's body with the trigraphs as characters), and the file names
//!   ([`file_kind`], [`var_name`], [`file_name`], [`folder_name`],
//!   [`dir_name`], [`backup_file_name`]).
//!
//! The page's JavaScript tests of these bindings are
//! `web/test/tx-bindings.test.mjs`.
//!
//! The logic is in plain Rust functions over `serde_json` values, tested
//! natively; the wasm wrappers only convert.

use std::io;

use js_sys::{Array, Reflect, Uint8Array};
use saturnus_tx::copy::{self, Mismatch};
use saturnus_tx::device::ConnectOptions;
use saturnus_tx::names::{self, FileKind};
use saturnus_tx::{
    Action, Connection, Device, DeviceOptions, Entry, Event, Format, LinkSettings, Model, Outcome,
    Parity, Progress, Request, TreeItem, TxError,
};
use satx_core::object::Family;
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

/// What comes with a request besides its JSON: a file's bytes, or a
/// tree's items.
#[derive(Debug, Default)]
pub enum Payload {
    /// Nothing.
    #[default]
    None,
    /// A file (put, restore).
    Bytes(Vec<u8>),
    /// A tree (put-tree): each item's path as file-system names (folders,
    /// and the file names with their extension last) and its bytes, none
    /// for a folder.
    Items(Vec<(Vec<String>, Option<Vec<u8>>)>),
}

/// A tree item from file-system names: a folder's name is the
/// directory's (`%XX` read back), a file's the variable's without its
/// extension ([`names::var_name`]).
pub fn tree_item(path: &[String], data: Option<Vec<u8>>) -> Result<TreeItem, String> {
    let last = path.len().saturating_sub(1);
    let path = path
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == last && data.is_some() {
                names::var_name(p)
            } else {
                names::dir_name(p)
            }
            .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<String>, String>>()?;
    Ok(TreeItem { path, data })
}

/// The file-system names of a tree item: folders as [`names::folder_name`],
/// a file as [`names::file_name`].
pub fn fs_path(item: &TreeItem, format: Format) -> Vec<String> {
    let last = item.path.len().saturating_sub(1);
    item.path
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == last && !item.is_directory() {
                names::file_name(p, format)
            } else {
                names::folder_name(p)
            }
        })
        .collect()
}

fn format_of(msg: &Value) -> Result<Format, String> {
    match msg.get("format").and_then(Value::as_str) {
        None | Some("binary") => Ok(Format::Binary),
        Some("text") => Ok(Format::Text),
        Some(f) => Err(format!("no format {f:?}")),
    }
}

/// The request a page message names:
///
/// - `{type: "connect", find, baud, parity}`, `{type: "disconnect"}`
/// - `{type: "list", dir}`, `{type: "get", dir, name, format: "binary" |
///   "text"}`, `{type: "put", dir, name, replace, asString}` (the file is
///   the payload)
/// - `{type: "mkdir", dir, name}`, `{type: "rename", dir, from, to}`,
///   `{type: "purge", dir, name, count?}` (`count`: the variables a
///   directory holds, as the `notEmpty` error told them)
/// - `{type: "get-tree", dir, format}`, `{type: "put-tree", dir, name?,
///   replace, asString}` (the items are the payload: `[{path, data?}]`,
///   file-system names, see [`tree_item`])
/// - `{type: "memory"}`, `{type: "backup"}`, `{type: "restore"}` (the
///   backup file is the payload), `{type: "pict"}`
pub fn parse_request(msg: &Value, data: Payload) -> Result<Request, String> {
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
    let text = |key: &str| -> Result<String, String> {
        msg.get(key)
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("{key} is needed"))
    };
    let mut data = data;
    let mut bytes = |what: &str| match std::mem::take(&mut data) {
        Payload::Bytes(b) => Ok(b),
        _ => Err(format!("{what} needs the file's bytes")),
    };
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
            format: format_of(msg)?,
        },
        "put" => Request::Put {
            dir: dir()?,
            name: name()?,
            data: bytes("a put")?,
            replace: flag("replace"),
            as_string: flag("asString"),
        },
        "disconnect" => Request::Disconnect,
        "mkdir" => Request::Mkdir {
            dir: dir()?,
            name: name()?,
        },
        "rename" => Request::Rename {
            dir: dir()?,
            from: text("from")?,
            to: text("to")?,
        },
        "purge" => Request::Purge {
            dir: dir()?,
            name: name()?,
            count: match msg.get("count") {
                None | Some(Value::Null) => None,
                Some(c) => Some(
                    c.as_u64()
                        .and_then(|c| usize::try_from(c).ok())
                        .ok_or("count is a number of variables")?,
                ),
            },
        },
        "get-tree" => Request::GetTree {
            dir: dir()?,
            format: format_of(msg)?,
            skip: match msg.get("skip") {
                None | Some(Value::Null) => Vec::new(),
                Some(Value::Array(kinds)) => kinds
                    .iter()
                    .map(|k| {
                        k.as_str()
                            .map(str::to_string)
                            .ok_or("skip is a list of kinds")
                    })
                    .collect::<Result<_, _>>()?,
                Some(_) => return Err("skip is a list of kinds".to_string()),
            },
        },
        "put-tree" => {
            let Payload::Items(items) = std::mem::take(&mut data) else {
                return Err("a tree put needs its items".to_string());
            };
            Request::PutTree {
                dir: dir()?,
                name: match msg.get("name") {
                    None | Some(Value::Null) => None,
                    Some(_) => Some(name()?),
                },
                items: items
                    .into_iter()
                    .map(|(path, data)| tree_item(&path, data))
                    .collect::<Result<_, _>>()?,
                replace: flag("replace"),
                as_string: flag("asString"),
            }
        }
        "memory" => Request::Memory,
        "backup" => Request::Backup,
        "restore" => Request::Restore {
            data: bytes("a restore")?,
        },
        "pict" => Request::Pict,
        other => return Err(format!("no request {other:?}")),
    })
}

fn model_name(model: Model) -> &'static str {
    match model {
        Model::Hp48Sx => "48sx",
        Model::Hp48Gx => "48gx",
        Model::Hp49G => "49g",
        Model::Unknown => "unknown",
    }
}

/// The model [`model_name`] names.
fn model_of(name: &str) -> Result<Model, String> {
    match name {
        "48sx" => Ok(Model::Hp48Sx),
        "48gx" => Ok(Model::Hp48Gx),
        "49g" => Ok(Model::Hp49G),
        other => Err(format!("no model {other:?}")),
    }
}

fn family_name(f: Family) -> &'static str {
    match f {
        Family::Hp48 => "48",
        Family::Hp49 => "49",
    }
}

fn mismatch_json(m: Mismatch, message: &str) -> Value {
    json!({"from": family_name(m.from), "to": family_name(m.to), "message": message})
}

fn entry_json(e: &Entry) -> Value {
    json!({
        "name": e.name,
        "kind": e.kind,
        "size": e.size,
        "checksum": e.checksum,
        "directory": e.is_directory(),
    })
}

fn format_name(f: Format) -> &'static str {
    match f {
        Format::Binary => "binary",
        Format::Text => "text",
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

/// Bytes that go with an outcome to the page as `Uint8Array`s.
#[derive(Debug, PartialEq, Eq)]
pub enum Attached {
    /// None.
    None,
    /// The result's `data`.
    Data(Vec<u8>),
    /// Each `outcome.items[i].data` (none for a directory).
    Items(Vec<Option<Vec<u8>>>),
}

/// An outcome as JSON, and its bytes apart ([`Attached`]):
///
/// - `connected` (`connection`), `listing` (`path`, `free`, `entries`),
///   `file` (`name`, `fileName`, `format`; data), `stored` (`name`),
///   `disconnected`
/// - `created` (`name`), `renamed` (`from`, `to`), `purged` (`name`,
///   `count`)
/// - `tree` (`dir`, `items`: `{path, fsPath, directory}`, each file's
///   bytes as its `data`), `treeStored` (`directories`, `variables`)
/// - `memory` (`free`, `home`, `needed`, `fits`), `backup` (`model`, the
///   memory; data), `restored` (`message`: the calculator restarted),
///   `pict` (`width`, `height`; data, a GROB file)
pub fn outcome_json(o: Outcome) -> (Value, Attached) {
    let memory = |m: saturnus_tx::Memory| json!({"free": m.free, "home": m.home, "needed": m.needed, "fits": m.fits()});
    match o {
        Outcome::Connected(c) => (
            json!({"type": "connected", "connection": connection_json(&c)}),
            Attached::None,
        ),
        Outcome::Listing(l) => (
            json!({
                "type": "listing",
                "path": l.path,
                "free": l.free,
                "entries": l.entries.iter().map(entry_json).collect::<Vec<_>>(),
            }),
            Attached::None,
        ),
        Outcome::File(f) => (
            json!({
                "type": "file",
                "name": f.name,
                "fileName": f.file_name,
                "format": format_name(f.format),
            }),
            Attached::Data(f.data),
        ),
        Outcome::Stored(name) => (json!({"type": "stored", "name": name}), Attached::None),
        Outcome::Disconnected => (json!({"type": "disconnected"}), Attached::None),
        Outcome::Created(name) => (json!({"type": "created", "name": name}), Attached::None),
        Outcome::Renamed { from, to } => (
            json!({"type": "renamed", "from": from, "to": to}),
            Attached::None,
        ),
        Outcome::Purged { name, count } => (
            json!({"type": "purged", "name": name, "count": count}),
            Attached::None,
        ),
        Outcome::Tree(t) => {
            let items: Vec<Value> = t
                .items
                .iter()
                .map(|i| {
                    // A got file is a binary transfer file or text.
                    let format = match i.data.as_deref().map(FileKind::of) {
                        Some(FileKind::Text) => Format::Text,
                        _ => Format::Binary,
                    };
                    json!({
                        "path": i.path,
                        "fsPath": fs_path(i, format),
                        "directory": i.is_directory(),
                    })
                })
                .collect();
            let data = t.items.into_iter().map(|i| i.data).collect();
            (
                json!({
                    "type": "tree",
                    "dir": t.dir,
                    "items": items,
                    "skipped": t.skipped.iter().map(|(path, kind)| json!({"path": path, "kind": kind})).collect::<Vec<_>>(),
                }),
                Attached::Items(data),
            )
        }
        Outcome::TreeStored {
            directories,
            variables,
        } => (
            json!({"type": "treeStored", "directories": directories, "variables": variables}),
            Attached::None,
        ),
        Outcome::Memory(m) => {
            let mut v = memory(m);
            v["type"] = json!("memory");
            (v, Attached::None)
        }
        Outcome::Backup(b) => {
            let mut v = memory(b.memory);
            v["type"] = json!("backup");
            v["model"] = json!(model_name(b.model));
            (v, Attached::Data(b.data))
        }
        Outcome::Restored => (
            json!({
                "type": "restored",
                "message": "The calculator restarted and left server mode. Start SERVER on it, then reconnect.",
            }),
            Attached::None,
        ),
        Outcome::Pict(p) => (
            json!({"type": "pict", "width": p.width, "height": p.height}),
            Attached::Data(p.data),
        ),
        _ => (json!({"type": "other"}), Attached::None),
    }
}

/// An error as JSON: `{kind, message, name?}`, `kind` one of `busy`,
/// `notConnected`, `alreadyConnected`, `noAnswer`, `nameClash`,
/// `isDirectory`, `notAnObject`, `badName`, `notFound`, `notEmpty`
/// (`count`: send the purge again with it once the user agreed),
/// `namesInUse` (`names`: send the tree put again with `replace`),
/// `notADirectory`, `duplicate`, `noRoom` (`needed`, `free`),
/// `restarted` (reconnect), `calculator`.
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
        TxError::NotFound(n) => ("notFound", Some(n.clone())),
        TxError::NotEmpty { name, .. } => ("notEmpty", Some(name.clone())),
        TxError::NamesInUse(_) => ("namesInUse", None),
        TxError::NotADirectory(n) => ("notADirectory", Some(n.clone())),
        TxError::Duplicate(n) => ("duplicate", Some(n.clone())),
        TxError::NoRoom { .. } => ("noRoom", None),
        TxError::Restarted => ("restarted", None),
        TxError::Partial { .. } => ("partial", None),
        _ => ("calculator", None),
    };
    let mut v = json!({"kind": kind, "message": e.to_string(), "name": name});
    match e {
        TxError::NotEmpty { count, .. } => v["count"] = json!(count),
        TxError::NamesInUse(names) => v["names"] = json!(names),
        TxError::NoRoom { needed, free } => {
            v["needed"] = json!(needed);
            v["free"] = json!(free);
        }
        TxError::Partial {
            directories,
            variables,
            root_made,
            source,
        } => {
            v["directories"] = json!(directories);
            v["variables"] = json!(variables);
            v["rootMade"] = json!(root_made);
            v["source"] = error_json(source);
        }
        _ => {}
    }
    v
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
        Event::Item(path) => json!({"type": "item", "path": path}),
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
        // Rounded, not cut: the page's clock at a deadline it was given
        // back (`base_ms + ms` in floating point) must reach it.
        self.base + Duration::from_micros((ms * 1000.0).round() as u64)
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

    /// Begin a request (see [`parse_request`]); `data` is a put's or a
    /// restore's file (`Uint8Array`), or a tree put's items (an array of
    /// `{path, data?}`). Throws for a malformed request or while one runs.
    pub fn start(&mut self, now_ms: f64, request: &str, data: JsValue) -> Result<(), JsValue> {
        let msg: Value = serde_json::from_str(request).map_err(js_err)?;
        let request = parse_request(&msg, payload(&data)?).map_err(js_err)?;
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

    /// Whether a restore has restarted the calculator since the last
    /// connect: it has left server mode, so every request but `connect`
    /// fails with `restarted` until `SERVER` runs on it again.
    pub fn restarted(&self) -> bool {
        self.device.restarted()
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
                    let (o, attached) = outcome_json(outcome);
                    let v = to_js(&json!({"type": "result", "ok": true, "outcome": o}))?;
                    match attached {
                        Attached::None => {}
                        Attached::Data(d) => {
                            Reflect::set(&v, &"data".into(), &Uint8Array::from(d.as_slice()))?;
                        }
                        Attached::Items(items) => {
                            let outcome = Reflect::get(&v, &"outcome".into())?;
                            let list = Array::from(&Reflect::get(&outcome, &"items".into())?);
                            for (i, d) in items.iter().enumerate() {
                                if let Some(d) = d {
                                    let item = list.get(i as u32);
                                    Reflect::set(
                                        &item,
                                        &"data".into(),
                                        &Uint8Array::from(d.as_slice()),
                                    )?;
                                }
                            }
                        }
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

/// A start's `data` as a [`Payload`].
fn payload(data: &JsValue) -> Result<Payload, JsValue> {
    if data.is_undefined() || data.is_null() {
        return Ok(Payload::None);
    }
    if data.is_instance_of::<Uint8Array>() {
        return Ok(Payload::Bytes(Uint8Array::new(data).to_vec()));
    }
    if Array::is_array(data) {
        let mut items = Vec::new();
        for item in Array::from(data).iter() {
            let path = Reflect::get(&item, &"path".into())?;
            if !Array::is_array(&path) {
                return Err(js_err("a tree item's path is a list of names"));
            }
            let path = Array::from(&path)
                .iter()
                .map(|p| p.as_string().ok_or_else(|| js_err("a name is a string")))
                .collect::<Result<Vec<String>, JsValue>>()?;
            let bytes = Reflect::get(&item, &"data".into())?;
            let bytes = if bytes.is_undefined() || bytes.is_null() {
                None
            } else if bytes.is_instance_of::<Uint8Array>() {
                Some(Uint8Array::new(&bytes).to_vec())
            } else {
                return Err(js_err("a tree item's data is a Uint8Array"));
            };
            items.push((path, bytes));
        }
        return Ok(Payload::Items(items));
    }
    Err(js_err("data is a Uint8Array or a list of tree items"))
}

// ------------------------------------------------------------ copying

/// The plan for a copy (see `saturnus_tx::copy`) from the page's JSON
/// `{from: "48gx" | null (a folder), to, selection: [entry], target:
/// [entry]}` (entries as in a listing: `name`, `kind`, `size`,
/// `checksum`): `{steps: [{name, directory, format, get}], inUse,
/// mismatch: {from, to, message} | null}`; `get` is the request that gets
/// the step's variable or directory from `fromDir`.
pub fn copy_plan_json(msg: &Value) -> Result<Value, String> {
    let from = match msg.get("from") {
        None | Some(Value::Null) => None,
        Some(v) => Some(model_of(v.as_str().unwrap_or(""))?),
    };
    let to = model_of(msg.get("to").and_then(Value::as_str).unwrap_or(""))?;
    let entries = |key: &str| -> Result<Vec<Entry>, String> {
        let Some(list) = msg.get(key).and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        list.iter()
            .map(|e| {
                Ok(Entry {
                    name: e
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| format!("{key}: an entry needs its name"))?
                        .to_string(),
                    size: e.get("size").and_then(Value::as_f64).unwrap_or(0.0),
                    kind: e
                        .get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    checksum: e
                        .get("checksum")
                        .and_then(Value::as_u64)
                        .and_then(|c| u16::try_from(c).ok())
                        .unwrap_or(0),
                })
            })
            .collect()
    };
    let from_dir: Vec<String> = match msg.get("fromDir").and_then(Value::as_array) {
        Some(parts) => parts
            .iter()
            .map(|p| {
                p.as_str()
                    .map(str::to_string)
                    .ok_or("a directory is a list of names")
            })
            .collect::<Result<_, _>>()?,
        None => vec!["HOME".to_string()],
    };
    let plan = copy::plan(from, to, &entries("selection")?, &entries("target")?);
    let steps: Vec<Value> = plan
        .steps
        .iter()
        .map(|s| {
            let get = match s.get(&from_dir) {
                Request::GetTree { dir, format, skip } => {
                    json!({"type": "get-tree", "dir": dir, "format": format_name(format), "skip": skip})
                }
                Request::Get { dir, name, format } => {
                    json!({"type": "get", "dir": dir, "name": name, "format": format_name(format)})
                }
                _ => Value::Null,
            };
            json!({
                "name": s.name,
                "directory": s.directory,
                "format": format_name(s.format),
                "get": get,
            })
        })
        .collect();
    Ok(json!({
        "steps": steps,
        "inUse": plan.in_use,
        "blocked": plan.blocked,
        "untranslatable": plan.untranslatable,
        "mismatch": plan.mismatch.map(|m| mismatch_json(m, m.message())),
        "warning": plan.warning,
    }))
}

/// [`copy_plan_json`] for the page: a JSON string in, an object out.
#[wasm_bindgen]
pub fn copy_plan(request: &str) -> Result<JsValue, JsValue> {
    let msg: Value = serde_json::from_str(request).map_err(js_err)?;
    to_js(&copy_plan_json(&msg).map_err(js_err)?)
}

/// Whether the file `data` put to a `model` (`"48sx"`, `"48gx"`, `"49g"`)
/// crosses families: `{from, to, message}` (the calculator stores it as a
/// string), else `null`. Throws for an unknown model.
#[wasm_bindgen]
pub fn header_check(data: &[u8], model: &str) -> Result<JsValue, JsValue> {
    let model = model_of(model).map_err(js_err)?;
    match copy::header_check(data, model) {
        Some(m) => to_js(&mismatch_json(m, m.file_message())),
        None => Ok(JsValue::NULL),
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

/// The folder name of directory `name` (`%XX` for what a file system
/// refuses).
#[wasm_bindgen]
pub fn folder_name(name: &str) -> String {
    names::folder_name(name)
}

/// The directory name of a folder named `folder` (`%XX` read back, dots
/// kept); throws when that is not UTF-8.
#[wasm_bindgen]
pub fn dir_name(folder: &str) -> Result<String, JsValue> {
    names::dir_name(folder).map_err(js_err)
}

/// A backup's file name, `48GX-backup-2026-10-10-1432.hp`, for `model`
/// (`"48sx"`, `"48gx"`, `"49g"`) and the local date and time.
#[wasm_bindgen]
pub fn backup_file_name(
    model: &str,
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
) -> Result<String, JsValue> {
    let model = model_of(model).map_err(js_err)?;
    Ok(names::backup_file_name(
        model, year, month, day, hour, minute,
    ))
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
        let r = parse_request(&json!({"type": "connect"}), Payload::None).unwrap();
        assert!(matches!(r, Request::Connect(o) if o.find && o.settings == LinkSettings::DEFAULT));
        let r = parse_request(
            &json!({"type": "connect", "find": false, "baud": 2400, "parity": "odd"}),
            Payload::None,
        )
        .unwrap();
        assert!(
            matches!(r, Request::Connect(o) if !o.find && o.settings.baud == 2400 && o.settings.parity == Parity::Odd)
        );
        assert!(parse_request(&json!({"type": "connect", "baud": 1234}), Payload::None).is_err());
        let r = parse_request(
            &json!({"type": "list", "dir": ["HOME", "D"]}),
            Payload::None,
        )
        .unwrap();
        assert!(matches!(r, Request::List { dir } if dir == ["HOME", "D"]));
        let r = parse_request(
            &json!({"type": "get", "name": "A", "format": "text"}),
            Payload::None,
        )
        .unwrap();
        assert!(
            matches!(r, Request::Get { name, format: Format::Text, dir } if name == "A" && dir == ["HOME"])
        );
        assert!(parse_request(&json!({"type": "get"}), Payload::None).is_err());
        assert!(parse_request(&json!({"type": "put", "name": "A"}), Payload::None).is_err());
        let r = parse_request(
            &json!({"type": "put", "name": "A", "replace": true}),
            Payload::Bytes(vec![1]),
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
        assert!(parse_request(&json!({"type": "nope"}), Payload::None).is_err());
    }

    /// The page's clock at a deadline the device gave it reaches that
    /// deadline, whatever floating point does to `base + ms` (a 0 ms timer
    /// again and again otherwise).
    #[test]
    fn the_clock_reaches_its_deadline() {
        for base in [1000.0, 0.1, 123_456.789] {
            let mut d = TxDevice::new(base, 1.0, None);
            let now = d.at(base + 6690.0 - 1e-9);
            d.device
                .start(now, Request::Connect(Default::default()))
                .unwrap();
            let deadline = d.deadline().unwrap();
            assert!(d.at(deadline) >= d.device.next_timeout().unwrap(), "{base}");
        }
    }

    #[test]
    fn the_new_requests_parse() {
        let p = |v: Value| parse_request(&v, Payload::None);
        assert!(matches!(
            p(json!({"type": "mkdir", "name": "D"})).unwrap(),
            Request::Mkdir { name, dir } if name == "D" && dir == ["HOME"]
        ));
        assert!(matches!(
            p(json!({"type": "rename", "from": "A", "to": "B"})).unwrap(),
            Request::Rename { from, to, .. } if from == "A" && to == "B"
        ));
        assert!(p(json!({"type": "rename", "from": "A"})).is_err());
        assert!(matches!(
            p(json!({"type": "purge", "name": "D", "count": 4})).unwrap(),
            Request::Purge { count: Some(4), .. }
        ));
        assert!(matches!(
            p(json!({"type": "purge", "name": "D"})).unwrap(),
            Request::Purge { count: None, .. }
        ));
        assert!(p(json!({"type": "purge", "name": "D", "count": -1})).is_err());
        assert!(matches!(
            p(json!({"type": "get-tree", "dir": ["HOME", "D"], "format": "text", "skip": ["Code"]})).unwrap(),
            Request::GetTree { format: Format::Text, skip, .. } if skip == ["Code"]
        ));
        assert!(p(json!({"type": "get-tree", "skip": "Code"})).is_err());
        for (t, want) in [("memory", "Memory"), ("backup", "Backup"), ("pict", "Pict")] {
            assert!(format!("{:?}", p(json!({"type": t})).unwrap()).contains(want));
        }
        assert!(p(json!({"type": "restore"})).is_err());
        assert!(matches!(
            parse_request(&json!({"type": "restore"}), Payload::Bytes(vec![1])).unwrap(),
            Request::Restore { data } if data == [1]
        ));
        assert!(p(json!({"type": "put-tree", "name": "D"})).is_err());
        let items = Payload::Items(vec![
            (vec!["A%2FB.C".into()], None),
            (vec!["A%2FB.C".into(), "X.hp".into()], Some(vec![1])),
        ]);
        let r = parse_request(&json!({"type": "put-tree", "replace": true}), items).unwrap();
        let Request::PutTree {
            name: None,
            items,
            replace: true,
            ..
        } = r
        else {
            panic!("{r:?}");
        };
        // A folder keeps its dot; a file loses its extension.
        assert_eq!(items[0].path, ["A/B.C"]);
        assert_eq!(items[1].path, ["A/B.C", "X"]);
    }

    #[test]
    fn trees_and_errors_as_json() {
        let tree = Outcome::Tree(saturnus_tx::Tree {
            dir: vec!["HOME".into(), "D".into()],
            items: vec![
                TreeItem {
                    path: vec!["S/T".into()],
                    data: None,
                },
                TreeItem {
                    path: vec!["S/T".into(), "X".into()],
                    data: Some(b"HPHP48-R".to_vec()),
                },
            ],
            skipped: vec![(vec!["L".into()], "Library".into())],
        });
        let (v, attached) = outcome_json(tree);
        assert_eq!(v["skipped"], json!([{"path": ["L"], "kind": "Library"}]));
        assert_eq!(v["items"][1]["fsPath"], json!(["S%2FT", "X.hp"]));
        assert_eq!(v["items"][0]["directory"], true);
        assert_eq!(
            attached,
            Attached::Items(vec![None, Some(b"HPHP48-R".to_vec())])
        );
        let v = error_json(&TxError::NotEmpty {
            name: "D".into(),
            count: 4,
        });
        assert_eq!(
            (v["kind"].clone(), v["count"].clone()),
            (json!("notEmpty"), json!(4))
        );
        let v = error_json(&TxError::NamesInUse(vec!["D/X".into()]));
        assert_eq!(v["names"], json!(["D/X"]));
        let v = error_json(&TxError::NoRoom {
            needed: 9.0,
            free: 1.0,
        });
        assert_eq!(
            (v["needed"].clone(), v["free"].clone()),
            (json!(9.0), json!(1.0))
        );
        assert_eq!(error_json(&TxError::Restarted)["kind"], "restarted");
        let v = error_json(&TxError::Partial {
            directories: 2,
            variables: 1,
            root_made: true,
            source: Box::new(TxError::NotAnObject),
        });
        assert_eq!(v["kind"], "partial");
        assert_eq!(
            (
                v["directories"].clone(),
                v["variables"].clone(),
                v["rootMade"].clone()
            ),
            (json!(2), json!(1), json!(true))
        );
        assert_eq!(v["source"]["kind"], "notAnObject");
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("1 variable(s) and 2")
        );
        let (v, _) = outcome_json(Outcome::Restored);
        assert!(v["message"].as_str().unwrap().contains("reconnect"));
    }

    #[test]
    fn a_copy_plan_as_json() {
        let v = copy_plan_json(&json!({
            "from": "48gx",
            "to": "49g",
            "selection": [{"name": "P", "kind": "Program"}, {"name": "D", "kind": "Directory"}],
            "target": [{"name": "P", "kind": "String"}],
        }))
        .unwrap();
        assert_eq!(v["inUse"], json!(["P"]));
        assert_eq!(v["blocked"], json!([]));
        assert_eq!(v["untranslatable"], json!([]));
        assert!(v["warning"].as_str().unwrap().contains("as text"));
        assert!(
            v["steps"][1]["get"]["skip"]
                .as_array()
                .unwrap()
                .contains(&json!("Code"))
        );
        assert_eq!(v["mismatch"]["from"], "48");
        assert_eq!(v["steps"][0]["get"]["format"], "text");
        assert_eq!(v["steps"][1]["get"]["type"], "get-tree");
        assert_eq!(v["steps"][1]["get"]["dir"], json!(["HOME", "D"]));
        let same = copy_plan_json(&json!({"from": null, "to": "48sx", "selection": []})).unwrap();
        assert_eq!(same["mismatch"], Value::Null);
        assert!(copy_plan_json(&json!({"to": "42s"})).is_err());
        assert_eq!(
            names::backup_file_name(model_of("49g").unwrap(), 2026, 10, 11, 9, 5),
            "49G-backup-2026-10-11-0905.hp"
        );
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
