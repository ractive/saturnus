//! HP 42S keyboard matrix (wiki: hardware/hp42s "Keyboard").
//!
//! Same scanning model as the 48 (see [`super::keyboard`]): the CPU drives
//! OUT lines and reads IN lines. The 42S uses 6 OUT lines (#01-#20) and 7
//! IN lines (#01-#40); EXIT is the ON key on IN bit 15. 36 matrix keys and
//! EXIT make the 37 keys. The top row (Σ+ to XEQ) doubles as the six menu
//! keys; it keeps its printed labels here, so the 42S has no `Key::A` to
//! `Key::F`.

use super::keyboard::{Key, KeyPos};

/// Position of `key` on the 42S keyboard, or `None` for a key it lacks.
pub(crate) fn position(key: Key) -> Option<KeyPos> {
    let (out, mask) = match key {
        // Row 1: Σ+ 1/x √x LOG LN XEQ (IN #40).
        Key::SigmaPlus => (5, 0x40),
        Key::Inv => (4, 0x40),
        Key::Sqrt => (3, 0x40),
        Key::Log => (2, 0x40),
        Key::Ln => (1, 0x40),
        Key::Xeq => (0, 0x40),
        // Row 2: STO RCL R↓ SIN COS TAN (IN #20).
        Key::Sto => (5, 0x20),
        Key::Rcl => (4, 0x20),
        Key::RollDown => (3, 0x20),
        Key::Sin => (2, 0x20),
        Key::Cos => (1, 0x20),
        Key::Tan => (0, 0x20),
        // Row 3: ENTER x≷y +/- E ← (IN #10).
        Key::Enter => (4, 0x10),
        Key::Swap => (3, 0x10),
        Key::Neg => (2, 0x10),
        Key::Eex => (1, 0x10),
        Key::Backspace => (0, 0x10),
        // Row 4: ▲ 7 8 9 ÷ (IN #08).
        Key::Up => (5, 0x08),
        Key::Seven => (3, 0x08),
        Key::Eight => (2, 0x08),
        Key::Nine => (1, 0x08),
        Key::Divide => (0, 0x08),
        // Row 5: ▼ 4 5 6 × (IN #04).
        Key::Down => (5, 0x04),
        Key::Four => (3, 0x04),
        Key::Five => (2, 0x04),
        Key::Six => (1, 0x04),
        Key::Multiply => (0, 0x04),
        // Row 6: shift 1 2 3 - (IN #02).
        Key::Shift => (5, 0x02),
        Key::One => (3, 0x02),
        Key::Two => (2, 0x02),
        Key::Three => (1, 0x02),
        Key::Minus => (0, 0x02),
        // Row 7: EXIT (ON) 0 . R/S + (IN #01).
        Key::Zero => (3, 0x01),
        Key::Point => (2, 0x01),
        Key::Rs => (1, 0x01),
        Key::Plus => (0, 0x01),
        Key::On => return Some(KeyPos::On),
        _ => return None,
    };
    Some(KeyPos::Matrix { out, mask })
}

#[cfg(test)]
mod tests {
    use super::super::keyboard::{Keyboard, Layout};
    use super::*;

    /// The 42S matrix as (key, OUT bit, IN value), written out
    /// independently of `position` (wiki: hardware/hp42s "Keyboard"). EXIT
    /// is IN bit 15 (32768), the ON line, for any OUT.
    const OUT_IN_CODES: [(Key, u16, u16); 37] = [
        (Key::SigmaPlus, 5, 64),
        (Key::Inv, 4, 64),
        (Key::Sqrt, 3, 64),
        (Key::Log, 2, 64),
        (Key::Ln, 1, 64),
        (Key::Xeq, 0, 64),
        (Key::Sto, 5, 32),
        (Key::Rcl, 4, 32),
        (Key::RollDown, 3, 32),
        (Key::Sin, 2, 32),
        (Key::Cos, 1, 32),
        (Key::Tan, 0, 32),
        (Key::Enter, 4, 16),
        (Key::Swap, 3, 16),
        (Key::Neg, 2, 16),
        (Key::Eex, 1, 16),
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
        (Key::Rs, 1, 1),
        (Key::Plus, 0, 1),
    ];

    #[test]
    fn every_key_matches_the_out_in_codes() {
        let mut listed: Vec<Key> = OUT_IN_CODES.iter().map(|e| e.0).collect();
        let mut on_42: Vec<Key> = Key::on_layout(Layout::Hp42).collect();
        listed.sort_by_key(|k| k.name());
        on_42.sort_by_key(|k| k.name());
        assert_eq!(listed, on_42, "table covers exactly the 42S keys");
        for (key, out_bit, inp) in OUT_IN_CODES {
            let mut kb = Keyboard::with_layout(Layout::Hp42);
            kb.press(key);
            assert!(kb.is_pressed(key));
            let out = 1u16 << out_bit;
            assert_eq!(kb.read_in(out), inp, "{key:?} on its own row");
            for bit in 0..12 {
                let other = 1u16 << bit;
                if other == out {
                    continue;
                }
                let want = if key == Key::On { 0x8000 } else { 0 };
                assert_eq!(kb.read_in(other), want, "{key:?} with OUT {other:#05X}");
            }
            assert_eq!(kb.read_in(0x3F), inp);
            kb.release(key);
            assert_eq!(kb.read_in(0xFFF), 0);
        }
    }

    #[test]
    fn matrix_positions_unique() {
        let positions: std::collections::HashSet<_> =
            Key::ALL.iter().filter_map(|k| position(*k)).collect();
        assert_eq!(positions.len(), 37, "36 matrix cells and EXIT");
        assert_eq!(Key::from_name("exit"), Some(Key::On));
        assert!(position(Key::A).is_none(), "menu keys keep their labels");
    }
}
