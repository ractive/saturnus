//! A read-only view of a paused calculator's user memory: the HOME
//! directory tree, the current directory, the data stack and the flags,
//! read straight from RAM through the ROM's own system pointers. Nothing
//! is written and the calculator does not change mode, so a host can show
//! it at any time; the values are those the ROM left when it last went
//! idle (in its outer loop, the stack showing), which is where a paused
//! machine normally sits.
//!
//! Locations per model (48SX, 48GX, 49G) and the layouts: wiki:
//! hardware/hp48-system-ram. The 38G, 39G and 40G keep aplets, not a HOME
//! tree, and the 42S has no RPL user memory (its RAM holds HP-41-style
//! programs and registers; wiki: hardware/hp42s); neither is covered.

use saturnus::{Machine, Model};
use serde::Serialize;

use anyhow::{Context, Result, bail, ensure};

use crate::object::{Base, Memory, Object, Reader};
use crate::prolog::{ObjectType, Record, records};

/// Most stack levels read (the 49G's 256 KB of RAM holds about 50 000
/// pointers; more means the pointers are not the ROM's).
const MAX_LEVELS: usize = 100_000;
/// Largest region read in one piece, in nibbles (the whole address space).
const MAX_REGION: u32 = 1 << 20;

impl Memory for Machine {
    fn nibble(&self, addr: u32) -> Option<u8> {
        (addr < MAX_REGION).then(|| self.peek(addr))
    }
}

/// Where a model's ROM keeps its system pointers and flags in RAM (wiki:
/// hardware/hp48-system-ram). Each `*_ptr` field is the address of a
/// 5-nibble pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The HOME directory object.
    pub home_ptr: u32,
    /// The nibble after HOME.
    pub home_end_ptr: u32,
    /// The current (context) directory object.
    pub context_ptr: u32,
    /// The saved data stack pointer (D1): level 1's entry.
    pub stack_ptr: u32,
    /// The nibble after the data stack's 0 end marker.
    pub stack_end_ptr: u32,
    /// The system flags -1, -2, ... (bit 0 of the first nibble is -1).
    pub system_flags: u32,
    /// The user flags 1, 2, ...
    pub user_flags: u32,
    /// 64-flag words of each kind: 1 on the 48, 2 on the 49G.
    pub flag_words: usize,
}

impl Layout {
    /// 48SX (ROM J; the same addresses in the 1991 entry list of ROM E).
    pub const HP48SX: Layout = Layout {
        home_ptr: 0x70592,
        home_end_ptr: 0x70597,
        context_ptr: 0x7059C,
        stack_ptr: 0x70579,
        stack_end_ptr: 0x7057E,
        system_flags: 0x706C5,
        user_flags: 0x706D5,
        flag_words: 1,
    };
    /// 48GX (ROM R).
    pub const HP48GX: Layout = Layout {
        home_ptr: 0x80711,
        home_end_ptr: 0x80716,
        context_ptr: 0x8071B,
        stack_ptr: 0x806F8,
        stack_end_ptr: 0x806FD,
        system_flags: 0x80843,
        user_flags: 0x80853,
        flag_words: 1,
    };
    /// 49G (ROM 2.10).
    pub const HP49G: Layout = Layout {
        home_ptr: 0x80711,
        home_end_ptr: 0x80716,
        context_ptr: 0x8071B,
        stack_ptr: 0x806F8,
        stack_end_ptr: 0x806FD,
        system_flags: 0x80F02,
        user_flags: 0x80F22,
        flag_words: 2,
    };

    /// The layout of `model`, or `None` for the aplet models and the 42S.
    pub fn of(model: Model) -> Option<Layout> {
        match model {
            Model::Hp48sx => Some(Self::HP48SX),
            Model::Hp48gx => Some(Self::HP48GX),
            Model::Hp49g => Some(Self::HP49G),
            Model::Hp38g | Model::Hp39g | Model::Hp40g | Model::Hp42s => None,
        }
    }
}

/// A variable of a directory as the calculator lists it (`VARS` order,
/// `BYTES` size and checksum).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Variable {
    /// Name (HP characters as Unicode).
    pub name: String,
    /// The type name as `G D` prints it, e.g. `Real Number`, `Directory`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Size in bytes as `BYTES` reports it: the whole variable record
    /// (name and object), so it can end in .5.
    pub size: f64,
    /// `BYTES`'s checksum: the CRC of the object's nibbles.
    pub checksum: u16,
    /// Address of the object in memory.
    pub address: u32,
    /// A directory's variables, newest first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Vec<Variable>>,
}

/// The flags as 64-flag words, flag 1 (or -1) in bit 0 of the first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flags {
    /// System flags: -1..-64, then -65..-128 on the 49G.
    pub system: Vec<u64>,
    /// User flags: 1..64, then 65..128 on the 49G.
    pub user: Vec<u64>,
}

impl Flags {
    /// Whether flag `n` is set: negative for system flags, positive for
    /// user flags; `None` for 0 or past the model's flags.
    pub fn get(&self, n: i32) -> Option<bool> {
        let (words, i) = match n {
            0 => return None,
            n if n < 0 => (&self.system, n.unsigned_abs() - 1),
            n => (&self.user, n.unsigned_abs() - 1),
        };
        let word = words.get(usize::try_from(i / 64).ok()?)?;
        Some(word >> (i % 64) & 1 == 1)
    }

    /// The binary integer display base (flags -11 and -12).
    pub fn base(&self) -> Base {
        match (self.get(-11), self.get(-12)) {
            (Some(true), Some(true)) => Base::Hex,
            (Some(true), _) => Base::Oct,
            (_, Some(true)) => Base::Bin,
            _ => Base::Dec,
        }
    }

    /// The flags that are set, system flags first (-1, -2, ...), then user
    /// flags (1, 2, ...).
    pub fn set(&self) -> Vec<i32> {
        let mut out = Vec::new();
        for (words, sign) in [(&self.system, -1i32), (&self.user, 1)] {
            for (w, word) in words.iter().enumerate() {
                for bit in 0..64 {
                    if word >> bit & 1 == 1 {
                        let n = i32::try_from(w * 64 + bit + 1).unwrap_or(i32::MAX);
                        out.push(sign * n);
                    }
                }
            }
        }
        out
    }
}

impl Serialize for Flags {
    /// `{"system": ["0000000204010FF0"], "user": [...], "set": [-5, ...]}`:
    /// words as 16 hex digits (a JSON number would lose bits), and the set
    /// flags.
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let hex = |w: &Vec<u64>| w.iter().map(|v| format!("{v:016X}")).collect::<Vec<_>>();
        let mut st = s.serialize_struct("Flags", 3)?;
        st.serialize_field("system", &hex(&self.system))?;
        st.serialize_field("user", &hex(&self.user))?;
        st.serialize_field("set", &self.set())?;
        st.end()
    }
}

/// The calculator's CRC over `nibbles` (wiki: hardware/crc; `BYTES` zeroes
/// the CRC register and reads the object).
pub fn crc(nibbles: &[u8]) -> u16 {
    let mut c: u32 = 0;
    for &n in nibbles {
        c = (c >> 4) ^ (((c ^ u32::from(n)) & 0xF) * 0x1081);
    }
    (c & 0xFFFF) as u16
}

/// User memory of a paused machine, read through `layout`.
#[derive(Clone, Copy)]
pub struct UserMemory<'a> {
    mem: &'a dyn Memory,
    layout: Layout,
}

impl std::fmt::Debug for UserMemory<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserMemory")
            .field("layout", &self.layout)
            .finish_non_exhaustive()
    }
}

/// HOME read in one piece.
struct Home {
    /// Address of the HOME object.
    addr: u32,
    /// Its nibbles.
    nibbles: Vec<u8>,
    /// Nibble of its last-variable offset field.
    offset_at: usize,
}

impl<'a> UserMemory<'a> {
    /// `mem` with the system pointers at `layout`.
    pub fn new(mem: &'a dyn Memory, layout: Layout) -> Self {
        Self { mem, layout }
    }

    /// The user memory of `machine` (48SX, 48GX, 49G).
    pub fn of(machine: &'a Machine) -> Result<Self> {
        let model = machine.model();
        let layout = Layout::of(model).with_context(|| {
            let why = if model == Model::Hp42s {
                "has no RPL user memory (no HOME directory, no RPL stack)"
            } else {
                "keeps aplets, not a HOME directory"
            };
            format!("the {} {why}: no memory view", model.name().to_uppercase())
        })?;
        Ok(Self::new(machine, layout))
    }

    /// The layout in use.
    pub fn layout(&self) -> Layout {
        self.layout
    }

    fn nibble(&self, addr: u32) -> Result<u8> {
        self.mem
            .nibble(addr)
            .with_context(|| format!("#{addr:05X} is not readable"))
    }

    /// The 5-nibble field at `addr`.
    fn pointer(&self, addr: u32) -> Result<u32> {
        let mut v = 0;
        for i in (0..5).rev() {
            v = (v << 4) | u32::from(self.nibble(addr + i)?);
        }
        Ok(v)
    }

    /// The nibbles from `from` up to `to`.
    fn region(&self, from: u32, to: u32) -> Result<Vec<u8>> {
        ensure!(
            from <= to && to - from <= MAX_REGION,
            "region #{from:05X}-#{to:05X} is not plausible"
        );
        (from..to).map(|a| self.nibble(a)).collect()
    }

    fn home(&self) -> Result<Home> {
        let addr = self.pointer(self.layout.home_ptr)?;
        let end = self.pointer(self.layout.home_end_ptr)?;
        let nibbles = self.region(addr, end).context("reading HOME")?;
        let prolog = crate::prolog::read_field(&nibbles, 0, 5);
        ensure!(
            prolog == Some(ObjectType::Directory.prolog()),
            "no directory at HOME (#{addr:05X}): the ROM has not set up memory yet"
        );
        // HOME attaches a list of libraries where an ordinary directory has
        // one library id: a 3-nibble count, then 13 nibbles per library.
        let count = crate::prolog::read_field(&nibbles, 5, 3).context("HOME truncated")?;
        let offset_at = 8 + 13 * usize::try_from(count)?;
        Ok(Home {
            addr,
            nibbles,
            offset_at,
        })
    }

    /// The variables of the directory body in `n` whose offset field is at
    /// `offset_at`, newest first, sub-directories expanded; `base` is the
    /// address of `n[0]`.
    fn variables(
        &self,
        n: &[u8],
        base: u32,
        offset_at: usize,
        depth: usize,
    ) -> Result<Vec<Variable>> {
        let rs = records(n, offset_at, depth)?;
        let mut out = Vec::with_capacity(rs.len());
        for r in rs.iter().rev() {
            // The empty name is HOME's hidden directory (user keys, alarms).
            if r.name_len == 0 {
                continue;
            }
            out.push(self.variable(n, base, r, depth)?);
        }
        Ok(out)
    }

    fn variable(&self, n: &[u8], base: u32, r: &Record, depth: usize) -> Result<Variable> {
        let name_bytes: Vec<u8> = (0..r.name_len)
            .map(|i| {
                crate::prolog::read_field(n, r.name_at + 2 + 2 * i, 2)
                    .map(|b| (b & 0xFF) as u8)
                    .context("name truncated")
            })
            .collect::<Result<_>>()?;
        let object = &n[r.object..r.end()];
        let address = base + u32::try_from(r.object)?;
        let prolog = crate::prolog::read_field(object, 0, 5).context("object truncated")?;
        let ty = ObjectType::from_prolog(prolog);
        let (kind, variables) = match ty {
            Some(ObjectType::Directory) => (
                ty.map(ObjectType::name),
                Some(self.variables(n, base, r.object + 8, depth + 1)?),
            ),
            Some(t) => (Some(t.name()), None),
            // A 5-nibble pointer to a ROM object (inferred: the ROM's STO
            // copies objects, so this is rare); the target's type.
            None => (
                self.pointer(prolog)
                    .ok()
                    .and_then(ObjectType::from_prolog)
                    .map(ObjectType::name),
                None,
            ),
        };
        Ok(Variable {
            name: crate::charset::decode(&name_bytes),
            kind: kind.unwrap_or("ROM Pointer").to_string(),
            size: (r.end() - r.start) as f64 / 2.0,
            checksum: crc(object),
            address,
            variables,
        })
    }

    /// HOME's variables, newest first (as `VARS`), with every
    /// sub-directory's variables. Needs no decode budget: the walk sizes
    /// objects without following ROM pointers, so its work is linear in
    /// HOME's nibbles (at most the address space).
    pub fn tree(&self) -> Result<Vec<Variable>> {
        let home = self.home()?;
        let vars = self.variables(&home.nibbles, home.addr, home.offset_at, 0)?;
        // The records must fill HOME exactly, else the layout is wrong.
        let end = records(&home.nibbles, home.offset_at, 0)?
            .last()
            .map_or(home.offset_at + 5, Record::end);
        ensure!(
            end == home.nibbles.len(),
            "HOME's variables end at #{:05X}, HOME at #{:05X}",
            home.addr + u32::try_from(end)?,
            home.addr + u32::try_from(home.nibbles.len())?
        );
        Ok(vars)
    }

    /// The current directory as a path from HOME, e.g. `["HOME", "A"]`.
    pub fn current_path(&self) -> Result<Vec<String>> {
        let context = self.pointer(self.layout.context_ptr)?;
        let home = self.pointer(self.layout.home_ptr)?;
        let mut path = vec!["HOME".to_string()];
        if context == home {
            return Ok(path);
        }
        if find(&self.tree()?, context, &mut path) {
            Ok(path)
        } else {
            bail!("the current directory (#{context:05X}) is not under HOME")
        }
    }

    /// The addresses of the objects on the data stack, level 1 first.
    pub fn stack_addresses(&self) -> Result<Vec<u32>> {
        let top = self.pointer(self.layout.stack_ptr)?;
        let end = self.pointer(self.layout.stack_end_ptr)?;
        let span = end
            .checked_sub(top)
            .filter(|s| s % 5 == 0 && *s >= 5)
            .with_context(|| format!("stack pointers #{top:05X}-#{end:05X} are not plausible"))?;
        let depth = usize::try_from(span / 5 - 1)?;
        ensure!(depth <= MAX_LEVELS, "stack deeper than {MAX_LEVELS} levels");
        ensure!(
            self.pointer(end - 5)? == 0,
            "no end marker at #{:05X}: the ROM is not idle",
            end - 5
        );
        (0..depth)
            .map(|i| {
                let at = top + 5 * u32::try_from(i)?;
                let p = self.pointer(at)?;
                ensure!(p != 0, "stack level {} is 0 at #{at:05X}", i + 1);
                Ok(p)
            })
            .collect()
    }

    /// The data stack as typed objects, level 1 first; binary integers
    /// carry the display base of flags -11 and -12.
    pub fn stack(&self) -> Result<Vec<Object>> {
        let base = self.flags()?.base();
        // One budget for the whole stack: the result is one document a
        // host must hold, however many levels share an object.
        let reader = Reader::new(self.mem);
        self.stack_addresses()?
            .into_iter()
            .enumerate()
            .map(|(i, addr)| {
                let mut obj = reader
                    .decode_at(addr)
                    .with_context(|| format!("stack level {}", i + 1))?;
                obj.set_base(base);
                Ok(obj)
            })
            .collect()
    }

    fn words(&self, at: u32) -> Result<Vec<u64>> {
        (0..self.layout.flag_words)
            .map(|w| {
                let start = at + 16 * u32::try_from(w)?;
                let mut v = 0u64;
                for i in (0..16).rev() {
                    v = (v << 4) | u64::from(self.nibble(start + i)?);
                }
                Ok(v)
            })
            .collect()
    }

    /// The system and user flags.
    pub fn flags(&self) -> Result<Flags> {
        Ok(Flags {
            system: self.words(self.layout.system_flags)?,
            user: self.words(self.layout.user_flags)?,
        })
    }

    /// A number that changes whenever HOME (any variable anywhere in the
    /// tree), the current directory, the stack or the flags change: an
    /// FNV-1a hash of HOME's nibbles, the system pointers, the stack's
    /// entries and the flag words. A host polls it and re-reads only when
    /// it moves. Reads HOME once (up to the whole RAM). It hashes the stack's
    /// pointers, not the objects they point to: an object rewritten in place
    /// at the same address would not move it.
    pub fn change_counter(&self) -> Result<u64> {
        let mut h = Fnv::default();
        let l = self.layout;
        for p in [
            l.home_ptr,
            l.home_end_ptr,
            l.context_ptr,
            l.stack_ptr,
            l.stack_end_ptr,
        ] {
            h.word(u64::from(self.pointer(p)?));
        }
        let home = self.pointer(l.home_ptr)?;
        let end = self.pointer(l.home_end_ptr)?;
        for n in self.region(home, end)? {
            h.byte(n);
        }
        // Stack entries; garbage pointers are hashed as they are.
        let top = self.pointer(l.stack_ptr)?;
        let stack_end = self
            .pointer(l.stack_end_ptr)?
            .min(top + 5 * MAX_LEVELS as u32);
        for a in (top..stack_end).step_by(5) {
            h.word(u64::from(self.pointer(a)?));
        }
        let flags = self.flags()?;
        for w in flags.system.iter().chain(&flags.user) {
            h.word(*w);
        }
        Ok(h.0)
    }
}

/// Extend `path` to the directory at `addr` under `vars`.
fn find(vars: &[Variable], addr: u32, path: &mut Vec<String>) -> bool {
    for v in vars {
        let Some(children) = &v.variables else {
            continue;
        };
        path.push(v.name.clone());
        if v.address == addr || find(children, addr, path) {
            return true;
        }
        path.pop();
    }
    false
}

/// 64-bit FNV-1a.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xCBF2_9CE4_8422_2325)
    }
}

impl Fnv {
    fn byte(&mut self, b: u8) {
        self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3);
    }

    fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.byte(b);
        }
    }
}

/// HOME's variables of `machine`, newest first, with sub-directories.
pub fn memory_tree(machine: &Machine) -> Result<Vec<Variable>> {
    UserMemory::of(machine)?.tree()
}

/// The current directory of `machine`, e.g. `["HOME", "A"]`.
pub fn current_path(machine: &Machine) -> Result<Vec<String>> {
    UserMemory::of(machine)?.current_path()
}

/// The data stack of `machine`, level 1 first.
pub fn stack_objects(machine: &Machine) -> Result<Vec<Object>> {
    UserMemory::of(machine)?.stack()
}

/// The flags of `machine`.
pub fn flags(machine: &Machine) -> Result<Flags> {
    UserMemory::of(machine)?.flags()
}

/// See [`UserMemory::change_counter`].
pub fn change_counter(machine: &Machine) -> Result<u64> {
    UserMemory::of(machine)?.change_counter()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The aplet models and the 42S have no layout, each with its own
    /// reason.
    #[test]
    fn models_without_a_memory_view() {
        for model in [Model::Hp38g, Model::Hp39g, Model::Hp40g, Model::Hp42s] {
            assert!(Layout::of(model).is_none(), "{}", model.name());
        }
        let m = Machine::new(Model::Hp42s, &vec![0u8; Model::Hp42s.rom_bytes()]).unwrap();
        let e = UserMemory::of(&m).err().unwrap().to_string();
        assert!(e.contains("the 42S has no RPL user memory"), "{e}");
        let m = Machine::new(Model::Hp38g, &vec![0u8; Model::Hp38g.rom_bytes()]).unwrap();
        let e = UserMemory::of(&m).err().unwrap().to_string();
        assert!(e.contains("keeps aplets"), "{e}");
    }
    use crate::object::{MAX_DECODED_OBJECTS, Real, decode_at};

    /// RAM #70000-#7FFFF of a 48SX.
    struct Ram(Vec<u8>);

    impl Memory for Ram {
        fn nibble(&self, addr: u32) -> Option<u8> {
            let i = usize::try_from(addr.checked_sub(0x70000)?).ok()?;
            self.0.get(i).copied()
        }
    }

    impl Ram {
        fn put(&mut self, addr: u32, hex: &str) {
            let at = (addr - 0x70000) as usize;
            for (i, c) in hex.chars().enumerate() {
                self.0[at + i] = c.to_digit(16).unwrap() as u8;
            }
        }

        fn put_ptr(&mut self, addr: u32, value: u32) {
            let hex: String = (0..5)
                .map(|i| format!("{:X}", value >> (4 * i) & 0xF))
                .collect();
            self.put(addr, &hex);
        }
    }

    /// HOME of a 48SX (ROM J in saturnus, 2026-10-05) after `42 'X' STO
    /// "HI" 'YY' STO 'DIRA' CRDIR DIRA 5 'Z' STO`, with IOPAR from the
    /// Kermit server and the hidden directory; current directory DIRA.
    fn sx() -> Ram {
        let mut r = Ram(vec![0; 0x10000]);
        r.put(
            0x7FE9B,
            concat!(
                "69A20200200746228D356007EFD2200000501000",
                "00000069A20FF7C500000000C055375627B45697",
                "37E2342534C0E4A20900006E57A2000805537562",
                "7B45697378047A20B2130E10006014C61627D637",
                "6047A20B2130080005094F40514255047A201932",
                "24B2A24B2A24B2A23F2A29C2A2B2130630001085",
                "10339201000000000000240B100020959520C2A2",
                "09000084946100040449425144069A20FF7A0000",
                "0000010A51033920000000000000005",
            ),
        );
        let l = Layout::HP48SX;
        r.put_ptr(l.home_ptr, 0x7FE9B);
        r.put_ptr(l.home_end_ptr, 0x7FFFB);
        r.put_ptr(l.context_ptr, 0x7FFCE);
        // Stack: "AB" (in RAM) on a ROM-free real 7 and a binary integer.
        r.put(0x71000, "C2A2090000142433920000000000000007");
        r.put(0x71030, "E4A2051000A200000000000000");
        r.put_ptr(0x7F000, 0x71000);
        r.put_ptr(0x7F005, 0x7100E);
        r.put_ptr(0x7F00A, 0x71030);
        r.put_ptr(l.stack_ptr, 0x7F000);
        r.put_ptr(l.stack_end_ptr, 0x7F014);
        // Flags -11 and -12 (HEX), -40; user flag 7.
        r.put(l.system_flags, "0FF0000008000000");
        r.put(l.user_flags, "0400000000000000");
        r
    }

    #[test]
    fn tree_sizes_and_checksums_match_the_calculator() {
        let ram = sx();
        let m = UserMemory::new(&ram, Layout::HP48SX);
        let tree = m.tree().unwrap();
        // Name, type, size, checksum as the calculator's `G D` listed them.
        let flat: Vec<(&str, &str, f64, u16)> = tree
            .iter()
            .map(|v| (v.name.as_str(), v.kind.as_str(), v.size, v.checksum))
            .collect();
        assert_eq!(
            flat,
            [
                ("DIRA", "Directory", 31.0, 44093),
                ("YY", "String", 13.5, 5889),
                ("X", "Real Number", 16.0, 59472),
                ("IOPAR", "List", 29.5, 8861),
            ]
        );
        let dira = tree[0].variables.as_ref().unwrap();
        assert_eq!(dira.len(), 1);
        assert_eq!(
            (dira[0].name.as_str(), dira[0].size, dira[0].checksum),
            ("Z", 16.0, 23381)
        );
        assert_eq!(dira[0].address, 0x7FFE6);
        assert_eq!(m.current_path().unwrap(), ["HOME", "DIRA"]);
        assert_eq!(
            decode_at(tree[2].address, &ram).unwrap(),
            Object::Real {
                value: Real::parse("42").unwrap()
            }
        );
    }

    #[test]
    fn stack_and_flags() {
        let ram = sx();
        let m = UserMemory::new(&ram, Layout::HP48SX);
        let flags = m.flags().unwrap();
        assert_eq!(flags.get(-40), Some(true));
        assert_eq!(flags.get(7), Some(true));
        assert_eq!(flags.get(8), Some(false));
        assert_eq!(flags.get(65), None);
        assert_eq!(flags.base(), Base::Hex);
        assert_eq!(flags.set(), [-5, -6, -7, -8, -9, -10, -11, -12, -40, 7]);
        let json = serde_json::to_value(&flags).unwrap();
        assert_eq!(json["system"][0], "0000008000000FF0");
        assert_eq!(json["user"][0], "0000000000000040");
        let stack = m.stack().unwrap();
        assert_eq!(stack.len(), 3);
        assert_eq!(stack[0], Object::String { value: "AB".into() });
        assert_eq!(
            stack[1],
            Object::Real {
                value: Real::parse("7").unwrap()
            }
        );
        assert_eq!(
            serde_json::to_value(&stack[2]).unwrap(),
            serde_json::json!({"type": "binary", "value": 42, "base": "hex", "text": "# 2Ah"})
        );
    }

    #[test]
    fn change_counter_moves_with_the_tree_and_the_stack() {
        let mut ram = sx();
        let c0 = UserMemory::new(&ram, Layout::HP48SX)
            .change_counter()
            .unwrap();
        assert_eq!(
            UserMemory::new(&ram, Layout::HP48SX)
                .change_counter()
                .unwrap(),
            c0
        );
        // X = 42 becomes 43.
        ram.put(0x7FF39 + 16, "3");
        let c1 = UserMemory::new(&ram, Layout::HP48SX)
            .change_counter()
            .unwrap();
        assert_ne!(c1, c0);
        ram.put_ptr(Layout::HP48SX.stack_ptr, 0x7F005);
        assert_ne!(
            UserMemory::new(&ram, Layout::HP48SX)
                .change_counter()
                .unwrap(),
            c1
        );
    }

    /// Lists L0..L39 at #71000, each holding two pointers to the next (the
    /// last empty): 2^39 objects if every pointer were decoded anew.
    fn shared_pointers(levels: u32) -> Ram {
        let mut r = sx();
        let base = 0x71000;
        for i in 0..levels {
            let a = base + 20 * i;
            r.put(a, "47A20");
            if i + 1 < levels {
                r.put_ptr(a + 5, a + 20);
                r.put_ptr(a + 10, a + 20);
                r.put(a + 15, "B2130");
            } else {
                r.put(a + 5, "B2130");
            }
        }
        r.put_ptr(0x7F000, base);
        r.put_ptr(0x7F005, 0);
        r.put_ptr(Layout::HP48SX.stack_ptr, 0x7F000);
        r.put_ptr(Layout::HP48SX.stack_end_ptr, 0x7F00A);
        r
    }

    #[test]
    fn shared_pointers_hit_the_decode_budget() {
        let ram = shared_pointers(40);
        let m = UserMemory::new(&ram, Layout::HP48SX);
        let calls: [&dyn Fn() -> Result<()>; 2] = [&|| decode_at(0x71000, &ram).map(drop), &|| {
            m.stack().map(drop)
        }];
        for call in calls {
            // The budget error is the bound on the work: no wall-clock
            // assertion here, a loaded CI runner would make it flaky.
            let e = call().unwrap_err();
            assert!(format!("{e:#}").contains("MAX_DECODED_OBJECTS"), "{e:#}");
        }
        // 2^12 - 1 lists: shared, but within the budget, and exact.
        let ram = shared_pointers(12);
        let obj = decode_at(0x71000, &ram).unwrap();
        fn count(o: &Object) -> usize {
            match o {
                Object::List { items } => 1 + items.iter().map(count).sum::<usize>(),
                _ => 1,
            }
        }
        assert_eq!(count(&obj), (1 << 12) - 1);
        assert!(count(&obj) < MAX_DECODED_OBJECTS);
    }

    #[test]
    fn broken_pointers_are_errors() {
        let mut ram = sx();
        ram.put_ptr(Layout::HP48SX.home_end_ptr, 0x7FFF0);
        assert!(UserMemory::new(&ram, Layout::HP48SX).tree().is_err());
        let mut ram = sx();
        ram.put_ptr(Layout::HP48SX.stack_end_ptr, 0x7F013);
        assert!(UserMemory::new(&ram, Layout::HP48SX).stack().is_err());
        ram.put_ptr(Layout::HP48SX.stack_end_ptr, 0x7F00F);
        assert!(UserMemory::new(&ram, Layout::HP48SX).stack().is_err());
    }
}
