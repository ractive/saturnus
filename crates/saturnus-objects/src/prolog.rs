//! Object types by prolog and the size walk over nibbles.
//!
//! Prologs, type names and the rules for finding an object's end: wiki:
//! protocols/hp-object-format (RPLMAN chapter 3; the 49G types from the
//! Conn4x sources; DOEXT0-4 from the 48 entry list). The names are those
//! the calculator's `G D` directory listing prints.

use anyhow::{Context, Result, bail};

/// SEMI, the end marker of composites (lists, programs, algebraics, units).
pub const SEMI: u32 = 0x0312B;
/// Deepest nesting the walks follow.
pub const MAX_DEPTH: usize = 64;
/// Prologs live in this ROM range on the 48 and the 49G; an embedded value
/// in it that is no known prolog is an error, not a ROM pointer.
const PROLOG_RANGE: std::ops::RangeInclusive<u32> = 0x02600..=0x02FFF;

/// Object types by prolog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectType {
    /// #02911.
    SystemBinary,
    /// #02933.
    Real,
    /// #02955.
    LongReal,
    /// #02977.
    Complex,
    /// #0299D.
    LongComplex,
    /// #029BF.
    Character,
    /// #029E8.
    Array,
    /// #02A0A.
    LinkedArray,
    /// #02A2C.
    String,
    /// #02A4E.
    BinaryInteger,
    /// #02A74.
    List,
    /// #02A96.
    Directory,
    /// #02AB8.
    Algebraic,
    /// #02ADA.
    Unit,
    /// #02AFC.
    Tagged,
    /// #02B1E.
    Graphic,
    /// #02B40.
    Library,
    /// #02B62.
    Backup,
    /// #02B88.
    LibraryData,
    /// #02D9D.
    Program,
    /// #02DCC.
    Code,
    /// #02E48.
    GlobalName,
    /// #02E6D.
    LocalName,
    /// #02E92.
    XlibName,
    /// #02614, 49G.
    Integer,
    /// #02686, 49G symbolic matrix.
    SymbolicMatrix,
    /// #0263A, 49G long real.
    LongReal49,
    /// #02660, 49G long complex.
    LongComplex49,
    /// #026AC, 49G flash pointer.
    FlashPointer,
    /// #026D5, 49G aplet.
    Aplet,
    /// #026FE, 49G mini font.
    MiniFont,
    /// #02BAA, extended (access) pointer.
    ExtendedPointer,
    /// #02BCC.
    Extended2,
    /// #02BEE.
    Extended3,
    /// #02C10.
    Extended4,
}

/// How an object's end is found.
#[derive(Clone, Copy, Debug)]
enum Extent {
    /// A fixed size in nibbles, prolog included.
    Fixed(usize),
    /// `n` consecutive 5-nibble length fields after the prolog, each
    /// counting itself and its body (most objects: 1).
    Lengths(usize),
    /// Elements up to and including SEMI.
    Composite,
    /// 2-nibble character count, characters, one element.
    Tagged,
    /// 2-nibble character count, characters.
    Name,
    /// Library id, last-variable offset, variable records.
    Directory,
}

use Extent::{Composite, Fixed, Lengths, Name};

/// (type, prolog, `G D` name, extent), in [`ObjectType`] order.
const TYPES: [(ObjectType, u32, &str, Extent); 35] = [
    (
        ObjectType::SystemBinary,
        0x02911,
        "System Binary",
        Fixed(10),
    ),
    (ObjectType::Real, 0x02933, "Real Number", Fixed(21)),
    (ObjectType::LongReal, 0x02955, "Long Real", Fixed(26)),
    (ObjectType::Complex, 0x02977, "Complex Number", Fixed(37)),
    (ObjectType::LongComplex, 0x0299D, "Long Complex", Fixed(47)),
    (ObjectType::Character, 0x029BF, "Character", Fixed(7)),
    (ObjectType::Array, 0x029E8, "Array", Lengths(1)),
    (ObjectType::LinkedArray, 0x02A0A, "Linked Array", Lengths(1)),
    (ObjectType::String, 0x02A2C, "String", Lengths(1)),
    (
        ObjectType::BinaryInteger,
        0x02A4E,
        "Binary Integer",
        Lengths(1),
    ),
    (ObjectType::List, 0x02A74, "List", Composite),
    (
        ObjectType::Directory,
        0x02A96,
        "Directory",
        Extent::Directory,
    ),
    (ObjectType::Algebraic, 0x02AB8, "Algebraic", Composite),
    (ObjectType::Unit, 0x02ADA, "Unit", Composite),
    (ObjectType::Tagged, 0x02AFC, "Tagged", Extent::Tagged),
    (ObjectType::Graphic, 0x02B1E, "Graphic", Lengths(1)),
    (ObjectType::Library, 0x02B40, "Library", Lengths(1)),
    (ObjectType::Backup, 0x02B62, "Backup", Lengths(1)),
    (ObjectType::LibraryData, 0x02B88, "Library Data", Lengths(1)),
    (ObjectType::Program, 0x02D9D, "Program", Composite),
    (ObjectType::Code, 0x02DCC, "Code", Lengths(1)),
    (ObjectType::GlobalName, 0x02E48, "Global Name", Name),
    (ObjectType::LocalName, 0x02E6D, "Local Name", Name),
    (ObjectType::XlibName, 0x02E92, "XLIB Name", Fixed(11)),
    (ObjectType::Integer, 0x02614, "Integer", Lengths(1)),
    (
        ObjectType::SymbolicMatrix,
        0x02686,
        "Symbolic Matrix",
        Composite,
    ),
    // Mantissa and exponent, each a length-prefixed integer body.
    (
        ObjectType::LongReal49,
        0x0263A,
        "Long Real (49G)",
        Lengths(2),
    ),
    // Two long reals without their prologs.
    (
        ObjectType::LongComplex49,
        0x02660,
        "Long Complex (49G)",
        Lengths(4),
    ),
    // Prolog, 3-nibble bank, 4-nibble address.
    (
        ObjectType::FlashPointer,
        0x026AC,
        "Flash Pointer",
        Fixed(12),
    ),
    // Length-prefixed per the Conn4x sources; not checked on a real object.
    (ObjectType::Aplet, 0x026D5, "Aplet", Lengths(1)),
    (ObjectType::MiniFont, 0x026FE, "Mini Font", Lengths(1)),
    // Prolog and two 5-nibble fields.
    (
        ObjectType::ExtendedPointer,
        0x02BAA,
        "Extended Pointer",
        Fixed(15),
    ),
    (ObjectType::Extended2, 0x02BCC, "Extended 2", Lengths(1)),
    (ObjectType::Extended3, 0x02BEE, "Extended 3", Lengths(1)),
    (ObjectType::Extended4, 0x02C10, "Extended 4", Lengths(1)),
];

impl ObjectType {
    /// The type with this prolog, if known.
    pub fn from_prolog(prolog: u32) -> Option<Self> {
        TYPES.iter().find(|t| t.1 == prolog).map(|t| t.0)
    }

    fn entry(self) -> &'static (ObjectType, u32, &'static str, Extent) {
        // TYPES lists every variant once, in declaration order.
        &TYPES[self as usize]
    }

    /// The prolog address.
    pub fn prolog(self) -> u32 {
        self.entry().1
    }

    /// The type name as `G D` prints it, e.g. `Real Number`.
    pub fn name(self) -> &'static str {
        self.entry().2
    }
}

/// The `width`-nibble field at `at`, low nibble first; `None` past the end
/// or for fields wider than 8 nibbles.
pub fn read_field(n: &[u8], at: usize, width: usize) -> Option<u32> {
    if width > 8 {
        return None;
    }
    let f = n.get(at..at.checked_add(width)?)?;
    Some(
        f.iter()
            .rev()
            .fold(0, |acc, &d| (acc << 4) | u32::from(d & 0xF)),
    )
}

fn field(n: &[u8], at: usize, width: usize) -> Result<usize> {
    let v = read_field(n, at, width).with_context(|| format!("object truncated at nibble {at}"))?;
    Ok(usize::try_from(v)?)
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).context("object size overflows")
}

/// Size in nibbles of the object at nibble `at`. Fails on an unknown
/// prolog, a truncated object or nesting deeper than [`MAX_DEPTH`].
pub fn object_size(n: &[u8], at: usize) -> Result<usize> {
    walk(n, at, 0)
}

/// Size of an embedded object (composite element, tagged payload,
/// directory variable): an object with a known prolog, else a 5-nibble ROM
/// pointer.
pub fn element_size(n: &[u8], at: usize, depth: usize) -> Result<usize> {
    let p = field(n, at, 5)? as u32;
    if ObjectType::from_prolog(p).is_some() {
        walk(n, at, depth)
    } else if PROLOG_RANGE.contains(&p) {
        bail!("unknown prolog #{p:05X} at nibble {at}")
    } else {
        Ok(5)
    }
}

fn walk(n: &[u8], at: usize, depth: usize) -> Result<usize> {
    if depth > MAX_DEPTH {
        bail!("objects nested deeper than {MAX_DEPTH} levels");
    }
    let prolog = field(n, at, 5)? as u32;
    let ty = ObjectType::from_prolog(prolog)
        .with_context(|| format!("unknown prolog #{prolog:05X} at nibble {at}"))?;
    let body = add(at, 5)?;
    let size = match ty.entry().3 {
        Fixed(size) => size,
        Lengths(count) => {
            let mut pos = body;
            for _ in 0..count {
                let len = field(n, pos, 5)?;
                if len < 5 {
                    bail!("length field #{len:05X} at nibble {pos} is shorter than itself");
                }
                pos = add(pos, len)?;
            }
            pos - at
        }
        Composite => {
            let mut pos = body;
            while field(n, pos, 5)? != SEMI as usize {
                pos = add(pos, element_size(n, pos, depth + 1)?)?;
            }
            add(pos, 5)? - at
        }
        Extent::Tagged => {
            let inner = add(body, 2 + 2 * field(n, body, 2)?)?;
            add(inner, element_size(n, inner, depth + 1)?)? - at
        }
        Name => 7 + 2 * field(n, body, 2)?,
        Extent::Directory => directory_size(n, at, depth)?,
    };
    if add(at, size)? > n.len() {
        bail!(
            "{} at nibble {at} needs {size} nibbles, only {} left",
            ty.name(),
            n.len().saturating_sub(at)
        );
    }
    Ok(size)
}

/// One variable record of a directory body (wiki: hardware/hp48-system-ram
/// "Directories"): a 5-nibble offset from this field back to the previous
/// record's name length field (0 for the first record), the name's length
/// n in 2 nibbles, its characters, n again (absent when n = 0), the
/// object. Records lie oldest first; the directory's offset field points
/// at the newest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    /// Nibble of the back-offset field (the record's start).
    pub start: usize,
    /// Nibble of the first name length field.
    pub name_at: usize,
    /// Name length in characters.
    pub name_len: usize,
    /// Nibble of the object (or the 5-nibble ROM pointer).
    pub object: usize,
    /// Size of the object in nibbles.
    pub object_size: usize,
}

impl Record {
    /// The record at `start`.
    pub fn at(n: &[u8], start: usize, depth: usize) -> Result<Record> {
        let name_at = add(start, 5)?;
        let name_len = field(n, name_at, 2)?;
        let mut object = add(name_at, 2 + 2 * name_len)?;
        if name_len != 0 {
            object = add(object, 2)?;
        }
        let object_size = element_size(n, object, depth + 1)?;
        Ok(Record {
            start,
            name_at,
            name_len,
            object,
            object_size,
        })
    }

    /// The nibble after the record.
    pub fn end(&self) -> usize {
        self.object + self.object_size
    }
}

/// The records of the directory body whose last-variable offset field is
/// at `offset_at` (records follow it), in memory order (oldest first).
pub fn records(n: &[u8], offset_at: usize, depth: usize) -> Result<Vec<Record>> {
    let offset = field(n, offset_at, 5)?;
    let mut out = Vec::new();
    if offset == 0 {
        return Ok(out);
    }
    let last = add(offset_at, offset)?;
    let mut pos = add(offset_at, 5)?;
    loop {
        let r = Record::at(n, pos, depth)?;
        if r.name_at > last {
            bail!("directory at nibble {offset_at}: the last-variable offset matches no record");
        }
        pos = r.end();
        let done = r.name_at == last;
        out.push(r);
        if done {
            return Ok(out);
        }
    }
}

/// An ordinary directory: prolog, 3-nibble attached library (#7FF none),
/// 5-nibble offset from that field to the last record's name length (0 =
/// empty), then the records.
fn directory_size(n: &[u8], at: usize, depth: usize) -> Result<usize> {
    let offset_at = add(at, 8)?;
    let end = records(n, offset_at, depth)?
        .last()
        .map_or(offset_at + 5, Record::end);
    Ok(end - at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nib(s: &str) -> Vec<u8> {
        s.chars()
            .map(|c| c.to_digit(16).unwrap_or(0) as u8)
            .collect()
    }

    #[test]
    fn table_is_in_declaration_order() {
        for (i, t) in TYPES.iter().enumerate() {
            assert_eq!(t.0 as usize, i, "{:?}", t.0);
            assert_eq!(ObjectType::from_prolog(t.1), Some(t.0));
        }
    }

    #[test]
    fn sizes() {
        assert_eq!(object_size(&nib("339200000000000000050"), 0).ok(), Some(21));
        // { 1 "A" } with 1 as a ROM pointer.
        assert_eq!(
            object_size(&nib("47A209C2A2C2A207000014B2130"), 0).ok(),
            Some(27)
        );
        assert!(object_size(&nib("3392000"), 0).is_err());
        assert!(object_size(&nib("C2A2030000"), 0).is_err());
        // A directory holding Z = 5. (48SX RAM, wiki: hardware/hp48-system-ram).
        let d = nib("69A20FF7A00000000010A510339200000000000000050");
        assert_eq!(object_size(&d, 0).ok(), Some(45));
        let rs = records(&d, 8, 0).unwrap_or_default();
        assert_eq!(rs.len(), 1);
        assert_eq!(
            (rs[0].name_len, rs[0].object, rs[0].object_size),
            (1, 24, 21)
        );
        // An empty directory.
        assert_eq!(object_size(&nib("69A20FF700000"), 0).ok(), Some(13));
    }
}
