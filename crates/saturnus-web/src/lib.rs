#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! WebAssembly bindings of the saturnus core for the browser UI in `web/`:
//! a thin layer over [`saturnus_host`], which holds the emulator, the key
//! queue, typing, layouts, skins and ROM identification for every front
//! end. Here only errors and typed answers become `JsValue`.
//!
//! [`Emulator`] owns one machine; the Worker (`web/worker.js`) is its
//! only caller and these are exactly the calls it makes. The model and
//! key names are the script names (`saturnus::io::Key::name`).
//!
//! - Running: `run_slice(ms, keys)` runs at most `ms` emulated ms and
//!   feeds the key queue; `idle_ms()` says how long a shut-down CPU sleeps
//!   before its next timer event (negative while it runs), so the Worker
//!   can stop and set a timer; `cycles()`, `emulated_ms()`.
//! - Keys (see `saturnus_host::host`): `press`/`release`, `release_held`,
//!   `release_keys`, `type_letter`, `type_keys`, `pump`, `keys_busy`,
//!   `has_key`, `take_errors`.
//! - Events: `take_frame()` and `take_keys()` give the `frame` and `keys`
//!   events of `web/protocol.md` as JSON text, only when they changed;
//!   `invalidate()` makes the next frame go out anyway.
//! - The user memory read straight from RAM (48SX, 48GX, 49G; nothing
//!   written, see `saturnus_objects::ram`): `memory_tree()` gives `{path,
//!   variables: [{name, type, size, checksum, address, variables?}]}`,
//!   `stack()` the typed levels, level 1 first, `flags()` `{system, user,
//!   set}`, `object_at(address)` one variable's typed value,
//!   `memory_changes()` a counter (16 hex digits) to poll, and
//!   `memory_refusal()` why a model has no memory view.
//! - Typing into the command line (see `saturnus_host::typing`):
//!   `command_line()`, `start_typing(verb, text)`, `typing_step(ms)`,
//!   `typing_result()` and `stop_typing()`.
//! - State: `save_state()`, `load_state(bytes)`, `reset()`, `model()`.
//!
//! Free functions: `model_names()`, `skin(model)` and `layout(model)`
//! (the drawn calculator and the plain key grid, see
//! `saturnus_host::skins` and `saturnus_host::layout`), `model_for(rom,
//! preferred)` (the model a ROM file fits), and ROM identification (see
//! `saturnus_host::romid`): `identify_rom` tells a known image (by
//! SHA-256) from one that only fits by size, `plan_roms` assigns a batch
//! of them to the remembered model slots.

use saturnus::Model;
use saturnus_host::{host, layout, romid, skins};
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// The emulated calculator.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Emulator {
    inner: saturnus_host::Emulator,
}

fn js_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// A typed answer of the host crate as JSON text.
fn json_text<T: serde::Serialize>(v: &T) -> Result<String, JsValue> {
    serde_json::to_string(v).map_err(js_err)
}

/// A typed answer of the host crate as a JavaScript value.
fn js_json<T: serde::Serialize>(v: &T) -> Result<JsValue, JsValue> {
    js_sys::JSON::parse(&json_text(v)?)
}

/// The model called `name`.
fn parse_model(name: &str) -> Result<Model, JsValue> {
    name.parse().map_err(js_err)
}

#[wasm_bindgen]
impl Emulator {
    /// Build `model` ("48sx", "48gx", "38g", "49g", "39g", "40g", "42s")
    /// from its ROM image.
    #[wasm_bindgen(constructor)]
    pub fn new(model: &str, rom: &[u8]) -> Result<Emulator, JsValue> {
        saturnus_host::Emulator::new(parse_model(model)?, rom)
            .map(|inner| Emulator { inner })
            .map_err(js_err)
    }

    /// The model's name.
    pub fn model(&self) -> String {
        self.inner.model().name().to_string()
    }

    /// The whole machine state (binds to this model and ROM).
    pub fn save_state(&self) -> Vec<u8> {
        self.inner.save_state()
    }

    /// Restore a state from `save_state`; refuses another model or ROM.
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), JsValue> {
        self.inner.load_state(data).map_err(js_err)
    }

    /// Hardware reset; RAM is kept.
    pub fn reset(&mut self) {
        self.inner.reset()
    }

    /// CPU cycles since power-on (exact below 2^53).
    pub fn cycles(&self) -> f64 {
        self.inner.cycles()
    }

    /// Emulated milliseconds since power-on.
    pub fn emulated_ms(&self) -> f64 {
        self.inner.emulated_ms()
    }

    /// `{path, variables}`: the current directory and HOME's tree, read
    /// from RAM (see the crate docs). Fails on the 38G, 39G and 40G and
    /// before the ROM has set up memory.
    pub fn memory_tree(&self) -> Result<JsValue, JsValue> {
        js_json(&self.inner.memory_tree().map_err(js_err)?)
    }

    /// The stack's typed levels, level 1 first, read from RAM.
    pub fn stack(&self) -> Result<JsValue, JsValue> {
        js_json(&self.inner.stack().map_err(js_err)?)
    }

    /// `{system, user, set}`: the flags, read from RAM.
    pub fn flags(&self) -> Result<JsValue, JsValue> {
        js_json(&self.inner.flags().map_err(js_err)?)
    }

    /// The typed object at `address` (a variable's `address`).
    pub fn object_at(&self, address: u32) -> Result<JsValue, JsValue> {
        js_json(&self.inner.object_at(address).map_err(js_err)?)
    }

    /// A counter (16 hex digits) that moves whenever a variable, the
    /// current directory, the stack or a flag changes.
    pub fn memory_changes(&self) -> Result<String, JsValue> {
        self.inner
            .memory_changes()
            .map(|c| format!("{c:016X}"))
            .map_err(js_err)
    }

    /// Why this model has no memory view, or `undefined` if it has one.
    pub fn memory_refusal(&self) -> Option<String> {
        self.inner.memory_refusal()
    }

    /// Emulated milliseconds the shut-down CPU will sleep before its next
    /// timer or UART event, so the page can stop its animation loop and
    /// set a timer instead; negative while the CPU runs or has a wake
    /// condition pending (the page must keep stepping).
    pub fn idle_ms(&self) -> f64 {
        self.inner.idle_ms().unwrap_or(-1.0)
    }

    /// Queue a press of `name`, held until `release(name)`; false if the
    /// model has no such key. Takes effect at the next `pump`.
    pub fn press(&mut self, name: &str) -> bool {
        self.inner.press(name)
    }

    /// Whether the model has a key called `name`.
    pub fn has_key(&self, name: &str) -> bool {
        self.inner.has_key(name)
    }

    /// Release the newest held press of `name` (at the next `pump`).
    pub fn release(&mut self, name: &str) {
        self.inner.release(name)
    }

    /// Release every held key once it was down long enough.
    pub fn release_held(&mut self) {
        self.inner.release_held()
    }

    /// Queue the first character of `ch` typed through alpha mode; false
    /// if the model has no key for it.
    pub fn type_letter(&mut self, ch: &str) -> bool {
        self.inner.type_letter(ch)
    }

    /// Queue full presses of the space-separated key `names`.
    pub fn type_keys(&mut self, names: &str) {
        self.inner.type_keys(names)
    }

    /// Release keys held long enough and start queued presses, now.
    #[wasm_bindgen(js_name = pump)]
    pub fn pump_js(&mut self) {
        self.inner.pump();
    }

    /// Release every key at once, the machine's and the queue's (reset,
    /// state load).
    pub fn release_keys(&mut self) {
        self.inner.release_keys()
    }

    /// Keys are down or queued: run in short slices.
    pub fn keys_busy(&self) -> bool {
        self.inner.keys_busy()
    }

    /// One slice of at most `left_ms`, see the crate's `host` module;
    /// returns the emulated ms run. Fails if the CPU halts.
    pub fn run_slice(&mut self, left_ms: f64, keys: bool) -> Result<f64, JsValue> {
        self.inner.run_slice(left_ms, keys).map_err(js_err)
    }

    /// The `frame` event as a JSON string if the display changed, else
    /// undefined.
    pub fn take_frame(&mut self) -> Result<Option<String>, JsValue> {
        self.inner
            .frame_if_changed()
            .map(|f| json_text(&f))
            .transpose()
    }

    /// The `keys` event as a JSON string if the keys down changed, else
    /// undefined.
    pub fn take_keys(&mut self) -> Result<Option<String>, JsValue> {
        self.inner
            .keys_if_changed()
            .map(|k| json_text(&k))
            .transpose()
    }

    /// Send the next frame even if unchanged.
    pub fn invalidate(&mut self) {
        self.inner.reshow()
    }

    /// Errors from refused key presses since the last call.
    pub fn take_errors(&mut self) -> Vec<String> {
        self.inner
            .take_errors()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// `{active, text, cursor}` of the command line (48SX, 48GX, 49G).
    pub fn command_line(&self) -> Result<JsValue, JsValue> {
        js_json(&self.inner.command_line().map_err(js_err)?)
    }

    /// Start an `insert`, `run` or `replace`; true if it freezes the screen.
    pub fn start_typing(&mut self, verb: &str, text: &str) -> Result<bool, JsValue> {
        self.inner.start_typing(verb, text).map_err(js_err)
    }

    /// Run the send at most `ms` emulated ms; true once done.
    pub fn typing_step(&mut self, ms: f64) -> Result<bool, JsValue> {
        self.inner.typing_step(ms).map_err(js_err)
    }

    /// The finished send's reply.
    pub fn typing_result(&mut self) -> Result<JsValue, JsValue> {
        js_json(&self.inner.typing_result().map_err(js_err)?)
    }

    /// Stop the send where it is.
    pub fn stop_typing(&mut self) {
        self.inner.stop_typing()
    }
}

/// The supported model names.
#[wasm_bindgen]
pub fn model_names() -> Vec<String> {
    Model::ALL.iter().map(|m| m.name().to_string()).collect()
}

/// The drawn skin of `model` (see `saturnus_host::skins::skin_view`), for
/// showing the calculator before a
/// ROM is loaded.
#[wasm_bindgen]
pub fn skin(model: &str) -> Result<JsValue, JsValue> {
    js_json(&skins::skin_view(parse_model(model)?))
}

/// The plain key grid of `model` (`{columns, rows, keys: [{name, label,
/// row, x, w, alpha?}]}`, see `saturnus_host::layout::grid`).
#[wasm_bindgen]
pub fn layout(model: &str) -> Result<JsValue, JsValue> {
    js_json(&layout::grid(parse_model(model)?))
}

/// The model (by name) that runs `rom`, preferring `preferred`, see
/// `saturnus_host::host::model_for_rom`.
#[wasm_bindgen]
pub fn model_for(rom: &[u8], preferred: &str) -> Result<String, JsValue> {
    Ok(host::model_for_rom(rom, parse_model(preferred)?)
        .name()
        .to_string())
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

    /// The page gets a running CPU's idle span as -1 (it keeps stepping).
    #[test]
    fn idle_ms_is_negative_while_running() {
        let emu = Emulator::new("48sx", &vec![0u8; 256 * 1024]).unwrap();
        assert_eq!(emu.idle_ms(), -1.0);
    }
}
