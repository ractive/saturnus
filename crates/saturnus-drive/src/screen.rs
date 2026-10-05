//! Screen dumps: the text form shared with the oracle diff script, and a
//! 1-bit PNG, to a file or in memory.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result, bail};
use saturnus::machine::{LCD_HEIGHT, LCD_WIDTH, Lcd};

/// Write `lcd` to `path`: `.txt` gets [`Lcd::to_text`], `.png` a 131x64
/// 1-bit grayscale image (dark pixel = black).
pub fn write(lcd: &Lcd, path: &Path) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("txt") => std::fs::write(path, lcd.to_text())
            .with_context(|| format!("cannot write {}", path.display())),
        Some("png") => write_png(lcd, path),
        _ => bail!(
            "unsupported screen file {}: use a .txt or .png extension",
            path.display()
        ),
    }
}

/// Pack the pixels as 1-bit grayscale rows, most significant bit leftmost,
/// 0 = black (a dark LCD pixel).
#[cfg(test)]
fn png_rows(lcd: &Lcd) -> Vec<u8> {
    scaled_rows(lcd, 1)
}

/// [`png_rows`] with every pixel drawn as a `scale` x `scale` block.
fn scaled_rows(lcd: &Lcd, scale: usize) -> Vec<u8> {
    let width = LCD_WIDTH * scale;
    let stride = width.div_ceil(8);
    let mut data = vec![0xFFu8; stride * LCD_HEIGHT * scale];
    for (y, row) in lcd.pixels.iter().enumerate() {
        for (x, &dark) in row.iter().enumerate() {
            if !dark {
                continue;
            }
            for dy in 0..scale {
                let line = (y * scale + dy) * stride;
                for dx in 0..scale {
                    let px = x * scale + dx;
                    data[line + px / 8] &= !(0x80 >> (px % 8));
                }
            }
        }
    }
    data
}

/// Largest scale factor [`png_bytes`] accepts.
pub const MAX_SCALE: u32 = 8;

/// Encode `lcd` as a 1-bit grayscale PNG with each pixel drawn `scale`
/// times wider and taller (1 gives 131x64).
pub fn png_bytes(lcd: &Lcd, scale: u32) -> Result<Vec<u8>> {
    if !(1..=MAX_SCALE).contains(&scale) {
        bail!("PNG scale {scale} is out of range 1-{MAX_SCALE}");
    }
    let mut out = Vec::new();
    encode_png(lcd, scale, &mut out)?;
    Ok(out)
}

fn encode_png(lcd: &Lcd, scale: u32, sink: impl std::io::Write) -> Result<()> {
    let (w, h) = (LCD_WIDTH as u32 * scale, LCD_HEIGHT as u32 * scale);
    let mut enc = png::Encoder::new(sink, w, h);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::One);
    let mut writer = enc.write_header().context("cannot write PNG header")?;
    writer
        .write_image_data(&scaled_rows(lcd, scale as usize))
        .context("cannot write PNG data")?;
    writer.finish().context("cannot finish PNG")
}

fn write_png(lcd: &Lcd, path: &Path) -> Result<()> {
    let file = File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    encode_png(lcd, 1, BufWriter::new(file))
        .with_context(|| format!("cannot write PNG {}", path.display()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn png_rows_pack_dark_as_zero_msb_first() {
        let mut lcd = Lcd::blank();
        lcd.pixels[0][0] = true;
        lcd.pixels[0][9] = true;
        lcd.pixels[63][130] = true;
        let rows = png_rows(&lcd);
        let stride = 17;
        assert_eq!(rows.len(), stride * 64);
        assert_eq!(rows[0], 0x7F);
        assert_eq!(rows[1], 0xBF);
        // x = 130 is bit 2 of byte 16 (MSB first).
        assert_eq!(rows[63 * stride + 16], !0x20);
    }

    #[test]
    fn png_round_trips_through_the_decoder() {
        let mut lcd = Lcd::blank();
        lcd.pixels[5][7] = true;
        let dir = std::env::temp_dir().join(format!("saturnus-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.png");
        write(&lcd, &path).unwrap();
        let dec = png::Decoder::new(std::io::BufReader::new(File::open(&path).unwrap()));
        let reader = dec.read_info().unwrap();
        let info = reader.info();
        assert_eq!((info.width, info.height), (131, 64));
        assert_eq!(info.bit_depth, png::BitDepth::One);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_unknown_extension() {
        assert!(write(&Lcd::blank(), Path::new("screen.bmp")).is_err());
    }

    #[test]
    fn scaled_png_decodes_to_the_scaled_size() {
        let mut lcd = Lcd::blank();
        lcd.pixels[0][1] = true;
        let rows = scaled_rows(&lcd, 3);
        let stride = (131 * 3usize).div_ceil(8);
        // Pixel x = 1 covers bits 3-5 of byte 0 on the first three lines.
        for line in 0..3 {
            assert_eq!(rows[line * stride], 0b1110_0011);
        }
        assert_eq!(rows[3 * stride], 0xFF);
        let png = png_bytes(&lcd, 3).unwrap();
        let reader = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        assert_eq!((reader.info().width, reader.info().height), (393, 192));
        assert!(png_bytes(&lcd, 0).is_err());
        assert!(png_bytes(&lcd, MAX_SCALE + 1).is_err());
    }
}
