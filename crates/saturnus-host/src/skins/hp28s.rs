//! The HP 28S, open on the desk: the letter half on the left, the display
//! half on the right, the hinge between them ([`Fold`]).
//!
//! Sources (references only; nothing of them is stored here):
//!
//! - **P1**: an HP 28S, open, front, flat light (striegel, Wikimedia
//!   Commons, CC BY 2.0), 2921 x 2379 px, measured on its 1920 x 1564
//!   rendering:
//!   <https://commons.wikimedia.org/wiki/File:HP-28S_calculator,_open,_power_off_(24687429911).jpg>
//!   Letter half 895 x 1521 px at (50, 25), display half 879 x 1488 px
//!   at (960, 47); menu-key pitch 105.2 px.
//! - **P2**: an HP 28S, open, front (Ralf Pfeifer, Wikimedia Commons,
//!   CC BY-SA 3.0 de), 698 x 587 px:
//!   <https://commons.wikimedia.org/wiki/File:HP-28S.jpg>
//!   Checks P1's key grid (the letter keys' pitch equals the menu keys'
//!   within 3%) and the colours.
//! - The HP 28S Owner's Manual's keyboard figures (OM p. 26-27; wiki:
//!   sources/hp28s-om, hardware/hp28s "Keyboard"): which key sits where,
//!   the labels on and above the keys, the cursor menu printed over the
//!   menu keys, the label bands and the PROGRAM bracket. Line drawings, not
//!   to scale: the grid, not the sizes.
//!
//! Unit: 100 = the display half's menu-key pitch on P1 (the skins' rule);
//! the measurements are on the wiki (hardware/hp28s "Case layout"). Each
//! half is measured from its own corner: the display half at 0.951 units
//! per pixel, the letter half at 0.932, which brings both halves to one
//! size (836 x 1414 units; P1 shows the letter half 2% larger, nearer the
//! lens). Measured: the key footprints (cap and skirt, [`SKIRT`]), the
//! keyboard plates, the display housing, the name plate, the LCD glass,
//! the HP logo's square (the saturnus logo's place), the reference card's
//! recess and boxes, the label bands, the hinge barrel.
//!
//! Inferred: the corner radii, the 14-unit gap between the halves; the
//! reference card's examples are our own (the card lists the object types
//! and their delimiters, which are the 28S's syntax, OM p. 26); "28S" and
//! "ADVANCED SCIENTIFIC CALCULATOR" are the case's lettering as text. The
//! cursor menu over the menu keys and ATTN under ON are printed white on
//! the plate; here they are dark, so they keep 4.5:1 on the light plate.
//! The LCD's 137 x 40 canvas (32 rows and the annunciator strip) is 596
//! units wide so its menu labels (23 pixels apart) sit over the menu keys
//! (100 units apart), inside the measured glass.

use super::*;

/// The black skirt every cap stands on, beyond the cap on every side.
pub const SKIRT: i16 = 7;

/// Width of one half.
const HALF_W: i16 = 836;
/// Height of the drawing (and of each half).
const HEIGHT: i16 = 1414;
/// Where the display half starts: the letter half and a 14-unit gap.
const DX: i16 = HALF_W + 14;

/// The case.
const CASE: &str = "#363437";
/// The keyboard plates.
const PLATE: &str = "#a7a297";
/// The light bands behind the menu names.
const BAND: &str = "#cdc7b8";
/// Dark ink printed on the plate (the cursor menu, ATTN).
const PRINT: &str = "#2a2824";
/// The reference card's boxes and their text.
const BOX: &str = "#5e5a54";
const BOX_INK: &str = "#eeebe4";

/// The dark keys with white labels.
const DARK: Cap = Cap {
    fill: "#67635b",
    ink: "#faf7ee",
    well: SKIRT,
};
/// The light keys (menu keys, digits, operators, NEXT) with dark labels.
const LIGHT: Cap = Cap {
    fill: "#cdcabb",
    ink: "#1f1d16",
    well: SKIRT,
};
/// The red shift key, blank.
const SHIFT: Cap = Cap {
    fill: "#7e2a35",
    ink: "#faf7ee",
    well: SKIRT,
};

/// Cap of letter key `col` (0-5) in row `row` (0-5): footprints at 83 +
/// 102.8 col and 683 + 115.8 row, 83 x 73.
const fn letter_cap(col: i16, row: i16) -> Rect {
    r(
        90 + (1028 * col + 5) / 10,
        690 + (1158 * row + 5) / 10,
        69,
        59,
    )
}

/// Cap tops of the display half's seven rows (footprints at 646 + 101.7
/// row).
const ROW_Y: [i16; 7] = [653, 755, 856, 958, 1060, 1161, 1263];

/// Cap of the display half's key `col` (0-5) in rows 0-2 (footprints
/// at 164 + 100 col of the half, 80 wide).
const fn top(col: i16, row: usize) -> Rect {
    r(DX + 171 + 100 * col, ROW_Y[row], 66, 57)
}

/// Cap of the display half's key `col` (0-4) in rows 3-6: the first
/// column as above, then four wide keys (footprints at 276 + 123.3 (col -
/// 1) of the half, 103 wide).
const fn low(col: i16, row: usize) -> Rect {
    if col == 0 {
        top(0, row)
    } else {
        r(DX + 283 + (1233 * (col - 1) + 5) / 10, ROW_Y[row], 89, 57)
    }
}

/// A letter-half key with its shifted label.
const fn lk(
    name: &'static str,
    col: i16,
    row: i16,
    label: &'static str,
    left: &'static str,
    alpha: &'static str,
) -> SkinKey {
    k(name, letter_cap(col, row), DARK, label, left, "", alpha, "")
}

const KEYS: [SkinKey; 72] = [
    // The letter half (OM p. 26), rows from the top.
    lk("lettera", 0, 0, "A", "ARRAY", "A"),
    lk("letterb", 1, 0, "B", "BINARY", "B"),
    lk("letterc", 2, 0, "C", "COMPLX", "C"),
    lk("letterd", 3, 0, "D", "STRING", "D"),
    lk("lettere", 4, 0, "E", "LIST", "E"),
    lk("letterf", 5, 0, "F", "REAL", "F"),
    lk("letterg", 0, 1, "G", "STACK", "G"),
    lk("letterh", 1, 1, "H", "STORE", "H"),
    lk("letteri", 2, 1, "I", "MEMORY", "I"),
    lk("letterj", 3, 1, "J", "ALGBRA", "J"),
    lk("letterk", 4, 1, "K", "STAT", "K"),
    lk("letterl", 5, 1, "L", "PRINT", "L"),
    lk("letterm", 0, 2, "M", "CONTRL", "M"),
    lk("lettern", 1, 2, "N", "BRANCH", "N"),
    lk("lettero", 2, 2, "O", "TEST", "O"),
    lk("letterp", 3, 2, "P", "", "P"),
    lk("letterq", 4, 2, "Q", "CATALOG", "Q"),
    lk("letterr", 5, 2, "R", "UNITS", "R"),
    lk("letters", 0, 3, "S", "≤", "S"),
    lk("lettert", 1, 3, "T", "≥", "T"),
    lk("letteru", 2, 3, "U", "→", "U"),
    lk("letterv", 3, 3, "V", "Σ", "V"),
    lk("letterw", 4, 3, "W", "°", "W"),
    lk("letterx", 5, 3, "X", "µ", "X"),
    lk("lettery", 0, 4, "Y", "<", "Y"),
    lk("letterz", 1, 4, "Z", ">", "Z"),
    lk("hash", 2, 4, "#", "\"", ""),
    lk("lbrace", 3, 4, "{", "}", ""),
    lk("lbracket", 4, 4, "[", "]", ""),
    lk("lparen", 5, 4, "(", ")", ""),
    // SPACE spans the first two columns, gap included.
    k(
        "space",
        r(90, letter_cap(0, 5).y, 172, 59),
        DARK,
        "SPACE",
        "NEWLINE",
        "",
        "",
        "",
    ),
    lk("program", 2, 5, "«", "»", ""),
    lk("equals", 3, 5, "=", "≠", ""),
    lk("lc", 4, 5, "LC", "?", ""),
    lk("alpha", 5, 5, "α", "MENUS", ""),
    // The display half (OM p. 27): the six blank menu keys, the cursor
    // menu printed over them (marks).
    k("a", top(0, 0), LIGHT, "", "", "", "", ""),
    k("b", top(1, 0), LIGHT, "", "", "", "", ""),
    k("c", top(2, 0), LIGHT, "", "", "", "", ""),
    k("d", top(3, 0), LIGHT, "", "", "", "", ""),
    k("e", top(4, 0), LIGHT, "", "", "", "", ""),
    k("f", top(5, 0), LIGHT, "", "", "", "", ""),
    k("shift", top(0, 1), SHIFT, "", "", "", "", ""),
    k("cursor", top(1, 1), DARK, "✥", "MODE", "", "", ""),
    k("trig", top(2, 1), DARK, "TRIG", "LOGS", "", "", ""),
    k("solv", top(3, 1), DARK, "SOLV", "PLOT", "", "", ""),
    k("user", top(4, 1), DARK, "USER", "CUSTOM", "", "", ""),
    k("nxt", top(5, 1), LIGHT, "NEXT", "PREV", "", "", ""),
    // ENTER spans the first two columns, gap included.
    k(
        "enter",
        r(DX + 171, ROW_Y[2], 166, 57),
        DARK,
        "ENTER",
        "EDIT",
        "",
        "",
        "",
    ),
    k("neg", top(2, 2), DARK, "CHS", "VIEW↑", "", "", ""),
    k("eex", top(3, 2), DARK, "EEX", "VIEW↓", "", "", ""),
    k("drop", top(4, 2), DARK, "DROP", "ROLL", "", "", ""),
    k("backspace", top(5, 2), DARK, "⬅", "SWAP", "", "", ""),
    k("quote", low(0, 3), DARK, "'", "VISIT", "", "", ""),
    k("7", low(1, 3), LIGHT, "7", "COMMAND", "", "", ""),
    k("8", low(2, 3), LIGHT, "8", "UNDO", "", "", ""),
    k("9", low(3, 3), LIGHT, "9", "LAST", "", "", ""),
    k("divide", low(4, 3), LIGHT, "÷", "1/x", "", "", ""),
    k("sto", low(0, 4), DARK, "STO", "RCL", "", "", ""),
    k("4", low(1, 4), LIGHT, "4", "PURGE", "", "", ""),
    k("5", low(2, 4), LIGHT, "5", "∫", "", "", ""),
    k("6", low(3, 4), LIGHT, "6", "d/dx", "", "", ""),
    k("multiply", low(4, 4), LIGHT, "×", "^", "", "", ""),
    k("eval", low(0, 5), DARK, "EVAL", "→NUM", "", "", ""),
    k("1", low(1, 5), LIGHT, "1", "CONT", "", "", ""),
    k("2", low(2, 5), LIGHT, "2", "%", "", "", ""),
    k("3", low(3, 5), LIGHT, "3", "%CH", "", "", ""),
    k("minus", low(4, 5), LIGHT, "−", "√x", "", "", ""),
    k("on", low(0, 6), DARK, "ON", "OFF", "", "", "ATTN"),
    k("0", low(1, 6), LIGHT, "0", "CLEAR", "", "", ""),
    k("point", low(2, 6), LIGHT, "·", "π", "", "", ""),
    k("comma", low(3, 6), LIGHT, ",", "CONVERT", "", "", ""),
    k("plus", low(4, 6), LIGHT, "+", "x²", "", "", ""),
];

/// The band behind the menu name over the cap `cap` (OM p. 26-27: the
/// menus of the letter half's top rows, PROGRAM's three, and MODE to
/// CUSTOM): 11 units beyond the cap on each side, ending 5 above its
/// skirt.
const fn band(cap: Rect) -> Panel {
    panel(
        r(cap.x - 11, cap.y - SKIRT - 23, cap.w + 22, 18),
        3,
        3,
        BAND,
    )
}

/// Index of the letter half's keyboard plate among [`PANELS`].
#[cfg(test)]
pub const LETTER_PLATE: usize = 2;
/// Index of the display half's keyboard plate.
#[cfg(test)]
pub const DISPLAY_PLATE: usize = 6;
/// Index of the first band.
#[cfg(test)]
pub const FIRST_BAND: usize = 7;
/// Index of the reference card's recess.
#[cfg(test)]
pub const CARD: usize = 1;
/// Index of the name plate.
#[cfg(test)]
pub const NAME_PLATE: usize = 4;

/// Boxes of the reference card: tab, then the three columns of one group.
const fn card_box(x: i16, y: i16, w: i16, h: i16) -> Panel {
    panel(r(x, y, w, h), 6, 6, BOX)
}

const PANELS: [Panel; 38] = [
    // The case: the whole drawing, drawn per half by the page.
    panel(r(0, 0, DX + HALF_W, HEIGHT), 22, 22, CASE),
    // The letter half: the reference card's recess and the plate.
    sunk(r(42, 28, 704, 582), 8, 8, "#2b292a"),
    raised(r(37, 620, 699, 744), 6, 6, PLATE),
    // The display half: the housing, the name plate, the LCD bezel.
    raised(r(DX + 100, 6, 728, 598), 16, 10, "#424043"),
    sunk(r(DX + 143, 43, 661, 204), 6, 6, "#38363a"),
    sunk(r(DX + 146, 324, 649, 226), 6, 6, "#2e2c2f"),
    raised(r(DX + 112, 621, 706, 749), 6, 6, PLATE),
    // Bands behind the menu names: ARRAY to REAL, STACK to PRINT,
    // CONTRL BRANCH TEST, MODE LOGS PLOT CUSTOM.
    band(letter_cap(0, 0)),
    band(letter_cap(1, 0)),
    band(letter_cap(2, 0)),
    band(letter_cap(3, 0)),
    band(letter_cap(4, 0)),
    band(letter_cap(5, 0)),
    band(letter_cap(0, 1)),
    band(letter_cap(1, 1)),
    band(letter_cap(2, 1)),
    band(letter_cap(3, 1)),
    band(letter_cap(4, 1)),
    band(letter_cap(5, 1)),
    band(letter_cap(0, 2)),
    band(letter_cap(1, 2)),
    band(letter_cap(2, 2)),
    band(top(1, 1)),
    band(top(2, 1)),
    band(top(3, 1)),
    band(top(4, 1)),
    // The reference card: Data, Names, Procedures, each a tab and three
    // columns (type, delimiters, example).
    card_box(56, 115, 90, 34),
    card_box(56, 144, 247, 231),
    card_box(312, 149, 117, 226),
    card_box(438, 149, 240, 226),
    card_box(56, 396, 126, 30),
    card_box(56, 422, 247, 37),
    card_box(312, 422, 117, 37),
    card_box(438, 422, 240, 37),
    card_box(56, 480, 164, 34),
    card_box(56, 510, 247, 64),
    card_box(312, 510, 117, 64),
    card_box(438, 510, 240, 64),
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

/// A heading of the reference card.
const fn heading(x: i16, y: i16, size: i16, text: &'static str) -> Mark {
    Mark {
        x,
        y,
        size,
        fill: "#dedbd4",
        text,
        italic: false,
        bold: true,
    }
}

/// The cursor menu's name printed over menu key `col`.
const fn cursor(col: i16, text: &'static str) -> Mark {
    Mark {
        x: DX + 204 + 100 * col,
        y: 638,
        size: 17,
        fill: PRINT,
        text,
        italic: false,
        bold: false,
    }
}

/// The card's columns: type names, delimiters, examples.
const TYPE_X: i16 = 179;
const SYMBOL_X: i16 = 370;
const EXAMPLE_X: i16 = 558;
/// Baseline of the Data group's row `i`.
const fn data_row(i: i16) -> i16 {
    178 + 31 * i
}

const MARKS: [Mark; 45] = [
    // The reference card (letter half): our own examples.
    heading(TYPE_X, 93, 24, "Object Type"),
    heading(SYMBOL_X, 93, 24, "Symbol"),
    heading(EXAMPLE_X, 93, 24, "Example"),
    heading(101, 139, 19, "Data"),
    card(TYPE_X, data_row(0), "Real Number"),
    card(TYPE_X, data_row(1), "Complex Number"),
    card(TYPE_X, data_row(2), "Binary Integer"),
    card(TYPE_X, data_row(3), "String"),
    card(TYPE_X, data_row(4), "Vector"),
    card(TYPE_X, data_row(5), "Matrix"),
    card(TYPE_X, data_row(6), "List"),
    card(SYMBOL_X, data_row(1), "(  ,  )"),
    card(SYMBOL_X, data_row(2), "#"),
    card(SYMBOL_X, data_row(3), "\"   \""),
    card(SYMBOL_X, data_row(4), "[   ]"),
    card(SYMBOL_X, data_row(5), "[[   ]]"),
    card(SYMBOL_X, data_row(6), "{   }"),
    card(EXAMPLE_X, data_row(0), "6.02E23"),
    card(EXAMPLE_X, data_row(1), "(3, -4)"),
    card(EXAMPLE_X, data_row(2), "#1F"),
    card(EXAMPLE_X, data_row(3), "\"HELLO\""),
    card(EXAMPLE_X, data_row(4), "[ 1 2 3 ]"),
    card(EXAMPLE_X, data_row(5), "[[ 1 0 ][ 0 1 ]]"),
    card(EXAMPLE_X, data_row(6), "{ 7 \"A\" #2 }"),
    heading(119, 418, 19, "Names"),
    card(TYPE_X, 447, "Name"),
    card(SYMBOL_X, 447, "'   '"),
    card(EXAMPLE_X, 447, "'AREA'"),
    heading(138, 504, 19, "Procedures"),
    card(TYPE_X, 535, "Program"),
    card(TYPE_X, 561, "Algebraic"),
    card(SYMBOL_X, 535, "«   »"),
    card(SYMBOL_X, 561, "'   '"),
    card(EXAMPLE_X, 535, "« DUP * »"),
    card(EXAMPLE_X, 561, "'R^2*π'"),
    // The bracket over CONTRL BRANCH TEST.
    Mark {
        x: 227,
        y: 888,
        size: 12,
        fill: "#651c27",
        text: "PROGRAM",
        italic: false,
        bold: false,
    },
    // The display half: the model and the case's lettering.
    Mark {
        x: DX + 230,
        y: 223,
        size: 33,
        fill: "#ece9e3",
        text: "28S",
        italic: true,
        bold: true,
    },
    Mark {
        x: DX + 674,
        y: 193,
        size: 27,
        fill: "#ece9e3",
        text: "ADVANCED",
        italic: true,
        bold: false,
    },
    Mark {
        x: DX + 574,
        y: 223,
        size: 27,
        fill: "#ece9e3",
        text: "SCIENTIFIC CALCULATOR",
        italic: true,
        bold: false,
    },
    // The cursor menu over the menu keys.
    cursor(0, "INS"),
    cursor(1, "DEL"),
    cursor(2, "▲"),
    cursor(3, "▼"),
    cursor(4, "◀"),
    cursor(5, "▶"),
];

/// The PROGRAM bracket's two arms, either side of its name.
const LINES: [Line; 2] = [
    Line {
        x1: 81,
        y1: 884,
        x2: 190,
        y2: 884,
        stroke: "#651c27",
    },
    Line {
        x1: 264,
        y1: 884,
        x2: 374,
        y2: 884,
        stroke: "#651c27",
    },
];

/// The HP 28S skin.
pub const SKIN: Skin = Skin {
    width: DX + HALF_W,
    height: HEIGHT,
    panels: &PANELS,
    lcd: r(DX + 156, 351, 596, 174),
    lcd_fill: "#b7c2a2",
    glass: Some(r(DX + 155, 337, 627, 203)),
    fold: Some(Fold {
        letters: r(0, 0, HALF_W, HEIGHT),
        display: r(DX, 0, HALF_W, HEIGHT),
        hinge: r(776, 177, 52, 1146),
        stacked_letters: r(29, 614, 715, 800),
        stacked_display: r(DX + 100, 0, 728, HEIGHT),
    }),
    logo: r(DX + 198, 76, 52, 52),
    marks: &MARKS,
    lines: &LINES,
    left_ink: "#651c27",
    right_ink: "#651c27",
    alpha_ink: "#faf7ee",
    alpha_badge: "#000000",
    alpha_style: AlphaStyle::Label,
    below_ink: PRINT,
    small: 18,
    well_fill: "#1d1c1a",
    round: 14,
    texture: 22,
    keys: &KEYS,
};
