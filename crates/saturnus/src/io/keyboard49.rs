//! HP 49G keyboard matrix (wiki: hardware/keyboard "HP49G matrix").
//!
//! Same scanning model as the 48 (see [`super::keyboard`]): the CPU drives
//! OUT lines and reads IN lines, a pressed key connects one OUT line to one
//! IN line, and ON is wired to IN bit 15 whatever OUT holds. The 49G uses 8
//! OUT lines (#001-#080) and 8 IN lines (#01-#80), with a different layout.
//! The keys are the shared [`Key`] set; the softkeys F1-F6 are `Key::A` to
//! `Key::F` (their alpha letters).

use super::keyboard::{Key, KeyPos};

/// Position of `key` on the 49G keyboard, or `None` for a 48-only key.
pub(crate) fn position(key: Key) -> Option<KeyPos> {
    let (out, mask) = match key {
        Key::Alpha => (7, 0x08),
        Key::LeftShift => (7, 0x04),
        Key::RightShift => (7, 0x02),
        Key::Up => (6, 0x08),
        Key::Left => (6, 0x04),
        Key::Down => (6, 0x02),
        Key::Right => (6, 0x01),
        Key::Apps => (5, 0x80),
        Key::F => (5, 0x20),
        Key::E => (5, 0x10),
        Key::D => (5, 0x08),
        Key::C => (5, 0x04),
        Key::B => (5, 0x02),
        Key::A => (5, 0x01),
        Key::Mode => (4, 0x80),
        Key::Hist => (4, 0x40),
        Key::Power => (4, 0x20),
        Key::Eex => (4, 0x10),
        Key::Tool => (3, 0x80),
        Key::Cat => (3, 0x40),
        Key::Sqrt => (3, 0x20),
        Key::Neg => (3, 0x10),
        Key::Seven => (3, 0x08),
        Key::Four => (3, 0x04),
        Key::One => (3, 0x02),
        Key::Zero => (3, 0x01),
        Key::Var => (2, 0x80),
        Key::Eqw => (2, 0x40),
        Key::Sin => (2, 0x20),
        Key::X => (2, 0x10),
        Key::Eight => (2, 0x08),
        Key::Five => (2, 0x04),
        Key::Two => (2, 0x02),
        Key::Point => (2, 0x01),
        Key::Sto => (1, 0x80),
        Key::Symb => (1, 0x40),
        Key::Cos => (1, 0x20),
        Key::Inv => (1, 0x10),
        Key::Nine => (1, 0x08),
        Key::Six => (1, 0x04),
        Key::Three => (1, 0x02),
        Key::Space => (1, 0x01),
        Key::Nxt => (0, 0x80),
        Key::Backspace => (0, 0x40),
        Key::Tan => (0, 0x20),
        Key::Divide => (0, 0x10),
        Key::Multiply => (0, 0x08),
        Key::Minus => (0, 0x04),
        Key::Plus => (0, 0x02),
        Key::Enter => (0, 0x01),
        Key::On => return Some(KeyPos::On),
        Key::Prg
        | Key::Cst
        | Key::Eval
        | Key::Del
        | Key::Mth
        | Key::Quote
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

#[cfg(test)]
mod tests {
    use super::super::keyboard::{Keyboard, Layout};
    use super::*;

    /// The 49G matrix from wiki: hardware/keyboard ("HP49G matrix", after
    /// Sylvester 1.1 and the tutorial p. 136-137), cell by cell as (OUT
    /// mask, IN mask), written out independently of `Key::matrix`. ON is
    /// IN #8000 for any OUT, so its OUT mask here is 0.
    const WIKI_MATRIX: [(Key, u16, u16); 51] = [
        (Key::Alpha, 0x080, 0x08),
        (Key::LeftShift, 0x080, 0x04),
        (Key::RightShift, 0x080, 0x02),
        (Key::Up, 0x040, 0x08),
        (Key::Left, 0x040, 0x04),
        (Key::Down, 0x040, 0x02),
        (Key::Right, 0x040, 0x01),
        (Key::Apps, 0x020, 0x80),
        (Key::F, 0x020, 0x20),
        (Key::E, 0x020, 0x10),
        (Key::D, 0x020, 0x08),
        (Key::C, 0x020, 0x04),
        (Key::B, 0x020, 0x02),
        (Key::A, 0x020, 0x01),
        (Key::Mode, 0x010, 0x80),
        (Key::Hist, 0x010, 0x40),
        (Key::Power, 0x010, 0x20),
        (Key::Eex, 0x010, 0x10),
        (Key::Tool, 0x008, 0x80),
        (Key::Cat, 0x008, 0x40),
        (Key::Sqrt, 0x008, 0x20),
        (Key::Neg, 0x008, 0x10),
        (Key::Seven, 0x008, 0x08),
        (Key::Four, 0x008, 0x04),
        (Key::One, 0x008, 0x02),
        (Key::Zero, 0x008, 0x01),
        (Key::Var, 0x004, 0x80),
        (Key::Eqw, 0x004, 0x40),
        (Key::Sin, 0x004, 0x20),
        (Key::X, 0x004, 0x10),
        (Key::Eight, 0x004, 0x08),
        (Key::Five, 0x004, 0x04),
        (Key::Two, 0x004, 0x02),
        (Key::Point, 0x004, 0x01),
        (Key::Sto, 0x002, 0x80),
        (Key::Symb, 0x002, 0x40),
        (Key::Cos, 0x002, 0x20),
        (Key::Inv, 0x002, 0x10),
        (Key::Nine, 0x002, 0x08),
        (Key::Six, 0x002, 0x04),
        (Key::Three, 0x002, 0x02),
        (Key::Space, 0x002, 0x01),
        (Key::Nxt, 0x001, 0x80),
        (Key::Backspace, 0x001, 0x40),
        (Key::Tan, 0x001, 0x20),
        (Key::Divide, 0x001, 0x10),
        (Key::Multiply, 0x001, 0x08),
        (Key::Minus, 0x001, 0x04),
        (Key::Plus, 0x001, 0x02),
        (Key::Enter, 0x001, 0x01),
        (Key::On, 0x000, 0x8000),
    ];

    #[test]
    fn every_key_matches_the_wiki_matrix() {
        let mut listed: Vec<Key> = WIKI_MATRIX.iter().map(|e| e.0).collect();
        let mut on_49: Vec<Key> = Key::on_layout(Layout::Hp49).collect();
        listed.sort_by_key(|k| k.name());
        on_49.sort_by_key(|k| k.name());
        assert_eq!(listed, on_49, "table covers exactly the 49G keys");
        for (key, out, inp) in WIKI_MATRIX {
            let mut kb = Keyboard::with_layout(Layout::Hp49);
            kb.press(key);
            assert!(kb.is_pressed(key));
            assert_eq!(kb.read_in(out), inp, "{key:?} on its own row");
            for bit in 0..12 {
                let other = 1u16 << bit;
                if other == out {
                    continue;
                }
                let want = if key == Key::On { 0x8000 } else { 0 };
                assert_eq!(kb.read_in(other), want, "{key:?} with OUT {other:#05X}");
            }
            // All 8 rows at once; OUT bit 8 and the buzzer bit change
            // nothing.
            assert_eq!(kb.read_in(0x0FF), inp);
            assert_eq!(kb.read_in(0x9FF), inp);
            kb.release(key);
            assert_eq!(kb.read_in(0xFFF), 0);
        }
    }

    #[test]
    fn matrix_positions_unique_and_full() {
        let positions: std::collections::HashSet<_> =
            Key::ALL.iter().filter_map(|k| position(*k)).collect();
        assert_eq!(positions.len(), 51, "50 matrix cells and ON");
        let mut kb = Keyboard::with_layout(Layout::Hp49);
        for k in Key::ALL {
            kb.press(k);
        }
        // Every IN line is used; 48-only keys added nothing.
        assert_eq!(kb.read_in(0x1FF), 0x80FF);
        assert!(!kb.is_pressed(Key::Prg));
        kb.release_all();
        assert_eq!(kb.read_in(0x0FF), 0);
        assert_eq!(kb.layout(), Layout::Hp49);
    }

    #[test]
    fn softkey_aliases() {
        let mut kb = Keyboard::with_layout(Layout::Hp49);
        let f1 = Key::from_name("f1").unwrap();
        kb.press(f1);
        assert_eq!(kb.read_in(0x020), 0x01);
    }
}
