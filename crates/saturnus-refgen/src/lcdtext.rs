//! The text on the key-driven models' displays, read back from the
//! pixels for the key-driven reference ([`crate::keyed`]): the Pioneer
//! display (131 x 16 dots: the 42S, 27S, 17BII and 17B) and the 19BII's
//! (137 x 32 dots).
//!
//! The Pioneer display has two lines of 22 character cells, the 19BII's
//! four lines of 23; each cell is 5 dots wide at a pitch of 6 and 8 rows
//! tall (the eighth row carries descenders and the cursor). When a menu is
//! shown, the last line is six boxes (21 dots at a pitch of 22 on the
//! Pioneers, 22 at a pitch of 23 on the 19BII), dark with light letters
//! in a smaller font of 5 rows inside a dark border one row above and
//! below them. A box with nothing in it is not drawn.
//! The 38G, 39G and 40G draw their menu boxes the same way on the last
//! 7 of their 64 rows (the Pioneer layout, 8 lines), so the menu functions
//! read those too.
//!
//! No font is stored: [`Glyphs`] learns both fonts at run time from text
//! the calculator is made to show (the characters typed through its ALPHA
//! menu or letter keys, the labels the reference expects on its menus),
//! and reads back what it learned. A cell or a letter it has not seen
//! reads as `�`.

use std::collections::HashMap;

use saturnus_core::Lcd;

/// Pitch of the character cells, in dots.
const CELL_PITCH: usize = 6;
/// Width of a character cell, in dots.
const CELL_WIDTH: usize = 5;
/// Rows of a text line.
const LINE_ROWS: usize = 8;
/// Menu boxes on the last line.
pub const BOXES: usize = 6;

/// What a cell or a letter reads as when it was never learned.
pub const UNKNOWN: char = '\u{fffd}';

/// One character cell: a column mask per dot column, bit 0 the top row.
pub type Cell = [u8; CELL_WIDTH];

/// The letters of a menu box: one mask per dot column (bit 0 the top
/// letter row), light dots set, the blank columns at either end trimmed.
pub type BoxColumns = Vec<u8>;

/// Where the text and the menu boxes sit on a display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout {
    /// Character cells per line.
    cells: usize,
    /// Text lines; the last one holds the menu boxes.
    lines: usize,
    /// Pitch and width of a menu box.
    box_pitch: usize,
    box_width: usize,
}

impl Layout {
    fn of(lcd: &Lcd) -> Self {
        if lcd.width() == 137 {
            Layout {
                cells: 23,
                lines: lcd.height() / LINE_ROWS,
                box_pitch: 23,
                box_width: 22,
            }
        } else {
            Layout {
                cells: 22,
                lines: lcd.height() / LINE_ROWS,
                box_pitch: 22,
                box_width: 21,
            }
        }
    }

    /// The border row above the menu letters.
    fn box_top(&self) -> usize {
        (self.lines - 1) * LINE_ROWS + 1
    }

    /// The border row below them.
    fn box_bottom(&self) -> usize {
        self.box_top() + 6
    }

    /// The rows of the menu letters.
    fn letter_rows(&self) -> std::ops::Range<usize> {
        self.box_top() + 1..self.box_bottom()
    }

    fn box_xs(&self, b: usize) -> std::ops::Range<usize> {
        b * self.box_pitch..b * self.box_pitch + self.box_width
    }
}

/// Whether `lcd` is a display this module reads.
pub fn fits(lcd: &Lcd) -> bool {
    (lcd.height() == 16 && lcd.width() == 131) || (lcd.height() == 32 && lcd.width() == 137)
}

/// Text lines of `lcd`, the menu line included.
pub fn lines(lcd: &Lcd) -> usize {
    Layout::of(lcd).lines
}

fn dark(lcd: &Lcd, x: usize, y: usize) -> bool {
    lcd.pixels
        .get(y)
        .and_then(|r| r.get(x))
        .copied()
        .unwrap_or(false)
}

/// Cell `i` of text line `line` (0 the upper).
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

/// Whether box `b` shows both its border rows dark.
fn box_framed(lcd: &Lcd, l: &Layout, b: usize) -> bool {
    l.box_xs(b)
        .all(|x| dark(lcd, x, l.box_top()) && dark(lcd, x, l.box_bottom()))
}

/// Whether box `b` is drawn: one of its border rows dark (a character
/// such as the 42S's `"` reaches into the upper one).
fn box_drawn(lcd: &Lcd, l: &Layout, b: usize) -> bool {
    [l.box_top(), l.box_bottom()]
        .iter()
        .any(|&y| l.box_xs(b).all(|x| dark(lcd, x, y)))
}

/// Whether the last line shows menu boxes: some box is framed.
pub fn has_menu(lcd: &Lcd) -> bool {
    let l = Layout::of(lcd);
    (0..BOXES).any(|b| box_framed(lcd, &l, b))
}

/// The letter columns of menu box `b` (0-5), trimmed; empty for a blank
/// box or one not drawn.
pub fn box_columns(lcd: &Lcd, b: usize) -> BoxColumns {
    let l = Layout::of(lcd);
    if !box_drawn(lcd, &l, b) {
        return Vec::new();
    }
    let cols: Vec<u8> = l
        .box_xs(b)
        .map(|x| {
            l.letter_rows()
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

    /// Learn that `cell` shows `c`, refusing a cell already learned as
    /// another character (a learn screen whose text does not line up with
    /// the display): `Err` holds that character.
    pub fn learn_cell_checked(&mut self, cell: Cell, c: char) -> Result<(), char> {
        match self.cells.get(&cell) {
            Some(&old) if old != c => Err(old),
            _ => {
                self.learn_cell(cell, c);
                Ok(())
            }
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
        let s: String = (0..Layout::of(lcd).cells)
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

    /// The display as text: the text lines from the top, the last one
    /// the menu ([`Glyphs::read_menu`]) when one shows. Lines with nothing
    /// on them are empty strings.
    pub fn read(&self, lcd: &Lcd) -> Vec<String> {
        let n = lines(lcd);
        let mut out: Vec<String> = (0..n - 1).map(|l| self.read_line(lcd, l)).collect();
        out.push(if has_menu(lcd) {
            self.read_menu(lcd)
        } else {
            self.read_line(lcd, n - 1)
        });
        out
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
        menu_sized(131, 16, letters)
    }

    fn menu_sized(w: usize, h: usize, letters: &[&str]) -> Lcd {
        let mut l = Lcd::blank_sized(w, h);
        let lay = Layout::of(&l);
        for b in 0..BOXES {
            for x in lay.box_xs(b) {
                for y in lay.box_top()..=lay.box_bottom() {
                    l.pixels[y][x] = true;
                }
            }
        }
        for (dy, r) in letters.iter().enumerate() {
            for (dx, c) in r.chars().enumerate() {
                if c == '#' {
                    l.pixels[lay.letter_rows().start + dy][2 + dx] = false;
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
    fn the_19bii_has_four_lines_and_wider_boxes() {
        let mut l = menu_sized(137, 32, &[".#.", "#.#", "###", "#.#", "#.#"]);
        assert!(fits(&l));
        let mut g = Glyphs::default();
        g.check_label(&box_columns(&l, 0), "A").unwrap();
        assert_eq!(g.read_box(&box_columns(&l, 1)), "");
        assert_eq!(g.read(&l).len(), 4);
        assert_eq!(g.read(&l)[3], "[A][][][][][]");
        // A box not drawn is blank, not a box of light dots.
        for x in 23..45 {
            l.pixels[25][x] = false;
            l.pixels[31][x] = false;
        }
        assert!(box_columns(&l, 1).is_empty());
        assert!(has_menu(&l));
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
