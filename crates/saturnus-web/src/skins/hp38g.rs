//! HP 38G skin.
//!
//! Geometry and colours: the owner's straight-on photographs of his own
//! HP 38G (October 2026, not in the repository; menu-key pitch 125 px on a
//! 1500 x 2000 frame). Measured from them: the case (714 x 1525 units),
//! the key wells (70 x 52 units at a pitch of 100 both ways, 93 wide at a
//! pitch of 121 in the number columns), the navy top with the display, the
//! slot under it, the keyboard panel and its teal zone, which begins
//! above the HOME row and holds the lower six rows. The photographs
//! confirm the labels and the alpha letters of the keyboard figure inside
//! the cover of the HP 38G User's Guide (`hp38g-ug-en.pdf`, PDF page 2).
//! The second row has no keys at the 48's VAR and NXT places.
//!
//! Differs from the unit on purpose: the display window is placed and
//! sized so the ROM's six menu labels sit over the six menu keys (see
//! [`super::SOFTKEY_LABEL_PITCH`]), 595 units wide where the real glass is
//! 538, so its frame is thinner and the slot sits lower; the top of the
//! real case is tilted towards the user and narrower in the photographs,
//! here it is flat; the rows keep one pitch (the photographs show the
//! lower rows 4% larger, the unit lying nearer the camera there) and the
//! case ends 85 units below the ON row; the shades are toned down from
//! the sunlit exposure. The series mark beside the model name is left off.

use super::*;

/// Dark key caps with warm cream labels.
const KEY: Cap = Cap {
    fill: "#262122",
    ink: "#f6dba6",
    well: 5,
};
/// The cream cursor keys.
const ARROW: Cap = Cap {
    fill: "#f3cf94",
    ink: "#262122",
    well: 5,
};
/// The light turquoise shift key.
const SHIFT: Cap = Cap {
    fill: "#a8d8ce",
    ink: "#262122",
    well: 5,
};

const KEYS: [SkinKey; 47] = [
    k("a", r(78, 575, 60, 42), KEY, "", "", "", "", ""),
    k("b", r(178, 575, 60, 42), KEY, "", "", "", "", ""),
    k("c", r(278, 575, 60, 42), KEY, "", "", "", "", ""),
    k("d", r(378, 575, 60, 42), KEY, "", "", "", "", ""),
    k("e", r(478, 575, 60, 42), KEY, "", "", "", "", ""),
    k("f", r(578, 575, 60, 42), KEY, "", "", "", "", ""),
    k("plot", r(78, 673, 60, 42), KEY, "PLOT", "", "", "", ""),
    k("symb", r(178, 673, 60, 42), KEY, "SYMB", "", "", "", ""),
    k("num", r(278, 673, 60, 42), KEY, "NUM", "", "", "", ""),
    k("up", r(478, 673, 60, 42), ARROW, "▲", "", "", "", ""),
    k("lib", r(78, 773, 60, 42), KEY, "LIB", "VIEWS", "", "", ""),
    k("var", r(178, 773, 60, 42), KEY, "VAR", "NOTE", "", "", ""),
    k(
        "math",
        r(278, 773, 60, 42),
        KEY,
        "MATH",
        "SKETCH",
        "",
        "",
        "",
    ),
    k("left", r(378, 773, 60, 42), ARROW, "◀", "", "", "", ""),
    k("down", r(478, 773, 60, 42), ARROW, "▼", "", "", "", ""),
    k("right", r(578, 773, 60, 42), ARROW, "▶", "", "", "", ""),
    k(
        "home",
        r(78, 873, 60, 42),
        KEY,
        "HOME",
        "MODES",
        "",
        "A",
        "",
    ),
    k("sin", r(178, 873, 60, 42), KEY, "SIN", "ASIN", "", "B", ""),
    k("cos", r(278, 873, 60, 42), KEY, "COS", "ACOS", "", "C", ""),
    k("tan", r(378, 873, 60, 42), KEY, "TAN", "ATAN", "", "D", ""),
    k("xt", r(478, 873, 60, 42), KEY, "X,T,θ", "x⁻¹", "", "E", ""),
    k("sqrt", r(578, 873, 60, 42), KEY, "√x", "ⁿ√x", "", "F", ""),
    k(
        "enter",
        r(78, 973, 160, 42),
        KEY,
        "ENTER",
        "ANSWER",
        "",
        "",
        "",
    ),
    k(
        "lparen",
        r(278, 973, 60, 42),
        KEY,
        "(",
        "CHARS",
        "",
        "G",
        "",
    ),
    k("rparen", r(378, 973, 60, 42), KEY, ")", "EEX", "", "H", ""),
    k("neg", r(478, 973, 60, 42), KEY, "-x", "ABS", "", "I", ""),
    k("power", r(578, 973, 60, 42), KEY, "xʸ", "x²", "", "J", ""),
    k("alpha", r(78, 1073, 60, 42), KEY, "A…Z", "a…z", "", "", ""),
    k("7", r(191, 1073, 83, 42), KEY, "7", "LIST", "", "K", ""),
    k("8", r(312, 1073, 83, 42), KEY, "8", "{", "", "L", ""),
    k("9", r(434, 1073, 83, 42), KEY, "9", "}", "", "M", ""),
    k("divide", r(555, 1073, 83, 42), KEY, "/", "LOG", "", "N", ""),
    k("shift", r(78, 1173, 60, 42), SHIFT, "", "", "", "", ""),
    k("4", r(191, 1173, 83, 42), KEY, "4", "MATRIX", "", "O", ""),
    k("5", r(312, 1173, 83, 42), KEY, "5", "[", "", "P", ""),
    k("6", r(434, 1173, 83, 42), KEY, "6", "]", "", "Q", ""),
    k(
        "multiply",
        r(555, 1173, 83, 42),
        KEY,
        "*",
        "10ˣ",
        "",
        "R",
        "",
    ),
    k("del", r(78, 1273, 60, 42), KEY, "DEL", "CLEAR", "", "", ""),
    k("1", r(191, 1273, 83, 42), KEY, "1", "NOTEPAD", "", "S", ""),
    k("2", r(312, 1273, 83, 42), KEY, "2", "SPACE", "", "T", ""),
    k("3", r(434, 1273, 83, 42), KEY, "3", "π", "", "U", ""),
    k("minus", r(555, 1273, 83, 42), KEY, "−", "LN", "", "V", ""),
    k(
        "on",
        r(78, 1373, 60, 42),
        KEY,
        "ON",
        "OFF",
        "",
        "",
        "CANCEL",
    ),
    k("0", r(191, 1373, 83, 42), KEY, "0", "PROGRAM", "", "W", ""),
    k("point", r(312, 1373, 83, 42), KEY, "·", ":", "", "X", ""),
    k("comma", r(434, 1373, 83, 42), KEY, ",", ";", "", "Y", ""),
    k("plus", r(555, 1373, 83, 42), KEY, "+", "eˣ", "", "Z", ""),
];

const PANELS: [Panel; 7] = [
    // The rim, the navy top, the display's frame, the slot under it.
    panel(r(0, 0, 714, 1500), 22, 22, "#25222a"),
    panel(r(12, 10, 690, 518), 16, 6, "#2b2c3a"),
    panel(r(46, 110, 622, 371), 6, 6, "#17161c"),
    panel(r(171, 492, 372, 22), 11, 11, "#22232f"),
    // The keyboard panel, its teal zone, the stripe joining the menu keys.
    panel(r(40, 536, 634, 930), 6, 8, "#2c2829"),
    panel(r(40, 831, 634, 635), 0, 8, "#30494a"),
    panel(r(108, 592, 500, 7), 0, 0, "#3c5553"),
];

const MARKS: [Mark; 2] = [
    Mark {
        x: 622,
        y: 74,
        size: 30,
        fill: "#f3dfae",
        text: "38G",
        italic: false,
    },
    Mark {
        x: 208,
        y: 661,
        size: 20,
        fill: "#9cc4bb",
        text: "SETUP",
        italic: false,
    },
];

const LINES: [Line; 4] = [
    Line {
        x1: 108,
        y1: 664,
        x2: 108,
        y2: 654,
        stroke: "#9cc4bb",
    },
    Line {
        x1: 108,
        y1: 654,
        x2: 170,
        y2: 654,
        stroke: "#9cc4bb",
    },
    Line {
        x1: 246,
        y1: 654,
        x2: 308,
        y2: 654,
        stroke: "#9cc4bb",
    },
    Line {
        x1: 308,
        y1: 654,
        x2: 308,
        y2: 664,
        stroke: "#9cc4bb",
    },
];

/// The HP 38G skin.
pub const SKIN: Skin = Skin {
    width: 714,
    height: 1500,
    panels: &PANELS,
    lcd: r(60, 132, 595, 327),
    lcd_fill: "#b7c2a2",
    logo: r(50, 28, 56, 56),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#9cc4bb",
    right_ink: "#9cc4bb",
    alpha_ink: "#cf8e78",
    alpha_badge: "#cf8e78",
    alpha_style: AlphaStyle::Corner,
    below_ink: "#b9b596",
    small: 20,
    well_fill: "#0f0d0e",
    round: 18,
    keys: &KEYS,
};
