//! HP 27S skin.
//!
//! Sources (wiki: hardware/hp27s "Key labels", "Colours"): the colour
//! keyboard drawing on PDF page 2 of the HP-27S Owner's Manual (edition 2,
//! 1988; wiki: sources/hp27s-om), read as text for the labels, and the
//! Wikimedia Commons photograph P3 of a 1987 27S (wiki:
//! sources/commons-pioneer-photos), whose sampled medians the wiki lists
//! as the colours. Nothing of the figure or the photo is stored here.
//!
//! Inferred: the geometry. The 27S is the 42S's Pioneer case with the same
//! 37-key grid (six keys in rows 1-2, a two-key-wide INPUT and four in row
//! 3, a narrow key and four wide ones in rows 4-7), so every key, panel,
//! the LCD window and the logo take the 42S skin's measured positions
//! ([`super::hp42s`]); they were not re-measured on the manual's figure.
//! The 42S's lighter band behind the top row is left off (the wiki names
//! none on the 27S). The case lettering ("27S", "SCIENTIFIC") sits where
//! the 42S prints its own. The recessed rim and the black skirts under
//! the caps follow the 42S, darkened for the black case.
//!
//! Toned for 4.5:1 (the 39G rule): the shifted labels are a little lighter
//! than the photo's light blue (#82acc4) and the grey bars behind the
//! digit block's menu names a little darker than its #48443f, so the blue
//! reads on the bars as on the faceplate.
//!
//! The menu keys print a small ^ only; the 27S has one shift key, a blank
//! blue cap; its labels use `left`. CLR is the ON-line key, with OFF above
//! and ON below it.

use super::*;

/// The black skirt every cap stands on (the 42S's).
const SKIRT: i16 = 6;

/// The near-black keys with off-white labels.
const KEY: Cap = Cap {
    fill: "#151413",
    ink: "#cfd1c1",
    well: SKIRT,
};
/// The blue shift key, blank.
const SHIFT: Cap = Cap {
    fill: "#5e94bc",
    ink: "#151413",
    well: SKIRT,
};

const KEYS: [SkinKey; 37] = [
    // Row 1: the six menu keys, each printed with a small ^.
    k("a", r(59, 531, 61, 45), KEY, "^", "√x", "", "", ""),
    k("b", r(160, 531, 61, 45), KEY, "^", "x²", "", "", ""),
    k("c", r(260, 531, 61, 45), KEY, "^", "eˣ", "", "", ""),
    k("d", r(360, 531, 61, 45), KEY, "^", "LN", "", "", ""),
    k("e", r(459, 531, 61, 45), KEY, "^", "yˣ", "", "", ""),
    k("f", r(559, 531, 61, 45), KEY, "^", "1/x", "", "", ""),
    // Row 2.
    k("sto", r(59, 629, 61, 45), KEY, "STO", "10ˣ", "", "", ""),
    k("rcl", r(160, 629, 61, 45), KEY, "RCL", "LOG", "", "", ""),
    k("sin", r(260, 629, 61, 45), KEY, "SIN", "ASIN", "", "", ""),
    k("cos", r(360, 629, 61, 45), KEY, "COS", "ACOS", "", "", ""),
    k("tan", r(459, 629, 61, 45), KEY, "TAN", "ATAN", "", "", ""),
    k("exit", r(559, 629, 61, 45), KEY, "EXIT", "MAIN", "", "", ""),
    // Row 3: INPUT spans two keys.
    k(
        "input",
        r(59, 728, 162, 45),
        KEY,
        "INPUT",
        "CLEAR DATA",
        "",
        "",
        "",
    ),
    k("neg", r(260, 728, 61, 45), KEY, "+/-", "E", "", "", ""),
    k("lparen", r(360, 728, 61, 45), KEY, "(", "%", "", "", ""),
    k("rparen", r(459, 728, 61, 45), KEY, ")", "π", "", "", ""),
    k("backspace", r(559, 728, 61, 45), KEY, "⬅", "", "", "", ""),
    // Rows 4-7: a narrow key on the left, four wide keys.
    k("up", r(59, 827, 62, 45), KEY, "▲", "", "", "", ""),
    k("7", r(174, 827, 83, 45), KEY, "7", "SOLVE", "", "", ""),
    k("8", r(295, 827, 84, 45), KEY, "8", "STAT", "", "", ""),
    k("9", r(417, 827, 83, 45), KEY, "9", "TVM", "", "", ""),
    k("divide", r(538, 827, 84, 45), KEY, "÷", "TIME", "", "", ""),
    k("down", r(59, 926, 62, 45), KEY, "▼", "", "", "", ""),
    k("4", r(174, 926, 83, 45), KEY, "4", "BASE", "", "", ""),
    k("5", r(295, 926, 84, 45), KEY, "5", "PROB", "", "", ""),
    k("6", r(417, 926, 83, 45), KEY, "6", "HYP", "", "", ""),
    k(
        "multiply",
        r(538, 926, 84, 45),
        KEY,
        "×",
        "CONVERT",
        "",
        "",
        "",
    ),
    k("shift", r(59, 1025, 62, 45), SHIFT, "", "", "", "", ""),
    k("1", r(174, 1025, 83, 45), KEY, "1", "%CHG", "", "", ""),
    k("2", r(295, 1025, 84, 45), KEY, "2", "PARTS", "", "", ""),
    k("3", r(417, 1025, 83, 45), KEY, "3", "MODES", "", "", ""),
    k(
        "minus",
        r(538, 1025, 84, 45),
        KEY,
        "−",
        "PRINTER",
        "",
        "",
        "",
    ),
    k("on", r(59, 1125, 62, 45), KEY, "CLR", "OFF", "", "", "ON"),
    k("0", r(174, 1125, 83, 45), KEY, "0", "MEM", "", "", ""),
    k("point", r(295, 1125, 84, 45), KEY, "·", "SHOW", "", "", ""),
    k("equals", r(417, 1125, 83, 45), KEY, "=", "LAST", "", "", ""),
    k("plus", r(538, 1125, 84, 45), KEY, "+", "PRT", "", "", ""),
];

/// The grey bar behind a menu name of the digit block: as wide as the
/// key's skirt, from 23 to 6 units above the skirt's top (`skirt_y`; the
/// 42S's bands).
const fn bar(x: i16, skirt_y: i16, w: i16) -> Panel {
    panel(r(x, skirt_y - 23, w, 17), 2, 2, "#403c37")
}

const PANELS: [Panel; 17] = [
    // The black case.
    panel(r(0, 0, 677, 1236), 24, 34, "#1c1a18"),
    // The recessed face, its rim around the plates.
    sunk(r(30, 126, 617, 1097), 8, 27, "#0e0d0c"),
    // The dark grey display surround.
    raised(r(34, 129, 609, 345), 6, 0, "#4f4944"),
    // The keyboard faceplate.
    raised(r(34, 484, 609, 735), 0, 24, "#292623"),
    // The silver LCD frame.
    raised(r(47, 236, 583, 148), 3, 3, "#cbc1b0"),
    // Grey bars, row 4: SOLVE STAT TVM TIME.
    bar(168, 821, 95),
    bar(289, 821, 96),
    bar(411, 821, 95),
    bar(532, 821, 96),
    // Row 5: BASE PROB HYP CONVERT.
    bar(168, 920, 95),
    bar(289, 920, 96),
    bar(411, 920, 95),
    bar(532, 920, 96),
    // Row 6: %CHG PARTS MODES PRINTER.
    bar(168, 1019, 95),
    bar(289, 1019, 96),
    bar(411, 1019, 95),
    bar(532, 1019, 96),
];

const MARKS: [Mark; 2] = [
    Mark {
        x: 113,
        y: 187,
        size: 20,
        fill: "#ffffff",
        text: "27S",
        italic: false,
    },
    Mark {
        x: 490,
        y: 187,
        size: 20,
        fill: "#ffffff",
        text: "SCIENTIFIC",
        italic: false,
    },
];

/// The HP 27S skin.
pub const SKIN: Skin = Skin {
    width: 677,
    height: 1236,
    panels: &PANELS,
    // The 42S's window: 131 x 24 (16 rows and the annunciator strip).
    lcd: r(65, 260, 547, 100),
    lcd_fill: "#b7c2a2",
    // Where the 42S's logo sits (inferred: the same case).
    logo: r(82, 48, 48, 48),
    marks: &MARKS,
    lines: &[],
    left_ink: "#8fb6cd",
    right_ink: "#8fb6cd",
    alpha_ink: "#cfd1c1",
    alpha_badge: "#cfd1c1",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#cfd1c1",
    small: 17,
    well_fill: "#0a0909",
    round: 18,
    // Inferred: the 42S's matte plastic.
    texture: 9,
    keys: &KEYS,
};
