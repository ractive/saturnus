//! HP 49G skin.
//!
//! Geometry and labels: figure 1.1, "HP 49G key map", page 1-2 of the HP
//! 49G User's Manual (`hp49g-um-en.pdf`, PDF page 16, 400 dpi; softkey
//! pitch 131.4 px), checked against the owner's straight-on photographs
//! of his own HP 49G (October 2026, not in the repository): the key
//! columns and rows agree; the photographs gave the case height (1446
//! units), the place of the display surround (it starts 34 units from the
//! top and its lower edge bows down), the six tick marks and the model
//! name under the display, and every colour: a light silver-blue face
//! with a stronger blue rim, slate menu, operator and cursor keys,
//! charcoal number keys, taupe function keys with white labels, a
//! petrol ALPHA, a blue-violet left shift and a red right shift, black
//! outlines around all keys, navy left-shift and dark red right-shift
//! labels, white alpha letters on petrol.
//!
//! Differs from the unit on purpose: the display window is placed and
//! sized so the ROM's six menu labels sit over the six menu keys (see
//! [`super::SOFTKEY_LABEL_PITCH`]), 595 units wide where the real glass is
//! 509, so the surround spans almost the whole face and the ticks under
//! the display are shorter; the case outline is a rounded rectangle (the
//! real case widens into grips beside the display); the small glyphs
//! printed around the cursor pad are left off; the upper key rows sit
//! about 10 units lower than on the photographs; the case ends 61 units
//! below the ENTER row (83 on the unit) to keep the bottom margin near the
//! side margin. The shades are toned down from the sunlit exposure.

use super::*;

/// The slate menu and operator keys, white labels.
const DARK: Cap = Cap {
    fill: "#2f3d43",
    ink: "#f2eee8",
    well: 3,
};
/// The charcoal number keys.
const DIGIT: Cap = Cap {
    fill: "#2d2a29",
    ink: "#f2eee8",
    well: 3,
};
/// The taupe function keys, white labels.
const BEIGE: Cap = Cap {
    fill: "#8c806f",
    ink: "#f6f2ea",
    well: 3,
};
/// The slate cursor keys.
const ARROW: Cap = Cap {
    fill: "#33434a",
    ink: "#efeae4",
    well: 3,
};
/// ALPHA.
const ALPHA: Cap = Cap {
    fill: "#2c8399",
    ink: "#f6f2ea",
    well: 3,
};
/// The blue-violet left-shift key.
const LSHIFT: Cap = Cap {
    fill: "#4d59a3",
    ink: "#f6f2ea",
    well: 3,
};
/// The red right-shift key.
const RSHIFT: Cap = Cap {
    fill: "#d5493c",
    ink: "#f6f2ea",
    well: 3,
};

const KEYS: [SkinKey; 51] = [
    k("a", r(49, 534, 75, 39), DARK, "F1", "Y=", "", "A", ""),
    k("b", r(149, 534, 75, 39), DARK, "F2", "WIN", "", "B", ""),
    k("c", r(249, 534, 75, 39), DARK, "F3", "GRAPH", "", "C", ""),
    k("d", r(349, 534, 75, 39), DARK, "F4", "2D/3D", "", "D", ""),
    k("e", r(449, 534, 75, 39), DARK, "F5", "TBLSET", "", "E", ""),
    k("f", r(549, 534, 75, 39), DARK, "F6", "TABLE", "", "F", ""),
    k(
        "apps",
        r(62, 632, 89, 46),
        BEIGE,
        "APPS",
        "FILES",
        "BEGIN",
        "G",
        "",
    ),
    k(
        "mode",
        r(177, 632, 89, 46),
        BEIGE,
        "MODE",
        "CUSTOM",
        "END",
        "H",
        "",
    ),
    k(
        "tool",
        r(291, 632, 89, 46),
        BEIGE,
        "TOOL",
        "i",
        "|",
        "I",
        "",
    ),
    k(
        "var",
        r(62, 709, 89, 46),
        BEIGE,
        "VAR",
        "UPDIR",
        "COPY",
        "J",
        "",
    ),
    k(
        "sto",
        r(177, 709, 89, 46),
        BEIGE,
        "STO▶",
        "RCL",
        "CUT",
        "K",
        "",
    ),
    k(
        "nxt",
        r(291, 709, 89, 46),
        BEIGE,
        "NXT",
        "PREV",
        "PASTE",
        "L",
        "",
    ),
    k(
        "hist",
        r(62, 817, 89, 46),
        BEIGE,
        "HIST",
        "CMD",
        "UNDO",
        "M",
        "",
    ),
    k(
        "cat",
        r(177, 817, 89, 46),
        BEIGE,
        "CAT",
        "PRG",
        "CHARS",
        "N",
        "",
    ),
    k(
        "eqw",
        r(291, 817, 89, 46),
        BEIGE,
        "EQW",
        "MTRW",
        "'",
        "O",
        "",
    ),
    k(
        "symb",
        r(406, 817, 89, 46),
        BEIGE,
        "SYMB",
        "MTH",
        "EVAL",
        "P",
        "",
    ),
    k(
        "backspace",
        r(521, 817, 89, 46),
        BEIGE,
        "⬅",
        "DEL",
        "CLEAR",
        "",
        "",
    ),
    k(
        "power",
        r(62, 893, 89, 46),
        BEIGE,
        "yˣ",
        "eˣ",
        "LN",
        "Q",
        "",
    ),
    k(
        "sqrt",
        r(177, 893, 89, 46),
        BEIGE,
        "√x",
        "x²",
        "ˣ√y",
        "R",
        "",
    ),
    k(
        "sin",
        r(291, 893, 89, 46),
        BEIGE,
        "SIN",
        "ASIN",
        "Σ",
        "S",
        "",
    ),
    k(
        "cos",
        r(406, 893, 89, 46),
        BEIGE,
        "COS",
        "ACOS",
        "∂",
        "T",
        "",
    ),
    k(
        "tan",
        r(521, 893, 89, 46),
        BEIGE,
        "TAN",
        "ATAN",
        "∫",
        "U",
        "",
    ),
    k(
        "eex",
        r(62, 970, 89, 56),
        BEIGE,
        "EEX",
        "10ˣ",
        "LOG",
        "V",
        "",
    ),
    k("neg", r(177, 970, 90, 56), BEIGE, "+/−", "≠", "=", "W", ""),
    k("x", r(291, 970, 90, 56), BEIGE, "X", "≤", "<", "X", ""),
    k("inv", r(406, 970, 89, 56), BEIGE, "1/x", "≥", ">", "Y", ""),
    k(
        "divide",
        r(520, 970, 90, 56),
        DARK,
        "÷",
        "ABS",
        "ARG",
        "Z",
        "",
    ),
    k(
        "alpha",
        r(62, 1055, 89, 55),
        ALPHA,
        "ALPHA",
        "USER",
        "ENTRY",
        "",
        "",
    ),
    k(
        "7",
        r(177, 1055, 90, 55),
        DIGIT,
        "7",
        "S.SLV",
        "NUM.SLV",
        "",
        "",
    ),
    k(
        "8",
        r(291, 1055, 90, 55),
        DIGIT,
        "8",
        "EXP&LN",
        "TRIG",
        "",
        "",
    ),
    k(
        "9",
        r(406, 1055, 89, 55),
        DIGIT,
        "9",
        "FINANCE",
        "TIME",
        "",
        "",
    ),
    k(
        "multiply",
        r(520, 1055, 90, 55),
        DARK,
        "×",
        "[ ]",
        "\" \"",
        "",
        "",
    ),
    k(
        "leftshift",
        r(62, 1139, 89, 55),
        LSHIFT,
        "↰",
        "",
        "",
        "",
        "",
    ),
    k("4", r(177, 1139, 90, 55), DIGIT, "4", "CALC", "ALG", "", ""),
    k(
        "5",
        r(291, 1139, 90, 55),
        DIGIT,
        "5",
        "MATRICES",
        "STAT",
        "",
        "",
    ),
    k(
        "6",
        r(406, 1139, 89, 55),
        DIGIT,
        "6",
        "CONVERT",
        "UNITS",
        "",
        "",
    ),
    k("minus", r(520, 1139, 90, 55), DARK, "−", "( )", "_", "", ""),
    k(
        "rightshift",
        r(62, 1224, 89, 56),
        RSHIFT,
        "↱",
        "",
        "",
        "",
        "",
    ),
    k(
        "1",
        r(177, 1224, 90, 56),
        DIGIT,
        "1",
        "ARITH",
        "CMPLX",
        "",
        "",
    ),
    k("2", r(291, 1224, 90, 56), DIGIT, "2", "DEF", "LIB", "", ""),
    k("3", r(406, 1224, 89, 56), DIGIT, "3", "#", "BASE", "", ""),
    k(
        "plus",
        r(520, 1224, 90, 56),
        DARK,
        "+",
        "{ }",
        "« »",
        "",
        "",
    ),
    k(
        "on",
        r(62, 1308, 89, 56),
        BEIGE,
        "ON",
        "CONT",
        "OFF",
        "",
        "CANCEL",
    ),
    k("0", r(177, 1308, 90, 56), DIGIT, "0", "∞", "→", "", ""),
    k("point", r(291, 1308, 90, 56), DIGIT, "·", "::", "↵", "", ""),
    k(
        "space",
        r(406, 1308, 89, 56),
        DIGIT,
        "SPC",
        "π",
        ",",
        "",
        "",
    ),
    k(
        "enter",
        r(520, 1308, 90, 56),
        DARK,
        "ENTER",
        "ANS",
        "→NUM",
        "",
        "",
    ),
    SkinKey {
        shape: Shape::Up,
        ..k("up", r(463, 583, 88, 49), ARROW, "▲", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Left,
        ..k("left", r(403, 657, 84, 53), ARROW, "◀", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Right,
        ..k("right", r(528, 657, 84, 53), ARROW, "▶", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Down,
        ..k("down", r(463, 734, 88, 50), ARROW, "▼", "", "", "", "")
    },
];

const PANELS: [Panel; 3] = [
    panel(r(0, 0, 671, 1425), 70, 110, "#6f90bd"),
    panel(r(15, 13, 641, 1399), 58, 98, "#a9bdcc"),
    // The black display surround; its lower edge bows down.
    Panel {
        bow: 30,
        ..panel(r(26, 34, 619, 466), 44, 60, "#191514")
    },
];

const MARKS: [Mark; 1] = [Mark {
    x: 336,
    y: 474,
    size: 30,
    fill: "#d8cabf",
    text: "49G",
    italic: true,
}];

/// The six tick marks under the display.
const LINES: [Line; 6] = [
    Line {
        x1: 119,
        y1: 441,
        x2: 119,
        y2: 478,
        stroke: "#cdbfb4",
    },
    Line {
        x1: 206,
        y1: 441,
        x2: 206,
        y2: 478,
        stroke: "#cdbfb4",
    },
    Line {
        x1: 293,
        y1: 441,
        x2: 293,
        y2: 478,
        stroke: "#cdbfb4",
    },
    Line {
        x1: 380,
        y1: 441,
        x2: 380,
        y2: 478,
        stroke: "#cdbfb4",
    },
    Line {
        x1: 467,
        y1: 441,
        x2: 467,
        y2: 478,
        stroke: "#cdbfb4",
    },
    Line {
        x1: 554,
        y1: 441,
        x2: 554,
        y2: 478,
        stroke: "#cdbfb4",
    },
];

/// The HP 49G skin.
pub const SKIN: Skin = Skin {
    width: 671,
    height: 1425,
    panels: &PANELS,
    lcd: r(39, 98, 595, 327),
    lcd_fill: "#b7c2a2",
    logo: r(314, 42, 44, 44),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#33436b",
    right_ink: "#a0393d",
    alpha_ink: "#ffffff",
    alpha_badge: "#2d7397",
    alpha_style: AlphaStyle::Badge,
    below_ink: "#f1f3f1",
    small: 15,
    well_fill: "#0e0b0b",
    round: 34,
    keys: &KEYS,
};
