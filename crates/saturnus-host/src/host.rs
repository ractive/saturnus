//! What a host needs around an [`Emulator`] to run it behind the front
//! end's command/event protocol (`web/protocol.md`): the key queue that
//! times presses and types letters in emulated time ([`KeyQueue`]), the
//! slice runner that feeds it, the change-detected `frame` and `keys`
//! events, and the choice of model for a ROM file.
//!
//! Every host uses this through [`crate::protocol`], which adds the
//! wall-clock pacing; everything here is in emulated time and has no I/O.

use std::collections::VecDeque;

use saturnus::io::Key;
use saturnus::{LCD_WIDTH, Machine, Model};

use crate::layout;
use crate::skins::{self, Typing};
use crate::{AnnunciatorFlags, Emulator, Error};

/// Shortest key press the ROM sees, in emulated ms (its debounce needs >10).
pub const MIN_HOLD_MS: f64 = 60.0;
/// Shortest pause between two queued key presses, in emulated ms.
pub const GAP_MS: f64 = 30.0;
/// How long a queued press waits for the ROM to go idle (SHUTDN) after
/// the previous key, in emulated ms; while the ROM stays busy longer, as
/// in a running program, the press goes in anyway. A press sent while
/// the ROM still handles the last key may be lost (the 48SX ROM drops
/// it), so the cap lies above the longest time each ROM stays awake
/// after an ordinary key: measured over 40 timer phases for a shift, a
/// digit, ENTER and a function key (kb: decision log, "Key waits per
/// model"), plus a margin; 300 ms where the ROM never stays awake that
/// long. The 48SX stays awake about 250 ms or, by its timer's phase,
/// about 525 ms after a shift, and up to 750 ms after √x.
pub fn busy_gap_ms(model: Model) -> f64 {
    match model {
        // Measured up to 753 ms (√x), 605 (ENTER), 583 (shift).
        Model::Hp48sx => 850.0,
        // Up to 521 ms (shift), 516 (ENTER).
        Model::Hp48gx => 600.0,
        // Up to 462 ms (ENTER).
        Model::Hp49g => 550.0,
        // Up to 968 ms (ENTER), 523 (a digit, √x, shift).
        Model::Hp38g => 1100.0,
        // Up to 244 ms (39G, 40G) and 78 ms (42S).
        Model::Hp39g | Model::Hp40g | Model::Hp42s => 300.0,
    }
}
/// Quiet time before a typed letter or a shifted press reads an
/// annunciator while the ROM stays busy, in emulated ms, at least: the
/// 48SX ROM blinks alpha while it redraws the command line, up to about
/// 250 ms after a key is released. Never shorter than the model's
/// [`busy_gap_ms`], which a queued press waits for first.
pub const LETTER_SETTLE_MS: f64 = 400.0;
/// Runs are cut in slices of this many emulated ms, so key timing is fine.
pub const SLICE_MS: f64 = 10.0;

/// The machine as the key queue sees it.
pub trait Keyboard {
    /// Emulated milliseconds since power-on.
    fn now_ms(&self) -> f64;
    /// True while the CPU sleeps in SHUTDN.
    fn idle(&self) -> bool;
    /// The alpha annunciator.
    fn alpha_on(&self) -> bool;
    /// The annunciator of shift key `shift` (`leftshift`, `rightshift`);
    /// a model with one `shift` key reports either.
    fn shift_on(&self, shift: &str) -> bool;
    /// Press the key named `name`.
    fn down(&mut self, name: &str) -> crate::Result<()>;
    /// Release the key named `name`.
    fn up(&mut self, name: &str);
}

impl Keyboard for Machine {
    fn now_ms(&self) -> f64 {
        self.cycles() as f64 * 1000.0 / f64::from(self.model().clock_hz())
    }

    fn idle(&self) -> bool {
        self.is_shutdown()
    }

    fn alpha_on(&self) -> bool {
        self.framebuffer().annunciators.alpha
    }

    fn shift_on(&self, shift: &str) -> bool {
        let a = self.framebuffer().annunciators;
        match shift {
            "leftshift" => a.left_shift,
            "rightshift" => a.right_shift,
            _ => a.left_shift || a.right_shift,
        }
    }

    fn down(&mut self, name: &str) -> crate::Result<()> {
        let k = Key::from_name(name).ok_or_else(|| format!("unknown key {name:?}"))?;
        Ok(self.key_down(k)?)
    }

    fn up(&mut self, name: &str) {
        if let Some(k) = Key::from_name(name) {
            // Only fails for a key the model lacks, which was never pressed.
            let _ = self.key_up(k);
        }
    }
}

/// One key press, queued or down.
#[derive(Clone, Debug)]
struct Press {
    name: &'static str,
    /// Released by the user (or a typed press, released once held long
    /// enough).
    up: bool,
    /// Emulated ms it went down.
    down_at: f64,
    /// Pressed by the queue for a typed letter or key sequence.
    typed: bool,
}

#[derive(Clone, Debug)]
enum Item {
    Press(Press),
    /// A letter, expanded into presses when its turn comes.
    Letter {
        letter: char,
        lower: bool,
    },
    /// A press of a key's shifted function (a modifier click): when its
    /// turn comes, the shift is tapped first unless its annunciator is on.
    Shifted {
        shift: &'static str,
        press: Press,
    },
}

/// The shift keys a [`KeyQueue::press_shifted`] may name.
const SHIFT_KEYS: [&str; 3] = ["leftshift", "rightshift", "shift"];

/// Key presses in emulated time: `active` keys are down in the machine;
/// `pending` presses (or letters, expanded when their turn comes) wait for
/// a fast-typed predecessor to be released first.
#[derive(Clone, Debug)]
pub struct KeyQueue {
    /// The model's keys (its drawn layout), the only names accepted.
    names: Vec<&'static str>,
    /// `None` on a model without typing data (the 42S types letters from
    /// its ALPHA menus), which then types no letters.
    typing: Option<Typing>,
    letters: Vec<(char, &'static str)>,
    active: Vec<Press>,
    pending: VecDeque<Item>,
    last_release_ms: f64,
    /// [`busy_gap_ms`] of the model.
    busy_gap_ms: f64,
    /// [`LETTER_SETTLE_MS`], or the model's busy gap if longer.
    settle_ms: f64,
    /// Alpha is known to be off: a typed letter pressed alpha and its key
    /// used it up, and no alpha press came since. Letters then skip reading
    /// the annunciator and the settling wait.
    alpha_spent: bool,
    /// Errors from presses the machine refused, for the host to report.
    errors: Vec<Error>,
}

impl KeyQueue {
    /// An empty queue for `model`'s keys and typing rules.
    pub fn new(model: Model) -> Self {
        Self {
            names: layout::layout(model).iter().map(|k| k.name).collect(),
            typing: skins::typing(model),
            letters: skins::letters(model),
            active: Vec::new(),
            pending: VecDeque::new(),
            last_release_ms: f64::NEG_INFINITY,
            busy_gap_ms: busy_gap_ms(model),
            settle_ms: busy_gap_ms(model).max(LETTER_SETTLE_MS),
            alpha_spent: false,
            errors: Vec::new(),
        }
    }

    fn name(&self, name: &str) -> Option<&'static str> {
        self.names.iter().copied().find(|&n| n == name)
    }

    /// Whether the model has a key called `name` (its drawn layout).
    pub fn has_key(&self, name: &str) -> bool {
        self.name(name).is_some()
    }

    /// Keys are down or presses are queued: the run must keep its slices
    /// short and pump.
    pub fn busy(&self) -> bool {
        !self.active.is_empty() || !self.pending.is_empty()
    }

    /// Queue a press of `name` (held until [`KeyQueue::release`]); false
    /// if the model has no such key.
    pub fn press(&mut self, name: &str) -> bool {
        let Some(name) = self.name(name) else {
            return false;
        };
        self.pending.push_back(Item::Press(Press {
            name,
            up: false,
            down_at: 0.0,
            typed: false,
        }));
        true
    }

    /// Queue a press of `name` held until [`KeyQueue::release`], after a
    /// tap of shift key `shift` unless that shift is on when the press's
    /// turn comes: decided then, not now, so a shift still queued or one
    /// a queued key will use up counts. False if the model has no such key
    /// or `shift` is not one of its shift keys.
    pub fn press_shifted(&mut self, name: &str, shift: &str) -> bool {
        let (Some(name), Some(shift)) = (self.name(name), self.name(shift)) else {
            return false;
        };
        if !SHIFT_KEYS.contains(&shift) {
            return false;
        }
        self.pending.push_back(Item::Shifted {
            shift,
            press: Press {
                name,
                up: false,
                down_at: 0.0,
                typed: false,
            },
        });
        true
    }

    /// Queue `ch` typed through the calculator's alpha mode; false if the
    /// model has no key for it.
    pub fn type_letter(&mut self, ch: char) -> bool {
        let upper = ch.to_ascii_uppercase();
        if self.typing.is_none() || !self.letters.iter().any(|&(c, _)| c == upper) {
            return false;
        }
        self.pending.push_back(Item::Letter {
            letter: upper,
            lower: ch != upper,
        });
        true
    }

    /// Queue full presses of `names`, one after the other (unknown names
    /// are skipped).
    pub fn type_keys(&mut self, names: &[&str]) {
        for n in names {
            if let Some(name) = self.name(n) {
                self.pending.push_back(Item::Press(Press {
                    name,
                    up: true,
                    down_at: 0.0,
                    typed: true,
                }));
            }
        }
    }

    /// Release the newest press of `name` that the user still holds.
    pub fn release(&mut self, name: &str) {
        let queued = self.pending.iter_mut().rev().filter_map(|i| match i {
            Item::Press(p) | Item::Shifted { press: p, .. } => Some(p),
            Item::Letter { .. } => None,
        });
        if let Some(p) = queued
            .chain(self.active.iter_mut().rev())
            .find(|p| p.name == name && !p.up)
        {
            p.up = true;
        }
    }

    /// Release every held key once it has been down long enough (the
    /// window lost the focus).
    pub fn release_held(&mut self) {
        for p in &mut self.active {
            p.up = true;
        }
        for i in &mut self.pending {
            if let Item::Press(p) | Item::Shifted { press: p, .. } = i {
                p.up = true;
            }
        }
    }

    /// Drop the queue and forget every key; the machine's keys stay as
    /// they are ([`Emulator::release_keys`] releases both).
    pub fn clear(&mut self) {
        self.active.clear();
        self.pending.clear();
        self.last_release_ms = f64::NEG_INFINITY;
        self.alpha_spent = false;
    }

    /// The keys down now, for the view.
    pub fn down(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = self.active.iter().map(|p| p.name).collect();
        v.dedup();
        v
    }

    /// Errors collected since the last call.
    pub fn take_errors(&mut self) -> Vec<Error> {
        std::mem::take(&mut self.errors)
    }

    /// The presses that type a queued letter now: the letter's key after
    /// the alpha key, unless alpha is already on (one alpha press lasts for
    /// the next key; two lock it on the 48 and 49G, cancel it on the
    /// others); a lowercase letter adds the model's shift, before or after
    /// alpha as the model wants. Alpha is read from the annunciator, except
    /// right after a letter this queue pressed alpha for: that alpha was
    /// spent by its key.
    fn expand_letter(&mut self, kb: &impl Keyboard, letter: char, lower: bool) -> Vec<Item> {
        let Some(t) = self.typing else {
            return Vec::new();
        };
        let Some(&(_, key)) = self.letters.iter().find(|&&(c, _)| c == letter) else {
            return Vec::new();
        };
        let alpha_on = !self.alpha_spent && kb.alpha_on();
        let mut seq = Vec::new();
        if t.shift_first && lower {
            seq.push(t.lower_shift);
        }
        if !alpha_on {
            seq.push(t.alpha);
        }
        if !t.shift_first && lower {
            seq.push(t.lower_shift);
        }
        seq.push(key);
        self.alpha_spent = !alpha_on;
        seq.into_iter()
            .filter_map(|n| self.name(n))
            .map(|name| {
                Item::Press(Press {
                    name,
                    up: true,
                    down_at: 0.0,
                    typed: true,
                })
            })
            .collect()
    }

    /// Release keys held long enough, then start queued presses.
    pub fn pump(&mut self, kb: &mut impl Keyboard) {
        let now = kb.now_ms();
        let mut released = false;
        self.active.retain(|p| {
            if p.up && now - p.down_at >= MIN_HOLD_MS {
                kb.up(p.name);
                released = true;
                false
            } else {
                true
            }
        });
        if released {
            self.last_release_ms = now;
        }
        let idle = kb.idle();
        while let Some(head) = self.pending.front() {
            // Wait while a fast-typed key is still on its way up; a key the
            // user keeps holding (ON for a chord) does not block.
            if self.active.iter().any(|p| p.up) {
                break;
            }
            let since = now - self.last_release_ms;
            if since < GAP_MS || (!idle && since < self.busy_gap_ms) {
                break;
            }
            if let &Item::Letter { letter, lower } = head {
                // A letter waits for an empty keyboard, and, unless it
                // follows a letter whose alpha this queue pressed, for the
                // ROM to settle the alpha annunciator it reads.
                if !self.active.is_empty() {
                    break;
                }
                if !self.alpha_spent && !idle && since < self.settle_ms {
                    break;
                }
                self.pending.pop_front();
                for item in self.expand_letter(kb, letter, lower).into_iter().rev() {
                    self.pending.push_front(item);
                }
                continue;
            }
            if let &Item::Shifted { shift, .. } = head {
                // As a letter: the shift annunciator is read with every key
                // before it played and settled (a shift queued before it
                // lit it, a key before it used it up).
                if !self.active.is_empty() || (!idle && since < self.settle_ms) {
                    break;
                }
                let Some(Item::Shifted { press, .. }) = self.pending.pop_front() else {
                    continue;
                };
                self.pending.push_front(Item::Press(press));
                if !kb.shift_on(shift) {
                    self.pending.push_front(Item::Press(Press {
                        name: shift,
                        up: true,
                        down_at: 0.0,
                        typed: true,
                    }));
                }
                continue;
            }
            let Some(Item::Press(mut p)) = self.pending.pop_front() else {
                continue;
            };
            if self.typing.is_some_and(|t| p.name == t.alpha) && !p.typed {
                self.alpha_spent = false;
            }
            if let Some(o) = self.active.iter_mut().find(|o| o.name == p.name) {
                // Same key still down (held): count this press as the same one.
                if p.up {
                    o.up = true;
                }
                continue;
            }
            if let Err(e) = kb.down(p.name) {
                self.errors.push(e);
                continue;
            }
            p.down_at = now;
            self.active.push(p);
        }
    }
}

/// Packed LCD bytes per row of `width` pixels.
pub fn packed_row_bytes(width: usize) -> usize {
    width.div_ceil(8)
}

/// The pixels packed one bit each, row-major from the top-left, each row
/// starting on a byte, the leftmost pixel in the most significant bit,
/// 1 = dark (`web/protocol.md`, `frame`).
pub fn pack_bits(rows: &[[bool; LCD_WIDTH]]) -> Vec<u8> {
    let w = LCD_WIDTH;
    let mut out = vec![0u8; packed_row_bytes(w) * rows.len()];
    for (y, row) in rows.iter().enumerate() {
        for (x, &on) in row.iter().enumerate() {
            if on {
                out[y * packed_row_bytes(w) + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    out
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i)) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decode standard base64 (padding optional, no whitespace): the JSON form
/// of the protocol's *bytes* fields.
pub fn base64_decode(s: &str) -> crate::Result<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        Some(u32::from(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        }))
    }
    let s = s.trim_end_matches('=').as_bytes();
    if s.len() % 4 == 1 {
        return Err("bad base64 length".into());
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= value(c).ok_or("bad base64 character")? << (18 - 6 * i);
        }
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Ok(out)
}

/// Serialize packed pixels as base64 (`web/protocol.md`, `frame`).
fn as_base64<S: serde::Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&base64(bytes))
}

/// The `frame` event (`web/protocol.md`): the display, sent when it
/// changed. Serializes as `{"type":"frame","width","height","pixels",
/// "annunciators","contrast","contrastRange","contrastDefault"}`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename = "frame", rename_all = "camelCase")]
pub struct Frame {
    /// Pixels per row: 131.
    pub width: usize,
    /// Rows: 64, or 16 on the 42S.
    pub height: usize,
    /// The pixels packed one bit each ([`pack_bits`]); base64 in JSON.
    #[serde(serialize_with = "as_base64")]
    pub pixels: Vec<u8>,
    /// The annunciators.
    pub annunciators: AnnunciatorFlags,
    /// Raw 5-bit contrast, 0-31, higher is darker.
    pub contrast: u8,
    /// The model's usable contrast range, `[low, high]`.
    pub contrast_range: [u8; 2],
    /// The contrast the ROM sets at power-on.
    pub contrast_default: u8,
}

/// The `keys` event (`web/protocol.md`): the keys down, sent when they
/// changed. Serializes as `{"type":"keys","down":[names]}`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename = "keys")]
pub struct KeysDown {
    /// Script names of the keys down.
    pub down: Vec<&'static str>,
}

/// The model that runs `rom`, preferring `preferred`: a 2 MB file fits
/// both the 49G (packed) and the 39G/40G (unpacked, every byte a nibble);
/// the bytes tell which ([`crate::romid::fitting_models`]). `preferred`
/// when nothing fits (the boot then reports the size).
pub fn model_for_rom(rom: &[u8], preferred: Model) -> Model {
    let fits = crate::romid::fitting_models(rom);
    if fits.contains(&preferred) {
        return preferred;
    }
    fits.first().copied().unwrap_or(preferred)
}

impl Emulator {
    /// One slice of at most `left_ms` emulated ms: a sleeping CPU with no
    /// keys to time skips to its next timer event in one step; with `keys`
    /// the queue is fed afterwards. Returns the emulated ms run.
    pub fn run_slice(&mut self, left_ms: f64, keys: bool) -> crate::Result<f64> {
        if !left_ms.is_finite() || left_ms <= 0.0 {
            return Ok(0.0);
        }
        let timed = keys && self.queue.busy();
        let idle = if timed { None } else { self.idle_ms() };
        let step = left_ms.min(idle.filter(|&i| i > SLICE_MS).unwrap_or(SLICE_MS));
        self.run_ms(step)?;
        if keys {
            self.queue.pump(&mut self.machine);
        }
        Ok(step)
    }

    /// Feed the key queue now (after a command added to it).
    pub fn pump(&mut self) {
        self.queue.pump(&mut self.machine);
    }

    /// The key queue.
    pub fn queue(&mut self) -> &mut KeyQueue {
        &mut self.queue
    }

    /// Release every key at once: the machine's keyboard (matrix and ON)
    /// and the key queue (held presses, presses and letters waiting, with
    /// no minimum hold), as after a reset, a state load or before a
    /// script. The one way to let go of everything; [`KeyQueue::release_held`]
    /// is the gentle one (the window lost the focus).
    pub fn release_keys(&mut self) {
        self.machine.release_all_keys();
        self.queue.clear();
    }

    /// The `frame` event if the display changed since the last one
    /// (pixels, annunciators or contrast), else `None`.
    pub fn frame_if_changed(&mut self) -> Option<Frame> {
        let fb = self.machine.framebuffer();
        let model = self.machine.model();
        let range = model.contrast_range();
        let frame = Frame {
            width: LCD_WIDTH,
            height: fb.pixels.pixels.len(),
            pixels: pack_bits(&fb.pixels.pixels),
            annunciators: AnnunciatorFlags(fb.annunciators),
            contrast: fb.contrast,
            contrast_range: [*range.start(), *range.end()],
            contrast_default: model.default_contrast(),
        };
        if self.shown.as_ref() == Some(&frame) {
            return None;
        }
        self.shown = Some(frame.clone());
        Some(frame)
    }

    /// Forget what was shown, so the next [`Emulator::frame_if_changed`]
    /// and [`Emulator::keys_if_changed`] send their events (a new view
    /// subscribed, a state was loaded).
    pub fn reshow(&mut self) {
        self.shown = None;
        self.shown_keys = None;
    }

    /// The `keys` event if the keys down changed since the last one, else
    /// `None`.
    pub fn keys_if_changed(&mut self) -> Option<KeysDown> {
        let down = self.queue.down();
        if self.shown_keys.as_deref() == Some(down.as_slice()) {
            return None;
        }
        self.shown_keys = Some(down.clone());
        Some(KeysDown { down })
    }
}

impl Emulator {
    /// Queue a press of `name`, held until `release(name)`; false if the
    /// model has no such key. Takes effect at the next `pump`.
    pub fn press(&mut self, name: &str) -> bool {
        self.queue.press(name)
    }

    /// Queue a press of `name` after a tap of shift key `shift`, unless
    /// that shift is on when it plays ([`KeyQueue::press_shifted`]); false
    /// if the model has no such key or shift.
    pub fn press_shifted(&mut self, name: &str, shift: &str) -> bool {
        self.queue.press_shifted(name, shift)
    }

    /// Whether the model has a key called `name`.
    pub fn has_key(&self, name: &str) -> bool {
        self.queue.has_key(name)
    }

    /// Release the newest held press of `name` (at the next `pump`).
    pub fn release(&mut self, name: &str) {
        self.queue.release(name);
    }

    /// Release every held key once it was down long enough.
    pub fn release_held(&mut self) {
        self.queue.release_held();
    }

    /// Queue the first character of `ch` typed through alpha mode; false
    /// if the model has no key for it.
    pub fn type_letter(&mut self, ch: &str) -> bool {
        ch.chars().next().is_some_and(|c| self.queue.type_letter(c))
    }

    /// Queue full presses of the space-separated key `names`.
    pub fn type_keys(&mut self, names: &str) {
        let names: Vec<&str> = names.split_whitespace().collect();
        self.queue.type_keys(&names);
    }

    /// Keys are down or queued: run in short slices.
    pub fn keys_busy(&self) -> bool {
        self.queue.busy()
    }

    /// Errors from refused key presses since the last call.
    pub fn take_errors(&mut self) -> Vec<Error> {
        self.queue.take_errors()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keyboard that records presses; time and idleness are set by hand.
    /// Its shift annunciators follow the keys as a ROM's do: a shift key
    /// turns its shift on (the other off), again off; any other key uses it
    /// up.
    #[derive(Default)]
    struct Fake {
        now: f64,
        idle: bool,
        /// The ROM stays awake this long after each key goes up.
        busy_after_up: f64,
        last_up: f64,
        /// Keys pressed while the ROM was awake (it may drop them).
        pressed_busy: Vec<String>,
        /// When each key went down.
        down_at: Vec<f64>,
        alpha: bool,
        left: bool,
        right: bool,
        log: Vec<String>,
    }

    impl Keyboard for Fake {
        fn now_ms(&self) -> f64 {
            self.now
        }
        fn idle(&self) -> bool {
            self.idle && self.now >= self.last_up + self.busy_after_up
        }
        fn alpha_on(&self) -> bool {
            self.alpha
        }
        fn shift_on(&self, shift: &str) -> bool {
            match shift {
                "rightshift" => self.right,
                "leftshift" => self.left,
                _ => self.left || self.right,
            }
        }
        fn down(&mut self, name: &str) -> crate::Result<()> {
            self.log.push(format!("+{name}"));
            self.down_at.push(self.now);
            if !self.idle() {
                self.pressed_busy.push(name.to_string());
            }
            match name {
                "leftshift" | "shift" => (self.left, self.right) = (!self.left, false),
                "rightshift" => (self.left, self.right) = (false, !self.right),
                _ => (self.left, self.right) = (false, false),
            }
            Ok(())
        }
        fn up(&mut self, name: &str) {
            self.log.push(format!("-{name}"));
            self.last_up = self.now;
        }
    }

    /// Run the fake in 10 ms steps for `ms`, pumping after each.
    fn run(q: &mut KeyQueue, kb: &mut Fake, ms: f64) {
        let end = kb.now + ms;
        while kb.now < end {
            kb.now += 10.0;
            q.pump(kb);
        }
    }

    #[test]
    fn a_press_is_held_at_least_min_hold() {
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake {
            idle: true,
            ..Fake::default()
        };
        assert!(q.press("enter"));
        assert!(!q.press("apps"), "a 49G key on a 48SX");
        q.pump(&mut kb);
        assert_eq!(kb.log, ["+enter"]);
        assert_eq!(q.down(), ["enter"]);
        q.release("enter");
        kb.now = 50.0;
        q.pump(&mut kb);
        assert_eq!(kb.log, ["+enter"], "released before 60 ms");
        kb.now = 60.0;
        q.pump(&mut kb);
        assert_eq!(kb.log, ["+enter", "-enter"]);
        assert!(!q.busy());
    }

    #[test]
    fn queued_presses_wait_for_the_gap_and_a_busy_rom() {
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake::default();
        q.type_keys(&["1", "2"]);
        q.pump(&mut kb);
        assert_eq!(kb.log, ["+1"]);
        // Busy ROM: the next press waits the 48SX's busy gap after the release at 60.
        run(&mut q, &mut kb, 50.0 + busy_gap_ms(Model::Hp48sx));
        assert_eq!(kb.log, ["+1", "-1"]);
        run(&mut q, &mut kb, 20.0);
        assert_eq!(kb.log, ["+1", "-1", "+2"]);
        // Idle ROM: only GAP_MS.
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake {
            idle: true,
            ..Fake::default()
        };
        q.type_keys(&["1", "2"]);
        // Down at 10, up at 70, the next one 30 ms later.
        run(&mut q, &mut kb, 100.0);
        assert_eq!(kb.log, ["+1", "-1", "+2"]);
    }

    #[test]
    fn a_busy_rom_takes_keys_at_its_models_rate() {
        // A program keeps the ROM awake: each queued key is held 60 ms and
        // the next waits the model's busy gap, so the 39G (which sleeps
        // within 250 ms of a key) takes keys every 360 ms, about 2.8 a
        // second, and the 48SX, which may stay awake 750 ms, every 910 ms.
        for (model, every) in [
            (Model::Hp39g, 360.0),
            (Model::Hp42s, 360.0),
            (Model::Hp48sx, 910.0),
        ] {
            let mut q = KeyQueue::new(model);
            let mut kb = Fake::default();
            q.type_keys(&["1", "2", "3", "4"]);
            run(&mut q, &mut kb, 5000.0);
            assert_eq!(downs(&kb), ["1", "2", "3", "4"], "{model:?}");
            let gaps: Vec<f64> = kb.down_at.windows(2).map(|w| w[1] - w[0]).collect();
            assert_eq!(gaps, [every; 3], "{model:?}");
        }
    }

    /// The keys pressed, in order.
    fn downs(kb: &Fake) -> Vec<&str> {
        kb.log.iter().filter_map(|s| s.strip_prefix('+')).collect()
    }

    #[test]
    fn two_quick_shifted_presses_each_get_their_shift() {
        // Two Ctrl+clicks on √x in quick succession: both are queued before
        // the first plays, and the first one's shift is spent by its key,
        // so the second taps the shift again.
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake {
            idle: true,
            ..Fake::default()
        };
        assert!(q.press_shifted("sqrt", "leftshift"));
        q.pump(&mut kb);
        q.release("sqrt");
        assert!(q.press_shifted("sqrt", "leftshift"));
        q.release("sqrt");
        run(&mut q, &mut kb, 1000.0);
        assert_eq!(downs(&kb), ["leftshift", "sqrt", "leftshift", "sqrt"]);
        assert!(!q.busy());
    }

    #[test]
    fn quick_shifted_presses_wait_out_the_roms_long_key_wait() {
        // The 48SX ROM, depending on its timer's phase, stays awake about
        // 520 ms after a key goes up and may drop a key pressed then: two
        // quick Ctrl+clicks lost the second shift and gave √x for x²
        // (the page test under load). Every press waits for the ROM.
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake {
            idle: true,
            busy_after_up: 520.0,
            last_up: f64::NEG_INFINITY,
            ..Fake::default()
        };
        assert!(q.press_shifted("sqrt", "leftshift"));
        q.release("sqrt");
        assert!(q.press_shifted("sqrt", "leftshift"));
        q.release("sqrt");
        run(&mut q, &mut kb, 5000.0);
        assert_eq!(downs(&kb), ["leftshift", "sqrt", "leftshift", "sqrt"]);
        assert_eq!(
            kb.pressed_busy,
            Vec::<String>::new(),
            "no key pressed while the ROM was awake"
        );
        assert!(!q.busy());
    }

    #[test]
    fn a_shifted_press_after_a_queued_shift_does_not_tap_it_again() {
        // A click on the left shift, then at once a Ctrl+click: the shift
        // is still queued, then on when the shifted press plays.
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake {
            idle: true,
            ..Fake::default()
        };
        assert!(q.press("leftshift"));
        q.release("leftshift");
        assert!(q.press_shifted("sqrt", "leftshift"));
        q.release("sqrt");
        run(&mut q, &mut kb, 1000.0);
        assert_eq!(downs(&kb), ["leftshift", "sqrt"]);
        // The right shift on, a left-shifted press: the left one is tapped.
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake {
            idle: true,
            right: true,
            ..Fake::default()
        };
        assert!(q.press_shifted("sqrt", "leftshift"));
        q.release("sqrt");
        run(&mut q, &mut kb, 1000.0);
        assert_eq!(downs(&kb), ["leftshift", "sqrt"]);
    }

    #[test]
    fn a_shifted_press_is_held_until_released_and_checks_its_names() {
        let mut q = KeyQueue::new(Model::Hp39g);
        let mut kb = Fake {
            idle: true,
            ..Fake::default()
        };
        assert!(
            !q.press_shifted("enter", "leftshift"),
            "no left shift on a 39G"
        );
        assert!(!q.press_shifted("enter", "alpha"), "not a shift key");
        assert!(!q.press_shifted("bogus", "shift"));
        assert!(q.press_shifted("enter", "shift"));
        run(&mut q, &mut kb, 500.0);
        assert_eq!(downs(&kb), ["shift", "enter"]);
        assert_eq!(q.down(), ["enter"], "held while the button is");
        q.release("enter");
        run(&mut q, &mut kb, 100.0);
        assert!(!q.busy());
        // One shift: either annunciator counts as on.
        let mut q = KeyQueue::new(Model::Hp39g);
        let mut kb = Fake {
            idle: true,
            right: true,
            ..Fake::default()
        };
        assert!(q.press_shifted("enter", "shift"));
        q.release("enter");
        run(&mut q, &mut kb, 500.0);
        assert_eq!(downs(&kb), ["enter"]);
    }

    #[test]
    fn letters_press_alpha_then_skip_it_while_spent() {
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake {
            idle: true,
            ..Fake::default()
        };
        assert!(q.type_letter('A'));
        assert!(q.type_letter('b'));
        assert!(!q.type_letter('!'));
        run(&mut q, &mut kb, 2000.0);
        let downs: Vec<&str> = kb.log.iter().filter_map(|s| s.strip_prefix('+')).collect();
        // A = alpha + key; b = alpha + left shift + key (shift after alpha).
        let a = skins::letters(Model::Hp48sx)
            .into_iter()
            .find(|&(c, _)| c == 'A')
            .map(|(_, k)| k)
            .unwrap();
        let b = skins::letters(Model::Hp48sx)
            .into_iter()
            .find(|&(c, _)| c == 'B')
            .map(|(_, k)| k)
            .unwrap();
        assert_eq!(downs, ["alpha", a, "alpha", "leftshift", b]);
        assert!(!q.busy());
    }

    #[test]
    fn aplet_models_shift_before_alpha_and_respect_alpha_on() {
        let mut q = KeyQueue::new(Model::Hp38g);
        let mut kb = Fake {
            idle: true,
            alpha: true,
            ..Fake::default()
        };
        q.type_letter('c');
        run(&mut q, &mut kb, 1000.0);
        let downs: Vec<&str> = kb.log.iter().filter_map(|s| s.strip_prefix('+')).collect();
        // Alpha already on: shift, then the key, no alpha press.
        assert_eq!(downs.len(), 2);
        assert_eq!(downs[0], "shift");
    }

    #[test]
    fn release_held_lets_go_of_user_presses() {
        let mut q = KeyQueue::new(Model::Hp48sx);
        let mut kb = Fake {
            idle: true,
            ..Fake::default()
        };
        q.press("on");
        q.pump(&mut kb);
        q.release_held();
        run(&mut q, &mut kb, 100.0);
        assert_eq!(kb.log, ["+on", "-on"]);
    }

    #[test]
    fn packs_bits_msb_first_per_row() {
        let mut rows = vec![[false; LCD_WIDTH]; 2];
        rows[0][0] = true;
        rows[0][9] = true;
        rows[1][130] = true;
        let p = pack_bits(&rows);
        assert_eq!(p.len(), 2 * 17);
        assert_eq!(p[0], 0x80);
        assert_eq!(p[1], 0x40);
        assert_eq!(p[17 + 16], 0x20);
        assert_eq!(p.iter().map(|b| b.count_ones()).sum::<u32>(), 3);
    }

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe]), "//4=");
        for n in 0..40 {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(base64_decode(&base64(&bytes)).unwrap(), bytes);
        }
        assert_eq!(base64_decode("aGk").unwrap(), b"hi");
        assert!(base64_decode("a").is_err());
        assert!(base64_decode("a*bc").is_err());
    }

    #[test]
    fn rom_picks_the_model() {
        let two = 2 * 1024 * 1024;
        assert_eq!(
            model_for_rom(&vec![0u8; 256 * 1024], Model::Hp49g),
            Model::Hp48sx
        );
        assert_eq!(
            model_for_rom(&vec![0u8; 256 * 1024], Model::Hp48sx),
            Model::Hp48sx
        );
        // 2 MB of nibbles: an unpacked 39G/40G, not a 49G.
        assert_eq!(model_for_rom(&vec![0u8; two], Model::Hp49g), Model::Hp39g);
        assert_eq!(model_for_rom(&vec![0u8; two], Model::Hp40g), Model::Hp40g);
        // 2 MB with full bytes: the 49G.
        assert_eq!(
            model_for_rom(&vec![0xffu8; two], Model::Hp39g),
            Model::Hp49g
        );
        assert_eq!(model_for_rom(&[0u8; 1000], Model::Hp38g), Model::Hp38g);
    }

    #[test]
    fn frames_and_keys_only_when_changed() {
        let mut emu = Emulator::new(Model::Hp48sx, &vec![0u8; 256 * 1024]).unwrap();
        let f = serde_json::to_string(&emu.frame_if_changed().unwrap()).unwrap();
        assert!(
            f.starts_with("{\"type\":\"frame\",\"width\":131,\"height\":64,\"pixels\":\"AAAA"),
            "{f}"
        );
        assert!(f.contains(",\"annunciators\":{\"leftshift\":false,"), "{f}");
        assert!(
            f.ends_with(",\"contrastRange\":[3,19],\"contrastDefault\":11}"),
            "{f}"
        );
        assert!(emu.frame_if_changed().is_none());
        emu.reshow();
        assert!(emu.frame_if_changed().is_some());
        let keys = |emu: &mut Emulator| {
            emu.keys_if_changed()
                .map(|k| serde_json::to_string(&k).unwrap())
        };
        assert_eq!(
            keys(&mut emu).as_deref(),
            Some("{\"type\":\"keys\",\"down\":[]}")
        );
        assert!(emu.keys_if_changed().is_none());
        emu.queue().press("enter");
        emu.pump();
        assert_eq!(
            keys(&mut emu).as_deref(),
            Some("{\"type\":\"keys\",\"down\":[\"enter\"]}")
        );
    }
}
