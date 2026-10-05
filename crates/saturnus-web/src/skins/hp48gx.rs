//! HP 48GX skin.
//!
//! Geometry: as the 48SX (one case): keys from the HP 48SX Owner's Manual
//! vol. 1 page 26 figure, the display and the case top from the HP 48G
//! Series User's Guide page 1-9 figure (`hp48gug.pdf`, PDF page 23); the
//! 48G guide's own keyboard figures (pages 1-5 and 2-3, PDF pages 19 and
//! 27) agree with that key grid. Labels and their colours from the
//! photograph "Hewlett-Packard 48GX Scientific Graphing Calculator.jpg"
//! (Wikimedia Commons) and the 48G guide's cover photo: purple left
//! shift, green right shift; single labels are either colour, as printed.
//! The number keys and the right-shift key sit on lighter tiles.
//!
//! Inferred: the case below the ON row (52 units, about the side margin,
//! from the photo); the shades; the tiles' exact extent. The display window
//! is placed so the ROM's six menu labels sit over the six menu keys (see
//! [`super::SOFTKEY_LABEL_PITCH`]); its bezel follows.

use super::*;

/// Black key caps with light grey labels.
const KEY: Cap = Cap {
    fill: "#191d1f",
    ink: "#e6e9e4",
};
/// The light grey menu keys.
const MENU: Cap = Cap {
    fill: "#c9cdc7",
    ink: "#191d1f",
};
/// The purple left-shift key.
const LSHIFT: Cap = Cap {
    fill: "#8c86cc",
    ink: "#191d1f",
};
/// The green right-shift key.
const RSHIFT: Cap = Cap {
    fill: "#2ba6a2",
    ink: "#191d1f",
};

const KEYS: [SkinKey; 49] = [
    k("a", r(46, 462, 77, 51), MENU, "", "", "", "A", ""),
    k("b", r(145, 462, 77, 51), MENU, "", "", "", "B", ""),
    k("c", r(245, 462, 77, 51), MENU, "", "", "", "C", ""),
    k("d", r(345, 462, 77, 51), MENU, "", "", "", "D", ""),
    k("e", r(446, 462, 76, 51), MENU, "", "", "", "E", ""),
    k("f", r(546, 462, 77, 51), MENU, "", "", "", "F", ""),
    k(
        "mth",
        r(46, 549, 77, 52),
        KEY,
        "MTH",
        "RAD",
        "POLAR",
        "G",
        "",
    ),
    k("prg", r(145, 549, 77, 52), KEY, "PRG", "", "CHARS", "H", ""),
    k("cst", r(245, 549, 77, 52), KEY, "CST", "", "MODES", "I", ""),
    k(
        "var",
        r(345, 549, 77, 52),
        KEY,
        "VAR",
        "",
        "MEMORY",
        "J",
        "",
    ),
    k("up", r(447, 549, 76, 52), KEY, "▲", "", "STACK", "K", ""),
    k(
        "nxt",
        r(546, 549, 77, 52),
        KEY,
        "NXT",
        "PREV",
        "MENU",
        "L",
        "",
    ),
    k("quote", r(45, 639, 77, 52), KEY, "'", "UP", "HOME", "M", ""),
    k(
        "sto",
        r(145, 639, 77, 52),
        KEY,
        "STO",
        "DEF",
        "RCL",
        "N",
        "",
    ),
    k(
        "eval",
        r(244, 639, 77, 52),
        KEY,
        "EVAL",
        "→NUM",
        "UNDO",
        "O",
        "",
    ),
    k(
        "left",
        r(346, 639, 77, 52),
        KEY,
        "◀",
        "PICTURE",
        "",
        "P",
        "",
    ),
    k("down", r(445, 639, 77, 52), KEY, "▼", "VIEW", "", "Q", ""),
    k("right", r(547, 639, 77, 52), KEY, "▶", "SWAP", "", "R", ""),
    k("sin", r(45, 728, 77, 51), KEY, "SIN", "ASIN", "∂", "S", ""),
    k("cos", r(145, 728, 77, 51), KEY, "COS", "ACOS", "∫", "T", ""),
    k("tan", r(244, 728, 77, 51), KEY, "TAN", "ATAN", "Σ", "U", ""),
    k("sqrt", r(345, 728, 77, 51), KEY, "√x", "x²", "ˣ√y", "V", ""),
    k(
        "power",
        r(443, 728, 77, 51),
        KEY,
        "yˣ",
        "10ˣ",
        "LOG",
        "W",
        "",
    ),
    k("inv", r(545, 728, 77, 51), KEY, "1/x", "eˣ", "LN", "X", ""),
    k(
        "enter",
        r(46, 811, 177, 53),
        KEY,
        "ENTER",
        "EQUATION",
        "MATRIX",
        "",
        "",
    ),
    k(
        "neg",
        r(246, 811, 77, 53),
        KEY,
        "+/-",
        "EDIT",
        "CMD",
        "Y",
        "",
    ),
    k(
        "eex",
        r(346, 811, 75, 53),
        KEY,
        "EEX",
        "PURG",
        "ARG",
        "Z",
        "",
    ),
    k("del", r(446, 811, 77, 53), KEY, "DEL", "CLEAR", "", "", ""),
    k(
        "backspace",
        r(547, 811, 77, 53),
        KEY,
        "⬅",
        "DROP",
        "",
        "",
        "",
    ),
    k(
        "alpha",
        r(48, 900, 76, 51),
        KEY,
        "α",
        "USER",
        "ENTRY",
        "",
        "",
    ),
    k("7", r(149, 900, 96, 51), KEY, "7", "", "SOLVE", "", ""),
    k("8", r(274, 900, 96, 51), KEY, "8", "", "PLOT", "", ""),
    k("9", r(400, 900, 96, 51), KEY, "9", "", "SYMBOLIC", "", ""),
    k("divide", r(528, 900, 95, 51), KEY, "÷", "( )", "#", "", ""),
    k("leftshift", r(48, 989, 77, 51), LSHIFT, "↰", "", "", "", ""),
    k("4", r(146, 989, 96, 51), KEY, "4", "", "TIME", "", ""),
    k("5", r(273, 989, 96, 51), KEY, "5", "", "STAT", "", ""),
    k("6", r(398, 989, 96, 51), KEY, "6", "", "UNITS", "", ""),
    k(
        "multiply",
        r(526, 989, 96, 51),
        KEY,
        "×",
        "[ ]",
        "_",
        "",
        "",
    ),
    k(
        "rightshift",
        r(47, 1076, 77, 53),
        RSHIFT,
        "↱",
        "",
        "",
        "",
        "",
    ),
    k("1", r(146, 1076, 96, 53), KEY, "1", "", "I/O", "", ""),
    k("2", r(273, 1076, 96, 53), KEY, "2", "", "LIBRARY", "", ""),
    k("3", r(399, 1076, 96, 53), KEY, "3", "", "EQ LIB", "", ""),
    k(
        "minus",
        r(526, 1076, 96, 53),
        KEY,
        "−",
        "« »",
        "\" \"",
        "",
        "",
    ),
    k(
        "on",
        r(48, 1162, 77, 51),
        KEY,
        "ON",
        "CONT",
        "OFF",
        "",
        "CANCEL",
    ),
    k("0", r(147, 1162, 96, 51), KEY, "0", "=", "→", "", ""),
    k("point", r(273, 1162, 96, 51), KEY, "·", ",", "↵", "", ""),
    k("space", r(399, 1162, 96, 51), KEY, "SPC", "π", "∡", "", ""),
    k("plus", r(527, 1162, 96, 51), KEY, "+", "{ }", "::", "", ""),
];

const PANELS: [Panel; 14] = [
    panel(r(0, 0, 667, 1265), 40, 40, "#2c3233"),
    panel(r(14, 14, 639, 1237), 30, 30, "#3e4544"),
    panel(r(14, 14, 639, 432), 30, 0, "#556260"),
    panel(r(136, 864, 122, 95), 3, 3, "#4a5351"),
    panel(r(261, 864, 122, 95), 3, 3, "#4a5351"),
    panel(r(387, 864, 122, 95), 3, 3, "#4a5351"),
    panel(r(133, 953, 122, 95), 3, 3, "#4a5351"),
    panel(r(260, 953, 122, 95), 3, 3, "#4a5351"),
    panel(r(385, 953, 122, 95), 3, 3, "#4a5351"),
    panel(r(133, 1040, 122, 97), 3, 3, "#4a5351"),
    panel(r(260, 1040, 122, 97), 3, 3, "#4a5351"),
    panel(r(386, 1040, 122, 97), 3, 3, "#4a5351"),
    panel(r(29, 1040, 113, 97), 3, 3, "#4a5351"),
    panel(r(24, 86, 619, 355), 18, 18, "#252b2a"),
];

const MARKS: [Mark; 1] = [Mark {
    x: 182,
    y: 70,
    size: 34,
    fill: "#e8e4da",
    text: "48GX",
}];

const LINES: [Line; 0] = [];

/// The HP 48GX skin.
pub const SKIN: Skin = Skin {
    width: 667,
    height: 1265,
    panels: &PANELS,
    lcd: r(37, 100, 595, 327),
    lcd_fill: "#b7c2a2",
    logo: r(44, 26, 56, 56),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#a99fe6",
    right_ink: "#43c6b5",
    alpha_ink: "#dfe3dd",
    alpha_badge: "#dfe3dd",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#dfe3dd",
    small: 17,
    keys: &KEYS,
};
