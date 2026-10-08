//! HP 39G and HP 40G skin.
//!
//! Geometry and labels: the keyboard figure on page 1-3 of the HP 39G/40G
//! Graphing Calculator User's Guide (`hp39g40g-ug-en.pdf`, PDF page 13,
//! 400 dpi; softkey pitch 92.6 px). Colours follow the owner's reference
//! photo of an HP 39g+ (2026-10-08), whose keyboard is the 39G's: a
//! metallic blue case, a black display surround, light blue menu keys,
//! cream function keys with black labels, dark blue number and operator
//! keys (ON and ENTER among them) with white labels, a black ALPHA key, an
//! orange SHIFT key, chrome cursor keys (round on the photo; their shapes
//! here are the figure's), orange-red shifted labels and SETUP bracket,
//! white CANCEL.
//!
//! Each alpha letter is printed below and to the right of its key, so it
//! reads as belonging to the row below; the letters here are the ones the
//! ROM types (wiki: hardware/hp39g-40g "Alpha letters"), and the figure
//! prints them in exactly these places.
//!
//! The 40G is drawn as the 39G with its own name: the user's guide shows
//! only the 39G and says the 40G differs in a CAS menu label, which the
//! ROM draws on the display itself (inferred: no 40G figure or photo was
//! found).
//!
//! Inferred: the case ends 57 units below the ON row. The display window is
//! placed so the ROM's six menu labels sit over the six menu keys (see
//! [`super::SOFTKEY_LABEL_PITCH`]); the surround follows. Shifted from the
//! photo so every label keeps a contrast of at least 4.5:1 (WCAG AA): the
//! case is the photo's shaded blue, not its lit mid blue (orange-red cannot
//! reach 4.5:1 on that); the shifted labels are a lighter coral; the alpha
//! letters are light, not the photo's dark navy, on the darker case; the
//! number keys are darker than the photo's so they stand off the case; the
//! SHIFT key is a deeper orange so its white label reads.

use super::*;

/// The light blue menu keys, unlabelled.
const MENU: Cap = Cap {
    fill: "#c6d7ec",
    ink: "#1d2b48",
    well: 0,
};
/// The cream function keys, black labels.
const CREAM: Cap = Cap {
    fill: "#f1eee5",
    ink: "#1e1b18",
    well: 0,
};
/// The dark blue number and operator keys, white labels.
const NAVY: Cap = Cap {
    fill: "#142a5e",
    ink: "#ffffff",
    well: 0,
};
/// The chrome cursor keys.
const ARROW: Cap = Cap {
    fill: "#a9adb0",
    ink: "#2a2d33",
    well: 0,
};
/// ALPHA: black with a white label.
const ALPHA: Cap = Cap {
    fill: "#1f1e22",
    ink: "#ffffff",
    well: 0,
};
/// SHIFT: orange with a white label.
const SHIFT: Cap = Cap {
    fill: "#c64d0a",
    ink: "#ffffff",
    well: 0,
};

const KEYS: [SkinKey; 51] = [
    k("a", r(49, 552, 73, 39), MENU, "", "", "", "", ""),
    k("b", r(149, 552, 73, 39), MENU, "", "", "", "", ""),
    k("c", r(248, 552, 73, 39), MENU, "", "", "", "", ""),
    k("d", r(349, 552, 73, 39), MENU, "", "", "", "", ""),
    k("e", r(448, 552, 73, 39), MENU, "", "", "", "", ""),
    k("f", r(549, 552, 73, 39), MENU, "", "", "", "", ""),
    k("symb", r(62, 650, 90, 48), CREAM, "SYMB", "", "", "", ""),
    k("plot", r(176, 650, 90, 48), CREAM, "PLOT", "", "", "", ""),
    k("num", r(289, 650, 90, 48), CREAM, "NUM", "", "", "", ""),
    k(
        "home",
        r(62, 726, 90, 48),
        CREAM,
        "HOME",
        "MODES",
        "",
        "",
        "",
    ),
    k(
        "aplet",
        r(174, 726, 90, 48),
        CREAM,
        "APLET",
        "NOTE",
        "",
        "",
        "",
    ),
    k(
        "views",
        r(289, 726, 90, 48),
        CREAM,
        "VIEWS",
        "SKETCH",
        "",
        "",
        "",
    ),
    k(
        "vars",
        r(62, 833, 90, 49),
        CREAM,
        "VARS",
        "CHARS",
        "",
        "A",
        "",
    ),
    k(
        "math",
        r(175, 833, 90, 49),
        CREAM,
        "MATH",
        "CMDS",
        "",
        "B",
        "",
    ),
    k("ddx", r(289, 833, 90, 49), CREAM, "d/dx", "∫", "", "C", ""),
    k(
        "xt",
        r(403, 833, 90, 49),
        CREAM,
        "X,T,θ",
        "EEX",
        "",
        "D",
        "",
    ),
    k(
        "del",
        r(517, 833, 90, 49),
        CREAM,
        "DEL",
        "CLEAR",
        "",
        "",
        "",
    ),
    k("sin", r(62, 909, 90, 48), CREAM, "SIN", "ASIN", "", "E", ""),
    k(
        "cos",
        r(175, 909, 90, 48),
        CREAM,
        "COS",
        "ACOS",
        "",
        "F",
        "",
    ),
    k(
        "tan",
        r(289, 909, 90, 48),
        CREAM,
        "TAN",
        "ATAN",
        "",
        "G",
        "",
    ),
    k("ln", r(403, 909, 90, 48), CREAM, "ln", "eˣ", "", "H", ""),
    k("log", r(517, 909, 90, 48), CREAM, "log", "10ˣ", "", "I", ""),
    k("square", r(62, 985, 90, 57), CREAM, "x²", "√", "", "J", ""),
    k("power", r(175, 985, 90, 57), CREAM, "xʸ", "ⁿ√", "", "K", ""),
    k(
        "lparen",
        r(289, 985, 90, 57),
        CREAM,
        "(",
        "ABS",
        "",
        "L",
        "",
    ),
    k(
        "rparen",
        r(403, 985, 90, 57),
        CREAM,
        ")",
        "ARG",
        "",
        "M",
        "",
    ),
    k("divide", r(517, 985, 90, 57), NAVY, "÷", "x⁻¹", "", "N", ""),
    k(
        "comma",
        r(62, 1069, 90, 57),
        CREAM,
        ",",
        "MEMORY",
        "",
        "O",
        "",
    ),
    k("7", r(175, 1069, 90, 57), NAVY, "7", "LIST", "", "P", ""),
    k("8", r(289, 1069, 90, 57), NAVY, "8", "{", "", "Q", ""),
    k("9", r(403, 1069, 90, 57), NAVY, "9", "}", "", "R", ""),
    k(
        "multiply",
        r(517, 1069, 90, 57),
        NAVY,
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
    k("4", r(175, 1152, 90, 57), NAVY, "4", "MATRIX", "", "T", ""),
    k("5", r(289, 1152, 90, 57), NAVY, "5", "[", "", "U", ""),
    k("6", r(403, 1152, 90, 57), NAVY, "6", "]", "", "V", ""),
    k("minus", r(517, 1152, 90, 57), NAVY, "−", "Δ", "", "W", ""),
    k("shift", r(62, 1237, 90, 57), SHIFT, "SHIFT", "", "", "", ""),
    k("1", r(175, 1237, 90, 57), NAVY, "1", "PROGRM", "", "X", ""),
    k("2", r(289, 1237, 90, 57), NAVY, "2", "SYNTAX", "", "Y", ""),
    k("3", r(403, 1237, 90, 57), NAVY, "3", "π", "", "Z", ""),
    k(
        "plus",
        r(517, 1237, 90, 57),
        NAVY,
        "+",
        "Σ",
        "",
        "SPACE",
        "",
    ),
    k(
        "on",
        r(62, 1321, 90, 57),
        NAVY,
        "ON",
        "OFF",
        "",
        "",
        "CANCEL",
    ),
    k("0", r(175, 1321, 90, 57), NAVY, "0", "NOTEPAD", "", "θ", ""),
    k("point", r(289, 1321, 90, 57), NAVY, "·", "=", "", ":", ""),
    k("neg", r(403, 1321, 90, 57), NAVY, "(-)", "AND", "", ";", ""),
    k(
        "enter",
        r(517, 1321, 90, 57),
        NAVY,
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
    panel(r(0, 0, 673, 1435), 60, 100, "#18305e"),
    raised(r(8, 8, 657, 1419), 54, 92, "#27467f"),
    sunk(r(24, 49, 625, 476), 36, 90, "#121314"),
];

const LINES: [Line; 4] = [
    Line {
        x1: 107,
        y1: 634,
        x2: 107,
        y2: 624,
        stroke: "#ff9f85",
    },
    Line {
        x1: 107,
        y1: 624,
        x2: 184,
        y2: 624,
        stroke: "#ff9f85",
    },
    Line {
        x1: 256,
        y1: 624,
        x2: 334,
        y2: 624,
        stroke: "#ff9f85",
    },
    Line {
        x1: 334,
        y1: 624,
        x2: 334,
        y2: 634,
        stroke: "#ff9f85",
    },
];

const MARKS_39G: [Mark; 2] = [
    Mark {
        x: 336,
        y: 489,
        size: 34,
        fill: "#e9edf3",
        text: "39G",
        italic: false,
    },
    Mark {
        x: 220,
        y: 630,
        size: 17,
        fill: "#ff9f85",
        text: "SETUP",
        italic: false,
    },
];

const MARKS_40G: [Mark; 2] = [
    Mark {
        x: 336,
        y: 489,
        size: 34,
        fill: "#e9edf3",
        text: "40G",
        italic: false,
    },
    Mark {
        x: 220,
        y: 630,
        size: 17,
        fill: "#ff9f85",
        text: "SETUP",
        italic: false,
    },
];

/// The HP 39G skin.
pub const SKIN_39G: Skin = Skin {
    width: 673,
    height: 1435,
    panels: &PANELS,
    lcd: r(38, 112, 595, 327),
    lcd_fill: "#b7c2a2",
    logo: r(312, 56, 48, 48),
    marks: &MARKS_39G,
    lines: &LINES,
    left_ink: "#ff9f85",
    right_ink: "#ff9f85",
    alpha_ink: "#e2e9f6",
    alpha_badge: "#e2e9f6",
    alpha_style: AlphaStyle::Below,
    below_ink: "#f5f7fb",
    small: 15,
    well_fill: "#0e1220",
    round: 22,
    // The photo's metallic flake: a light grain.
    texture: 7,
    keys: &KEYS,
};

/// The HP 40G skin.
pub const SKIN_40G: Skin = Skin {
    width: 673,
    height: 1435,
    panels: &PANELS,
    lcd: r(38, 112, 595, 327),
    lcd_fill: "#b7c2a2",
    logo: r(312, 56, 48, 48),
    marks: &MARKS_40G,
    lines: &LINES,
    left_ink: "#ff9f85",
    right_ink: "#ff9f85",
    alpha_ink: "#e2e9f6",
    alpha_badge: "#e2e9f6",
    alpha_style: AlphaStyle::Below,
    below_ink: "#f5f7fb",
    small: 15,
    well_fill: "#0e1220",
    round: 22,
    // The photo's metallic flake: a light grain.
    texture: 7,
    keys: &KEYS,
};
