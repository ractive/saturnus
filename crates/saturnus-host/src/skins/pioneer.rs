//! The Pioneer case of the 27S, 17BII and 17B: the key grid and the plates
//! the three share, measured on two square-on photographs.
//!
//! Sources (references only; nothing of them is stored here):
//!
//! - **P3**: a 1987 HP 27S, front, flat light (striegel, Wikimedia Commons,
//!   CC BY 2.0; wiki: sources/commons-pioneer-photos), 1495 x 2154 px:
//!   <https://commons.wikimedia.org/wiki/File:HP_27S_Scientific_calculator,_front,_power_on,_end_of_keyboard_test,_on_blue_background_(30268535976).png>
//!   Case 933 x 1737 px at (283, 260); menu-key pitch 139.2 px.
//! - **P4**: an HP 17BII, front, scanned (striegel, Wikimedia Commons,
//!   CC BY 2.0, 2015-07-29), 1055 x 1870 px:
//!   <https://commons.wikimedia.org/wiki/File:HP_17B_II_Business_calculator_(19934267028).jpg>
//!   Case 936 x 1754 px at (60, 64); menu-key pitch 138.8 px.
//!
//! The owner's manuals' keyboard figures (27S, 17BII, 17B: PDF page 2) are
//! line drawings, not to scale; they confirm the grid (six keys in rows
//! 1-2, a two-key INPUT and four in row 3, a narrow key and four wide ones
//! in rows 4-7) and the plates, not the sizes.
//!
//! Unit: 100 = the menu-key pitch (the skins' rule); the measurements are
//! on the wiki (hardware/hp27s "Case layout"). Each value is the
//! mean of P3 and P4, which agree within 3 units across the keyboard
//! (P4 runs 0.7% taller). Measured: the case width (670 and 674 units),
//! the dark key footprints (cap and skirt; the cap is the footprint less
//! [`SKIRT`]), the display surround (33-641 x 128-466), the LCD bezel
//! (50-624 x 200-418), the LCD glass (64-610 x 226-384), the band behind
//! the menu row (478-583) and the keyboard plate from 478, the HP badge
//! (whose logo square, centred at (100, 78), gives the saturnus logo's
//! place), and the 27S's grey bars (19 units tall, 3 above the footprint,
//! 3 wider on the left).
//!
//! Inferred: the case ends 72 units below the bottom caps (the photos show
//! about 85; trimmed to the skins' rule), the corner radii, the recessed
//! rim's width (the 42S's). The LCD's dots are about 3.9 units wide and
//! 4.6 tall on the photos; the page draws square pixels, so the 131 x 24
//! canvas is 520 x 95 units, centred in the measured glass, which is drawn
//! as the skin's glass around it.

use super::*;

/// The black skirt every cap stands on, beyond the cap on every side.
pub const SKIRT: i16 = 6;

/// The drawing's size.
pub const WIDTH: i16 = 674;
/// The drawing's height.
pub const HEIGHT: i16 = 1235;
/// The LCD canvas: 131 x 24 (16 rows and the annunciator strip).
pub const LCD: Rect = r(77, 258, 520, 95);
/// The LCD's background (and the glass around it).
pub const LCD_FILL: &str = "#b7c2a2";
/// The LCD glass, measured: taller than the canvas, which is centred in it.
pub const GLASS: Rect = r(64, 226, 546, 158);
/// The saturnus logo, on the HP logo's square.
pub const LOGO: Rect = r(74, 52, 52, 52);

/// Cap of a key in rows 1-3, column `col` (0-5): footprints at 53 + 100
/// col, 70 wide.
const fn top(col: i16, row: usize) -> Rect {
    r(59 + 100 * col, ROW_Y[row], 58, ROW_H[row])
}

/// Cap of the narrow key in rows 4-7 (footprint 51, 73 wide).
const fn narrow(row: usize) -> Rect {
    r(57, ROW_Y[row], 61, ROW_H[row])
}

/// Cap of wide key `col` (0-3) in rows 4-7: footprints at 165, 286, 408
/// and 530, 94 wide.
const fn wide(col: usize, row: usize) -> Rect {
    const X: [i16; 4] = [171, 292, 414, 536];
    r(X[col], ROW_Y[row], 82, ROW_H[row])
}

/// Cap tops per row (footprints at 517, 615, 713, 813, 912, 1012, 1111).
const ROW_Y: [i16; 7] = [523, 621, 719, 819, 918, 1018, 1117];
/// Cap heights per row (footprints 55 in rows 1-2, 58 below).
const ROW_H: [i16; 7] = [43, 43, 46, 46, 46, 46, 46];

/// The key rectangles, in reading order: rows 1-2 (six each),
/// row 3 (INPUT and four), rows 4-7 (narrow and four).
pub const fn grid() -> [Rect; 37] {
    [
        top(0, 0),
        top(1, 0),
        top(2, 0),
        top(3, 0),
        top(4, 0),
        top(5, 0),
        top(0, 1),
        top(1, 1),
        top(2, 1),
        top(3, 1),
        top(4, 1),
        top(5, 1),
        // INPUT: footprint from column 0's left to column 1's right.
        r(59, ROW_Y[2], 158, ROW_H[2]),
        top(2, 2),
        top(3, 2),
        top(4, 2),
        top(5, 2),
        narrow(3),
        wide(0, 3),
        wide(1, 3),
        wide(2, 3),
        wide(3, 3),
        narrow(4),
        wide(0, 4),
        wide(1, 4),
        wide(2, 4),
        wide(3, 4),
        narrow(5),
        wide(0, 5),
        wide(1, 5),
        wide(2, 5),
        wide(3, 5),
        narrow(6),
        wide(0, 6),
        wide(1, 6),
        wide(2, 6),
        wide(3, 6),
    ]
}

/// The shared key rectangles.
pub const GRID: [Rect; 37] = grid();

/// A key of the grid: name `name` at grid position `i`.
pub const fn key(
    i: usize,
    name: &'static str,
    cap: Cap,
    label: &'static str,
    left: &'static str,
    below: &'static str,
) -> SkinKey {
    k(name, GRID[i], cap, label, left, "", "", below)
}

/// Index of the display surround among [`plates`].
#[cfg(test)]
pub const SURROUND: usize = 2;
/// Index of the keyboard plate among [`plates`].
#[cfg(test)]
pub const PLATE: usize = 4;
/// Index of the band behind the menu row among [`plates`].
#[cfg(test)]
pub const BAND: usize = 5;

/// The plates, back to front: the case, its recessed rim, the display
/// surround, the LCD bezel (the page draws the glass, [`GLASS`]), the
/// keyboard plate and the band
/// behind the menu row.
pub const fn plates(
    case: &'static str,
    rim: &'static str,
    surround: &'static str,
    bezel: &'static str,
    plate: &'static str,
    band: &'static str,
) -> [Panel; 6] {
    [
        panel(r(0, 0, WIDTH, HEIGHT), 24, 34, case),
        sunk(r(29, 124, 616, 1100), 12, 27, rim),
        raised(r(33, 128, 608, 338), 10, 0, surround),
        raised(r(50, 200, 574, 218), 4, 4, bezel),
        raised(r(33, 478, 608, 741), 0, 24, plate),
        panel(r(33, 478, 608, 105), 0, 0, band),
    ]
}

/// The grey bar behind a menu name over wide key `col` of row `row` (the
/// 27S's digit block): from 3 units left of the footprint to its right
/// edge, from 22 to 3 units above it.
pub const fn bar(col: usize, row: usize, fill: &'static str) -> Panel {
    let cap = wide(col, row);
    panel(
        r(
            cap.x - SKIRT - 3,
            cap.y - SKIRT - 22,
            cap.w + 2 * SKIRT + 3,
            19,
        ),
        2,
        2,
        fill,
    )
}
