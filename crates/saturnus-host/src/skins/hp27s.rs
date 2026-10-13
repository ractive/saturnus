//! HP 27S skin.
//!
//! Geometry: the Pioneer case measured on the photographs P3 (this 27S)
//! and P4 (a 17BII), see [`super::pioneer`]. Labels: the keyboard figure
//! on PDF page 2 of the HP-27S Owner's Manual (edition 2, 1988; wiki:
//! sources/hp27s-om), checked against P3. Colours: medians of P3
//! (striegel, Wikimedia Commons, CC BY 2.0; wiki:
//! sources/commons-pioneer-photos):
//! <https://commons.wikimedia.org/wiki/File:HP_27S_Scientific_calculator,_front,_power_on,_end_of_keyboard_test,_on_blue_background_(30268535976).png>
//! Nothing of the figure or the photo is stored here.
//!
//! Measured on P3: the black case (#1e1d1b), the dark grey display
//! surround (#4e4742) with "27S" in heavy italics on the left and
//! "SCIENTIFIC" in italics on the right, both white (cap height 19 units,
//! baselines 189 and 188; centres 109 and 527), the silver bezel
//! (#d4c9b9), the lighter band behind the menu row (#46403c), the
//! keyboard plate (#242220), the black caps (#040403, drawn a little
//! lighter so the cap shows on its skirt) with white labels, the blue
//! shift key (#6098c0), the light blue shifted labels (#79abc2, most
//! saturated pixels of CONVERT), the grey bars (#4b4642) behind the menu
//! names of the digit block, white ON under CLR.
//!
//! Toned for 4.5:1 (the 39G rule): the shifted labels are lighter than the
//! photo's blue (3.7:1 on the bars), #9cc3d8.
//!
//! The menu keys print a large ^; one shift key, a blank blue cap, whose
//! labels use `left`. CLR is the ON-line key, with OFF above and ON below.

use super::pioneer::{self, SKIRT, bar, key};
use super::*;

/// The black keys with white labels.
const KEY: Cap = Cap {
    fill: "#121110",
    ink: "#f4f2ec",
    well: SKIRT,
};
/// The blue shift key, blank.
const SHIFT: Cap = Cap {
    fill: "#6098c0",
    ink: "#121110",
    well: SKIRT,
};

const KEYS: [SkinKey; 37] = [
    // Row 1: the six menu keys, each printed with a ^.
    key(0, "a", KEY, "^", "√x", ""),
    key(1, "b", KEY, "^", "x²", ""),
    key(2, "c", KEY, "^", "eˣ", ""),
    key(3, "d", KEY, "^", "LN", ""),
    key(4, "e", KEY, "^", "yˣ", ""),
    key(5, "f", KEY, "^", "1/x", ""),
    // Row 2.
    key(6, "sto", KEY, "STO", "10ˣ", ""),
    key(7, "rcl", KEY, "RCL", "LOG", ""),
    key(8, "sin", KEY, "SIN", "ASIN", ""),
    key(9, "cos", KEY, "COS", "ACOS", ""),
    key(10, "tan", KEY, "TAN", "ATAN", ""),
    key(11, "exit", KEY, "EXIT", "MAIN", ""),
    // Row 3: INPUT spans two keys.
    key(12, "input", KEY, "INPUT", "CLEAR DATA", ""),
    key(13, "neg", KEY, "+/-", "E", ""),
    key(14, "lparen", KEY, "(", "%", ""),
    key(15, "rparen", KEY, ")", "π", ""),
    key(16, "backspace", KEY, "⬅", "", ""),
    // Rows 4-7: a narrow key on the left, four wide keys.
    key(17, "up", KEY, "▲", "", ""),
    key(18, "7", KEY, "7", "SOLVE", ""),
    key(19, "8", KEY, "8", "STAT", ""),
    key(20, "9", KEY, "9", "TVM", ""),
    key(21, "divide", KEY, "÷", "TIME", ""),
    key(22, "down", KEY, "▼", "", ""),
    key(23, "4", KEY, "4", "BASE", ""),
    key(24, "5", KEY, "5", "PROB", ""),
    key(25, "6", KEY, "6", "HYP", ""),
    key(26, "multiply", KEY, "×", "CONVERT", ""),
    key(27, "shift", SHIFT, "", "", ""),
    key(28, "1", KEY, "1", "%CHG", ""),
    key(29, "2", KEY, "2", "PARTS", ""),
    key(30, "3", KEY, "3", "MODES", ""),
    key(31, "minus", KEY, "−", "PRINTER", ""),
    key(32, "on", KEY, "CLR", "OFF", "ON"),
    key(33, "0", KEY, "0", "MEM", ""),
    key(34, "point", KEY, "·", "SHOW", ""),
    key(35, "equals", KEY, "=", "LAST", ""),
    key(36, "plus", KEY, "+", "PRT", ""),
];

/// The grey bars behind the menu names of the digit block.
const BAR: &str = "#4b4642";

const PANELS: [Panel; 18] = {
    let p = pioneer::plates(
        "#1e1d1b", "#0f0e0d", "#4e4742", "#d4c9b9", "#242220", "#46403c",
    );
    [
        p[0],
        p[1],
        p[2],
        p[3],
        p[4],
        p[5],
        // Rows 4-6: SOLVE STAT TVM TIME, BASE PROB HYP CONVERT, %CHG PARTS
        // MODES PRINTER.
        bar(0, 3, BAR),
        bar(1, 3, BAR),
        bar(2, 3, BAR),
        bar(3, 3, BAR),
        bar(0, 4, BAR),
        bar(1, 4, BAR),
        bar(2, 4, BAR),
        bar(3, 4, BAR),
        bar(0, 5, BAR),
        bar(1, 5, BAR),
        bar(2, 5, BAR),
        bar(3, 5, BAR),
    ]
};

const MARKS: [Mark; 2] = [
    Mark {
        x: 109,
        y: 189,
        size: 28,
        fill: "#f4f2ec",
        text: "27S",
        italic: true,
        bold: true,
    },
    Mark {
        x: 527,
        y: 188,
        size: 23,
        fill: "#f4f2ec",
        text: "SCIENTIFIC",
        italic: true,
        bold: false,
    },
];

/// The HP 27S skin.
pub const SKIN: Skin = Skin {
    width: pioneer::WIDTH,
    height: pioneer::HEIGHT,
    panels: &PANELS,
    lcd: pioneer::LCD,
    lcd_fill: pioneer::LCD_FILL,
    glass: Some(pioneer::GLASS),
    logo: pioneer::LOGO,
    marks: &MARKS,
    lines: &[],
    left_ink: "#9cc3d8",
    right_ink: "#9cc3d8",
    alpha_ink: "#f4f2ec",
    alpha_badge: "#f4f2ec",
    alpha_style: AlphaStyle::Outside,
    below_ink: "#f4f2ec",
    small: 17,
    well_fill: "#050505",
    round: 18,
    // Matte plastic (P3 shows a fine grain on the case and plate).
    texture: 9,
    keys: &KEYS,
};
