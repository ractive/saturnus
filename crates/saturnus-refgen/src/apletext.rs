//! The text of HOME on the 38G, 39G and 40G (131 x 64 dots), read back
//! from the pixels for the key-driven reference ([`crate::keyed`]).
//!
//! HOME shows a title on rows 0-6, then up to four lines of history in a
//! small font (8 rows from row 8, one line every 9 rows: an entry's input
//! on the left, its result on the right), a rule on row 44, the edit line
//! in a larger font (10 rows from row 46) and the menu labels on rows
//! 57-63. Both fonts sit in cells 6 dots wide, 22 to a line, on the same
//! grid whether the text is set from the left or from the right.
//!
//! No font is stored: [`Fonts`] learns each font at run time from text
//! the reference expects on the screen (the names a function's keys put
//! on the edit line, the edit line copied into the history by ENTER, the
//! `learn` screens), and checks it both ways: a cell reads as one
//! character, a character shows as one cell. A cell it has not seen reads
//! as `�`. One cell may show two characters: `⁻¹` (the reciprocal, as
//! [`TOKENS`] lists).

use std::collections::HashMap;

use saturnus_core::Lcd;

/// Character cells per line.
pub const CELLS: usize = 22;
/// Pitch of the cells, in dots (the last cell has 5 columns).
const PITCH: usize = 6;
/// History lines.
pub const HISTORY: usize = 4;
/// Top row and rows of the history lines.
const HISTORY_TOP: usize = 8;
const HISTORY_PITCH: usize = 9;
const SMALL_ROWS: usize = 8;
/// Top row and rows of the edit line.
const EDIT_TOP: usize = 46;
const LARGE_ROWS: usize = 10;

/// What a cell reads as when it was never learned.
pub const UNKNOWN: char = '\u{fffd}';

/// Texts that one cell shows.
pub const TOKENS: [&str; 1] = ["⁻¹"];

/// `text` cut into what each cell shows.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let t = TOKENS
            .iter()
            .find(|t| rest.starts_with(**t))
            .map_or_else(|| c.to_string(), |t| t.to_string());
        rest = &rest[t.len()..];
        out.push(t);
    }
    out
}

/// One cell: a mask per dot column, bit 0 the top row.
pub type Cell = [u16; PITCH];

const BLANK: Cell = [0; PITCH];

/// Whether `lcd` is the display this module reads.
pub fn fits(lcd: &Lcd) -> bool {
    lcd.width() == 131 && lcd.height() == 64
}

fn dark(lcd: &Lcd, x: usize, y: usize) -> bool {
    lcd.pixels
        .get(y)
        .and_then(|r| r.get(x))
        .copied()
        .unwrap_or(false)
}

fn cells(lcd: &Lcd, top: usize, rows: usize) -> Vec<Cell> {
    (0..CELLS)
        .map(|i| {
            let mut out = BLANK;
            for (dx, col) in out.iter_mut().enumerate() {
                for dy in 0..rows {
                    if dark(lcd, i * PITCH + dx, top + dy) {
                        *col |= 1 << dy;
                    }
                }
            }
            out
        })
        .collect()
}

/// The cells of history line `k` (0 the top one).
pub fn history_cells(lcd: &Lcd, k: usize) -> Vec<Cell> {
    cells(lcd, HISTORY_TOP + k * HISTORY_PITCH, SMALL_ROWS)
}

/// The cells of the edit line.
pub fn edit_cells(lcd: &Lcd) -> Vec<Cell> {
    cells(lcd, EDIT_TOP, LARGE_ROWS)
}

/// One font as learned so far, both ways.
#[derive(Clone, Debug, Default)]
struct Font {
    chars: HashMap<Cell, String>,
    cells: HashMap<String, Cell>,
}

impl Font {
    fn read(&self, cell: &Cell) -> String {
        if *cell == BLANK {
            " ".to_string()
        } else {
            self.chars
                .get(cell)
                .cloned()
                .unwrap_or_else(|| UNKNOWN.to_string())
        }
    }

    fn line(&self, cells: &[Cell]) -> String {
        cells
            .iter()
            .map(|c| self.read(c))
            .collect::<String>()
            .trim_end()
            .to_string()
    }
}

/// A line that is not the text expected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// What the line reads as, with what was learned.
    pub shown: String,
}

/// The two fonts of HOME, and the edit line's cursor.
#[derive(Clone, Debug, Default)]
pub struct Fonts {
    small: Font,
    large: Font,
    cursor: Option<Cell>,
}

/// Which font a line is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    /// The history.
    Small,
    /// The edit line (and the cursor after its text).
    Large,
}

impl Fonts {
    fn font(&self, which: Which) -> &Font {
        match which {
            Which::Small => &self.small,
            Which::Large => &self.large,
        }
    }

    /// How many characters of each font are known: (small, large).
    pub fn known(&self) -> (usize, usize) {
        (self.small.chars.len(), self.large.chars.len())
    }

    /// The edit line as text, the cursor left out.
    pub fn read_edit(&self, lcd: &Lcd) -> String {
        let cells: Vec<Cell> = edit_cells(lcd)
            .into_iter()
            .map(|c| if Some(c) == self.cursor { BLANK } else { c })
            .collect();
        self.large.line(&cells)
    }

    /// History line `k` as text, its leading spaces kept (a result is set
    /// from the right).
    pub fn read_history(&self, lcd: &Lcd, k: usize) -> String {
        self.small.line(&history_cells(lcd, k))
    }

    /// HOME as text: the four history lines, then the edit line.
    pub fn read(&self, lcd: &Lcd) -> Vec<String> {
        let mut out: Vec<String> = (0..HISTORY).map(|k| self.read_history(lcd, k)).collect();
        out.push(self.read_edit(lcd));
        out
    }

    /// Check that `cells` show `text` (spaces as blank cells) and nothing
    /// after it, learning the characters not seen before. On the edit
    /// line the cell right after the text may hold the cursor. Nothing is
    /// learned when the line does not match.
    pub fn expect(&mut self, which: Which, cells: &[Cell], text: &str) -> Result<(), Mismatch> {
        let chars = tokens(text);
        let mismatch = |f: &Fonts| Mismatch {
            shown: f.font(which).line(cells),
        };
        if chars.len() > cells.len() {
            return Err(mismatch(self));
        }
        let mut new: Vec<(Cell, String)> = Vec::new();
        for (cell, c) in cells.iter().zip(&chars) {
            let font = self.font(which);
            if c == " " {
                if *cell != BLANK {
                    return Err(mismatch(self));
                }
                continue;
            }
            let by_cell = font
                .chars
                .get(cell)
                .cloned()
                .or_else(|| new.iter().find(|(g, _)| g == cell).map(|(_, x)| x.clone()));
            let by_char = font
                .cells
                .get(c)
                .copied()
                .or_else(|| new.iter().find(|(_, x)| x == c).map(|(g, _)| *g));
            match (by_cell, by_char) {
                (Some(x), _) if x != *c => return Err(mismatch(self)),
                (_, Some(g)) if g != *cell => return Err(mismatch(self)),
                (None, None) if *cell == BLANK || Some(*cell) == self.cursor => {
                    return Err(mismatch(self));
                }
                (None, None) => new.push((*cell, c.clone())),
                _ => {}
            }
        }
        let rest = &cells[chars.len()..];
        let mut cursor = None;
        for cell in rest.iter().filter(|c| **c != BLANK) {
            // The first thing after the text (and its spaces) may be the
            // cursor; nothing else may follow.
            let may_be_cursor = which == Which::Large
                && cursor.is_none()
                && self.cursor.is_none_or(|c| c == *cell)
                && !self.large.chars.contains_key(cell);
            if !may_be_cursor {
                return Err(mismatch(self));
            }
            cursor = Some(*cell);
        }
        if cursor.is_some() {
            self.cursor = cursor;
        }
        let font = match which {
            Which::Small => &mut self.small,
            Which::Large => &mut self.large,
        };
        for (g, c) in new {
            font.chars.insert(g, c.clone());
            font.cells.insert(c, g);
        }
        Ok(())
    }

    /// [`Fonts::expect`] on the edit line.
    pub fn expect_edit(&mut self, lcd: &Lcd, text: &str) -> Result<(), Mismatch> {
        self.expect(Which::Large, &edit_cells(lcd), text)
    }

    /// [`Fonts::expect`] on history line `k`.
    pub fn expect_history(&mut self, lcd: &Lcd, k: usize, text: &str) -> Result<(), Mismatch> {
        self.expect(Which::Small, &history_cells(lcd, k), text)
    }

    /// The history line the last entry's input stands on: two above the
    /// last line with anything on it (the input, then its one-line
    /// result), or `None`.
    pub fn last_input_line(lcd: &Lcd) -> Option<usize> {
        let last = (0..HISTORY)
            .rev()
            .find(|&k| history_cells(lcd, k).iter().any(|c| *c != BLANK))?;
        last.checked_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A HOME display with `text` drawn as 3-dot-wide glyphs from a tiny
    /// font of our own (each character a different column pattern) in
    /// history line `k` (or the edit line for `None`), from cell `at`.
    fn screen(lines: &[(Option<usize>, usize, &str)]) -> Lcd {
        let mut l = Lcd::blank_sized(131, 64);
        for (line, at, text) in lines {
            let top = line.map_or(EDIT_TOP, |k| HISTORY_TOP + k * HISTORY_PITCH);
            for (i, c) in text.chars().enumerate() {
                if c == ' ' {
                    continue;
                }
                let code = c as u32;
                for dx in 0..3 {
                    for dy in 0..6 {
                        if dy == 0 || (code >> ((dx * 5 + dy - 1) % 16)) & 1 == 1 {
                            l.pixels[top + dy][(at + i) * PITCH + dx] = true;
                        }
                    }
                }
            }
        }
        l
    }

    #[test]
    fn a_line_is_learned_then_checked_both_ways() {
        let l = screen(&[(None, 0, "COMB(")]);
        let mut f = Fonts::default();
        assert_eq!(f.read_edit(&l), "\u{fffd}".repeat(5));
        f.expect_edit(&l, "COMB(").unwrap();
        assert_eq!(f.read_edit(&l), "COMB(");
        // A known cell must be its character, a known character its cell.
        assert_eq!(f.expect_edit(&l, "CBMO(").unwrap_err().shown, "COMB(");
        assert!(f.expect_edit(&l, "COMBX").is_err());
        // Too short or too long.
        assert!(f.expect_edit(&l, "COMB").is_err());
        assert!(f.expect_edit(&l, "COMB((").is_err());
        assert_eq!(f.known(), (0, 5));
    }

    #[test]
    fn the_cursor_after_the_text_is_learned_and_left_out() {
        let l = screen(&[(None, 0, "SIN(\u{25c0}")]);
        let mut f = Fonts::default();
        f.expect_edit(&l, "SIN(").unwrap();
        assert_eq!(f.read_edit(&l), "SIN(");
        assert_eq!(f.known(), (0, 4));
        // After spaces too, but only once.
        let l = screen(&[(None, 0, "SIN  \u{25c0}")]);
        f.expect_edit(&l, "SIN").unwrap();
        let l = screen(&[(None, 0, "SIN \u{25c0}\u{25c0}")]);
        assert!(f.expect_edit(&l, "SIN").is_err());
    }

    #[test]
    fn one_cell_may_show_two_characters() {
        let l = screen(&[(None, 0, "Ans\u{2070}")]);
        let mut f = Fonts::default();
        f.expect_edit(&l, "Ans⁻¹").unwrap();
        assert_eq!(f.read_edit(&l), "Ans⁻¹");
        assert_eq!(tokens("x⁻¹y"), ["x", "⁻¹", "y"]);
    }

    #[test]
    fn history_lines_keep_their_place() {
        let l = screen(&[(Some(0), 0, "2+3"), (Some(1), 21, "5")]);
        let mut f = Fonts::default();
        f.expect_history(&l, 0, "2+3").unwrap();
        f.expect_history(&l, 1, &format!("{:>22}", "5")).unwrap();
        assert_eq!(
            f.read(&l),
            vec![
                "2+3".to_string(),
                format!("{:>22}", "5"),
                String::new(),
                String::new(),
                String::new()
            ]
        );
        assert_eq!(Fonts::last_input_line(&l), Some(0));
        assert_eq!(Fonts::last_input_line(&Lcd::blank_sized(131, 64)), None);
        // The large font knows nothing of the small one.
        assert_eq!(f.known(), (4, 0));
    }
}
