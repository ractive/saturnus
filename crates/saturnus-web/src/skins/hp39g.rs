//! HP 39G and HP 40G skin.
//!
//! Geometry and labels: the keyboard figure on page 1-3 of the HP 39G/40G
//! Graphing Calculator User's Guide (`hp39g40g-ug-en.pdf`, PDF page 13,
//! 400 dpi; softkey pitch 92.6 px). Colours from Thimet's photograph of an
//! HP-39G (thimet.de, CalcCollection, `HP-39G-M.JPG`): navy case, light
//! grey function keys with navy labels, dark number and operator keys with
//! orange labels, white shifted labels, orange alpha letters.
//!
//! Each alpha letter is printed below and to the right of its key, so it
//! reads as belonging to the row below; the letters here are the ones the
//! ROM types (wiki: hardware/hp39g-40g "Alpha letters"), and the figure
//! prints them in exactly these places. Thimet: "the orange ALPHA letters
//! below [belong] to the key above but the blue SHIFT symbols to the key
//! below".
//!
//! The 40G is drawn as the 39G with its own name: the user's guide shows
//! only the 39G and says the 40G differs in a CAS menu label, which the
//! ROM draws on the display itself (inferred: no 40G figure or photo was
//! found).
//!
//! Inferred: the shades; the shifted-label colour (the photo shows near
//! white, Thimet calls them blue).

use super::*;

/// The light grey function keys, navy labels.
const GREY: Cap = Cap {
    fill: "#b9bec3",
    ink: "#1d2b48",
};
/// The dark number and operator keys, orange labels.
const DARK: Cap = Cap {
    fill: "#1c2333",
    ink: "#f39a45",
};
/// The navy cursor keys.
const ARROW: Cap = Cap {
    fill: "#18223a",
    ink: "#c9ced4",
};
/// ALPHA: grey with an orange label.
const ALPHA: Cap = Cap {
    fill: "#b9bec3",
    ink: "#e07a2a",
};
/// SHIFT: grey with a light blue label.
const SHIFT: Cap = Cap {
    fill: "#b9bec3",
    ink: "#3d7fc2",
};

const KEYS: [SkinKey; 51] = [
    k("a", r(49, 552, 73, 39), GREY, "", "", "", "", ""),
    k("b", r(149, 552, 73, 39), GREY, "", "", "", "", ""),
    k("c", r(248, 552, 73, 39), GREY, "", "", "", "", ""),
    k("d", r(349, 552, 73, 39), GREY, "", "", "", "", ""),
    k("e", r(448, 552, 73, 39), GREY, "", "", "", "", ""),
    k("f", r(549, 552, 73, 39), GREY, "", "", "", "", ""),
    k("symb", r(62, 650, 90, 48), GREY, "SYMB", "", "", "", ""),
    k("plot", r(176, 650, 90, 48), GREY, "PLOT", "", "", "", ""),
    k("num", r(289, 650, 90, 48), GREY, "NUM", "", "", "", ""),
    k(
        "home",
        r(62, 726, 90, 48),
        GREY,
        "HOME",
        "MODES",
        "",
        "",
        "",
    ),
    k(
        "aplet",
        r(174, 726, 90, 48),
        GREY,
        "APLET",
        "NOTE",
        "",
        "",
        "",
    ),
    k(
        "views",
        r(289, 726, 90, 48),
        GREY,
        "VIEWS",
        "SKETCH",
        "",
        "",
        "",
    ),
    k(
        "vars",
        r(62, 833, 90, 49),
        GREY,
        "VARS",
        "CHARS",
        "",
        "A",
        "",
    ),
    k(
        "math",
        r(175, 833, 90, 49),
        GREY,
        "MATH",
        "CMDS",
        "",
        "B",
        "",
    ),
    k("ddx", r(289, 833, 90, 49), GREY, "d/dx", "∫", "", "C", ""),
    k("xt", r(403, 833, 90, 49), GREY, "X,T,θ", "EEX", "", "D", ""),
    k("del", r(517, 833, 90, 49), GREY, "DEL", "CLEAR", "", "", ""),
    k("sin", r(62, 909, 90, 48), GREY, "SIN", "ASIN", "", "E", ""),
    k("cos", r(175, 909, 90, 48), GREY, "COS", "ACOS", "", "F", ""),
    k("tan", r(289, 909, 90, 48), GREY, "TAN", "ATAN", "", "G", ""),
    k("ln", r(403, 909, 90, 48), GREY, "ln", "eˣ", "", "H", ""),
    k("log", r(517, 909, 90, 48), GREY, "log", "10ˣ", "", "I", ""),
    k("square", r(62, 985, 90, 57), GREY, "x²", "√", "", "J", ""),
    k("power", r(175, 985, 90, 57), GREY, "xʸ", "ⁿ√", "", "K", ""),
    k("lparen", r(289, 985, 90, 57), GREY, "(", "ABS", "", "L", ""),
    k("rparen", r(403, 985, 90, 57), GREY, ")", "ARG", "", "M", ""),
    k("divide", r(517, 985, 90, 57), DARK, "÷", "x⁻¹", "", "N", ""),
    k(
        "comma",
        r(62, 1069, 90, 57),
        GREY,
        ",",
        "MEMORY",
        "",
        "O",
        "",
    ),
    k("7", r(175, 1069, 90, 57), DARK, "7", "LIST", "", "P", ""),
    k("8", r(289, 1069, 90, 57), DARK, "8", "{", "", "Q", ""),
    k("9", r(403, 1069, 90, 57), DARK, "9", "}", "", "R", ""),
    k(
        "multiply",
        r(517, 1069, 90, 57),
        DARK,
        "×",
        "!",
        "",
        "S",
        "",
    ),
    k(
        "alpha",
        r(62, 1152, 90, 57),
        ALPHA,
        "ALPHA",
        "alpha",
        "",
        "",
        "",
    ),
    k("4", r(175, 1152, 90, 57), DARK, "4", "MATRIX", "", "T", ""),
    k("5", r(289, 1152, 90, 57), DARK, "5", "[", "", "U", ""),
    k("6", r(403, 1152, 90, 57), DARK, "6", "]", "", "V", ""),
    k("minus", r(517, 1152, 90, 57), DARK, "−", "Δ", "", "W", ""),
    k("shift", r(62, 1237, 90, 57), SHIFT, "SHIFT", "", "", "", ""),
    k("1", r(175, 1237, 90, 57), DARK, "1", "PROGRM", "", "X", ""),
    k("2", r(289, 1237, 90, 57), DARK, "2", "SYNTAX", "", "Y", ""),
    k("3", r(403, 1237, 90, 57), DARK, "3", "π", "", "Z", ""),
    k(
        "plus",
        r(517, 1237, 90, 57),
        DARK,
        "+",
        "Σ",
        "",
        "SPACE",
        "",
    ),
    k(
        "on",
        r(62, 1321, 90, 57),
        GREY,
        "ON",
        "OFF",
        "",
        "",
        "CANCEL",
    ),
    k("0", r(175, 1321, 90, 57), DARK, "0", "NOTEPAD", "", "θ", ""),
    k("point", r(289, 1321, 90, 57), DARK, "·", "=", "", ":", ""),
    k("neg", r(403, 1321, 90, 57), DARK, "(-)", "AND", "", ";", ""),
    k(
        "enter",
        r(517, 1321, 90, 57),
        DARK,
        "ENTER",
        "ANS",
        "",
        "",
        "",
    ),
    SkinKey {
        shape: Shape::Up,
        ..k("up", r(461, 616, 87, 51), ARROW, "▲", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Left,
        ..k("left", r(402, 689, 81, 54), ARROW, "◀", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Right,
        ..k("right", r(527, 688, 81, 55), ARROW, "▶", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Down,
        ..k("down", r(461, 766, 87, 51), ARROW, "▼", "", "", "", "")
    },
];

const PANELS: [Panel; 3] = [
    panel(r(0, 0, 673, 1472), 60, 140, "#21375a"),
    panel(r(8, 8, 657, 1456), 54, 132, "#28416a"),
    panel(r(33, 49, 602, 476), 36, 90, "#1b2a47"),
];

const LINES: [Line; 4] = [
    Line {
        x1: 107,
        y1: 634,
        x2: 107,
        y2: 624,
        stroke: "#e3e9f2",
    },
    Line {
        x1: 107,
        y1: 624,
        x2: 184,
        y2: 624,
        stroke: "#e3e9f2",
    },
    Line {
        x1: 256,
        y1: 624,
        x2: 334,
        y2: 624,
        stroke: "#e3e9f2",
    },
    Line {
        x1: 334,
        y1: 624,
        x2: 334,
        y2: 634,
        stroke: "#e3e9f2",
    },
];

const MARKS_39G: [Mark; 2] = [
    Mark {
        x: 336,
        y: 489,
        size: 34,
        fill: "#e9edf3",
        text: "39G",
    },
    Mark {
        x: 220,
        y: 630,
        size: 17,
        fill: "#e3e9f2",
        text: "SETUP",
    },
];

const MARKS_40G: [Mark; 2] = [
    Mark {
        x: 336,
        y: 489,
        size: 34,
        fill: "#e9edf3",
        text: "40G",
    },
    Mark {
        x: 220,
        y: 630,
        size: 17,
        fill: "#e3e9f2",
        text: "SETUP",
    },
];

/// The HP 39G skin.
pub const SKIN_39G: Skin = Skin {
    width: 673,
    height: 1472,
    panels: &PANELS,
    lcd: r(83, 119, 501, 306),
    lcd_fill: "#b7c2a2",
    logo: r(312, 56, 48, 48),
    marks: &MARKS_39G,
    lines: &LINES,
    left_ink: "#e3e9f2",
    right_ink: "#e3e9f2",
    alpha_ink: "#f39a45",
    alpha_badge: "#f39a45",
    alpha_style: AlphaStyle::Below,
    below_ink: "#e3e9f2",
    small: 15,
    keys: &KEYS,
};

/// The HP 40G skin.
pub const SKIN_40G: Skin = Skin {
    width: 673,
    height: 1472,
    panels: &PANELS,
    lcd: r(83, 119, 501, 306),
    lcd_fill: "#b7c2a2",
    logo: r(312, 56, 48, 48),
    marks: &MARKS_40G,
    lines: &LINES,
    left_ink: "#e3e9f2",
    right_ink: "#e3e9f2",
    alpha_ink: "#f39a45",
    alpha_badge: "#f39a45",
    alpha_style: AlphaStyle::Below,
    below_ink: "#e3e9f2",
    small: 15,
    keys: &KEYS,
};
