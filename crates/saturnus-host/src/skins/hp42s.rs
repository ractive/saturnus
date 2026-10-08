//! HP 42S skin.
//!
//! Geometry and colours from the owner's own 42S, photographed square on
//! (`WhatsApp Image 2026-10-05 at 16.51.44 (3).jpeg`, 1500 x 2000 px; the
//! camera was parallel to the case: the case is 1004 px wide at the top and
//! at the bottom and the key widths do not change down the keyboard). Unit:
//! the top-row key pitch, 148.2 px (left edges 210 and 951 px, five
//! pitches); origin the case's top-left corner at (131, 46) px. Key
//! outlines are the bounding boxes of the dark caps (with their black
//! skirts) on the lighter plate. A second photo of the same calculator
//! (`... 16.51.44 (4).jpeg`) was taken at a tilt (key widths grow by 5%
//! down the keyboard) and was used only to confirm the labels, the label
//! bands and the colours.
//!
//! Measured: the case (1004 x 1863 px), the recessed face (dark rim), the
//! lighter display plate, the silver LCD bezel and the glass, the lighter
//! band behind the top row, every key, the grey bands behind the shifted
//! labels that open menus (none behind LAST x, BST, SST, ASSIGN, OFF, SHOW
//! and PRGM), the "42S" and "RPN SCIENTIFIC" text. Colours are medians of
//! photo pixels: case side, display plate, bezel, glass, top-row band,
//! label bands, key caps (the cap of 7), the shift key, the shifted labels
//! (the most saturated pixels of COMPLEX, SOLVER and CONVERT).
//!
//! Inferred: the corner radii; the keyboard plate is one colour (the photo
//! shades from #655d5a under row 2 to #443e40 under row 7, lighting); row
//! heights are evened to 57 units (56-58 measured). The 42S has one shift
//! key, an unlabelled orange cap; its labels use `left`. Each cap stands on
//! a black skirt ([`SKIRT`]), the measured footprint less the skirt being
//! the cap. The shifted labels are the photo's saturated orange (the
//! most saturated pixels of COMPLEX, SOLVER and CONVERT), not a median.

use super::*;

/// The black skirt every cap stands on, how far it shows beyond the cap
/// on each side (photo: 7-8 units at the sides, a thin lip above and the
/// key's front below). Drawn as the cap's well; a key's rect is its cap,
/// the skirt around it is the measured footprint.
const SKIRT: i16 = 6;

/// The dark key caps with white labels (cap of 7; ink from STO and the
/// case lettering).
const KEY: Cap = Cap {
    fill: "#332a29",
    ink: "#ecebe2",
    well: SKIRT,
};
/// The orange shift key, unlabelled.
const SHIFT: Cap = Cap {
    fill: "#ef8b1d",
    ink: "#362d2c",
    well: SKIRT,
};

const KEYS: [SkinKey; 37] = [
    // Row 1, on the lighter band; doubles as the menu keys.
    k("sigmaplus", r(59, 531, 61, 45), KEY, "Σ+", "Σ−", "", "", ""),
    k("inv", r(160, 531, 61, 45), KEY, "1/x", "yˣ", "", "", ""),
    k("sqrt", r(260, 531, 61, 45), KEY, "√x", "x²", "", "", ""),
    k("log", r(360, 531, 61, 45), KEY, "LOG", "10ˣ", "", "", ""),
    k("ln", r(459, 531, 61, 45), KEY, "LN", "eˣ", "", "", ""),
    k("xeq", r(559, 531, 61, 45), KEY, "XEQ", "GTO", "", "", ""),
    // Row 2.
    k("sto", r(59, 629, 61, 45), KEY, "STO", "COMPLEX", "", "", ""),
    k("rcl", r(160, 629, 61, 45), KEY, "RCL", "%", "", "", ""),
    k("rdn", r(260, 629, 61, 45), KEY, "R↓", "π", "", "", ""),
    k("sin", r(360, 629, 61, 45), KEY, "SIN", "ASIN", "", "", ""),
    k("cos", r(459, 629, 61, 45), KEY, "COS", "ACOS", "", "", ""),
    k("tan", r(559, 629, 61, 45), KEY, "TAN", "ATAN", "", "", ""),
    // Row 3: ENTER spans two keys.
    k(
        "enter",
        r(59, 728, 162, 45),
        KEY,
        "ENTER",
        "ALPHA",
        "",
        "",
        "",
    ),
    k(
        "swap",
        r(260, 728, 61, 45),
        KEY,
        "x≷y",
        "LAST x",
        "",
        "",
        "",
    ),
    k("neg", r(360, 728, 61, 45), KEY, "+/-", "MODES", "", "", ""),
    k("eex", r(459, 728, 61, 45), KEY, "E", "DISP", "", "", ""),
    k(
        "backspace",
        r(559, 728, 61, 45),
        KEY,
        "⬅",
        "CLEAR",
        "",
        "",
        "",
    ),
    // Rows 4-7: a narrow key on the left, four wide keys.
    k("up", r(59, 827, 62, 45), KEY, "▲", "BST", "", "", ""),
    k("7", r(174, 827, 83, 45), KEY, "7", "SOLVER", "", "", ""),
    k("8", r(295, 827, 84, 45), KEY, "8", "∫f(x)", "", "", ""),
    k("9", r(417, 827, 83, 45), KEY, "9", "MATRIX", "", "", ""),
    k("divide", r(538, 827, 84, 45), KEY, "÷", "STAT", "", "", ""),
    k("down", r(59, 926, 62, 45), KEY, "▼", "SST", "", "", ""),
    k("4", r(174, 926, 83, 45), KEY, "4", "BASE", "", "", ""),
    k("5", r(295, 926, 84, 45), KEY, "5", "CONVERT", "", "", ""),
    k("6", r(417, 926, 83, 45), KEY, "6", "FLAGS", "", "", ""),
    k(
        "multiply",
        r(538, 926, 84, 45),
        KEY,
        "×",
        "PROB",
        "",
        "",
        "",
    ),
    k("shift", r(59, 1025, 62, 45), SHIFT, "", "", "", "", ""),
    k("1", r(174, 1025, 83, 45), KEY, "1", "ASSIGN", "", "", ""),
    k("2", r(295, 1025, 84, 45), KEY, "2", "CUSTOM", "", "", ""),
    k("3", r(417, 1025, 83, 45), KEY, "3", "PGM.FCN", "", "", ""),
    k("minus", r(538, 1025, 84, 45), KEY, "−", "PRINT", "", "", ""),
    k("on", r(59, 1125, 62, 45), KEY, "EXIT", "OFF", "", "", "ON"),
    k("0", r(174, 1125, 83, 45), KEY, "0", "TOP.FCN", "", "", ""),
    k("point", r(295, 1125, 84, 45), KEY, "·", "SHOW", "", "", ""),
    k("rs", r(417, 1125, 83, 45), KEY, "R/S", "PRGM", "", "", ""),
    k(
        "plus",
        r(538, 1125, 84, 45),
        KEY,
        "+",
        "CATALOG",
        "",
        "",
        "",
    ),
];

/// Grey band behind a shifted label that opens a menu: as wide as the key,
/// from 23 to 6 units above it.
const fn band(x: i16, key_y: i16, w: i16) -> Panel {
    panel(r(x, key_y - 23, w, 17), 2, 2, "#6f6564")
}

const PANELS: [Panel; 23] = [
    // The case (side, sampled beside the keyboard), ending 60 units below
    // the bottom row (the photo shows 81; trimmed to the skins' rule that
    // the case ends shortly below the keys).
    panel(r(0, 0, 677, 1236), 24, 34, "#5a5654"),
    // The recessed face: its dark rim shows around the plates and in the
    // gap between the display plate and the keyboard plate.
    sunk(r(30, 126, 617, 1097), 8, 27, "#1c1717"),
    // The display plate, standing in the recess.
    raised(r(34, 129, 609, 345), 6, 0, "#89807a"),
    // The keyboard plate and the lighter band behind the top row.
    raised(r(34, 484, 609, 735), 0, 24, "#504848"),
    panel(r(34, 484, 609, 104), 0, 0, "#857c76"),
    // The silver LCD bezel. The page draws the glass inside it around the
    // LCD (`lcd` plus its margin); the bezel is 12 units wide at the sides
    // and 18 above and below, where the unit's glass (153 units tall, with
    // taller dots) shows a wider frame of 12 and 30.
    raised(r(47, 236, 583, 148), 3, 3, "#d6d1c5"),
    // Label bands, row 3: ALPHA MODES DISP CLEAR.
    band(53, 722, 174),
    band(354, 722, 73),
    band(453, 722, 73),
    band(553, 722, 73),
    // Row 4: SOLVER ∫f(x) MATRIX STAT.
    band(168, 821, 95),
    band(289, 821, 96),
    band(411, 821, 95),
    band(532, 821, 96),
    // Row 5: BASE CONVERT FLAGS PROB.
    band(168, 920, 95),
    band(289, 920, 96),
    band(411, 920, 95),
    band(532, 920, 96),
    // Row 6: CUSTOM PGM.FCN PRINT (none behind ASSIGN).
    band(289, 1019, 96),
    band(411, 1019, 95),
    band(532, 1019, 96),
    // Row 7: TOP.FCN and CATALOG (none behind SHOW and PRGM).
    band(168, 1119, 95),
    band(532, 1119, 96),
];

const MARKS: [Mark; 2] = [
    Mark {
        x: 113,
        y: 187,
        size: 20,
        fill: "#f6f6ee",
        text: "42S",
        italic: false,
    },
    Mark {
        x: 490,
        y: 187,
        size: 20,
        fill: "#f6f6ee",
        text: "RPN SCIENTIFIC",
        italic: false,
    },
];

/// The HP 42S skin.
pub const SKIN: Skin = Skin {
    width: 677,
    height: 1236,
    panels: &PANELS,
    // The LCD's active area, 131 x 24 pixels (16 rows and the annunciator
    // strip) at square pixels across the glass's width, centred in it.
    // Inferred: the real dots are taller than wide.
    lcd: r(65, 260, 547, 100),
    lcd_fill: "#b7c2a2",
    // Centred on the HP logo's square (74, 52, 64 x 40).
    logo: r(82, 48, 48, 48),
    marks: &MARKS,
    lines: &[],
    left_ink: "#f6902f",
    right_ink: "#f6902f",
    alpha_ink: "#ecebe2",
    alpha_badge: "#ecebe2",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#ecebe2",
    small: 17,
    well_fill: "#141010",
    round: 18,
    // Matte plastic with a visible speckle (photo).
    texture: 9,
    keys: &KEYS,
};
