//! HP 27S and HP 17B/17BII keyboards (wiki: hardware/hp27s and
//! hardware/hp17bii "Keyboard", from the KML 2.0 OutIn tables).
//!
//! Both put their keys on the 42S's 6 OUT x 7 IN matrix (see
//! [`super::keyboard42`]): the top row is the six menu keys (`A`-`F`, as
//! the 48's softkeys), the third row's wide INPUT key takes OUT bit 4, and
//! the bottom-left key sits on the ON line (IN bit 15). The two layouts
//! differ only in the second row: STO RCL % DSP PRT EXIT on the 17B and
//! 17BII, STO RCL SIN COS TAN EXIT on the 27S.

use super::keyboard::{Key, KeyPos, Layout};

/// Position of `key` on the 27S (`Layout::Hp27`) or 17B/17BII
/// (`Layout::Hp17`) keyboard, or `None` for a key it lacks.
pub(crate) fn position(key: Key, layout: Layout) -> Option<KeyPos> {
    let (out, mask) = match (key, layout) {
        // Row 1: the six menu keys (IN #40).
        (Key::A, _) => (5, 0x40),
        (Key::B, _) => (4, 0x40),
        (Key::C, _) => (3, 0x40),
        (Key::D, _) => (2, 0x40),
        (Key::E, _) => (1, 0x40),
        (Key::F, _) => (0, 0x40),
        // Row 2 (IN #20): STO RCL, then % DSP PRT or SIN COS TAN, EXIT.
        (Key::Sto, _) => (5, 0x20),
        (Key::Rcl, _) => (4, 0x20),
        (Key::Percent, Layout::Hp17) | (Key::Sin, Layout::Hp27) => (3, 0x20),
        (Key::Dsp, Layout::Hp17) | (Key::Cos, Layout::Hp27) => (2, 0x20),
        (Key::Prt, Layout::Hp17) | (Key::Tan, Layout::Hp27) => (1, 0x20),
        (Key::Exit, _) => (0, 0x20),
        // Row 3: INPUT +/- ( ) ← (IN #10).
        (Key::Input, _) => (4, 0x10),
        (Key::Neg, _) => (3, 0x10),
        (Key::LParen, _) => (2, 0x10),
        (Key::RParen, _) => (1, 0x10),
        (Key::Backspace, _) => (0, 0x10),
        // Row 4: ▲ 7 8 9 ÷ (IN #08).
        (Key::Up, _) => (5, 0x08),
        (Key::Seven, _) => (3, 0x08),
        (Key::Eight, _) => (2, 0x08),
        (Key::Nine, _) => (1, 0x08),
        (Key::Divide, _) => (0, 0x08),
        // Row 5: ▼ 4 5 6 × (IN #04).
        (Key::Down, _) => (5, 0x04),
        (Key::Four, _) => (3, 0x04),
        (Key::Five, _) => (2, 0x04),
        (Key::Six, _) => (1, 0x04),
        (Key::Multiply, _) => (0, 0x04),
        // Row 6: shift 1 2 3 − (IN #02).
        (Key::Shift, _) => (5, 0x02),
        (Key::One, _) => (3, 0x02),
        (Key::Two, _) => (2, 0x02),
        (Key::Three, _) => (1, 0x02),
        (Key::Minus, _) => (0, 0x02),
        // Row 7: the ON line's key, 0 . = + (IN #01).
        (Key::Zero, _) => (3, 0x01),
        (Key::Point, _) => (2, 0x01),
        (Key::Equals, _) => (1, 0x01),
        (Key::Plus, _) => (0, 0x01),
        (Key::On, _) => return Some(KeyPos::On),
        _ => return None,
    };
    Some(KeyPos::Matrix { out, mask })
}

#[cfg(test)]
mod tests {
    use super::super::keyboard::Keyboard;
    use super::*;

    /// The row-2 keys at OUT bits 5..0 per layout, and the rows every
    /// Pioneer layout shares, as (key, OUT bit, IN value), written out from
    /// the wiki tables independently of `position`.
    const ROW2_17: [Key; 6] = [
        Key::Sto,
        Key::Rcl,
        Key::Percent,
        Key::Dsp,
        Key::Prt,
        Key::Exit,
    ];
    const ROW2_27: [Key; 6] = [Key::Sto, Key::Rcl, Key::Sin, Key::Cos, Key::Tan, Key::Exit];
    const SHARED: [(Key, u16, u16); 31] = [
        (Key::A, 5, 64),
        (Key::B, 4, 64),
        (Key::C, 3, 64),
        (Key::D, 2, 64),
        (Key::E, 1, 64),
        (Key::F, 0, 64),
        (Key::Input, 4, 16),
        (Key::Neg, 3, 16),
        (Key::LParen, 2, 16),
        (Key::RParen, 1, 16),
        (Key::Backspace, 0, 16),
        (Key::Up, 5, 8),
        (Key::Seven, 3, 8),
        (Key::Eight, 2, 8),
        (Key::Nine, 1, 8),
        (Key::Divide, 0, 8),
        (Key::Down, 5, 4),
        (Key::Four, 3, 4),
        (Key::Five, 2, 4),
        (Key::Six, 1, 4),
        (Key::Multiply, 0, 4),
        (Key::Shift, 5, 2),
        (Key::One, 3, 2),
        (Key::Two, 2, 2),
        (Key::Three, 1, 2),
        (Key::Minus, 0, 2),
        (Key::On, 0, 32768),
        (Key::Zero, 3, 1),
        (Key::Point, 2, 1),
        (Key::Equals, 1, 1),
        (Key::Plus, 0, 1),
    ];

    fn table(row2: [Key; 6]) -> Vec<(Key, u16, u16)> {
        let mut t: Vec<_> = SHARED.to_vec();
        t.extend(
            row2.iter()
                .zip([5u16, 4, 3, 2, 1, 0])
                .map(|(&k, o)| (k, o, 32)),
        );
        t
    }

    #[test]
    fn every_key_matches_the_out_in_codes() {
        for (layout, row2) in [(Layout::Hp17, ROW2_17), (Layout::Hp27, ROW2_27)] {
            let codes = table(row2);
            let mut listed: Vec<Key> = codes.iter().map(|e| e.0).collect();
            let mut on: Vec<Key> = Key::on_layout(layout).collect();
            listed.sort_by_key(|k| k.name());
            on.sort_by_key(|k| k.name());
            assert_eq!(listed, on, "{layout:?}: table covers exactly its keys");
            assert_eq!(on.len(), 37);
            for (key, out_bit, inp) in codes {
                let mut kb = Keyboard::with_layout(layout);
                kb.press(key);
                assert_eq!(kb.read_in(1 << out_bit), inp, "{layout:?} {key:?}");
                let want_other = if key == Key::On { 0x8000 } else { 0 };
                let other = if out_bit == 0 { 2 } else { 1 };
                assert_eq!(kb.read_in(other), want_other, "{key:?} off its row");
                kb.release(key);
                assert_eq!(kb.read_in(0xFFF), 0);
            }
        }
    }

    #[test]
    fn the_layouts_differ_in_row_two_only() {
        assert!(position(Key::Percent, Layout::Hp27).is_none());
        assert!(position(Key::Sin, Layout::Hp17).is_none());
        assert!(
            position(Key::Enter, Layout::Hp17).is_none(),
            "INPUT, not ENTER"
        );
        assert_eq!(Key::from_name("exit"), Some(Key::Exit));
        assert!(matches!(
            position(Key::Exit, Layout::Hp17),
            Some(KeyPos::Matrix { out: 0, mask: 0x20 })
        ));
    }
}
