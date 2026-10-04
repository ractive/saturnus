//! The 131x64 LCD, rendered from the display bitmaps in memory
//! (wiki: hardware/display).

use crate::cpu::ADDR_MASK;
use crate::io::IoRegisters;

/// LCD width in pixels.
pub const LCD_WIDTH: usize = 131;
/// LCD height in pixels.
pub const LCD_HEIGHT: usize = 64;
/// Nibbles per bitmap row without the line offset (131 pixels rounded up
/// to whole bytes: 136 bits = 34 nibbles).
const ROW_NIBBLES: i64 = 34;

/// A snapshot of the LCD pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lcd {
    /// 64 rows of 131 pixels, `true` = dark.
    pub pixels: Vec<[bool; LCD_WIDTH]>,
}

impl Lcd {
    /// A blank display.
    pub fn blank() -> Self {
        Self {
            pixels: vec![[false; LCD_WIDTH]; LCD_HEIGHT],
        }
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
        let last_main = match io.line_count() {
            0 | 1 => LCD_HEIGHT - 1,
            n => usize::from(n).min(LCD_HEIGHT - 1),
        };
        let offset = usize::from(io.bit_offset());
        let main_stride = ROW_NIBBLES + i64::from(io.line_offset()) + 2 * (offset as i64 / 4);
        let mut addr = io.display_start();
        for row in lcd.pixels.iter_mut().take(last_main + 1) {
            render_row(row, addr, offset, &peek);
            addr = (((i64::from(addr) + main_stride) as u32) & ADDR_MASK) & !1;
        }
        let mut addr = io.menu_start();
        for row in lcd.pixels.iter_mut().skip(last_main + 1) {
            render_row(row, addr, 0, &peek);
            addr = (addr.wrapping_add(ROW_NIBBLES as u32) & ADDR_MASK) & !1;
        }
        lcd
    }

    /// 64 lines of 131 characters, `#` for a dark pixel and `.` for a
    /// light one, each line ended by `\n`.
    pub fn to_text(&self) -> String {
        let mut s = String::with_capacity((LCD_WIDTH + 1) * self.pixels.len());
        for row in &self.pixels {
            s.extend(row.iter().map(|&on| if on { '#' } else { '.' }));
            s.push('\n');
        }
        s
    }
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
