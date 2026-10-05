//! HP48 SX/GX keyboard matrix (wiki: hardware/keyboard "HP48 matrix").
//!
//! The CPU drives the 9 OUT lines and reads back the IN lines: a pressed key
//! connects one OUT line to one IN line. The ON key is wired to IN bit 15
//! and reads as pressed regardless of OUT.

/// A key of the HP48 keyboard.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum Key {
    // OUT bit 8
    B,
    C,
    D,
    E,
    F,
    // OUT bit 7
    Prg,
    Cst,
    Var,
    Up,
    Nxt,
    // OUT bit 6
    Sto,
    Eval,
    Left,
    Down,
    Right,
    // OUT bit 5
    Cos,
    Tan,
    Sqrt,
    Power,
    Inv,
    // OUT bit 4
    Enter,
    Neg,
    Eex,
    Del,
    Backspace,
    // OUT bit 3
    Alpha,
    Sin,
    Seven,
    Eight,
    Nine,
    Divide,
    // OUT bit 2
    LeftShift,
    Mth,
    Four,
    Five,
    Six,
    Multiply,
    // OUT bit 1
    RightShift,
    A,
    One,
    Two,
    Three,
    Minus,
    // OUT bit 0
    Quote,
    Zero,
    Point,
    Space,
    Plus,
    /// The ON key, wired to IN bit 15 independent of OUT.
    On,
}

/// IN bit of the ON key.
const ON_IN_BIT: u16 = 0x8000;

impl Key {
    /// Every key, in matrix order (OUT bit 8 first), ON last.
    pub const ALL: [Key; 49] = [
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::Prg,
        Key::Cst,
        Key::Var,
        Key::Up,
        Key::Nxt,
        Key::Sto,
        Key::Eval,
        Key::Left,
        Key::Down,
        Key::Right,
        Key::Cos,
        Key::Tan,
        Key::Sqrt,
        Key::Power,
        Key::Inv,
        Key::Enter,
        Key::Neg,
        Key::Eex,
        Key::Del,
        Key::Backspace,
        Key::Alpha,
        Key::Sin,
        Key::Seven,
        Key::Eight,
        Key::Nine,
        Key::Divide,
        Key::LeftShift,
        Key::Mth,
        Key::Four,
        Key::Five,
        Key::Six,
        Key::Multiply,
        Key::RightShift,
        Key::A,
        Key::One,
        Key::Two,
        Key::Three,
        Key::Minus,
        Key::Quote,
        Key::Zero,
        Key::Point,
        Key::Space,
        Key::Plus,
        Key::On,
    ];

    /// Matrix position as (OUT bit index, IN mask); `None` for ON.
    pub fn matrix(self) -> Option<(usize, u8)> {
        let pos = match self {
            Key::B => (8, 0x10),
            Key::C => (8, 0x08),
            Key::D => (8, 0x04),
            Key::E => (8, 0x02),
            Key::F => (8, 0x01),
            Key::Prg => (7, 0x10),
            Key::Cst => (7, 0x08),
            Key::Var => (7, 0x04),
            Key::Up => (7, 0x02),
            Key::Nxt => (7, 0x01),
            Key::Sto => (6, 0x10),
            Key::Eval => (6, 0x08),
            Key::Left => (6, 0x04),
            Key::Down => (6, 0x02),
            Key::Right => (6, 0x01),
            Key::Cos => (5, 0x10),
            Key::Tan => (5, 0x08),
            Key::Sqrt => (5, 0x04),
            Key::Power => (5, 0x02),
            Key::Inv => (5, 0x01),
            Key::Enter => (4, 0x10),
            Key::Neg => (4, 0x08),
            Key::Eex => (4, 0x04),
            Key::Del => (4, 0x02),
            Key::Backspace => (4, 0x01),
            Key::Alpha => (3, 0x20),
            Key::Sin => (3, 0x10),
            Key::Seven => (3, 0x08),
            Key::Eight => (3, 0x04),
            Key::Nine => (3, 0x02),
            Key::Divide => (3, 0x01),
            Key::LeftShift => (2, 0x20),
            Key::Mth => (2, 0x10),
            Key::Four => (2, 0x08),
            Key::Five => (2, 0x04),
            Key::Six => (2, 0x02),
            Key::Multiply => (2, 0x01),
            Key::RightShift => (1, 0x20),
            Key::A => (1, 0x10),
            Key::One => (1, 0x08),
            Key::Two => (1, 0x04),
            Key::Three => (1, 0x02),
            Key::Minus => (1, 0x01),
            Key::Quote => (0, 0x10),
            Key::Zero => (0, 0x08),
            Key::Point => (0, 0x04),
            Key::Space => (0, 0x02),
            Key::Plus => (0, 0x01),
            Key::On => return None,
        };
        Some(pos)
    }

    /// Script name: "0".."9" for digit keys, the lowercase variant name
    /// otherwise (e.g. "enter", "leftshift", "on").
    pub fn name(self) -> &'static str {
        match self {
            Key::B => "b",
            Key::C => "c",
            Key::D => "d",
            Key::E => "e",
            Key::F => "f",
            Key::Prg => "prg",
            Key::Cst => "cst",
            Key::Var => "var",
            Key::Up => "up",
            Key::Nxt => "nxt",
            Key::Sto => "sto",
            Key::Eval => "eval",
            Key::Left => "left",
            Key::Down => "down",
            Key::Right => "right",
            Key::Cos => "cos",
            Key::Tan => "tan",
            Key::Sqrt => "sqrt",
            Key::Power => "power",
            Key::Inv => "inv",
            Key::Enter => "enter",
            Key::Neg => "neg",
            Key::Eex => "eex",
            Key::Del => "del",
            Key::Backspace => "backspace",
            Key::Alpha => "alpha",
            Key::Sin => "sin",
            Key::Seven => "7",
            Key::Eight => "8",
            Key::Nine => "9",
            Key::Divide => "divide",
            Key::LeftShift => "leftshift",
            Key::Mth => "mth",
            Key::Four => "4",
            Key::Five => "5",
            Key::Six => "6",
            Key::Multiply => "multiply",
            Key::RightShift => "rightshift",
            Key::A => "a",
            Key::One => "1",
            Key::Two => "2",
            Key::Three => "3",
            Key::Minus => "minus",
            Key::Quote => "quote",
            Key::Zero => "0",
            Key::Point => "point",
            Key::Space => "space",
            Key::Plus => "plus",
            Key::On => "on",
        }
    }

    /// Look up a key by its script name (case-insensitive).
    pub fn from_name(name: &str) -> Option<Key> {
        Key::ALL
            .iter()
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(name))
    }
}

/// Pressed-key state of the keyboard matrix.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Keyboard {
    /// IN bits pressed on each OUT line (index = OUT bit).
    pub(crate) rows: [u8; 9],
    /// ON key state.
    pub(crate) on: bool,
}

impl Keyboard {
    /// A keyboard with no key pressed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Press `k`.
    pub fn press(&mut self, k: Key) {
        match k.matrix() {
            Some((out, mask)) => self.rows[out] |= mask,
            None => self.on = true,
        }
    }

    /// Release `k`.
    pub fn release(&mut self, k: Key) {
        match k.matrix() {
            Some((out, mask)) => self.rows[out] &= !mask,
            None => self.on = false,
        }
    }

    /// Release every key.
    pub fn release_all(&mut self) {
        *self = Self::default();
    }

    /// Whether `k` is held down.
    pub fn is_pressed(&self, k: Key) -> bool {
        match k.matrix() {
            Some((out, mask)) => self.rows[out] & mask != 0,
            None => self.on,
        }
    }

    /// Whether ON is held down.
    pub fn on_pressed(&self) -> bool {
        self.on
    }

    /// IN lines seen by the CPU for the OUT lines it drives: the OR of the
    /// rows of every set OUT bit (0..9), plus bit 15 while ON is held.
    pub fn read_in(&self, out: u16) -> u16 {
        let mut value = self
            .rows
            .iter()
            .enumerate()
            .filter(|(i, _)| out & (1 << i) != 0)
            .fold(0u16, |acc, (_, row)| acc | u16::from(*row));
        if self.on {
            value |= ON_IN_BIT;
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_row() {
        let mut kb = Keyboard::new();
        kb.press(Key::Enter);
        assert_eq!(kb.read_in(0x010), 0x0010);
        assert_eq!(kb.read_in(0x1FF), 0x0010);
        assert_eq!(kb.read_in(0x001), 0);
        kb.release(Key::Enter);
        assert_eq!(kb.read_in(0x1FF), 0);
    }

    #[test]
    fn on_ignores_out() {
        let mut kb = Keyboard::new();
        kb.press(Key::On);
        assert_eq!(kb.read_in(0), 0x8000);
        assert!(kb.on_pressed());
        kb.release_all();
        assert_eq!(kb.read_in(0x1FF), 0);
    }

    #[test]
    fn names_round_trip() {
        for k in Key::ALL {
            assert_eq!(Key::from_name(k.name()), Some(k));
            assert_eq!(Key::from_name(&k.name().to_uppercase()), Some(k));
        }
        assert_eq!(Key::from_name("7"), Some(Key::Seven));
        assert_eq!(Key::from_name("LeftShift"), Some(Key::LeftShift));
        assert_eq!(Key::from_name("nope"), None);
    }

    /// The HP48 matrix from wiki: hardware/keyboard ("HP48 matrix"), cell
    /// by cell as (OUT mask, IN mask), written out independently of
    /// `Key::matrix`. Left shift is OUT #004 / IN #20 and right shift OUT
    /// #002 / IN #20 (the tutorial and Ervin; Mastracci 4.9 has them
    /// swapped). ON is IN #8000 for any OUT, so its OUT mask here is 0.
    const WIKI_MATRIX: [(Key, u16, u16); 49] = [
        (Key::B, 0x100, 0x10),
        (Key::C, 0x100, 0x08),
        (Key::D, 0x100, 0x04),
        (Key::E, 0x100, 0x02),
        (Key::F, 0x100, 0x01),
        (Key::Prg, 0x080, 0x10),
        (Key::Cst, 0x080, 0x08),
        (Key::Var, 0x080, 0x04),
        (Key::Up, 0x080, 0x02),
        (Key::Nxt, 0x080, 0x01),
        (Key::Sto, 0x040, 0x10),
        (Key::Eval, 0x040, 0x08),
        (Key::Left, 0x040, 0x04),
        (Key::Down, 0x040, 0x02),
        (Key::Right, 0x040, 0x01),
        (Key::Cos, 0x020, 0x10),
        (Key::Tan, 0x020, 0x08),
        (Key::Sqrt, 0x020, 0x04),
        (Key::Power, 0x020, 0x02),
        (Key::Inv, 0x020, 0x01),
        (Key::Enter, 0x010, 0x10),
        (Key::Neg, 0x010, 0x08),
        (Key::Eex, 0x010, 0x04),
        (Key::Del, 0x010, 0x02),
        (Key::Backspace, 0x010, 0x01),
        (Key::Alpha, 0x008, 0x20),
        (Key::Sin, 0x008, 0x10),
        (Key::Seven, 0x008, 0x08),
        (Key::Eight, 0x008, 0x04),
        (Key::Nine, 0x008, 0x02),
        (Key::Divide, 0x008, 0x01),
        (Key::LeftShift, 0x004, 0x20),
        (Key::Mth, 0x004, 0x10),
        (Key::Four, 0x004, 0x08),
        (Key::Five, 0x004, 0x04),
        (Key::Six, 0x004, 0x02),
        (Key::Multiply, 0x004, 0x01),
        (Key::RightShift, 0x002, 0x20),
        (Key::A, 0x002, 0x10),
        (Key::One, 0x002, 0x08),
        (Key::Two, 0x002, 0x04),
        (Key::Three, 0x002, 0x02),
        (Key::Minus, 0x002, 0x01),
        (Key::Quote, 0x001, 0x10),
        (Key::Zero, 0x001, 0x08),
        (Key::Point, 0x001, 0x04),
        (Key::Space, 0x001, 0x02),
        (Key::Plus, 0x001, 0x01),
        (Key::On, 0x000, 0x8000),
    ];

    #[test]
    fn every_key_matches_the_wiki_matrix() {
        let listed: Vec<Key> = WIKI_MATRIX.iter().map(|e| e.0).collect();
        assert_eq!(listed, Key::ALL.to_vec(), "table covers Key::ALL in order");
        for (key, out, inp) in WIKI_MATRIX {
            let mut kb = Keyboard::new();
            kb.press(key);
            // Driving only the key's row gives exactly its column.
            assert_eq!(kb.read_in(out), inp, "{key:?} on its own row");
            // Every other single row is silent, except ON, which ignores OUT.
            for bit in 0..12 {
                let other = 1u16 << bit;
                if other == out {
                    continue;
                }
                let want = if key == Key::On { 0x8000 } else { 0 };
                assert_eq!(kb.read_in(other), want, "{key:?} with OUT {other:#05X}");
            }
            // All rows at once, plus the buzzer bit (OUT bit 11), which
            // the matrix ignores (wiki: hardware/keyboard, Voyage p. 80).
            assert_eq!(kb.read_in(0x1FF), inp);
            assert_eq!(kb.read_in(0x9FF), inp);
            assert_eq!(kb.read_in(0x800), if key == Key::On { inp } else { 0 });
        }
    }

    #[test]
    fn matrix_positions_unique() {
        let positions: std::collections::HashSet<_> =
            Key::ALL.iter().filter_map(|k| k.matrix()).collect();
        assert_eq!(positions.len(), 48);
        let mut kb = Keyboard::new();
        for k in Key::ALL {
            assert!(!kb.is_pressed(k));
            kb.press(k);
            assert!(kb.is_pressed(k));
        }
        assert_eq!(kb.read_in(0x1FF), 0x803F);
    }
}
