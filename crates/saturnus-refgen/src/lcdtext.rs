//! The text on a Pioneer display (131 x 16 dots: the 42S, 27S, 17BII and
//! 17B), read back from the pixels for the key-driven reference
//! ([`crate::keyed`]).
//!
//! The display has two lines of 22 character cells, each cell 5 dots wide
//! at a pitch of 6 and 8 rows tall (the eighth row carries descenders and
//! the cursor). When a menu is shown, the lower line is six boxes of 21
//! dots at a pitch of 22, dark with light letters in a smaller font of 5
//! rows (rows 10-14, inside a dark border on rows 9 and 15).
//!
//! No font is stored: [`Glyphs`] learns both fonts at run time from text
//! the calculator is made to show (the characters typed through its ALPHA
//! menu, the labels the reference expects on its menus), and reads back
//! what it learned. A cell or a letter it has not seen reads as `�`.

use std::collections::HashMap;

use saturnus_core::Lcd;

/// Character cells per line.
pub const CELLS: usize = 22;
/// Pitch of the character cells, in dots.
const CELL_PITCH: usize = 6;
/// Width of a character cell, in dots.
const CELL_WIDTH: usize = 5;
/// Rows of a text line.
const LINE_ROWS: usize = 8;
/// Menu boxes on the lower line.
pub const BOXES: usize = 6;
/// Pitch and width of a menu box.
const BOX_PITCH: usize = 22;
const BOX_WIDTH: usize = 21;
/// The menu boxes' border rows and the rows of their letters.
const BOX_TOP: usize = 9;
const BOX_BOTTOM: usize = 15;
const LETTER_ROWS: std::ops::Range<usize> = 10..15;

/// What a cell or a letter reads as when it was never learned.
pub const UNKNOWN: char = '\u{fffd}';

/// One character cell: a column mask per dot column, bit 0 the top row.
pub type Cell = [u8; CELL_WIDTH];

/// The letters of a menu box: one mask per dot column (bit 0 the top
/// letter row), light dots set, the blank columns at either end trimmed.
pub type BoxColumns = Vec<u8>;

/// Whether `lcd` is a Pioneer display this module reads.
pub fn fits(lcd: &Lcd) -> bool {
    lcd.height() == 16 && lcd.width() == 131
}

fn dark(lcd: &Lcd, x: usize, y: usize) -> bool {
    lcd.pixels
        .get(y)
        .and_then(|r| r.get(x))
        .copied()
        .unwrap_or(false)
}

/// Cell `i` of text line `line` (0 the upper, 1 the lower).
pub fn cell(lcd: &Lcd, line: usize, i: usize) -> Cell {
    let mut out = [0u8; CELL_WIDTH];
    for (dx, col) in out.iter_mut().enumerate() {
        for dy in 0..LINE_ROWS {
            if dark(lcd, i * CELL_PITCH + dx, line * LINE_ROWS + dy) {
                *col |= 1 << dy;
            }
        }
    }
    out
}

/// Whether the lower line shows menu boxes: some box has its dark border
/// rows.
pub fn has_menu(lcd: &Lcd) -> bool {
    (0..BOXES).any(|b| {
        (b * BOX_PITCH..b * BOX_PITCH + BOX_WIDTH)
            .all(|x| dark(lcd, x, BOX_TOP) && dark(lcd, x, BOX_BOTTOM))
    })
}

/// The letter columns of menu box `b` (0-5), trimmed; empty for a blank
/// box.
pub fn box_columns(lcd: &Lcd, b: usize) -> BoxColumns {
    let cols: Vec<u8> = (b * BOX_PITCH..b * BOX_PITCH + BOX_WIDTH)
        .map(|x| {
            LETTER_ROWS
                .enumerate()
                .filter(|&(_, y)| !dark(lcd, x, y))
                .fold(0u8, |m, (i, _)| m | 1 << i)
        })
        .collect();
    trim(&cols).to_vec()
}

fn trim(cols: &[u8]) -> &[u8] {
    let start = cols.iter().position(|&c| c != 0).unwrap_or(cols.len());
    let end = cols.iter().rposition(|&c| c != 0).map_or(start, |e| e + 1);
    &cols[start..end]
}

/// The two fonts as learned so far.
#[derive(Clone, Debug, Default)]
pub struct Glyphs {
    cells: HashMap<Cell, char>,
    /// Menu letters, in the order learned; a letter's columns may hold
    /// blank columns (`"`), and a letter may have more than one form
    /// (`…` alone in a box is wider than in `A…F`).
    letters: Vec<(char, Vec<u8>)>,
}

/// A label that is not what a box shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// What the box reads as, with what was learned.
    pub shown: String,
}

impl Glyphs {
    /// Learn that `cell` shows `c` (the first sight of a cell wins).
    pub fn learn_cell(&mut self, cell: Cell, c: char) {
        if cell != [0; CELL_WIDTH] {
            self.cells.entry(cell).or_insert(c);
        }
    }

    /// Learn that a box holding the single letter `c` shows `cols`.
    pub fn learn_letter(&mut self, c: char, cols: &[u8]) {
        let cols = trim(cols);
        if !cols.is_empty() && self.letter(c).is_none() {
            self.letters.push((c, cols.to_vec()));
        }
    }

    fn letter(&self, c: char) -> Option<&[u8]> {
        self.forms(c).next()
    }

    /// Every learned form of `c`.
    fn forms(&self, c: char) -> impl Iterator<Item = &[u8]> {
        self.letters
            .iter()
            .filter(move |(x, _)| *x == c)
            .map(|(_, g)| g.as_slice())
    }

    /// Learn a box that shows `label` where all letters but one are known
    /// (in some form) and match from either end: the one left takes the
    /// columns between, as another form of it if it had one. A label whose
    /// letters are all known and match is fine as it is.
    pub fn teach_label(&mut self, cols: &[u8], label: &str) -> Result<(), Mismatch> {
        let cols = trim(cols);
        let chars: Vec<char> = label.chars().filter(|c| *c != ' ').collect();
        let skip = |i: &mut usize| {
            while *i < cols.len() && cols[*i] == 0 {
                *i += 1;
            }
        };
        let (mut i, mut k) = (0, 0);
        while k < chars.len() {
            skip(&mut i);
            let Some(g) = self.forms(chars[k]).find(|g| cols[i..].starts_with(g)) else {
                break;
            };
            i += g.len();
            k += 1;
        }
        skip(&mut i);
        if k == chars.len() {
            return if i >= cols.len() {
                Ok(())
            } else {
                Err(self.mismatch(cols))
            };
        }
        let (mut j, mut m) = (cols.len(), chars.len());
        while m > k + 1 {
            while j > i && cols[j - 1] == 0 {
                j -= 1;
            }
            let Some(g) = self.forms(chars[m - 1]).find(|g| cols[..j].ends_with(g)) else {
                return Err(self.mismatch(cols));
            };
            j -= g.len();
            m -= 1;
        }
        let middle = trim(&cols[i..j.max(i)]);
        if middle.is_empty() {
            return Err(self.mismatch(cols));
        }
        let c = chars[k];
        if !self.forms(c).any(|g| g == middle) {
            self.letters.push((c, middle.to_vec()));
        }
        Ok(())
    }

    /// How many characters of each font are known: (cells, letters).
    pub fn known(&self) -> (usize, usize) {
        (self.cells.len(), self.letters.len())
    }

    /// The character a cell shows: a space for a blank cell.
    pub fn read_cell(&self, cell: &Cell) -> char {
        if *cell == [0; CELL_WIDTH] {
            ' '
        } else {
            self.cells.get(cell).copied().unwrap_or(UNKNOWN)
        }
    }

    /// Text line `line` as characters, trailing spaces dropped.
    pub fn read_line(&self, lcd: &Lcd, line: usize) -> String {
        let s: String = (0..CELLS)
            .map(|i| self.read_cell(&cell(lcd, line, i)))
            .collect();
        s.trim_end().to_string()
    }

    /// A menu box's label: at each position the widest learned letter
    /// that matches (followed by a blank column or the end), else the
    /// columns up to the next blank one as [`UNKNOWN`].
    pub fn read_box(&self, cols: &[u8]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < cols.len() {
            if cols[i] == 0 {
                i += 1;
                continue;
            }
            let best = self
                .letters
                .iter()
                .filter(|(_, g)| {
                    cols[i..].starts_with(g) && cols.get(i + g.len()).is_none_or(|&c| c == 0)
                })
                .max_by_key(|(_, g)| g.len());
            match best {
                Some((c, g)) => {
                    out.push(*c);
                    i += g.len();
                }
                None => {
                    out.push(UNKNOWN);
                    while i < cols.len() && cols[i] != 0 {
                        i += 1;
                    }
                }
            }
        }
        out
    }

    /// Check that `cols` (a box) shows `label`, letter by letter, learning
    /// each letter not seen before from the columns up to the next blank
    /// one. A letter learned earlier must match where it stands.
    pub fn check_label(&mut self, cols: &[u8], label: &str) -> Result<(), Mismatch> {
        let cols = trim(cols);
        let mut i = 0;
        let mut learned = Vec::new();
        for c in label.chars().filter(|c| *c != ' ') {
            while i < cols.len() && cols[i] == 0 {
                i += 1;
            }
            let known: Vec<&[u8]> = self
                .forms(c)
                .chain(
                    learned
                        .iter()
                        .filter(|(x, _)| *x == c)
                        .map(|(_, g): &(char, Vec<u8>)| g.as_slice()),
                )
                .collect();
            if !known.is_empty() {
                let Some(g) = known.iter().find(|g| cols[i..].starts_with(g)) else {
                    return Err(self.mismatch(cols));
                };
                i += g.len();
            } else {
                let start = i;
                while i < cols.len() && cols[i] != 0 {
                    i += 1;
                }
                if start == i {
                    return Err(self.mismatch(cols));
                }
                learned.push((c, cols[start..i].to_vec()));
            }
        }
        if cols[i.min(cols.len())..].iter().any(|&c| c != 0) {
            return Err(self.mismatch(cols));
        }
        for (c, g) in learned {
            self.learn_letter(c, &g);
        }
        Ok(())
    }

    fn mismatch(&self, cols: &[u8]) -> Mismatch {
        Mismatch {
            shown: self.read_box(cols),
        }
    }

    /// The menu line: each box's label in brackets, `[]` for a blank box.
    pub fn read_menu(&self, lcd: &Lcd) -> String {
        (0..BOXES)
            .map(|b| format!("[{}]", self.read_box(&box_columns(lcd, b))))
            .collect()
    }

    /// The display as text: the upper line, then the lower line or the
    /// menu ([`Glyphs::read_menu`]). Lines with nothing on them are empty
    /// strings.
    pub fn read(&self, lcd: &Lcd) -> Vec<String> {
        let lower = if has_menu(lcd) {
            self.read_menu(lcd)
        } else {
            self.read_line(lcd, 1)
        };
        vec![self.read_line(lcd, 0), lower]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 131 x 16 display from rows of `#` and `.`, padded with blank dots.
    fn lcd(rows: &[&str]) -> Lcd {
        let mut l = Lcd::blank_sized(131, 16);
        for (y, r) in rows.iter().enumerate() {
            for (x, c) in r.chars().enumerate() {
                l.pixels[y][x] = c == '#';
            }
        }
        l
    }

    /// A display whose lower line is a menu: every box dark, with `letters`
    /// (rows of `#` = light dots) drawn into box 0 from column 2.
    fn menu(letters: &[&str]) -> Lcd {
        let mut l = Lcd::blank_sized(131, 16);
        for b in 0..BOXES {
            for x in b * BOX_PITCH..b * BOX_PITCH + BOX_WIDTH {
                for y in BOX_TOP..=BOX_BOTTOM {
                    l.pixels[y][x] = true;
                }
            }
        }
        for (dy, r) in letters.iter().enumerate() {
            for (dx, c) in r.chars().enumerate() {
                if c == '#' {
                    l.pixels[LETTER_ROWS.start + dy][2 + dx] = false;
                }
            }
        }
        l
    }

    #[test]
    fn cells_read_back_what_was_learned() {
        let l = lcd(&[
            "#...#.......",
            "#...#.......",
            ".#.#..#.....",
            "..#.........",
            ".#.#..#.....",
            "#...#.......",
            "#...#.......",
        ]);
        let mut g = Glyphs::default();
        assert_eq!(g.read_line(&l, 0), "\u{fffd}\u{fffd}");
        g.learn_cell(cell(&l, 0, 0), 'x');
        g.learn_cell(cell(&l, 0, 1), ':');
        assert_eq!(g.read_line(&l, 0), "x:");
        assert_eq!(g.read(&l), vec!["x:".to_string(), String::new()]);
        assert_eq!(g.known(), (2, 0));
    }

    #[test]
    fn labels_are_learned_checked_and_read() {
        // "AB" in the menu font: A is 3 columns, B 3, one blank between.
        let l = menu(&[".#..##.", "#.#.#.#", "###.##.", "#.#.#.#", "#.#.##."]);
        assert!(has_menu(&l));
        let cols = box_columns(&l, 0);
        assert_eq!(cols.len(), 7);
        let mut g = Glyphs::default();
        assert_eq!(g.read_box(&cols), "\u{fffd}\u{fffd}");
        g.check_label(&cols, "AB").unwrap();
        assert_eq!(g.read_box(&cols), "AB");
        assert_eq!(g.read_menu(&l), "[AB][][][][][]");
        // A known letter must match where the label puts it.
        let e = g.check_label(&cols, "BA").unwrap_err();
        assert_eq!(e.shown, "AB");
        // One letter too many, or too few.
        assert!(g.check_label(&cols, "ABA").is_err());
        assert!(g.check_label(&cols, "A").is_err());
    }

    #[test]
    fn the_one_unknown_letter_of_a_label_is_taught_with_its_gaps() {
        // A, then `…` as three one-column dots, then A again.
        let l = menu(&[
            ".#.........#.",
            "#.#.......#.#",
            "###.......###",
            "#.#.#.#.#.#.#",
            "#.#.......#.#",
        ]);
        let cols = box_columns(&l, 0);
        let mut g = Glyphs::default();
        g.check_label(
            &box_columns(&menu(&[".#.", "#.#", "###", "#.#", "#.#"]), 0),
            "A",
        )
        .unwrap();
        g.teach_label(&cols, "A…A").unwrap();
        assert_eq!(g.read_box(&cols), "A…A");
        g.teach_label(&cols, "A…A").unwrap();
        assert!(g.teach_label(&cols, "A…AA").is_err());
        // Two unknown letters cannot be told apart.
        assert!(Glyphs::default().teach_label(&cols, "A…A").is_err());
    }

    #[test]
    fn a_letter_of_two_pieces_is_learned_from_a_box_of_its_own() {
        // `"`: two columns with a blank one between.
        let quote = menu(&["#.#", "#.#"]);
        let mut g = Glyphs::default();
        g.learn_letter('"', &box_columns(&quote, 0));
        assert_eq!(g.read_box(&box_columns(&quote, 0)), "\"");
        g.check_label(&box_columns(&quote, 0), "\"").unwrap();
    }
}
