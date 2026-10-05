//! Command names, read at run time from the ROM's own library tables.
//!
//! The ROM's decompiler names a command from the 6-nibble XLIB body
//! (library, command) that precedes every built-in command object, and
//! from the library's hash table; the link table maps command numbers to
//! objects (RPLMAN p. 13; layouts observed in the 48SX, 48GX and 49G ROMs:
//! wiki: protocols/rpl-libraries). Nothing ROM-derived is compiled in: a
//! [`NameTable`] is built from the ROM image the user loaded, once per ROM.
//!
//! What the table holds:
//!
//! - every library whose header, hash table and link table check out
//!   (found by scanning the image, so the 49G's flash banks need no map),
//!   with its command names and, where the command starts with one of the
//!   argument-checking dispatchers, its argument count;
//! - the five unit operator markers (`*`, `/`, `^`, prefix, end).
//!
//! Build cost: one pass over the image (512 K nibbles on the 48SX, 4 M on
//! the 49G) and a few thousand table reads; see the ROM-gated test.

use std::collections::HashMap;

use saturnus::Model;

use crate::charset;
use crate::object::Memory;
use crate::prolog::{ObjectType, SEMI, read_field};

/// Nibbles in a 49G flash bank: one window (#00000-#3FFFF or
/// #40000-#7FFFF) shows one bank (wiki: hardware/hp49g).
const BANK_NIBBLES: usize = 0x40000;
/// Library header: number (3), then offsets to the hash, message and link
/// tables and the configuration object (5 each).
const HEADER_NIBBLES: usize = 23;
/// An empty list, the unit operator markers: prolog #02A74 and SEMI.
const EMPTY_LIST: [u8; 10] = [4, 7, 0xA, 2, 0, 0xB, 2, 1, 3, 0];
/// Distance between the unit operator markers.
const MARKER_STRIDE: u32 = 10;
/// Most elements of a ROM unit object read when looking for the markers.
const MAX_UNIT_ELEMENTS: usize = 40;
/// Most common dispatch objects considered when looking for CK1-CK4.
const DISPATCH_CANDIDATES: usize = 12;

/// The 5-nibble field at `at`, or `None` past the end.
fn f5(n: &[u8], at: usize) -> Option<usize> {
    read_field(n, at, 5).map(|v| v as usize)
}

/// The target of the self-relative offset field at `at` (offsets wrap at
/// #100000 like addresses).
fn rel(n: &[u8], at: usize) -> Option<usize> {
    let off = f5(n, at)?;
    let target = if off >= 0x80000 {
        at.checked_sub(0x100000 - off)?
    } else {
        at.checked_add(off)?
    };
    (target < n.len()).then_some(target)
}

/// A hash table: where its number table starts, how many numbers it has,
/// where it ends.
#[derive(Clone, Copy, Debug)]
struct Hash {
    names_start: usize,
    numbers: usize,
    count: usize,
}

/// The hash table (hex string) at `t`, if its layout is valid.
fn hash_at(n: &[u8], t: usize) -> Option<Hash> {
    if f5(n, t)? != ObjectType::BinaryInteger.prolog() as usize {
        return None;
    }
    let len = f5(n, t + 5)?;
    let end = t.checked_add(5)?.checked_add(len)?;
    let body = t + 10;
    // 16 offsets by name length, the number table's offset, one name.
    if len < 5 + 85 || end > n.len() {
        return None;
    }
    for i in 0..16 {
        let at = body + 5 * i;
        if f5(n, at)? != 0 && rel(n, at)? >= end {
            return None;
        }
    }
    let numbers = rel(n, body + 80)?;
    if numbers < body + 85 || numbers > end || (end - numbers) % 5 != 0 {
        return None;
    }
    Some(Hash {
        names_start: body + 85,
        numbers,
        count: (end - numbers) / 5,
    })
}

/// The name of command `c` in `h`, if it has one.
fn name_in(n: &[u8], h: &Hash, c: usize) -> Option<String> {
    let at = h.numbers + 5 * c;
    let back = f5(n, at)?;
    if back == 0 {
        return None;
    }
    let entry = at.checked_sub(back)?;
    if entry < h.names_start {
        return None;
    }
    let len = read_field(n, entry, 2)? as usize;
    if len == 0 || entry + 2 + 2 * len + 3 > h.numbers {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..len)
        .map(|i| read_field(n, entry + 2 + 2 * i, 2).map(|b| b as u8))
        .collect();
    Some(charset::decode(&bytes?))
}

/// One library found in the ROM.
#[derive(Clone, Debug)]
struct Library {
    id: u16,
    /// Name by command number (shorter than `link` when the last commands
    /// have none).
    names: Vec<Option<String>>,
    /// Image index of each command's object (`usize::MAX`: outside).
    link: Vec<usize>,
    /// Argument count by command number, where the command starts with a
    /// dispatcher (filled after all libraries are found).
    arity: Vec<Option<u8>>,
}

/// A command as the name table knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandInfo<'a> {
    /// Library number.
    pub library: u16,
    /// Command number in the library.
    pub number: u16,
    /// Its name, if the library's hash table has one.
    pub name: Option<&'a str>,
    /// Its argument count, if it starts with an argument dispatcher
    /// (CK0-CK5).
    pub arity: Option<u8>,
    /// An unnamed command of the structure library (the one holding `«`),
    /// such as the word that quotes a program inside a program: the ROM's
    /// decompiler shows nothing for it.
    pub silent: bool,
}

/// The unit operators: ROM pointers to five empty lists (wiki:
/// protocols/rpl-libraries "Unit operators").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitMarkers {
    /// `*`.
    pub times: u32,
    /// `/`.
    pub divide: u32,
    /// `^`.
    pub power: u32,
    /// A prefix (the character before the unit name).
    pub prefix: u32,
    /// The end of the unit expression.
    pub end: u32,
}

/// What a built table holds, for reports and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NameStats {
    /// Libraries found.
    pub libraries: usize,
    /// Command numbers with a name.
    pub names: usize,
    /// Commands with a known argument count.
    pub arities: usize,
}

/// Command names and the other ROM facts the decompiler needs, built from
/// one ROM image.
#[derive(Clone, Default)]
pub struct NameTable {
    /// Libraries by number; a number can occur twice (a ROM can carry an
    /// older copy), each is tried.
    libraries: Vec<Library>,
    index: HashMap<u16, Vec<usize>>,
    /// Nibbles per bank when the image is banked (the 49G).
    bank: Option<usize>,
    units: Option<UnitMarkers>,
    /// The library holding `«` and the other structure words.
    structure: Option<u16>,
}

impl std::fmt::Debug for NameTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NameTable")
            .field("stats", &self.stats())
            .field("units", &self.units)
            .finish_non_exhaustive()
    }
}

impl NameTable {
    /// The table of `model`'s ROM image `rom` (one nibble per element, as
    /// [`saturnus::Machine::rom_nibbles`] gives it). A ROM without
    /// libraries gives an empty table (every command stays unnamed).
    pub fn build(model: Model, rom: &[u8]) -> NameTable {
        let bank = (model == Model::Hp49g).then_some(BANK_NIBBLES);
        let mut libraries = scan(rom, bank);
        let dispatch = Dispatch::find(rom, &libraries);
        for lib in &mut libraries {
            lib.arity = lib
                .link
                .iter()
                .map(|&t| dispatch.as_ref().and_then(|d| d.arity(rom, t)))
                .collect();
        }
        let mut index: HashMap<u16, Vec<usize>> = HashMap::new();
        for (i, lib) in libraries.iter().enumerate() {
            index.entry(lib.id).or_default().push(i);
        }
        let structure = libraries
            .iter()
            .find(|l| l.names.iter().any(|n| n.as_deref() == Some("«")))
            .map(|l| l.id);
        NameTable {
            libraries,
            index,
            bank,
            units: unit_markers(rom, bank),
            structure,
        }
    }

    /// The table of `machine`'s ROM.
    pub fn of(machine: &saturnus::Machine) -> NameTable {
        Self::build(machine.model(), machine.rom_nibbles())
    }

    /// Counts for reports.
    pub fn stats(&self) -> NameStats {
        let mut s = NameStats {
            libraries: self.libraries.len(),
            ..NameStats::default()
        };
        for lib in &self.libraries {
            s.names += lib.names.iter().flatten().count();
            s.arities += lib.arity.iter().flatten().count();
        }
        s
    }

    /// The heap memory the table holds, in bytes (approximate: vector
    /// and string capacities).
    pub fn heap_bytes(&self) -> usize {
        use std::mem::size_of;
        let libs: usize = self
            .libraries
            .iter()
            .map(|l| {
                l.names.capacity() * size_of::<Option<String>>()
                    + l.names
                        .iter()
                        .flatten()
                        .map(String::capacity)
                        .sum::<usize>()
                    + l.link.capacity() * size_of::<usize>()
                    + l.arity.capacity() * size_of::<Option<u8>>()
            })
            .sum();
        let index: usize = self
            .index
            .values()
            .map(|v| size_of::<(u16, Vec<usize>)>() + v.capacity() * size_of::<usize>())
            .sum();
        libs + self.libraries.capacity() * size_of::<Library>() + index
    }

    /// Every named command: (library, number, name).
    pub fn names(&self) -> impl Iterator<Item = (u16, u16, &str)> + '_ {
        self.libraries.iter().flat_map(|lib| {
            lib.names
                .iter()
                .enumerate()
                .filter_map(move |(c, n)| Some((lib.id, u16::try_from(c).ok()?, n.as_deref()?)))
        })
    }

    /// The unit operator markers, if the ROM's unit table revealed them.
    pub fn unit_markers(&self) -> Option<UnitMarkers> {
        self.units
    }

    fn info<'s>(&'s self, lib: &'s Library, c: usize) -> Option<CommandInfo<'s>> {
        let name = lib.names.get(c).and_then(|n| n.as_deref());
        Some(CommandInfo {
            library: lib.id,
            number: u16::try_from(c).ok()?,
            name,
            arity: lib.arity.get(c).copied().flatten(),
            silent: name.is_none() && Some(lib.id) == self.structure,
        })
    }

    /// The command XLIB `library` `number` (an XLIB name object).
    pub fn xlib(&self, library: u16, number: u16) -> Option<CommandInfo<'_>> {
        let c = usize::from(number);
        self.index.get(&library)?.iter().find_map(|&i| {
            let lib = &self.libraries[i];
            (c < lib.link.len()).then(|| self.info(lib, c)).flatten()
        })
    }

    /// The command whose object is at `addr` in `mem` (a ROM pointer), as
    /// the ROM's decompiler finds it: the XLIB body in the six nibbles
    /// before the object names a library and command whose link table
    /// entry is this object.
    pub fn command_at(&self, addr: u32, mem: &dyn Memory) -> Option<CommandInfo<'_>> {
        let start = addr.checked_sub(6)?;
        let mut prefix = [0u8; 6];
        for (i, p) in prefix.iter_mut().enumerate() {
            *p = mem.nibble(start + i as u32)?;
        }
        let library = read_field(&prefix, 0, 3)? as u16;
        let c = read_field(&prefix, 3, 3)? as usize;
        let a = addr as usize;
        self.index.get(&library)?.iter().find_map(|&i| {
            let lib = &self.libraries[i];
            let t = *lib.link.get(c)?;
            let same = match self.bank {
                None => t == a,
                // The CPU sees a bank through one of two windows.
                Some(b) => a < 2 * b && t % b == a % b,
            };
            if same { self.info(lib, c) } else { None }
        })
    }
}

/// Every library header in `rom` whose hash and link tables check out.
fn scan(rom: &[u8], bank: Option<usize>) -> Vec<Library> {
    let hex = ObjectType::BinaryInteger.prolog() as usize;
    let sysbin = ObjectType::SystemBinary.prolog() as usize;
    let ext = ObjectType::ExtendedPointer.prolog() as usize;
    let mut out = Vec::new();
    let last = rom.len().saturating_sub(HEADER_NIBBLES);
    for p in 0..last {
        // Cheap rejections first: both offsets set, the link table a hex
        // string, the hash table a hex string or an indirection.
        if f5(rom, p + 3) == Some(0) || f5(rom, p + 13) == Some(0) {
            continue;
        }
        let Some(tl) = rel(rom, p + 13) else { continue };
        if f5(rom, tl) != Some(hex) {
            continue;
        }
        let Some(th) = rel(rom, p + 3) else { continue };
        let Some(ph) = f5(rom, th) else { continue };
        if ph != hex && ph != sysbin && ph != ext {
            continue;
        }
        let Some(len) = f5(rom, tl + 5) else { continue };
        if len < 5 || (len - 5) % 5 != 0 || tl + 5 + len > rom.len() {
            continue;
        }
        let count = (len - 5) / 5;
        let hash = if ph == hex {
            hash_at(rom, th)
        } else {
            // An indirection directly followed by the link table.
            let size = if ph == sysbin { 10 } else { 15 };
            if tl != th + size {
                continue;
            }
            let Some(a) = f5(rom, th + 5) else { continue };
            let mut found = candidates(a, bank, rom.len()).filter_map(|t| hash_at(rom, t));
            match (found.next(), found.next()) {
                (Some(h), None) => Some(h),
                _ => None,
            }
        };
        let Some(hash) = hash.filter(|h| h.count <= count) else {
            continue;
        };
        let link = (0..count)
            .map(|c| rel(rom, tl + 10 + 5 * c).unwrap_or(usize::MAX))
            .collect();
        let names = (0..hash.count).map(|c| name_in(rom, &hash, c)).collect();
        out.push(Library {
            id: read_field(rom, p, 3).unwrap_or(0) as u16,
            names,
            link,
            arity: Vec::new(),
        });
    }
    out
}

/// Image indexes that CPU address `a` can stand for: itself, or on a
/// banked ROM the same offset in every bank.
fn candidates(a: usize, bank: Option<usize>, len: usize) -> Box<dyn Iterator<Item = usize>> {
    match bank {
        None => Box::new((a < len).then_some(a).into_iter()),
        Some(b) => Box::new((0..len / b).map(move |k| k * b + a % b)),
    }
}

/// The argument dispatchers CK0-CK5 as ROM addresses.
#[derive(Clone, Copy, Debug)]
struct Dispatch {
    ck0: Option<usize>,
    /// CK1&Dispatch; CKn is `ck1 + (n - 1) * stride`.
    ck1: usize,
    stride: usize,
}

impl Dispatch {
    /// The dispatchers, from the library that holds `+`: its commands'
    /// first objects include four evenly spaced ones (CK1-CK4), `+`
    /// starts with CK2, and the most common other one is CK0 (wiki:
    /// protocols/rpl-libraries "Dispatchers").
    fn find(rom: &[u8], libraries: &[Library]) -> Option<Dispatch> {
        let (lib, plus) = libraries.iter().find_map(|l| {
            let c = l.names.iter().position(|n| n.as_deref() == Some("+"))?;
            Some((l, c))
        })?;
        let mut counts: HashMap<usize, usize> = HashMap::new();
        for &t in &lib.link {
            if let Some(p) = first_object(rom, t) {
                *counts.entry(p).or_default() += 1;
            }
        }
        let ck2 = first_object(rom, *lib.link.get(plus)?)?;
        let mut common: Vec<(usize, usize)> = counts.into_iter().collect();
        common.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        common.truncate(DISPATCH_CANDIDATES);
        let has = |v: usize| common.iter().any(|&(p, _)| p == v);
        // CK1 and CK3 around CK2, CK4 after: the smallest spacing that fits.
        let stride = common
            .iter()
            .filter_map(|&(p, _)| p.checked_sub(ck2).filter(|s| *s > 0))
            .filter(|&s| ck2 >= s && has(ck2 - s) && has(ck2 + 2 * s))
            .min()?;
        let ck1 = ck2 - stride;
        let cks: Vec<usize> = (0..5).map(|k| ck1 + k * stride).collect();
        let ck0 = common.iter().map(|&(p, _)| p).find(|p| !cks.contains(p));
        Some(Dispatch { ck0, ck1, stride })
    }

    /// The argument count of the command whose object is at image index
    /// `t`.
    fn arity(&self, rom: &[u8], t: usize) -> Option<u8> {
        let p = first_object(rom, t)?;
        if Some(p) == self.ck0 {
            return Some(0);
        }
        let k = p.checked_sub(self.ck1)?;
        if k % self.stride != 0 || k / self.stride > 4 {
            return None;
        }
        u8::try_from(k / self.stride + 1).ok()
    }
}

/// The first element of the program at image index `t`, if it is one.
fn first_object(rom: &[u8], t: usize) -> Option<usize> {
    (f5(rom, t)? == ObjectType::Program.prolog() as usize).then(|| f5(rom, t + 5))?
}

/// The unit operator markers: the most common last element of the ROM's
/// own unit objects is the end marker, the other four precede it.
fn unit_markers(rom: &[u8], bank: Option<usize>) -> Option<UnitMarkers> {
    let unit = ObjectType::Unit.prolog() as usize;
    let mut ends: HashMap<usize, usize> = HashMap::new();
    for p in 0..rom.len().saturating_sub(5) {
        if f5(rom, p) != Some(unit) {
            continue;
        }
        if let Some(e) = unit_end(rom, p + 5) {
            *ends.entry(e).or_default() += 1;
        }
    }
    let (&end, _) = ends.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))?;
    let end = u32::try_from(end).ok()?;
    let first = end.checked_sub(4 * MARKER_STRIDE)? as usize;
    // All five are empty lists, side by side, in one bank.
    let is_block = |at: usize| {
        (0..5).all(|k| {
            let s = at + k * MARKER_STRIDE as usize;
            rom.get(s..s + 10) == Some(&EMPTY_LIST[..])
        })
    };
    if !candidates(first, bank, rom.len()).any(is_block) {
        return None;
    }
    let m = |k: u32| end - (4 - k) * MARKER_STRIDE;
    Some(UnitMarkers {
        times: m(0),
        divide: m(1),
        power: m(2),
        prefix: m(3),
        end,
    })
}

/// The last element (a ROM pointer) of the unit body at `at`, if the body
/// holds only reals, strings, characters and pointers.
fn unit_end(rom: &[u8], mut at: usize) -> Option<usize> {
    let real = ObjectType::Real.prolog() as usize;
    let string = ObjectType::String.prolog() as usize;
    let chr = ObjectType::Character.prolog() as usize;
    let mut last = None;
    for _ in 0..MAX_UNIT_ELEMENTS {
        let p = f5(rom, at)?;
        if p == SEMI as usize {
            return last;
        }
        if p == real {
            at += 21;
        } else if p == string {
            at += 5 + f5(rom, at + 5)?;
        } else if p == chr {
            at += 7;
        } else if (0x02600..=0x02FFF).contains(&p) {
            return None;
        } else {
            last = Some(p);
            at += 5;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(out: &mut Vec<u8>, at: usize, value: usize, width: usize) {
        if out.len() < at + width {
            out.resize(at + width, 0);
        }
        for i in 0..width {
            out[at + i] = ((value >> (4 * i)) & 0xF) as u8;
        }
    }

    fn put_rel(out: &mut Vec<u8>, at: usize, target: usize) {
        let off = (target + 0x100000 - at) & 0xFFFFF;
        put(out, at, off, 5);
    }

    /// A ROM of `size` nibbles with library `id` at #100: names `names`
    /// (by command number), the commands' objects (programs starting with
    /// the pointer in `first`) each preceded by their XLIB body.
    fn synthetic(id: usize, names: &[Option<&str>], first: &[usize]) -> Vec<u8> {
        let mut r = vec![0u8; 0x4000];
        let header = 0x100;
        let hash = 0x200;
        let objects = 0x1000;
        // Hash table: 16 length offsets, number table offset, names, numbers.
        let body = hash + 10;
        let mut at = body + 85;
        let mut entries = vec![0usize; names.len()];
        let mut first_of_len = [0usize; 16];
        for len in 1..=16 {
            for (c, n) in names.iter().enumerate() {
                let Some(n) = n else { continue };
                // HP characters: « and » are bytes #AB and #BB.
                let bytes: Vec<u8> = n.chars().map(|c| c as u32 as u8).collect();
                if bytes.len() != len {
                    continue;
                }
                if first_of_len[len - 1] == 0 {
                    first_of_len[len - 1] = at;
                }
                entries[c] = at;
                put(&mut r, at, len, 2);
                for (i, b) in bytes.iter().enumerate() {
                    put(&mut r, at + 2 + 2 * i, usize::from(*b), 2);
                }
                put(&mut r, at + 2 + 2 * len, c, 3);
                at += 2 + 2 * len + 3;
            }
        }
        for (i, &e) in first_of_len.iter().enumerate() {
            if e != 0 {
                put_rel(&mut r, body + 5 * i, e);
            }
        }
        let numbers = at;
        put_rel(&mut r, body + 80, numbers);
        for (c, &e) in entries.iter().enumerate() {
            let field = numbers + 5 * c;
            put(&mut r, field, if e == 0 { 0 } else { field - e }, 5);
        }
        let end = numbers + 5 * names.len();
        put(&mut r, hash, 0x02A4E, 5);
        put(&mut r, hash + 5, end - hash - 5, 5);
        // Link table right after the hash table.
        let link = end;
        put(&mut r, link, 0x02A4E, 5);
        put(&mut r, link + 5, 5 + 5 * names.len(), 5);
        for c in 0..names.len() {
            let obj = objects + 0x40 * c;
            put(&mut r, obj - 6, id, 3);
            put(&mut r, obj - 3, c, 3);
            put(&mut r, obj, 0x02D9D, 5);
            put(&mut r, obj + 5, first.get(c).copied().unwrap_or(0x3000), 5);
            put(&mut r, obj + 10, SEMI as usize, 5);
            put_rel(&mut r, link + 10 + 5 * c, obj);
        }
        put(&mut r, header, id, 3);
        put_rel(&mut r, header + 3, hash);
        put_rel(&mut r, header + 13, link);
        r
    }

    struct Image<'a>(&'a [u8]);
    impl Memory for Image<'_> {
        fn nibble(&self, addr: u32) -> Option<u8> {
            self.0.get(addr as usize).copied()
        }
    }

    #[test]
    fn names_resolve_by_number_and_by_address() {
        // + is binary through "CK2" #2005; the others share "CK1" #2000,
        // "CK3" #200A and "CK4" #200F appear once each.
        let names = [
            Some("SIN"),
            Some("+"),
            None,
            Some("+"),
            Some("IFTE"),
            Some("INTG"),
            Some("COS"),
        ];
        let first = [0x2000, 0x2005, 0x2000, 0x2005, 0x200A, 0x200F, 0x2000];
        let rom = synthetic(0x2, &names, &first);
        let t = NameTable::build(Model::Hp48sx, &rom);
        assert_eq!(
            t.stats(),
            NameStats {
                libraries: 1,
                names: 6,
                arities: 7
            }
        );
        let sin = t.xlib(2, 0).unwrap();
        assert_eq!((sin.name, sin.arity), (Some("SIN"), Some(1)));
        assert_eq!(t.xlib(2, 3).unwrap().name, Some("+"));
        assert_eq!(t.xlib(2, 2).unwrap().name, None);
        assert_eq!(t.xlib(2, 4).unwrap().arity, Some(3));
        assert_eq!(t.xlib(2, 5).unwrap().arity, Some(4));
        assert_eq!(t.xlib(2, 7), None);
        assert_eq!(t.xlib(3, 0), None);
        let mem = Image(&rom);
        let plus = t.command_at(0x1040, &mem).unwrap();
        assert_eq!(
            (plus.number, plus.name, plus.arity),
            (1, Some("+"), Some(2))
        );
        // Not a command: no XLIB body before it, or one that lies.
        assert_eq!(t.command_at(0x1041, &mem), None);
        let mut liar = rom.clone();
        put(&mut liar, 0x1040 - 3, 2, 3);
        assert_eq!(t.command_at(0x1040, &Image(&liar)), None);
        let all: Vec<&str> = t.names().map(|(_, _, n)| n).collect();
        assert_eq!(all, ["SIN", "+", "+", "IFTE", "INTG", "COS"]);
    }

    /// The synthetic library's ROM with RAM objects at #3000.
    fn with_ram(objects: &[(usize, &str)]) -> (Vec<u8>, NameTable) {
        let names = [
            Some("SIN"),
            Some("+"),
            Some("«"),
            Some("»"),
            None,
            Some("IFTE"),
            Some("INTG"),
        ];
        let first = [0x2000, 0x2005, 0x2000, 0x2000, 0x2000, 0x200A, 0x200F];
        let mut rom = synthetic(0x2, &names, &first);
        let t = NameTable::build(Model::Hp48sx, &rom);
        for (at, hex) in objects {
            for (i, c) in hex.chars().enumerate() {
                let v = c.to_digit(16).unwrap() as usize;
                put(&mut rom, at + i, v, 1);
            }
        }
        (rom, t)
    }

    fn ptr(a: usize) -> String {
        (0..5)
            .map(|i| format!("{:X}", (a >> (4 * i)) & 0xF))
            .collect()
    }

    #[test]
    fn decoding_with_names() {
        use crate::decompile::Settings;
        use crate::object::{Object, Reader};
        // { 1 SIN }, « 1 SIN » with « and » as commands, 'A+1'.
        let one = "339200000000000000010";
        let list = format!("47A20{one}{}B2130", ptr(0x1000));
        let prog = format!(
            "D9D20{}{one}{}{}B2130",
            ptr(0x1080),
            ptr(0x1000),
            ptr(0x10C0)
        );
        let alg = format!("8BA2084E201014{one}{}B2130", ptr(0x1040));
        let (rom, t) = with_ram(&[(0x3000, &list), (0x3100, &prog), (0x3200, &alg)]);
        let mem = Image(&rom);
        let r = Reader::with_names(&mem, &t, Settings::default());
        let json = |a: u32| serde_json::to_value(r.decode_at(a).unwrap()).unwrap();
        assert_eq!(
            json(0x3000),
            serde_json::json!({"type": "list", "items": [
                {"type": "real", "value": 1.0},
                {"type": "command", "name": "SIN", "address": 4096}]})
        );
        assert_eq!(
            r.decode_at(0x3100).unwrap(),
            Object::Program {
                source: Some("« 1 SIN »".into())
            }
        );
        assert_eq!(
            r.decode_at(0x3200).unwrap(),
            Object::Algebraic {
                source: Some("'A+1'".into())
            }
        );
        // A stack level that is a command's object is that command.
        assert_eq!(
            json(0x1000),
            serde_json::json!({"type": "command", "name": "SIN", "address": 4096})
        );
        // An unnamed command of the structure library would be silent; this
        // one is in the same library as « so it is.
        assert!(t.command_at(0x1100, &mem).unwrap().silent);
        // Without names: the pointer is still a command, not a program.
        let plain = Reader::new(&mem);
        assert_eq!(
            serde_json::to_value(plain.decode_at(0x3000).unwrap()).unwrap()["items"][1],
            serde_json::json!({"type": "command", "address": 4096})
        );
        assert_eq!(
            plain.decode_at(0x3100).unwrap(),
            Object::Program { source: None }
        );
    }

    #[test]
    fn garbage_ram_gives_errors_or_unknowns() {
        use crate::decompile::Settings;
        use crate::object::{Object, Reader};
        // A list holding a pointer to itself: bounded by the nesting limit.
        let cycle = format!("47A20{}B2130", ptr(0x3000));
        // An algebraic whose elements do not form an expression, one whose
        // operator lacks operands, a unit without its operators' operands.
        let loose = "8BA2084E2010184E2010288E20B2130";
        let short = format!("8BA20{}B2130", ptr(0x1040));
        // A program pointing at itself: a ROM-style command, not followed.
        let selfp = format!("D9D20{}B2130", ptr(0x3400));
        let (rom, t) = with_ram(&[
            (0x3000, &cycle),
            (0x3100, loose),
            (0x3200, &short),
            (0x3400, &selfp),
        ]);
        let mem = Image(&rom);
        let r = Reader::with_names(&mem, &t, Settings::default());
        assert!(r.decode_at(0x3000).is_err());
        assert!(matches!(
            r.decode_at(0x3100),
            Ok(Object::Algebraic { source: None }) | Err(_)
        ));
        assert_eq!(
            r.decode_at(0x3200).unwrap(),
            Object::Algebraic { source: None }
        );
        assert_eq!(
            r.decode_at(0x3400).unwrap(),
            Object::Program {
                source: Some("External".into())
            }
        );
    }

    #[test]
    fn garbage_gives_an_empty_table() {
        let mut rom = vec![0u8; 0x10000];
        for (i, n) in rom.iter_mut().enumerate() {
            *n = ((i * 7919) >> 3) as u8 & 0xF;
        }
        let t = NameTable::build(Model::Hp49g, &rom);
        assert_eq!(t.stats().names, 0);
        assert_eq!(t.unit_markers(), None);
        assert_eq!(t.command_at(5, &Image(&rom)), None);
        assert_eq!(NameTable::build(Model::Hp48sx, &[]).stats().libraries, 0);
    }

    #[test]
    fn unit_markers_from_the_rom_units() {
        let mut rom = vec![0u8; 0x2000];
        let block = 0x400;
        for k in 0..5 {
            rom[block + 10 * k..block + 10 * k + 10].copy_from_slice(&EMPTY_LIST);
        }
        let end = block + 40;
        // Three units 1_m ending in the end marker, one ending elsewhere.
        for (u, last) in [(0x800, end), (0x900, end), (0xA00, end), (0xB00, 0x1234)] {
            put(&mut rom, u, 0x02ADA, 5);
            put(&mut rom, u + 5, 0x2A2C9, 5);
            put(&mut rom, u + 10, 0x02A2C, 5);
            put(&mut rom, u + 15, 7, 5);
            put(&mut rom, u + 20, 0x6D, 2);
            put(&mut rom, u + 22, last, 5);
            put(&mut rom, u + 27, SEMI as usize, 5);
        }
        let m = NameTable::build(Model::Hp48sx, &rom)
            .unit_markers()
            .unwrap();
        assert_eq!(
            (m.times, m.divide, m.power, m.prefix, m.end),
            (0x400, 0x40A, 0x414, 0x41E, 0x428)
        );
    }
}
