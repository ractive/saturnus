//! HP 19BII keyboard matrix (wiki: hardware/hp19bii "Keyboard", from the
//! KML 2.0 OutIn table and the Owner's Manual's key figures).
//!
//! The 28S's matrix (see [`super::keyboard28`]) with the 19BII's keys: OUT
//! bits 1-7, IN bits 0-11, ON on IN bit 15. The letter half (left) takes
//! IN #100 #800 #040 #020 #001 #400 column by column, as the 28S's; its
//! bottom row is SPACE (two columns wide), INS, DEL, ◀ and ▶. The right
//! half takes IN #002 #080 #200 #004 #008 #010; its top row is the six
//! menu keys `A`-`F`, INPUT is two columns wide, ON bottom left.

use super::keyboard::{Key, KeyPos};

/// Position of `key` on the 19BII keyboard, or `None` for a key it lacks.
pub(crate) fn position(key: Key) -> Option<KeyPos> {
    let (out, mask) = match key {
        Key::LetterA => (6, 0x100),
        Key::LetterB => (6, 0x800),
        Key::LetterC => (6, 0x040),
        Key::LetterD => (6, 0x020),
        Key::LetterE => (6, 0x001),
        Key::LetterF => (6, 0x400),
        Key::LetterG => (5, 0x100),
        Key::LetterH => (5, 0x800),
        Key::LetterI => (5, 0x040),
        Key::LetterJ => (5, 0x020),
        Key::LetterK => (5, 0x001),
        Key::LetterL => (5, 0x400),
        Key::LetterM => (4, 0x100),
        Key::LetterN => (4, 0x800),
        Key::LetterO => (4, 0x040),
        Key::LetterP => (4, 0x020),
        Key::LetterQ => (4, 0x001),
        Key::LetterR => (4, 0x400),
        Key::LetterS => (3, 0x100),
        Key::LetterT => (3, 0x800),
        Key::LetterU => (3, 0x040),
        Key::LetterV => (3, 0x020),
        Key::LetterW => (3, 0x001),
        Key::LetterX => (3, 0x400),
        Key::LetterY => (2, 0x100),
        Key::LetterZ => (2, 0x800),
        Key::Question => (2, 0x040),
        Key::Dollar => (2, 0x020),
        Key::Hash => (2, 0x001),
        Key::Colon => (2, 0x400),
        Key::Space => (1, 0x800),
        Key::Ins => (1, 0x040),
        Key::Del => (1, 0x020),
        Key::Left => (1, 0x001),
        Key::Right => (1, 0x400),
        Key::A => (7, 0x002),
        Key::B => (7, 0x080),
        Key::C => (7, 0x200),
        Key::D => (7, 0x004),
        Key::E => (7, 0x008),
        Key::F => (7, 0x010),
        Key::Shift => (6, 0x002),
        Key::Sto => (6, 0x080),
        Key::Rcl => (6, 0x200),
        Key::Dsp => (6, 0x004),
        Key::Prt => (6, 0x008),
        Key::Exit => (6, 0x010),
        Key::Input => (5, 0x080),
        Key::Neg => (5, 0x200),
        Key::LParen => (5, 0x004),
        Key::RParen => (5, 0x008),
        Key::Backspace => (5, 0x010),
        Key::Up => (4, 0x002),
        Key::Seven => (4, 0x080),
        Key::Eight => (4, 0x200),
        Key::Nine => (4, 0x004),
        Key::Divide => (4, 0x008),
        Key::Down => (3, 0x002),
        Key::Four => (3, 0x080),
        Key::Five => (3, 0x200),
        Key::Six => (3, 0x004),
        Key::Multiply => (3, 0x008),
        Key::Percent => (2, 0x002),
        Key::One => (2, 0x080),
        Key::Two => (2, 0x200),
        Key::Three => (2, 0x004),
        Key::Minus => (2, 0x008),
        Key::Zero => (1, 0x080),
        Key::Point => (1, 0x200),
        Key::Equals => (1, 0x004),
        Key::Plus => (1, 0x008),
        Key::On => return Some(KeyPos::On),
        _ => return None,
    };
    Some(KeyPos::Matrix { out, mask })
}

#[cfg(test)]
mod tests {
    use super::super::keyboard::{Keyboard, Layout};
    use super::*;

    /// The KML 2.0 "OutIn Codes HP19BII" entries as (key, OUT bit, IN
    /// value), written out independently of `position`. INPUT is 5 128
    /// (the table prints 6 128, the code of STO; the ROM answers 5 128,
    /// wiki: hardware/hp19bii "Keyboard").
    const OUT_IN_CODES: [(Key, u16, u16); 71] = [
        (Key::LetterA, 6, 256),
        (Key::LetterB, 6, 2048),
        (Key::LetterC, 6, 64),
        (Key::LetterD, 6, 32),
        (Key::LetterE, 6, 1),
        (Key::LetterF, 6, 1024),
        (Key::LetterG, 5, 256),
        (Key::LetterH, 5, 2048),
        (Key::LetterI, 5, 64),
        (Key::LetterJ, 5, 32),
        (Key::LetterK, 5, 1),
        (Key::LetterL, 5, 1024),
        (Key::LetterM, 4, 256),
        (Key::LetterN, 4, 2048),
        (Key::LetterO, 4, 64),
        (Key::LetterP, 4, 32),
        (Key::LetterQ, 4, 1),
        (Key::LetterR, 4, 1024),
        (Key::LetterS, 3, 256),
        (Key::LetterT, 3, 2048),
        (Key::LetterU, 3, 64),
        (Key::LetterV, 3, 32),
        (Key::LetterW, 3, 1),
        (Key::LetterX, 3, 1024),
        (Key::LetterY, 2, 256),
        (Key::LetterZ, 2, 2048),
        (Key::Question, 2, 64),
        (Key::Dollar, 2, 32),
        (Key::Hash, 2, 1),
        (Key::Colon, 2, 1024),
        (Key::Space, 1, 2048),
        (Key::Ins, 1, 64),
        (Key::Del, 1, 32),
        (Key::Left, 1, 1),
        (Key::Right, 1, 1024),
        (Key::A, 7, 2),
        (Key::B, 7, 128),
        (Key::C, 7, 512),
        (Key::D, 7, 4),
        (Key::E, 7, 8),
        (Key::F, 7, 16),
        (Key::Shift, 6, 2),
        (Key::Sto, 6, 128),
        (Key::Rcl, 6, 512),
        (Key::Dsp, 6, 4),
        (Key::Prt, 6, 8),
        (Key::Exit, 6, 16),
        (Key::Input, 5, 128),
        (Key::Neg, 5, 512),
        (Key::LParen, 5, 4),
        (Key::RParen, 5, 8),
        (Key::Backspace, 5, 16),
        (Key::Up, 4, 2),
        (Key::Seven, 4, 128),
        (Key::Eight, 4, 512),
        (Key::Nine, 4, 4),
        (Key::Divide, 4, 8),
        (Key::Down, 3, 2),
        (Key::Four, 3, 128),
        (Key::Five, 3, 512),
        (Key::Six, 3, 4),
        (Key::Multiply, 3, 8),
        (Key::Percent, 2, 2),
        (Key::One, 2, 128),
        (Key::Two, 2, 512),
        (Key::Three, 2, 4),
        (Key::Minus, 2, 8),
        (Key::Zero, 1, 128),
        (Key::Point, 1, 512),
        (Key::Equals, 1, 4),
        (Key::Plus, 1, 8),
    ];

    #[test]
    fn every_key_matches_the_out_in_codes() {
        let mut listed: Vec<Key> = OUT_IN_CODES.iter().map(|e| e.0).collect();
        listed.push(Key::On);
        let mut on_19: Vec<Key> = Key::on_layout(Layout::Hp19).collect();
        listed.sort_by_key(|k| k.name());
        on_19.sort_by_key(|k| k.name());
        assert_eq!(listed, on_19, "table covers exactly the 19BII keys");
        for (key, out_bit, inp) in OUT_IN_CODES {
            let mut kb = Keyboard::with_layout(Layout::Hp19);
            assert!(kb.press(key));
            assert_eq!(kb.read_in(1 << out_bit), inp, "{key:?}");
            assert_eq!(
                kb.read_in(0xFF & !(1 << out_bit)),
                0,
                "{key:?} only on its OUT"
            );
        }
        let mut kb = Keyboard::with_layout(Layout::Hp19);
        assert!(kb.press(Key::On));
        assert_eq!(kb.read_in(0), 0x8000);
    }

    #[test]
    fn rows_use_twelve_in_lines() {
        let mut kb = Keyboard::with_layout(Layout::Hp19);
        for k in Key::on_layout(Layout::Hp19) {
            kb.press(k);
        }
        assert_eq!(kb.read_in(0xFE), 0x8FFF);
        assert_eq!(Layout::Hp19.in_mask(), 0xFFF);
        assert_eq!(
            Key::on_layout(Layout::Hp19).count(),
            72,
            "71 matrix keys and ON"
        );
        assert!(Key::Exit.position(Layout::Hp19) != Key::On.position(Layout::Hp19));
    }
}
