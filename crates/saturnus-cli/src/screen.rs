//! Screen dumps: the text form shared with the oracle diff script, and a
//! 1-bit PNG.

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
fn png_rows(lcd: &Lcd) -> Vec<u8> {
    let stride = LCD_WIDTH.div_ceil(8);
    let mut data = vec![0xFFu8; stride * LCD_HEIGHT];
    for (y, row) in lcd.pixels.iter().enumerate() {
        for (x, &dark) in row.iter().enumerate() {
            if dark {
                data[y * stride + x / 8] &= !(0x80 >> (x % 8));
            }
        }
    }
    data
}

fn write_png(lcd: &Lcd, path: &Path) -> Result<()> {
    let file = File::create(path).with_context(|| format!("cannot create {}", path.display()))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), LCD_WIDTH as u32, LCD_HEIGHT as u32);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::One);
    let mut w = enc
        .write_header()
        .with_context(|| format!("cannot write PNG header to {}", path.display()))?;
    w.write_image_data(&png_rows(lcd))
        .with_context(|| format!("cannot write PNG data to {}", path.display()))?;
    w.finish()
        .with_context(|| format!("cannot finish PNG {}", path.display()))
}

#[cfg(test)]
mod tests {
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
}
