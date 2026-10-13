//! The HP 19BII, open on the desk: the letter half on the left, the
//! display half on the right, the hinge between them ([`Fold`]), as on the
//! 28S ([`super::hp28s`]).
//!
//! Sources (references only; nothing of them is stored here):
//!
//! - **P1**: an HP 19BII, open, front, flat light, showing its self-test
//!   result (striegel, Wikimedia Commons, CC BY 2.0), 2957 x 2413 px,
//!   measured on its 1920 x 1567 rendering:
//!   <https://commons.wikimedia.org/wiki/File:HP_19B_II_calculator,_open,_power_on,_showing_successful_self-test_(20191371831).jpg>
//!   Letter half 845 x 1451 px at (70, 80), display half 905 x 1508 px
//!   at (925, 42); menu-key pitch 108.8 px, number-key pitch 135.6 px,
//!   row pitch 109.3 px (display half); letter keys 102.4 px apart across
//!   and 121.5 px down.
//! - **P2**: an HP 19BII, open, front, shown in plan view with improved
//!   colours (Pittigrilli, from J. C. Barros's photograph, Wikimedia
//!   Commons, CC BY 2.0), 1149 x 959 px:
//!   <https://commons.wikimedia.org/wiki/File:HP-19BII_Business_Consultant_II,_Financial_Calculator_from_Hewlett-Packard_(changed_to_plan_view,_improved_colours).jpg>
//!   Checks P1's grid and gives the colours with less colour cast: a
//!   near-black case, grey letter and function keys with white labels,
//!   cream menu and number keys with dark labels, an orange-yellow shift
//!   key and orange shifted labels.
//! - The HP 19BII Owner's Manual's keyboard figure (Figure 1-2, OM p.
//!   22-23, PDF p. 26-27; wiki: sources/hp19bii-om, hardware/hp19bii
//!   "Keyboard"): which key sits where and the labels on and above the
//!   keys. A line drawing, not to scale: the grid, not the sizes.
//!
//! Unit: 100 = the display half's menu-key pitch on P1 (the skins' rule),
//! 0.919 units per pixel there. The letter half, smaller on P1 (further
//! from the lens), is brought to the display half's height at 0.955 units
//! per pixel and drawn from its hinge side. Measured: the key caps, the
//! keyboard plates, the display housing, the name plate, the LCD glass,
//! the HP logo's square (the saturnus logo's place), the reference card's
//! recess and its four groups, the hinge barrel.
//!
//! Inferred: the corner radii, the skirts around the caps (taken from the
//! 28S), the 14-unit gap between the halves. The reference card's text is
//! our own (the card lists the six MAIN menus, clearing, contrast and the
//! language and digits settings, from the manual); "19BII" and "BUSINESS
//! CONSULTANT II" are the case's lettering as text. The LCD's 137 x 40
//! canvas (32 rows and the annunciator strip) is 596 units wide so its
//! menu labels (23 pixels apart, as on the 28S) sit over the menu keys
//! (100 units apart), inside the measured glass.

use super::*;

/// The black skirt every cap stands on, beyond the cap on every side.
pub const SKIRT: i16 = 7;

/// Width of one half.
const HALF_W: i16 = 832;
/// Height of the drawing (and of each half).
const HEIGHT: i16 = 1386;
/// Where the display half starts: the letter half and a 14-unit gap.
const DX: i16 = HALF_W + 14;

/// The case.
const CASE: &str = "#2b2a2b";
/// The keyboard plates.
const PLATE: &str = "#242323";
/// The reference card's groups and their text.
const BOX: &str = "#5f5c55";
const BOX_INK: &str = "#eeebe4";
/// Orange ink of the shifted labels.
const GOLD: &str = "#eaa54b";
/// Light ink printed on the plate (ENTER).
const PRINT: &str = "#e8e6df";

/// The grey keys with white labels.
const GREY: Cap = Cap {
    fill: "#5d5b57",
    ink: "#f5f3ec",
    well: SKIRT,
};
/// The cream keys (menu keys, digits, operators, EXIT) with dark labels.
const CREAM: Cap = Cap {
    fill: "#e4e1d2",
    ink: "#1d1c18",
    well: SKIRT,
};
/// The orange shift key, blank.
const SHIFT: Cap = Cap {
    fill: "#eda646",
    ink: "#1d1c18",
    well: SKIRT,
};

/// Cap tops of the letter half's six rows.
const LETTER_Y: [i16; 6] = [667, 780, 894, 1010, 1128, 1247];

/// Cap of letter key `col` (0-5) in row `row` (0-4): 70 x 55, 97.8 apart.
const fn letter_cap(col: i16, row: usize) -> Rect {
    r(113 + (978 * col + 5) / 10, LETTER_Y[row], 70, 55)
}

/// Cap tops of the display half's seven rows.
const ROW_Y: [i16; 7] = [639, 735, 835, 935, 1036, 1137, 1242];

/// Cap of the display half's key in column `col` (0-5) of the menu grid:
/// the menu keys, the shift row, the INPUT row and the first column.
const fn grid(col: i16, row: usize) -> Rect {
    r(DX + 166 + 100 * col, ROW_Y[row], 65, 55)
}

/// Cap of the display half's wide key `col` (1-4) in rows 3-6: the
/// digits and the operators, 124.6 apart.
const fn wide(col: i16, row: usize) -> Rect {
    r(DX + 280 + (1246 * (col - 1) + 5) / 10, ROW_Y[row], 90, 55)
}

/// A letter-half key with its shifted character; `alpha` is the letter it
/// types from the computer keyboard ("" for none).
const fn lk(
    name: &'static str,
    col: i16,
    row: usize,
    label: &'static str,
    left: &'static str,
    alpha: &'static str,
) -> SkinKey {
    k(name, letter_cap(col, row), GREY, label, left, "", alpha, "")
}

const KEYS: [SkinKey; 72] = [
    // The letter half (OM p. 22), rows from the top, with the characters
    // the shift types above them.
    lk("lettera", 0, 0, "A", "Ä", "A"),
    lk("letterb", 1, 0, "B", "Ã", "B"),
    lk("letterc", 2, 0, "C", "Ç", "C"),
    lk("letterd", 3, 0, "D", "Ø", "D"),
    lk("lettere", 4, 0, "E", "Æ", "E"),
    lk("letterf", 5, 0, "F", "Å", "F"),
    lk("letterg", 0, 1, "G", ";", "G"),
    lk("letterh", 1, 1, "H", "°", "H"),
    lk("letteri", 2, 1, "I", "{", "I"),
    lk("letterj", 3, 1, "J", "}", "J"),
    lk("letterk", 4, 1, "K", "[", "K"),
    lk("letterl", 5, 1, "L", "]", "L"),
    lk("letterm", 0, 2, "M", "∡", "M"),
    lk("lettern", 1, 2, "N", "Ñ", "N"),
    lk("lettero", 2, 2, "O", "Ö", "O"),
    lk("letterp", 3, 2, "P", "~", "P"),
    lk("letterq", 4, 2, "Q", "'", "Q"),
    lk("letterr", 5, 2, "R", "\"", "R"),
    lk("letters", 0, 3, "S", "@", "S"),
    lk("lettert", 1, 3, "T", "&", "T"),
    lk("letteru", 2, 3, "U", "Ü", "U"),
    lk("letterv", 3, 3, "V", "!", "V"),
    lk("letterw", 4, 3, "W", "/", "W"),
    lk("letterx", 5, 3, "X", "*", "X"),
    lk("lettery", 0, 4, "Y", "<", "Y"),
    lk("letterz", 1, 4, "Z", ">", "Z"),
    lk("question", 2, 4, "?", "¿", ""),
    lk("dollar", 3, 4, "$", "£", ""),
    lk("hash", 4, 4, "#", ",", ""),
    lk("colon", 5, 4, ":", ".", ""),
    // The bottom row: SPACE two keys wide, then four narrower keys.
    k(
        "space",
        r(102, LETTER_Y[5], 170, 55),
        GREY,
        "SPACE",
        "Σ",
        "",
        "SPACE",
        "",
    ),
    k(
        "ins",
        r(297, LETTER_Y[5], 74, 55),
        GREY,
        "INS",
        "",
        "",
        "",
        "",
    ),
    k(
        "del",
        r(396, LETTER_Y[5], 74, 55),
        GREY,
        "DEL",
        "",
        "",
        "",
        "",
    ),
    k(
        "left",
        r(498, LETTER_Y[5], 72, 55),
        GREY,
        "←",
        "⇤",
        "",
        "",
        "",
    ),
    k(
        "right",
        r(599, LETTER_Y[5], 72, 55),
        GREY,
        "→",
        "⇥",
        "",
        "",
        "",
    ),
    // The display half (OM p. 23): the six blank menu keys.
    k("a", grid(0, 0), CREAM, "", "", "", "", ""),
    k("b", grid(1, 0), CREAM, "", "", "", "", ""),
    k("c", grid(2, 0), CREAM, "", "", "", "", ""),
    k("d", grid(3, 0), CREAM, "", "", "", "", ""),
    k("e", grid(4, 0), CREAM, "", "", "", "", ""),
    k("f", grid(5, 0), CREAM, "", "", "", "", ""),
    k("shift", grid(0, 1), SHIFT, "", "", "", "", ""),
    k("sto", grid(1, 1), GREY, "STO", "", "", "", ""),
    k("rcl", grid(2, 1), GREY, "RCL", "", "", "", ""),
    k("dsp", grid(3, 1), GREY, "DISP", "MODES", "", "", ""),
    k("prt", grid(4, 1), GREY, "PRNT", "PRINTER", "", "", ""),
    k("exit", grid(5, 1), CREAM, "EXIT", "MAIN", "", "", ""),
    // INPUT spans the first two columns, gap included.
    k(
        "input",
        r(DX + 166, ROW_Y[2], 165, 55),
        GREY,
        "INPUT",
        "CLEAR DATA",
        "",
        "",
        "",
    ),
    k("neg", grid(2, 2), GREY, "+/−", "E", "", "", ""),
    k("lparen", grid(3, 2), GREY, "(", "R↓", "", "", ""),
    k("rparen", grid(4, 2), GREY, ")", "x≷y", "", "", ""),
    k("backspace", grid(5, 2), GREY, "⬅", "CLEAR", "", "", ""),
    k("up", grid(0, 3), GREY, "↑", "", "", "", ""),
    k("7", wide(1, 3), CREAM, "7", "", "", "", ""),
    k("8", wide(2, 3), CREAM, "8", "", "", "", ""),
    k("9", wide(3, 3), CREAM, "9", "", "", "", ""),
    k("divide", wide(4, 3), CREAM, "÷", "1/x", "", "", ""),
    k("down", grid(0, 4), GREY, "↓", "", "", "", ""),
    k("4", wide(1, 4), CREAM, "4", "", "", "", ""),
    k("5", wide(2, 4), CREAM, "5", "", "", "", ""),
    k("6", wide(3, 4), CREAM, "6", "", "", "", ""),
    k("multiply", wide(4, 4), CREAM, "×", "^", "", "", ""),
    k("percent", grid(0, 5), GREY, "%", "MATH", "", "", ""),
    k("1", wide(1, 5), CREAM, "1", "", "", "", ""),
    k("2", wide(2, 5), CREAM, "2", "", "", "", ""),
    k("3", wide(3, 5), CREAM, "3", "", "", "", ""),
    k("minus", wide(4, 5), CREAM, "−", "√x", "", "", ""),
    k("on", grid(0, 6), GREY, "ON", "", "", "", ""),
    k("0", wide(1, 6), CREAM, "0", "MEM", "", "", ""),
    k("point", wide(2, 6), CREAM, "·", "SHOW", "", "", ""),
    k("equals", wide(3, 6), CREAM, "=", "LAST", "", "", "ENTER"),
    k("plus", wide(4, 6), CREAM, "+", "x²", "", "", ""),
];

/// Index of the reference card's recess among [`PANELS`].
#[cfg(test)]
pub const CARD: usize = 1;
/// Index of the letter half's keyboard plate.
#[cfg(test)]
pub const LETTER_PLATE: usize = 2;
/// Index of the name plate.
#[cfg(test)]
pub const NAME_PLATE: usize = 4;
/// Index of the display half's keyboard plate.
#[cfg(test)]
pub const DISPLAY_PLATE: usize = 6;
/// Index of the card's first group.
#[cfg(test)]
pub const FIRST_GROUP: usize = 7;

/// A group of the reference card, across the card.
const fn group(y: i16, h: i16) -> Panel {
    panel(r(116, y, 596, h), 6, 6, BOX)
}

const PANELS: [Panel; 11] = [
    // The case: the whole drawing, drawn per half by the page.
    panel(r(0, 0, DX + HALF_W, HEIGHT), 22, 22, CASE),
    // The letter half: the reference card's recess and the plate.
    sunk(r(82, 19, 654, 564), 8, 8, "#1c1b1c"),
    raised(r(54, 594, 688, 752), 6, 6, PLATE),
    // The display half: the housing, the name plate, the LCD bezel.
    raised(r(DX + 95, 10, 730, 575), 16, 10, "#333132"),
    sunk(r(DX + 133, 37, 648, 205), 6, 6, "#2a2829"),
    sunk(r(DX + 128, 300, 650, 240), 6, 6, "#1f1e1f"),
    raised(r(DX + 110, 600, 700, 760), 6, 6, PLATE),
    // The reference card's four groups.
    group(76, 131),
    group(224, 110),
    group(344, 89),
    group(444, 115),
];

/// A line of the reference card's text, centred at `x`.
const fn card(x: i16, y: i16, text: &'static str) -> Mark {
    Mark {
        x,
        y,
        size: 19,
        fill: BOX_INK,
        text,
        italic: false,
        bold: false,
    }
}

/// A group's heading.
const fn heading(x: i16, y: i16, text: &'static str) -> Mark {
    Mark {
        x,
        y,
        size: 19,
        fill: "#dedbd4",
        text,
        italic: false,
        bold: true,
    }
}

/// Centre x of the MAIN menu's name `i` (0-5) on the card.
const fn menu_x(i: i16) -> i16 {
    166 + 99 * i
}

const MARKS: [Mark; 25] = [
    // The reference card (letter half): our own wording.
    heading(200, 102, "MAIN MENU"),
    card(menu_x(0), 140, "FIN"),
    card(menu_x(1), 140, "BUS"),
    card(menu_x(2), 140, "SUM"),
    card(menu_x(3), 140, "TIME"),
    card(menu_x(4), 140, "SOLVE"),
    card(menu_x(5), 140, "TEXT"),
    card(menu_x(0), 166, "Finance"),
    card(menu_x(1), 166, "Business"),
    card(menu_x(2), 166, "Totals"),
    card(menu_x(3), 166, "Clock"),
    card(menu_x(4), 166, "Solver"),
    card(menu_x(5), 166, "Text"),
    card(414, 196, "Back to MAIN: shift, then EXIT"),
    heading(166, 250, "CLEAR"),
    card(414, 282, "The calculator line: shift, then ⬅"),
    card(414, 314, "The data of a menu: shift, then INPUT"),
    heading(190, 370, "CONTRAST"),
    card(414, 398, "Darker: hold ON and press +"),
    card(414, 424, "Lighter: hold ON and press −"),
    heading(220, 470, "LANGUAGE, DIGITS"),
    card(414, 502, "English: shift, DISP, then INTL, ENG"),
    card(414, 534, "Decimals: DISP, FIX, a number, INPUT"),
    // The display half: the model and the case's lettering.
    Mark {
        x: DX + 236,
        y: 220,
        size: 36,
        fill: "#ece9e3",
        text: "19BII",
        italic: true,
        bold: true,
    },
    Mark {
        x: DX + 560,
        y: 220,
        size: 27,
        fill: "#ece9e3",
        text: "BUSINESS CONSULTANT II",
        italic: true,
        bold: false,
    },
];

/// The HP 19BII skin.
pub const SKIN: Skin = Skin {
    width: DX + HALF_W,
    height: HEIGHT,
    panels: &PANELS,
    lcd: r(DX + 151, 335, 596, 174),
    lcd_fill: "#b7c2a2",
    glass: Some(r(DX + 146, 322, 610, 196)),
    fold: Some(Fold {
        letters: r(0, 0, HALF_W, HEIGHT),
        display: r(DX, 0, HALF_W, HEIGHT),
        hinge: r(762, 143, 60, 1065),
        stacked_letters: r(40, 590, 720, 780),
        stacked_display: r(DX + 95, 0, 730, HEIGHT),
    }),
    logo: r(DX + 182, 78, 50, 50),
    marks: &MARKS,
    lines: &[],
    left_ink: GOLD,
    right_ink: GOLD,
    alpha_ink: "#f5f3ec",
    alpha_badge: "#000000",
    alpha_style: AlphaStyle::Label,
    below_ink: PRINT,
    small: 18,
    well_fill: "#141313",
    round: 14,
    texture: 22,
    keys: &KEYS,
};
