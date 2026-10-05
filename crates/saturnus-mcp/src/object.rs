//! Typed calculator objects: the JSON model of the semantic tools, the
//! exact decoder for the binary objects a Kermit GET returns (`HPHP48-x` /
//! `HPHP49-x` files), the encoder for a binary SEND, and RPL source text
//! for objects sent as text.
//!
//! Self-contained on top of `hptx-core`'s public object walk so that it can
//! move into `hptx-core` later. Body layouts: wiki:
//! protocols/hp-object-format ("Object bodies"), from RPLMAN chapter 3 and
//! objects observed on the 48SX and 49G.
//!
//! - Reals keep their exact 12-digit mantissa ([`Real`]); JSON shows them as
//!   numbers (12 significant digits survive an `f64`) unless the exponent
//!   is beyond an `f64`'s range, then as text such as `"1.5E-400"`.
//! - Built-in constants inside composites are 5-nibble ROM pointers; a
//!   [`Memory`] reads the object they point to (the emulator's ROM).
//! - Programs, algebraics, unit expressions and commands are not decoded
//!   from their bodies: their text comes from the ASCII transfer of the
//!   same object ([`fill_sources`]), walked in step with the decoded tree.

use std::fmt;

use anyhow::{Context, Result, bail};
use hptx_core::charset;
use hptx_core::object::{BinaryHeader, Family, HEADER_LEN, ObjectType, object_size, pack, unpack};
use serde::de::{self, Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

/// SEMI, the end marker of composites (lists, programs, algebraics, units).
const SEMI: u32 = 0x0312B;
/// Deepest nesting the decoder follows.
const MAX_DEPTH: usize = 64;
/// Longest hex dump of an unknown object, in nibbles.
const MAX_HEX_NIBBLES: usize = 4096;
/// Largest object read through a ROM pointer, in nibbles.
const MAX_ROM_OBJECT: usize = 1 << 16;
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

    /// The nearest `f64` (exact to 12 digits within [`F64_EXPONENTS`]).
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
    fn from_nibbles(n: &[u8]) -> Result<Integer> {
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

    fn to_nibbles(&self) -> Vec<u8> {
        if self.digits == "0" {
            return vec![0];
        }
        let mut n: Vec<u8> = self.digits.bytes().rev().map(|b| b - b'0').collect();
        n.push(if self.negative { 9 } else { 0 });
        n
    }

    fn to_source(&self) -> String {
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
    /// A built-in command or other ROM object inside a composite.
    Command {
        /// Its name as the calculator shows it, e.g. `+`.
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
    },
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
    hptx_core::object::read_field(n, at, width)
        .with_context(|| format!("object truncated at nibble {at}"))
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

/// Decode a binary transfer file (`HPHP48-x` / `HPHP49-x` header).
pub fn decode_file(data: &[u8], mem: &dyn Memory) -> Result<Object> {
    if BinaryHeader::parse(data).is_none() {
        bail!("not an HP binary object (no HPHP48-x / HPHP49-x header)");
    }
    let nibbles = unpack(&data[HEADER_LEN..]);
    decode(&nibbles, mem)
}

/// Decode the object at the start of `nibbles`. An object the walk does
/// not understand becomes [`Object::Unknown`] with all of `nibbles`.
pub fn decode(nibbles: &[u8], mem: &dyn Memory) -> Result<Object> {
    let decoder = Decoder { mem };
    match decoder.object(nibbles, 0, 0) {
        Ok((obj, _)) => Ok(obj),
        Err(_) => {
            let prolog = field(nibbles, 0, 5)?;
            Ok(unknown(prolog, nibbles))
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
    }
}

struct Decoder<'a> {
    mem: &'a dyn Memory,
}

impl Decoder<'_> {
    /// The object with a prolog at `at` and its size in nibbles.
    fn object(&self, n: &[u8], at: usize, depth: usize) -> Result<(Object, usize)> {
        if depth > MAX_DEPTH {
            bail!("objects nested deeper than {MAX_DEPTH} levels");
        }
        let prolog = field(n, at, 5)?;
        let ty = ObjectType::from_prolog(prolog)
            .with_context(|| format!("unknown prolog {prolog:05X} at nibble {at}"))?;
        let size = object_size(n, at).map_err(|e| anyhow::anyhow!("{e}"))?;
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
                items: self.elements(n, body, depth)?,
            },
            ObjectType::Tagged => {
                let count = usize_field(n, body, 2)?;
                let tag = chars(n, body + 2, count)?;
                let (object, _) = self.element(n, body + 2 + 2 * count, depth)?;
                Object::Tagged {
                    tag,
                    object: Box::new(object),
                }
            }
            ObjectType::Unit => {
                let (first, _) = self.element(n, body, depth)?;
                let Object::Real { value } = first else {
                    bail!("unit does not start with a real");
                };
                Object::Unit { value, unit: None }
            }
            ObjectType::Array => self.array(n, at, size)?,
            ObjectType::Program => Object::Program { source: None },
            ObjectType::Algebraic => Object::Algebraic { source: None },
            ObjectType::XlibName => Object::Command { source: None },
            _ => unknown(prolog, &n[at..at + size]),
        };
        Ok((obj, size))
    }

    /// Composite elements from `at` up to SEMI.
    fn elements(&self, n: &[u8], mut at: usize, depth: usize) -> Result<Vec<Object>> {
        let mut items = Vec::new();
        while field(n, at, 5)? != SEMI {
            let (obj, size) = self.element(n, at, depth)?;
            items.push(obj);
            at += size;
        }
        Ok(items)
    }

    /// An embedded object or a 5-nibble ROM pointer at `at`.
    fn element(&self, n: &[u8], at: usize, depth: usize) -> Result<(Object, usize)> {
        let p = field(n, at, 5)?;
        if ObjectType::from_prolog(p).is_some() {
            self.object(n, at, depth + 1)
        } else {
            Ok((self.rom_object(p, depth + 1)?, 5))
        }
    }

    /// The object a ROM pointer points to, or a command.
    fn rom_object(&self, addr: u32, depth: usize) -> Result<Object> {
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
        let Some(prolog) = hptx_core::object::read_field(&head, 0, 5) else {
            return Ok(Object::Command { source: None });
        };
        if ObjectType::from_prolog(prolog).is_none() {
            // A primitive (its prolog is its own code) or a word in ROM.
            return Ok(Object::Command { source: None });
        }
        let mut len = 64;
        loop {
            let nib = read(len);
            match object_size(&nib, 0) {
                Ok(_) => return Ok(self.object(&nib, 0, depth)?.0),
                Err(_) if nib.len() == len && len < MAX_ROM_OBJECT => len *= 8,
                Err(e) => bail!("ROM object at {addr:05X}: {e}"),
            }
        }
    }

    /// A real or complex array.
    fn array(&self, n: &[u8], at: usize, size: usize) -> Result<Object> {
        let body = at + 5;
        let elem = field(n, body + 5, 5)?;
        let ndims = usize_field(n, body + 10, 5)?;
        let mut dims = Vec::with_capacity(ndims);
        for i in 0..ndims {
            dims.push(usize_field(n, body + 15 + 5 * i, 5)?);
        }
        let width = match ObjectType::from_prolog(elem) {
            Some(ObjectType::Real) => 16,
            Some(ObjectType::Complex) => 32,
            _ => return Ok(unknown(field(n, at, 5)?, &n[at..at + size])),
        };
        let count: usize = dims.iter().product();
        let start = body + 15 + 5 * ndims;
        if ndims == 0 || start + count * width > at + size {
            bail!("array dimensions do not match its length");
        }
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

/// Elements in lexicographic index order, nested by dimension.
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
    /// transfer ([`fill_sources`]).
    pub fn needs_source(&self) -> bool {
        match self {
            Object::Program { source }
            | Object::Algebraic { source }
            | Object::Command { source } => source.is_none(),
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

/// Fill the text of programs, algebraics, units, commands and unknown
/// objects in `obj` from `text`, the ASCII transfer of the same object (the
/// `%%HP:` header is skipped). Walks the text in step with the decoded
/// tree; if they disagree, nested texts stay unset and a top-level
/// program, algebraic or unknown object gets the whole text.
pub fn fill_sources(obj: &mut Object, text: &str) {
    let body = match hptx_core::object::AsciiHeader::parse(text.as_bytes()) {
        Some((_, len)) => text.get(len..).unwrap_or(text),
        None => text,
    };
    let body = body.replace("\r\n", "\n");
    let mut filled = obj.clone();
    let mut scan = Scanner { s: &body, pos: 0 };
    let ok = scan.fill(&mut filled).is_some() && scan.rest().trim().is_empty();
    if ok {
        *obj = filled;
        return;
    }
    let whole = Some(body.trim().to_string());
    match obj {
        Object::Program { source }
        | Object::Algebraic { source }
        | Object::Command { source }
        | Object::Unknown { source, .. } => *source = whole,
        _ => {}
    }
}

/// A cursor over RPL text.
struct Scanner<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    fn skip_ws(&mut self) {
        let r = self.rest();
        self.pos += r.len() - r.trim_start().len();
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn expect(&mut self, lit: &str) -> Option<()> {
        self.skip_ws();
        if self.rest().starts_with(lit) {
            self.pos += lit.len();
            Some(())
        } else {
            None
        }
    }

    /// Up to the next whitespace.
    fn token(&mut self) -> Option<&'a str> {
        self.skip_ws();
        let r = self.rest();
        let end = r.find(char::is_whitespace).unwrap_or(r.len());
        if end == 0 {
            return None;
        }
        self.pos += end;
        Some(&r[..end])
    }

    /// From `open` to its matching `close`, skipping string literals.
    fn balanced(&mut self, open: char, close: char) -> Option<&'a str> {
        self.skip_ws();
        let r = self.rest();
        if !r.starts_with(open) {
            return None;
        }
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (i, c) in r.char_indices() {
            if in_string {
                match c {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            if c == '"' {
                in_string = true;
            } else if c == open && (open != close || depth == 0) {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    let end = i + c.len_utf8();
                    self.pos += end;
                    return Some(&r[..end]);
                }
            }
        }
        None
    }

    /// A string literal (49G escapes `\"` and `\\`).
    fn string(&mut self) -> Option<&'a str> {
        self.skip_ws();
        let r = self.rest();
        if !r.starts_with('"') {
            return None;
        }
        let mut escaped = false;
        for (i, c) in r.char_indices().skip(1) {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => {
                    self.pos += i + 1;
                    return Some(&r[..=i]);
                }
                _ => {}
            }
        }
        None
    }

    /// Text quoted with `'`.
    fn quoted(&mut self) -> Option<&'a str> {
        self.skip_ws();
        let r = self.rest();
        let inner = r.strip_prefix('\'')?;
        let end = inner.find('\'')? + 2;
        self.pos += end;
        Some(&r[..end])
    }

    /// Any one object's text: a delimited group or a token.
    fn any(&mut self) -> Option<&'a str> {
        self.skip_ws();
        match self.peek()? {
            '{' => self.balanced('{', '}'),
            '[' => self.balanced('[', ']'),
            '«' => self.balanced('«', '»'),
            '(' => self.balanced('(', ')'),
            '"' => self.string(),
            '\'' => self.quoted(),
            _ => self.token(),
        }
    }

    /// Consume the text of `obj`, filling its source fields.
    fn fill(&mut self, obj: &mut Object) -> Option<()> {
        self.skip_ws();
        match obj {
            Object::Real { .. } | Object::Integer { .. } => {
                self.token()?;
            }
            Object::Name { .. } | Object::LocalName { .. } => {
                if self.peek() == Some('\'') {
                    self.quoted()?;
                } else {
                    self.token()?;
                }
            }
            Object::Character { .. } => {
                self.any()?;
            }
            Object::Binary { .. } => {
                if self.token()? == "#" {
                    self.token()?;
                }
            }
            Object::String { .. } => {
                self.string()?;
            }
            Object::Complex { .. } => {
                self.balanced('(', ')')?;
            }
            Object::Array { .. } => {
                self.balanced('[', ']')?;
            }
            Object::List { items } => {
                self.expect("{")?;
                for item in items.iter_mut() {
                    self.fill(item)?;
                }
                self.expect("}")?;
            }
            Object::Tagged { tag, object } => {
                self.expect(&format!(":{tag}:"))?;
                self.fill(object)?;
            }
            Object::Program { source } => {
                *source = Some(self.balanced('«', '»')?.to_string());
            }
            Object::Algebraic { source } => {
                *source = Some(self.quoted()?.to_string());
            }
            Object::Unit { unit, .. } => {
                let t = if self.peek() == Some('\'') {
                    let q = self.quoted()?;
                    &q[1..q.len() - 1]
                } else {
                    self.token()?
                };
                *unit = Some(t.split_once('_')?.1.to_string());
            }
            Object::Command { source } | Object::Unknown { source, .. } => {
                *source = Some(self.any()?.to_string());
            }
        }
        Some(())
    }
}

/// Nibbles of a little-endian `width`-nibble field.
fn put_field(out: &mut Vec<u8>, value: u64, width: usize) {
    out.extend((0..width).map(|i| ((value >> (4 * i)) & 0xF) as u8));
}

fn put_chars(out: &mut Vec<u8>, text: &str) -> Result<usize> {
    let bytes = charset::encode(text).map_err(|e| anyhow::anyhow!("{e}"))?;
    for b in &bytes {
        put_field(out, u64::from(*b), 2);
    }
    Ok(bytes.len())
}

/// The binary transfer file for `obj` (header and packed nibbles, exactly
/// as long as the object), or `None` for objects that only travel as text
/// (programs, algebraics, units, arrays, commands).
pub fn encode_file(obj: &Object, family: Family) -> Result<Option<Vec<u8>>> {
    let mut nibbles = Vec::new();
    if !encode_into(obj, family, &mut nibbles)? {
        return Ok(None);
    }
    // The ROM letter is not checked by the calculators (wiki:
    // protocols/hp-object-format).
    let header = BinaryHeader { family, rom: b'X' };
    let mut data = header.to_bytes().to_vec();
    data.extend(pack(&nibbles));
    Ok(Some(data))
}

fn prolog(out: &mut Vec<u8>, ty: ObjectType) {
    put_field(out, u64::from(ty.prolog()), 5);
}

fn encode_into(obj: &Object, family: Family, out: &mut Vec<u8>) -> Result<bool> {
    match obj {
        Object::Real { value } => {
            prolog(out, ObjectType::Real);
            out.extend(value.to_body());
        }
        Object::Complex { re, im } => {
            prolog(out, ObjectType::Complex);
            out.extend(re.to_body());
            out.extend(im.to_body());
        }
        Object::String { value } => {
            prolog(out, ObjectType::String);
            let mut body = Vec::new();
            let n = put_chars(&mut body, value)?;
            put_field(out, 5 + 2 * n as u64, 5);
            out.extend(body);
        }
        Object::Name { value } | Object::LocalName { value } => {
            let ty = if matches!(obj, Object::Name { .. }) {
                ObjectType::GlobalName
            } else {
                ObjectType::LocalName
            };
            prolog(out, ty);
            let mut body = Vec::new();
            let n = put_chars(&mut body, value)?;
            if n == 0 || n > 255 {
                bail!("a name has 1 to 255 characters: {value:?}");
            }
            put_field(out, n as u64, 2);
            out.extend(body);
        }
        Object::Character { value } => {
            prolog(out, ObjectType::Character);
            if put_chars(out, value)? != 1 {
                bail!("a character object holds one character: {value:?}");
            }
        }
        Object::Binary { value, .. } => {
            prolog(out, ObjectType::BinaryInteger);
            put_field(out, 21, 5);
            put_field(out, *value, 16);
        }
        Object::Integer { value } => {
            if family != Family::Hp49 {
                bail!("exact integers exist on the 49G only; send a real instead");
            }
            prolog(out, ObjectType::Integer);
            let n = value.to_nibbles();
            put_field(out, 5 + n.len() as u64, 5);
            out.extend(n);
        }
        Object::List { items } => {
            prolog(out, ObjectType::List);
            for item in items {
                if !encode_into(item, family, out)? {
                    return Ok(false);
                }
            }
            put_field(out, u64::from(SEMI), 5);
        }
        Object::Tagged { tag, object } => {
            prolog(out, ObjectType::Tagged);
            let mut body = Vec::new();
            let n = put_chars(&mut body, tag)?;
            if n > 255 {
                bail!("tag longer than 255 characters");
            }
            put_field(out, n as u64, 2);
            out.extend(body);
            return encode_into(object, family, out);
        }
        Object::Unknown { hex, truncated, .. } => {
            if *truncated {
                return Ok(false);
            }
            for c in hex.chars() {
                let d = c
                    .to_digit(16)
                    .with_context(|| format!("hex holds {c:?}, not a hex digit"))?;
                out.push(d as u8);
            }
        }
        Object::Unit { .. }
        | Object::Array { .. }
        | Object::Program { .. }
        | Object::Algebraic { .. }
        | Object::Command { .. } => return Ok(false),
    }
    Ok(true)
}

/// RPL source that compiles to `obj`, or `None` when it has none (a string
/// holding `"`, a local name, a character, an unknown object).
pub fn to_source(obj: &Object, family: Family) -> Result<Option<String>> {
    Ok(match obj {
        Object::Real { value } => Some(value.to_source()),
        Object::Integer { value } => {
            if family != Family::Hp49 {
                bail!("exact integers exist on the 49G only; send a real instead");
            }
            Some(value.to_source())
        }
        Object::Complex { re, im } => Some(format!("({},{})", re.to_source(), im.to_source())),
        Object::String { value } => (!value.contains(['"', '\\'])).then(|| format!("\"{value}\"")),
        Object::Name { value } => Some(format!("'{value}'")),
        Object::Binary { value, .. } => Some(format!("#{value:X}h")),
        Object::List { items } => {
            let mut parts = Vec::new();
            for item in items {
                match to_source(item, family)? {
                    Some(s) => parts.push(s),
                    None => return Ok(None),
                }
            }
            Some(format!("{{ {} }}", parts.join(" ")))
        }
        Object::Tagged { tag, object } => to_source(object, family)?.map(|s| format!(":{tag}:{s}")),
        Object::Unit { value, unit } => {
            let unit = unit.as_deref().context("a unit object needs its unit")?;
            Some(format!("{}_{unit}", value.to_source()))
        }
        Object::Array { items, .. } => array_source(items, family)?,
        Object::Program { source } | Object::Algebraic { source } | Object::Command { source } => {
            Some(source.clone().context("this object needs its source")?)
        }
        Object::LocalName { .. } | Object::Character { .. } | Object::Unknown { .. } => None,
    })
}

fn array_source(items: &[ArrayItem], family: Family) -> Result<Option<String>> {
    let mut parts = Vec::new();
    for item in items {
        let s = match item {
            ArrayItem::Row(row) => array_source(row, family)?,
            ArrayItem::Item(obj) => {
                if !matches!(**obj, Object::Real { .. } | Object::Complex { .. }) {
                    bail!("array elements must be reals or complex numbers");
                }
                to_source(obj, family)?
            }
        };
        match s {
            Some(s) => parts.push(s),
            None => return Ok(None),
        }
    }
    Ok(Some(format!("[ {} ]", parts.join(" "))))
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
        assert_eq!(items[0], Object::Command { source: None });
        assert_eq!(items[2], Object::String { value: "s".into() });
        assert_eq!(items[3], Object::Name { value: "X".into() });
        assert_eq!(
            items[4],
            Object::List {
                items: vec![Object::Command { source: None }]
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
                    object: Box::new(Object::Command { source: None })
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
                    Object::Command { source: None },
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
        assert_eq!(
            to_source(&m, Family::Hp48).unwrap().unwrap(),
            "[ [ 1. 2. ] [ 3. 4. ] ]"
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
    fn sources_come_from_the_ascii_text() {
        // Program, algebraic, unit (48SX), each at top level.
        let mut p = dec("D9D20E16329C2A2ED2A276BA193632B21300");
        assert!(p.needs_source());
        fill_sources(&mut p, "%%HP: T(1)A(D)F(.);\r\n« 1 2 +\r\n»\r\n");
        assert_eq!(
            p,
            Object::Program {
                source: Some("« 1 2 +\n»".into())
            }
        );
        let mut a = dec("8BA2084E201085ED2A2D20B1B21300");
        fill_sources(&mut a, "%%HP: T(1)A(D)F(.);\r\n'X^2'\r\n");
        assert_eq!(
            a,
            Object::Algebraic {
                source: Some("'X^2'".into())
            }
        );
        // The unit's leading 1 is a ROM pointer: without memory the unit
        // cannot be decoded and stays unknown, with it the number is exact.
        let unit = "ADA209C2A2C2A2070000D6C2A207000037ED2A227B0186B0168B01B21300";
        assert!(matches!(dec(unit), Object::Unknown { .. }));
        let mut u = decode(&nib(unit), &Rom).unwrap();
        assert_eq!(
            u,
            Object::Unit {
                value: real("1"),
                unit: None
            }
        );
        fill_sources(&mut u, "%%HP: T(1)A(D)F(.);\r\n'1_m/s^2'\r\n");
        assert_eq!(
            u,
            Object::Unit {
                value: real("1"),
                unit: Some("m/s^2".into())
            }
        );
    }

    #[test]
    fn nested_sources_follow_the_tree() {
        let mut list = Object::List {
            items: vec![
                Object::Real { value: real("1") },
                Object::Program { source: None },
                Object::String {
                    value: "a » b".into(),
                },
                Object::Tagged {
                    tag: "T".into(),
                    object: Box::new(Object::Command { source: None }),
                },
                Object::Binary {
                    value: 42,
                    base: None,
                    text: None,
                },
                Object::List {
                    items: vec![Object::Algebraic { source: None }],
                },
                Object::Complex {
                    re: real("1"),
                    im: real("2"),
                },
            ],
        };
        fill_sources(
            &mut list,
            "{ 1 « 2 \"x»\" + » \"a » b\" :T: + # 42d { 'X+1' } (1,2) }",
        );
        let Object::List { items } = &list else {
            panic!()
        };
        assert_eq!(
            items[1],
            Object::Program {
                source: Some("« 2 \"x»\" + »".into())
            }
        );
        assert_eq!(
            items[3],
            Object::Tagged {
                tag: "T".into(),
                object: Box::new(Object::Command {
                    source: Some("+".into())
                })
            }
        );
        assert_eq!(
            items[5],
            Object::List {
                items: vec![Object::Algebraic {
                    source: Some("'X+1'".into())
                }]
            }
        );
        // A mismatch fills nothing nested.
        let mut l = Object::List {
            items: vec![Object::Program { source: None }],
        };
        fill_sources(&mut l, "{ 1 2 }");
        assert_eq!(
            l,
            Object::List {
                items: vec![Object::Program { source: None }]
            }
        );
        // ... but a top-level program takes the whole text.
        let mut p = Object::Program { source: None };
        fill_sources(&mut p, "« garbled");
        assert_eq!(
            p,
            Object::Program {
                source: Some("« garbled".into())
            }
        );
    }

    #[test]
    fn encode_round_trips_through_decode() {
        let objs = [
            Object::Real {
                value: real("-4.79425538604E-1"),
            },
            Object::Complex {
                re: real("1.5"),
                im: real("-2"),
            },
            Object::String {
                value: "Hi «x» \"q\"".into(),
            },
            Object::Name {
                value: "ABC".into(),
            },
            Object::LocalName { value: "x".into() },
            Object::Character { value: "A".into() },
            Object::Binary {
                value: u64::MAX,
                base: None,
                text: None,
            },
            Object::List {
                items: vec![
                    Object::Real { value: real("1") },
                    Object::List { items: vec![] },
                    Object::Tagged {
                        tag: "T".into(),
                        object: Box::new(Object::Real { value: real("5") }),
                    },
                ],
            },
        ];
        for o in &objs {
            let file = encode_file(o, Family::Hp48).unwrap().unwrap();
            assert_eq!(&file[..8], b"HPHP48-X");
            assert_eq!(decode_file(&file, &NoMemory).unwrap(), *o, "{o:?}");
            // Exactly the object's bytes: the 48SX rejects a trailing byte.
            let size = object_size(&unpack(&file[8..]), 0).unwrap();
            assert_eq!(file.len(), 8 + size.div_ceil(2));
        }
        // The known 48SX encoding of 8.72653549837E-3.
        let file = encode_file(
            &Object::Real {
                value: real("8.72653549837E-3"),
            },
            Family::Hp48,
        )
        .unwrap()
        .unwrap();
        assert_eq!(unpack(&file[8..])[..21], nib("339207997389453562780")[..]);
        let i = Object::Integer {
            value: Integer::parse("-1234567890123456789").unwrap(),
        };
        let file = encode_file(&i, Family::Hp49).unwrap().unwrap();
        assert_eq!(
            unpack(&file[8..])[..30],
            nib("416209100098765432109876543219")[..]
        );
        assert!(encode_file(&i, Family::Hp48).is_err());
        assert_eq!(
            encode_file(
                &Object::Program {
                    source: Some("« »".into())
                },
                Family::Hp48
            )
            .unwrap(),
            None
        );
        // An unknown object travels back as its hex.
        let u = unknown(0x02B1E, &nib("E1B20"));
        assert_eq!(
            encode_file(&u, Family::Hp48).unwrap().unwrap()[8..],
            pack(&nib("E1B20"))[..]
        );
    }

    #[test]
    fn sources_for_text_transfers() {
        let src = |o: Object| to_source(&o, Family::Hp48).unwrap();
        assert_eq!(src(Object::Real { value: real("0.5") }).unwrap(), "5.E-1");
        assert_eq!(
            src(Object::Complex {
                re: real("1"),
                im: real("-2")
            })
            .unwrap(),
            "(1.,-2.)"
        );
        assert_eq!(
            src(Object::String {
                value: "a b".into()
            })
            .unwrap(),
            "\"a b\""
        );
        assert_eq!(
            src(Object::String {
                value: "a\"b".into()
            }),
            None
        );
        assert_eq!(
            src(Object::Binary {
                value: 255,
                base: None,
                text: None
            })
            .unwrap(),
            "#FFh"
        );
        assert_eq!(
            src(Object::List {
                items: vec![
                    Object::Name { value: "X".into() },
                    Object::Unit {
                        value: real("9.81"),
                        unit: Some("m/s^2".into())
                    }
                ]
            })
            .unwrap(),
            "{ 'X' 9.81_m/s^2 }"
        );
        assert_eq!(src(Object::LocalName { value: "x".into() }), None);
        assert!(to_source(&Object::Program { source: None }, Family::Hp48).is_err());
    }

    #[test]
    fn json_shapes_parse() {
        let o: Object = serde_json::from_value(json!({"type": "list", "items": [
            {"type": "real", "value": 0.5},
            {"type": "string", "value": "s"},
            {"type": "program", "source": "« 1 »"},
            {"type": "binary", "value": 42},
            {"type": "array", "dims": [2], "items": [{"type": "real", "value": 1}, {"type": "real", "value": 2}]}
        ]}))
        .unwrap();
        let Object::List { items } = o else { panic!() };
        assert_eq!(items.len(), 5);
        assert!(serde_json::from_value::<Object>(json!({"type": "real", "value": "x"})).is_err());
        assert!(serde_json::from_value::<Object>(json!({"type": "nope"})).is_err());
    }
}
