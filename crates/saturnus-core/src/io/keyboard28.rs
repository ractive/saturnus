//! HP 28S keyboard matrix (wiki: hardware/hp28s "Keyboard", from the KML
//! 2.0 OutIn table and the Owner's Manual's key figures).
//!
//! The 48's scanning model (see [`super::keyboard`]) on a wider matrix: the
//! CPU drives OUT bits 1-7 and reads IN bits 0-11; ON is IN bit 15 for any
//! OUT. The left (letter) half takes IN #001, #020, #040, #100, #400 and
//! #800; the right half IN #002, #004, #008, #010, #080 and #200. The six
//! menu keys are the right half's top row (`A`-`F`, the softkey names of
//! every model); with no menu shown they act as the cursor menu printed
//! above them (INS DEL ▲ ▼ ◀ ▶). The letters of the left half are
//! `LetterA`-`LetterZ`.

use super::keyboard::{Key, KeyPos};

/// Position of `key` on the 28S keyboard, or `None` for a key it lacks.
pub(crate) fn position(key: Key) -> Option<KeyPos> {
    let (out, mask) = match key {
        // Left half: rows of six from the top on OUT 6 down to 1, columns
        // left to right on IN #100 #800 #040 #020 #001 #400; SPACE is two
        // columns wide.
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
        Key::Hash => (2, 0x040),
        Key::LBrace => (2, 0x020),
        Key::LBracket => (2, 0x001),
        Key::LParen => (2, 0x400),
        Key::Space => (1, 0x800),
        Key::Program => (1, 0x040),
        Key::Equals => (1, 0x020),
        Key::Lc => (1, 0x001),
        Key::Alpha => (1, 0x400),
        // Right half: rows from the top on OUT 7 down to 1, columns on IN
        // #002 #080 #200 #004 #008 #010; ENTER is two columns wide, ON
        // bottom left.
        Key::A => (7, 0x002),
        Key::B => (7, 0x080),
        Key::C => (7, 0x200),
        Key::D => (7, 0x004),
        Key::E => (7, 0x008),
        Key::F => (7, 0x010),
        Key::Shift => (6, 0x002),
        Key::Cursor => (6, 0x080),
        Key::Trig => (6, 0x200),
        Key::Solv => (6, 0x004),
        Key::User => (6, 0x008),
        Key::Nxt => (6, 0x010),
        Key::Enter => (5, 0x080),
        Key::Neg => (5, 0x200),
        Key::Eex => (5, 0x004),
        Key::Drop => (5, 0x008),
        Key::Backspace => (5, 0x010),
        Key::Quote => (4, 0x002),
        Key::Seven => (4, 0x080),
        Key::Eight => (4, 0x200),
        Key::Nine => (4, 0x004),
        Key::Divide => (4, 0x008),
        Key::Sto => (3, 0x002),
        Key::Four => (3, 0x080),
        Key::Five => (3, 0x200),
        Key::Six => (3, 0x004),
        Key::Multiply => (3, 0x008),
        Key::Eval => (2, 0x002),
        Key::One => (2, 0x080),
        Key::Two => (2, 0x200),
        Key::Three => (2, 0x004),
        Key::Minus => (2, 0x008),
        Key::Zero => (1, 0x080),
        Key::Point => (1, 0x200),
        Key::Comma => (1, 0x004),
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

    /// The KML 2.0 "OutIn Codes HP28S" entries as (key, OUT bit, IN
    /// value), written out independently of `position`.
    const OUT_IN_CODES: [(Key, u16, u16); 72] = [
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
        (Key::Hash, 2, 64),
        (Key::LBrace, 2, 32),
        (Key::LBracket, 2, 1),
        (Key::LParen, 2, 1024),
        (Key::Space, 1, 2048),
        (Key::Program, 1, 64),
        (Key::Equals, 1, 32),
        (Key::Lc, 1, 1),
        (Key::Alpha, 1, 1024),
        (Key::A, 7, 2),
        (Key::B, 7, 128),
        (Key::C, 7, 512),
        (Key::D, 7, 4),
        (Key::E, 7, 8),
        (Key::F, 7, 16),
        (Key::Shift, 6, 2),
        (Key::Cursor, 6, 128),
        (Key::Trig, 6, 512),
        (Key::Solv, 6, 4),
        (Key::User, 6, 8),
        (Key::Nxt, 6, 16),
        (Key::Enter, 5, 128),
        (Key::Neg, 5, 512),
        (Key::Eex, 5, 4),
        (Key::Drop, 5, 8),
        (Key::Backspace, 5, 16),
        (Key::Quote, 4, 2),
        (Key::Seven, 4, 128),
        (Key::Eight, 4, 512),
        (Key::Nine, 4, 4),
        (Key::Divide, 4, 8),
        (Key::Sto, 3, 2),
        (Key::Four, 3, 128),
        (Key::Five, 3, 512),
        (Key::Six, 3, 4),
        (Key::Multiply, 3, 8),
        (Key::Eval, 2, 2),
        (Key::One, 2, 128),
        (Key::Two, 2, 512),
        (Key::Three, 2, 4),
        (Key::Minus, 2, 8),
        (Key::Zero, 1, 128),
        (Key::Point, 1, 512),
        (Key::Comma, 1, 4),
        (Key::Plus, 1, 8),
        (Key::On, 0, 32768),
    ];

    #[test]
    fn every_key_matches_the_out_in_codes() {
        let mut listed: Vec<Key> = OUT_IN_CODES.iter().map(|e| e.0).collect();
        let mut on_28: Vec<Key> = Key::on_layout(Layout::Hp28).collect();
        listed.sort_by_key(|k| k.name());
        on_28.sort_by_key(|k| k.name());
        assert_eq!(listed, on_28, "table covers exactly the 28S keys");
        for (key, out_bit, inp) in OUT_IN_CODES {
            let mut kb = Keyboard::with_layout(Layout::Hp28);
            assert!(kb.press(key));
            if key == Key::On {
                assert_eq!(kb.read_in(0), inp);
                continue;
            }
            assert_eq!(kb.read_in(1 << out_bit), inp, "{key:?}");
            assert_eq!(
                kb.read_in(0xFF & !(1 << out_bit)),
                0,
                "{key:?} only on its OUT"
            );
        }
    }

    #[test]
    fn rows_use_twelve_in_lines() {
        let mut kb = Keyboard::with_layout(Layout::Hp28);
        for k in Key::on_layout(Layout::Hp28) {
            kb.press(k);
        }
        assert_eq!(kb.read_in(0xFE), 0x8FFF);
        assert_eq!(Layout::Hp28.in_mask(), 0xFFF);
        assert_eq!(
            Key::on_layout(Layout::Hp28).count(),
            72,
            "71 matrix keys and ON"
        );
    }
}
