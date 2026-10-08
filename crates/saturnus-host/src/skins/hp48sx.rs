//! HP 48SX skin.
//!
//! Geometry and colours: the owner's straight-on photographs of his own
//! HP 48SX (October 2026, not in the repository; menu-key pitch 125 px on
//! a 1500 x 2000 frame). Measured from them: the case (685 x 1532 units),
//! the key wells (72 x 54 units at a pitch of 100 both ways, 93 wide at a
//! pitch of 122 in the number columns) with the caps inset in them, the
//! light plate around the display down to the step and the end of the
//! plate under the menu keys, the label tabs behind PRINT, I/O, MODES,
//! MEMORY and LIBRARY and behind the labels of 4-9, the LAST bracket.
//! Labels from the keyboard figure on page 26 of the HP 48SX Owner's
//! Manual vol. 1 (`hp48sx-om-vol1-en.pdf`, PDF page 30), which the
//! photographs confirm.
//!
//! Differs from the unit on purpose: the display window is placed and
//! sized so the ROM's six menu labels sit over the six menu keys (see
//! [`super::SOFTKEY_LABEL_PITCH`]), which makes it 595 units wide where
//! the real glass is 543, so the frame and the rim beside it are thinner;
//! the case ends 74 units below the ON row (82 on the unit) to keep the
//! bottom margin near the side margin; the photographs' shades are toned
//! down from their sunlit exposure. The logo plate holds our logo.

use super::*;

/// Dark key caps with cream-white labels.
const KEY: Cap = Cap {
    fill: "#2a2523",
    ink: "#efe8d3",
    well: 4,
};
/// The cream menu keys, small caps in wide wells.
const MENU: Cap = Cap {
    fill: "#ddd3b5",
    ink: "#2a2523",
    well: 9,
};
/// The orange left-shift key.
const LSHIFT: Cap = Cap {
    fill: "#e2893c",
    ink: "#221c1a",
    well: 7,
};
/// The light blue right-shift key.
const RSHIFT: Cap = Cap {
    fill: "#8ebfd2",
    ink: "#221c1a",
    well: 7,
};

const KEYS: [SkinKey; 49] = [
    k("a", r(64, 605, 54, 29), MENU, "", "", "", "A", ""),
    k("b", r(164, 605, 54, 29), MENU, "", "", "", "B", ""),
    k("c", r(264, 605, 54, 29), MENU, "", "", "", "C", ""),
    k("d", r(364, 605, 54, 29), MENU, "", "", "", "D", ""),
    k("e", r(464, 605, 54, 29), MENU, "", "", "", "E", ""),
    k("f", r(564, 605, 54, 29), MENU, "", "", "", "F", ""),
    k("mth", r(59, 700, 64, 46), KEY, "MTH", "PRINT", "", "G", ""),
    k("prg", r(159, 700, 64, 46), KEY, "PRG", "I/O", "", "H", ""),
    k("cst", r(259, 700, 64, 46), KEY, "CST", "MODES", "", "I", ""),
    k(
        "var",
        r(359, 700, 64, 46),
        KEY,
        "VAR",
        "MEMORY",
        "",
        "J",
        "",
    ),
    k("up", r(459, 700, 64, 46), KEY, "▲", "LIBRARY", "", "K", ""),
    k("nxt", r(559, 700, 64, 46), KEY, "NXT", "PREV", "", "L", ""),
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
        "→Q",
        "→NUM",
        "O",
        "",
    ),
    k("left", r(359, 800, 64, 46), KEY, "◀", "GRAPH", "", "P", ""),
    k("down", r(459, 800, 64, 46), KEY, "▼", "REVIEW", "", "Q", ""),
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
        "VISIT",
        "Y",
        "",
    ),
    k("eex", r(359, 1000, 64, 46), KEY, "EEX", "2D", "3D", "Z", ""),
    k("del", r(459, 1000, 64, 46), KEY, "DEL", "PURGE", "", "", ""),
    k(
        "backspace",
        r(559, 1000, 64, 46),
        KEY,
        "⬅",
        "DROP",
        "CLR",
        "",
        "",
    ),
    k(
        "alpha",
        r(59, 1100, 64, 46),
        KEY,
        "α",
        "USR",
        "ENTRY",
        "",
        "",
    ),
    k("7", r(172, 1100, 85, 46), KEY, "7", "SOLVE", "", "", ""),
    k("8", r(294, 1100, 85, 46), KEY, "8", "PLOT", "", "", ""),
    k("9", r(416, 1100, 85, 46), KEY, "9", "ALGEBRA", "", "", ""),
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
    k("4", r(172, 1200, 85, 46), KEY, "4", "TIME", "", "", ""),
    k("5", r(294, 1200, 85, 46), KEY, "5", "STAT", "", "", ""),
    k("6", r(416, 1200, 85, 46), KEY, "6", "UNITS", "", "", ""),
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
    k("1", r(172, 1300, 85, 46), KEY, "1", "RAD", "POLAR", "", ""),
    k("2", r(294, 1300, 85, 46), KEY, "2", "STACK", "ARG", "", ""),
    k("3", r(416, 1300, 85, 46), KEY, "3", "CMD", "MENU", "", ""),
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
        "ATTN",
    ),
    k("0", r(172, 1400, 85, 46), KEY, "0", "=", "→", "", ""),
    k("point", r(294, 1400, 85, 46), KEY, "·", ",", "↵", "", ""),
    k("space", r(416, 1400, 85, 46), KEY, "SPC", "π", "∡", "", ""),
    k("plus", r(538, 1400, 85, 46), KEY, "+", "{ }", "::", "", ""),
];

const PANELS: [Panel; 16] = [
    // The rim, the keyboard plate, the light plate around the display down
    // to the end under the menu keys, its lower part below the step.
    panel(r(0, 0, 685, 1520), 44, 44, "#2b2832"),
    sunk(r(20, 22, 645, 1476), 26, 30, "#2c2725"),
    sunk(r(20, 22, 645, 640), 26, 0, "#7d756b"),
    panel(r(20, 555, 645, 107), 0, 0, "#6d655c"),
    // The display's frame.
    panel(r(31, 137, 623, 371), 10, 10, "#262120"),
    // The label tabs.
    panel(r(48, 671, 86, 20), 4, 4, "#3d3836"),
    panel(r(148, 671, 86, 20), 4, 4, "#3d3836"),
    panel(r(248, 671, 86, 20), 4, 4, "#3d3836"),
    panel(r(348, 671, 86, 20), 4, 4, "#3d3836"),
    panel(r(448, 671, 86, 20), 4, 4, "#3d3836"),
    panel(r(168, 1070, 93, 19), 4, 4, "#3d3836"),
    panel(r(290, 1070, 93, 19), 4, 4, "#3d3836"),
    panel(r(412, 1070, 93, 19), 4, 4, "#3d3836"),
    panel(r(168, 1170, 93, 19), 4, 4, "#3d3836"),
    panel(r(290, 1170, 93, 19), 4, 4, "#3d3836"),
    panel(r(412, 1170, 93, 19), 4, 4, "#3d3836"),
];

const MARKS: [Mark; 3] = [
    Mark {
        x: 112,
        y: 127,
        size: 24,
        fill: "#f4f1e6",
        text: "48SX",
        italic: true,
    },
    Mark {
        x: 486,
        y: 126,
        size: 18,
        fill: "#f4f1e6",
        text: "SCIENTIFIC EXPANDABLE",
        italic: true,
    },
    Mark {
        x: 397,
        y: 1267,
        size: 13,
        fill: "#e8e4da",
        text: "LAST",
        italic: false,
    },
];

const LINES: [Line; 5] = [
    // The step in the light plate.
    Line {
        x1: 21,
        y1: 555,
        x2: 664,
        y2: 555,
        stroke: "#4f4841",
    },
    // The LAST bracket over 2 and 3.
    Line {
        x1: 291,
        y1: 1270,
        x2: 291,
        y2: 1262,
        stroke: "#e8e4da",
    },
    Line {
        x1: 291,
        y1: 1262,
        x2: 374,
        y2: 1262,
        stroke: "#e8e4da",
    },
    Line {
        x1: 420,
        y1: 1262,
        x2: 504,
        y2: 1262,
        stroke: "#e8e4da",
    },
    Line {
        x1: 504,
        y1: 1262,
        x2: 504,
        y2: 1270,
        stroke: "#e8e4da",
    },
];

/// The HP 48SX skin.
pub const SKIN: Skin = Skin {
    width: 685,
    height: 1520,
    panels: &PANELS,
    lcd: r(43, 155, 595, 327),
    lcd_fill: "#b7c2a2",
    logo: r(66, 42, 48, 48),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#ee9c68",
    right_ink: "#93bde0",
    alpha_ink: "#f2ecd6",
    alpha_badge: "#f2ecd6",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#e3e6da",
    small: 19,
    well_fill: "#0f0c0b",
    round: 16,
    // The 48's textured plastic (photo).
    texture: 10,
    keys: &KEYS,
};
