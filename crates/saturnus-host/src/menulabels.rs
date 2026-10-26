//! What the six menu keys show on the 42S, 27S, 17BII, 17B, 19BII and
//! 28S, read from RAM and the LCD without pressing a key (the `menu`
//! event; the 48SX, 48GX and 49G read theirs in
//! [`saturnus_objects::softkeys`]).
//!
//! - **The Pioneers and the 19BII** keep the six label texts in a buffer
//!   of six entries of 11 nibbles: a length nibble (0 to 5), then up to
//!   five characters, two nibbles each, low nibble first (wiki:
//!   hardware/hp42s, hp27s, hp17bii, hp19bii, "Menu labels in RAM"). The
//!   buffer is not cleared when the menu goes away, so a label counts only
//!   while the LCD draws its box; on the 42S also only while the menu
//!   depth (#50201) is not 0.
//! - **The 28S** keeps the number of the menu shown and the first label
//!   shown (wiki: hardware/hp28s, "Menu in RAM"); the labels of each menu
//!   come from a table generated from the ROM on the emulator
//!   (`menulabels/menus-28s.json`, `saturnus-refgen menus28`). The USER
//!   menu's variables are not read.
//!
//! A label holding a character code not known here reads as no label.
//! The buffer holds a label whole where the box cuts it short (`TRANS`
//! on the 42S shows as `TRAN`), and with spaces the key reference
//! leaves out (`C X,Y` for its `CX,Y`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use saturnus_core::{Lcd, Machine, Model};
use saturnus_objects::softkeys::{PAGE, ShownKey};
use serde::Deserialize;

/// Nibbles of one label entry: the length and five characters.
const ENTRY: u32 = 11;
/// The longest label.
const MAX_CHARS: u8 = 5;
/// The 42S's menu depth: 0 with no menu (wiki: hardware/hp42s).
pub const HP42S_MENU_DEPTH: u32 = 0x50201;
/// The 28S's menu number and first label shown, two nibbles each, low
/// first (wiki: hardware/hp28s).
pub const HP28S_MENU_NUMBER: u32 = 0xC0151;
pub const HP28S_FIRST_LABEL: u32 = 0xC0153;

/// The models whose menu keys this module reads.
pub fn reads(model: Model) -> bool {
    buffer(model).is_some() || model == Model::Hp28s
}

/// Where `model` keeps its label buffer (wiki: "Menu labels in RAM").
pub fn buffer(model: Model) -> Option<u32> {
    match model {
        Model::Hp42s => Some(0x5033C),
        Model::Hp27s => Some(0x503DF),
        Model::Hp17bii => Some(0x503D2),
        Model::Hp17b => Some(0x503D1),
        Model::Hp19bii => Some(0xC4376),
        _ => None,
    }
}

/// A character of a label as `model` codes it, `None` for a code not
/// known here. Codes 0x20 to 0x5F are ASCII on all of them, and on the
/// 42S up to 0x7E; the others were seen where the screen shows the
/// character (wiki: "Menu labels in RAM"): on the 42S Σ 0x05, ≤ 0x09,
/// ≥ 0x0B, ≠ 0x0C, → 0x0F; Σ 0x76 on the 27S, 17BII and 17B; ∡ 0x77 on
/// the 27S, 0x78 on the 19BII; ° 0x65 on the 19BII.
pub fn char_of(model: Model, code: u8) -> Option<char> {
    use Model::*;
    match (model, code) {
        (_, 0x20..=0x5F) | (Hp42s, 0x60..=0x7E) => Some(char::from(code)),
        (Hp42s, 0x05) => Some('Σ'),
        (Hp42s, 0x09) => Some('≤'),
        (Hp42s, 0x0B) => Some('≥'),
        (Hp42s, 0x0C) => Some('≠'),
        (Hp42s, 0x0F) => Some('→'),
        (Hp27s | Hp17bii | Hp17b, 0x76) => Some('Σ'),
        (Hp27s, 0x77) | (Hp19bii, 0x78) => Some('∡'),
        (Hp19bii, 0x65) => Some('°'),
        _ => None,
    }
}

/// The label of one entry (its 11 nibbles), `None` for a blank key (length
/// 0) or a character not known; characters past the length are stale and
/// not read.
pub fn decode(model: Model, entry: &[u8]) -> Option<String> {
    let len = *entry.first()?;
    if len == 0 || len > MAX_CHARS {
        return None;
    }
    (0..usize::from(len))
        .map(|i| {
            let lo = *entry.get(1 + 2 * i)?;
            let hi = *entry.get(2 + 2 * i)?;
            char_of(model, (hi << 4) | (lo & 0xF))
        })
        .collect()
}

/// Where the menu boxes sit: the border rows above and below the letters,
/// each box's pitch and width (a box is dark with light letters, framed
/// by a dark row above and below).
struct Boxes {
    top: usize,
    bottom: usize,
    pitch: usize,
    width: usize,
}

impl Boxes {
    fn of(lcd: &Lcd) -> Option<Boxes> {
        match (lcd.width(), lcd.height()) {
            // The Pioneers: the lower of two 8-row lines.
            (131, 16) => Some(Boxes {
                top: 9,
                bottom: 15,
                pitch: 22,
                width: 21,
            }),
            // The 19BII and the 28S: the fourth line.
            (137, 32) => Some(Boxes {
                top: 25,
                bottom: 31,
                pitch: 23,
                width: 22,
            }),
            _ => None,
        }
    }
}

/// Which of the six menu boxes the LCD draws. A border row counts for a
/// box when it is dark all along the box and light in the gap columns
/// beside it (a line across the display is no box: the 28S's PLOT axis
/// on its last row, a 42S program's line). The row of boxes shows only
/// when some box has both border rows (as refgen's `lcdtext::has_menu`
/// and `web/keys28.js`'s `labelsShown` require); then a box counts with
/// one of them (a character such as the 42S's `"` can reach into the
/// upper one).
pub fn boxes_drawn(lcd: &Lcd) -> [bool; PAGE] {
    let Some(b) = Boxes::of(lcd) else {
        return [false; PAGE];
    };
    let dark = |x: usize, y: usize| {
        lcd.pixels
            .get(y)
            .and_then(|r| r.get(x))
            .copied()
            .unwrap_or(false)
    };
    let border = |i: usize, y: usize| {
        let x0 = i * b.pitch;
        let gap_before = i > 0 && dark(x0 - b.pitch + b.width, y);
        let gap_after = i + 1 < PAGE && dark(x0 + b.width, y);
        (x0..x0 + b.width).all(|x| dark(x, y)) && !gap_before && !gap_after
    };
    let framed = (0..PAGE).any(|i| border(i, b.top) && border(i, b.bottom));
    if !framed {
        return [false; PAGE];
    }
    std::array::from_fn(|i| border(i, b.top) || border(i, b.bottom))
}

/// One key of the 28S's table.
#[derive(Clone, Debug, Deserialize)]
struct TableKey {
    label: String,
    #[serde(default)]
    command: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TableMenu {
    labels: Vec<Option<TableKey>>,
}

#[derive(Clone, Debug, Deserialize)]
struct Table {
    menus: BTreeMap<u32, TableMenu>,
}

/// The 28S's menus by number (`None` if the table does not parse, which
/// its unit test rules out).
fn table28() -> Option<&'static Table> {
    static TABLE: OnceLock<Option<Table>> = OnceLock::new();
    TABLE
        .get_or_init(|| serde_json::from_str(include_str!("menulabels/menus-28s.json")).ok())
        .as_ref()
}

/// The six keys of the 28S's menu `number` on the page that starts at
/// label `first`; `None` for a menu not in the table (USER, the cursor
/// menu, a custom menu) or a page that does not start inside it.
pub fn page28(number: u32, first: usize) -> Option<Vec<Option<ShownKey>>> {
    let menu = table28()?.menus.get(&number)?;
    if !first.is_multiple_of(PAGE) || first >= menu.labels.len().max(1) {
        return None;
    }
    Some(
        (first..first + PAGE)
            .map(|i| {
                menu.labels.get(i)?.as_ref().map(|k| ShownKey {
                    label: k.label.clone(),
                    command: k.command.clone(),
                    ..Default::default()
                })
            })
            .collect(),
    )
}

fn nibbles(machine: &Machine, at: u32, n: u32) -> Vec<u8> {
    (at..at + n).map(|a| machine.peek(a) & 0xF).collect()
}

/// The six menu keys the screen shows now, `None` where none is known
/// (no box drawn, the 42S's menu depth 0, a 28S menu not in the table,
/// another model). A key whose box is not drawn reads as blank. To be
/// called while the ROM waits for a key.
pub fn read(machine: &Machine) -> Option<Vec<Option<ShownKey>>> {
    let model = machine.model();
    if !reads(model) {
        return None;
    }
    if model == Model::Hp42s && machine.peek(HP42S_MENU_DEPTH) & 0xF == 0 {
        return None;
    }
    let drawn = boxes_drawn(&machine.lcd());
    if !drawn.iter().any(|&d| d) {
        return None;
    }
    let keys: Vec<Option<ShownKey>> = if let Some(at) = buffer(model) {
        let buf = nibbles(machine, at, ENTRY * PAGE as u32);
        buf.chunks(ENTRY as usize)
            .map(|e| {
                decode(model, e).map(|label| ShownKey {
                    label,
                    ..Default::default()
                })
            })
            .collect()
    } else {
        let two = |a: u32| {
            let n = nibbles(machine, a, 2);
            usize::from(n[0]) | usize::from(n[1]) << 4
        };
        let number = u32::try_from(two(HP28S_MENU_NUMBER)).ok()?;
        page28(number, two(HP28S_FIRST_LABEL))?
    };
    Some(
        keys.into_iter()
            .zip(drawn)
            .map(|(k, d)| k.filter(|_| d))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An entry for `text` (ASCII), stale characters after it.
    fn entry(text: &str) -> Vec<u8> {
        let mut e = vec![text.len() as u8];
        for b in text.bytes().chain("ZZZZZ".bytes()).take(5) {
            e.push(b & 0xF);
            e.push(b >> 4);
        }
        e
    }

    #[test]
    fn entries_decode_up_to_their_length() {
        assert_eq!(
            decode(Model::Hp17bii, &entry("I%YR")).as_deref(),
            Some("I%YR")
        );
        assert_eq!(decode(Model::Hp17bii, &entry("")), None, "a blank key");
        assert_eq!(
            decode(Model::Hp27s, &entry("AMRT")).as_deref(),
            Some("AMRT")
        );
        // The 42S's → and Σ.
        let mut e = entry("xDEG");
        e[1] = 0xF;
        e[2] = 0;
        assert_eq!(decode(Model::Hp42s, &e).as_deref(), Some("→DEG"));
        e[1] = 0x5;
        assert_eq!(decode(Model::Hp42s, &e).as_deref(), Some("ΣDEG"));
        assert_eq!(decode(Model::Hp17bii, &e), None, "a code not known here");
        // The business models' Σ, ∡ and °, where ASCII has lower case.
        let mut s = entry("vX");
        assert_eq!(decode(Model::Hp17bii, &s).as_deref(), Some("ΣX"));
        assert_eq!(decode(Model::Hp19bii, &s), None);
        s[1] = 0x5;
        s[2] = 0x6;
        assert_eq!(decode(Model::Hp19bii, &s).as_deref(), Some("°X"));
        assert_eq!(decode(Model::Hp42s, &s).as_deref(), Some("eX"));
        let mut long = entry("ABCDE");
        long[0] = 6;
        assert_eq!(
            decode(Model::Hp42s, &long),
            None,
            "no label is longer than 5"
        );
    }

    #[test]
    fn boxes_are_drawn_where_a_border_row_is_dark() {
        let mut lcd = Lcd::blank_sized(131, 16);
        assert_eq!(boxes_drawn(&lcd), [false; PAGE]);
        // One border row alone: no row of boxes.
        for x in 22..43 {
            lcd.pixels[9][x] = true;
        }
        assert_eq!(boxes_drawn(&lcd), [false; PAGE], "no box framed");
        // Box 2 framed: it shows, and so does box 1 with its upper row.
        for x in 44..65 {
            lcd.pixels[9][x] = true;
            lcd.pixels[15][x] = true;
        }
        assert_eq!(boxes_drawn(&lcd), [false, true, true, false, false, false]);
        assert_eq!(
            boxes_drawn(&Lcd::blank()),
            [false; PAGE],
            "the 48's display"
        );
    }

    #[test]
    fn a_line_across_the_display_is_no_box() {
        let mut big = Lcd::blank_sized(137, 32);
        for x in 0..137 {
            big.pixels[31][x] = true;
        }
        assert_eq!(boxes_drawn(&big), [false; PAGE], "a dark last row");
        for x in 0..137 {
            big.pixels[25][x] = true;
        }
        assert_eq!(boxes_drawn(&big), [false; PAGE], "two dark rows, gaps dark");
        // Six framed boxes with light gaps.
        let mut menu = Lcd::blank_sized(137, 32);
        for i in 0..PAGE {
            for x in i * 23..i * 23 + 22 {
                menu.pixels[25][x] = true;
                menu.pixels[31][x] = true;
            }
        }
        assert_eq!(boxes_drawn(&menu), [true; PAGE]);
    }

    #[test]
    fn the_28s_table_gives_pages_of_six() {
        let stack = page28(7, 0).unwrap();
        let labels: Vec<&str> = stack.iter().flatten().map(|k| k.label.as_str()).collect();
        assert_eq!(labels, ["DUP", "OVER", "DUP2", "DROP2", "ROT", "LIST→"]);
        assert_eq!(stack[0].as_ref().unwrap().command.as_deref(), Some("DUP"));
        let second = page28(7, 6).unwrap();
        assert_eq!(second[0].as_ref().unwrap().label, "ROLLD");
        assert_eq!(page28(7, 12), None, "no third page");
        assert_eq!(page28(23, 0), None, "USER is not in the table");
        assert_eq!(page28(7, 3), None, "not a page start");
        // A blank key and a label cut short.
        let branch = page28(14, 12).unwrap();
        assert_eq!(branch[1].as_ref().unwrap().label, "UNTI");
        assert_eq!(
            branch[1].as_ref().unwrap().command.as_deref(),
            Some("UNTIL")
        );
        assert!(page28(14, 0).unwrap()[5].is_none());
        // A mode without a command.
        let mode = page28(16, 6).unwrap();
        assert_eq!(mode[0].as_ref().unwrap().label, "CMD");
        assert_eq!(mode[0].as_ref().unwrap().command, None);
    }

    #[test]
    fn only_these_models() {
        for m in [
            Model::Hp42s,
            Model::Hp27s,
            Model::Hp17bii,
            Model::Hp17b,
            Model::Hp19bii,
            Model::Hp28s,
        ] {
            assert!(reads(m), "{}", m.name());
        }
        for m in [Model::Hp48sx, Model::Hp49g, Model::Hp38g] {
            assert!(!reads(m));
        }
    }
}
