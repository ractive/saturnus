#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! WebAssembly bindings of the saturnus core for the browser UI in `web/`:
//! a thin layer over [`saturnus_host`], whose protocol state machine
//! (`saturnus_host::protocol::Engine`) answers every command the Worker
//! does not keep for itself. Here only messages, errors and bytes cross
//! between Rust and JavaScript.
//!
//! [`Host`] is the engine with the Worker's pacing and its clock (a
//! JavaScript function, `performance.now`); the Worker (`web/worker.js`)
//! is its only caller and these are exactly the calls it makes:
//!
//! - `command(json, bytes, tag)`: one protocol message as JSON text, its
//!   *bytes* field (`rom`, `state`) apart, and a tag for its reply.
//! - `drain()`: the events and replies since, in order, ready to post (a
//!   reply carries its `tag`, and `saveState`'s `state` as a
//!   `Uint8Array`), and the auto-saved states for the Worker's store as
//!   `{type: "autoSave", model, romName, cycles, state}` (iteration 27;
//!   the Worker keeps them, the page never sees them).
//! - `deadline()` and `timer()`: when to call `timer` next, on the clock.
//! - `check(json)` and `boot(model, rom, name, kept)`: for the ROM slots
//!   the Worker keeps in IndexedDB (`romstore.js`): the version check and
//!   the refusals during a send, and the boot of a remembered ROM with the
//!   state the Worker kept for the model, if any.
//!
//! Free functions for the ROM slots: `model_names()`, and ROM
//! identification (see `saturnus_host::romid`): `identify_rom` tells a
//! known image (by SHA-256) from one that only fits by size, `plan_roms`
//! assigns a batch of them to the model slots, `rom_download` tells where
//! hpcalc.org offers a model's image (the page links to it).

use saturnus::Model;
use saturnus_host::protocol::{Clock, Engine, Output, Pacing};
use saturnus_host::romid;
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

/// The page's clock: a JavaScript function returning ms.
#[derive(Debug)]
struct JsClock(js_sys::Function);

impl Clock for JsClock {
    fn now_ms(&self) -> f64 {
        self.0
            .call0(&JsValue::NULL)
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    }
}

fn js_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// A typed answer of the host crate as a JavaScript value.
fn js_json<T: serde::Serialize>(v: &T) -> Result<JsValue, JsValue> {
    js_sys::JSON::parse(&serde_json::to_string(v).map_err(js_err)?)
}

/// The protocol's state machine with the Worker's pacing.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Host {
    engine: Engine,
    clock: JsClock,
}

#[wasm_bindgen]
impl Host {
    /// An engine with no machine yet; `now` is the clock (`() =>
    /// performance.now()`).
    #[wasm_bindgen(constructor)]
    pub fn new(now: js_sys::Function) -> Host {
        let mut engine = Engine::new("worker", Pacing::WORKER);
        engine.set_auto_save(true);
        engine.set_answer_recover(true);
        Host {
            engine,
            clock: JsClock(now),
        }
    }

    /// One protocol message (JSON text), with its *bytes* field apart and
    /// the tag its reply will carry (none: errors become `error` events).
    pub fn command(&mut self, msg: &str, bytes: Option<Vec<u8>>, tag: Option<f64>) {
        let tag = tag.map(|t| t as u64);
        match serde_json::from_str::<Value>(msg) {
            Ok(m) => self.engine.command(&self.clock, &m, bytes, tag),
            Err(e) => self.engine.answer(
                &self.clock,
                tag,
                Err(format!("not a JSON message: {e}").into()),
            ),
        }
    }

    /// Throws the version or refusal error of a message the Worker serves
    /// itself.
    pub fn check(&self, msg: &str) -> Result<(), JsValue> {
        let m: Value = serde_json::from_str(msg).map_err(js_err)?;
        self.engine.admit(&m).map(|_| ()).map_err(js_err)
    }

    /// Boot `model` from `rom` (a remembered ROM), restoring `kept` (the
    /// model's auto-saved state) if it loads; `{model, romName, restored?,
    /// restoreError?}`.
    pub fn boot(
        &mut self,
        model: &str,
        rom: &[u8],
        name: &str,
        kept: Option<Vec<u8>>,
    ) -> Result<JsValue, JsValue> {
        let booted = self
            .engine
            .boot_restoring(&self.clock, model, rom, name, kept.as_deref())
            .map_err(js_err)?;
        js_json(&booted)
    }

    /// Hand out the owed save now if the machine has settled (it comes
    /// with the next `drain`): the Worker stores it before a boot reads or
    /// clears the slot.
    #[wasm_bindgen(js_name = saveNow)]
    pub fn save_now(&mut self) {
        self.engine.save_now();
    }

    /// The host's timer fired.
    pub fn timer(&mut self) {
        self.engine.timer(&self.clock);
    }

    /// When `timer` is due, on the clock; `undefined` for never.
    pub fn deadline(&self) -> Option<f64> {
        self.engine.deadline()
    }

    /// The events and replies since the last call, in order: protocol
    /// messages, a reply as `{type: "reply", tag, ok, result | error}`.
    pub fn drain(&mut self) -> Result<js_sys::Array, JsValue> {
        let out = self.engine.take_output();
        if out.is_empty() {
            return Ok(js_sys::Array::new());
        }
        let mut bytes = Vec::new();
        let mut states = Vec::new();
        let mut texts = Vec::with_capacity(out.len());
        for (i, o) in out.into_iter().enumerate() {
            let v = match o {
                Output::Event(e) => serde_json::to_string(&e),
                Output::Save(s) => {
                    states.push((i, s.state));
                    serde_json::to_string(&json!({
                        "type": "autoSave",
                        "model": s.model,
                        "romName": s.rom_name,
                        "cycles": s.cycles,
                    }))
                }
                Output::Reply(r) => {
                    if let Some(b) = r.bytes {
                        bytes.push((i, b));
                    }
                    serde_json::to_string(&match r.result {
                        Ok(result) => {
                            json!({"type": "reply", "tag": r.tag, "ok": true, "result": result})
                        }
                        Err(error) => {
                            json!({"type": "reply", "tag": r.tag, "ok": false, "error": error})
                        }
                    })
                }
            };
            texts.push(v.map_err(js_err)?);
        }
        let all: js_sys::Array = js_sys::JSON::parse(&format!("[{}]", texts.join(",")))?.into();
        for (i, (field, b)) in bytes {
            let result = js_sys::Reflect::get(&all.get(i as u32), &"result".into())?;
            js_sys::Reflect::set(
                &result,
                &field.into(),
                &js_sys::Uint8Array::from(b.as_slice()),
            )?;
        }
        for (i, state) in states {
            js_sys::Reflect::set(
                &all.get(i as u32),
                &"state".into(),
                &js_sys::Uint8Array::from(state.as_slice()),
            )?;
        }
        Ok(all)
    }
}

/// The supported model names.
#[wasm_bindgen]
pub fn model_names() -> Vec<String> {
    Model::ALL.iter().map(|m| m.name().to_string()).collect()
}

/// `romid::identify` for the Worker, see `romid::rom_id_json`.
#[wasm_bindgen]
pub fn identify_rom(rom: &[u8]) -> Result<JsValue, JsValue> {
    js_json(&romid::rom_id_json(&romid::identify(rom)))
}

/// `romid::plan_json` for the Worker; `input` is JSON text.
#[wasm_bindgen]
pub fn plan_roms(input: &str) -> Result<JsValue, JsValue> {
    let v: Value = serde_json::from_str(input).map_err(js_err)?;
    js_json(&romid::plan_json(&v).map_err(js_err)?)
}

/// `romid::download_json` for the Worker: `{file, size, url, page,
/// revision}` or `null` (the 42S).
#[wasm_bindgen]
pub fn rom_download(model: &str) -> Result<JsValue, JsValue> {
    let model: Model = model.parse().map_err(js_err)?;
    js_json(&romid::download_json(model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_names_in_order() {
        assert_eq!(
            model_names(),
            vec!["48sx", "48gx", "38g", "49g", "39g", "40g", "42s"]
        );
    }
}
