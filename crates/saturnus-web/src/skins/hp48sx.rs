//! HP 48SX skin.
//!
//! Geometry: keys from the keyboard figure on page 26 of the HP 48SX
//! Owner's Manual vol. 1 (literature.hpcalc.org `hp48sx-om-vol1-en.pdf`,
//! PDF page 30, 300 dpi; softkey pitch 142.2 px); the display, bezel and
//! the case above the softkeys from the figure on page 1-9 of the HP 48G
//! Series User's Guide (`hp48gug.pdf`, PDF page 23; the 48SX and 48GX
//! share the case). Labels from the page 26 figure. Colours from the
//! manual's cover photograph: orange left-shift and blue right-shift keys
//! and labels, white menu keys, white alpha letters; a single shifted
//! label is orange.
//!
//! Inferred: the case below the ON row (the figures stop at the keys; 200
//! units, from the 48GX photograph); the exact shades; ATTN under ON is
//! drawn in the right-shift blue as in the figure.

use super::*;

/// Dark key caps with white labels.
const KEY: Cap = Cap {
    fill: "#2a2624",
    ink: "#f2ede3",
};
/// The white menu keys.
const MENU: Cap = Cap {
    fill: "#ebe7dc",
    ink: "#2a2624",
};
/// The orange left-shift key.
const LSHIFT: Cap = Cap {
    fill: "#e07b38",
    ink: "#2a2624",
};
/// The blue right-shift key.
const RSHIFT: Cap = Cap {
    fill: "#4c8fcf",
    ink: "#f2ede3",
};

const KEYS: [SkinKey; 49] = [
    k("a", r(46, 462, 77, 51), MENU, "", "", "", "A", ""),
    k("b", r(145, 462, 77, 51), MENU, "", "", "", "B", ""),
    k("c", r(245, 462, 77, 51), MENU, "", "", "", "C", ""),
    k("d", r(345, 462, 77, 51), MENU, "", "", "", "D", ""),
    k("e", r(446, 462, 76, 51), MENU, "", "", "", "E", ""),
    k("f", r(546, 462, 77, 51), MENU, "", "", "", "F", ""),
    k("mth", r(46, 549, 77, 52), KEY, "MTH", "PRINT", "", "G", ""),
    k("prg", r(145, 549, 77, 52), KEY, "PRG", "I/O", "", "H", ""),
    k("cst", r(245, 549, 77, 52), KEY, "CST", "MODES", "", "I", ""),
    k(
        "var",
        r(345, 549, 77, 52),
        KEY,
        "VAR",
        "MEMORY",
        "",
        "J",
        "",
    ),
    k("up", r(447, 549, 76, 52), KEY, "▲", "LIBRARY", "", "K", ""),
    k("nxt", r(546, 549, 77, 52), KEY, "NXT", "PREV", "", "L", ""),
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
        "→Q",
        "→NUM",
        "O",
        "",
    ),
    k("left", r(346, 639, 77, 52), KEY, "◀", "GRAPH", "", "P", ""),
    k("down", r(445, 639, 77, 52), KEY, "▼", "REVIEW", "", "Q", ""),
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
        "VISIT",
        "Y",
        "",
    ),
    k("eex", r(346, 811, 75, 53), KEY, "EEX", "2D", "3D", "Z", ""),
    k("del", r(446, 811, 77, 53), KEY, "DEL", "PURGE", "", "", ""),
    k(
        "backspace",
        r(547, 811, 77, 53),
        KEY,
        "⬅",
        "DROP",
        "CLR",
        "",
        "",
    ),
    k(
        "alpha",
        r(48, 900, 76, 51),
        KEY,
        "α",
        "USR",
        "ENTRY",
        "",
        "",
    ),
    k("7", r(149, 900, 96, 51), KEY, "7", "SOLVE", "", "", ""),
    k("8", r(274, 900, 96, 51), KEY, "8", "PLOT", "", "", ""),
    k("9", r(400, 900, 96, 51), KEY, "9", "ALGEBRA", "", "", ""),
    k("divide", r(528, 900, 95, 51), KEY, "÷", "( )", "#", "", ""),
    k("leftshift", r(48, 989, 77, 51), LSHIFT, "↰", "", "", "", ""),
    k("4", r(146, 989, 96, 51), KEY, "4", "TIME", "", "", ""),
    k("5", r(273, 989, 96, 51), KEY, "5", "STAT", "", "", ""),
    k("6", r(398, 989, 96, 51), KEY, "6", "UNITS", "", "", ""),
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
    k("1", r(146, 1076, 96, 53), KEY, "1", "RAD", "POLAR", "", ""),
    k("2", r(273, 1076, 96, 53), KEY, "2", "STACK", "ARG", "", ""),
    k("3", r(399, 1076, 96, 53), KEY, "3", "CMD", "MENU", "", ""),
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
        "ATTN",
    ),
    k("0", r(147, 1162, 96, 51), KEY, "0", "=", "→", "", ""),
    k("point", r(273, 1162, 96, 51), KEY, "·", ",", "↵", "", ""),
    k("space", r(399, 1162, 96, 51), KEY, "SPC", "π", "∡", "", ""),
    k("plus", r(527, 1162, 96, 51), KEY, "+", "{ }", "::", "", ""),
];

const PANELS: [Panel; 3] = [
    panel(r(0, 0, 667, 1413), 40, 56, "#2e2a28"),
    panel(r(14, 14, 639, 1385), 30, 44, "#3a3532"),
    panel(r(31, 99, 607, 316), 18, 18, "#211e1c"),
];

const MARKS: [Mark; 2] = [
    Mark {
        x: 182,
        y: 70,
        size: 34,
        fill: "#e8e4da",
        text: "48SX",
    },
    Mark {
        x: 384,
        y: 1051,
        size: 13,
        fill: "#e8e4da",
        text: "LAST",
    },
];

const LINES: [Line; 4] = [
    Line {
        x1: 273,
        y1: 1052,
        x2: 273,
        y2: 1046,
        stroke: "#e8e4da",
    },
    Line {
        x1: 273,
        y1: 1046,
        x2: 362,
        y2: 1046,
        stroke: "#e8e4da",
    },
    Line {
        x1: 406,
        y1: 1046,
        x2: 495,
        y2: 1046,
        stroke: "#e8e4da",
    },
    Line {
        x1: 495,
        y1: 1046,
        x2: 495,
        y2: 1052,
        stroke: "#e8e4da",
    },
];

/// The HP 48SX skin.
pub const SKIN: Skin = Skin {
    width: 667,
    height: 1413,
    panels: &PANELS,
    lcd: r(50, 113, 568, 279),
    lcd_fill: "#b7c2a2",
    logo: r(44, 26, 56, 56),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#f39250",
    right_ink: "#86b6e6",
    alpha_ink: "#ece7dc",
    alpha_badge: "#ece7dc",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#86b6e6",
    small: 17,
    keys: &KEYS,
};
