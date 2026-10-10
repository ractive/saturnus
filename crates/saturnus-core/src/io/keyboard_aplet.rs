//! Key labels of the "aplet" calculators: the 38G on the 48 matrix and the
//! 39G/40G on the 49G matrix.
//!
//! Each table puts a key at the matrix position of the 48 (or 49G) key in
//! the same place on the case. That is an inference: the KML OutIn tables
//! give codes only by 48/49G key position and say they apply to these
//! models; the reset chords (ON plus the third menu key, ON plus the first
//! and last menu keys) fit it (wiki: hardware/hp38g "Keyboard",
//! hardware/hp39g-40g "Keyboard"). The 38G arithmetic `6 * 7 ENTER` and
//! the 39G entries in the e2e tests confirm the digit, operator and ENTER
//! rows by booting the ROMs.
//!
//! A label shared with another model keeps its [`Key`] variant (SIN is
//! `Key::Sin` everywhere) even where it sits at another matrix position.

use super::keyboard::Key;

/// The 48 key at the place of the 38G key `key`, or `None` when the 38G
/// has no such key (wiki: hardware/hp38g "Keyboard"; its VAR and NXT
/// positions are empty).
pub(crate) fn as_48_key(key: Key) -> Option<Key> {
    Some(match key {
        // Row 1: menu keys.
        Key::A | Key::B | Key::C | Key::D | Key::E | Key::F => key,
        // Row 2.
        Key::Plot => Key::Mth,
        Key::Symb => Key::Prg,
        Key::Num => Key::Cst,
        Key::Up => Key::Up,
        // Row 3.
        Key::Lib => Key::Quote,
        Key::Var => Key::Sto,
        Key::Math => Key::Eval,
        Key::Left | Key::Down | Key::Right => key,
        // Row 4.
        Key::Home => Key::Sin,
        Key::Sin => Key::Cos,
        Key::Cos => Key::Tan,
        Key::Tan => Key::Sqrt,
        Key::Xt => Key::Power,
        Key::Sqrt => Key::Inv,
        // Row 5.
        Key::Enter => Key::Enter,
        Key::LParen => Key::Neg,
        Key::RParen => Key::Eex,
        Key::Neg => Key::Del,
        Key::Power => Key::Backspace,
        // Rows 6-9.
        Key::Alpha => Key::Alpha,
        Key::Shift => Key::LeftShift,
        Key::Del => Key::RightShift,
        Key::Comma => Key::Space,
        Key::Seven
        | Key::Eight
        | Key::Nine
        | Key::Divide
        | Key::Four
        | Key::Five
        | Key::Six
        | Key::Multiply
        | Key::One
        | Key::Two
        | Key::Three
        | Key::Minus
        | Key::Zero
        | Key::Point
        | Key::Plus
        | Key::On => key,
        _ => return None,
    })
}

/// The 49G key at the place of the 39G/40G key `key`, or `None` when the
/// 39G has no such key (wiki: hardware/hp39g-40g "Keyboard"; every 49G
/// position is used).
pub(crate) fn as_49_key(key: Key) -> Option<Key> {
    Some(match key {
        // Row 1: menu keys.
        Key::A | Key::B | Key::C | Key::D | Key::E | Key::F => key,
        // Row 2.
        Key::Symb => Key::Apps,
        Key::Plot => Key::Mode,
        Key::Num => Key::Tool,
        Key::Up => Key::Up,
        // Row 3.
        Key::Home => Key::Var,
        Key::Aplet => Key::Sto,
        Key::Views => Key::Nxt,
        Key::Left | Key::Down | Key::Right => key,
        // Row 4.
        Key::Vars => Key::Hist,
        Key::Math => Key::Cat,
        Key::Ddx => Key::Eqw,
        Key::Xt => Key::Symb,
        Key::Del => Key::Backspace,
        // Row 5.
        Key::Sin => Key::Power,
        Key::Cos => Key::Sqrt,
        Key::Tan => Key::Sin,
        Key::Ln => Key::Cos,
        Key::Log => Key::Tan,
        // Row 6.
        Key::Square => Key::Eex,
        Key::Power => Key::Neg,
        Key::LParen => Key::X,
        Key::RParen => Key::Inv,
        Key::Divide => Key::Divide,
        // Rows 7-10.
        Key::Comma => Key::Alpha,
        Key::Alpha => Key::LeftShift,
        Key::Shift => Key::RightShift,
        Key::Neg => Key::Space,
        Key::Seven
        | Key::Eight
        | Key::Nine
        | Key::Multiply
        | Key::Four
        | Key::Five
        | Key::Six
        | Key::Minus
        | Key::One
        | Key::Two
        | Key::Three
        | Key::Plus
        | Key::Zero
        | Key::Point
        | Key::Enter
        | Key::On => key,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::keyboard::{KeyPos, Keyboard, Layout};
    use super::*;
    use std::collections::HashSet;

    /// The 38G table from wiki: hardware/hp38g "Keyboard", cell by cell as
    /// (key, OUT mask, IN mask), written out independently of
    /// `as_48_key`. ON is IN #8000 for any OUT.
    const WIKI_38G: [(Key, u16, u16); 47] = [
        (Key::A, 0x002, 0x10),
        (Key::B, 0x100, 0x10),
        (Key::C, 0x100, 0x08),
        (Key::D, 0x100, 0x04),
        (Key::E, 0x100, 0x02),
        (Key::F, 0x100, 0x01),
        (Key::Plot, 0x004, 0x10),
        (Key::Symb, 0x080, 0x10),
        (Key::Num, 0x080, 0x08),
        (Key::Up, 0x080, 0x02),
        (Key::Lib, 0x001, 0x10),
        (Key::Var, 0x040, 0x10),
        (Key::Math, 0x040, 0x08),
        (Key::Left, 0x040, 0x04),
        (Key::Down, 0x040, 0x02),
        (Key::Right, 0x040, 0x01),
        (Key::Home, 0x008, 0x10),
        (Key::Sin, 0x020, 0x10),
        (Key::Cos, 0x020, 0x08),
        (Key::Tan, 0x020, 0x04),
        (Key::Xt, 0x020, 0x02),
        (Key::Sqrt, 0x020, 0x01),
        (Key::Enter, 0x010, 0x10),
        (Key::LParen, 0x010, 0x08),
        (Key::RParen, 0x010, 0x04),
        (Key::Neg, 0x010, 0x02),
        (Key::Power, 0x010, 0x01),
        (Key::Alpha, 0x008, 0x20),
        (Key::Seven, 0x008, 0x08),
        (Key::Eight, 0x008, 0x04),
        (Key::Nine, 0x008, 0x02),
        (Key::Divide, 0x008, 0x01),
        (Key::Shift, 0x004, 0x20),
        (Key::Four, 0x004, 0x08),
        (Key::Five, 0x004, 0x04),
        (Key::Six, 0x004, 0x02),
        (Key::Multiply, 0x004, 0x01),
        (Key::Del, 0x002, 0x20),
        (Key::One, 0x002, 0x08),
        (Key::Two, 0x002, 0x04),
        (Key::Three, 0x002, 0x02),
        (Key::Minus, 0x002, 0x01),
        (Key::On, 0x000, 0x8000),
        (Key::Zero, 0x001, 0x08),
        (Key::Point, 0x001, 0x04),
        (Key::Comma, 0x001, 0x02),
        (Key::Plus, 0x001, 0x01),
    ];

    /// The 39G/40G table from wiki: hardware/hp39g-40g "Keyboard".
    const WIKI_39G: [(Key, u16, u16); 51] = [
        (Key::A, 0x020, 0x01),
        (Key::B, 0x020, 0x02),
        (Key::C, 0x020, 0x04),
        (Key::D, 0x020, 0x08),
        (Key::E, 0x020, 0x10),
        (Key::F, 0x020, 0x20),
        (Key::Symb, 0x020, 0x80),
        (Key::Plot, 0x010, 0x80),
        (Key::Num, 0x008, 0x80),
        (Key::Up, 0x040, 0x08),
        (Key::Home, 0x004, 0x80),
        (Key::Aplet, 0x002, 0x80),
        (Key::Views, 0x001, 0x80),
        (Key::Left, 0x040, 0x04),
        (Key::Down, 0x040, 0x02),
        (Key::Right, 0x040, 0x01),
        (Key::Vars, 0x010, 0x40),
        (Key::Math, 0x008, 0x40),
        (Key::Ddx, 0x004, 0x40),
        (Key::Xt, 0x002, 0x40),
        (Key::Del, 0x001, 0x40),
        (Key::Sin, 0x010, 0x20),
        (Key::Cos, 0x008, 0x20),
        (Key::Tan, 0x004, 0x20),
        (Key::Ln, 0x002, 0x20),
        (Key::Log, 0x001, 0x20),
        (Key::Square, 0x010, 0x10),
        (Key::Power, 0x008, 0x10),
        (Key::LParen, 0x004, 0x10),
        (Key::RParen, 0x002, 0x10),
        (Key::Divide, 0x001, 0x10),
        (Key::Comma, 0x080, 0x08),
        (Key::Seven, 0x008, 0x08),
        (Key::Eight, 0x004, 0x08),
        (Key::Nine, 0x002, 0x08),
        (Key::Multiply, 0x001, 0x08),
        (Key::Alpha, 0x080, 0x04),
        (Key::Four, 0x008, 0x04),
        (Key::Five, 0x004, 0x04),
        (Key::Six, 0x002, 0x04),
        (Key::Minus, 0x001, 0x04),
        (Key::Shift, 0x080, 0x02),
        (Key::One, 0x008, 0x02),
        (Key::Two, 0x004, 0x02),
        (Key::Three, 0x002, 0x02),
        (Key::Plus, 0x001, 0x02),
        (Key::On, 0x000, 0x8000),
        (Key::Zero, 0x008, 0x01),
        (Key::Point, 0x004, 0x01),
        (Key::Neg, 0x002, 0x01),
        (Key::Enter, 0x001, 0x01),
    ];

    fn check(layout: Layout, table: &[(Key, u16, u16)]) {
        let listed: HashSet<Key> = table.iter().map(|e| e.0).collect();
        let on_layout: HashSet<Key> = Key::on_layout(layout).collect();
        assert_eq!(
            listed, on_layout,
            "{layout:?}: table covers exactly the keys"
        );
        for &(key, out, inp) in table {
            let mut kb = Keyboard::with_layout(layout);
            assert!(kb.press(key));
            assert_eq!(kb.read_in(out), inp, "{key:?} on its own row");
            for bit in 0..12 {
                let other = 1u16 << bit;
                if other != out {
                    let want = if key == Key::On { 0x8000 } else { 0 };
                    assert_eq!(kb.read_in(other), want, "{key:?} with OUT {other:#05X}");
                }
            }
        }
        let positions: HashSet<KeyPos> = on_layout
            .iter()
            .filter_map(|k| k.position(layout))
            .collect();
        assert_eq!(
            positions.len(),
            on_layout.len(),
            "{layout:?}: no shared cell"
        );
    }

    #[test]
    fn hp38_matches_the_wiki_table() {
        check(Layout::Hp38, &WIKI_38G);
        assert_eq!(Key::on_layout(Layout::Hp38).count(), 47, "46 keys and ON");
        // The 48's VAR and NXT cells stay empty.
        let mut kb = Keyboard::with_layout(Layout::Hp38);
        for k in Key::ALL {
            kb.press(k);
        }
        assert_eq!(kb.read_in(0x080) & 0xFF, 0x10 | 0x08 | 0x02);
        assert!(!kb.press(Key::Nxt), "no NXT on the 38G");
        assert!(!kb.press(Key::Eval), "no EVAL on the 38G");
    }

    #[test]
    fn hp39_matches_the_wiki_table() {
        check(Layout::Hp39, &WIKI_39G);
        assert_eq!(Key::on_layout(Layout::Hp39).count(), 51, "50 keys and ON");
        assert!(!Keyboard::with_layout(Layout::Hp39).press(Key::Apps));
    }

    #[test]
    fn names_resolve() {
        for n in [
            "plot", "NUM", "aplet", "views", "ddx", "xt", "comma", "shift",
        ] {
            assert!(Key::from_name(n).is_some(), "{n}");
        }
    }
}
