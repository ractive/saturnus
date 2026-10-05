//! HP 49G skin.
//!
//! Geometry and labels: figure 1.1, "HP 49G key map", page 1-2 of the HP
//! 49G User's Manual (`hp49g-um-en.pdf`, PDF page 16, 400 dpi; softkey
//! pitch 131.4 px). Label colours from the manual's text (page 1-3:
//! left-shift labels blue, right-shift labels red, alpha letters white on
//! green). Case and key colours from the photograph "HP49G.jpg" (Wikimedia
//! Commons): light metallic blue case, black display surround, dark green
//! menu, number and operator keys, beige function keys, a teal ALPHA, a
//! blue left shift and a red right shift.
//!
//! Inferred: the shades; the case outline is a rounded rectangle (the real
//! case has side grips near the top); the small glyphs printed around the
//! cursor pad (page and line jumps) are left off; the case ends 56 units
//! below the ENTER row. The display window is placed so the ROM's six menu
//! labels sit over the six menu keys (see [`super::SOFTKEY_LABEL_PITCH`]);
//! the black surround follows.

use super::*;

/// The dark green menu, number and operator keys, white labels.
const DARK: Cap = Cap {
    fill: "#34483f",
    ink: "#f4f6f2",
};
/// The beige function keys, dark labels.
const BEIGE: Cap = Cap {
    fill: "#bcb69d",
    ink: "#1f2b28",
};
/// The dark teal cursor keys.
const ARROW: Cap = Cap {
    fill: "#2c4b48",
    ink: "#e9eeec",
};
/// ALPHA.
const ALPHA: Cap = Cap {
    fill: "#2f9d91",
    ink: "#f4f6f2",
};
/// The blue left-shift key.
const LSHIFT: Cap = Cap {
    fill: "#3d6fc6",
    ink: "#f4f6f2",
};
/// The red right-shift key.
const RSHIFT: Cap = Cap {
    fill: "#d9443a",
    ink: "#f4f6f2",
};

const KEYS: [SkinKey; 51] = [
    k("a", r(49, 569, 75, 39), DARK, "F1", "Y=", "", "A", ""),
    k("b", r(148, 569, 75, 39), DARK, "F2", "WIN", "", "B", ""),
    k("c", r(249, 569, 75, 39), DARK, "F3", "GRAPH", "", "C", ""),
    k("d", r(349, 569, 75, 39), DARK, "F4", "2D/3D", "", "D", ""),
    k("e", r(449, 569, 75, 39), DARK, "F5", "TBLSET", "", "E", ""),
    k("f", r(549, 569, 75, 39), DARK, "F6", "TABLE", "", "F", ""),
    k(
        "apps",
        r(62, 667, 89, 46),
        BEIGE,
        "APPS",
        "FILES",
        "BEGIN",
        "G",
        "",
    ),
    k(
        "mode",
        r(177, 667, 89, 46),
        BEIGE,
        "MODE",
        "CUSTOM",
        "END",
        "H",
        "",
    ),
    k(
        "tool",
        r(291, 667, 89, 46),
        BEIGE,
        "TOOL",
        "i",
        "|",
        "I",
        "",
    ),
    k(
        "var",
        r(62, 744, 89, 46),
        BEIGE,
        "VAR",
        "UPDIR",
        "COPY",
        "J",
        "",
    ),
    k(
        "sto",
        r(177, 744, 89, 46),
        BEIGE,
        "STO▶",
        "RCL",
        "CUT",
        "K",
        "",
    ),
    k(
        "nxt",
        r(291, 744, 89, 46),
        BEIGE,
        "NXT",
        "PREV",
        "PASTE",
        "L",
        "",
    ),
    k(
        "hist",
        r(62, 852, 89, 46),
        BEIGE,
        "HIST",
        "CMD",
        "UNDO",
        "M",
        "",
    ),
    k(
        "cat",
        r(177, 852, 89, 46),
        BEIGE,
        "CAT",
        "PRG",
        "CHARS",
        "N",
        "",
    ),
    k(
        "eqw",
        r(291, 852, 89, 46),
        BEIGE,
        "EQW",
        "MTRW",
        "'",
        "O",
        "",
    ),
    k(
        "symb",
        r(406, 852, 89, 46),
        BEIGE,
        "SYMB",
        "MTH",
        "EVAL",
        "P",
        "",
    ),
    k(
        "backspace",
        r(521, 852, 89, 46),
        BEIGE,
        "⬅",
        "DEL",
        "CLEAR",
        "",
        "",
    ),
    k(
        "power",
        r(62, 928, 89, 46),
        BEIGE,
        "yˣ",
        "eˣ",
        "LN",
        "Q",
        "",
    ),
    k(
        "sqrt",
        r(177, 928, 89, 46),
        BEIGE,
        "√x",
        "x²",
        "ˣ√y",
        "R",
        "",
    ),
    k(
        "sin",
        r(291, 928, 89, 46),
        BEIGE,
        "SIN",
        "ASIN",
        "Σ",
        "S",
        "",
    ),
    k(
        "cos",
        r(406, 928, 89, 46),
        BEIGE,
        "COS",
        "ACOS",
        "∂",
        "T",
        "",
    ),
    k(
        "tan",
        r(521, 928, 89, 46),
        BEIGE,
        "TAN",
        "ATAN",
        "∫",
        "U",
        "",
    ),
    k(
        "eex",
        r(62, 1005, 89, 56),
        BEIGE,
        "EEX",
        "10ˣ",
        "LOG",
        "V",
        "",
    ),
    k("neg", r(177, 1005, 90, 56), BEIGE, "+/−", "≠", "=", "W", ""),
    k("x", r(291, 1005, 90, 56), BEIGE, "X", "≤", "<", "X", ""),
    k("inv", r(406, 1005, 89, 56), BEIGE, "1/x", "≥", ">", "Y", ""),
    k(
        "divide",
        r(520, 1005, 90, 56),
        DARK,
        "÷",
        "ABS",
        "ARG",
        "Z",
        "",
    ),
    k(
        "alpha",
        r(62, 1090, 89, 55),
        ALPHA,
        "ALPHA",
        "USER",
        "ENTRY",
        "",
        "",
    ),
    k(
        "7",
        r(177, 1090, 90, 55),
        DARK,
        "7",
        "S.SLV",
        "NUM.SLV",
        "",
        "",
    ),
    k(
        "8",
        r(291, 1090, 90, 55),
        DARK,
        "8",
        "EXP&LN",
        "TRIG",
        "",
        "",
    ),
    k(
        "9",
        r(406, 1090, 89, 55),
        DARK,
        "9",
        "FINANCE",
        "TIME",
        "",
        "",
    ),
    k(
        "multiply",
        r(520, 1090, 90, 55),
        DARK,
        "×",
        "[ ]",
        "\" \"",
        "",
        "",
    ),
    k(
        "leftshift",
        r(62, 1174, 89, 55),
        LSHIFT,
        "↰",
        "",
        "",
        "",
        "",
    ),
    k("4", r(177, 1174, 90, 55), DARK, "4", "CALC", "ALG", "", ""),
    k(
        "5",
        r(291, 1174, 90, 55),
        DARK,
        "5",
        "MATRICES",
        "STAT",
        "",
        "",
    ),
    k(
        "6",
        r(406, 1174, 89, 55),
        DARK,
        "6",
        "CONVERT",
        "UNITS",
        "",
        "",
    ),
    k("minus", r(520, 1174, 90, 55), DARK, "−", "( )", "_", "", ""),
    k(
        "rightshift",
        r(62, 1259, 89, 56),
        RSHIFT,
        "↱",
        "",
        "",
        "",
        "",
    ),
    k(
        "1",
        r(177, 1259, 90, 56),
        DARK,
        "1",
        "ARITH",
        "CMPLX",
        "",
        "",
    ),
    k("2", r(291, 1259, 90, 56), DARK, "2", "DEF", "LIB", "", ""),
    k("3", r(406, 1259, 89, 56), DARK, "3", "#", "BASE", "", ""),
    k(
        "plus",
        r(520, 1259, 90, 56),
        DARK,
        "+",
        "{ }",
        "« »",
        "",
        "",
    ),
    k(
        "on",
        r(62, 1343, 89, 56),
        BEIGE,
        "ON",
        "CONT",
        "OFF",
        "",
        "CANCEL",
    ),
    k("0", r(177, 1343, 90, 56), DARK, "0", "∞", "→", "", ""),
    k("point", r(291, 1343, 90, 56), DARK, "·", "::", "↵", "", ""),
    k("space", r(406, 1343, 89, 56), DARK, "SPC", "π", ",", "", ""),
    k(
        "enter",
        r(520, 1343, 90, 56),
        DARK,
        "ENTER",
        "ANS",
        "→NUM",
        "",
        "",
    ),
    SkinKey {
        shape: Shape::Up,
        ..k("up", r(463, 632, 88, 49), ARROW, "▲", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Left,
        ..k("left", r(403, 706, 84, 53), ARROW, "◀", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Right,
        ..k("right", r(528, 706, 84, 53), ARROW, "▶", "", "", "", "")
    },
    SkinKey {
        shape: Shape::Down,
        ..k("down", r(463, 783, 88, 50), ARROW, "▼", "", "", "", "")
    },
];

const PANELS: [Panel; 3] = [
    panel(r(0, 0, 671, 1455), 70, 110, "#8db1bd"),
    panel(r(8, 8, 655, 1439), 64, 102, "#9cbec9"),
    panel(r(26, 62, 619, 482), 40, 110, "#121517"),
];

const MARKS: [Mark; 1] = [Mark {
    x: 335,
    y: 490,
    size: 34,
    fill: "#e9eef0",
    text: "49G",
}];

const LINES: [Line; 5] = [
    Line {
        x1: 166,
        y1: 464,
        x2: 166,
        y2: 506,
        stroke: "#c9d2d6",
    },
    Line {
        x1: 251,
        y1: 464,
        x2: 251,
        y2: 506,
        stroke: "#c9d2d6",
    },
    Line {
        x1: 336,
        y1: 464,
        x2: 336,
        y2: 506,
        stroke: "#c9d2d6",
    },
    Line {
        x1: 420,
        y1: 464,
        x2: 420,
        y2: 506,
        stroke: "#c9d2d6",
    },
    Line {
        x1: 504,
        y1: 464,
        x2: 504,
        y2: 506,
        stroke: "#c9d2d6",
    },
];

/// The HP 49G skin.
pub const SKIN: Skin = Skin {
    width: 671,
    height: 1455,
    panels: &PANELS,
    lcd: r(39, 130, 595, 327),
    lcd_fill: "#b7c2a2",
    logo: r(313, 81, 44, 44),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#2a56a6",
    right_ink: "#c1312a",
    alpha_ink: "#ffffff",
    alpha_badge: "#2f9d91",
    alpha_style: AlphaStyle::Badge,
    below_ink: "#5a6d74",
    small: 15,
    keys: &KEYS,
};
