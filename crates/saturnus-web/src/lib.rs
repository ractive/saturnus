#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! WebAssembly bindings of the saturnus core for the browser UI in `web/`:
//! a thin layer over [`saturnus_host`], which holds the emulator, the key
//! queue, typing, layouts, skins and ROM identification for every front
//! end. Here only errors and JSON become `JsValue`.
//!
//! [`Emulator`] owns one machine. The page advances it in emulated
//! milliseconds from `requestAnimationFrame`, presses keys by script name
//! (see `saturnus::io::Key::name`), and reads back the display:
//!
//! - `framebuffer()`: 131 x 64 pixels (131 x 16 on the 42S, see
//!   `lcd_height()`), **one byte per pixel**, row-major from the top-left,
//!   1 = dark and 0 = light (8384 bytes, 2096 on the 42S).
//! - `annunciators()`: `{leftshift, rightshift, alpha, alert, busy,
//!   transmit, updown, battery, g, rad}` booleans; the 48's six in strip
//!   order, then the 42S-only ones (the 42S reports its shift, print and
//!   run annunciators as leftshift, transmit and busy). All ten keys on
//!   every model, so the shape is stable.
//! - `contrast()`: the raw 5-bit contrast 0-31, higher is darker;
//!   `contrast_range()` gives the model's usable `[low, high]`.
//! - `keys()`: `{columns, rows, keys: [{name, label, row, x, w}]}`, the
//!   model's keys in their places on the case, `x` and `w` in units of
//!   `columns` per row (see `saturnus_host::layout`).
//! - `skin()`: the model's drawn skin (case, display window, keys with
//!   their labels and colours) as JSON, see `saturnus_host::skins`.
//! - The user memory read straight from RAM (48SX, 48GX, 49G; no Kermit
//!   server, nothing written, see `saturnus_objects::ram`):
//!   `memory_tree()` gives `{path, variables: [{name, type, size,
//!   checksum, address, variables?}]}` (HOME's tree, newest first, the
//!   current directory as `path`), `stack()` the typed levels, level 1
//!   first, `flags()` `{system, user, set}` (words as 16 hex digits),
//!   `object_at(address)` one variable's typed value, and
//!   `memory_changes()` a counter (16 hex digits) to poll: re-read only
//!   when it moves.
//! - Typing into the command line (see `saturnus_host::typing`):
//!   `command_line()` gives `{active, text, cursor}` from RAM;
//!   `start_typing(verb, text)`, `typing_step(ms)`, `typing_result()` and
//!   `stop_typing()` run an `insert`, `run` or `replace` in emulated time.
//! - `idle_ms()`: how long a shut-down CPU sleeps before its next timer
//!   event, so the page can stop animating; negative while it runs.
//! - The host side of `web/protocol.md` (see `saturnus_host::host`):
//!   `press`/`release`, `type_letter`, `type_keys` go through a key queue
//!   timed in emulated time; `run_slice` runs and feeds it;
//!   `take_frame`/`take_keys` give the `frame` and `keys` events only
//!   when they changed; `model_for` picks the model a ROM file fits.
//! - ROM identification (see `saturnus_host::romid`): `identify_rom`
//!   tells a known image (by SHA-256) from one that only fits by size,
//!   and `plan_roms` assigns a batch of them to the remembered model
//!   slots, with the rules every host shares.

use saturnus::Model;
use saturnus_host::{annunciators_json, host, layout, model_from_name, romid, skins};
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// The emulated calculator.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Emulator {
    inner: saturnus_host::Emulator,
}

fn js_err(e: String) -> JsValue {
    JsValue::from_str(&e)
}

fn json_value(s: &str) -> Result<JsValue, JsValue> {
    js_sys::JSON::parse(s)
}

#[wasm_bindgen]
impl Emulator {
    /// Build `model` ("48sx", "48gx", "38g", "49g") from its ROM image.
    #[wasm_bindgen(constructor)]
    pub fn new(model: &str, rom: &[u8]) -> Result<Emulator, JsValue> {
        saturnus_host::Emulator::new_inner(model, rom)
            .map(|inner| Emulator { inner })
            .map_err(js_err)
    }

    /// The model's name.
    pub fn model(&self) -> String {
        self.inner.model()
    }

    /// The model's CPU clock in Hz.
    pub fn clock_hz(&self) -> u32 {
        self.inner.clock_hz()
    }

    /// Advance emulated time by `ms` milliseconds; returns the cycles run.
    /// Fails if the CPU meets an undefined opcode.
    pub fn run_ms(&mut self, ms: f64) -> Result<f64, JsValue> {
        self.inner.run_ms_inner(ms).map_err(js_err)
    }

    /// Press a key by script name ("7", "enter", "f1", "on", ...).
    pub fn key_down(&mut self, name: &str) -> Result<(), JsValue> {
        self.inner.key_down_inner(name).map_err(js_err)
    }

    /// Release a key by script name.
    pub fn key_up(&mut self, name: &str) -> Result<(), JsValue> {
        self.inner.key_up_inner(name).map_err(js_err)
    }

    /// Release every held key.
    pub fn release_all(&mut self) {
        self.inner.release_all()
    }

    /// The model's keys in their places on the case, see the crate docs.
    pub fn keys(&self) -> Result<JsValue, JsValue> {
        json_value(&layout::layout_json(self.inner.machine().model()))
    }

    /// The model's drawn skin, see `saturnus_host::skins::skin_json`.
    pub fn skin(&self) -> Result<JsValue, JsValue> {
        json_value(&skins::skin_json(self.inner.machine().model()))
    }

    /// The 131 x 64 pixels (131 x 16 on the 42S), one byte per pixel,
    /// row-major, 1 = dark.
    pub fn framebuffer(&self) -> Vec<u8> {
        self.inner.framebuffer()
    }

    /// LCD rows: 64, or 16 on the 42S.
    pub fn lcd_height(&self) -> usize {
        self.inner.lcd_height()
    }

    /// The annunciators as an object of booleans.
    pub fn annunciators(&self) -> Result<JsValue, JsValue> {
        json_value(&annunciators_json(
            &self.inner.machine().framebuffer().annunciators,
        ))
    }

    /// Raw 5-bit contrast, 0-31, higher is darker.
    pub fn contrast(&self) -> u8 {
        self.inner.contrast()
    }

    /// The model's usable contrast range as `[low, high]`.
    pub fn contrast_range(&self) -> Vec<u8> {
        self.inner.contrast_range()
    }

    /// The whole machine state (binds to this model and ROM).
    pub fn save_state(&self) -> Vec<u8> {
        self.inner.save_state()
    }

    /// Restore a state from `save_state`; refuses another model or ROM.
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), JsValue> {
        self.inner.load_state_inner(data).map_err(js_err)
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

    /// True while the CPU sleeps in SHUTDN.
    pub fn is_shutdown(&self) -> bool {
        self.inner.is_shutdown()
    }

    /// `{path, variables}`: the current directory and HOME's tree, read
    /// from RAM (see the crate docs). Fails on the 38G, 39G and 40G and
    /// before the ROM has set up memory.
    pub fn memory_tree(&self) -> Result<JsValue, JsValue> {
        json_value(&self.inner.memory_tree_inner().map_err(js_err)?)
    }

    /// The stack's typed levels, level 1 first, read from RAM.
    pub fn stack(&self) -> Result<JsValue, JsValue> {
        json_value(&self.inner.stack_inner().map_err(js_err)?)
    }

    /// `{system, user, set}`: the flags, read from RAM.
    pub fn flags(&self) -> Result<JsValue, JsValue> {
        json_value(&self.inner.flags_inner().map_err(js_err)?)
    }

    /// The typed object at `address` (a variable's `address`).
    pub fn object_at(&self, address: u32) -> Result<JsValue, JsValue> {
        json_value(&self.inner.object_at_inner(address).map_err(js_err)?)
    }

    /// A counter (16 hex digits) that moves whenever a variable, the
    /// current directory, the stack or a flag changes.
    pub fn memory_changes(&self) -> Result<String, JsValue> {
        self.inner.memory_changes_inner().map_err(js_err)
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
        self.inner.idle_ms()
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

    /// Release every key and drop the queue (reset, state load).
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
        self.inner.run_slice_inner(left_ms, keys).map_err(js_err)
    }

    /// The `frame` event as a JSON string if the display changed, else
    /// undefined.
    pub fn take_frame(&mut self) -> Option<String> {
        self.inner.take_frame()
    }

    /// The `keys` event as a JSON string if the keys down changed, else
    /// undefined.
    pub fn take_keys(&mut self) -> Option<String> {
        self.inner.take_keys()
    }

    /// Send the next frame even if unchanged.
    pub fn invalidate(&mut self) {
        self.inner.invalidate()
    }

    /// Errors from refused key presses since the last call.
    pub fn take_errors(&mut self) -> Vec<String> {
        self.inner.take_errors()
    }

    /// `{active, text, cursor}` of the command line (48SX, 48GX, 49G).
    pub fn command_line(&self) -> Result<JsValue, JsValue> {
        json_value(&self.inner.command_line_inner().map_err(js_err)?)
    }

    /// Start an `insert`, `run` or `replace`; true if it freezes the screen.
    pub fn start_typing(&mut self, verb: &str, text: &str) -> Result<bool, JsValue> {
        self.inner.start_typing_inner(verb, text).map_err(js_err)
    }

    /// Whether a send is in progress.
    pub fn typing(&self) -> bool {
        self.inner.typing()
    }

    /// Run the send at most `ms` emulated ms; true once done.
    pub fn typing_step(&mut self, ms: f64) -> Result<bool, JsValue> {
        self.inner.typing_step_inner(ms).map_err(js_err)
    }

    /// The finished send's reply.
    pub fn typing_result(&mut self) -> Result<JsValue, JsValue> {
        json_value(&self.inner.typing_result_inner().map_err(js_err)?)
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

/// The drawn skin of `model` as JSON (see
/// `saturnus_host::skins::skin_json`), for showing the calculator before a
/// ROM is loaded.
#[wasm_bindgen]
pub fn skin(model: &str) -> Result<JsValue, JsValue> {
    let model = model_from_name(model).map_err(js_err)?;
    json_value(&skins::skin_json(model))
}

/// The ROM size in bytes `model` expects (the 49G, 39G and 40G also take
/// twice this, unpacked), or 0 for an unknown model.
#[wasm_bindgen]
pub fn rom_bytes(model: &str) -> usize {
    model_from_name(model).map_or(0, |m| m.rom_bytes())
}

/// Whether `model` takes a ROM file of `bytes` bytes (packed, or for the
/// 49G, 39G and 40G also unpacked).
#[wasm_bindgen]
pub fn rom_fits(model: &str, bytes: usize) -> bool {
    model_from_name(model).is_ok_and(|m| m.accepts_rom_len(bytes))
}

/// The drawn keyboard of `model`, see `Emulator.keys()`.
#[wasm_bindgen]
pub fn layout(model: &str) -> Result<JsValue, JsValue> {
    json_value(&host::layout_of(model).map_err(js_err)?)
}

/// The model (by name) that runs `rom`, preferring `preferred`, see
/// `saturnus_host::host::model_for_rom`.
#[wasm_bindgen]
pub fn model_for(rom: &[u8], preferred: &str) -> Result<String, JsValue> {
    host::model_for_rom_name(rom, preferred)
        .map(|m| m.name().to_string())
        .map_err(js_err)
}

/// `romid::identify` for the Worker, see `romid::rom_id_json`.
#[wasm_bindgen]
pub fn identify_rom(rom: &[u8]) -> Result<JsValue, JsValue> {
    json_value(&romid::rom_id_json(&romid::identify(rom)).to_string())
}

/// `romid::plan_json` for the Worker; `input` is JSON text.
#[wasm_bindgen]
pub fn plan_roms(input: &str) -> Result<JsValue, JsValue> {
    let v: Value = serde_json::from_str(input).map_err(|e| js_err(e.to_string()))?;
    let out = romid::plan_json(&v).map_err(js_err)?;
    json_value(&out.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_and_rom_sizes_by_name() {
        assert_eq!(
            model_names(),
            vec!["48sx", "48gx", "38g", "49g", "39g", "40g", "42s"]
        );
        assert!(rom_fits("42s", 64 * 1024));
        assert_eq!(rom_bytes("48sx"), 256 * 1024);
        assert!(rom_fits("39g", 2 * 1024 * 1024));
        assert!(rom_fits("49g", 4 * 1024 * 1024));
        assert!(!rom_fits("48gx", 2 * 1024 * 1024));
        assert_eq!(rom_bytes("nope"), 0);
    }
}
