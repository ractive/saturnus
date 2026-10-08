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
//! The case, the key wells and the plate with its step are the 48SX's,
//! measured from the owner's photographs of his 48SX (see `hp48sx.rs`): the
//! two models share the mould. The owner has no 48GX to photograph, so
//! its colours stay those of the references above.
//!
//! Inferred: the shades; the tiles' exact extent; that the lighter plate
//! ends at the step. The display window is placed so the ROM's six menu
//! labels sit over the six menu keys (see [`super::SOFTKEY_LABEL_PITCH`]);
//! its frame follows. The case ends 74 units below the ON row.

use super::*;

/// Black key caps with light grey labels.
const KEY: Cap = Cap {
    fill: "#191d1f",
    ink: "#e6e9e4",
    well: 4,
};
/// The light grey menu keys.
const MENU: Cap = Cap {
    fill: "#c9cdc7",
    ink: "#191d1f",
    well: 9,
};
/// The purple left-shift key.
const LSHIFT: Cap = Cap {
    fill: "#8c86cc",
    ink: "#191d1f",
    well: 7,
};
/// The green right-shift key.
const RSHIFT: Cap = Cap {
    fill: "#2ba6a2",
    ink: "#191d1f",
    well: 7,
};

const KEYS: [SkinKey; 49] = [
    k("a", r(64, 605, 54, 29), MENU, "", "", "", "A", ""),
    k("b", r(164, 605, 54, 29), MENU, "", "", "", "B", ""),
    k("c", r(264, 605, 54, 29), MENU, "", "", "", "C", ""),
    k("d", r(364, 605, 54, 29), MENU, "", "", "", "D", ""),
    k("e", r(464, 605, 54, 29), MENU, "", "", "", "E", ""),
    k("f", r(564, 605, 54, 29), MENU, "", "", "", "F", ""),
    k(
        "mth",
        r(59, 700, 64, 46),
        KEY,
        "MTH",
        "RAD",
        "POLAR",
        "G",
        "",
    ),
    k("prg", r(159, 700, 64, 46), KEY, "PRG", "", "CHARS", "H", ""),
    k("cst", r(259, 700, 64, 46), KEY, "CST", "", "MODES", "I", ""),
    k(
        "var",
        r(359, 700, 64, 46),
        KEY,
        "VAR",
        "",
        "MEMORY",
        "J",
        "",
    ),
    k("up", r(459, 700, 64, 46), KEY, "▲", "", "STACK", "K", ""),
    k(
        "nxt",
        r(559, 700, 64, 46),
        KEY,
        "NXT",
        "PREV",
        "MENU",
        "L",
        "",
    ),
    k("quote", r(59, 800, 64, 46), KEY, "'", "UP", "HOME", "M", ""),
    k(
        "sto",
        r(159, 800, 64, 46),
        KEY,
        "STO",
        "DEF",
        "RCL",
        "N",
        "",
    ),
    k(
        "eval",
        r(259, 800, 64, 46),
        KEY,
        "EVAL",
        "→NUM",
        "UNDO",
        "O",
        "",
    ),
    k(
        "left",
        r(359, 800, 64, 46),
        KEY,
        "◀",
        "PICTURE",
        "",
        "P",
        "",
    ),
    k("down", r(459, 800, 64, 46), KEY, "▼", "VIEW", "", "Q", ""),
    k("right", r(559, 800, 64, 46), KEY, "▶", "SWAP", "", "R", ""),
    k("sin", r(59, 900, 64, 46), KEY, "SIN", "ASIN", "∂", "S", ""),
    k("cos", r(159, 900, 64, 46), KEY, "COS", "ACOS", "∫", "T", ""),
    k("tan", r(259, 900, 64, 46), KEY, "TAN", "ATAN", "Σ", "U", ""),
    k("sqrt", r(359, 900, 64, 46), KEY, "√x", "x²", "ˣ√y", "V", ""),
    k(
        "power",
        r(459, 900, 64, 46),
        KEY,
        "yˣ",
        "10ˣ",
        "LOG",
        "W",
        "",
    ),
    k("inv", r(559, 900, 64, 46), KEY, "1/x", "eˣ", "LN", "X", ""),
    k(
        "enter",
        r(59, 1000, 164, 46),
        KEY,
        "ENTER",
        "EQUATION",
        "MATRIX",
        "",
        "",
    ),
    k(
        "neg",
        r(259, 1000, 64, 46),
        KEY,
        "+/-",
        "EDIT",
        "CMD",
        "Y",
        "",
    ),
    k(
        "eex",
        r(359, 1000, 64, 46),
        KEY,
        "EEX",
        "PURG",
        "ARG",
        "Z",
        "",
    ),
    k("del", r(459, 1000, 64, 46), KEY, "DEL", "CLEAR", "", "", ""),
    k(
        "backspace",
        r(559, 1000, 64, 46),
        KEY,
        "⬅",
        "DROP",
        "",
        "",
        "",
    ),
    k(
        "alpha",
        r(59, 1100, 64, 46),
        KEY,
        "α",
        "USER",
        "ENTRY",
        "",
        "",
    ),
    k("7", r(172, 1100, 85, 46), KEY, "7", "", "SOLVE", "", ""),
    k("8", r(294, 1100, 85, 46), KEY, "8", "", "PLOT", "", ""),
    k("9", r(416, 1100, 85, 46), KEY, "9", "", "SYMBOLIC", "", ""),
    k("divide", r(538, 1100, 85, 46), KEY, "÷", "( )", "#", "", ""),
    k(
        "leftshift",
        r(62, 1203, 58, 40),
        LSHIFT,
        "↰",
        "",
        "",
        "",
        "",
    ),
    k("4", r(172, 1200, 85, 46), KEY, "4", "", "TIME", "", ""),
    k("5", r(294, 1200, 85, 46), KEY, "5", "", "STAT", "", ""),
    k("6", r(416, 1200, 85, 46), KEY, "6", "", "UNITS", "", ""),
    k(
        "multiply",
        r(538, 1200, 85, 46),
        KEY,
        "×",
        "[ ]",
        "_",
        "",
        "",
    ),
    k(
        "rightshift",
        r(62, 1303, 58, 40),
        RSHIFT,
        "↱",
        "",
        "",
        "",
        "",
    ),
    k("1", r(172, 1300, 85, 46), KEY, "1", "", "I/O", "", ""),
    k("2", r(294, 1300, 85, 46), KEY, "2", "", "LIBRARY", "", ""),
    k("3", r(416, 1300, 85, 46), KEY, "3", "", "EQ LIB", "", ""),
    k(
        "minus",
        r(538, 1300, 85, 46),
        KEY,
        "−",
        "« »",
        "\" \"",
        "",
        "",
    ),
    k(
        "on",
        r(59, 1400, 64, 46),
        KEY,
        "ON",
        "CONT",
        "OFF",
        "",
        "CANCEL",
    ),
    k("0", r(172, 1400, 85, 46), KEY, "0", "=", "→", "", ""),
    k("point", r(294, 1400, 85, 46), KEY, "·", ",", "↵", "", ""),
    k("space", r(416, 1400, 85, 46), KEY, "SPC", "π", "∡", "", ""),
    k("plus", r(538, 1400, 85, 46), KEY, "+", "{ }", "::", "", ""),
];

const PANELS: [Panel; 14] = [
    panel(r(0, 0, 685, 1520), 44, 44, "#2c3233"),
    sunk(r(20, 22, 645, 1476), 26, 30, "#3e4544"),
    sunk(r(20, 22, 645, 533), 26, 0, "#556260"),
    panel(r(154, 1060, 119, 97), 3, 3, "#4a5351"),
    panel(r(276, 1060, 119, 97), 3, 3, "#4a5351"),
    panel(r(398, 1060, 119, 97), 3, 3, "#4a5351"),
    panel(r(154, 1160, 119, 97), 3, 3, "#4a5351"),
    panel(r(276, 1160, 119, 97), 3, 3, "#4a5351"),
    panel(r(398, 1160, 119, 97), 3, 3, "#4a5351"),
    panel(r(154, 1260, 119, 97), 3, 3, "#4a5351"),
    panel(r(276, 1260, 119, 97), 3, 3, "#4a5351"),
    panel(r(398, 1260, 119, 97), 3, 3, "#4a5351"),
    panel(r(32, 1260, 119, 97), 3, 3, "#4a5351"),
    panel(r(31, 137, 623, 371), 10, 10, "#252b2a"),
];

const MARKS: [Mark; 1] = [Mark {
    x: 112,
    y: 127,
    size: 24,
    fill: "#e8e4da",
    text: "48GX",
    italic: true,
}];

/// The step at the end of the lighter plate.
const LINES: [Line; 1] = [Line {
    x1: 21,
    y1: 555,
    x2: 664,
    y2: 555,
    stroke: "#2f3635",
}];

/// The HP 48GX skin.
pub const SKIN: Skin = Skin {
    width: 685,
    height: 1520,
    panels: &PANELS,
    lcd: r(43, 155, 595, 327),
    lcd_fill: "#b7c2a2",
    logo: r(66, 42, 48, 48),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#a99fe6",
    right_ink: "#43c6b5",
    alpha_ink: "#dfe3dd",
    alpha_badge: "#dfe3dd",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#dfe3dd",
    small: 19,
    well_fill: "#0c0f10",
    round: 16,
    // The 48's textured plastic (the 48SX photo; same mould).
    texture: 10,
    keys: &KEYS,
};
