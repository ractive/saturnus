#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! WebAssembly bindings of the saturnus core for the browser UI in `web/`.
//!
//! [`Emulator`] owns one [`Machine`]. The page advances it in emulated
//! milliseconds from `requestAnimationFrame`, presses keys by script name
//! (see `saturnus::io::Key::name`), and reads back the display:
//!
//! - `framebuffer()`: 131 x 64 pixels, **one byte per pixel**, row-major
//!   from the top-left, 1 = dark and 0 = light (8384 bytes).
//! - `annunciators()`: `{leftshift, rightshift, alpha, alert, busy,
//!   transmit}` booleans, in strip order left to right.
//! - `contrast()`: the raw 5-bit contrast 0-31, higher is darker;
//!   `contrast_range()` gives the model's usable `[low, high]`.
//! - `keys()`: `{columns, rows, keys: [{name, label, row, x, w}]}`, the
//!   model's keys in their places on the case, `x` and `w` in units of
//!   `columns` per row (see [`layout`]).
//!
//! Everything that can be tested without a JavaScript host lives in plain
//! Rust functions (`*_inner`, [`layout`], [`pack_pixels`]); the bindings
//! only convert errors and JSON to `JsValue`, which is unavailable on a
//! native target.

pub mod layout;

use saturnus::io::Key;
use saturnus::machine::{Annunciators, LCD_HEIGHT, LCD_WIDTH, Lcd};
use saturnus::{Machine, Model};
use wasm_bindgen::prelude::*;

/// Bytes `framebuffer()` returns: one per pixel.
pub const FRAMEBUFFER_BYTES: usize = LCD_WIDTH * LCD_HEIGHT;

/// The model called `name` ("48sx", "48gx", "38g", "49g", "39g", "40g";
/// case-insensitive).
pub fn model_from_name(name: &str) -> Result<Model, String> {
    Model::ALL
        .into_iter()
        .find(|m| m.name().eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            let names: Vec<&str> = Model::ALL.iter().map(|m| m.name()).collect();
            format!(
                "unknown model {name:?}; expected one of {}",
                names.join(", ")
            )
        })
}

/// The pixels as one byte per pixel, row-major, 1 = dark.
pub fn pack_pixels(lcd: &Lcd) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAMEBUFFER_BYTES);
    for row in &lcd.pixels {
        out.extend(row.iter().map(|&on| u8::from(on)));
    }
    out
}

/// The annunciators as a JSON object of booleans.
pub fn annunciators_json(a: &Annunciators) -> String {
    let fields: Vec<String> = a
        .list()
        .iter()
        .map(|(name, on)| format!("{}:{on}", layout::json_string(name)))
        .collect();
    format!("{{{}}}", fields.join(","))
}

/// The emulated calculator.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Emulator {
    machine: Machine,
    /// Fractional cycles owed by `run_ms` calls, so many short frames add
    /// up to exactly `clock_hz` cycles per emulated second.
    cycle_debt: f64,
}

impl Emulator {
    /// Build `model` from its ROM image.
    pub fn new_inner(model: &str, rom: &[u8]) -> Result<Self, String> {
        let model = model_from_name(model)?;
        let machine = Machine::new(model, rom).map_err(|e| e.to_string())?;
        Ok(Self {
            machine,
            cycle_debt: 0.0,
        })
    }

    /// Run `ms` emulated milliseconds; returns the cycles run.
    pub fn run_ms_inner(&mut self, ms: f64) -> Result<f64, String> {
        if !ms.is_finite() || ms <= 0.0 {
            return Ok(0.0);
        }
        let want = ms * f64::from(self.machine.model().clock_hz()) / 1000.0 + self.cycle_debt;
        let whole = want.floor();
        if whole < 1.0 {
            self.cycle_debt = want;
            return Ok(0.0);
        }
        let start = self.machine.cycles();
        // `whole` is positive and far below 2^53 for any sane frame.
        let n = whole as u64;
        let result = self.machine.run_cycles(n);
        let ran = self.machine.cycles() - start;
        // The machine may overshoot by up to one instruction (or a SHUTDN
        // skip); carry that as a credit into the next call.
        self.cycle_debt = want - ran as f64;
        result.map_err(|h| format!("CPU halted: {h}"))?;
        Ok(ran as f64)
    }

    fn key(&self, name: &str) -> Result<Key, String> {
        Key::from_name(name).ok_or_else(|| format!("unknown key {name:?}"))
    }

    /// Press the key named `name`.
    pub fn key_down_inner(&mut self, name: &str) -> Result<(), String> {
        let k = self.key(name)?;
        self.machine.key_down(k).map_err(|e| e.to_string())
    }

    /// Release the key named `name`.
    pub fn key_up_inner(&mut self, name: &str) -> Result<(), String> {
        let k = self.key(name)?;
        self.machine.key_up(k).map_err(|e| e.to_string())
    }

    /// Restore a state saved by `save_state` for the same model and ROM.
    pub fn load_state_inner(&mut self, data: &[u8]) -> Result<(), String> {
        self.machine.load_state(data).map_err(|e| e.to_string())?;
        self.cycle_debt = 0.0;
        Ok(())
    }

    /// The machine, for native callers and tests.
    pub fn machine(&self) -> &Machine {
        &self.machine
    }
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
        Self::new_inner(model, rom).map_err(js_err)
    }

    /// The model's name.
    pub fn model(&self) -> String {
        self.machine.model().name().to_string()
    }

    /// The model's CPU clock in Hz.
    pub fn clock_hz(&self) -> u32 {
        self.machine.model().clock_hz()
    }

    /// Advance emulated time by `ms` milliseconds; returns the cycles run.
    /// Fails if the CPU meets an undefined opcode.
    pub fn run_ms(&mut self, ms: f64) -> Result<f64, JsValue> {
        self.run_ms_inner(ms).map_err(js_err)
    }

    /// Press a key by script name ("7", "enter", "f1", "on", ...).
    pub fn key_down(&mut self, name: &str) -> Result<(), JsValue> {
        self.key_down_inner(name).map_err(js_err)
    }

    /// Release a key by script name.
    pub fn key_up(&mut self, name: &str) -> Result<(), JsValue> {
        self.key_up_inner(name).map_err(js_err)
    }

    /// Release every held key.
    pub fn release_all(&mut self) {
        self.machine.hw.keyboard.release_all();
    }

    /// The model's keys in their places on the case, see the crate docs.
    pub fn keys(&self) -> Result<JsValue, JsValue> {
        json_value(&layout::layout_json(self.machine.model()))
    }

    /// The 131 x 64 pixels, one byte per pixel, row-major, 1 = dark.
    pub fn framebuffer(&self) -> Vec<u8> {
        pack_pixels(&self.machine.lcd())
    }

    /// The six annunciators as an object of booleans.
    pub fn annunciators(&self) -> Result<JsValue, JsValue> {
        json_value(&annunciators_json(&self.machine.framebuffer().annunciators))
    }

    /// Raw 5-bit contrast, 0-31, higher is darker.
    pub fn contrast(&self) -> u8 {
        self.machine.hw.io.contrast()
    }

    /// The model's usable contrast range as `[low, high]`.
    pub fn contrast_range(&self) -> Vec<u8> {
        let r = self.machine.model().contrast_range();
        vec![*r.start(), *r.end()]
    }

    /// The whole machine state (binds to this model and ROM).
    pub fn save_state(&self) -> Vec<u8> {
        self.machine.save_state()
    }

    /// Restore a state from `save_state`; refuses another model or ROM.
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), JsValue> {
        self.load_state_inner(data).map_err(js_err)
    }

    /// Hardware reset; RAM is kept.
    pub fn reset(&mut self) {
        self.machine.reset();
    }

    /// CPU cycles since power-on (exact below 2^53).
    pub fn cycles(&self) -> f64 {
        self.machine.cycles() as f64
    }

    /// Emulated milliseconds since power-on.
    pub fn emulated_ms(&self) -> f64 {
        self.machine.cycles() as f64 * 1000.0 / f64::from(self.machine.model().clock_hz())
    }

    /// True while the CPU sleeps in SHUTDN.
    pub fn is_shutdown(&self) -> bool {
        self.machine.is_shutdown()
    }
}

/// The supported model names.
#[wasm_bindgen]
pub fn model_names() -> Vec<String> {
    Model::ALL.iter().map(|m| m.name().to_string()).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_pack_one_byte_each_row_major() {
        let mut lcd = Lcd::blank();
        lcd.pixels[0][0] = true;
        lcd.pixels[0][130] = true;
        lcd.pixels[1][0] = true;
        lcd.pixels[63][130] = true;
        let fb = pack_pixels(&lcd);
        assert_eq!(fb.len(), FRAMEBUFFER_BYTES);
        assert_eq!(fb.len(), 8384);
        let lit: Vec<usize> = (0..fb.len()).filter(|&i| fb[i] == 1).collect();
        assert_eq!(lit, vec![0, 130, 131, 8383]);
        assert!(fb.iter().all(|&b| b <= 1));
    }

    #[test]
    fn annunciators_as_json() {
        assert_eq!(
            annunciators_json(&Annunciators::default()),
            "{\"leftshift\":false,\"rightshift\":false,\"alpha\":false,\
             \"alert\":false,\"busy\":false,\"transmit\":false}"
        );
        let a = Annunciators {
            alpha: true,
            busy: true,
            ..Annunciators::default()
        };
        let j = annunciators_json(&a);
        assert!(j.contains("\"alpha\":true") && j.contains("\"busy\":true"));
        assert!(j.contains("\"leftshift\":false"));
    }

    #[test]
    fn models_by_name() {
        assert_eq!(model_from_name("48SX"), Ok(Model::Hp48sx));
        assert_eq!(model_from_name("49g"), Ok(Model::Hp49g));
        assert!(model_from_name("42s").is_err());
        assert_eq!(
            model_names(),
            vec!["48sx", "48gx", "38g", "49g", "39g", "40g"]
        );
        assert_eq!(rom_bytes("48sx"), 256 * 1024);
        assert!(rom_fits("39g", 2 * 1024 * 1024));
        assert!(rom_fits("49g", 4 * 1024 * 1024));
        assert!(!rom_fits("48gx", 2 * 1024 * 1024));
        assert_eq!(rom_bytes("nope"), 0);
    }

    #[test]
    fn rejects_wrong_rom_size() {
        let e = Emulator::new_inner("48sx", &[0u8; 1000]).unwrap_err();
        assert!(e.contains("1000"), "{e}");
    }

    /// A machine on a ROM of zeros runs (it loops through nonsense or
    /// halts) and keeps exact time across many short frames.
    #[test]
    fn run_ms_keeps_exact_time() {
        // A ROM of #F nibbles... any content; use zeros and tolerate a halt.
        let mut emu = Emulator::new_inner("48sx", &vec![0u8; 256 * 1024]).unwrap();
        let mut total = 0.0;
        for _ in 0..100 {
            match emu.run_ms_inner(0.3) {
                Ok(c) => total += c,
                Err(e) => {
                    assert!(e.contains("halted"), "{e}");
                    return;
                }
            }
        }
        // 100 x 0.3 ms at 2 MHz = 60000 cycles, give or take one
        // instruction of overshoot.
        assert!((total - 60_000.0).abs() < 100.0, "{total}");
        assert_eq!(emu.run_ms_inner(0.0), Ok(0.0));
        assert_eq!(emu.run_ms_inner(f64::NAN), Ok(0.0));
    }

    #[test]
    fn keys_by_name_per_model() {
        let mut emu = Emulator::new_inner("48sx", &vec![0u8; 256 * 1024]).unwrap();
        assert!(emu.key_down_inner("enter").is_ok());
        assert!(emu.key_up_inner("enter").is_ok());
        assert!(emu.key_down_inner("f1").is_ok());
        assert!(
            emu.key_down_inner("bogus")
                .unwrap_err()
                .contains("unknown key")
        );
        // The 49G-only key is refused on a 48.
        assert!(emu.key_down_inner("apps").is_err());
    }

    #[test]
    fn state_round_trip() {
        let mut emu = Emulator::new_inner("48sx", &vec![0u8; 256 * 1024]).unwrap();
        let _ = emu.run_ms_inner(1.0);
        let saved = emu.machine().save_state();
        let cycles = emu.machine().cycles();
        let _ = emu.run_ms_inner(5.0);
        emu.load_state_inner(&saved).unwrap();
        assert_eq!(emu.machine().cycles(), cycles);
        assert!(emu.load_state_inner(&[1, 2, 3]).is_err());
    }
}
