//! HP 17BII and HP 17B skin.
//!
//! Geometry: the Pioneer case measured on the photographs P3 (a 27S) and
//! P4 (this 17BII), see [`super::pioneer`]. Labels: the keyboard figures
//! on PDF page 2 of the HP 17BII Owner's Manual (edition 2, 1991; wiki:
//! sources/hp17bii-om) and of the HP-17B Owner's Manual (edition 1, 1987;
//! wiki: sources/hp17b-om), checked against the photographs. Colours:
//! medians of Wikimedia Commons photographs (wiki:
//! sources/commons-pioneer-photos); nothing of the figures or the photos
//! is stored here.
//!
//! - P4, a 17BII scanned front on (striegel, CC BY 2.0, 2015):
//!   <https://commons.wikimedia.org/wiki/File:HP_17B_II_Business_calculator_(19934267028).jpg>
//! - P1, a 1999 17BII in warm light (striegel, CC BY 2.0, 2016):
//!   <https://commons.wikimedia.org/wiki/File:Hewlett_Packard_17BII_Business_calculator_and_slipcase,_front,_power_on,_showing_end_of_self-test_(27717487764).jpg>
//! - P2, a 17B at an angle with flash (Joe Haupt, CC BY-SA 2.0, 2014):
//!   <https://commons.wikimedia.org/wiki/File:Vintage_Hewlett_Packard_Model_17B_Electronic_Business_Pocket_Calculator,_Made_In_USA,_Circa_1988_-_1990_(15954311920).jpg>
//!
//! **17BII**, measured on P4 (neutral light): the black case (#201f1d),
//! the champagne display surround (#d1b797) with "17B II" in heavy
//! italics on the left and "BUSINESS" in italics on the right, both
//! near-black (cap height 19 units; baselines 186; "17B II" spans 79-173,
//! "BUSINESS" 474-599), the taupe bezel (#93836c), the brown band behind
//! the menu row (#4b3d34), the black keyboard plate (#1a1917) framed by a
//! thin light gold line 9 units inside it, down both sides from the band
//! and along the bottom (x 42 and 632, y 1205), the black caps with white
//! labels, the gold shift key (#d5a356). The scan clips the label ink to
//! near white (#f3e9d5); P1 shows its hue (#f2c892): the gold labels are
//! drawn between the two, #e8c48c. ON and ENTER are printed cream, not
//! gold (P1 #f9e9cb against #f2c892), #f0e8d8.
//!
//! **17B**, from P2 only (too oblique to measure: the geometry is the
//! 17BII's, whose figure it matches): a dark brown case, a chocolate
//! display surround with "17B" in heavy gold italics and "BUSINESS"
//! upright in grey-tan, a gold bezel, a lighter brown band behind the menu
//! row, no gold frame, orange-gold labels, a gold-yellow shift key. The
//! flash darkens the case and plates and brightens the shift key and the
//! bezel, so every 17B colour is P2's median toned: case #0d090a drawn
//! #1e1817, surround #412e32 drawn #3f2f2c, bezel #785d3a drawn #8a6c44,
//! shift #f6bc54 drawn #e0a850, labels #c38552 drawn #d39a62 (4.5:1),
//! "17B" #7a664f drawn #c9a86e (4.5:1 on the surround).
//!
//! One drawing serves both, as the 39G's serves the 40G: the 17B's
//! keyboard is the 17BII's without the RND, R↓ and x≷y labels and without
//! ENTER under = ([`keys`]). Every label keeps 4.5:1 (the 39G rule).
//!
//! The menu keys print a large ^; one shift key, a blank coloured cap,
//! whose labels use `left`. CLR is the ON-line key, with OFF above and ON
//! below.

use super::pioneer::{self, SKIRT, key};
use super::*;

/// The 17BII's black keys with white labels.
const KEY_17BII: Cap = Cap {
    fill: "#121110",
    ink: "#f3f3f1",
    well: SKIRT,
};
/// The 17BII's gold shift key, blank.
const SHIFT_17BII: Cap = Cap {
    fill: "#d5a356",
    ink: "#121110",
    well: SKIRT,
};
/// The 17B's brown-black keys.
const KEY_17B: Cap = Cap {
    fill: "#161212",
    ink: "#e6e2cc",
    well: SKIRT,
};
/// The 17B's gold-yellow shift key, blank.
const SHIFT_17B: Cap = Cap {
    fill: "#e0a850",
    ink: "#161212",
    well: SKIRT,
};

/// `label` on the 17BII, nothing on the 17B.
const fn pick(bii: bool, label: &'static str) -> &'static str {
    if bii { label } else { "" }
}

/// The keyboard, in `cap` and `shift` caps; `bii` adds the 17BII's labels
/// (RND, R↓, x≷y, ENTER under =).
const fn keys(cap: Cap, shift: Cap, bii: bool) -> [SkinKey; 37] {
    [
        // Row 1: the six menu keys, each printed with a ^.
        key(0, "a", cap, "^", "", ""),
        key(1, "b", cap, "^", "", ""),
        key(2, "c", cap, "^", "", ""),
        key(3, "d", cap, "^", "", ""),
        key(4, "e", cap, "^", "", ""),
        key(5, "f", cap, "^", "", ""),
        // Row 2.
        key(6, "sto", cap, "STO", "", ""),
        key(7, "rcl", cap, "RCL", pick(bii, "RND"), ""),
        key(8, "percent", cap, "%", "MATH", ""),
        key(9, "dsp", cap, "DSP", "MODES", ""),
        key(10, "prt", cap, "PRT", "PRINTER", ""),
        key(11, "exit", cap, "EXIT", "MAIN", ""),
        // Row 3: INPUT spans two keys.
        key(12, "input", cap, "INPUT", "CLEAR DATA", ""),
        key(13, "neg", cap, "+/-", "E", ""),
        key(14, "lparen", cap, "(", pick(bii, "R↓"), ""),
        key(15, "rparen", cap, ")", pick(bii, "x≷y"), ""),
        key(16, "backspace", cap, "⬅", "", ""),
        // Rows 4-7: a narrow key on the left, four wide keys.
        key(17, "up", cap, "▲", "", ""),
        key(18, "7", cap, "7", "", ""),
        key(19, "8", cap, "8", "", ""),
        key(20, "9", cap, "9", "", ""),
        key(21, "divide", cap, "÷", "1/x", ""),
        key(22, "down", cap, "▼", "", ""),
        key(23, "4", cap, "4", "", ""),
        key(24, "5", cap, "5", "", ""),
        key(25, "6", cap, "6", "", ""),
        key(26, "multiply", cap, "×", "yˣ", ""),
        key(27, "shift", shift, "", "", ""),
        key(28, "1", cap, "1", "", ""),
        key(29, "2", cap, "2", "", ""),
        key(30, "3", cap, "3", "", ""),
        key(31, "minus", cap, "−", "√x", ""),
        key(32, "on", cap, "CLR", "OFF", "ON"),
        key(33, "0", cap, "0", "MEM", ""),
        key(34, "point", cap, "·", "SHOW", ""),
        key(35, "equals", cap, "=", "LAST", pick(bii, "ENTER")),
        key(36, "plus", cap, "+", "x²", ""),
    ]
}

const KEYS_17BII: [SkinKey; 37] = keys(KEY_17BII, SHIFT_17BII, true);
const KEYS_17B: [SkinKey; 37] = keys(KEY_17B, SHIFT_17B, false);

const PANELS_17BII: [Panel; 6] = pioneer::plates(
    "#201f1d", "#0f0e0d", "#d1b797", "#93836c", "#1a1917", "#4b3d34",
);
const PANELS_17B: [Panel; 6] = pioneer::plates(
    "#1e1817", "#0e0b0a", "#3f2f2c", "#8a6c44", "#1a1616", "#3a2c26",
);

/// The thin light gold line framing the 17BII's keyboard plate, 9 units
/// inside it: down the left side from the band, along the bottom, up the
/// right side.
const GOLD: &str = "#d8caa6";
const LINES_17BII: [Line; 3] = [
    Line {
        x1: 42,
        y1: 478,
        x2: 42,
        y2: 1205,
        stroke: GOLD,
    },
    Line {
        x1: 42,
        y1: 1205,
        x2: 632,
        y2: 1205,
        stroke: GOLD,
    },
    Line {
        x1: 632,
        y1: 1205,
        x2: 632,
        y2: 478,
        stroke: GOLD,
    },
];

const MARKS_17BII: [Mark; 2] = [
    Mark {
        x: 126,
        y: 186,
        size: 28,
        fill: "#14110d",
        text: "17B II",
        italic: true,
        bold: true,
    },
    Mark {
        x: 537,
        y: 186,
        size: 23,
        fill: "#14110d",
        text: "BUSINESS",
        italic: true,
        bold: false,
    },
];

const MARKS_17B: [Mark; 2] = [
    Mark {
        x: 104,
        y: 186,
        size: 28,
        fill: "#c9a86e",
        text: "17B",
        italic: true,
        bold: true,
    },
    Mark {
        x: 537,
        y: 186,
        size: 23,
        fill: "#bdb4a0",
        text: "BUSINESS",
        italic: false,
        bold: false,
    },
];

/// The HP 17BII skin.
pub const SKIN_17BII: Skin = Skin {
    width: pioneer::WIDTH,
    height: pioneer::HEIGHT,
    panels: &PANELS_17BII,
    lcd: pioneer::LCD,
    lcd_fill: pioneer::LCD_FILL,
    glass: Some(pioneer::GLASS),
    logo: pioneer::LOGO,
    marks: &MARKS_17BII,
    lines: &LINES_17BII,
    left_ink: "#e8c48c",
    right_ink: "#e8c48c",
    alpha_ink: "#f3f3f1",
    alpha_badge: "#f3f3f1",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#f0e8d8",
    small: 17,
    well_fill: "#050505",
    round: 18,
    // Matte plastic (P4 shows a fine grain on the plate).
    texture: 9,
    keys: &KEYS_17BII,
};

/// The HP 17B skin: the 17BII's drawing with its own labels, colours and
/// name.
pub const SKIN_17B: Skin = Skin {
    width: pioneer::WIDTH,
    height: pioneer::HEIGHT,
    panels: &PANELS_17B,
    lcd: pioneer::LCD,
    lcd_fill: pioneer::LCD_FILL,
    glass: Some(pioneer::GLASS),
    logo: pioneer::LOGO,
    marks: &MARKS_17B,
    lines: &[],
    left_ink: "#d39a62",
    right_ink: "#d39a62",
    alpha_ink: "#e6e2cc",
    alpha_badge: "#e6e2cc",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#d39a62",
    small: 17,
    well_fill: "#070505",
    round: 18,
    texture: 9,
    keys: &KEYS_17B,
};
