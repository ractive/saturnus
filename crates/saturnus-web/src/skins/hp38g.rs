//! HP 38G skin.
//!
//! Geometry and labels: the keyboard figure inside the cover of the HP 38G
//! User's Guide (`hp38g-ug-en.pdf`, PDF page 2, 400 dpi; softkey pitch
//! 139.3 px), which shows the whole case. Colours from the photograph
//! "HP-38G scientific graphing calculator (edited, without background).JPG"
//! (Wikimedia Commons): dark grey case and keys with cream labels, cream
//! cursor keys, a turquoise shift key and turquoise shifted labels, orange
//! alpha letters, a teal panel behind the lower six rows. The second row
//! has no keys at the 48's VAR and NXT places.
//!
//! Inferred: the shades; the slot under the display (from the photo).

use super::*;

/// Dark grey key caps with cream labels.
const KEY: Cap = Cap {
    fill: "#26292b",
    ink: "#f0e4ae",
};
/// The cream cursor keys.
const ARROW: Cap = Cap {
    fill: "#ece0a8",
    ink: "#26292b",
};
/// The turquoise shift key.
const SHIFT: Cap = Cap {
    fill: "#a3d8cf",
    ink: "#26292b",
};

const KEYS: [SkinKey; 47] = [
    k("a", r(56, 609, 67, 47), KEY, "", "", "", "", ""),
    k("b", r(156, 609, 67, 47), KEY, "", "", "", "", ""),
    k("c", r(256, 609, 67, 47), KEY, "", "", "", "", ""),
    k("d", r(356, 609, 67, 47), KEY, "", "", "", "", ""),
    k("e", r(456, 609, 67, 47), KEY, "", "", "", "", ""),
    k("f", r(556, 609, 67, 47), KEY, "", "", "", "", ""),
    k("plot", r(57, 701, 67, 49), KEY, "PLOT", "", "", "", ""),
    k("symb", r(156, 701, 68, 49), KEY, "SYMB", "", "", "", ""),
    k("num", r(256, 701, 68, 49), KEY, "NUM", "", "", "", ""),
    k("up", r(456, 701, 67, 49), ARROW, "▲", "", "", "", ""),
    k("lib", r(57, 798, 67, 49), KEY, "LIB", "VIEWS", "", "", ""),
    k("var", r(156, 798, 67, 49), KEY, "VAR", "NOTE", "", "", ""),
    k(
        "math",
        r(256, 798, 68, 49),
        KEY,
        "MATH",
        "SKETCH",
        "",
        "",
        "",
    ),
    k("left", r(356, 798, 68, 49), ARROW, "◀", "", "", "", ""),
    k("down", r(456, 798, 68, 49), ARROW, "▼", "", "", "", ""),
    k("right", r(556, 798, 67, 49), ARROW, "▶", "", "", "", ""),
    k(
        "home",
        r(57, 894, 67, 49),
        KEY,
        "HOME",
        "MODES",
        "",
        "A",
        "",
    ),
    k("sin", r(156, 894, 67, 49), KEY, "SIN", "ASIN", "", "B", ""),
    k("cos", r(256, 894, 67, 49), KEY, "COS", "ACOS", "", "C", ""),
    k("tan", r(357, 894, 67, 49), KEY, "TAN", "ATAN", "", "D", ""),
    k("xt", r(456, 894, 68, 49), KEY, "X,T,θ", "x⁻¹", "", "E", ""),
    k("sqrt", r(556, 894, 67, 49), KEY, "√x", "ⁿ√x", "", "F", ""),
    k(
        "enter",
        r(51, 991, 177, 49),
        KEY,
        "ENTER",
        "ANSWER",
        "",
        "",
        "",
    ),
    k(
        "lparen",
        r(256, 991, 68, 49),
        KEY,
        "(",
        "CHARS",
        "",
        "G",
        "",
    ),
    k("rparen", r(357, 991, 68, 49), KEY, ")", "EEX", "", "H", ""),
    k("neg", r(456, 991, 68, 49), KEY, "-x", "ABS", "", "I", ""),
    k("power", r(556, 991, 67, 49), KEY, "xʸ", "x²", "", "J", ""),
    k("alpha", r(57, 1089, 67, 49), KEY, "A…Z", "a…z", "", "", ""),
    k("7", r(169, 1089, 95, 49), KEY, "7", "LIST", "", "K", ""),
    k("8", r(289, 1089, 96, 49), KEY, "8", "{", "", "L", ""),
    k("9", r(409, 1089, 96, 49), KEY, "9", "}", "", "M", ""),
    k("divide", r(530, 1089, 96, 49), KEY, "/", "LOG", "", "N", ""),
    k("shift", r(57, 1187, 67, 49), SHIFT, "", "", "", "", ""),
    k("4", r(169, 1187, 95, 49), KEY, "4", "MATRIX", "", "O", ""),
    k("5", r(289, 1187, 96, 49), KEY, "5", "[", "", "P", ""),
    k("6", r(409, 1187, 97, 49), KEY, "6", "]", "", "Q", ""),
    k(
        "multiply",
        r(531, 1187, 95, 49),
        KEY,
        "*",
        "10ˣ",
        "",
        "R",
        "",
    ),
    k("del", r(57, 1286, 67, 50), KEY, "DEL", "CLEAR", "", "", ""),
    k("1", r(169, 1286, 95, 50), KEY, "1", "NOTEPAD", "", "S", ""),
    k("2", r(289, 1286, 95, 50), KEY, "2", "SPACE", "", "T", ""),
    k("3", r(410, 1286, 95, 50), KEY, "3", "π", "", "U", ""),
    k("minus", r(531, 1286, 96, 50), KEY, "−", "LN", "", "V", ""),
    k(
        "on",
        r(58, 1384, 67, 49),
        KEY,
        "ON",
        "OFF",
        "",
        "",
        "CANCEL",
    ),
    k("0", r(169, 1384, 95, 49), KEY, "0", "PROGRAM", "", "W", ""),
    k("point", r(289, 1384, 95, 49), KEY, "·", ":", "", "X", ""),
    k("comma", r(410, 1384, 96, 49), KEY, ",", ";", "", "Y", ""),
    k("plus", r(531, 1384, 96, 49), KEY, "+", "eˣ", "", "Z", ""),
];

const PANELS: [Panel; 6] = [
    panel(r(0, 0, 676, 1505), 26, 26, "#2b2e30"),
    panel(r(14, 14, 648, 538), 18, 6, "#33373a"),
    panel(r(50, 116, 577, 370), 6, 6, "#1f2224"),
    panel(r(150, 511, 376, 18), 9, 9, "#1d2022"),
    panel(r(24, 562, 628, 918), 8, 8, "#303437"),
    panel(r(24, 860, 628, 620), 0, 8, "#2f5653"),
];

const MARKS: [Mark; 2] = [
    Mark {
        x: 556,
        y: 72,
        size: 34,
        fill: "#e7dca4",
        text: "38G",
    },
    Mark {
        x: 190,
        y: 681,
        size: 17,
        fill: "#8fd5c8",
        text: "SETUP",
    },
];

const LINES: [Line; 4] = [
    Line {
        x1: 90,
        y1: 685,
        x2: 90,
        y2: 675,
        stroke: "#8fd5c8",
    },
    Line {
        x1: 90,
        y1: 675,
        x2: 154,
        y2: 675,
        stroke: "#8fd5c8",
    },
    Line {
        x1: 226,
        y1: 675,
        x2: 290,
        y2: 675,
        stroke: "#8fd5c8",
    },
    Line {
        x1: 290,
        y1: 675,
        x2: 290,
        y2: 685,
        stroke: "#8fd5c8",
    },
];

/// The HP 38G skin.
pub const SKIN: Skin = Skin {
    width: 676,
    height: 1505,
    panels: &PANELS,
    lcd: r(83, 146, 511, 309),
    lcd_fill: "#b7c2a2",
    logo: r(44, 26, 56, 56),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#8fd5c8",
    right_ink: "#8fd5c8",
    alpha_ink: "#d99f62",
    alpha_badge: "#d99f62",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#8fd5c8",
    small: 17,
    keys: &KEYS,
};
