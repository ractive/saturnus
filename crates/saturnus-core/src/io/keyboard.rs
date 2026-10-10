//! Keyboard matrices of the supported models (wiki: hardware/keyboard).
//!
//! The CPU drives OUT lines and reads back IN lines: a pressed key
//! connects one OUT line to one IN line. The ON key is wired to IN bit 15
//! and reads as pressed regardless of OUT. The 48SX and 48GX share one
//! matrix ("HP48 matrix": 9 OUT x 6 IN); the 49G has its own ("HP49G
//! matrix": 8 OUT x 8 IN, see [`super::keyboard49`]). The 38G uses the 48
//! matrix and the 39G/40G the 49G matrix, each with its own key labels
//! (see [`super::keyboard_aplet`]). The 42S has its own 6 OUT x 7 IN
//! matrix with EXIT on the ON line (see [`super::keyboard42`]).
//!
//! [`Key`] is one key set across models: keys with the same label and
//! function share a variant and a script name ("enter", "sto", "sin"),
//! and a model only answers the keys it has ([`Key::position`]). A label
//! sits wherever that model has it: SIN on the 38G is at the 48's COS
//! position. The six softkeys (menu keys) are `A`-`F` on every model
//! (their alpha letters on the 48 and 49G); "f1"-"f6" are accepted as
//! names for them.

/// Which keyboard matrix a model has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Layout {
    /// HP48 SX/GX (and the 38G, which uses the same matrix).
    #[default]
    Hp48,
    /// HP49G.
    Hp49,
    /// HP 38G: the 48 matrix with the 38G's labels, VAR and NXT positions
    /// empty (wiki: hardware/hp38g "Keyboard").
    Hp38,
    /// HP 39G and 40G: the 49G matrix with the 39G's labels (wiki:
    /// hardware/hp39g-40g "Keyboard").
    Hp39,
    /// HP 42S: 6 OUT x 7 IN, EXIT on IN bit 15 (wiki: hardware/hp42s
    /// "Keyboard").
    Hp42,
}

impl Layout {
    /// IN lines the matrix uses: 6 on the 48 matrix, 8 on the 49G's, 7
    /// on the 42S's.
    pub fn in_mask(self) -> u8 {
        match self {
            Layout::Hp48 | Layout::Hp38 => 0x3F,
            Layout::Hp42 => 0x7F,
            Layout::Hp49 | Layout::Hp39 => 0xFF,
        }
    }
}

/// Where a key sits in a matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyPos {
    /// Crossing of OUT bit `out` and the IN bits in `mask`.
    Matrix {
        /// OUT bit index.
        out: usize,
        /// IN mask (one bit).
        mask: u8,
    },
    /// The ON key: IN bit 15 for any OUT.
    On,
}

/// A calculator key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum Key {
    // Softkeys A-F (48: OUT bit 8 and #002/#10; 49G: F1-F6)
    A,
    B,
    C,
    D,
    E,
    F,
    Prg,
    Cst,
    Var,
    Up,
    Nxt,
    Sto,
    Eval,
    Left,
    Down,
    Right,
    Cos,
    Tan,
    Sqrt,
    Power,
    Inv,
    Enter,
    Neg,
    Eex,
    Del,
    Backspace,
    Alpha,
    Sin,
    Seven,
    Eight,
    Nine,
    Divide,
    LeftShift,
    Mth,
    Four,
    Five,
    Six,
    Multiply,
    RightShift,
    One,
    Two,
    Three,
    Minus,
    Quote,
    Zero,
    Point,
    Space,
    Plus,
    // 49G only
    Apps,
    Mode,
    Tool,
    Hist,
    Cat,
    Eqw,
    Symb,
    /// The 49G's variable key "X".
    X,
    // 38G and 39G/40G labels
    Plot,
    Num,
    Lib,
    Math,
    Home,
    /// The X,T,θ key.
    Xt,
    LParen,
    RParen,
    Shift,
    Comma,
    Aplet,
    Views,
    Vars,
    /// The 39G's d/dx key.
    Ddx,
    Ln,
    Log,
    /// The 39G's x² key.
    Square,
    // 42S labels
    /// Σ+ (the 42S's first top-row key).
    SigmaPlus,
    /// XEQ, execute.
    Xeq,
    /// RCL, recall.
    Rcl,
    /// R↓, roll down.
    RollDown,
    /// x≷y, swap X and Y.
    Swap,
    /// R/S, run/stop.
    Rs,
    /// The ON key, wired to IN bit 15 independent of OUT.
    On,
}

/// IN bit of the ON key.
const ON_IN_BIT: u16 = 0x8000;

impl Key {
    /// Every key of every model, ON last.
    pub const ALL: [Key; 80] = [
        Key::A,
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
        Key::One,
        Key::Two,
        Key::Three,
        Key::Minus,
        Key::Quote,
        Key::Zero,
        Key::Point,
        Key::Space,
        Key::Plus,
        Key::Apps,
        Key::Mode,
        Key::Tool,
        Key::Hist,
        Key::Cat,
        Key::Eqw,
        Key::Symb,
        Key::X,
        Key::Plot,
        Key::Num,
        Key::Lib,
        Key::Math,
        Key::Home,
        Key::Xt,
        Key::LParen,
        Key::RParen,
        Key::Shift,
        Key::Comma,
        Key::Aplet,
        Key::Views,
        Key::Vars,
        Key::Ddx,
        Key::Ln,
        Key::Log,
        Key::Square,
        Key::SigmaPlus,
        Key::Xeq,
        Key::Rcl,
        Key::RollDown,
        Key::Swap,
        Key::Rs,
        Key::On,
    ];

    /// Position of the key on `layout`, or `None` if that keyboard does
    /// not have it.
    pub(crate) fn position(self, layout: Layout) -> Option<KeyPos> {
        match layout {
            Layout::Hp48 => position_48(self),
            Layout::Hp49 => super::keyboard49::position(self),
            Layout::Hp38 => super::keyboard_aplet::as_48_key(self).and_then(position_48),
            Layout::Hp39 => {
                super::keyboard_aplet::as_49_key(self).and_then(super::keyboard49::position)
            }
            Layout::Hp42 => super::keyboard42::position(self),
        }
    }

    /// The keys `layout` has, in [`Key::ALL`] order.
    pub(crate) fn on_layout(layout: Layout) -> impl Iterator<Item = Key> {
        Key::ALL
            .into_iter()
            .filter(move |k| k.position(layout).is_some())
    }

    /// Script name: "0".."9" for digit keys, the lowercase variant name
    /// otherwise (e.g. "enter", "leftshift", "on").
    pub fn name(self) -> &'static str {
        match self {
            Key::A => "a",
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
            Key::One => "1",
            Key::Two => "2",
            Key::Three => "3",
            Key::Minus => "minus",
            Key::Quote => "quote",
            Key::Zero => "0",
            Key::Point => "point",
            Key::Space => "space",
            Key::Plus => "plus",
            Key::Apps => "apps",
            Key::Mode => "mode",
            Key::Tool => "tool",
            Key::Hist => "hist",
            Key::Cat => "cat",
            Key::Eqw => "eqw",
            Key::Symb => "symb",
            Key::X => "x",
            Key::Plot => "plot",
            Key::Num => "num",
            Key::Lib => "lib",
            Key::Math => "math",
            Key::Home => "home",
            Key::Xt => "xt",
            Key::LParen => "lparen",
            Key::RParen => "rparen",
            Key::Shift => "shift",
            Key::Comma => "comma",
            Key::Aplet => "aplet",
            Key::Views => "views",
            Key::Vars => "vars",
            Key::Ddx => "ddx",
            Key::Ln => "ln",
            Key::Log => "log",
            Key::Square => "square",
            Key::SigmaPlus => "sigmaplus",
            Key::Xeq => "xeq",
            Key::Rcl => "rcl",
            Key::RollDown => "rdn",
            Key::Swap => "swap",
            Key::Rs => "rs",
            Key::On => "on",
        }
    }

    /// Look up a key by its script name (case-insensitive); "f1"-"f6"
    /// name the softkeys A-F and "exit" the ON key (the 42S labels it
    /// EXIT). Model-agnostic by design: "exit" is ON on every model, and
    /// "f1" is A everywhere (the 42S, which has no A-F, then refuses it).
    pub fn from_name(name: &str) -> Option<Key> {
        const SOFTKEYS: [(&str, Key); 7] = [
            ("f1", Key::A),
            ("f2", Key::B),
            ("f3", Key::C),
            ("f4", Key::D),
            ("f5", Key::E),
            ("f6", Key::F),
            ("exit", Key::On),
        ];
        Key::ALL
            .iter()
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(name))
            .or_else(|| {
                SOFTKEYS
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                    .map(|&(_, k)| k)
            })
    }
}

/// The HP48 matrix (wiki: hardware/keyboard "HP48 matrix").
pub(crate) fn position_48(key: Key) -> Option<KeyPos> {
    let (out, mask) = match key {
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
        Key::On => return Some(KeyPos::On),
        Key::Apps
        | Key::Mode
        | Key::Tool
        | Key::Hist
        | Key::Cat
        | Key::Eqw
        | Key::Symb
        | Key::X
        | Key::Plot
        | Key::Num
        | Key::Lib
        | Key::Math
        | Key::Home
        | Key::Xt
        | Key::LParen
        | Key::RParen
        | Key::Shift
        | Key::Comma
        | Key::Aplet
        | Key::Views
        | Key::Vars
        | Key::Ddx
        | Key::Ln
        | Key::Log
        | Key::Square
        | Key::SigmaPlus
        | Key::Xeq
        | Key::Rcl
        | Key::RollDown
        | Key::Swap
        | Key::Rs => return None,
    };
    Some(KeyPos::Matrix { out, mask })
}

/// Pressed-key state of the keyboard matrix.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Keyboard {
    /// The matrix keys are placed on.
    pub(crate) layout: Layout,
    /// IN bits pressed on each OUT line (index = OUT bit).
    pub(crate) rows: [u8; 9],
    /// ON key state.
    pub(crate) on: bool,
}

impl Keyboard {
    /// An HP48 keyboard with no key pressed.
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    /// A keyboard of `layout` with no key pressed.
    pub fn with_layout(layout: Layout) -> Self {
        Self {
            layout,
            ..Self::default()
        }
    }

    /// The matrix this keyboard uses.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Press `k`. Returns false, changing nothing, when the layout does
    /// not have the key; [`crate::Machine::key_down`] turns that into an
    /// error before it gets here.
    pub fn press(&mut self, k: Key) -> bool {
        match k.position(self.layout) {
            Some(KeyPos::Matrix { out, mask }) => self.rows[out] |= mask,
            Some(KeyPos::On) => self.on = true,
            None => return false,
        }
        true
    }

    /// Release `k`. Returns false, changing nothing, when the layout does
    /// not have the key.
    pub fn release(&mut self, k: Key) -> bool {
        match k.position(self.layout) {
            Some(KeyPos::Matrix { out, mask }) => self.rows[out] &= !mask,
            Some(KeyPos::On) => self.on = false,
            None => return false,
        }
        true
    }

    /// Release every key.
    pub fn release_all(&mut self) {
        *self = Self::with_layout(self.layout);
    }

    /// Whether `k` is held down. A key the layout lacks is never held, so
    /// this is false for it.
    pub fn is_pressed(&self, k: Key) -> bool {
        match k.position(self.layout) {
            Some(KeyPos::Matrix { out, mask }) => self.rows[out] & mask != 0,
            Some(KeyPos::On) => self.on,
            None => false,
        }
    }

    /// Whether ON is held down.
    pub fn on_pressed(&self) -> bool {
        self.on
    }

    /// IN lines seen by the CPU for the OUT lines it drives: the OR of the
    /// rows of every set OUT bit (0..9; the 49G uses 0..8 and its row 8
    /// stays empty), plus bit 15 while ON is held.
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
        assert_eq!(Key::from_name("F1"), Some(Key::A));
        assert_eq!(Key::from_name("f6"), Some(Key::F));
        let names: std::collections::HashSet<_> = Key::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(names.len(), Key::ALL.len());
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
        let mut on_48: Vec<Key> = Key::on_layout(Layout::Hp48).collect();
        let mut listed = listed;
        listed.sort_by_key(|k| k.name());
        on_48.sort_by_key(|k| k.name());
        assert_eq!(listed, on_48, "table covers exactly the 48 keys");
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
        let positions: std::collections::HashSet<_> = Key::ALL
            .iter()
            .filter_map(|k| k.position(Layout::Hp48))
            .collect();
        assert_eq!(positions.len(), 49, "48 matrix cells and ON");
        let mut kb = Keyboard::new();
        for k in Key::on_layout(Layout::Hp48) {
            assert!(!kb.is_pressed(k));
            kb.press(k);
            assert!(kb.is_pressed(k));
        }
        assert_eq!(kb.read_in(0x1FF), 0x803F);
        // 49G-only keys are not on the 48.
        kb.release_all();
        assert!(!kb.press(Key::Apps), "APPS is not on the 48");
        assert!(!kb.release(Key::Apps));
        assert!(!kb.is_pressed(Key::Apps));
        assert_eq!(kb.read_in(0x1FF), 0);
    }
}
