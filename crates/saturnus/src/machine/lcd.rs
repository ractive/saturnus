//! The LCD: 131x64 on the 48-family models, rendered from the display
//! bitmaps in memory (wiki: hardware/display); 131x16 on the 42S, rendered
//! from the Lewis display RAM (wiki: hardware/lewis "Display").

use crate::cpu::ADDR_MASK;
use crate::io::{IoRegisters, LewisIo};

/// LCD width in pixels (every model).
pub const LCD_WIDTH: usize = 131;
/// LCD height in pixels of the 48-family models.
pub const LCD_HEIGHT: usize = 64;
/// LCD height in pixels of the 42S: two lines of 8 rows.
pub const LCD_HEIGHT_42S: usize = 16;
/// Columns the Lewis's left column driver serves (0-65); the right one
/// serves 66-130 (wiki: hardware/lewis "Display").
const LEWIS_LEFT_COLUMNS: usize = 66;
/// First nibble of the seven Lewis annunciator words, eight nibbles apart
/// (#40218-#4024C).
const LEWIS_ANNUNCIATORS: usize = 0x218;
/// The Lewis word that lights every annunciator (#40210).
const LEWIS_ANNUNCIATOR_ALL: usize = 0x210;
/// Nibbles per bitmap row without the line offset (131 pixels rounded up
/// to whole bytes: 136 bits = 34 nibbles).
const ROW_NIBBLES: i64 = 34;

/// A snapshot of the LCD pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lcd {
    /// Rows of 131 pixels, `true` = dark: 64 rows on the 48-family
    /// models, 16 on the 42S.
    pub pixels: Vec<[bool; LCD_WIDTH]>,
}

impl Lcd {
    /// A blank 131x64 display.
    pub fn blank() -> Self {
        Self::blank_rows(LCD_HEIGHT)
    }

    /// A blank display of `rows` rows.
    pub fn blank_rows(rows: usize) -> Self {
        Self {
            pixels: vec![[false; LCD_WIDTH]; rows],
        }
    }

    /// Height in pixels.
    pub fn height(&self) -> usize {
        self.pixels.len()
    }

    /// Render the 42S's 131x16 display from the Lewis display RAM.
    ///
    /// Two column drivers share the RAM byte by byte: byte `4k` holds
    /// column `k` of the upper line, `4k+1` column `k` of the lower line
    /// (k = 0-65), `4k+2` and `4k+3` the same for column `66+k` (k =
    /// 0-64); 524 nibbles from #40000. Bit 0 of a byte is the top row of
    /// its line. Put the other way: column `c` is four consecutive
    /// nibbles, top to bottom, least significant bit on top, at #40000 +
    /// 8c for c < 66 and #40004 + 8(c-66) above (wiki:
    /// hardware/lewis "Display", checked on the ROM's "Memory Clear"
    /// screen). Blank while DON is clear.
    pub fn render_lewis(lewis: &LewisIo) -> Self {
        let mut lcd = Self::blank_rows(LCD_HEIGHT_42S);
        if !lewis.display_on() {
            return lcd;
        }
        for x in 0..LCD_WIDTH {
            let (k, half) = if x < LEWIS_LEFT_COLUMNS {
                (x, 0)
            } else {
                (x - LEWIS_LEFT_COLUMNS, 2)
            };
            for line in 0..2 {
                let byte = 4 * k + half + line;
                let bits = lewis.ram_nibble(2 * byte) | (lewis.ram_nibble(2 * byte + 1) << 4);
                for row in 0..8 {
                    lcd.pixels[8 * line + row][x] = bits >> row & 1 != 0;
                }
            }
        }
        lcd
    }

    /// Render from the display registers in `io`, reading bitmap nibbles
    /// through `peek`.
    ///
    /// Rows `0..=line_count` come from the main bitmap at `display_start`,
    /// shifted left by `bit_offset` pixels; a line count of 0 or 1 is
    /// treated as 63 (inferred). After each main row the address advances
    /// by 34 nibbles plus the line offset plus 2 when the bit offset spans a
    /// nibble, aligned down to a byte. The remaining rows come from the menu
    /// bitmap at `menu_start`, 34 nibbles per row (wiki: hardware/display).
    pub fn render(io: &IoRegisters, peek: impl Fn(u32) -> u8) -> Self {
        let mut lcd = Self::blank();
        if !io.display_on() {
            return lcd;
        }
        for (row, (addr, offset)) in lcd.pixels.iter_mut().zip(row_starts(io)) {
            render_row(row, addr, offset, &peek);
        }
        lcd
    }

    /// The first and last nibble address each of the 64 rows reads, in
    /// row order; [`Lcd::render`] reads exactly the nibbles from the first
    /// to the last of every row, through the mapping.
    pub fn row_spans(io: &IoRegisters) -> impl Iterator<Item = (u32, u32)> {
        row_starts(io).map(|(addr, offset)| {
            let last = ((LCD_WIDTH - 1 + offset) / 4) as u32;
            (addr, addr.wrapping_add(last) & ADDR_MASK)
        })
    }

    /// One line of 131 characters per row (64, or 16 on the 42S), `#`
    /// for a dark pixel and `.` for a light one, each line ended by `\n`.
    pub fn to_text(&self) -> String {
        let mut s = String::with_capacity((LCD_WIDTH + 1) * self.pixels.len());
        for row in &self.pixels {
            s.extend(row.iter().map(|&on| if on { '#' } else { '.' }));
            s.push('\n');
        }
        s
    }
}

/// Start address and bit offset of each of the 64 rows: rows
/// `0..=line_count` from the main bitmap, shifted left by the bit offset;
/// a line count of 0 or 1 is treated as 63 (inferred). After each main row
/// the address advances by 34 nibbles plus the line offset plus 2 when the
/// bit offset spans a nibble, aligned down to a byte. The remaining rows
/// come from the menu bitmap, 34 nibbles per row, no offset (wiki:
/// hardware/display).
fn row_starts(io: &IoRegisters) -> impl Iterator<Item = (u32, usize)> {
    let last_main = match io.line_count() {
        0 | 1 => LCD_HEIGHT - 1,
        n => usize::from(n).min(LCD_HEIGHT - 1),
    };
    let offset = usize::from(io.bit_offset());
    let main_stride = ROW_NIBBLES + i64::from(io.line_offset()) + 2 * (offset as i64 / 4);
    let main = std::iter::successors(Some(io.display_start()), move |&a| {
        Some((((i64::from(a) + main_stride) as u32) & ADDR_MASK) & !1)
    })
    .take(last_main + 1)
    .map(move |a| (a, offset));
    let menu = std::iter::successors(Some(io.menu_start()), |&a| {
        Some((a.wrapping_add(ROW_NIBBLES as u32) & ADDR_MASK) & !1)
    })
    .take(LCD_HEIGHT - 1 - last_main)
    .map(|a| (a, 0));
    main.chain(menu)
}

/// Fill one row: pixel x is bit `(x+offset)%4` of the nibble at
/// `addr + (x+offset)/4`, least significant bit leftmost.
fn render_row(row: &mut [bool; LCD_WIDTH], addr: u32, offset: usize, peek: &impl Fn(u32) -> u8) {
    for (x, px) in row.iter_mut().enumerate() {
        let bit = x + offset;
        let nib = peek(addr.wrapping_add((bit / 4) as u32) & ADDR_MASK);
        *px = nib >> (bit % 4) & 1 != 0;
    }
}

/// The annunciators above the pixel area: six on the 48-family models
/// (wiki: hardware/display, bit names after Giesselink: #10B LA1-LA4, #10C
/// LA5-LA6), seven on the 42S (wiki: hardware/lewis "Annunciators"). The
/// 42S's shift, print and run annunciators are reported as `left_shift`,
/// `transmitting` and `busy`; `updown`, `battery`, `g` and `rad` exist
/// only on the 42S.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Annunciators {
    /// Left shift (#10B bit 0); the 42S's shift.
    pub left_shift: bool,
    /// Right shift (#10B bit 1).
    pub right_shift: bool,
    /// Alpha (#10B bit 2).
    pub alpha: bool,
    /// Alert, the bell (#10B bit 3).
    pub alert: bool,
    /// Busy (#10C bit 0); the 42S's run annunciator.
    pub busy: bool,
    /// Transmitting, the I/O arrow (#10C bit 1); the 42S's print
    /// annunciator.
    pub transmitting: bool,
    /// 42S: more menu rows (▲▼).
    pub updown: bool,
    /// 42S: low battery.
    pub battery: bool,
    /// 42S: "G" (lit with RAD in GRAD mode).
    pub g: bool,
    /// 42S: "RAD".
    pub rad: bool,
}

impl Annunciators {
    /// Decode the annunciator byte (#10B in bits 0-3, #10C in bits 4-7).
    /// Nothing is lit unless AON (#10C bit 3) is set; AON is independent of
    /// DON (wiki: hardware/display "Bit names").
    pub fn from_bits(bits: u8) -> Self {
        if bits & 0x80 == 0 {
            return Self::default();
        }
        Self {
            left_shift: bits & 0x01 != 0,
            right_shift: bits & 0x02 != 0,
            alpha: bits & 0x04 != 0,
            alert: bits & 0x08 != 0,
            busy: bits & 0x10 != 0,
            transmitting: bits & 0x20 != 0,
            ..Self::default()
        }
    }

    /// The 42S's seven annunciators from the Lewis display RAM: 5-nibble
    /// words, eight nibbles apart, at #40218 (▲▼), #40220 (shift), #40228
    /// (print), #40230 (busy), #40238 (battery), #40240 (G) and #40248
    /// (RAD); the ROM writes #FFFFF (on) or 0 (off). The word at #40210
    /// lights all of them (the ROM's display test writes it). Shift, ▲▼,
    /// battery,
    /// G and RAD were also seen in the ROM's behaviour (wiki:
    /// hardware/lewis "Annunciators"). A word counts as lit when its
    /// first nibble is not 0 (inferred), and nothing is lit while DON is
    /// clear (inferred: the words are display RAM).
    pub fn from_lewis(lewis: &LewisIo) -> Self {
        if !lewis.display_on() {
            return Self::default();
        }
        let all = lewis.ram_nibble(LEWIS_ANNUNCIATOR_ALL) != 0;
        let lit = |i: usize| all || lewis.ram_nibble(LEWIS_ANNUNCIATORS + 8 * i) != 0;
        Self {
            updown: lit(0),
            left_shift: lit(1),
            transmitting: lit(2),
            busy: lit(3),
            battery: lit(4),
            g: lit(5),
            rad: lit(6),
            ..Self::default()
        }
    }

    /// Names and states: the 48's six left to right as on its label
    /// strip, then the 42S-only ones.
    pub fn list(&self) -> [(&'static str, bool); 10] {
        [
            ("leftshift", self.left_shift),
            ("rightshift", self.right_shift),
            ("alpha", self.alpha),
            ("alert", self.alert),
            ("busy", self.busy),
            ("transmit", self.transmitting),
            ("updown", self.updown),
            ("battery", self.battery),
            ("g", self.g),
            ("rad", self.rad),
        ]
    }

    /// One line naming the lit annunciators separated by spaces, or `-`
    /// when none is lit; no trailing newline.
    pub fn to_line(&self) -> String {
        let lit: Vec<&str> = self
            .list()
            .iter()
            .filter(|(_, on)| *on)
            .map(|(n, _)| *n)
            .collect();
        if lit.is_empty() {
            "-".to_string()
        } else {
            lit.join(" ")
        }
    }
}

/// Everything the display shows: pixels, annunciators and contrast.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framebuffer {
    /// The pixel area: 131x64, or 131x16 on the 42S.
    pub pixels: Lcd,
    /// The annunciator row.
    pub annunciators: Annunciators,
    /// 5-bit contrast, 0-31, higher is darker (#101 bits 0-3, #102 bit 0).
    pub contrast: u8,
}

impl Framebuffer {
    /// The pixel area as text, identical to [`Lcd::to_text`].
    pub fn to_text(&self) -> String {
        self.pixels.to_text()
    }

    /// The annunciators as one line, see [`Annunciators::to_line`].
    pub fn annunciator_line(&self) -> String {
        self.annunciators.to_line()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annunciators_need_aon() {
        assert_eq!(Annunciators::from_bits(0x3F), Annunciators::default());
        let a = Annunciators::from_bits(0x80 | 0x21);
        assert!(a.left_shift && a.transmitting);
        assert!(!a.right_shift && !a.alpha && !a.alert && !a.busy);
        assert_eq!(a.to_line(), "leftshift transmit");
        assert_eq!(Annunciators::default().to_line(), "-");
        // XTRA (#10C bit 2) is not an annunciator.
        assert_eq!(
            Annunciators::from_bits(0x80 | 0x40),
            Annunciators::default()
        );
    }
}
