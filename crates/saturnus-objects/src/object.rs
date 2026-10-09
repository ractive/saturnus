//! Typed calculator objects (the JSON model of saturnus's tools) and the
//! exact decoder for objects in the calculator's nibble format, whether
//! from a binary transfer or read straight from memory.
//!
//! Body layouts: wiki: protocols/hp-object-format ("Object bodies"), from
//! RPLMAN chapter 3 and objects observed on the 48SX and 49G.
//!
//! - Reals keep their exact 12-digit mantissa ([`Real`]); JSON shows them as
//!   numbers (12 significant digits survive an `f64`) unless the exponent
//!   is beyond an `f64`'s range, then as text such as `"1.5E-400"`.
//! - Built-in constants inside composites are 5-nibble ROM pointers; a
//!   [`Memory`] reads the object they point to (the emulator's ROM). A
//!   pointer to a command (a program, code or a primitive in ROM) is not
//!   followed: it becomes [`Object::Command`] with its address, and with
//!   its name when the reader has the ROM's [`NameTable`].
//! - With a [`NameTable`], programs, algebraics and unit expressions get
//!   the calculator's text ([`crate::decompile`]); without one their
//!   `source` stays unset and a host may fill it from the calculator's own
//!   text (an ASCII transfer: [`crate::transfer::fill_sources`]).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;

use anyhow::{Context, Result, bail};
use serde::de::{self, Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use crate::charset;
use crate::decompile::{self, Element, Settings, UnitOp};
use crate::names::NameTable;
use crate::prolog::{MAX_DEPTH, ObjectType, SEMI, object_size, read_field};

/// Longest hex dump of an unknown object, in nibbles.
const MAX_HEX_NIBBLES: usize = 4096;
/// Largest object read from memory, in nibbles: the whole address space.
const MAX_MEMORY_OBJECT: usize = 1 << 20;
/// Most objects one decode may produce. ROM pointers can share a target,
/// so without a bound a few hundred nibbles of crafted or corrupt memory
/// (lists of two pointers to the next such list) expand to 2^64 objects.
/// The largest object a calculator holds is under 512 K nibbles (the
/// 49G's 256 KB of RAM; a 48GX card port holds at most 128 KB), so it has
/// at most about 105 000 elements (5-nibble ROM pointers, the smallest);
/// 2^18 is two and a half times that. The work to reach the bound grows
/// with it (every object is built before the next is charged): 2^18 keeps
/// a refused decode near 50 ms in a debug build.
pub const MAX_DECODED_OBJECTS: usize = 1 << 18;
/// Most nibbles one decode may read from memory, each object once: four
/// times the 49G's whole RAM (512 K nibbles), so nothing a calculator
/// holds is refused.
pub const MAX_DECODED_NIBBLES: usize = 1 << 21;
/// Most nibbles one decode may hand out again for pointers that repeat an
/// object already read: DUP copies the pointer, not the object, so a real
/// stack can hold one large object at many levels, and each level is a
/// copy in the result. 16 M nibbles (8 MB of object data) covers forty
/// copies of a 400 K-nibble GROB; past it a crafted stack of thousands of
/// pointers to one large object is refused instead of producing gigabytes.
pub const MAX_CLONED_NIBBLES: usize = 1 << 24;
/// Most dimensions of an array: the calculator's real and complex arrays
/// are vectors or matrices (RPLMAN chapter 3). More is refused, so no
/// object's header sets how deep the rows nest.
const MAX_ARRAY_DIMS: usize = 2;
/// Exponents an `f64` carries without overflow or loss of digits.
const F64_EXPONENTS: std::ops::RangeInclusive<i32> = -307..=307;

/// A real number exactly as the calculator stores it: sign, 12 decimal
/// digits with the point after the first, exponent -499..=499.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Real {
    /// Negative.
    pub negative: bool,
    /// Mantissa digits, most significant first; the first is nonzero unless
    /// the number is 0.
    pub digits: [u8; 12],
    /// Decimal exponent.
    pub exponent: i32,
}

impl Real {
    /// Zero.
    pub const ZERO: Real = Real {
        negative: false,
        digits: [0; 12],
        exponent: 0,
    };

    /// Decode a 16-nibble real body: 3-nibble tens-complement exponent, 12
    /// mantissa digits from the least significant up, sign (0 or 9).
    pub fn from_body(body: &[u8]) -> Result<Real> {
        let body = body
            .get(..16)
            .context("real body shorter than 16 nibbles")?;
        if body.iter().any(|&d| d > 9) {
            bail!("real body is not BCD: {}", hex(body));
        }
        let e = i32::from(body[0]) + 10 * i32::from(body[1]) + 100 * i32::from(body[2]);
        let exponent = if e >= 500 { e - 1000 } else { e };
        let mut digits = [0u8; 12];
        for (i, d) in digits.iter_mut().enumerate() {
            *d = body[14 - i];
        }
        let negative = match body[15] {
            0 => false,
            9 => true,
            s => bail!("real sign nibble {s} is neither 0 nor 9"),
        };
        Ok(Real {
            negative,
            digits,
            exponent,
        })
    }

    /// The 16-nibble body.
    pub fn to_body(&self) -> [u8; 16] {
        let mut body = [0u8; 16];
        let e = u32::try_from(self.exponent.rem_euclid(1000)).unwrap_or(0);
        body[0] = (e % 10) as u8;
        body[1] = (e / 10 % 10) as u8;
        body[2] = (e / 100) as u8;
        for (i, d) in self.digits.iter().enumerate() {
            body[14 - i] = *d;
        }
        body[15] = if self.negative { 9 } else { 0 };
        body
    }

    fn is_zero(&self) -> bool {
        self.digits.iter().all(|&d| d == 0)
    }

    /// The nearest `f64` (exact to 12 digits within `F64_EXPONENTS`).
    pub fn to_f64(&self) -> f64 {
        format!("{}e{}", self.mantissa_text(), self.exponent)
            .parse()
            .unwrap_or(f64::NAN)
    }

    /// `d.ddd` with trailing zeros dropped (`5.`, `4.79425538604`), signed.
    fn mantissa_text(&self) -> String {
        let mut s = String::new();
        if self.negative {
            s.push('-');
        }
        s.push(char::from(b'0' + self.digits[0]));
        s.push('.');
        let last = self.digits.iter().rposition(|&d| d != 0).unwrap_or(0);
        for d in &self.digits[1..=last] {
            s.push(char::from(b'0' + d));
        }
        s
    }

    /// RPL source that compiles to exactly this real: `5.`, `-1.5E-300`,
    /// `4.79425538604E-1`.
    pub fn to_source(&self) -> String {
        if self.exponent == 0 {
            self.mantissa_text()
        } else {
            format!("{}E{}", self.mantissa_text(), self.exponent)
        }
    }

    /// Parse decimal text (`0.5`, `-1.5E-300`, `12`, `.479425538604`),
    /// rounding to 12 significant digits (half away from zero).
    pub fn parse(text: &str) -> Result<Real> {
        let bad = || anyhow::anyhow!("not a real number: {text:?}");
        let t = text.trim();
        let (negative, t) = match t.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        let (mant, exp) = match t.find(['e', 'E']) {
            Some(i) => (&t[..i], t[i + 1..].parse::<i32>().map_err(|_| bad())?),
            None => (t, 0),
        };
        let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
        if (int.is_empty() && frac.is_empty())
            || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit())
        {
            return Err(bad());
        }
        let all: Vec<u8> = int.bytes().chain(frac.bytes()).map(|b| b - b'0').collect();
        let Some(first) = all.iter().position(|&d| d != 0) else {
            return Ok(Real::ZERO);
        };
        // Value = 0.ALL * 10^(int.len() + exp); the first nonzero digit at
        // index `first` has weight 10^(int.len() - 1 - first + exp).
        let int_len = i32::try_from(int.len()).map_err(|_| bad())?;
        let first_i = i32::try_from(first).map_err(|_| bad())?;
        let mut exponent = int_len - 1 - first_i + exp;
        let sig = &all[first..];
        let mut digits = [0u8; 12];
        for (d, s) in digits.iter_mut().zip(sig) {
            *d = *s;
        }
        if sig.get(12).is_some_and(|&d| d >= 5) {
            // Round up, carrying.
            let mut i = 12;
            loop {
                if i == 0 {
                    digits = [0; 12];
                    digits[0] = 1;
                    exponent += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
        if !(-499..=499).contains(&exponent) {
            bail!("{text:?} is outside the calculator's range (exponents -499 to 499)");
        }
        Ok(Real {
            negative,
            digits,
            exponent,
        })
    }

    /// The real nearest to `v`, rounded to 12 digits.
    pub fn from_f64(v: f64) -> Result<Real> {
        if !v.is_finite() {
            bail!("{v} is not a finite number");
        }
        Real::parse(&format!("{v:e}"))
    }
}

impl fmt::Display for Real {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_source())
    }
}

impl Serialize for Real {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        if self.is_zero() || F64_EXPONENTS.contains(&self.exponent) {
            s.serialize_f64(self.to_f64())
        } else {
            s.serialize_str(&self.to_source())
        }
    }
}

impl<'de> Deserialize<'de> for Real {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Real, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Real;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a number or decimal text such as \"1.5E-400\"")
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Real, E> {
                Real::from_f64(v).map_err(E::custom)
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Real, E> {
                Real::parse(&v.to_string()).map_err(E::custom)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Real, E> {
                Real::parse(&v.to_string()).map_err(E::custom)
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Real, E> {
                Real::parse(v).map_err(E::custom)
            }
        }
        d.deserialize_any(V)
    }
}

/// A 49G exact integer: sign and decimal digits, any length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Integer {
    /// Negative (never for zero).
    pub negative: bool,
    /// Decimal digits, most significant first, no leading zeros; `0` for
    /// zero.
    pub digits: String,
}

impl Integer {
    /// Parse `-123`, `456`.
    pub fn parse(text: &str) -> Result<Integer> {
        let t = text.trim();
        let (negative, d) = match t.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        if d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) {
            bail!("not an integer: {text:?}");
        }
        let digits = d.trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        Ok(Integer {
            negative: negative && digits != "0",
            digits: digits.to_string(),
        })
    }

    /// Decode the nibbles after the length field: digits from the least
    /// significant up, then the sign nibble (0 or 9). Zero is one 0 nibble.
    pub fn from_nibbles(n: &[u8]) -> Result<Integer> {
        let Some((&sign, digits)) = n.split_last() else {
            bail!("empty integer");
        };
        if n.iter().any(|&d| d > 9) {
            bail!("integer body is not BCD: {}", hex(n));
        }
        let text: String = digits.iter().rev().map(|d| char::from(b'0' + d)).collect();
        let mut i = Integer::parse(if text.is_empty() { "0" } else { &text })?;
        i.negative = sign == 9 && i.digits != "0";
        Ok(i)
    }

    /// The nibbles after the length field (see [`Integer::from_nibbles`]).
    pub fn to_nibbles(&self) -> Vec<u8> {
        if self.digits == "0" {
            return vec![0];
        }
        let mut n: Vec<u8> = self.digits.bytes().rev().map(|b| b - b'0').collect();
        n.push(if self.negative { 9 } else { 0 });
        n
    }

    /// RPL source: `-123`.
    pub fn to_source(&self) -> String {
        if self.negative {
            format!("-{}", self.digits)
        } else {
            self.digits.clone()
        }
    }
}

impl Serialize for Integer {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        // Up to 15 digits fit an f64 exactly, so every JSON reader agrees.
        match self.to_source().parse::<i64>() {
            Ok(v) if self.digits.len() <= 15 => s.serialize_i64(v),
            _ => s.serialize_str(&self.to_source()),
        }
    }
}

impl<'de> Deserialize<'de> for Integer {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Integer, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Integer;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an integer or a string of decimal digits")
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Integer, E> {
                Integer::parse(&v.to_string()).map_err(E::custom)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Integer, E> {
                Integer::parse(&v.to_string()).map_err(E::custom)
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Integer, E> {
                if v.fract() != 0.0 || !v.is_finite() || v.abs() > 9.0e15 {
                    return Err(E::custom(format!("{v} is not an exact integer")));
                }
                Integer::parse(&format!("{v:.0}")).map_err(E::custom)
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Integer, E> {
                Integer::parse(v).map_err(E::custom)
            }
        }
        d.deserialize_any(V)
    }
}

/// The display base of binary integers (flags -11 and -12).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    /// `h`.
    Hex,
    /// `d`.
    Dec,
    /// `o`.
    Oct,
    /// `b`.
    Bin,
}

impl Base {
    /// The base from the suffix of a displayed binary integer (`# 2Ah`).
    pub fn from_display(text: &str) -> Option<Base> {
        let t = text.trim();
        if !t.starts_with('#') {
            return None;
        }
        match t.chars().last()? {
            'h' => Some(Base::Hex),
            'd' => Some(Base::Dec),
            'o' => Some(Base::Oct),
            'b' => Some(Base::Bin),
            _ => None,
        }
    }

    /// `value` as the calculator shows it, e.g. `# 2Ah`.
    pub fn format(self, value: u64) -> String {
        match self {
            Base::Hex => format!("# {value:X}h"),
            Base::Dec => format!("# {value}d"),
            Base::Oct => format!("# {value:o}o"),
            Base::Bin => format!("# {value:b}b"),
        }
    }
}

/// One array element or a row of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArrayItem {
    /// A row (one level of a multi-dimensional array).
    Row(Vec<ArrayItem>),
    /// An element.
    Item(Box<Object>),
}

/// A calculator object as the semantic tools return and accept it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Object {
    /// A real number, exact to its 12 digits.
    Real {
        /// The value (a JSON number, or text beyond an f64's exponent range).
        value: Real,
    },
    /// A 49G exact integer.
    Integer {
        /// The value (a JSON number up to 15 digits, else decimal text).
        value: Integer,
    },
    /// A complex number.
    Complex {
        /// Real part.
        re: Real,
        /// Imaginary part.
        im: Real,
    },
    /// A string (HP characters as Unicode).
    String {
        /// The text.
        value: String,
    },
    /// A global name.
    Name {
        /// The name.
        value: String,
    },
    /// A local (temporary) name.
    LocalName {
        /// The name.
        value: String,
    },
    /// A character object.
    Character {
        /// The character.
        value: String,
    },
    /// A binary integer (`# 2Ah`).
    Binary {
        /// The value (up to 64 bits).
        value: u64,
        /// The calculator's display base, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base: Option<Base>,
        /// The value as the calculator shows it in that base.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    /// A list.
    List {
        /// The elements.
        items: Vec<Object>,
    },
    /// A tagged object `:tag:object`.
    Tagged {
        /// The tag.
        tag: String,
        /// The tagged object.
        object: Box<Object>,
    },
    /// A unit object `value_unit`.
    Unit {
        /// The number, exact.
        value: Real,
        /// The unit expression, e.g. `m/s^2`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
    },
    /// A real or complex array (vector or matrix).
    Array {
        /// Dimensions, e.g. `[2, 3]` for 2 rows of 3.
        dims: Vec<usize>,
        /// Elements, nested by dimension (rows of elements for a matrix).
        items: Vec<ArrayItem>,
    },
    /// A program.
    Program {
        /// Its source, e.g. `« 1 2 + »`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// An algebraic expression.
    Algebraic {
        /// Its source with quotes, e.g. `'X^2'`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// A built-in command (a ROM pointer to a program, code or primitive,
    /// or an XLIB name).
    Command {
        /// Its name from the ROM's tables, e.g. `+`; absent when they have
        /// none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// The ROM address it points to (not for XLIB names).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        address: Option<u32>,
        /// Its library number, when known (always for an XLIB name; an
        /// XLIB name without `name` is a command of a library the ROM's
        /// tables do not hold, which the calculator shows `XLIB l n`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        library: Option<u16>,
        /// Its command number in the library, with `library`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<u16>,
        /// Its text from an ASCII transfer, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// Any other object: its prolog, type name and nibbles.
    Unknown {
        /// Prolog address, e.g. `02B1E`.
        prolog: String,
        /// Type name, e.g. `Graphic`, when the prolog is known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        /// Size in nibbles.
        nibbles: usize,
        /// The object's nibbles in memory order (prolog first), at most 4096.
        hex: String,
        /// Whether `hex` was cut.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        truncated: bool,
        /// The calculator's text for it, when known.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        /// A graphic's picture ([`Graphic`]), for a GROB of a sane size.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        graphic: Option<Graphic>,
    },
}

/// A GROB's picture: `width` by `height` pixels, `rows` the pixels packed
/// one bit each as hex digits (two per byte), every row starting on a
/// byte, the leftmost pixel in the most significant bit, 1 dark: the
/// page's frame layout, so a picture is drawn like the screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graphic {
    /// Pixels per row.
    pub width: u32,
    /// Rows.
    pub height: u32,
    /// The packed rows, hex.
    pub rows: String,
}

/// The largest GROB pictured, in pixels: a 2048 by 2048 one.
const MAX_GRAPHIC_PIXELS: u64 = 2048 * 2048;

/// The picture of the GROB `n` (all its nibbles, prolog first), or `None`
/// when it is not a GROB, has no pixels (0 wide or high, as `#0 #0
/// BLANK` makes), is too large, or its fields do not add up. The
/// layout (wiki: protocols/hp-object-format, "GROB layout"): prolog
/// #02B1E, length, height, width (5 nibbles each, low nibble first), then
/// the rows, each padded to a whole number of bytes; within a nibble the
/// least significant bit is the leftmost pixel.
pub fn graphic(n: &[u8]) -> Option<Graphic> {
    if field(n, 0, 5).ok()? != ObjectType::Graphic.prolog() {
        return None;
    }
    let length = usize::try_from(field(n, 5, 5).ok()?).ok()?;
    let height = field(n, 10, 5).ok()?;
    let width = field(n, 15, 5).ok()?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_GRAPHIC_PIXELS {
        return None;
    }
    let row_nibbles = usize::try_from(width.div_ceil(8) * 2).ok()?;
    let body = row_nibbles * usize::try_from(height).ok()?;
    // The length counts itself, the height, the width and the body.
    if length != 15 + body || n.len() < 20 + body {
        return None;
    }
    let row_bytes = row_nibbles / 2;
    let w = usize::try_from(width).ok()?;
    let mut rows = String::with_capacity(body);
    for y in 0..usize::try_from(height).ok()? {
        let row = &n[20 + y * row_nibbles..20 + (y + 1) * row_nibbles];
        for b in 0..row_bytes {
            let mut byte = 0u8;
            for bit in 0..8 {
                let x = b * 8 + bit;
                let on = (row[x / 4] >> (x % 4)) & 1 == 1;
                if on && x < w {
                    byte |= 0x80 >> bit;
                }
            }
            rows.push_str(&format!("{byte:02X}"));
        }
    }
    Some(Graphic {
        width,
        height,
        rows,
    })
}

/// Read access to the calculator's memory, for ROM pointers.
pub trait Memory {
    /// The nibble at `addr`, if readable.
    fn nibble(&self, addr: u32) -> Option<u8>;
}

/// No memory: ROM pointers stay [`Object::Command`].
#[derive(Clone, Copy, Debug, Default)]
pub struct NoMemory;

impl Memory for NoMemory {
    fn nibble(&self, _addr: u32) -> Option<u8> {
        None
    }
}

fn hex(nibbles: &[u8]) -> String {
    nibbles
        .iter()
        .map(|&n| char::from_digit(u32::from(n & 0xF), 16).unwrap_or('?'))
        .collect::<String>()
        .to_uppercase()
}

fn field(n: &[u8], at: usize, width: usize) -> Result<u32> {
    read_field(n, at, width).with_context(|| format!("object truncated at nibble {at}"))
}

fn usize_field(n: &[u8], at: usize, width: usize) -> Result<usize> {
    Ok(usize::try_from(field(n, at, width)?)?)
}

/// HP-character text of `count` bytes stored as nibble pairs at `at`.
fn chars(n: &[u8], at: usize, count: usize) -> Result<String> {
    let bytes = (0..count)
        .map(|i| field(n, at + 2 * i, 2).map(|b| b as u8))
        .collect::<Result<Vec<u8>>>()?;
    Ok(charset::decode(&bytes))
}

/// Decode the object at the start of `nibbles`. An object the walk does
/// not understand becomes [`Object::Unknown`] with all of `nibbles`.
pub fn decode(nibbles: &[u8], mem: &dyn Memory) -> Result<Object> {
    Reader::new(mem).decode(nibbles)
}

/// Decode the object at `addr` in `mem` (a stack level, a variable): an
/// object with a known prolog, else a primitive or ROM word
/// ([`Object::Command`] with its address).
pub fn decode_at(addr: u32, mem: &dyn Memory) -> Result<Object> {
    Reader::new(mem).decode_at(addr)
}

/// Decodes several objects in memory (the levels of a stack) under one
/// budget of [`MAX_DECODED_OBJECTS`] and [`MAX_DECODED_NIBBLES`]: the
/// caller gets one bounded result, however many levels point at the same
/// large object.
pub struct Reader<'a> {
    decoder: Decoder<'a>,
}

impl fmt::Debug for Reader<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reader")
            .field("budget", &self.decoder.budget.get())
            .field("nibbles", &self.decoder.nibbles.get())
            .field("names", &self.decoder.names.is_some())
            .finish_non_exhaustive()
    }
}

impl<'a> Reader<'a> {
    /// A reader over `mem` with a fresh budget, without command names
    /// (commands stay unnamed, programs and algebraics without text).
    pub fn new(mem: &'a dyn Memory) -> Self {
        Self {
            decoder: Decoder::new(mem, None, Settings::default()),
        }
    }

    /// A reader that names commands from `names` and writes the text of
    /// programs, algebraics and units with `settings`.
    pub fn with_names(mem: &'a dyn Memory, names: &'a NameTable, settings: Settings) -> Self {
        Self {
            decoder: Decoder::new(mem, Some(names), settings),
        }
    }

    /// The object at `addr`, as [`decode_at`], charged to this reader's
    /// budget. With names, an address that is a named command's object is
    /// that command.
    pub fn decode_at(&self, addr: u32) -> Result<Object> {
        let d = &self.decoder;
        if let Some(info) = d.names.and_then(|t| t.command_at(addr, d.mem)) {
            d.charge(1)?;
            return Ok(Object::Command {
                name: info.name.map(str::to_string),
                address: Some(addr),
                library: Some(info.library),
                command: Some(info.number),
                source: None,
            });
        }
        d.rom_object(addr, 0)
    }

    /// The object at the start of `nibbles`, as [`decode`].
    pub fn decode(&self, nibbles: &[u8]) -> Result<Object> {
        match self.decoder.object(nibbles, 0, 0) {
            Ok((obj, _)) => Ok(obj),
            // Past the budget the object is not unknown, it is too big.
            Err(e) if self.decoder.exhausted.get() => Err(e),
            Err(_) => {
                let prolog = field(nibbles, 0, 5)?;
                Ok(unknown(prolog, nibbles))
            }
        }
    }
}

fn unknown(prolog: u32, nibbles: &[u8]) -> Object {
    let shown = nibbles.len().min(MAX_HEX_NIBBLES);
    Object::Unknown {
        prolog: format!("{prolog:05X}"),
        kind: ObjectType::from_prolog(prolog).map(|t| t.name().to_string()),
        nibbles: nibbles.len(),
        hex: hex(&nibbles[..shown]),
        truncated: shown < nibbles.len(),
        source: None,
        graphic: graphic(nibbles),
    }
}

struct Decoder<'a> {
    mem: &'a dyn Memory,
    /// Command names; without them commands stay unnamed and programs,
    /// algebraics and units get no text.
    names: Option<&'a NameTable>,
    /// Display settings of the text.
    settings: Settings,
    /// Objects this decode may still produce.
    budget: Cell<usize>,
    /// Nibbles this decode may still read (see [`MAX_DECODED_NIBBLES`]).
    nibbles: Cell<usize>,
    /// Nibbles this decode may still clone (see [`MAX_CLONED_NIBBLES`]).
    cloned: Cell<usize>,
    /// Objects already read from memory by (address, depth), with the
    /// objects and nibbles they cost: a repeated pointer is cloned, not
    /// read again, and charged again before the clone, so the clones are
    /// bounded like the reads.
    seen: RefCell<HashMap<(u32, usize), (Object, Cost)>>,
    /// Whether a budget ran out.
    exhausted: Cell<bool>,
}

impl<'a> Decoder<'a> {
    fn new(mem: &'a dyn Memory, names: Option<&'a NameTable>, settings: Settings) -> Self {
        Self {
            mem,
            names,
            settings,
            budget: Cell::new(MAX_DECODED_OBJECTS),
            nibbles: Cell::new(MAX_DECODED_NIBBLES),
            cloned: Cell::new(MAX_CLONED_NIBBLES),
            seen: RefCell::new(HashMap::new()),
            exhausted: Cell::new(false),
        }
    }

    /// Take `count` objects from the budget.
    fn charge(&self, count: usize) -> Result<()> {
        let Some(left) = self.budget.get().checked_sub(count) else {
            self.exhausted.set(true);
            bail!(
                "more than {MAX_DECODED_OBJECTS} objects (MAX_DECODED_OBJECTS) to decode: \
                 pointers that repeat, memory corrupt or not set up"
            );
        };
        self.budget.set(left);
        Ok(())
    }

    /// Take `count` nibbles from the budget.
    fn charge_nibbles(&self, count: usize) -> Result<()> {
        let Some(left) = self.nibbles.get().checked_sub(count) else {
            self.exhausted.set(true);
            bail!(
                "more than {MAX_DECODED_NIBBLES} nibbles (MAX_DECODED_NIBBLES) to decode: \
                 objects that repeat, memory corrupt or not set up"
            );
        };
        self.nibbles.set(left);
        Ok(())
    }

    /// Take `count` nibbles from the clone budget.
    fn charge_cloned(&self, count: usize) -> Result<()> {
        let Some(left) = self.cloned.get().checked_sub(count) else {
            self.exhausted.set(true);
            bail!(
                "more than {MAX_CLONED_NIBBLES} nibbles (MAX_CLONED_NIBBLES) of repeated \
                 objects to decode: pointers that repeat, memory corrupt or not set up"
            );
        };
        self.cloned.set(left);
        Ok(())
    }

    /// What is left of the budgets; `nibbles` counts reads and clones
    /// together, so an object's cost includes the clones inside it.
    fn left(&self) -> Cost {
        Cost {
            objects: self.budget.get(),
            nibbles: self.nibbles.get() + self.cloned.get(),
        }
    }

    /// The object with a prolog at `at` and its size in nibbles.
    fn object(&self, n: &[u8], at: usize, depth: usize) -> Result<(Object, usize)> {
        if depth > MAX_DEPTH {
            bail!("objects nested deeper than {MAX_DEPTH} levels");
        }
        self.charge(1)?;
        let prolog = field(n, at, 5)?;
        let ty = ObjectType::from_prolog(prolog)
            .with_context(|| format!("unknown prolog {prolog:05X} at nibble {at}"))?;
        let size = object_size(n, at)?;
        let body = at + 5;
        let obj = match ty {
            ObjectType::Real => Object::Real {
                value: Real::from_body(&n[body..])?,
            },
            ObjectType::Complex => Object::Complex {
                re: Real::from_body(&n[body..])?,
                im: Real::from_body(&n[body + 16..])?,
            },
            ObjectType::String => {
                let len = usize_field(n, body, 5)?;
                let count = len.checked_sub(5).context("string length too small")?;
                if count % 2 != 0 {
                    bail!("string of an odd number of nibbles");
                }
                Object::String {
                    value: chars(n, body + 5, count / 2)?,
                }
            }
            ObjectType::BinaryInteger => {
                let len = usize_field(n, body, 5)?;
                let count = len.checked_sub(5).context("binary length too small")?;
                if count > 16 {
                    return Ok((unknown(prolog, &n[at..at + size]), size));
                }
                let value = n[body + 5..body + 5 + count]
                    .iter()
                    .rev()
                    .fold(0u64, |acc, &d| (acc << 4) | u64::from(d));
                Object::Binary {
                    value,
                    base: None,
                    text: None,
                }
            }
            ObjectType::Integer => {
                let len = usize_field(n, body, 5)?;
                let count = len.checked_sub(5).context("integer length too small")?;
                Object::Integer {
                    value: Integer::from_nibbles(&n[body + 5..body + 5 + count])?,
                }
            }
            ObjectType::GlobalName | ObjectType::LocalName => {
                let count = usize_field(n, body, 2)?;
                let value = chars(n, body + 2, count)?;
                if ty == ObjectType::GlobalName {
                    Object::Name { value }
                } else {
                    Object::LocalName { value }
                }
            }
            ObjectType::Character => Object::Character {
                value: chars(n, body, 1)?,
            },
            ObjectType::List => Object::List {
                items: self
                    .elements(n, body, depth)?
                    .into_iter()
                    .map(element_object)
                    .collect(),
            },
            ObjectType::Tagged => {
                let count = usize_field(n, body, 2)?;
                let tag = chars(n, body + 2, count)?;
                let (object, _) = self.element(n, body + 2 + 2 * count, depth)?;
                Object::Tagged {
                    tag,
                    object: Box::new(element_object(object)),
                }
            }
            ObjectType::Unit => {
                let mut elements = self.elements(n, body, depth)?.into_iter();
                let Some(Element::Object(Object::Real { value })) = elements.next() else {
                    bail!("unit does not start with a real");
                };
                let rest: Vec<Element> = elements.collect();
                let unit = self
                    .names
                    .and_then(|_| decompile::unit(&rest, &self.settings));
                Object::Unit { value, unit }
            }
            ObjectType::Array => self.array(n, at, size)?,
            ObjectType::SymbolicMatrix => self.symbolic_matrix(n, body, depth)?,
            ObjectType::Program => {
                let elements = self.elements(n, body, depth)?;
                Object::Program {
                    source: self
                        .names
                        .map(|_| decompile::program(&elements, &self.settings)),
                }
            }
            ObjectType::Algebraic => self.symbolic(n, body, depth)?.0,
            ObjectType::XlibName => element_object(self.xlib(n, body)?),
            _ => unknown(prolog, &n[at..at + size]),
        };
        Ok((obj, size))
    }

    /// The algebraic whose body starts at `body`, and its elements.
    fn symbolic(&self, n: &[u8], body: usize, depth: usize) -> Result<(Object, Vec<Element>)> {
        let elements = self.elements(n, body, depth)?;
        let object = Object::Algebraic {
            source: self
                .names
                .and_then(|_| decompile::algebraic(&elements, &self.settings)),
        };
        Ok((object, elements))
    }

    /// Composite elements from `at` up to SEMI.
    fn elements(&self, n: &[u8], mut at: usize, depth: usize) -> Result<Vec<Element>> {
        let mut items = Vec::new();
        while field(n, at, 5)? != SEMI {
            let (obj, size) = self.element(n, at, depth)?;
            items.push(obj);
            at += size;
        }
        Ok(items)
    }

    /// An embedded object or a 5-nibble ROM pointer at `at`.
    fn element(&self, n: &[u8], at: usize, depth: usize) -> Result<(Element, usize)> {
        let p = field(n, at, 5)?;
        match ObjectType::from_prolog(p) {
            Some(ObjectType::XlibName) => {
                self.charge(1)?;
                Ok((self.xlib(n, at + 5)?, 11))
            }
            Some(ObjectType::SystemBinary) => {
                self.charge(1)?;
                Ok((Element::SystemBinary(field(n, at + 5, 5)?), 10))
            }
            Some(ObjectType::Algebraic) if depth < MAX_DEPTH => {
                self.charge(1)?;
                let size = object_size(n, at)?;
                let (object, elements) = self.symbolic(n, at + 5, depth + 1)?;
                Ok((Element::Symbolic { object, elements }, size))
            }
            Some(_) => {
                let (obj, size) = self.object(n, at, depth + 1)?;
                Ok((Element::Object(obj), size))
            }
            None => Ok((self.pointer(p, depth + 1)?, 5)),
        }
    }

    /// The command an XLIB body (library, command) at `at` names.
    fn xlib(&self, n: &[u8], at: usize) -> Result<Element> {
        let lib = field(n, at, 3)?;
        let cmd = field(n, at + 3, 3)?;
        let (lib, cmd) = (lib as u16, cmd as u16);
        let info = self.names.and_then(|t| t.xlib(lib, cmd));
        Ok(Element::Command {
            name: info.and_then(|i| i.name).map(str::to_string),
            address: None,
            xlib: Some((lib, cmd)),
            arity: info.and_then(|i| i.arity),
            silent: false,
        })
    }

    /// The element a ROM pointer stands for: a command (named when the
    /// table knows it), a unit operator, a system binary, or the data
    /// object it points to.
    fn pointer(&self, p: u32, depth: usize) -> Result<Element> {
        if let Some(t) = self.names {
            if let Some(info) = t.command_at(p, self.mem) {
                self.charge(1)?;
                return Ok(Element::Command {
                    name: info.name.map(str::to_string),
                    address: Some(p),
                    xlib: Some((info.library, info.number)),
                    arity: info.arity,
                    silent: info.silent,
                });
            }
            if let Some(m) = t.unit_markers() {
                let op = [
                    (m.times, UnitOp::Times),
                    (m.divide, UnitOp::Divide),
                    (m.power, UnitOp::Power),
                    (m.prefix, UnitOp::Prefix),
                    (m.end, UnitOp::End),
                ]
                .into_iter()
                .find_map(|(a, op)| (a == p).then_some(op));
                if let Some(op) = op {
                    self.charge(1)?;
                    return Ok(Element::Unit(op));
                }
            }
        }
        let read = |at: u32, width: u32| -> Option<u32> {
            (0..width).rev().try_fold(0u32, |v, i| {
                Some((v << 4) | u32::from(self.mem.nibble(at.wrapping_add(i))?))
            })
        };
        let command = Element::Command {
            name: None,
            address: Some(p),
            xlib: None,
            arity: None,
            silent: false,
        };
        match read(p, 5).and_then(ObjectType::from_prolog) {
            // Unreadable, a primitive (its prolog is its own code), code
            // or a program in ROM: a command, never decoded.
            None | Some(ObjectType::Program | ObjectType::Code) => {
                self.charge(1)?;
                Ok(command)
            }
            Some(ObjectType::SystemBinary) => {
                self.charge(1)?;
                Ok(read(p + 5, 5).map_or(command, Element::SystemBinary))
            }
            Some(ObjectType::XlibName) => {
                self.charge(1)?;
                let body: Option<Vec<u8>> = (0..6).map(|i| self.mem.nibble(p + 5 + i)).collect();
                match body {
                    Some(b) => self.xlib(&b, 0),
                    None => Ok(command),
                }
            }
            Some(_) => Ok(Element::Object(self.rom_object(p, depth)?)),
        }
    }

    /// The object at `addr` in memory (a ROM pointer's target, a stack
    /// level), or a command.
    fn rom_object(&self, addr: u32, depth: usize) -> Result<Object> {
        if let Some((obj, cost)) = self.seen.borrow().get(&(addr, depth)) {
            self.charge(cost.objects)?;
            self.charge_cloned(cost.nibbles)?;
            return Ok(obj.clone());
        }
        let before = self.left();
        let obj = self.read_object(addr, depth)?;
        let after = self.left();
        let cost = Cost {
            objects: before.objects - after.objects,
            nibbles: before.nibbles - after.nibbles,
        };
        self.seen
            .borrow_mut()
            .insert((addr, depth), (obj.clone(), cost));
        Ok(obj)
    }

    /// [`Decoder::rom_object`] without the cache.
    fn read_object(&self, addr: u32, depth: usize) -> Result<Object> {
        // As many nibbles as are readable, up to `len`.
        let read = |len: usize| -> Vec<u8> {
            (0..len)
                .map_while(|i| {
                    u32::try_from(i)
                        .ok()
                        .and_then(|i| self.mem.nibble(addr.wrapping_add(i)))
                })
                .collect()
        };
        let head = read(5);
        if read_field(&head, 0, 5)
            .and_then(ObjectType::from_prolog)
            .is_none()
        {
            // Unreadable, a primitive (its prolog is its own code) or a
            // word in ROM.
            self.charge(1)?;
            return Ok(Object::Command {
                name: None,
                address: Some(addr),
                library: None,
                command: None,
                source: None,
            });
        }
        let mut len = 64;
        loop {
            let nib = read(len);
            match object_size(&nib, 0) {
                Ok(size) => {
                    self.charge_nibbles(size)?;
                    return Ok(self.object(&nib, 0, depth)?.0);
                }
                Err(_) if nib.len() == len && len < MAX_MEMORY_OBJECT => len *= 8,
                Err(e) => bail!("object at #{addr:05X}: {e}"),
            }
        }
    }

    /// A 49G symbolic matrix: its elements, or its rows (each a symbolic
    /// vector), as an array.
    fn symbolic_matrix(&self, n: &[u8], body: usize, depth: usize) -> Result<Object> {
        let items: Vec<Object> = self
            .elements(n, body, depth)?
            .into_iter()
            .map(element_object)
            .collect();
        let rows = !items.is_empty()
            && items
                .iter()
                .all(|o| matches!(o, Object::Array { dims, .. } if dims.len() == 1));
        if !rows {
            return Ok(Object::Array {
                dims: vec![items.len()],
                items: items
                    .into_iter()
                    .map(|o| ArrayItem::Item(Box::new(o)))
                    .collect(),
            });
        }
        let cols = match items.first() {
            Some(Object::Array { dims, .. }) => dims[0],
            _ => 0,
        };
        Ok(Object::Array {
            dims: vec![items.len(), cols],
            items: items
                .into_iter()
                .map(|o| match o {
                    Object::Array { items, .. } => ArrayItem::Row(items),
                    o => ArrayItem::Item(Box::new(o)),
                })
                .collect(),
        })
    }

    /// A real or complex array.
    fn array(&self, n: &[u8], at: usize, size: usize) -> Result<Object> {
        let body = at + 5;
        let elem = field(n, body + 5, 5)?;
        let ndims = usize_field(n, body + 10, 5)?;
        if ndims > MAX_ARRAY_DIMS {
            bail!("array of {ndims} dimensions (at most {MAX_ARRAY_DIMS})");
        }
        let bad = || anyhow::anyhow!("array dimensions do not match its length");
        // Every count below comes from the object: check it against the
        // object's size before using it (no allocation from a bad count).
        let start = ndims
            .checked_mul(5)
            .and_then(|d| d.checked_add(body + 15))
            .filter(|&s| s <= at + size)
            .ok_or_else(bad)?;
        let dims = (0..ndims)
            .map(|i| usize_field(n, body + 15 + 5 * i, 5))
            .collect::<Result<Vec<usize>>>()?;
        let width = match ObjectType::from_prolog(elem) {
            Some(ObjectType::Real) => 16,
            Some(ObjectType::Complex) => 32,
            _ => return Ok(unknown(field(n, at, 5)?, &n[at..at + size])),
        };
        // Rows at each level and the element count must fit the object
        // (an empty dimension keeps the elements at 0 but not the rows).
        let mut count: usize = 1;
        for d in &dims {
            count = count
                .checked_mul(*d)
                .filter(|&c| c <= size)
                .ok_or_else(bad)?;
        }
        let end = count
            .checked_mul(width)
            .and_then(|b| b.checked_add(start))
            .ok_or_else(bad)?;
        if ndims == 0 || end > at + size {
            return Err(bad());
        }
        self.charge(count)?;
        let mut flat = Vec::with_capacity(count);
        for i in 0..count {
            let p = start + i * width;
            flat.push(if width == 16 {
                Object::Real {
                    value: Real::from_body(&n[p..])?,
                }
            } else {
                Object::Complex {
                    re: Real::from_body(&n[p..])?,
                    im: Real::from_body(&n[p + 16..])?,
                }
            });
        }
        Ok(Object::Array {
            items: nest(&dims, &mut flat.into_iter()),
            dims,
        })
    }
}

/// Objects and nibbles, taken from or left in a decode's budgets.
#[derive(Clone, Copy, Debug)]
struct Cost {
    objects: usize,
    nibbles: usize,
}

/// An element of a list or tagged object as an object.
fn element_object(e: Element) -> Object {
    match e {
        Element::Object(o) | Element::Symbolic { object: o, .. } => o,
        Element::Command {
            name,
            address,
            xlib,
            ..
        } => Object::Command {
            name,
            address,
            library: xlib.map(|x| x.0),
            command: xlib.map(|x| x.1),
            source: None,
        },
        Element::SystemBinary(v) => Object::Unknown {
            prolog: format!("{:05X}", ObjectType::SystemBinary.prolog()),
            kind: Some(ObjectType::SystemBinary.name().to_string()),
            nibbles: 10,
            hex: format!("{:05X}", ObjectType::SystemBinary.prolog())
                .chars()
                .rev()
                .chain(format!("{v:05X}").chars().rev())
                .collect(),
            truncated: false,
            source: None,
            graphic: None,
        },
        // A unit operator is an empty list in ROM.
        Element::Unit(_) => Object::List { items: Vec::new() },
    }
}

/// Elements in lexicographic index order, nested by dimension (at most
/// [`MAX_ARRAY_DIMS`] levels deep).
fn nest(dims: &[usize], flat: &mut impl Iterator<Item = Object>) -> Vec<ArrayItem> {
    match dims {
        [] => Vec::new(),
        [n] => flat
            .take(*n)
            .map(|o| ArrayItem::Item(Box::new(o)))
            .collect(),
        [n, rest @ ..] => (0..*n).map(|_| ArrayItem::Row(nest(rest, flat))).collect(),
    }
}

impl Object {
    /// Whether this object or one inside it needs text from the ASCII
    /// transfer (the transfer layer fills them in).
    pub fn needs_source(&self) -> bool {
        match self {
            Object::Program { source } | Object::Algebraic { source } => source.is_none(),
            Object::Command { name, source, .. } => name.is_none() && source.is_none(),
            Object::Unit { unit, .. } => unit.is_none(),
            Object::Unknown { source, .. } => source.is_none(),
            Object::List { items } => items.iter().any(Object::needs_source),
            Object::Tagged { object, .. } => object.needs_source(),
            _ => false,
        }
    }

    /// Set the base (and display text) of every binary integer inside.
    pub fn set_base(&mut self, b: Base) {
        match self {
            Object::Binary { value, base, text } => {
                *base = Some(b);
                *text = Some(b.format(*value));
            }
            Object::List { items } => items.iter_mut().for_each(|o| o.set_base(b)),
            Object::Tagged { object, .. } => object.set_base(b),
            _ => {}
        }
    }

    /// Whether a binary integer is inside.
    pub fn has_binary(&self) -> bool {
        match self {
            Object::Binary { .. } => true,
            Object::List { items } => items.iter().any(Object::has_binary),
            Object::Tagged { object, .. } => object.has_binary(),
            _ => false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Nibbles from a hex string in memory order.
    fn nib(s: &str) -> Vec<u8> {
        s.chars().map(|c| c.to_digit(16).unwrap() as u8).collect()
    }

    fn dec(s: &str) -> Object {
        decode(&nib(s), &NoMemory).unwrap()
    }

    fn real(s: &str) -> Real {
        Real::parse(s).unwrap()
    }

    /// An unnamed command at `address`.
    fn cmd(address: u32) -> Object {
        Object::Command {
            name: None,
            address: Some(address),
            library: None,
            command: None,
            source: None,
        }
    }

    fn put_field(out: &mut Vec<u8>, value: u64, width: usize) {
        out.extend((0..width).map(|i| ((value >> (4 * i)) & 0xF) as u8));
    }

    // Byte-level fixtures: GETs from the 48SX ROM J and the 49G (2009 ROM)
    // in saturnus, 2026-10-05; wiki: protocols/hp-object-format.

    #[test]
    fn reals_decode_exactly() {
        // 8.72653549837E-3 (SIN 0.5 in degrees).
        let o = dec("339207997389453562780");
        assert_eq!(
            o,
            Object::Real {
                value: real("8.72653549837E-3")
            }
        );
        assert_eq!(
            serde_json::to_value(&o).unwrap(),
            json!({"type": "real", "value": 0.00872653549837})
        );
        // -1.5E-300 and 0.
        let o = dec("339200070000000000519");
        let Object::Real { value } = o else { panic!() };
        assert_eq!(value.to_source(), "-1.5E-300");
        assert!(value.negative);
        assert_eq!(
            dec("339200000000000000000"),
            Object::Real { value: Real::ZERO }
        );
    }

    #[test]
    fn real_text_round_trips_and_rounds() {
        for s in [
            "4.79425538604E-1",
            "5.",
            "-1.5E-300",
            "9.99999999999E499",
            "1.E-499",
            "0.",
        ] {
            let r = real(s);
            assert_eq!(r.to_source(), s);
            assert_eq!(Real::from_body(&r.to_body()).unwrap(), r);
        }
        assert_eq!(real("0.479425538604").to_source(), "4.79425538604E-1");
        assert_eq!(real(".5").to_source(), "5.E-1");
        assert_eq!(real("123").to_source(), "1.23E2");
        // 13 digits round half away from zero, with carry.
        assert_eq!(real("1.234567890125").to_source(), "1.23456789013");
        assert_eq!(real("9.999999999995").to_source(), "1.E1");
        assert_eq!(real("-0.000").to_source(), "0.");
        assert!(Real::parse("1E500").is_err());
        assert!(Real::parse("1.2.3").is_err());
        assert!(Real::parse("").is_err());
        assert!(Real::parse("e5").is_err());
        assert_eq!(
            Real::from_f64(0.479425538604).unwrap(),
            real("4.79425538604E-1")
        );
    }

    #[test]
    fn reals_beyond_f64_serialize_as_text() {
        let r = Object::Real {
            value: real("1.5E-400"),
        };
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            json!({"type": "real", "value": "1.5E-400"})
        );
        let back: Object =
            serde_json::from_value(json!({"type": "real", "value": "1.5E-400"})).unwrap();
        assert_eq!(back, r);
        let five: Object = serde_json::from_value(json!({"type": "real", "value": 5})).unwrap();
        assert_eq!(five, Object::Real { value: real("5") });
    }

    #[test]
    fn complex_string_binary() {
        assert_eq!(
            dec("7792000000000000000100000000000000029"),
            Object::Complex {
                re: real("1"),
                im: real("-2")
            }
        );
        // "Hi «x»": « and » are HP bytes 0xAB and 0xBB.
        assert_eq!(
            dec("C2A2011000849602BA87BB"),
            Object::String {
                value: "Hi «x»".into()
            }
        );
        let mut b = dec("E4A2051000FF00000000000000");
        assert_eq!(
            b,
            Object::Binary {
                value: 255,
                base: None,
                text: None
            }
        );
        b.set_base(Base::Hex);
        assert_eq!(
            serde_json::to_value(&b).unwrap(),
            json!({"type": "binary", "value": 255, "base": "hex", "text": "# FFh"})
        );
        assert_eq!(Base::from_display("# 255d"), Some(Base::Dec));
        assert_eq!(Base::from_display("42."), None);
    }

    #[test]
    fn integers_49g() {
        assert_eq!(
            dec("416207000050"),
            Object::Integer {
                value: Integer::parse("5").unwrap()
            }
        );
        let big = dec("416209100098765432109876543219");
        assert_eq!(
            big,
            Object::Integer {
                value: Integer::parse("-1234567890123456789").unwrap()
            }
        );
        assert_eq!(
            serde_json::to_value(&big).unwrap(),
            json!({"type": "integer", "value": "-1234567890123456789"})
        );
        assert_eq!(
            dec("416206000000"),
            Object::Integer {
                value: Integer::parse("0").unwrap()
            }
        );
        assert_eq!(
            serde_json::to_value(dec("416207000050")).unwrap(),
            json!({"type": "integer", "value": 5})
        );
    }

    #[test]
    fn names_tagged_and_lists_with_rom_pointers() {
        // 48SX { 1 2. "s" X { 5 } # 2Ah }: 1, 2 and 5 are ROM pointers.
        let list = dec(
            "47A209C2A2ED2A2C2A20700003784E20108547A20D13A2B2130E4A2051000A200000000000000B2130",
        );
        let Object::List { items } = &list else {
            panic!("{list:?}")
        };
        assert_eq!(items.len(), 6);
        assert_eq!(items[0], cmd(0x2A2C9));
        assert_eq!(items[2], Object::String { value: "s".into() });
        assert_eq!(items[3], Object::Name { value: "X".into() });
        assert_eq!(
            items[4],
            Object::List {
                items: vec![cmd(0x2A31D)]
            }
        );
        assert_eq!(
            items[5],
            Object::Binary {
                value: 42,
                base: None,
                text: None
            }
        );
        // { :T:5 } with the 5 as a pointer.
        let t = dec("47A20CFA201045D13A2B2130");
        assert_eq!(
            t,
            Object::List {
                items: vec![Object::Tagged {
                    tag: "T".into(),
                    object: Box::new(cmd(0x2A31D))
                }]
            }
        );
    }

    /// A memory holding the real 1 at #2A2C9 and 5 at #2A31D (48SX ROM J).
    struct Rom;
    impl Memory for Rom {
        fn nibble(&self, addr: u32) -> Option<u8> {
            let (base, obj) = match addr {
                0x2A2C9..=0x2A2DD => (0x2A2C9, "339200000000000000010"),
                0x2A31D..=0x2A331 => (0x2A31D, "339200000000000000050"),
                // A primitive: the "prolog" points past itself.
                0x10000..=0x10004 => (0x10000, "50001"),
                _ => return None,
            };
            nib(obj).get((addr - base) as usize).copied()
        }
    }

    #[test]
    fn rom_pointers_resolve_through_memory() {
        let list = decode(&nib("47A209C2A2D13A200001B2130"), &Rom).unwrap();
        assert_eq!(
            list,
            Object::List {
                items: vec![
                    Object::Real { value: real("1") },
                    Object::Real { value: real("5") },
                    cmd(0x10000),
                ]
            }
        );
    }

    #[test]
    fn arrays() {
        let m = dec(
            "8E920950003392020000200002000000000000000000100000000000000020000000000000003000000000000000400",
        );
        let item = |s: &str| ArrayItem::Item(Box::new(Object::Real { value: real(s) }));
        assert_eq!(
            m,
            Object::Array {
                dims: vec![2, 2],
                items: vec![
                    ArrayItem::Row(vec![item("1"), item("2")]),
                    ArrayItem::Row(vec![item("3"), item("4")])
                ],
            }
        );
        assert_eq!(
            serde_json::to_value(&m).unwrap(),
            json!({"type": "array", "dims": [2, 2], "items": [
                [{"type": "real", "value": 1.0}, {"type": "real", "value": 2.0}],
                [{"type": "real", "value": 3.0}, {"type": "real", "value": 4.0}]]})
        );
        let v = dec(
            "8E9204500077920100002000000000000000000100000000000000020000000000000003000000000000000400",
        );
        let Object::Array { dims, items } = v else {
            panic!()
        };
        assert_eq!(dims, vec![2]);
        assert_eq!(
            items[1],
            ArrayItem::Item(Box::new(Object::Complex {
                re: real("3"),
                im: real("4")
            }))
        );
    }

    #[test]
    fn array_dimensions_cannot_overflow() {
        // Length #00023, reals, 4 dimensions 0x80000 x 0x80000 x 0x80000
        // x 8: the product is 2^60 and times 16 wraps to 0.
        let mut o = nib("8E920");
        put_field(&mut o, 0x23, 5);
        put_field(&mut o, 0x02933, 5);
        put_field(&mut o, 4, 5);
        for d in [0x80000u64, 0x80000, 0x80000, 8] {
            put_field(&mut o, d, 5);
        }
        assert_eq!(o.len(), 40);
        let Ok(Object::Unknown { prolog, .. }) = decode(&o, &NoMemory) else {
            panic!("decoded");
        };
        assert_eq!(prolog, "029E8");
        // Empty arrays stay decodable; huge row counts with a zero do not.
        let mut e = nib("8E920");
        put_field(&mut e, 25, 5);
        put_field(&mut e, 0x02933, 5);
        put_field(&mut e, 2, 5);
        put_field(&mut e, 3, 5);
        put_field(&mut e, 0, 5);
        assert_eq!(
            decode(&e, &NoMemory).unwrap(),
            Object::Array {
                dims: vec![3, 0],
                items: vec![
                    ArrayItem::Row(vec![]),
                    ArrayItem::Row(vec![]),
                    ArrayItem::Row(vec![])
                ],
            }
        );
        let mut z = nib("8E920");
        put_field(&mut z, 30, 5);
        put_field(&mut z, 0x02933, 5);
        put_field(&mut z, 3, 5);
        for d in [0x80000u64, 0x80000, 0] {
            put_field(&mut z, d, 5);
        }
        assert!(matches!(
            decode(&z, &NoMemory).unwrap(),
            Object::Unknown { .. }
        ));
    }

    /// An array of `ndims` dimensions of 1 holding one real.
    fn dims_array(ndims: usize) -> Vec<u8> {
        let mut o = nib("8E920");
        put_field(&mut o, (15 + 5 * ndims + 16) as u64, 5);
        put_field(&mut o, 0x02933, 5);
        put_field(&mut o, ndims as u64, 5);
        for _ in 0..ndims {
            put_field(&mut o, 1, 5);
        }
        o.extend(nib("0000000000000010"));
        o
    }

    #[test]
    fn arrays_have_at_most_two_dimensions() {
        let item = ArrayItem::Item(Box::new(Object::Real { value: real("1") }));
        assert_eq!(
            dec(&hex(&dims_array(2))),
            Object::Array {
                dims: vec![1, 1],
                items: vec![ArrayItem::Row(vec![item])],
            }
        );
        // 12000 dimensions nested the rows 12000 deep (a stack overflow
        // that aborted the process); now any third one is refused.
        for ndims in [3, 12_000] {
            let Ok(Object::Unknown { prolog, .. }) = decode(&dims_array(ndims), &NoMemory) else {
                panic!("{ndims} dimensions decoded");
            };
            assert_eq!(prolog, "029E8");
            let mut list = nib("47A20");
            list.extend(dims_array(ndims));
            list.extend(nib("B2130"));
            let e = Reader::new(&NoMemory)
                .decoder
                .object(&list, 0, 0)
                .unwrap_err();
            assert!(e.to_string().contains("dimensions"), "{e:#}");
        }
    }

    /// A GROB's nibbles as the calculator stores them (wiki:
    /// protocols/hp-object-format): the prolog, the length, the height,
    /// the width, low nibble first, then each row padded to whole bytes,
    /// the leftmost pixel in a nibble's least significant bit.
    fn grob(rows: &[&str]) -> Vec<u8> {
        let height = rows.len();
        let width = rows.first().map_or(0, |r| r.len());
        let row_nibbles = width.div_ceil(8) * 2;
        let mut n = Vec::new();
        let put = |v: usize, n: &mut Vec<u8>| {
            for i in 0..5 {
                n.push(((v >> (4 * i)) & 0xF) as u8);
            }
        };
        put(0x02B1E, &mut n);
        put(15 + row_nibbles * height, &mut n);
        put(height, &mut n);
        put(width, &mut n);
        for r in rows {
            let mut row = vec![0u8; row_nibbles];
            for (x, c) in r.chars().enumerate() {
                if c == '#' {
                    row[x / 4] |= 1 << (x % 4);
                }
            }
            n.extend(row);
        }
        n
    }

    #[test]
    fn a_grob_is_pictured_row_by_row_leftmost_pixel_first() {
        let g = graphic(&grob(&["#"])).unwrap();
        assert_eq!((g.width, g.height, g.rows.as_str()), (1, 1, "80"));
        // An odd width: 5 pixels in one byte per row.
        let g = graphic(&grob(&["#.#.#", ".#.#.", "....#"])).unwrap();
        assert_eq!((g.width, g.height, g.rows.as_str()), (5, 3, "A85008"));
        // Nine pixels: two bytes a row, the ninth in the second's top bit.
        let g = graphic(&grob(&["........#", "#........"])).unwrap();
        assert_eq!(g.rows, "00808000");
        // The screen's size: 34 nibbles a row, length #0088F (the FAQ's).
        let mut row = ".".repeat(131);
        row.replace_range(130..131, "#");
        let screen: Vec<&str> = std::iter::repeat_n(row.as_str(), 64).collect();
        let n = grob(&screen);
        assert_eq!(n.len(), 2196);
        assert_eq!(field(&n, 5, 5).unwrap(), 0x88F);
        let g = graphic(&n).unwrap();
        assert_eq!((g.width, g.height), (131, 64));
        assert_eq!(&g.rows[..34], "0000000000000000000000000000000020");
        // As an object: an unknown one with its picture.
        let Object::Unknown {
            kind,
            graphic: Some(p),
            ..
        } = decode(&grob(&["##"]), &NoMemory).unwrap()
        else {
            panic!("not a pictured unknown");
        };
        assert_eq!((kind.as_deref(), p.rows.as_str()), (Some("Graphic"), "C0"));
    }

    #[test]
    fn a_grob_that_does_not_add_up_has_no_picture() {
        let mut n = grob(&["#.", ".#"]);
        n[5] ^= 1;
        assert_eq!(graphic(&n), None, "a wrong length");
        let n = grob(&["#.", ".#"]);
        assert_eq!(graphic(&n[..n.len() - 1]), None, "cut short");
        assert_eq!(graphic(&[0xE, 0x1, 0xB, 0x2, 0x1]), None, "no fields");
        // No pixels (`#0 #0 BLANK`, `#0 #5 BLANK`): an unknown with its
        // nibbles, not a picture the page cannot draw.
        assert_eq!(graphic(&grob(&[])), None, "0 by 0");
        assert_eq!(graphic(&grob(&[""; 5])), None, "0 wide, 5 high");
        assert!(matches!(
            decode(&grob(&[]), &NoMemory),
            Ok(Object::Unknown { graphic: None, .. })
        ));
        // Wider than the cap allows: no picture, still an object.
        let mut big = vec![0xE, 0x1, 0xB, 0x2, 0x0];
        for v in [0u32, 0x1000, 0x1001] {
            big.extend((0..5).map(|i| ((v >> (4 * i)) & 0xF) as u8));
        }
        assert_eq!(graphic(&big), None, "4096 by 4097 pixels");
        let mut trailing = grob(&["#"]);
        trailing.push(0);
        assert_eq!(
            graphic(&trailing).map(|g| g.rows),
            Some("80".into()),
            "a trailing nibble is not the GROB's"
        );
    }
}
