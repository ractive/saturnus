#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! What every front end of saturnus needs around the core, with no
//! bindings, no I/O and no threads (it builds for `wasm32` like the core):
//! the browser's wasm bindings (`saturnus-web`), the desktop app and the
//! CLI's control API (through `saturnus-drive`'s runner) all use it.
//!
//! [`Emulator`] owns one [`Machine`] and runs it in emulated milliseconds
//! ([`Emulator::run_ms`], [`Emulator::run_slice`]); it reads back the
//! display ([`Emulator::framebuffer`], one byte per pixel,
//! [`annunciators_json`]) and the user memory straight from RAM (48SX,
//! 48GX, 49G: [`Emulator::memory_tree`], [`Emulator::stack`],
//! [`Emulator::flags`]).
//!
//! - [`host`]: the key queue that times presses and types letters in
//!   emulated time, and the change-detected `frame` and `keys` events of
//!   the front end's protocol (`web/protocol.md` in the repository).
//! - [`typing`]: typing text into the command line (`insert`, `run`,
//!   `replace`) in emulated time.
//! - [`layout`] and [`skins`]: each model's keys in their places and its
//!   drawn calculator.
//! - [`romid`]: ROM identification by SHA-256 ([`sha256`]) and size, and
//!   the assignment of ROM files to models.
//!
//! Failures are an [`Error`] with a message for the user; the wasm
//! bindings turn it and the typed answers into JavaScript values.

mod error;
pub mod host;
pub mod layout;
pub mod romid;
pub mod sha256;
pub mod skins;
pub mod typing;

use saturnus::io::Key;
use saturnus::machine::{Annunciators, LCD_HEIGHT, LCD_WIDTH, Lcd};
use saturnus::{Machine, Model};
use saturnus_objects::{Flags, NameTable, UserMemory, Variable};
use std::sync::Arc;

pub use error::{Error, Result};

/// Bytes `framebuffer()` returns on the 131x64 models: one per pixel.
pub const FRAMEBUFFER_BYTES: usize = LCD_WIDTH * LCD_HEIGHT;

/// The model called `name` ("48sx", "48gx", "38g", "49g", "39g", "40g",
/// "42s"; case-insensitive).
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

/// The current directory and HOME's tree ([`Emulator::memory_tree`]).
#[derive(Clone, Debug, serde::Serialize)]
pub struct MemoryTree {
    /// The current directory, e.g. `["HOME", "A"]`.
    pub path: Vec<String>,
    /// HOME's variables, newest first, with sub-directories.
    pub variables: Vec<Variable>,
}

/// The emulated calculator.
#[derive(Debug)]
pub struct Emulator {
    machine: Machine,
    /// Fractional cycles owed by `run_ms` calls, so many short frames add
    /// up to exactly `clock_hz` cycles per emulated second.
    cycle_debt: f64,
    /// Key presses timed in emulated time (see [`host`]).
    queue: host::KeyQueue,
    /// What the last `frame` event showed.
    shown: Option<host::Shown>,
    /// The keys down in the last `keys` event.
    shown_keys: Option<Vec<&'static str>>,
    /// The ROM's command names with the ROM generation they were built
    /// at: built on the first object read, rebuilt when the ROM changed
    /// (the 49G's flash is written by storing a library in port 2).
    names: std::cell::RefCell<Option<(u64, Arc<NameTable>)>>,
    /// A send typing into the command line (see [`typing`]).
    typing: Option<typing::Job>,
}

impl Emulator {
    /// Build `model` from its ROM image.
    pub fn new(model: &str, rom: &[u8]) -> Result<Self> {
        let model = model_from_name(model)?;
        let machine = Machine::new(model, rom).map_err(|e| e.to_string())?;
        Ok(Self {
            queue: host::KeyQueue::new(model),
            machine,
            cycle_debt: 0.0,
            shown: None,
            shown_keys: None,
            names: std::cell::RefCell::new(None),
            typing: None,
        })
    }

    /// Wrap a machine a native host built (and perhaps already ran), with
    /// an empty key queue; the next frame and keys are sent in full.
    pub fn from_machine(machine: Machine) -> Self {
        Self {
            queue: host::KeyQueue::new(machine.model()),
            machine,
            cycle_debt: 0.0,
            shown: None,
            shown_keys: None,
            names: std::cell::RefCell::new(None),
            typing: None,
        }
    }

    /// The machine, for a native host that drives it directly (a key
    /// script) or keeps it after the emulator.
    pub fn into_machine(self) -> Machine {
        self.machine
    }

    /// The machine, mutably, for native hosts (the serial bridge, memory
    /// writes).
    pub fn machine_mut(&mut self) -> &mut Machine {
        &mut self.machine
    }

    /// Run `ms` emulated milliseconds; returns the cycles run.
    pub fn run_ms(&mut self, ms: f64) -> Result<f64> {
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

    fn key(&self, name: &str) -> Result<Key> {
        Key::from_name(name).ok_or_else(|| format!("unknown key {name:?}").into())
    }

    /// Press the key named `name`.
    pub fn key_down(&mut self, name: &str) -> Result<()> {
        let k = self.key(name)?;
        Ok(self.machine.key_down(k).map_err(|e| e.to_string())?)
    }

    /// Release the key named `name`.
    pub fn key_up(&mut self, name: &str) -> Result<()> {
        let k = self.key(name)?;
        Ok(self.machine.key_up(k).map_err(|e| e.to_string())?)
    }

    /// Restore a state saved by `save_state` for the same model and ROM.
    pub fn load_state(&mut self, data: &[u8]) -> Result<()> {
        self.machine.load_state(data).map_err(|e| e.to_string())?;
        self.cycle_debt = 0.0;
        Ok(())
    }

    /// Emulated milliseconds the shut-down CPU will sleep before its next
    /// timer or UART event, so a host can stop stepping and set a timer
    /// instead; `None` while the CPU runs or has a wake condition pending.
    pub fn idle_ms(&self) -> Option<f64> {
        self.machine
            .idle_cycles()
            .map(|c| c as f64 * 1000.0 / f64::from(self.machine.model().clock_hz()))
    }

    /// The machine, for native callers and tests.
    pub fn machine(&self) -> &Machine {
        &self.machine
    }

    fn user_memory(&self) -> Result<UserMemory<'_>> {
        UserMemory::of(&self.machine).map_err(|e| format!("{e:#}").into())
    }

    /// The ROM's command names, built on the first call and again
    /// whenever the ROM moved on ([`Machine::rom_generation`]).
    fn names(&self) -> Arc<NameTable> {
        let generation = self.machine.rom_generation();
        let mut cached = self.names.borrow_mut();
        match &*cached {
            Some((g, names)) if *g == generation => names.clone(),
            _ => {
                let names = Arc::new(NameTable::of(&self.machine));
                *cached = Some((generation, names.clone()));
                names
            }
        }
    }

    /// Why this model has no memory view (an aplet model, the 42S), or
    /// `None` for a model whose user memory can be read.
    pub fn memory_refusal(&self) -> Option<String> {
        self.user_memory().err().map(String::from)
    }

    /// The current directory and HOME's tree.
    pub fn memory_tree(&self) -> Result<MemoryTree> {
        let u = self.user_memory()?;
        let path = u.current_path().map_err(|e| format!("{e:#}"))?;
        let variables = u.tree().map_err(|e| format!("{e:#}"))?;
        Ok(MemoryTree { path, variables })
    }

    /// The stack's typed levels, level 1 first, each object with the
    /// calculator's own `text` (`saturnus_objects::described`).
    pub fn stack(&self) -> Result<Vec<serde_json::Value>> {
        let names = self.names();
        let levels = self
            .user_memory()?
            .with_names(&names)
            .stack_described()
            .map_err(|e| format!("{e:#}"))?;
        Ok(levels)
    }

    /// The flags (`{system, user, set}` in JSON).
    pub fn flags(&self) -> Result<Flags> {
        Ok(self.user_memory()?.flags().map_err(|e| format!("{e:#}"))?)
    }

    /// The typed object at `address` (a variable's `address`), with the
    /// calculator's own `text` on it and on every object inside.
    pub fn object_at(&self, address: u32) -> Result<serde_json::Value> {
        let names = self.names();
        let obj = self
            .user_memory()?
            .with_names(&names)
            .object_described(address)
            .map_err(|e| format!("{e:#}"))?;
        Ok(obj)
    }

    /// A counter that moves whenever a variable, the current directory,
    /// the stack or a flag changes.
    pub fn memory_changes(&self) -> Result<u64> {
        Ok(self
            .user_memory()?
            .change_counter()
            .map_err(|e| format!("{e:#}"))?)
    }
}

impl Emulator {
    /// The model's name.
    pub fn model(&self) -> String {
        self.machine.model().name().to_string()
    }

    /// The model's CPU clock in Hz.
    pub fn clock_hz(&self) -> u32 {
        self.machine.model().clock_hz()
    }

    /// Release every held key.
    pub fn release_all(&mut self) {
        self.machine.hw.keyboard.release_all();
    }

    /// The 131 x 64 pixels (131 x 16 on the 42S), one byte per pixel,
    /// row-major, 1 = dark.
    pub fn framebuffer(&self) -> Vec<u8> {
        pack_pixels(&self.machine.lcd())
    }

    /// LCD rows: 64, or 16 on the 42S.
    pub fn lcd_height(&self) -> usize {
        self.machine.lcd().height()
    }

    /// Raw 5-bit contrast, 0-31, higher is darker.
    pub fn contrast(&self) -> u8 {
        self.machine.hw.contrast()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn put(n: &mut [u8], at: usize, value: usize, width: usize) {
        for i in 0..width {
            n[at + i] = ((value >> (4 * i)) & 0xF) as u8;
        }
    }

    fn put_rel(n: &mut [u8], at: usize, target: usize) {
        put(n, at, (target + 0x100000 - at) & 0xFFFFF, 5);
    }

    /// Library `id` with one command named `name` at nibble `base` of `n`
    /// (layout: wiki protocols/rpl-libraries).
    fn library(n: &mut [u8], base: usize, id: usize, name: &str) {
        let (header, hash, object) = (base, base + 0x40, base + 0x200);
        let body = hash + 10;
        let entry = body + 85;
        put_rel(n, body + 5 * (name.len() - 1), entry);
        put(n, entry, name.len(), 2);
        for (i, b) in name.bytes().enumerate() {
            put(n, entry + 2 + 2 * i, usize::from(b), 2);
        }
        put(n, entry + 2 + 2 * name.len(), 0, 3);
        let numbers = entry + 2 + 2 * name.len() + 3;
        put_rel(n, body + 80, numbers);
        put(n, numbers, numbers - entry, 5);
        let end = numbers + 5;
        put(n, hash, 0x02A4E, 5);
        put(n, hash + 5, end - hash - 5, 5);
        let link = end;
        put(n, link, 0x02A4E, 5);
        put(n, link + 5, 10, 5);
        put_rel(n, link + 10, object);
        put(n, object - 6, id, 3);
        put(n, object - 3, 0, 3);
        put(n, object, 0x02D9D, 5);
        put(n, object + 5, 0x0312B, 5);
        put(n, header, id, 3);
        put_rel(n, header + 3, hash);
        put_rel(n, header + 13, link);
    }

    /// The 49G's flash changes under a running emulator (a library stored
    /// in port 2, here a loaded state): the name table is rebuilt and the
    /// new library resolves.
    #[test]
    fn names_follow_flash_changes() {
        let marker = b"saturnus-marker!";
        let mut image = vec![0xFFu8; Model::Hp49g.rom_bytes()];
        image[0x100..0x110].copy_from_slice(marker);
        let emu = Emulator::from_machine(Machine::new(Model::Hp49g, &image).unwrap());
        assert_eq!(emu.names().xlib(0x123, 0), None);
        let first = emu.names();
        assert!(Arc::ptr_eq(&first, &emu.names()), "cached while unchanged");

        // The same flash with the library in bank 5, through a state.
        let mut nibbles: Vec<u8> = image.iter().flat_map(|&b| [b & 0xF, b >> 4]).collect();
        library(&mut nibbles, 5 * 0x40000 + 0x1000, 0x123, "FOO");
        let packed: Vec<u8> = nibbles.chunks(2).map(|p| p[0] | (p[1] << 4)).collect();
        let mut state = emu.machine().save_state();
        let at = state
            .windows(marker.len())
            .position(|w| w == marker)
            .unwrap()
            - 0x100;
        state[at..at + packed.len()].copy_from_slice(&packed);
        let mut emu = emu;
        emu.load_state(&state).unwrap();
        let names = emu.names();
        assert!(!Arc::ptr_eq(&first, &names), "rebuilt after the change");
        assert_eq!(names.xlib(0x123, 0).and_then(|c| c.name), Some("FOO"));
    }

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
             \"alert\":false,\"busy\":false,\"transmit\":false,\"updown\":false,\
             \"battery\":false,\"g\":false,\"rad\":false}"
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
        assert_eq!(model_from_name("42S"), Ok(Model::Hp42s));
        assert!(model_from_name("41c").is_err());
    }

    #[test]
    fn rejects_wrong_rom_size() {
        let e = Emulator::new("48sx", &[0u8; 1000]).unwrap_err();
        assert!(e.message().contains("1000"), "{e}");
    }

    /// A machine on a ROM of zeros runs (it loops through nonsense or
    /// halts) and keeps exact time across many short frames.
    #[test]
    fn run_ms_keeps_exact_time() {
        // A ROM of #F nibbles... any content; use zeros and tolerate a halt.
        let mut emu = Emulator::new("48sx", &vec![0u8; 256 * 1024]).unwrap();
        let mut total = 0.0;
        for _ in 0..100 {
            match emu.run_ms(0.3) {
                Ok(c) => total += c,
                Err(e) => {
                    assert!(e.message().contains("halted"), "{e}");
                    return;
                }
            }
        }
        // 100 x 0.3 ms at 2 MHz = 60000 cycles, give or take one
        // instruction of overshoot.
        assert!((total - 60_000.0).abs() < 100.0, "{total}");
        assert_eq!(emu.run_ms(0.0), Ok(0.0));
        assert_eq!(emu.run_ms(f64::NAN), Ok(0.0));
    }

    #[test]
    fn keys_by_name_per_model() {
        let mut emu = Emulator::new("48sx", &vec![0u8; 256 * 1024]).unwrap();
        assert!(emu.key_down("enter").is_ok());
        assert!(emu.key_up("enter").is_ok());
        assert!(emu.key_down("f1").is_ok());
        assert!(
            emu.key_down("bogus")
                .unwrap_err()
                .message()
                .contains("unknown key")
        );
        // The 49G-only key is refused on a 48.
        assert!(emu.key_down("apps").is_err());
    }

    /// The memory view needs memory the ROM has set up; the aplet models
    /// have none.
    #[test]
    fn memory_view_errors() {
        let emu = Emulator::new("48sx", &vec![0u8; 256 * 1024]).unwrap();
        let e = emu.memory_tree().unwrap_err();
        assert!(e.message().contains("no directory at HOME"), "{e}");
        let flags = serde_json::to_string(&emu.flags().unwrap()).unwrap();
        assert!(flags.contains("\"set\":[]"));
        let emu = Emulator::new("38g", &vec![0u8; 512 * 1024]).unwrap();
        assert!(emu.stack().unwrap_err().message().contains("aplets"));
        let emu = Emulator::new("42s", &vec![0u8; 64 * 1024]).unwrap();
        for e in [
            emu.stack().unwrap_err(),
            emu.memory_tree().unwrap_err(),
            emu.flags().unwrap_err(),
        ] {
            assert!(e.message().contains("42S has no RPL user memory"), "{e}");
        }
    }

    /// A fresh machine on a ROM of zeros is running, so it reports no idle
    /// span.
    #[test]
    fn idle_ms_is_none_while_running() {
        let emu = Emulator::new("48sx", &vec![0u8; 256 * 1024]).unwrap();
        assert_eq!(emu.idle_ms(), None);
    }

    #[test]
    fn state_round_trip() {
        let mut emu = Emulator::new("48sx", &vec![0u8; 256 * 1024]).unwrap();
        let _ = emu.run_ms(1.0);
        let saved = emu.machine().save_state();
        let cycles = emu.machine().cycles();
        let _ = emu.run_ms(5.0);
        emu.load_state(&saved).unwrap();
        assert_eq!(emu.machine().cycles(), cycles);
        assert!(emu.load_state(&[1, 2, 3]).is_err());
    }

    #[test]
    fn hp42s_has_a_16_row_display_and_its_own_keys() {
        let mut emu = Emulator::new("42s", &vec![0u8; 64 * 1024]).unwrap();
        assert_eq!(emu.lcd_height(), 16);
        assert_eq!(emu.framebuffer().len(), 131 * 16);
        assert!(emu.key_down("xeq").is_ok());
        assert!(emu.key_down("exit").is_ok());
        assert!(
            emu.key_down("f1").is_err(),
            "the 42S's menu keys keep their labels"
        );
    }
}

// The README (the crate's page on crates.io) compiles as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
