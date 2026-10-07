//! The calculator's text for objects: numbers in the display mode, data
//! objects as the stack shows them, programs, algebraics (infix) and unit
//! expressions rebuilt from their elements.
//!
//! The rules (number formats, spacing, the operators' precedence and
//! parentheses) are those the ROM's own decompiler shows, checked against
//! it by the ROM-gated oracle test (`saturnus-mcp` e2e,
//! `decompiler_matches_the_rom`); wiki: protocols/rpl-libraries for how
//! commands are named.
//!
//! Text is capped at [`MAX_TEXT`] characters; longer text ends in
//! [`TRUNCATED`]. The texts [`described`] adds are capped together at
//! [`MAX_DESCRIBED_TEXT`].

use anyhow::{Result, bail};
use saturnus::Model;

use crate::object::{ArrayItem, Base, Object, Real};
use crate::ram::Flags;

/// Longest text produced for one object, in characters.
pub const MAX_TEXT: usize = 1 << 16;
/// Appended to text cut at [`MAX_TEXT`].
pub const TRUNCATED: &str = "…";
/// Most characters of `text` fields one [`described`] call (one stack
/// read) adds. Every object inside another carries its own text, so a
/// string nested 64 levels deep is written 64 times: the decode budget
/// bounds the objects, this bounds the copies of their text. 4 M
/// characters, a few MB of JSON; the 49G's whole RAM (256 KB) holds at
/// most 256 K characters of strings.
pub const MAX_DESCRIBED_TEXT: usize = 1 << 22;

/// How reals are shown (flags -49 and -50, digits in -45 to -48).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberFormat {
    /// Standard: up to 12 digits, no trailing zeros.
    Std,
    /// Fixed, with this many decimals.
    Fix(u8),
    /// Scientific, with this many decimals in the mantissa.
    Sci(u8),
    /// Engineering, with this many digits after the first.
    Eng(u8),
}

/// The display settings the text depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Real number format.
    pub format: NumberFormat,
    /// Comma as the fraction mark (flag -51); the separator in complex
    /// numbers becomes `;`.
    pub comma: bool,
    /// Binary integer base (flags -11, -12).
    pub base: Base,
    /// Binary integer word size in bits, 1-64 (flags -5 to -10).
    pub word_size: u32,
    /// Show every parenthesis in algebraics (flag -53).
    pub all_parens: bool,
    /// Reals with an integer value keep their point (`5.`), as on the 49G,
    /// whose exact integers show without one (not inside unit objects).
    pub real_point: bool,
    /// `^` groups to the right (`A^B^C` is `A^(B^C)`), as on the 49G; on
    /// the 48 it groups to the left.
    pub power_right: bool,
    /// FIX groups the integer digits by three (`1,234.500`; `1.234,500`
    /// with a comma fraction mark) inside lists, programs, arrays and
    /// tagged objects too, as the 49G does; both models group a real that
    /// is a whole stack level. Never inside complex numbers, algebraics or
    /// units.
    pub group_digits: bool,
    /// A tagged object inside a composite shows its tag between colons
    /// (`:T: 5`, the 48) or with a trailing one only (`T: 5`, the 49G, and
    /// both models for a whole stack level).
    pub tag_colon: bool,
}

impl Default for Settings {
    /// The calculator's defaults: STD, period, decimal binaries, 64 bits.
    fn default() -> Self {
        Settings {
            format: NumberFormat::Std,
            comma: false,
            base: Base::Dec,
            word_size: 64,
            all_parens: false,
            real_point: false,
            power_right: false,
            group_digits: false,
            tag_colon: true,
        }
    }
}

impl Settings {
    /// The defaults of `model`.
    pub fn standard(model: Model) -> Settings {
        let hp49 = model == Model::Hp49g;
        Settings {
            real_point: hp49,
            power_right: hp49,
            tag_colon: !hp49,
            ..Settings::default()
        }
    }

    /// The settings `flags` select on `model`.
    pub fn from_flags(flags: &Flags, model: Model) -> Settings {
        let set = |n: i32| flags.get(n).unwrap_or(false);
        // Bit k of a multi-flag value is flag -(first + k).
        let value = |first: i32, bits: i32| {
            (0..bits).fold(0u32, |v, k| v | (u32::from(set(-(first + k))) << k))
        };
        let digits = u8::try_from(value(45, 4)).unwrap_or(0);
        let format = match (set(-49), set(-50)) {
            (false, false) => NumberFormat::Std,
            (true, false) => NumberFormat::Fix(digits),
            (false, true) => NumberFormat::Sci(digits),
            (true, true) => NumberFormat::Eng(digits),
        };
        let hp49 = model == Model::Hp49g;
        Settings {
            format,
            comma: set(-51),
            base: flags.base(),
            word_size: value(5, 6) + 1,
            all_parens: set(-53),
            real_point: hp49,
            power_right: hp49,
            group_digits: hp49,
            tag_colon: !hp49,
        }
    }

    /// `value` as a binary integer in the base and word size.
    pub fn binary(&self, value: u64) -> String {
        let mask = if self.word_size >= 64 {
            u64::MAX
        } else {
            (1u64 << self.word_size) - 1
        };
        self.base.format(value & mask)
    }

    /// `r` as the calculator shows it.
    pub fn real(&self, r: &Real) -> String {
        let s = match self.format {
            NumberFormat::Std => std_text(r, self.real_point),
            NumberFormat::Fix(d) if self.group_digits => {
                group(&fix_text(r, usize::from(d)), self.comma)
            }
            NumberFormat::Fix(d) => fix_text(r, usize::from(d)),
            NumberFormat::Sci(d) => sci_text(r, usize::from(d), 1),
            NumberFormat::Eng(d) => sci_text(r, usize::from(d), 3),
        };
        if self.comma {
            s.replace('.', ",").replace('\u{1}', ".")
        } else {
            s
        }
    }

    /// A unit object's number or power: an integer value as an integer
    /// (`2_m^2` even in FIX), anything else in the display mode; never a
    /// trailing point or digit groups (oracle-checked: `1.50E0_m^2.50E0`
    /// in 2 SCI).
    pub fn unit_number(&self, r: &Real) -> String {
        let integer = r.exponent >= 0
            && r.exponent < 12
            && r.digits[(r.exponent as usize + 1).min(12)..]
                .iter()
                .all(|&d| d == 0);
        let format = if integer || r.digits.iter().all(|&d| d == 0) {
            NumberFormat::Std
        } else {
            self.format
        };
        Settings {
            format,
            real_point: false,
            group_digits: false,
            ..*self
        }
        .real(r)
    }

    /// The settings inside a complex number or an algebraic: no digit
    /// groups.
    fn inner(&self) -> Settings {
        Settings {
            group_digits: false,
            ..*self
        }
    }

    /// A complex number `(re,im)`.
    pub fn complex(&self, re: &Real, im: &Real) -> String {
        let sep = if self.comma { ';' } else { ',' };
        let s = self.inner();
        format!("({}{sep}{})", s.real(re), s.real(im))
    }
}

/// `text` (a FIX number, period as the mark) with its integer digits
/// grouped by three; with `comma`, the groups are separated by periods
/// (the caller swaps the mark afterwards).
fn group(text: &str, comma: bool) -> String {
    let sep = if comma { '\u{1}' } else { ',' };
    let (sign, rest) = match text.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", text),
    };
    if rest.contains('E') {
        return text.to_string();
    }
    let (int, frac) = rest.split_once('.').unwrap_or((rest, ""));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(c);
    }
    format!("{sign}{out}.{frac}")
}

/// Mantissa digits without trailing zeros (at least one).
fn significant(r: &Real) -> &[u8] {
    let last = r.digits.iter().rposition(|&d| d != 0).unwrap_or(0);
    &r.digits[..=last]
}

fn digit(d: u8) -> char {
    char::from(b'0' + d)
}

/// STD: fixed notation when the digits fit 12 places, else scientific.
fn std_text(r: &Real, real_point: bool) -> String {
    let sign = if r.negative { "-" } else { "" };
    let sig = significant(r);
    if r.digits.iter().all(|&d| d == 0) {
        return if real_point { "0.".into() } else { "0".into() };
    }
    let e = r.exponent;
    let k = sig.len() as i32;
    if (0..12).contains(&e) {
        // Integer part: the first e + 1 digits (zero-padded).
        let int: String = (0..=e)
            .map(|i| digit(sig.get(i as usize).copied().unwrap_or(0)))
            .collect();
        let frac: String = sig
            .iter()
            .skip((e + 1) as usize)
            .map(|&d| digit(d))
            .collect();
        if frac.is_empty() {
            let point = if real_point { "." } else { "" };
            return format!("{sign}{int}{point}");
        }
        return format!("{sign}{int}.{frac}");
    }
    if e < 0 && -e - 1 + k <= 12 {
        let zeros = "0".repeat((-e - 1) as usize);
        let frac: String = sig.iter().map(|&d| digit(d)).collect();
        return format!("{sign}.{zeros}{frac}");
    }
    let rest: String = sig[1..].iter().map(|&d| digit(d)).collect();
    format!("{sign}{}.{rest}E{e}", digit(sig[0]))
}

/// Round the 12-digit mantissa of `r` to `keep` digits (half up); the
/// digits and the exponent after a carry.
fn rounded(r: &Real, keep: usize) -> (Vec<u8>, i32) {
    let mut d: Vec<u8> = r.digits.to_vec();
    let mut e = r.exponent;
    if keep >= 12 {
        return (d, e);
    }
    let up = d[keep] >= 5;
    d.truncate(keep);
    if up {
        let mut i = keep;
        loop {
            if i == 0 {
                d.insert(0, 1);
                d.truncate(keep.max(1));
                e += 1;
                break;
            }
            i -= 1;
            if d[i] == 9 {
                d[i] = 0;
            } else {
                d[i] += 1;
                break;
            }
        }
    }
    (d, e)
}

/// FIX: `decimals` places, at most 12 digits in all (fewer decimals for
/// large numbers); scientific when the integer part needs more than 12
/// digits or a nonzero number rounds to zero.
fn fix_text(r: &Real, decimals: usize) -> String {
    if r.digits.iter().all(|&d| d == 0) {
        return format!("0.{}", "0".repeat(decimals));
    }
    let sign = if r.negative { "-" } else { "" };
    let places = |e: i32| -> i32 {
        if e >= 0 {
            (decimals as i32).min(11 - e)
        } else {
            decimals as i32
        }
    };
    // Digits kept from the first: the integer part and the decimals.
    let keep = r.exponent + 1 + places(r.exponent);
    let (d, e) = match keep {
        k if k >= 1 => rounded(r, k as usize),
        // Below the last place: rounds to one unit of it, or to zero.
        0 if r.digits[0] >= 5 => (vec![1], r.exponent + 1),
        _ => return sci_text(r, decimals.min(11), 1),
    };
    if e >= 12 {
        return sci_text(r, decimals.min(11), 1);
    }
    let dec = places(e).max(0);
    let at = |place: i32| -> u8 {
        let i = e - place;
        if i < 0 {
            0
        } else {
            d.get(i as usize).copied().unwrap_or(0)
        }
    };
    let int: String = if e >= 0 {
        (0..=e).rev().map(|p| digit(at(p))).collect()
    } else {
        "0".into()
    };
    let frac: String = (1..=dec).map(|p| digit(at(-p))).collect();
    format!("{sign}{int}.{frac}")
}

/// SCI (`group` 1) or ENG (`group` 3) with `decimals` + 1 significant
/// digits; ENG pads the integer part to put the exponent on a multiple of
/// three.
fn sci_text(r: &Real, decimals: usize, group: i32) -> String {
    let sign = if r.negative { "-" } else { "" };
    if r.digits.iter().all(|&d| d == 0) {
        let zeros = "0".repeat(decimals);
        return format!("0.{zeros}E0");
    }
    let sig = (decimals + 1).min(12);
    let (mut d, e) = rounded(r, sig);
    let shift = e.rem_euclid(group);
    let exp = e - shift;
    let int_digits = (shift + 1) as usize;
    d.resize(sig.max(int_digits), 0);
    let int: String = d[..int_digits].iter().map(|&x| digit(x)).collect();
    let frac: String = d[int_digits..sig.max(int_digits)]
        .iter()
        .map(|&x| digit(x))
        .collect();
    format!("{sign}{int}.{frac}E{exp}")
}

/// A string capped at [`MAX_TEXT`] characters.
#[derive(Debug, Default)]
pub(crate) struct Text {
    s: String,
    chars: usize,
    full: bool,
}

impl Text {
    pub(crate) fn push(&mut self, t: &str) {
        if self.full {
            return;
        }
        for c in t.chars() {
            if self.chars == MAX_TEXT {
                self.s.push_str(TRUNCATED);
                self.full = true;
                return;
            }
            self.s.push(c);
            self.chars += 1;
        }
    }

    pub(crate) fn full(&self) -> bool {
        self.full
    }

    pub(crate) fn finish(self) -> String {
        self.s
    }
}

/// `obj` as the stack shows it as a whole level.
pub fn display(obj: &Object, s: &Settings) -> String {
    let mut t = Text::default();
    match obj {
        Object::Real { value } => t.push(
            &Settings {
                group_digits: true,
                ..*s
            }
            .real(value),
        ),
        Object::Tagged { tag, object } => {
            t.push(tag);
            t.push(": ");
            write_object(&mut t, object, s);
        }
        _ => write_object(&mut t, obj, s),
    }
    t.finish()
}

/// Whether the calculator's text of `obj` is known in full: every
/// program, algebraic and unit inside it was decompiled and every command
/// has a name (or is an XLIB name, shown by its numbers).
pub fn has_text(obj: &Object) -> bool {
    match obj {
        Object::Program { source } | Object::Algebraic { source } => source.is_some(),
        Object::Command {
            name,
            source,
            address,
            library,
            command,
        } => {
            name.is_some()
                || source.is_some()
                || (address.is_none() && library.is_some() && command.is_some())
        }
        Object::Unit { unit, .. } => unit.is_some(),
        Object::Unknown { source, .. } => source.is_some(),
        Object::List { items } => items.iter().all(has_text),
        Object::Tagged { object, .. } => has_text(object),
        Object::Array { items, .. } => array_has_text(items),
        _ => true,
    }
}

fn array_has_text(items: &[ArrayItem]) -> bool {
    items.iter().all(|i| match i {
        ArrayItem::Item(o) => has_text(o),
        ArrayItem::Row(row) => array_has_text(row),
    })
}

/// The calculator's text of `obj` as a whole stack level or a variable's
/// value ([`display`]; a name with its quotes, as the stack shows one), or
/// `None` if a part of it has none ([`has_text`]).
pub fn text(obj: &Object, s: &Settings) -> Option<String> {
    if !has_text(obj) {
        return None;
    }
    Some(match obj {
        Object::Name { value } | Object::LocalName { value } => format!("'{value}'"),
        _ => display(obj, s),
    })
}

/// The text of `obj` as an element of a list or a tagged object.
fn element_text(obj: &Object, s: &Settings) -> Option<String> {
    has_text(obj).then(|| {
        let mut t = Text::default();
        write_object(&mut t, obj, s);
        t.finish()
    })
}

/// The text of `obj` as an element of an array (a name quoted).
fn cell_text(obj: &Object, s: &Settings) -> Option<String> {
    match obj {
        Object::Name { value } | Object::LocalName { value } => Some(format!("'{value}'")),
        _ => element_text(obj, s),
    }
}

/// `obj` as JSON (its serde shape) with the calculator's own text in a
/// `text` field on the object and on every object inside it (a list's
/// items, a tagged object's object, an array's elements), each as it is
/// written in that place; no `text` where there is none ([`has_text`]).
/// A front end shows and copies these texts and formats nothing itself.
/// An error past [`MAX_DESCRIBED_TEXT`] characters of text.
pub fn described(obj: &Object, s: &Settings) -> Result<serde_json::Value> {
    let mut left = MAX_DESCRIBED_TEXT;
    described_within(obj, s, &mut left)
}

/// [`described`] with `left` characters of text, shared by the caller's
/// other calls (the levels of one stack).
pub(crate) fn described_within(
    obj: &Object,
    s: &Settings,
    left: &mut usize,
) -> Result<serde_json::Value> {
    let mut v = serde_json::to_value(obj)?;
    describe(&mut v, obj, s, text(obj, s), left)?;
    Ok(v)
}

fn describe(
    v: &mut serde_json::Value,
    obj: &Object,
    s: &Settings,
    text: Option<String>,
    left: &mut usize,
) -> Result<()> {
    let Some(map) = v.as_object_mut() else {
        return Ok(());
    };
    if let Some(text) = text {
        let Some(rest) = left.checked_sub(text.chars().count()) else {
            bail!(
                "more than {MAX_DESCRIBED_TEXT} characters of text (MAX_DESCRIBED_TEXT): \
                 objects that repeat or nest deeply"
            );
        };
        *left = rest;
        map.insert("text".to_string(), serde_json::Value::String(text));
    }
    match obj {
        Object::List { items } => {
            if let Some(vs) = map.get_mut("items").and_then(|i| i.as_array_mut()) {
                for (cv, c) in vs.iter_mut().zip(items) {
                    describe(cv, c, s, element_text(c, s), left)?;
                }
            }
        }
        Object::Tagged { object, .. } => {
            if let Some(cv) = map.get_mut("object") {
                describe(cv, object, s, element_text(object, s), left)?;
            }
        }
        Object::Array { items, .. } => {
            if let Some(vs) = map.get_mut("items").and_then(|i| i.as_array_mut()) {
                describe_array(vs, items, s, left)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn describe_array(
    vs: &mut [serde_json::Value],
    items: &[ArrayItem],
    s: &Settings,
    left: &mut usize,
) -> Result<()> {
    for (cv, item) in vs.iter_mut().zip(items) {
        match item {
            ArrayItem::Item(o) => describe(cv, o, s, cell_text(o, s), left)?,
            ArrayItem::Row(row) => {
                if let Some(rv) = cv.as_array_mut() {
                    describe_array(rv, row, s, left)?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn write_object(t: &mut Text, obj: &Object, s: &Settings) {
    if t.full() {
        return;
    }
    match obj {
        Object::Real { value } => t.push(&s.real(value)),
        Object::Integer { value } => t.push(&value.to_source()),
        Object::Complex { re, im } => t.push(&s.complex(re, im)),
        Object::String { value } => {
            t.push("\"");
            t.push(value);
            t.push("\"");
        }
        Object::Name { value } | Object::LocalName { value } => t.push(value),
        Object::Character { value } => {
            t.push("CHR ");
            t.push(value);
        }
        Object::Binary { value, .. } => t.push(&s.binary(*value)),
        Object::List { items } => {
            t.push("{");
            for o in items {
                t.push(" ");
                write_object(t, o, s);
            }
            t.push(" }");
        }
        Object::Tagged { tag, object } => {
            if s.tag_colon {
                t.push(":");
            }
            t.push(tag);
            t.push(": ");
            write_object(t, object, s);
        }
        Object::Unit { value, unit } => {
            t.push(&s.unit_number(value));
            t.push("_");
            t.push(unit.as_deref().unwrap_or("?"));
        }
        Object::Array { items, .. } => write_array(t, items, s),
        Object::Program { source } | Object::Algebraic { source } => {
            t.push(source.as_deref().unwrap_or("?"));
        }
        Object::Command {
            name,
            source,
            address,
            library,
            command,
        } => match name.as_deref().or(source.as_deref()) {
            Some(text) => t.push(text),
            None => t.push(&command_text(None, *address, library.zip(*command))),
        },
        Object::Unknown { kind, source, .. } => match source {
            Some(src) => t.push(src),
            None => t.push(kind.as_deref().unwrap_or("External")),
        },
    }
}

fn write_array(t: &mut Text, items: &[ArrayItem], s: &Settings) {
    t.push("[");
    for (i, item) in items.iter().enumerate() {
        match item {
            ArrayItem::Item(o) => {
                t.push(" ");
                // A name in a (49G symbolic) array is shown quoted.
                if let Object::Name { value } | Object::LocalName { value } = o.as_ref() {
                    t.push("'");
                    t.push(value);
                    t.push("'");
                } else {
                    write_object(t, o, s);
                }
            }
            ArrayItem::Row(row) => {
                t.push(if i == 0 { "" } else { " " });
                write_array(t, row, s);
            }
        }
    }
    if matches!(items.first(), Some(ArrayItem::Row(_))) {
        t.push("]");
    } else {
        t.push(" ]");
    }
}

/// A command's text: its name; an XLIB name without one as the
/// calculator shows it (`XLIB 1234 5`); an unnamed ROM object `External`.
fn command_text(name: Option<&str>, address: Option<u32>, xlib: Option<(u16, u16)>) -> String {
    match (name, address, xlib) {
        (Some(n), _, _) => n.to_string(),
        (None, None, Some((l, c))) => format!("XLIB {l} {c}"),
        _ => "External".to_string(),
    }
}

/// One element of a program, algebraic or unit body.
#[derive(Clone, Debug)]
pub(crate) enum Element {
    /// A data object (embedded, or a ROM constant).
    Object(Object),
    /// A command: its name (`None`: no name in the ROM's tables), its ROM
    /// address (not for XLIB names), its argument count when known.
    Command {
        name: Option<String>,
        address: Option<u32>,
        /// Library and command number, when known.
        xlib: Option<(u16, u16)>,
        arity: Option<u8>,
        /// Shown as nothing (see `CommandInfo::silent`).
        silent: bool,
    },
    /// An algebraic inside a composite, with its own elements (an
    /// operand of `∂`, `Σ`, `|`, a user function's name).
    Symbolic {
        object: Object,
        elements: Vec<Element>,
    },
    /// A system binary (in an algebraic, the operand count of a user
    /// function or of `|`).
    SystemBinary(u32),
    /// A unit operator.
    Unit(UnitOp),
}

/// Unit operators (wiki: protocols/rpl-libraries "Unit operators").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnitOp {
    Times,
    Divide,
    Power,
    Prefix,
    End,
}

impl Element {
    fn token(&self, t: &mut Text, s: &Settings) {
        match self {
            Element::Object(o) | Element::Symbolic { object: o, .. } => write_object(t, o, s),
            Element::Command {
                name,
                address,
                xlib,
                ..
            } => t.push(&command_text(name.as_deref(), *address, *xlib)),
            Element::SystemBinary(v) => t.push(&format!("<{v:X}h>")),
            Element::Unit(_) => t.push("External"),
        }
    }

    fn is_quote(&self) -> bool {
        matches!(self, Element::Command { name: Some(n), .. } if n == "'")
    }
}

/// A program's text: its elements separated by spaces. A user program's
/// body starts with the command `«` and ends with `»`; a program inside a
/// structure (a CASE clause) has neither and shows its elements only. A
/// quoted object `' X '` closes up to `'X'`.
pub(crate) fn program(elements: &[Element], s: &Settings) -> String {
    let mut t = Text::default();
    let mut first = true;
    let mut open = false;
    let mut glue = false;
    for e in elements {
        if t.full() {
            break;
        }
        if matches!(e, Element::Command { silent: true, .. }) {
            continue;
        }
        if e.is_quote() && open {
            // Closing quote: no space before it.
            t.push("'");
            open = false;
            continue;
        }
        if !first && !glue {
            t.push(" ");
        }
        first = false;
        glue = false;
        if e.is_quote() {
            // Opening quote: no space after it.
            t.push("'");
            open = true;
            glue = true;
        } else {
            e.token(&mut t, s);
        }
    }
    t.finish()
}

/// An expression node of an algebraic or unit.
#[derive(Clone, Debug)]
enum Node {
    /// Text that binds tightest (a name, a number, a call).
    Atom(String),
    /// A number, which needs parentheses where its sign could bind
    /// differently.
    Negative(String),
    /// A binary infix operator.
    Infix {
        op: String,
        prec: u8,
        left: usize,
        right: usize,
        spaced: bool,
    },
    /// A prefix operator (`-`, `NOT `, `√`).
    Prefix { op: String, prec: u8, arg: usize },
    /// A postfix operator (`!`).
    Postfix { op: String, arg: usize },
    /// A function call `F(a,b)`; with `equation`, the first two
    /// arguments are written `a=b` (`Σ(K=0,M,K)`).
    Call {
        name: String,
        args: Vec<usize>,
        equation: bool,
    },
    /// `expr|(name=value,...)`.
    Where {
        expr: usize,
        pairs: Vec<(usize, usize)>,
    },
    /// The derivative `∂X(f)`.
    Derivative { var: usize, expr: usize },
    /// Juxtaposition without operator (a unit prefix `k` + `m`).
    Concat { left: usize, right: usize },
}

/// Binding strength of the operators (higher binds tighter).
const ATOM: u8 = 20;
const PREC_ROOT: u8 = 13;
const PREC_POWER: u8 = 12;
const PREC_NEG: u8 = 11;
const PREC_PRODUCT: u8 = 10;
const PREC_SUM: u8 = 9;
const PREC_COMPARE: u8 = 7;
const PREC_NOT: u8 = 6;
const PREC_AND: u8 = 5;
const PREC_OR: u8 = 4;
const PREC_EQUATION: u8 = 3;
const PREC_WHERE: u8 = 2;

/// The infix operators by name: precedence, whether written with spaces.
fn infix(name: &str) -> Option<(u8, bool)> {
    Some(match name {
        "^" => (PREC_POWER, false),
        "*" | "/" => (PREC_PRODUCT, false),
        "+" | "-" => (PREC_SUM, false),
        "==" | "≠" | "<" | ">" | "≤" | "≥" => (PREC_COMPARE, false),
        "AND" => (PREC_AND, true),
        "OR" | "XOR" => (PREC_OR, true),
        "=" => (PREC_EQUATION, false),
        "|" => (PREC_WHERE, false),
        _ => return None,
    })
}

struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    fn add(&mut self, n: Node) -> usize {
        self.nodes.push(n);
        self.nodes.len() - 1
    }

    fn prec(&self, i: usize) -> u8 {
        match &self.nodes[i] {
            Node::Atom(_) | Node::Call { .. } | Node::Derivative { .. } | Node::Concat { .. } => {
                ATOM
            }
            Node::Where { .. } => PREC_WHERE,
            Node::Negative(_) => PREC_NEG,
            Node::Infix { prec, .. } | Node::Prefix { prec, .. } => *prec,
            Node::Postfix { .. } => ATOM - 1,
        }
    }

    /// Render node `root` into `t` without recursion (garbage can nest
    /// arbitrarily deep).
    /// Whether node `i` starts with a sign or prefix operator.
    fn signed(&self, i: usize) -> bool {
        matches!(self.nodes[i], Node::Prefix { .. } | Node::Negative(_))
    }

    fn render(&self, root: usize, t: &mut Text, all_parens: bool, power_right: bool) {
        enum Work<'a> {
            Node(usize),
            Text(&'a str),
            Owned(String),
        }
        let mut stack = vec![Work::Node(root)];
        while let Some(w) = stack.pop() {
            if t.full() {
                return;
            }
            let i = match w {
                Work::Text(s) => {
                    t.push(s);
                    continue;
                }
                Work::Owned(s) => {
                    t.push(&s);
                    continue;
                }
                Work::Node(i) => i,
            };
            // Children are pushed in reverse order of output.
            let operand = |stack: &mut Vec<Work<'_>>, child: usize, parens: bool| {
                let compound = !matches!(
                    self.nodes[child],
                    Node::Atom(_) | Node::Call { .. } | Node::Derivative { .. }
                );
                if parens || (all_parens && compound) {
                    stack.push(Work::Text(")"));
                    stack.push(Work::Node(child));
                    stack.push(Work::Text("("));
                } else {
                    stack.push(Work::Node(child));
                }
            };
            match &self.nodes[i] {
                Node::Atom(s) | Node::Negative(s) => t.push(s),
                Node::Infix {
                    op,
                    prec,
                    left,
                    right,
                    spaced,
                } => {
                    // Operators group to the left, `^` to the right where
                    // `power_right`; a sign or prefix on the right reads
                    // unambiguously (`A^-B`).
                    let right_assoc = power_right && *prec == PREC_POWER;
                    // A where-expression on the left is written bare
                    // (`X|(X=2)+1`, `X|(X=2)^2`), on the right and under
                    // a prefix it is parenthesised (`1+(X|(X=2))`,
                    // `-(X|(X=2))`): oracle-checked on all three ROMs.
                    let where_left = matches!(self.nodes[*left], Node::Where { .. });
                    let lp = !where_left
                        && if right_assoc {
                            self.prec(*left) <= *prec
                        } else {
                            self.prec(*left) < *prec
                        };
                    let rp = if right_assoc {
                        self.prec(*right) < *prec
                    } else {
                        self.prec(*right) <= *prec
                    } && !self.signed(*right);
                    operand(&mut stack, *right, rp);
                    if *spaced {
                        stack.push(Work::Owned(format!(" {op} ")));
                    } else {
                        stack.push(Work::Owned(op.clone()));
                    }
                    operand(&mut stack, *left, lp);
                }
                Node::Prefix { op, prec, arg } => {
                    let p = self.prec(*arg) < *prec && !self.signed(*arg);
                    operand(&mut stack, *arg, p);
                    // `NOT A` but `NOT(A AND B)`.
                    t.push(if p { op.trim_end() } else { op });
                }
                Node::Postfix { op, arg } => {
                    stack.push(Work::Owned(op.clone()));
                    let p = self.prec(*arg) < ATOM;
                    operand(&mut stack, *arg, p);
                }
                Node::Call {
                    name,
                    args,
                    equation,
                } => {
                    stack.push(Work::Text(")"));
                    for (k, a) in args.iter().enumerate().rev() {
                        stack.push(Work::Node(*a));
                        if k == 1 && *equation {
                            stack.push(Work::Text("="));
                        } else if k > 0 {
                            stack.push(Work::Text(","));
                        }
                    }
                    t.push(name);
                    t.push("(");
                }
                Node::Where { expr, pairs } => {
                    stack.push(Work::Text(")"));
                    for (k, (n, v)) in pairs.iter().enumerate().rev() {
                        stack.push(Work::Node(*v));
                        stack.push(Work::Text("="));
                        stack.push(Work::Node(*n));
                        if k > 0 {
                            stack.push(Work::Text(","));
                        }
                    }
                    stack.push(Work::Text("|("));
                    let p = self.prec(*expr) <= PREC_WHERE;
                    operand(&mut stack, *expr, p);
                }
                Node::Derivative { var, expr } => {
                    stack.push(Work::Text(")"));
                    stack.push(Work::Node(*expr));
                    stack.push(Work::Text("("));
                    stack.push(Work::Node(*var));
                    t.push("∂");
                }
                Node::Concat { left, right } => {
                    stack.push(Work::Node(*right));
                    stack.push(Work::Node(*left));
                }
            }
        }
    }
}

/// An operand on the evaluation stack: a node, or a user function's
/// argument count.
#[derive(Clone, Copy, Debug)]
enum Slot {
    Node(usize),
    Count(u32),
}

/// An algebraic's text with its quotes, or `None` when its elements do
/// not form one expression (a command without a known argument count, a
/// malformed body).
pub(crate) fn algebraic(elements: &[Element], s: &Settings) -> Option<String> {
    let s = &s.inner();
    let mut tree = Tree { nodes: Vec::new() };
    let root = expression(elements, s, &mut tree)?;
    let mut t = Text::default();
    t.push("'");
    tree.render(root, &mut t, s.all_parens, s.power_right);
    t.push("'");
    Some(t.finish())
}

fn leaf(o: &Object, s: &Settings) -> Node {
    let mut t = Text::default();
    write_object(&mut t, o, s);
    let text = t.finish();
    let negative = match o {
        Object::Real { value } => value.negative,
        Object::Integer { value } => value.negative,
        Object::Unit { value, .. } => value.negative,
        _ => false,
    };
    if negative {
        Node::Negative(text)
    } else {
        Node::Atom(text)
    }
}

fn expression(elements: &[Element], s: &Settings, tree: &mut Tree) -> Option<usize> {
    let mut stack: Vec<Slot> = Vec::new();
    for e in elements {
        let node = match e {
            Element::Object(o) => tree.add(leaf(o, s)),
            Element::Symbolic { elements, .. } => expression(elements, s, tree)?,
            Element::SystemBinary(v) => {
                stack.push(Slot::Count(*v));
                continue;
            }
            Element::Unit(_) => return None,
            Element::Command { name, arity, .. } => {
                if let Some(Slot::Count(n)) = stack.last().copied() {
                    stack.pop();
                    counted(name.as_deref(), n, &mut stack, tree)?
                } else {
                    apply(name.as_deref()?, *arity, &mut stack, tree)?
                }
            }
        };
        stack.push(Slot::Node(node));
    }
    match stack.as_slice() {
        [Slot::Node(n)] => Some(*n),
        _ => None,
    }
}

/// A command that takes its operand count from the stack: `|` (the
/// expression, then name and value pairs; the count covers all of them),
/// or a user function (the arguments and its name; the count covers the
/// arguments; the command applying it has no name).
fn counted(name: Option<&str>, n: u32, stack: &mut Vec<Slot>, tree: &mut Tree) -> Option<usize> {
    let mut pop = || match stack.pop()? {
        Slot::Node(n) => Some(n),
        Slot::Count(_) => None,
    };
    match name {
        Some("|") => {
            let mut items = Vec::new();
            for _ in 0..n {
                items.push(pop()?);
            }
            items.reverse();
            let (&expr, rest) = items.split_first()?;
            if rest.len() % 2 != 0 {
                return None;
            }
            let pairs = rest.chunks(2).map(|p| (p[0], p[1])).collect();
            Some(tree.add(Node::Where { expr, pairs }))
        }
        None => {
            let f = pop()?;
            let mut args = Vec::new();
            for _ in 0..n {
                args.push(pop()?);
            }
            args.reverse();
            let Node::Atom(name) = &tree.nodes[f] else {
                return None;
            };
            let name = name.clone();
            Some(tree.add(Node::Call {
                name,
                args,
                equation: false,
            }))
        }
        Some(_) => None,
    }
}

/// Apply command `name` to operands on `stack`.
fn apply(name: &str, arity: Option<u8>, stack: &mut Vec<Slot>, tree: &mut Tree) -> Option<usize> {
    let mut pop = || match stack.pop()? {
        Slot::Node(n) => Some(n),
        Slot::Count(_) => None,
    };
    // Without a known argument count there is no telling how many
    // operands it takes: no text (the caller leaves `source` unset).
    let arity = arity?;
    if arity == 2 {
        if let Some((prec, spaced)) = infix(name) {
            let right = pop()?;
            let left = pop()?;
            return Some(tree.add(Node::Infix {
                op: name.to_string(),
                prec,
                left,
                right,
                spaced,
            }));
        }
    }
    let unary = |op: &str, prec: u8, arg: usize, tree: &mut Tree| {
        tree.add(Node::Prefix {
            op: op.to_string(),
            prec,
            arg,
        })
    };
    if arity == 1 {
        match name {
            "NEG" => return Some(unary("-", PREC_NEG, pop()?, tree)),
            "NOT" => return Some(unary("NOT ", PREC_NOT, pop()?, tree)),
            "√" => return Some(unary("√", PREC_ROOT, pop()?, tree)),
            "!" => {
                let arg = pop()?;
                return Some(tree.add(Node::Postfix {
                    op: "!".into(),
                    arg,
                }));
            }
            _ => {}
        }
    }
    if name == "∂" && arity == 2 {
        let expr = pop()?;
        let var = pop()?;
        return Some(tree.add(Node::Derivative { var, expr }));
    }
    let mut args = Vec::new();
    for _ in 0..arity {
        args.push(pop()?);
    }
    args.reverse();
    if arity == 0 {
        return Some(tree.add(Node::Atom(name.to_string())));
    }
    Some(tree.add(Node::Call {
        name: name.to_string(),
        args,
        equation: name == "Σ" && arity == 4,
    }))
}

/// A unit expression's text (`kg*m^2/s^2`) from the elements after the
/// number, or `None` if they do not form one.
pub(crate) fn unit(elements: &[Element], s: &Settings) -> Option<String> {
    let mut tree = Tree { nodes: Vec::new() };
    let mut stack: Vec<usize> = Vec::new();
    let mut ended = false;
    for e in elements {
        if ended {
            return None;
        }
        match e {
            Element::Object(Object::String { value })
            | Element::Object(Object::Character { value }) => {
                stack.push(tree.add(Node::Atom(value.clone())));
            }
            Element::Object(Object::Real { value }) => {
                stack.push(tree.add(Node::Atom(s.unit_number(value))));
            }
            Element::Object(o @ Object::Integer { .. }) => {
                stack.push(tree.add(leaf(o, s)));
            }
            Element::Unit(op) => {
                if *op == UnitOp::End {
                    ended = true;
                    continue;
                }
                let right = stack.pop()?;
                let left = stack.pop()?;
                let node = match op {
                    UnitOp::Prefix => Node::Concat { left, right },
                    UnitOp::Times | UnitOp::Divide | UnitOp::Power => {
                        let (op, prec) = match op {
                            UnitOp::Times => ("*", PREC_PRODUCT),
                            UnitOp::Divide => ("/", PREC_PRODUCT),
                            _ => ("^", PREC_POWER),
                        };
                        Node::Infix {
                            op: op.into(),
                            prec,
                            left,
                            right,
                            spaced: false,
                        }
                    }
                    UnitOp::End => return None,
                };
                stack.push(tree.add(node));
            }
            _ => return None,
        }
    }
    let [root] = stack.as_slice() else {
        return None;
    };
    let mut t = Text::default();
    tree.render(*root, &mut t, false, false);
    Some(t.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real(s: &str) -> Real {
        Real::parse(s).unwrap()
    }

    /// The texts a front end shows: the cases a formatter in the page got
    /// wrong (a name in a 49G array, a whole-number unit on the 49G, a
    /// small real), each on the object and on the objects inside.
    #[test]
    fn described_carries_the_text_of_every_object() {
        let name = |n: &str| Object::Name {
            value: n.to_string(),
        };
        let hp49 = Settings::standard(Model::Hp49g);
        let hp48 = Settings::standard(Model::Hp48gx);
        let array = Object::Array {
            dims: vec![1],
            items: vec![ArrayItem::Item(Box::new(name("A")))],
        };
        let v = described(&array, &hp49).unwrap();
        assert_eq!(v["text"], "[ 'A' ]");
        assert_eq!(v["items"][0]["text"], "'A'");
        let unit = Object::Unit {
            value: real("2"),
            unit: Some("m".to_string()),
        };
        assert_eq!(described(&unit, &hp49).unwrap()["text"], "2_m");
        assert_eq!(text(&real_obj("2"), &hp49), Some("2.".to_string()));
        assert_eq!(
            described(&real_obj("1.23456789012E-5"), &hp48).unwrap()["text"],
            "1.23456789012E-5"
        );
        // A list: its own text, and each element's as written inside it;
        // a name alone is quoted, inside a list it is not.
        let list = Object::List {
            items: vec![
                real_obj("1"),
                name("QQ"),
                Object::Tagged {
                    tag: "T".to_string(),
                    object: Box::new(real_obj("2")),
                },
                Object::Program { source: None },
            ],
        };
        let v = described(&list, &hp48).unwrap();
        assert!(v.get("text").is_none(), "{v}");
        assert_eq!(v["items"][1]["text"], "QQ");
        assert_eq!(v["items"][2]["text"], ":T: 2");
        assert_eq!(v["items"][2]["object"]["text"], "2");
        assert!(v["items"][3].get("text").is_none());
        assert_eq!(text(&name("QQ"), &hp48), Some("'QQ'".to_string()));
        assert_eq!(text(&Object::Program { source: None }, &hp48), None);
    }

    fn real_obj(s: &str) -> Object {
        Object::Real { value: real(s) }
    }

    #[test]
    fn std_numbers_as_the_48_shows_them() {
        let s = Settings::default();
        for (v, text) in [
            ("1", "1"),
            ("-2", "-2"),
            ("1.5", "1.5"),
            ("1E-5", ".00001"),
            (".001", ".001"),
            ("123456789012", "123456789012"),
            ("1E12", "1.E12"),
            ("1.5E300", "1.5E300"),
            ("-1.5E-300", "-1.5E-300"),
            ("1.23456", "1.23456"),
            ("0", "0"),
            ("100", "100"),
            (".5", ".5"),
        ] {
            assert_eq!(s.real(&real(v)), text, "{v}");
        }
        let p = Settings::standard(Model::Hp49g);
        assert_eq!(p.real(&real("5")), "5.");
        assert_eq!(p.real(&real("0")), "0.");
        assert_eq!(p.real(&real("2.5")), "2.5");
    }

    #[test]
    fn text_is_capped() {
        let mut t = Text::default();
        for _ in 0..MAX_TEXT {
            t.push("ab");
        }
        let s = t.finish();
        assert!(s.ends_with(TRUNCATED));
        assert_eq!(s.chars().count(), MAX_TEXT + 1);
    }

    fn cmd(name: &str, arity: u8) -> Element {
        Element::Command {
            name: Some(name.into()),
            address: None,
            xlib: None,
            arity: Some(arity),
            silent: false,
        }
    }

    fn name(n: &str) -> Element {
        Element::Object(Object::Name { value: n.into() })
    }

    fn num(v: &str) -> Element {
        Element::Object(Object::Real { value: real(v) })
    }

    #[test]
    fn infix_with_precedence() {
        let s = Settings::default();
        let alg = |e: &[Element]| algebraic(e, &s).unwrap();
        assert_eq!(alg(&[name("A"), num("1"), cmd("+", 2)]), "'A+1'");
        // (A+B)*C and A+B*C.
        assert_eq!(
            alg(&[name("A"), name("B"), cmd("+", 2), name("C"), cmd("*", 2)]),
            "'(A+B)*C'"
        );
        assert_eq!(
            alg(&[name("A"), name("B"), name("C"), cmd("*", 2), cmd("+", 2)]),
            "'A+B*C'"
        );
        assert_eq!(
            alg(&[name("X"), cmd("SIN", 1), num("2"), cmd("^", 2)]),
            "'SIN(X)^2'"
        );
        assert_eq!(alg(&[name("A"), cmd("NEG", 1)]), "'-A'");
        assert_eq!(alg(&[name("A"), name("B"), cmd("MAX", 2)]), "'MAX(A,B)'");
        // Unknown argument count, or leftovers: no text.
        assert_eq!(algebraic(&[name("A"), name("B")], &s), None);
        assert_eq!(
            algebraic(
                &[
                    name("A"),
                    Element::Command {
                        name: None,
                        address: None,
                        xlib: None,
                        arity: None,
                        silent: false,
                    }
                ],
                &s
            ),
            None
        );
    }

    #[test]
    fn deep_expressions_render_without_recursion() {
        let s = Settings::default();
        let mut e = vec![name("A")];
        for _ in 0..200_000 {
            e.push(name("B"));
            e.push(cmd("-", 2));
        }
        let text = algebraic(&e, &s).unwrap();
        assert!(text.starts_with("'A-B-B"));
        assert!(text.ends_with(TRUNCATED));
    }
}
