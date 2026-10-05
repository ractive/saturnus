//! The objects of the semantic tools: the typed model and its decoder
//! live in `saturnus-objects` (re-exported here); this module adds what
//! rides on the Kermit transfers: decoding a binary transfer file
//! (`HPHP48-x` / `HPHP49-x`), the text of programs, algebraics, units and
//! commands from the ASCII transfer of the same object, the encoder for a
//! binary SEND, and RPL source text for objects sent as text.

use anyhow::{Context, Result, bail};
use hptx_core::charset;
use hptx_core::object::{BinaryHeader, Family, HEADER_LEN, ObjectType, pack, unpack};

pub use saturnus_objects::object::{
    ArrayItem, Base, Integer, Memory, NoMemory, Object, Real, decode, decode_at,
};

/// SEMI, the end marker of composites.
const SEMI: u32 = 0x0312B;

/// Decode a binary transfer file (`HPHP48-x` / `HPHP49-x` header).
pub fn decode_file(data: &[u8], mem: &dyn Memory) -> Result<Object> {
    if BinaryHeader::parse(data).is_none() {
        bail!("not an HP binary object (no HPHP48-x / HPHP49-x header)");
    }
    let nibbles = unpack(&data[HEADER_LEN..]);
    decode(&nibbles, mem)
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
        // A name or tag that is not a plain token could close its quotes
        // and run commands: those travel in binary only.
        Object::Name { value } => hptx_core::calc::validate_name(value)
            .is_ok()
            .then(|| format!("'{value}'")),
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
        Object::Tagged { tag, object } => {
            if !plain_token(tag, false) {
                return Ok(None);
            }
            to_source(object, family)?.map(|s| format!(":{tag}:{s}"))
        }
        Object::Unit { value, unit } => {
            let unit = unit.as_deref().context("a unit object needs its unit")?;
            if !plain_token(unit, true) || unit.contains('_') {
                bail!(
                    "not a unit expression (spaces and RPL delimiters are not allowed): {unit:?}"
                );
            }
            Some(format!("{}_{unit}", value.to_source()))
        }
        Object::Array { items, .. } => array_source(items, family)?,
        Object::Program { source } | Object::Algebraic { source } | Object::Command { source } => {
            let source = source.as_deref().context("this object needs its source")?;
            Some(one_object_source(obj, source)?)
        }
        Object::LocalName { .. } | Object::Character { .. } | Object::Unknown { .. } => None,
    })
}

/// Whether `s` is one RPL token that cannot close a quote or a group:
/// no whitespace, control characters, quotes or delimiters (parentheses
/// only with `parens`, for unit expressions).
fn plain_token(s: &str, parens: bool) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            !c.is_whitespace()
                && !c.is_control()
                && !matches!(
                    c,
                    '\'' | '"' | '«' | '»' | '{' | '}' | '[' | ']' | ':' | '#' | ',' | ';' | '\\'
                )
                && (parens || !matches!(c, '(' | ')'))
        })
}

/// `source` checked to be exactly one program (one balanced `« »`), one
/// algebraic (one `' '` group) or one command (a plain token), with
/// nothing before or after it: a host command must push it, not run
/// anything.
fn one_object_source(obj: &Object, source: &str) -> Result<String> {
    // The calculator reads the trigraphs as the delimiters themselves.
    let text = source.trim().replace("\\<<", "«").replace("\\>>", "»");
    let mut scan = Scanner { s: &text, pos: 0 };
    let ok = match obj {
        Object::Program { .. } => scan.balanced('«', '»').is_some(),
        Object::Algebraic { .. } => scan.quoted().is_some_and(|q| q.len() > 2),
        _ => plain_token(&text, true) && scan.token().is_some(),
    };
    if !ok || !scan.rest().trim().is_empty() {
        let what = match obj {
            Object::Program { .. } => "a program's source must be one « ... » group",
            Object::Algebraic { .. } => "an algebraic's source must be one '...' group",
            _ => "a command's source must be one token",
        };
        bail!("{what} with nothing after it: {source:?}");
    }
    Ok(source.trim().to_string())
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

    /// A memory holding the real 1 at #2A2C9 (48SX ROM J).
    struct Rom;
    impl Memory for Rom {
        fn nibble(&self, addr: u32) -> Option<u8> {
            let i = addr.checked_sub(0x2A2C9)? as usize;
            nib("339200000000000000010").get(i).copied()
        }
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
            let size = hptx_core::object::object_size(&unpack(&file[8..]), 0).unwrap();
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
        let u = dec("E1B20A0000FFFFF");
        assert!(matches!(u, Object::Unknown { .. }));
        assert_eq!(
            encode_file(&u, Family::Hp48).unwrap().unwrap()[8..],
            pack(&nib("E1B20A0000FFFFF"))[..]
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
    fn text_cannot_break_out_of_its_quotes() {
        let src = |o: Object| to_source(&o, Family::Hp48);
        // Names and tags that are not plain tokens go binary instead.
        assert_eq!(
            src(Object::Name {
                value: "X' 11. 'QA' STO 'Y".into()
            })
            .unwrap(),
            None
        );
        let odd = Object::Tagged {
            tag: "a:b".into(),
            object: Box::new(Object::Real { value: Real::ZERO }),
        };
        assert_eq!(src(odd.clone()).unwrap(), None);
        assert!(encode_file(&odd, Family::Hp48).unwrap().is_some());
        let spaced = Object::Tagged {
            tag: "my tag".into(),
            object: Box::new(Object::Real { value: Real::ZERO }),
        };
        assert_eq!(src(spaced).unwrap(), None);
        // Programs, algebraics, units and commands are refused.
        let p = |s: &str| Object::Program {
            source: Some(s.into()),
        };
        assert!(src(p("« 1 » 22. 'QB' STO « 2 »")).is_err());
        assert!(src(p("1 2 +")).is_err());
        assert!(src(p("« 1 \"»\" »")).is_ok());
        assert!(src(p("\\<< 1 \\>>")).is_ok());
        assert!(src(p("« « 1 » »")).is_ok());
        let a = |s: &str| Object::Algebraic {
            source: Some(s.into()),
        };
        assert!(src(a("'X' 'Y' 44. 'QD' STO 'Z'")).is_err());
        assert!(src(a("X+1")).is_err());
        assert!(src(a("''")).is_err());
        assert_eq!(src(a("'X^2+1'")).unwrap().unwrap(), "'X^2+1'");
        let u = |s: &str| Object::Unit {
            value: Real::ZERO,
            unit: Some(s.into()),
        };
        assert!(src(u("m 33. 'QC' STO")).is_err());
        assert!(src(u("m'")).is_err());
        assert!(src(u("(m/s)^2")).is_ok());
        let c = |s: &str| Object::Command {
            source: Some(s.into()),
        };
        assert!(src(c("+ 1. 'QE' STO")).is_err());
        assert_eq!(src(c("\u{2192}LIST")).unwrap().unwrap(), "\u{2192}LIST");
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
