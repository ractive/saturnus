//! The aplet "disk drive" for the page: [`TxApletServer`], saturnus-tx's
//! [`ApletServer`] over a link the page holds, and [`aplet_info`], what a
//! file's first bytes say (`web/tx/serve.js`, `web/tx/aplet-worker.js`).
//!
//! As [`crate::tx::TxDevice`], every call takes the page's clock in
//! milliseconds; [`TxApletServer::drain`] hands out the writes and the
//! events in order.

use js_sys::{Array, Reflect, Uint8Array};
use saturnus_tx::Model;
use saturnus_tx::aplets::{ApletEvent, ApletOptions, ApletServer, Family, FolderFile, Header};
use satx_core::time::{Duration, Instant};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::tx::{js_err, model_name, to_js};

/// An event as the page's JSON, and the bytes that go with it (a received
/// file).
pub fn aplet_event_json(e: ApletEvent) -> (Value, Option<Vec<u8>>) {
    let v = match e {
        ApletEvent::Model(m) => json!({"type": "model", "model": model_name(m)}),
        ApletEvent::NeedFile { name } => json!({"type": "needFile", "name": name}),
        ApletEvent::Sending { name, done, total } => {
            json!({"type": "sending", "name": name, "done": done, "total": total})
        }
        ApletEvent::Sent { name, directory } => {
            json!({"type": "sent", "name": name, "directory": directory})
        }
        ApletEvent::NotFound { name } => json!({"type": "notFound", "name": name}),
        ApletEvent::Receiving { name, done } => {
            json!({"type": "receiving", "name": name, "done": done})
        }
        ApletEvent::Received {
            name,
            data,
            directory,
        } => {
            let title = Header::read(&data).and_then(|h| h.title);
            return (
                json!({"type": "received", "name": name, "directory": directory, "title": title, "size": data.len()}),
                Some(data),
            );
        }
        ApletEvent::Discarded { name } => json!({"type": "discarded", "name": name}),
        ApletEvent::Refused { name } => json!({"type": "refusedFile", "name": name}),
        ApletEvent::CalculatorSays(text) => json!({"type": "calculatorSays", "text": text}),
        ApletEvent::Failed(message) => json!({"type": "failed", "message": message}),
        ApletEvent::WireStarted => json!({"type": "wireStarted"}),
        ApletEvent::WireProgress { done, total } => {
            json!({"type": "wireProgress", "done": done, "total": total})
        }
        ApletEvent::WireDone => json!({"type": "wireDone"}),
        ApletEvent::WireFailed(message) => json!({"type": "wireFailed", "message": message}),
        _ => json!({"type": "other"}),
    };
    (v, None)
}

/// What the first bytes of a file say: `{family: "38" | "39", binary,
/// kind, title, wire}` (`wire`: a binary 39G/40G aplet, which XModem
/// takes), or `null` for a file that is not a calculator's.
pub fn aplet_info_json(head: &[u8]) -> Value {
    match Header::read(head) {
        Some(h) => json!({
            "family": match h.family { Family::Hp38 => "38", Family::Hp39 => "39" },
            "binary": h.binary,
            "kind": char::from(h.kind).to_string(),
            "title": h.title,
            "wire": h.is_wire_aplet(),
        }),
        None => Value::Null,
    }
}

/// [`aplet_info_json`] for the page.
#[wasm_bindgen]
pub fn aplet_info(head: &[u8]) -> Result<JsValue, JsValue> {
    to_js(&aplet_info_json(head))
}

/// The page's folder files: `[{name, head: Uint8Array}]`.
fn folder_files(files: &JsValue) -> Result<Vec<FolderFile>, JsValue> {
    if !Array::is_array(files) {
        return Err(js_err("the folder's files are a list of {name, head}"));
    }
    Array::from(files)
        .iter()
        .map(|f| {
            let name = Reflect::get(&f, &"name".into())?
                .as_string()
                .ok_or_else(|| js_err("a file's name is a string"))?;
            let head = Reflect::get(&f, &"head".into())?;
            let head = if head.is_instance_of::<Uint8Array>() {
                Uint8Array::new(&head).to_vec()
            } else {
                Vec::new()
            };
            Ok(FolderFile { name, head })
        })
        .collect()
}

/// saturnus-tx's [`ApletServer`] over a link the page holds.
#[wasm_bindgen]
pub struct TxApletServer {
    server: ApletServer,
    base: Instant,
    base_ms: f64,
}

impl std::fmt::Debug for TxApletServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxApletServer")
            .field("server", &self.server)
            .finish()
    }
}

impl TxApletServer {
    fn at(&self, now_ms: f64) -> Instant {
        let ms = (now_ms - self.base_ms).max(0.0);
        self.base + Duration::from_micros((ms * 1000.0).round() as u64)
    }
}

#[wasm_bindgen]
impl TxApletServer {
    /// Serve a folder: `files` as `[{name, head}]`, `head` a file's first
    /// 264 bytes (all of a directory file, `HP38DIR.*` or `HP39DIR.*`).
    #[wasm_bindgen(constructor)]
    pub fn new(now_ms: f64, files: JsValue) -> Result<TxApletServer, JsValue> {
        let files = folder_files(&files)?;
        Ok(TxApletServer {
            server: ApletServer::new(ApletOptions::default(), files),
            base: Instant::now(),
            base_ms: now_ms,
        })
    }

    /// The folder changed: its files now, as for the constructor.
    #[wasm_bindgen(js_name = setFiles)]
    pub fn set_files(&mut self, files: JsValue) -> Result<(), JsValue> {
        self.server.set_files(folder_files(&files)?);
        Ok(())
    }

    /// Bytes read from the link.
    pub fn input(&mut self, now_ms: f64, bytes: &[u8]) {
        let now = self.at(now_ms);
        self.server.handle_input(now, bytes);
    }

    /// The time passed (the page's timer for [`TxApletServer::deadline`]).
    pub fn timeout(&mut self, now_ms: f64) {
        let now = self.at(now_ms);
        self.server.handle_timeout(now);
    }

    /// When to call [`TxApletServer::timeout`], on the page's clock;
    /// `undefined` while the server only waits for the calculator.
    pub fn deadline(&self) -> Option<f64> {
        let at = self.server.next_timeout()?;
        let ms = at.saturating_duration_since(self.base).as_secs_f64() * 1000.0;
        Some(self.base_ms + ms)
    }

    /// Answer a `needFile` event for file `name` (as the event gave it):
    /// the file's bytes, or `undefined` when it could not be read. Throws
    /// when the calculator no longer waits for that file.
    pub fn provide(
        &mut self,
        now_ms: f64,
        name: &str,
        data: Option<Vec<u8>>,
    ) -> Result<(), JsValue> {
        let now = self.at(now_ms);
        self.server.provide(now, name, data).map_err(js_err)
    }

    /// The page could not store a received file: the server forgets it.
    pub fn forget(&mut self, name: &str) {
        self.server.forget(name);
    }

    /// Send a binary aplet by XModem (the calculator: RECEIVE, "HP39/40
    /// (Wire)"). Throws while a transfer runs.
    #[wasm_bindgen(js_name = sendWire)]
    pub fn send_wire(&mut self, now_ms: f64, data: Vec<u8>) -> Result<(), JsValue> {
        let now = self.at(now_ms);
        self.server.send_wire(now, data).map_err(js_err)
    }

    /// Stop the running transfer (an XModem send, a Kermit transfer).
    pub fn cancel(&mut self, now_ms: f64) {
        let now = self.at(now_ms);
        self.server.cancel(now);
    }

    /// Whether a transfer runs or waits for the page.
    #[wasm_bindgen(js_name = isBusy)]
    pub fn is_busy(&self) -> bool {
        self.server.is_busy()
    }

    /// The calculator's model once known (`38g`, `39g`), else `undefined`.
    pub fn model(&self) -> Option<String> {
        self.server
            .model()
            .filter(|m| *m != Model::Unknown)
            .map(|m| model_name(m).to_string())
    }

    /// What the page does next, in order: `{type: "write", bytes}` and the
    /// events (`model`, `needFile`, `sending`, `sent`, `notFound`,
    /// `receiving`, `received` with `data`, `discarded`, `calculatorSays`,
    /// `failed`, `wireStarted`, `wireProgress`, `wireDone`, `wireFailed`).
    pub fn drain(&mut self, now_ms: f64) -> Result<Array, JsValue> {
        let now = self.at(now_ms);
        let out = Array::new();
        loop {
            let mut any = false;
            while let Some(bytes) = self.server.poll_output(now) {
                any = true;
                let v = to_js(&json!({"type": "write"}))?;
                Reflect::set(&v, &"bytes".into(), &Uint8Array::from(bytes.as_slice()))?;
                out.push(&v);
            }
            while let Some(e) = self.server.poll_event() {
                any = true;
                let (v, data) = aplet_event_json(e);
                let v = to_js(&v)?;
                if let Some(d) = data {
                    Reflect::set(&v, &"data".into(), &Uint8Array::from(d.as_slice()))?;
                }
                out.push(&v);
            }
            if !any {
                return Ok(out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_as_json() {
        let (v, d) = aplet_event_json(ApletEvent::Model(Model::Hp38G));
        assert_eq!(v, json!({"type": "model", "model": "38g"}));
        assert!(d.is_none());
        let (v, d) = aplet_event_json(ApletEvent::Received {
            name: "XYZ00000.000".into(),
            data: b"HP39BinB\x03XYZ".to_vec(),
            directory: false,
        });
        assert_eq!(v["title"], json!("XYZ"));
        assert_eq!(v["size"], json!(12));
        assert_eq!(d.unwrap().len(), 12);
        let (v, _) = aplet_event_json(ApletEvent::CalculatorSays("Transfer Failed".into()));
        assert_eq!(
            v,
            json!({"type": "calculatorSays", "text": "Transfer Failed"})
        );
    }

    #[test]
    fn info() {
        assert_eq!(
            aplet_info_json(b"HP39BinB\x03XYZ\x00"),
            json!({"family": "39", "binary": true, "kind": "B", "title": "XYZ", "wire": true})
        );
        assert_eq!(
            aplet_info_json(b"HP38AscA "),
            json!({"family": "38", "binary": false, "kind": "A", "title": null, "wire": false})
        );
        assert_eq!(aplet_info_json(b"HPHP48-R"), Value::Null);
    }
}
