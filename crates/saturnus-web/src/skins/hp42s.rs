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
//! key, an unlabelled orange cap; its labels use `left`.

use super::*;

/// The dark key caps with white labels (cap of 7; ink from STO and the
/// case lettering).
const KEY: Cap = Cap {
    fill: "#362d2c",
    ink: "#ecebe2",
    well: 0,
};
/// The orange shift key, unlabelled.
const SHIFT: Cap = Cap {
    fill: "#ef8b1d",
    ink: "#362d2c",
    well: 0,
};

const KEYS: [SkinKey; 37] = [
    // Row 1, on the lighter band; doubles as the menu keys.
    k("sigmaplus", r(53, 525, 73, 57), KEY, "Σ+", "Σ−", "", "", ""),
    k("inv", r(154, 525, 73, 57), KEY, "1/x", "yˣ", "", "", ""),
    k("sqrt", r(254, 525, 73, 57), KEY, "√x", "x²", "", "", ""),
    k("log", r(354, 525, 73, 57), KEY, "LOG", "10ˣ", "", "", ""),
    k("ln", r(453, 525, 73, 57), KEY, "LN", "eˣ", "", "", ""),
    k("xeq", r(553, 525, 73, 57), KEY, "XEQ", "GTO", "", "", ""),
    // Row 2.
    k("sto", r(53, 623, 73, 57), KEY, "STO", "COMPLEX", "", "", ""),
    k("rcl", r(154, 623, 73, 57), KEY, "RCL", "%", "", "", ""),
    k("rdn", r(254, 623, 73, 57), KEY, "R↓", "π", "", "", ""),
    k("sin", r(354, 623, 73, 57), KEY, "SIN", "ASIN", "", "", ""),
    k("cos", r(453, 623, 73, 57), KEY, "COS", "ACOS", "", "", ""),
    k("tan", r(553, 623, 73, 57), KEY, "TAN", "ATAN", "", "", ""),
    // Row 3: ENTER spans two keys.
    k(
        "enter",
        r(53, 722, 174, 57),
        KEY,
        "ENTER",
        "ALPHA",
        "",
        "",
        "",
    ),
    k(
        "swap",
        r(254, 722, 73, 57),
        KEY,
        "x≷y",
        "LAST x",
        "",
        "",
        "",
    ),
    k("neg", r(354, 722, 73, 57), KEY, "+/-", "MODES", "", "", ""),
    k("eex", r(453, 722, 73, 57), KEY, "E", "DISP", "", "", ""),
    k(
        "backspace",
        r(553, 722, 73, 57),
        KEY,
        "⬅",
        "CLEAR",
        "",
        "",
        "",
    ),
    // Rows 4-7: a narrow key on the left, four wide keys.
    k("up", r(53, 821, 74, 57), KEY, "▲", "BST", "", "", ""),
    k("7", r(168, 821, 95, 57), KEY, "7", "SOLVER", "", "", ""),
    k("8", r(289, 821, 96, 57), KEY, "8", "∫f(x)", "", "", ""),
    k("9", r(411, 821, 95, 57), KEY, "9", "MATRIX", "", "", ""),
    k("divide", r(532, 821, 96, 57), KEY, "÷", "STAT", "", "", ""),
    k("down", r(53, 920, 74, 57), KEY, "▼", "SST", "", "", ""),
    k("4", r(168, 920, 95, 57), KEY, "4", "BASE", "", "", ""),
    k("5", r(289, 920, 96, 57), KEY, "5", "CONVERT", "", "", ""),
    k("6", r(411, 920, 95, 57), KEY, "6", "FLAGS", "", "", ""),
    k(
        "multiply",
        r(532, 920, 96, 57),
        KEY,
        "×",
        "PROB",
        "",
        "",
        "",
    ),
    k("shift", r(53, 1019, 74, 57), SHIFT, "", "", "", "", ""),
    k("1", r(168, 1019, 95, 57), KEY, "1", "ASSIGN", "", "", ""),
    k("2", r(289, 1019, 96, 57), KEY, "2", "CUSTOM", "", "", ""),
    k("3", r(411, 1019, 95, 57), KEY, "3", "PGM.FCN", "", "", ""),
    k("minus", r(532, 1019, 96, 57), KEY, "−", "PRINT", "", "", ""),
    k("on", r(53, 1119, 74, 57), KEY, "EXIT", "OFF", "", "", "ON"),
    k("0", r(168, 1119, 95, 57), KEY, "0", "TOP.FCN", "", "", ""),
    k("point", r(289, 1119, 96, 57), KEY, "·", "SHOW", "", "", ""),
    k("rs", r(411, 1119, 95, 57), KEY, "R/S", "PRGM", "", "", ""),
    k(
        "plus",
        r(532, 1119, 96, 57),
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

const PANELS: [Panel; 24] = [
    // The case (side, sampled beside the keyboard), ending 60 units below
    // the bottom row (the photo shows 81; trimmed to the skins' rule that
    // the case ends shortly below the keys).
    panel(r(0, 0, 677, 1236), 24, 34, "#5a5654"),
    // The recessed face: its dark rim shows around the plates and in the
    // gap between the display plate and the keyboard plate.
    panel(r(30, 126, 617, 1097), 8, 27, "#1c1717"),
    // The display plate.
    panel(r(34, 129, 609, 345), 6, 0, "#89807a"),
    // The keyboard plate and the lighter band behind the top row.
    panel(r(34, 484, 609, 735), 0, 24, "#504848"),
    panel(r(34, 484, 609, 104), 0, 0, "#857c76"),
    // The silver LCD bezel and the glass inside it.
    panel(r(53, 204, 571, 212), 3, 3, "#d6d1c5"),
    panel(r(65, 234, 547, 153), 2, 2, "#ebf0d8"),
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
    lcd_fill: "#ebf0d8",
    // Centred on the HP logo's square (74, 52, 64 x 40).
    logo: r(82, 48, 48, 48),
    marks: &MARKS,
    lines: &[],
    left_ink: "#f7985e",
    right_ink: "#f7985e",
    alpha_ink: "#ecebe2",
    alpha_badge: "#ecebe2",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#ecebe2",
    small: 17,
    well_fill: "#1c1717",
    round: 12,
    keys: &KEYS,
};
