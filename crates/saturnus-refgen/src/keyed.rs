//! The reference of a key-driven model (the 42S, 27S, 17BII, 17B,
//! 19BII, and the 38G, 39G and 40G): models without a command line to
//! type into, where a function is a key, a shifted key, a menu key under
//! a menu, (42S) XEQ and its name, or (38G, 39G, 40G) a key or a MATH
//! menu entry that puts its name on HOME's edit line. Written by hand as `crates/saturnus-cli/data/commands/keys-<model>.json`;
//! [`generate`] checks it on the emulator and runs its examples into
//! `examples-<model>.json`.
//!
//! # The data file, `keys-<model>.json`
//!
//! ```text
//! {
//!   "model": "42s",
//!   "manual": "hp42s-om",            // its id in manuals.json
//!   "menu_keys": ["sigmaplus", ...], // the keys under the menu boxes, left to right
//!   "marks": ["■"],                   // optional: what a menu box adds to a label when its setting is on
//!   "ready": [steps],                 // optional: after the boot, into the normal state
//!   "home": [steps],                  // optional: pressed (by the app) before every
//!                                     // function, after leaving the menus (the 38G's HOME)
//!   "leave": {"keys": [steps], "menu_box": {"x": 0, "width": 21, "rows": [9, 15]}},
//!                                     // optional: the keys that leave a menu, pressed (by
//!                                     // the app) while the screen shows one: dots x to
//!                                     // x+width-1 dark on each of the rows. Without
//!                                     // "menu_box" (a model whose normal state shows a
//!                                     // menu, the 17BII's MAIN) the keys go back to the
//!                                     // normal state from anywhere and are pressed once
//!   "learn": [{"keys": [steps], "lines": ["y:0.0000", "x:0.0000"], "menu": ["Σ+", ...]}],
//!                                     // optional: screens of known text, to read the fonts
//!                                     // ("lines" the text lines above the menu, if one shows)
//!   "alpha": {                        // optional: typing characters
//!     "start": [steps],               //   from the normal state into letter entry
//!     "chars": {"A": [steps], ...}    //   each character from there
//!   },
//!   "xeq": {"start": [steps], "end": [steps], "unknown": "Nonexistent"},
//!                                     // optional: running a function by name, and
//!                                     // what the screen says for a name it does not know
//!   "commands": [{
//!     "name": "→POL",                // as the manual's index names it
//!     "long": "To polar",            // optional: what the name stands for
//!     "description": "...",          // ours, plain and short
//!     "note": "...",                 // optional: shown after the description (a ROM bug)
//!     "stack": "x y → r θ",          // optional: the stack effect
//!     "param": "register or variable", // optional: what it asks for after the keys
//!     "keys": [steps],               // the keys from the normal state, or
//!     "xeq": "NAME",                 //   (no keys) the name XEQ runs it by; neither
//!                                    //   for a function only typed into an equation
//!     "menu": "CONVERT",             // optional: the menu path as the manual names it
//!     "inserts": "COMB(",            // optional: what the keys put on the edit line
//!     "page": 91,                    // the manual's PDF page
//!     "examples": [{"input": ["4", "enter", "3"], "then": ["enter"]}]
//!                                     // optional; `then`: keys after the function's
//!   }]
//! }
//! ```
//!
//! A step is `{"key": NAME}` or `{"key": NAME, "label": TEXT}`: `NAME` is
//! the key's script name (the skin's, `saturnus_core::io::Key::name`),
//! `TEXT` what the step shows: the key's legend (its shifted legend after
//! a shift key), or the label of the menu box above a menu key. Every
//! step but a shift has a label; the keys read as the labels in order,
//! a shift as `⇧`: `⇧ CONVERT →POL`. A box may show one of `marks`
//! after its label, or in place of its last letter (`DEG■`, `WRA■`). A
//! letter key may also be labelled with its alpha letter, and a key with
//! the text printed below it (the 38G's `CANCEL`). The normal state is the
//! calculator as it boots and `ready` leaves it: no menu (or, where the
//! calculator always shows one, its top menu: MAIN on the 17BII, 17B and
//! 19BII), no message, no entry in progress. A function run
//! by name is `xeq.start`, `alpha.start`, the name's characters from
//! `alpha.chars`, `xeq.end`.
//!
//! # What [`generate`] checks and writes
//!
//! From the booted state for each function: every key exists on the
//! model, every label is the key's legend or what the menu box shows
//! (read from the screen, [`crate::lcdtext`]); then each example's
//! `input` keys, the screen read (`before`), the function's keys, the
//! screen read again (`after`). When the input presses a menu key under a
//! menu, the example also keeps what each input key showed (`labels`),
//! so it reads as `BUS %CHG 100 OLD` rather than as key names. A function run by name must not leave
//! `xeq.unknown` on the screen, and `leave.menu_box` must see a menu
//! exactly when one shows. The fonts are learned first from the
//! `learn` screens and the characters of `alpha.chars`, typed one by one
//! (each shows in the first cell of the upper line).
//!
//! On the 38G, 39G and 40G ([`crate::apletext`]) the screen is HOME: its
//! four history lines and the edit line (`learn` lines are those five, a
//! history line spaced as it shows). A function's `inserts` must be what
//! its keys leave on the edit line, and no menu (`leave.menu_box`) may
//! show after them. An example whose `then` ends with ENTER teaches the
//! history's font: the edit line, read just before, must come back as
//! the last entry's input. No example may show a cell never learned.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use saturnus_core::io::Key;
use saturnus_core::{Lcd, Machine, Model};
use saturnus_drive::session::Session;
use serde::{Deserialize, Serialize};

use crate::apletext;
use crate::lcdtext::{self, Glyphs};

/// One key press of a function's keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// The key's script name.
    pub key: String,
    /// What it shows: the legend, or the menu box's label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Typing characters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alpha {
    /// From the normal state into letter entry.
    pub start: Vec<Step>,
    /// Each character, from letter entry (where it returns after one).
    pub chars: BTreeMap<String, Vec<Step>>,
}

/// Running a function by name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Xeq {
    /// Before the name (and `alpha.start`).
    pub start: Vec<Step>,
    /// After it.
    pub end: Vec<Step>,
    /// What the upper line shows when the name is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown: Option<String>,
}

/// Leaving menus before a function is pressed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Leave {
    /// The keys that leave one menu level, or (without `menu_box`) that
    /// go back to the normal state from any menu.
    pub keys: Vec<Step>,
    /// Where the screen shows that a menu is open; none on a model whose
    /// normal state shows a menu (the 17BII's MAIN), where `keys` are
    /// pressed once before every function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_box: Option<MenuBox>,
}

/// A row of dots that are all dark when a menu shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuBox {
    pub x: usize,
    pub width: usize,
    pub rows: Vec<usize>,
}

impl MenuBox {
    /// Whether `lcd` shows a menu by this test.
    pub fn shown(&self, lcd: &Lcd) -> bool {
        self.rows.iter().all(|&y| {
            (self.x..self.x + self.width).all(|x| {
                lcd.pixels
                    .get(y)
                    .and_then(|r| r.get(x))
                    .copied()
                    .unwrap_or(false)
            })
        })
    }
}

/// A screen of known text, to learn the font from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Learn {
    /// The keys from the normal state.
    pub keys: Vec<Step>,
    /// The text lines, upper first, one character per cell.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    /// The menu boxes' labels, left to right; each may have one letter
    /// not learned yet, or in another form, or new letters that stand
    /// apart (a blank column between them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menu: Vec<String>,
}

/// An example to run: keys pressed before the function's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExampleSpec {
    /// Key names, from the normal state.
    pub input: Vec<String>,
    /// Key names pressed after the function's (an algebraic entry's
    /// arguments and ENTER).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub then: Vec<String>,
}

/// One function.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    pub description: String,
    /// A remark shown after the description, such as a ROM bug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<Step>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xeq: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<String>,
    /// What the keys put on the edit line (38G, 39G, 40G).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inserts: Option<String>,
    pub page: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<ExampleSpec>,
}

/// A `keys-<model>.json` file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyFile {
    pub model: String,
    pub manual: String,
    pub menu_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ready: Vec<Step>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub home: Vec<Step>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub learn: Vec<Learn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leave: Option<Leave>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<Alpha>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xeq: Option<Xeq>,
    pub commands: Vec<Command>,
}

/// Whether `key` is a shift key.
pub fn is_shift(key: &str) -> bool {
    matches!(key, "shift" | "leftshift" | "rightshift")
}

impl KeyFile {
    /// The keys that type `text` from letter entry.
    pub fn spell(&self, text: &str) -> Result<Vec<Step>> {
        let alpha = self
            .alpha
            .as_ref()
            .context("the file has no \"alpha\" to type names with")?;
        let mut out = Vec::new();
        for c in text.chars() {
            let steps = alpha
                .chars
                .get(&c.to_string())
                .with_context(|| format!("no keys type {c:?}"))?;
            out.extend(steps.iter().cloned());
        }
        Ok(out)
    }

    /// The keys that run `cmd` from the normal state: its `keys`, or XEQ
    /// and its name spelt; none for a function only typed into an
    /// equation.
    pub fn sequence(&self, cmd: &Command) -> Result<Vec<Step>> {
        match (&cmd.keys, &cmd.xeq) {
            (Some(k), None) => Ok(k.clone()),
            (None, Some(name)) => {
                let xeq = self
                    .xeq
                    .as_ref()
                    .context("the file has no \"xeq\" to run names with")?;
                let alpha = self.alpha.as_ref().context("the file has no \"alpha\"")?;
                let mut out = xeq.start.clone();
                out.extend(alpha.start.iter().cloned());
                out.extend(self.spell(name)?);
                out.extend(xeq.end.iter().cloned());
                Ok(out)
            }
            // Typed into an equation (the Solver's functions), not pressed.
            (None, None) => Ok(Vec::new()),
            _ => bail!("{}: give either \"keys\" or \"xeq\", not both", cmd.name),
        }
    }
}

/// The steps as they read: labels in order, `⇧` for a shift.
pub fn keys_text(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|s| match &s.label {
            Some(l) => l.clone(),
            None if is_shift(&s.key) => "⇧".to_string(),
            None => s.key.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The file's shape: every step but a shift has a label, every function
/// has keys or a name, names are unique.
pub fn lint(file: &KeyFile) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    let check = |where_: &str, steps: &[Step]| -> Result<()> {
        for s in steps {
            if s.label.is_none() && !is_shift(&s.key) {
                bail!("{where_}: the step {:?} needs a label", s.key);
            }
        }
        Ok(())
    };
    if let Some(a) = &file.alpha {
        check("alpha.start", &a.start)?;
        for (c, steps) in &a.chars {
            check(&format!("alpha {c:?}"), steps)?;
        }
    }
    if let Some(x) = &file.xeq {
        check("xeq.start", &x.start)?;
        check("xeq.end", &x.end)?;
    }
    check("home", &file.home)?;
    for c in &file.commands {
        if !seen.insert(c.name.as_str()) {
            bail!("{} is listed twice", c.name);
        }
        check(&c.name, &file.sequence(c)?)?;
    }
    Ok(())
}

/// Emulated hold of a key press, as the key scripts' default.
const HOLD_MS: u64 = saturnus_drive::script::DEFAULT_HOLD_MS;
/// Longest wait for the calculator to settle after a key (a blinking
/// prompt never does).
const IDLE_CAP_MS: u64 = 3_000;

/// A booted calculator that presses steps from its booted state and reads
/// its screen.
#[derive(Debug)]
pub struct Driver {
    session: Session,
    booted: Vec<u8>,
    model: Model,
    menu_keys: Vec<String>,
    marks: Vec<String>,
    /// The fonts learned.
    pub glyphs: Glyphs,
    /// The fonts of HOME learned (38G, 39G, 40G).
    pub aplet: apletext::Fonts,
}

impl Driver {
    /// Boot `model` from `rom`, wait for it to settle and keep that state.
    pub fn boot(model: Model, rom: &Path, file: &KeyFile) -> Result<Self> {
        let image = saturnus_drive::rom::load(model, rom)?;
        let machine = Machine::new(model, &image)
            .with_context(|| format!("cannot build a {} from {}", model.name(), rom.display()))?;
        let mut session = Session::new(machine, 0, false);
        session.set_echo_warnings(false);
        session.wait_idle(20_000, 0)?;
        let mut d = Driver {
            booted: session.machine.save_state(),
            session,
            model,
            menu_keys: file.menu_keys.clone(),
            marks: file.marks.clone(),
            glyphs: Glyphs::default(),
            aplet: apletext::Fonts::default(),
        };
        d.press_all(&file.ready).context("ready")?;
        d.booted = d.session.machine.save_state();
        d.learn(file)?;
        Ok(d)
    }

    /// Back to the booted state.
    pub fn reset(&mut self) -> Result<()> {
        self.session
            .machine
            .load_state(&self.booted)
            .map_err(|e| anyhow::anyhow!("cannot restore the booted state: {e}"))
    }

    /// The screen now.
    pub fn lcd(&self) -> Lcd {
        self.session.machine.lcd()
    }

    /// The screen as text.
    pub fn read(&self) -> Vec<String> {
        let lcd = self.lcd();
        if apletext::fits(&lcd) {
            self.aplet.read(&lcd)
        } else {
            self.glyphs.read(&lcd)
        }
    }

    fn key(&self, name: &str) -> Result<Key> {
        let k = Key::from_name(name).with_context(|| format!("no key named {name:?}"))?;
        if !self.session.machine.has_key(k) {
            bail!("key {name:?} is not on the {}", self.model.name());
        }
        Ok(k)
    }

    /// Press one key and wait until the calculator settles.
    pub fn press_key(&mut self, name: &str) -> Result<()> {
        let k = self.key(name)?;
        let s = &mut self.session;
        s.machine
            .key_down(k)
            .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
        s.run(s.ms_to_cycles(HOLD_MS))?;
        s.machine
            .key_up(k)
            .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
        s.wait_idle(IDLE_CAP_MS, 0)?;
        s.take_warnings();
        Ok(())
    }

    /// Check `step`'s label against the screen or the key's legend
    /// (`shifted`: a shift was pressed just before), then press it.
    pub fn press(&mut self, step: &Step, shifted: bool) -> Result<()> {
        if let Some(label) = &step.label {
            let lcd = self.lcd();
            let menu_box = self.menu_keys.iter().position(|k| *k == step.key);
            let legend_now = legend(self.model, &step.key, shifted)?;
            match menu_box {
                // A menu key under a menu shows its box; shifted, it may
                // still be the key's own shifted function.
                Some(b) if lcdtext::has_menu(&lcd) && !(shifted && legend_now == *label) => {
                    let cols = lcdtext::box_columns(&lcd, b);
                    if label.chars().count() == 1 {
                        let c = label.chars().next().unwrap_or(' ');
                        self.glyphs.learn_letter(c, &cols);
                    }
                    let mut tries = vec![label.clone()];
                    for m in &self.marks {
                        tries.push(format!("{label}{m}"));
                        let mut cut = label.chars();
                        cut.next_back();
                        tries.push(format!("{}{m}", cut.as_str()));
                    }
                    let first = self.glyphs.check_label(&cols, label);
                    let ok = first.is_ok()
                        || tries[1..]
                            .iter()
                            .any(|t| self.glyphs.check_label(&cols, t).is_ok());
                    if !ok {
                        first.map_err(|m| {
                            anyhow::anyhow!(
                                "menu key {} shows {:?}, not {label:?} (menu: {})",
                                step.key,
                                m.shown,
                                self.glyphs.read_menu(&lcd)
                            )
                        })?;
                    }
                }
                _ => {
                    let legend = legend_now;
                    if legend != *label
                        && (shifted || !other_legends(self.model, &step.key)?.contains(label))
                    {
                        bail!(
                            "key {}{} is {legend:?}, not {label:?}",
                            if shifted { "shifted " } else { "" },
                            step.key
                        );
                    }
                }
            }
        }
        self.press_key(&step.key)
    }

    /// Press `steps` in order, checking each.
    pub fn press_all(&mut self, steps: &[Step]) -> Result<()> {
        let mut shifted = false;
        for s in steps {
            self.press(s, shifted)
                .with_context(|| format!("at {}", keys_text(std::slice::from_ref(s))))?;
            shifted = is_shift(&s.key) && !shifted;
        }
        Ok(())
    }

    /// Learn the fonts: each character of `alpha.chars` typed from the
    /// booted state, its menu letter checked against its label and its
    /// cell read from the first cell of the upper line.
    fn learn(&mut self, file: &KeyFile) -> Result<()> {
        for l in &file.learn {
            self.reset()?;
            self.press_all(&l.keys).context("learn")?;
            let lcd = self.lcd();
            if apletext::fits(&lcd) {
                for (k, text) in l.lines.iter().enumerate().filter(|(_, t)| !t.is_empty()) {
                    let seen = if k < apletext::HISTORY {
                        self.aplet.expect_history(&lcd, k, text)
                    } else {
                        self.aplet.expect_edit(&lcd, text)
                    };
                    seen.map_err(|m| {
                        anyhow::anyhow!("learn: line {} shows {:?}, not {text:?}", k + 1, m.shown)
                    })?;
                }
                continue;
            }
            for (line, text) in l.lines.iter().enumerate() {
                for (i, c) in text.chars().enumerate().filter(|(_, c)| *c != ' ') {
                    self.glyphs.learn_cell(lcdtext::cell(&lcd, line, i), c);
                }
            }
        }
        let chars = file.alpha.as_ref().map(|a| (&a.start, &a.chars));
        if let Some((start, chars)) = chars {
            for (c, steps) in chars {
                let Some(ch) = c.chars().next().filter(|_| c.chars().count() == 1) else {
                    bail!("alpha.chars: {c:?} is not one character");
                };
                self.reset()?;
                self.press_all(start)
                    .context("alpha.start")
                    .and_then(|()| self.press_all(steps))
                    .with_context(|| format!("typing {c:?}"))?;
                self.glyphs.learn_cell(lcdtext::cell(&self.lcd(), 0, 0), ch);
            }
        }
        // The menus last: their letters are mostly known by then.
        for l in file.learn.iter().filter(|l| !l.menu.is_empty()) {
            self.reset()?;
            self.press_all(&l.keys).context("learn")?;
            let lcd = self.lcd();
            for (b, label) in l.menu.iter().enumerate() {
                let cols = lcdtext::box_columns(&lcd, b);
                // One letter not learned yet, or every new letter apart
                // from its neighbours.
                self.glyphs
                    .teach_label(&cols, label)
                    .or_else(|_| self.glyphs.check_label(&cols, label))
                    .map_err(|m| {
                        anyhow::anyhow!("learn: box {} shows {:?}, not {label:?}", b + 1, m.shown)
                    })?;
            }
        }
        Ok(())
    }
}

/// What else an unshifted `key` may be labelled with: its alpha letter
/// and the text printed below it.
fn other_legends(model: Model, key: &str) -> Result<Vec<String>> {
    let k = skin_key(model, key)?;
    Ok([k.alpha, k.below]
        .iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

fn skin_key(model: Model, key: &str) -> Result<&'static saturnus_host::skins::SkinKey> {
    let skin = saturnus_host::skins::skin(model)
        .with_context(|| format!("no skin for the {}", model.name()))?;
    skin.keys
        .iter()
        .find(|k| k.name == key)
        .with_context(|| format!("the {} skin has no key {key:?}", model.name()))
}

/// The legend `key` shows on `model`'s skin: its label, or its shifted
/// label.
fn legend(model: Model, key: &str, shifted: bool) -> Result<String> {
    let k = skin_key(model, key)?;
    Ok(if shifted { k.left } else { k.label }.to_string())
}

/// One generated example.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Example {
    /// The keys pressed first (names).
    pub input: Vec<String>,
    /// What each input key showed when pressed (its legend, `⇧` for a
    /// shift, a menu key's box label), when a menu key among them
    /// pressed under a menu reads as its label rather than its legend.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// The keys pressed after the function's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub then: Vec<String>,
    /// The screen after them, upper line first.
    pub before: Vec<String>,
    /// The screen after the function's keys.
    pub after: Vec<String>,
}

/// The generated examples of a model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Examples {
    pub model: String,
    /// The screen of the booted calculator, as the examples start.
    pub booted: Vec<String>,
    pub examples: BTreeMap<String, Vec<Example>>,
}

/// Check every function of `file` (or those in `only`) on the emulator
/// and run their examples.
pub fn generate(
    model: Model,
    rom: &Path,
    file: &KeyFile,
    only: Option<&[String]>,
) -> Result<Examples> {
    if file.model != model.name() {
        bail!(
            "the file is the {}'s, not the {}'s",
            file.model,
            model.name()
        );
    }
    lint(file)?;
    let mut d = Driver::boot(model, rom, file)?;
    d.reset()?;
    let booted = d.lcd();
    // The screens are read at the end, with every font learned.
    let mut shots: Vec<Shot> = Vec::new();
    for cmd in &file.commands {
        if only.is_some_and(|o| !o.contains(&cmd.name)) {
            continue;
        }
        let mut steps = file.home.clone();
        steps.extend(file.sequence(cmd)?);
        d.reset()?;
        d.press_all(&steps).with_context(|| cmd.name.clone())?;
        check_after(&mut d, file, cmd)?;
        for ex in &cmd.examples {
            d.reset()?;
            let mut labels = Vec::new();
            let mut by_box = false;
            let mut shifted = false;
            let mut alpha = false;
            for k in &ex.input {
                let lcd = d.lcd();
                let menu_box = file.menu_keys.iter().position(|m| m == k);
                let letter = skin_key(model, k)
                    .ok()
                    .map(|x| x.alpha)
                    .filter(|a| alpha && !a.is_empty());
                labels.push(match menu_box {
                    _ if is_shift(k) => "⇧".to_string(),
                    // After ALPHA (38G, 39G, 40G) a key types its letter.
                    _ if letter.is_some() => letter.unwrap_or_default().to_string(),
                    Some(b) if lcdtext::has_menu(&lcd) && !shifted => {
                        by_box = true;
                        d.glyphs.read_box(&lcdtext::box_columns(&lcd, b))
                    }
                    _ => legend(model, k, shifted).unwrap_or_else(|_| k.to_uppercase()),
                });
                shifted = is_shift(k) && !shifted;
                alpha = k == "alpha";
                d.press_key(k)
                    .with_context(|| format!("{}: example input", cmd.name))?;
            }
            let before = d.lcd();
            d.press_all(&steps).with_context(|| cmd.name.clone())?;
            for (i, k) in ex.then.iter().enumerate() {
                let last_enter = i + 1 == ex.then.len() && k == "enter";
                let typed = d.aplet.read_edit(&d.lcd());
                d.press_key(k)
                    .with_context(|| format!("{}: example keys after", cmd.name))?;
                if last_enter && apletext::fits(&d.lcd()) {
                    learn_input(&mut d, &typed).with_context(|| cmd.name.clone())?;
                }
            }
            // The Pioneers' screens are read now, HOME's at the end with
            // every font learned.
            let after = d.lcd();
            let now =
                (!apletext::fits(&after)).then(|| (d.glyphs.read(&before), d.glyphs.read(&after)));
            shots.push(Shot {
                name: cmd.name.clone(),
                spec: ex.clone(),
                labels: if by_box { labels } else { Vec::new() },
                before,
                after,
                now,
            });
        }
    }
    let read = |lcd: &Lcd| {
        if apletext::fits(lcd) {
            d.aplet.read(lcd)
        } else {
            d.glyphs.read(lcd)
        }
    };
    let mut out = Examples {
        model: file.model.clone(),
        booted: read(&booted),
        examples: BTreeMap::new(),
    };
    for shot in shots {
        let name = shot.name;
        let (before, after) = shot
            .now
            .unwrap_or_else(|| (read(&shot.before), read(&shot.after)));
        let x = Example {
            input: shot.spec.input,
            labels: shot.labels,
            then: shot.spec.then,
            before,
            after,
        };
        if apletext::fits(&shot.after)
            && x.before
                .iter()
                .chain(&x.after)
                .any(|l| l.contains(apletext::UNKNOWN))
        {
            bail!(
                "{name}: the example shows characters never learned (add a learn screen): {:?}",
                x.after
            );
        }
        out.examples.entry(name).or_default().push(x);
    }
    Ok(out)
}

/// An example run, its screens read later.
struct Shot {
    name: String,
    spec: ExampleSpec,
    labels: Vec<String>,
    before: Lcd,
    after: Lcd,
    /// The screens as text, read at once (the Pioneers, the 19BII).
    now: Option<(Vec<String>, Vec<String>)>,
}

/// The checks after a function's keys: a menu shows exactly when
/// `leave.menu_box` says so (on the 38G, 39G and 40G: none shows), a
/// name run by XEQ is known, and the edit line holds `inserts`.
fn check_after(d: &mut Driver, file: &KeyFile, cmd: &Command) -> Result<()> {
    let lcd = d.lcd();
    let aplet = apletext::fits(&lcd);
    if let Some(menu_box) = file.leave.as_ref().and_then(|l| l.menu_box.as_ref()) {
        let menu = if aplet {
            false
        } else {
            lcdtext::has_menu(&lcd)
        };
        if menu_box.shown(&lcd) != menu {
            bail!(
                "{}: leave.menu_box does not tell whether a menu shows",
                cmd.name
            );
        }
    }
    if let (Some(_), Some(unknown)) = (&cmd.xeq, file.xeq.as_ref().and_then(|x| x.unknown.as_ref()))
        && d.read()[0].contains(unknown.as_str())
    {
        bail!(
            "{}: the calculator does not know the name {:?}",
            cmd.name,
            cmd.xeq
        );
    }
    if let Some(text) = &cmd.inserts {
        if !aplet {
            bail!("{}: \"inserts\" is for HOME's edit line", cmd.name);
        }
        d.aplet.expect_edit(&lcd, text).map_err(|m| {
            anyhow::anyhow!(
                "{}: the edit line shows {:?}, not {text:?}",
                cmd.name,
                m.shown
            )
        })?;
    }
    Ok(())
}

/// After ENTER on HOME: the last entry's input must read as `typed`, the
/// edit line just before, in the history's font (learning it).
fn learn_input(d: &mut Driver, typed: &str) -> Result<()> {
    if typed.contains(apletext::UNKNOWN) {
        bail!("the edit line shows characters never learned: {typed:?} (add a learn screen)");
    }
    let lcd = d.lcd();
    let Some(k) = apletext::Fonts::last_input_line(&lcd) else {
        bail!("ENTER left no input in the history");
    };
    // The history may write a minus as the sign of a negative number
    // (the 38G shows 4^−1 typed as 4^-1).
    let signed = typed.replace('\u{2212}', "-");
    d.aplet
        .expect_history(&lcd, k, typed)
        .or_else(|_| d.aplet.expect_history(&lcd, k, &signed))
        .map_err(|m| {
            anyhow::anyhow!(
                "the history shows the input {:?}, not {typed:?} as typed",
                m.shown
            )
        })
}

/// The examples as the file holds them: pretty JSON and a final newline.
pub fn to_file_text(examples: &Examples) -> Result<String> {
    Ok(serde_json::to_string_pretty(examples)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(key: &str, label: Option<&str>) -> Step {
        Step {
            key: key.into(),
            label: label.map(str::to_string),
        }
    }

    fn file() -> KeyFile {
        serde_json::from_str(
            r#"{
              "model": "42s", "manual": "hp42s-om",
              "menu_keys": ["sigmaplus", "inv", "sqrt", "log", "ln", "xeq"],
              "alpha": {
                "start": [{"key": "shift"}, {"key": "enter", "label": "ALPHA"}],
                "chars": {
                  "C": [{"key": "sigmaplus", "label": "ABCDE"}, {"key": "sqrt", "label": "C"}],
                  "H": [{"key": "inv", "label": "FGHI"}, {"key": "sqrt", "label": "H"}]
                }
              },
              "xeq": {"start": [{"key": "xeq", "label": "XEQ"}], "end": [{"key": "enter", "label": "ENTER"}]},
              "commands": [
                {"name": "→POL", "description": "d", "page": 91,
                 "keys": [{"key": "shift"}, {"key": "5", "label": "CONVERT"}, {"key": "xeq", "label": "→POL"}]},
                {"name": "CH", "description": "d", "page": 1, "xeq": "CH"}
              ]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn keys_read_as_their_labels() {
        let f = file();
        assert_eq!(
            keys_text(&f.sequence(&f.commands[0]).unwrap()),
            "⇧ CONVERT →POL"
        );
        assert_eq!(keys_text(&[step("enter", None)]), "ENTER");
    }

    #[test]
    fn a_name_is_run_by_xeq_and_its_letters() {
        let f = file();
        let s = f.sequence(&f.commands[1]).unwrap();
        assert_eq!(keys_text(&s), "XEQ ⇧ ALPHA ABCDE C FGHI H ENTER");
        let mut g = f.commands[1].clone();
        g.xeq = Some("CX".into());
        let e = f.sequence(&g).unwrap_err();
        assert!(e.to_string().contains("no keys type 'X'"), "{e}");
    }

    #[test]
    fn the_shape_is_checked() {
        let mut f = file();
        lint(&f).unwrap();
        f.commands[0].keys = Some(vec![step("5", None)]);
        assert!(lint(&f).unwrap_err().to_string().contains("needs a label"));
        let mut f = file();
        f.commands[1].keys = Some(vec![]);
        assert!(lint(&f).unwrap_err().to_string().contains("either"));
        // Neither: a function typed into an equation, pressed by nobody.
        let mut f = file();
        f.commands[1].xeq = None;
        lint(&f).unwrap();
        assert!(f.sequence(&f.commands[1]).unwrap().is_empty());
        let mut f = file();
        f.commands.push(f.commands[0].clone());
        assert!(lint(&f).unwrap_err().to_string().contains("twice"));
    }

    #[test]
    fn a_file_of_another_model_is_refused_before_booting() {
        let e = generate(Model::Hp27s, Path::new("/no/such/rom"), &file(), None).unwrap_err();
        assert!(e.to_string().contains("not the 27s's"), "{e}");
    }
}
