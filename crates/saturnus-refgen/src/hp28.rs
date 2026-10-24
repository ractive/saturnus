//! The 28S: no serial port, so no Kermit server and no stack to read as
//! objects. Text goes in by key presses, as a person types it, and what
//! comes back is read off the display: four lines of 23 character cells,
//! six pixel columns apart (wiki: hardware/hp28s "Display").
//!
//! - [`Font`]: the ROM's character glyphs, learnt on the emulator by
//!   showing every character code with `CHR` and `DISP`, so no glyph
//!   table is copied or committed.
//! - [`keys`]: the key script that types a text into the command line.
//! - [`Screen`]: the display read as text, split into messages and stack
//!   levels.
//! - [`catalog_names`]: the command names as the ROM's CATALOG steps
//!   through them.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use saturnus_core::Lcd;
use saturnus_kermit::Emulator;
use saturnus_objects::charset::HP28_HIGH as HIGH;

use crate::examples::Example;
use crate::reference::ExampleSpec;

/// Text lines on the display.
pub const LINES: usize = 4;
/// Character cells per line.
pub const COLUMNS: usize = 23;
/// Pixel columns from one cell to the next (the last cell is one short).
const CELL: usize = 6;
/// Pixel rows per text line.
const ROWS: usize = 8;

/// The character the ROM shows for code `c`; codes without a glyph of
/// their own (the control codes, 148-255) share a block.
pub fn char_of(c: u8) -> char {
    match c {
        10 | 32 => ' ',
        33..=126 => c as char,
        127 => '▒',
        128..=147 => HIGH[usize::from(c - 128)],
        _ => '■',
    }
}

/// What a cell no glyph matches reads as.
pub const UNKNOWN: char = '\u{FFFD}';

/// One cell's pixels: eight rows of five bits.
type Glyph = [u8; ROWS];

/// The display routine's ellipsis at the end of a cut-off object: three
/// dots in the two lowest rows of the cell (28S ROM).
const ELLIPSIS: Glyph = [0, 0, 0, 0, 0, 0b10101, 0b10101, 0];

/// The ROM's character glyphs, learnt on the emulator.
#[derive(Clone, Debug)]
pub struct Font {
    glyphs: HashMap<Glyph, char>,
}

/// The glyph in the cell whose left pixel column is `x`, in text line
/// `line`; `invert` reads light pixels as set (inverse video).
fn glyph_at(lcd: &Lcd, line: usize, x: usize, invert: bool) -> Glyph {
    let mut g = [0u8; ROWS];
    for (y, row) in g.iter_mut().enumerate() {
        let Some(pixels) = lcd.pixels.get(line * ROWS + y) else {
            continue;
        };
        for dx in 0..5 {
            // Columns past the right edge read as background.
            let dark = pixels.get(x + dx).map(|&p| p != invert).unwrap_or(false);
            if dark {
                *row |= 0b10000 >> dx;
            }
        }
    }
    g
}

impl Font {
    /// Learn the glyphs on `emu`: for each group of codes, a program
    /// builds the string of their characters and shows it in the first
    /// line with `DISP`. `state` is a state at the empty stack screen to
    /// start each group from; the emulator is left in some later state.
    pub fn learn(emu: &mut Emulator, state: &[u8]) -> Result<Font> {
        let mut glyphs = HashMap::new();
        // Code 10 starts a new line in DISP, so it is shown on its own.
        let groups: Vec<(u8, u8)> = std::iter::once((0, 9))
            .chain(
                (11u16..=255)
                    .step_by(COLUMNS)
                    .map(|a| (a as u8, (a + COLUMNS as u16 - 1).min(255) as u8)),
            )
            .collect();
        let mut by_code: Vec<(u8, Glyph)> = Vec::new();
        for (first, last) in groups {
            emu.load_state(state)?;
            let program = format!("« \"\" {first} {last} FOR N N CHR + NEXT 1 DISP »");
            type_text(emu, &program)?;
            emu.press_keys("enter eval\nwait-idle 30000")?;
            let lcd = lcd(emu);
            for (i, code) in (first..=last).enumerate() {
                by_code.push((code, glyph_at(&lcd, 0, i * CELL, false)));
            }
        }
        // The shared glyphs (blank, block) go to the plainest code: space
        // for blank, inserted last.
        by_code.sort_by_key(|(c, _)| std::cmp::Reverse(*c));
        for (code, g) in &by_code {
            glyphs.insert(*g, char_of(*code));
        }
        glyphs.insert([0; ROWS], ' ');
        glyphs.insert(ELLIPSIS, '…');
        let font = Font { glyphs };
        // A check that the learning saw characters at all.
        let a = by_code.iter().find(|(c, _)| *c == b'A').map(|(_, g)| *g);
        if a.is_none_or(|g| g == [0; ROWS] || font.glyphs.get(&g) != Some(&'A')) {
            bail!("the 28S's font could not be read off the display");
        }
        Ok(font)
    }

    /// Text line `line` (0 at the top) read cell by cell; a cell no glyph
    /// matches reads as [`UNKNOWN`]. Trailing blanks are trimmed.
    pub fn line(&self, lcd: &Lcd, line: usize) -> String {
        let s: String = (0..COLUMNS)
            .map(|c| {
                let g = glyph_at(lcd, line, c * CELL, false);
                self.glyphs.get(&g).copied().unwrap_or(UNKNOWN)
            })
            .collect();
        s.trim_end().to_string()
    }

    /// Text line `line` in inverse video (light on dark), read at
    /// whichever pixel offset the most cells match: the CATALOG shows
    /// its name so in the top line. Blanks around it are trimmed.
    pub fn inverse_line(&self, lcd: &Lcd, line: usize) -> String {
        let mut best: Option<(usize, String)> = None;
        for phase in 0..CELL {
            let mut misses = 0;
            let s: String = (0..)
                .map(|c| phase + c * CELL)
                .take_while(|&x| x < lcd.width())
                .map(|x| {
                    let g = glyph_at(lcd, line, x, true);
                    self.glyphs.get(&g).copied().unwrap_or_else(|| {
                        misses += 1;
                        UNKNOWN
                    })
                })
                .collect();
            if best.as_ref().is_none_or(|(m, _)| misses < *m) {
                best = Some((misses, s));
            }
        }
        best.map(|(_, s)| s.trim().to_string()).unwrap_or_default()
    }

    /// The whole display as text lines.
    pub fn screen(&self, lcd: &Lcd) -> Screen {
        Screen::parse((0..LINES).map(|l| self.line(lcd, l)).collect())
    }
}

/// The display's pixels now.
pub fn lcd(emu: &mut Emulator) -> Lcd {
    emu.with_machine(|m| m.framebuffer().pixels)
}

/// The display read as text: the lines above the stack (a message, an
/// error) and the stack levels it shows, level 1 first, each as its text
/// with the level number and the padding taken off (an object over
/// several lines joined with two spaces, as the 48's examples are).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Screen {
    /// The raw lines, top first.
    pub lines: Vec<String>,
    /// Lines above the first stack level.
    pub messages: Vec<String>,
    /// The stack levels shown, level 1 first.
    pub levels: Vec<String>,
}

/// The level number of a stack line (`1:` at its start) and the rest.
fn level_line(line: &str) -> Option<(usize, &str)> {
    let (n, rest) = line.split_once(':')?;
    if n.is_empty() || n.len() > 2 || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((n.parse().ok()?, rest))
}

impl Screen {
    /// Split `lines` into messages and stack levels.
    pub fn parse(lines: Vec<String>) -> Screen {
        let mut messages = Vec::new();
        let mut levels: Vec<(usize, Vec<String>)> = Vec::new();
        for line in &lines {
            if let Some((n, rest)) = level_line(line) {
                levels.push((n, vec![rest.trim().to_string()]));
            } else if let Some((_, parts)) = levels.last_mut() {
                // An object over several lines continues its level.
                parts.push(line.trim().to_string());
            } else if !line.trim().is_empty() {
                messages.push(line.trim().to_string());
            }
        }
        let mut shown: Vec<String> = levels
            .into_iter()
            .filter(|(_, parts)| parts.iter().any(|p| !p.is_empty()))
            .map(|(_, parts)| {
                parts
                    .into_iter()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join("  ")
            })
            .collect();
        shown.reverse();
        Screen {
            lines,
            messages,
            levels: shown,
        }
    }

    /// The error the screen reports: the message under an `Error:` line
    /// (`/ Error:` then `Infinite Result`), or a lone `Syntax Error`.
    pub fn error(&self) -> Option<String> {
        if let Some(i) = self.messages.iter().position(|m| m.ends_with("Error:")) {
            return Some(
                self.messages
                    .get(i + 1)
                    .cloned()
                    .unwrap_or_else(|| self.messages[i].clone()),
            );
        }
        self.messages
            .iter()
            .find(|m| m.as_str() == "Syntax Error")
            .cloned()
    }
}

/// How the 28S types one character in alpha entry mode: its keys, and
/// whether the ROM puts spaces around it (the operators and the
/// function keys insert ` + `, but no space after a space or at the
/// start of the line).
fn method(c: char) -> Option<(Vec<&'static str>, bool)> {
    const LETTERS: [&str; 26] = [
        "lettera", "letterb", "letterc", "letterd", "lettere", "letterf", "letterg", "letterh",
        "letteri", "letterj", "letterk", "letterl", "letterm", "lettern", "lettero", "letterp",
        "letterq", "letterr", "letters", "lettert", "letteru", "letterv", "letterw", "letterx",
        "lettery", "letterz",
    ];
    const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    let one = |k: &'static str| Some((vec![k], false));
    let shifted = |k: &'static str, pad: bool| Some((vec!["shift", k], pad));
    match c {
        'A'..='Z' => one(LETTERS[(c as u8 - b'A') as usize]),
        // The lower case: LC, the letter, LC again.
        'a'..='z' => Some((vec!["lc", LETTERS[(c as u8 - b'a') as usize], "lc"], false)),
        '0'..='9' => one(DIGITS[(c as u8 - b'0') as usize]),
        ' ' => one("space"),
        '.' => one("point"),
        ',' => one("comma"),
        '#' => one("hash"),
        '{' => one("lbrace"),
        '[' => one("lbracket"),
        '(' => one("lparen"),
        '«' => one("program"),
        '=' => one("equals"),
        '\'' => one("quote"),
        '"' => shifted("hash", false),
        '}' => shifted("lbrace", false),
        ']' => shifted("lbracket", false),
        ')' => shifted("lparen", false),
        '»' => shifted("program", false),
        '?' => shifted("lc", false),
        '°' => shifted("letterw", false),
        'µ' => shifted("letterx", false),
        'Σ' => shifted("letterv", false),
        '→' => shifted("letteru", true),
        '≤' => shifted("letters", true),
        '≥' => shifted("lettert", true),
        '<' => shifted("lettery", true),
        '>' => shifted("letterz", true),
        '≠' => shifted("equals", true),
        '^' => shifted("multiply", true),
        '%' => shifted("2", true),
        '√' => shifted("minus", true),
        '∫' => shifted("5", true),
        '∂' => shifted("6", true),
        'π' => shifted("point", true),
        '+' => Some((vec!["plus"], true)),
        '-' => Some((vec!["minus"], true)),
        '*' => Some((vec!["multiply"], true)),
        '/' => Some((vec!["divide"], true)),
        _ => None,
    }
}

/// The key script (`saturnus_drive::script::parse_keys` lines) that
/// types `text` into a new command line in alpha entry mode, without
/// ENTER. Where the ROM pads a character with spaces the text does not
/// have, they are taken out again: backspace for the one after it, and
/// the cursor menu's ◀ (menu key E), backspace, ▶ (F) for the one
/// before. So the display must show no menu labels (the cursor menu is
/// on), as at the stack screen.
pub fn keys(text: &str) -> Result<Vec<String>> {
    let mut out: Vec<String> = vec!["alpha".into()];
    let mut prev: Option<char> = None;
    for c in text.chars() {
        let (keys, pad) =
            method(c).with_context(|| format!("the 28S has no key that types {c:?}"))?;
        out.push(keys.join(" "));
        if pad {
            // The space after it.
            out.push("backspace".into());
            // The space before it, unless at the start or after a space.
            if prev.is_some_and(|p| p != ' ') {
                out.push("e backspace f".into());
            }
        }
        prev = Some(c);
    }
    Ok(out)
}

/// Most keys one `press_keys` call gets (each press may wait for the
/// calculator to settle; the script's time budget is bounded).
const CHUNK: usize = 40;

/// Type `text` into a new command line (no ENTER).
pub fn type_text(emu: &mut Emulator, text: &str) -> Result<()> {
    let lines = keys(text)?;
    let mut batch: Vec<String> = Vec::new();
    let mut count = 0;
    for l in lines {
        count += l.split_whitespace().count();
        batch.push(l);
        if count >= CHUNK {
            emu.press_keys(&batch.join("\n"))?;
            batch.clear();
            count = 0;
        }
    }
    if !batch.is_empty() {
        emu.press_keys(&batch.join("\n"))?;
    }
    Ok(())
}

/// Type `text` and press ENTER, waiting up to 30 s for the calculator to
/// settle.
pub fn run_text(emu: &mut Emulator, text: &str) -> Result<()> {
    type_text(emu, text)?;
    emu.press_keys("enter\nwait-idle 30000")?;
    Ok(())
}

/// The command names in the ROM's CATALOG order: shift Q starts it, the
/// first menu key (NEXT) steps on, and the top line shows each name in
/// inverse video; the last name repeats at the end.
pub fn catalog_names(emu: &mut Emulator, font: &Font) -> Result<Vec<String>> {
    emu.press_keys("shift letterq")?;
    let mut names: Vec<String> = Vec::new();
    for _ in 0..1000 {
        let name = font.inverse_line(&lcd(emu), 0);
        if name.is_empty() || name.contains(UNKNOWN) {
            bail!("CATALOG shows {name:?} after {:?}", names.last());
        }
        if names.last() == Some(&name) {
            emu.press_keys("on")?;
            return Ok(names);
        }
        names.push(name);
        emu.press_keys("a")?;
    }
    bail!("CATALOG did not end after 1000 names")
}

/// The type of an object from its display text, as the effects of the
/// 48's examples name types (`real`, `list`, ...): the 28S's stack
/// cannot be read as objects.
pub fn type_of(text: &str) -> &'static str {
    let t = text.trim();
    let inner = t.strip_prefix('\'').and_then(|x| x.strip_suffix('\''));
    match t.chars().next() {
        Some('"') => "string",
        Some('{') => "list",
        Some('[') => "array",
        Some('(') => "complex",
        Some('«') => "program",
        Some('#') => "binary",
        Some('\'') => {
            // A quoted name is a name, anything else an algebraic.
            if inner.is_some_and(|n| {
                !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || "Σ?".contains(c))
            }) {
                "name"
            } else {
                "algebraic"
            }
        }
        Some(c) if c.is_ascii_digit() || c == '-' || c == '.' => "real",
        _ => "object",
    }
}

/// `real real → real` from display texts given level 1 first, as
/// [`crate::examples::effect`] does from objects.
pub fn effect(before: &[String], after: &[String], error: Option<&str>) -> String {
    let side = |v: &[String]| -> String {
        v.iter()
            .rev()
            .map(|t| type_of(t))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let out = match error {
        Some(e) => format!("error: {e}"),
        None => side(after),
    };
    format!("{} → {}", side(before), out).trim().to_string()
}

/// Runs examples on the 28S, each from the same state: booted, the
/// "Memory Lost" message cleared, the stack empty.
#[derive(Debug)]
pub struct Runner {
    emu: Emulator,
    state: Vec<u8>,
    font: Font,
}

impl Runner {
    /// Boot the 28S from `rom`, learn its font and keep the state.
    pub fn new(rom: &std::path::Path) -> Result<Self> {
        let (emu, state, font) = boot(rom)?;
        Ok(Runner { emu, state, font })
    }

    /// The display now, read as text.
    fn screen(&mut self) -> Screen {
        let lcd = lcd(&mut self.emu);
        self.font.screen(&lcd)
    }

    /// Run `spec` for command `name`: the setup typed and ENTERed (its
    /// stack cleared with CLEAR), the input typed and ENTERed (the stack
    /// it shows is `before`), then the command (the stack it shows is
    /// `display`, or the error the display reports).
    pub fn run(&mut self, name: &str, spec: &ExampleSpec) -> Result<Example> {
        self.emu.load_state(&self.state)?;
        let run = spec.run.clone().unwrap_or_else(|| name.to_string());
        let mut example = Example {
            setup: spec.setup.clone(),
            input: spec.input.clone(),
            run: run.clone(),
            stack: Vec::new(),
            before: Vec::new(),
            result: Vec::new(),
            display: Vec::new(),
            error: None,
            effect: String::new(),
        };
        let fail = |mut example: Example, error: String| {
            example.effect = effect(&example.before, &[], Some(&error));
            example.error = Some(error);
            example
        };
        for text in [spec.setup.as_deref().unwrap_or(""), &spec.input, &run] {
            if let Err(e) = keys(text) {
                return Ok(fail(example, format!("cannot type: {e}")));
            }
        }
        if let Some(setup) = &spec.setup {
            run_text(&mut self.emu, setup)?;
            if let Some(e) = self.screen().error() {
                return Ok(fail(example, format!("in setup: {e}")));
            }
            run_text(&mut self.emu, "CLEAR")?;
        }
        if !spec.input.trim().is_empty() {
            run_text(&mut self.emu, &spec.input)?;
            let s = self.screen();
            if let Some(e) = s.error() {
                return Ok(fail(example, format!("in input: {e}")));
            }
            example.before = s.levels;
        }
        run_text(&mut self.emu, &run)?;
        let s = self.screen();
        example.error = s.error();
        example.display = s.levels;
        example.effect = effect(&example.before, &example.display, example.error.as_deref());
        Ok(example)
    }
}

/// Boot the 28S from `rom`, clear "Memory Lost" with ATTN, and learn the
/// font: the emulator, the state to start from, the font.
pub fn boot(rom: &std::path::Path) -> Result<(Emulator, Vec<u8>, Font)> {
    let mut emu = crate::catalog::boot(saturnus_core::Model::Hp28s, rom)?;
    emu.press_keys("on")?;
    let state = emu.save_state();
    let font = Font::learn(&mut emu, &state)?;
    emu.load_state(&state)?;
    Ok((emu, state, font))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padded_characters_lose_the_spaces_the_text_lacks() {
        assert_eq!(
            keys("1 2 +").unwrap(),
            ["alpha", "1", "space", "2", "space", "plus", "backspace"]
        );
        assert_eq!(
            keys("STO+").unwrap(),
            [
                "alpha",
                "letters",
                "lettert",
                "lettero",
                "plus",
                "backspace",
                "e backspace f"
            ]
        );
        assert_eq!(keys("→LIST").unwrap()[1..3], ["shift letteru", "backspace"]);
        assert_eq!(keys("e").unwrap(), ["alpha", "lc lettere lc"]);
        assert!(keys("1_m").is_err());
    }

    #[test]
    fn screens_split_into_messages_and_levels() {
        let s = Screen::parse(
            [
                "/ Error:",
                "Infinite Result",
                "2:                    1",
                "1:                    0",
            ]
            .map(String::from)
            .to_vec(),
        );
        assert_eq!(s.error().as_deref(), Some("Infinite Result"));
        assert_eq!(s.levels, ["0", "1"]);
        let m = Screen::parse(
            [
                "3:",
                "2:                    5",
                "1: [[ 1 2 ]",
                "    [ 3 4 ]]",
            ]
            .map(String::from)
            .to_vec(),
        );
        assert_eq!(m.levels, ["[[ 1 2 ]  [ 3 4 ]]", "5"]);
        assert!(m.messages.is_empty() && m.error().is_none());
    }

    #[test]
    fn types_come_from_the_display_text() {
        let t = |x: &str| type_of(x);
        assert_eq!(
            [
                t("5"),
                t("-2.5E-3"),
                t("(1,2)"),
                t("\"A\""),
                t("{ 1 }"),
                t("[[ 1 ]]")
            ],
            ["real", "real", "complex", "string", "list", "array"]
        );
        assert_eq!(
            [t("'X'"), t("'X+1'"), t("« 1 »"), t("# 5d")],
            ["name", "algebraic", "program", "binary"]
        );
        assert_eq!(
            effect(&["2".into(), "'X'".into()], &["'X^2'".into()], None),
            "name real → algebraic"
        );
    }

    #[test]
    fn codes_map_to_their_characters() {
        assert_eq!(char_of(b'A'), 'A');
        assert_eq!(char_of(141), '→');
        assert_eq!(char_of(133), 'Σ');
        assert_eq!(char_of(200), '■');
    }
}
