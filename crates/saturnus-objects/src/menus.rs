//! The built-in menus, read from the ROM image: which commands each menu
//! number offers and which keys lead to submenus.
//!
//! Where the definitions are (wiki: protocols/rpl-libraries, "Built-in
//! menus"): `MENU`'s own code names them, either as a list whose element
//! n is menu n (48SX), or as a library whose command n is menu n (48GX and
//! 49G: a library number in `MENU`'s code whose commands carry no names
//! and are menu definitions). A definition is a list of keys, an array of
//! XLIB names (one per key), or a program that builds the list. A key is
//! a command (its name is the label), a `{ label action }` list whose
//! action runs commands or switches to another menu, or a unit name.
//!
//! Nothing ROM-specific is compiled in: the reader finds the definitions
//! from `MENU`'s code in the loaded image. Work is bounded: at most
//! [`MAX_STEPS`] objects are visited, nesting at most [`MAX_NESTING`].

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, VecDeque};

use saturnus::Model;

use crate::charset;
use crate::names::NameTable;
use crate::object::Memory;
use crate::prolog::{ObjectType, SEMI, object_size, read_field};

/// Most objects one read visits (the 49G's menu library holds about 440
/// definitions of a few dozen keys each).
pub const MAX_STEPS: usize = 1 << 18;
/// Deepest nesting followed inside one definition.
pub const MAX_NESTING: usize = 12;
/// Elements read from one composite.
const MAX_ELEMENTS: usize = 4096;
/// How far `MENU`'s code is followed through other programs.
const MAX_CODE_DEPTH: usize = 4;
/// Fewest definitions a menu list or library has.
const MIN_MENUS: usize = 20;

/// What one menu offers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuDef {
    /// The commands its keys run, in key order, each once.
    pub commands: Vec<String>,
    /// Keys that switch to another menu: (label, menu number).
    pub submenus: Vec<(String, u32)>,
}

/// The built-in menus of one ROM, by menu number.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Menus {
    /// Definition of menu n at index n (`None`: no definition read).
    menus: Vec<Option<MenuDef>>,
    /// The menu library (48GX, 49G), when the menus are its commands.
    library: Option<u16>,
    /// CPU addresses of the list's definitions (48SX), by menu number.
    addresses: HashMap<u32, u32>,
}

impl Menus {
    /// The highest menu number plus one.
    pub fn len(&self) -> usize {
        self.menus.len()
    }

    /// Whether no menu was found.
    pub fn is_empty(&self) -> bool {
        self.menus.is_empty()
    }

    /// Menu `n`'s definition.
    pub fn get(&self, n: u32) -> Option<&MenuDef> {
        self.menus.get(n as usize)?.as_ref()
    }

    /// The menu a current-menu pointer (`Layout::menu_ptr`) designates:
    /// `pointer` is that pointer's value and `mem` the machine, which
    /// holds either the definition itself (48SX) or an XLIB name of the
    /// menu library's entry (48GX, 49G).
    pub fn number(&self, pointer: u32, mem: &dyn Memory) -> Option<u32> {
        if let Some(n) = self.addresses.get(&pointer) {
            return Some(*n);
        }
        let lib = self.library?;
        let field = |at: u32, w: u32| -> Option<u32> {
            (0..w).rev().try_fold(0u32, |v, i| {
                Some((v << 4) | u32::from(mem.nibble(at.checked_add(i)?)?))
            })
        };
        if field(pointer, 5)? != ObjectType::XlibName.prolog() {
            return None;
        }
        (field(pointer + 5, 3)? == u32::from(lib)).then(|| field(pointer + 8, 3))?
    }

    /// The menus with a definition: (number, definition).
    pub fn iter(&self) -> impl Iterator<Item = (u32, &MenuDef)> {
        self.menus
            .iter()
            .enumerate()
            .filter_map(|(n, d)| Some((u32::try_from(n).ok()?, d.as_ref()?)))
    }

    /// Each menu's name: a root menu's from `roots` (the keyboard legend
    /// of the key that opens it), a submenu's as its parent's name and the
    /// label of the key that leads to it (`MTH HYP`), the shortest path
    /// winning. A menu no key reaches is named as the calculator opens it,
    /// `MENU n`, and its submenus after it (`MENU 62 FMT`).
    pub fn paths(&self, roots: &[(u32, String)]) -> BTreeMap<u32, String> {
        let mut out: BTreeMap<u32, String> = BTreeMap::new();
        let mut queue: VecDeque<u32> = VecDeque::new();
        for (n, name) in roots {
            if self.get(*n).is_some() && !out.contains_key(n) {
                out.insert(*n, name.clone());
                queue.push_back(*n);
            }
        }
        self.descend(&mut out, &mut queue);
        let unnamed: Vec<u32> = self
            .iter()
            .map(|(n, _)| n)
            .filter(|n| !out.contains_key(n))
            .collect();
        for n in unnamed {
            if let std::collections::btree_map::Entry::Vacant(e) = out.entry(n) {
                e.insert(format!("MENU {n}"));
                queue.push_back(n);
                self.descend(&mut out, &mut queue);
            }
        }
        out
    }

    /// Name the submenus of the menus in `queue`, breadth first.
    fn descend(&self, out: &mut BTreeMap<u32, String>, queue: &mut VecDeque<u32>) {
        while let Some(n) = queue.pop_front() {
            let (Some(def), Some(parent)) = (self.get(n), out.get(&n).cloned()) else {
                continue;
            };
            for (label, sub) in &def.submenus {
                if out.contains_key(sub) || self.get(*sub).is_none() {
                    continue;
                }
                out.insert(*sub, format!("{parent} {label}"));
                queue.push_back(*sub);
            }
        }
    }

    /// For each command, the names of the menus that offer it (in menu
    /// number order), named by `paths`.
    pub fn placement(&self, paths: &BTreeMap<u32, String>) -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (n, def) in self.iter() {
            let name = paths
                .get(&n)
                .cloned()
                .unwrap_or_else(|| format!("MENU {n}"));
            for c in &def.commands {
                let list = out.entry(c.clone()).or_default();
                if !list.contains(&name) {
                    list.push(name.clone());
                }
            }
        }
        out
    }
}

/// The 5-nibble field at `at`.
fn f5(n: &[u8], at: usize) -> Option<u32> {
    read_field(n, at, 5)
}

/// One element of a composite in the image.
#[derive(Clone, Copy, Debug)]
enum El {
    /// An embedded object at this image index, with its prolog.
    Embedded(usize, u32),
    /// A 5-nibble pointer (a CPU address).
    Pointer(u32),
}

/// Where the menu definitions are.
#[derive(Clone, Debug)]
enum Source {
    /// Element n of the list at this image index is menu n.
    List(Vec<El>),
    /// Command n of this library is menu n.
    Library(u16),
}

/// The ROM image as the CPU sees it while one object runs: on the 49G the
/// low window shows the bank that holds `MENU` (the system's fixed bank)
/// and the high window the bank of the object being read.
#[derive(Clone, Copy)]
struct View<'a> {
    rom: &'a [u8],
    bank: Option<usize>,
    low: usize,
    high: usize,
}

impl View<'_> {
    /// The image index of CPU address `a`.
    fn index(&self, a: u32) -> Option<usize> {
        let a = a as usize;
        let i = match self.bank {
            None => a,
            Some(b) if a < b => self.low * b + a,
            Some(b) if a < 2 * b => self.high * b + (a - b),
            Some(_) => return None,
        };
        (i < self.rom.len()).then_some(i)
    }

    /// The same view with the high window on the bank holding image index
    /// `i`.
    fn at(&self, i: usize) -> Self {
        match self.bank {
            Some(b) => View {
                high: i / b,
                ..*self
            },
            None => *self,
        }
    }
}

impl Memory for View<'_> {
    fn nibble(&self, addr: u32) -> Option<u8> {
        self.rom.get(self.index(addr)?).copied()
    }
}

struct Reader<'a> {
    rom: &'a [u8],
    names: &'a NameTable,
    view: View<'a>,
    steps: Cell<usize>,
}

impl<'a> Reader<'a> {
    /// Charge one visited object; false once the budget is spent.
    fn step(&self) -> bool {
        let s = self.steps.get();
        if s >= MAX_STEPS {
            return false;
        }
        self.steps.set(s + 1);
        true
    }

    fn prolog(&self, i: usize) -> Option<u32> {
        f5(self.rom, i)
    }

    /// The elements of the list, program or algebraic at image index `i`.
    fn elements(&self, i: usize) -> Vec<El> {
        let mut out = Vec::new();
        let mut at = i + 5;
        while out.len() < MAX_ELEMENTS && self.step() {
            let Some(p) = f5(self.rom, at) else { break };
            if p == SEMI {
                break;
            }
            if ObjectType::from_prolog(p).is_some() {
                let Ok(size) = object_size(self.rom, at) else {
                    break;
                };
                out.push(El::Embedded(at, p));
                at += size;
            } else if (0x02600..=0x02FFF).contains(&p) {
                break;
            } else {
                out.push(El::Pointer(p));
                at += 5;
            }
        }
        out
    }

    /// The XLIB body (library, command) at image index `i`.
    fn xlib_at(&self, i: usize) -> Option<(u16, u16)> {
        Some((
            read_field(self.rom, i, 3)? as u16,
            read_field(self.rom, i + 3, 3)? as u16,
        ))
    }

    /// The named command an element is, if any.
    fn command(&self, e: El, view: View<'_>) -> Option<String> {
        match e {
            El::Embedded(i, p) if p == ObjectType::XlibName.prolog() => {
                let (l, c) = self.xlib_at(i + 5)?;
                self.names.xlib(l, c)?.name.map(str::to_string)
            }
            El::Pointer(a) => self.names.command_at(a, &view)?.name.map(str::to_string),
            El::Embedded(..) => None,
        }
    }

    /// The image index an element's object is at (an embedded object, a
    /// pointer's target, an XLIB name's object), with the view to read it.
    fn target(&self, e: El, view: View<'a>) -> Option<(usize, View<'a>)> {
        match e {
            El::Embedded(i, p) if p == ObjectType::XlibName.prolog() => {
                let (l, c) = self.xlib_at(i + 5)?;
                let t = self.names.object_index(l, c)?;
                Some((t, view.at(t)))
            }
            El::Embedded(i, _) => Some((i, view)),
            El::Pointer(a) => Some((view.index(a)?, view)),
        }
    }

    /// The XLIB names of an array of them at image index `i`.
    fn xlib_array(&self, i: usize) -> Option<Vec<(u16, u16)>> {
        if self.prolog(i)? != ObjectType::Array.prolog()
            || f5(self.rom, i + 10)? != ObjectType::XlibName.prolog()
            || f5(self.rom, i + 15)? != 1
        {
            return None;
        }
        let count = f5(self.rom, i + 20)? as usize;
        if count > MAX_ELEMENTS {
            return None;
        }
        (0..count).map(|k| self.xlib_at(i + 25 + 6 * k)).collect()
    }
}

/// Collects one definition.
struct Collect<'r, 'a> {
    r: &'r Reader<'a>,
    source: &'r Source,
    /// Image indexes of the list source's definitions, by menu number.
    defs: HashMap<usize, u32>,
    def: MenuDef,
}

impl<'a> Collect<'_, 'a> {
    fn add_command(&mut self, name: String) {
        if !self.def.commands.contains(&name) {
            self.def.commands.push(name);
        }
    }

    /// The menu an element switches to, if it is a reference to a menu
    /// definition (an XLIB name of the menu library, or a pointer to one
    /// of the list's definitions).
    fn menu_ref(&self, e: El, view: View<'a>) -> Option<u32> {
        match (e, self.source) {
            (El::Embedded(i, p), Source::Library(lib)) if p == ObjectType::XlibName.prolog() => {
                let (l, c) = self.r.xlib_at(i + 5)?;
                (l == *lib).then_some(u32::from(c))
            }
            (El::Pointer(a), Source::List(_)) => self.defs.get(&view.index(a)?).copied(),
            (El::Embedded(i, _), Source::List(_)) => self.defs.get(&i).copied(),
            _ => None,
        }
    }

    /// The first menu that the object at `i` switches to, looking into
    /// its programs and lists.
    fn find_menu(&self, i: usize, view: View<'a>, depth: usize) -> Option<u32> {
        if depth > MAX_NESTING || !composite(self.r.prolog(i)?) {
            return None;
        }
        for e in self.r.elements(i) {
            if let Some(n) = self.menu_ref(e, view) {
                return Some(n);
            }
            if let El::Embedded(j, p) = e {
                if composite(p) {
                    if let Some(n) = self.find_menu(j, view, depth + 1) {
                        return Some(n);
                    }
                }
            }
        }
        None
    }

    /// Every named command inside the object at `i` (not through pointers
    /// to unnamed ROM code).
    fn commands_in(&mut self, i: usize, view: View<'a>, depth: usize) {
        if depth > MAX_NESTING {
            return;
        }
        let Some(p) = self.r.prolog(i) else { return };
        if p == ObjectType::XlibName.prolog() {
            if let Some(name) = self.r.command(El::Embedded(i, p), view) {
                self.add_command(name);
            }
            return;
        }
        if !composite(p) {
            return;
        }
        for e in self.r.elements(i) {
            if let Some(name) = self.r.command(e, view) {
                self.add_command(name);
            } else if let El::Embedded(j, q) = e {
                if composite(q) {
                    self.commands_in(j, view, depth + 1);
                }
            }
        }
    }

    /// The text of a key's label: a string, the first string inside a
    /// program that draws it, or a command (shown by its name).
    fn label(&self, e: El, view: View<'a>, depth: usize) -> Option<String> {
        if let Some(name) = self.r.command(e, view) {
            return Some(name);
        }
        let (i, view) = self.r.target(e, view)?;
        let p = self.r.prolog(i)?;
        if p == ObjectType::String.prolog() {
            let len = (f5(self.r.rom, i + 5)? as usize).checked_sub(5)? / 2;
            let bytes: Option<Vec<u8>> = (0..len)
                .map(|k| read_field(self.r.rom, i + 10 + 2 * k, 2).map(|b| b as u8))
                .collect();
            // Without the control characters some labels carry (the
            // 49G's folder mark).
            let text: String = charset::decode(&bytes?)
                .chars()
                .filter(|c| !c.is_control())
                .collect();
            return Some(text.trim().to_string()).filter(|t| !t.is_empty());
        }
        if p == ObjectType::Program.prolog() && depth < 2 {
            return self
                .r
                .elements(i)
                .into_iter()
                .find_map(|x| self.label(x, view, depth + 1));
        }
        None
    }

    /// One key.
    fn key(&mut self, e: El, view: View<'a>, depth: usize) {
        if depth > MAX_NESTING || !self.r.step() {
            return;
        }
        if let Some(name) = self.r.command(e, view) {
            self.add_command(name);
            return;
        }
        let Some((i, view)) = self.r.target(e, view) else {
            return;
        };
        let Some(p) = self.r.prolog(i) else { return };
        if p == ObjectType::List.prolog() {
            let els = self.r.elements(i);
            // { label action ... }: a label first, then what the key does.
            let labelled = els
                .split_first()
                .and_then(|(&first, rest)| Some((self.label(first, view, 0)?, rest)));
            if let Some((label, rest)) = labelled {
                // A key that leads to a submenu offers that menu, not the
                // commands its shifted variants type (`IF`'s `IF THEN
                // END`): those are offered in the submenu itself.
                let submenu = rest.iter().find_map(|&a| {
                    self.menu_ref(a, view).or_else(|| {
                        let (j, v) = self.r.target(a, view)?;
                        self.find_menu(j, v, depth + 1)
                    })
                });
                if let Some(n) = submenu {
                    self.def.submenus.push((label, n));
                    return;
                }
                for &a in rest {
                    if let Some(name) = self.r.command(a, view) {
                        self.add_command(name);
                    } else if let Some((j, v)) = self.r.target(a, view) {
                        self.commands_in(j, v, depth + 1);
                    }
                }
                return;
            }
            self.commands_in(i, view, depth + 1);
        } else if p == ObjectType::Program.prolog() {
            self.commands_in(i, view, depth + 1);
        }
    }

    /// The keys of a definition at image index `i`.
    fn definition(&mut self, i: usize, view: View<'a>, depth: usize) {
        if depth > 3 {
            return;
        }
        let Some(p) = self.r.prolog(i) else { return };
        if let Some(keys) = self.r.xlib_array(i) {
            for (l, c) in keys {
                // Built as an XLIB name element, read the same way.
                match self.r.names.xlib(l, c).and_then(|x| x.name) {
                    Some(name) => self.add_command(name.to_string()),
                    None => {
                        if let Some(t) = self.r.names.object_index(l, c) {
                            let v = view.at(t);
                            self.key_object(t, v, depth + 1);
                        }
                    }
                }
            }
        } else if p == ObjectType::XlibName.prolog() {
            if let Some((t, v)) = self.r.target(El::Embedded(i, p), view) {
                self.definition(t, v, depth + 1);
            }
        } else if p == ObjectType::List.prolog() {
            for e in self.r.elements(i) {
                self.key(e, view, 1);
            }
        } else if p == ObjectType::Program.prolog() {
            // A program that builds the menu: its list of keys.
            if let Some(El::Embedded(j, _)) = self
                .r
                .elements(i)
                .into_iter()
                .find(|e| matches!(e, El::Embedded(_, q) if *q == ObjectType::List.prolog()))
            {
                self.definition(j, view, depth + 1);
            }
        }
    }

    /// A key stored as its own object (a library command that is a
    /// `{ label action }` list or a command).
    fn key_object(&mut self, t: usize, view: View<'a>, depth: usize) {
        let p = self.r.prolog(t);
        if p == Some(ObjectType::List.prolog()) || p == Some(ObjectType::Program.prolog()) {
            self.key(El::Embedded(t, p.unwrap_or(0)), view, depth);
        }
    }
}

fn composite(p: u32) -> bool {
    p == ObjectType::List.prolog() || p == ObjectType::Program.prolog()
}

/// The menus of `model`'s ROM image `rom` (one nibble per element), named
/// through `names` (the table of the same image). `None` when `MENU` or
/// its definitions cannot be found (also for models whose names table is
/// empty: the 38G, 39G, 40G and 42S).
pub fn read(model: Model, rom: &[u8], names: &NameTable) -> Option<Menus> {
    let _ = model;
    let (lib, cmd) = names.find("MENU")?;
    let menu = names.object_index(lib, cmd)?;
    let bank = names.bank();
    let low = bank.map_or(0, |b| menu / b);
    let view = View {
        rom,
        bank,
        low,
        high: low,
    };
    let r = Reader {
        rom,
        names,
        view,
        steps: Cell::new(0),
    };
    let source = locate(&r, menu)?;
    let count = match &source {
        Source::List(els) => els.len() + 1,
        Source::Library(l) => names.library_size(*l)?.0,
    };
    let mut c = Collect {
        r: &r,
        source: &source,
        defs: HashMap::new(),
        def: MenuDef::default(),
    };
    if let Source::List(els) = &source {
        for (k, &e) in els.iter().enumerate() {
            if let Some((i, _)) = r.target(e, view) {
                c.defs.insert(i, u32::try_from(k + 1).ok()?);
            }
        }
    }
    let mut addresses = HashMap::new();
    if let Source::List(els) = &source {
        for (k, &e) in els.iter().enumerate() {
            let a = match e {
                El::Pointer(a) => Some(a),
                // An embedded definition: its address is its image index
                // (unbanked ROM).
                El::Embedded(i, _) => u32::try_from(i).ok().filter(|_| bank.is_none()),
            };
            if let (Some(a), Ok(n)) = (a, u32::try_from(k + 1)) {
                addresses.insert(a, n);
            }
        }
    }
    let library = match &source {
        Source::Library(l) => Some(*l),
        Source::List(_) => None,
    };
    let mut menus = vec![None; count];
    for (n, slot) in menus.iter_mut().enumerate() {
        let at = match &source {
            // Menu 0 is the last menu: no definition of its own.
            Source::List(els) => n.checked_sub(1).and_then(|k| els.get(k).copied()),
            Source::Library(l) => names
                .object_index(*l, u16::try_from(n).ok()?)
                .and_then(|t| f5(rom, t).map(|p| El::Embedded(t, p))),
        };
        let Some(at) = at else { continue };
        let Some((i, v)) = r.target(at, view) else {
            continue;
        };
        c.def = MenuDef::default();
        c.definition(i, v.at(i), 0);
        *slot = Some(std::mem::take(&mut c.def));
    }
    Some(Menus {
        menus,
        library,
        addresses,
    })
}

/// Where `MENU`'s code (the program at image index `menu`, followed
/// through the unnamed programs it calls) finds the definitions: the
/// largest list of at least [`MIN_MENUS`] composites, or a library named
/// by a number in the code whose commands are unnamed menu definitions.
fn locate(r: &Reader<'_>, menu: usize) -> Option<Source> {
    let mut best: Option<(usize, Source)> = None;
    let mut seen = std::collections::HashSet::new();
    let mut queue: VecDeque<(usize, usize)> = VecDeque::from([(menu, 0)]);
    let view = r.view;
    while let Some((i, depth)) = queue.pop_front() {
        if depth > MAX_CODE_DEPTH || !seen.insert(i) || !r.step() {
            continue;
        }
        if r.prolog(i) != Some(ObjectType::Program.prolog()) {
            continue;
        }
        for e in r.elements(i) {
            let (t, embedded) = match e {
                El::Embedded(j, _) => (j, true),
                El::Pointer(a) => {
                    if r.names.command_at(a, &view).is_some() {
                        continue;
                    }
                    let Some(t) = view.index(a) else { continue };
                    (t, false)
                }
            };
            let Some(p) = r.prolog(t) else { continue };
            if p == ObjectType::Program.prolog() {
                queue.push_back((t, if embedded { depth } else { depth + 1 }));
            } else if p == ObjectType::List.prolog() {
                let els = r.elements(t);
                let menus = els
                    .iter()
                    .filter(|&&x| {
                        r.target(x, view)
                            .and_then(|(j, _)| r.prolog(j))
                            .is_some_and(composite)
                    })
                    .count();
                if els.len() >= MIN_MENUS && menus * 5 >= els.len() * 4 {
                    consider(&mut best, menus, Source::List(els));
                }
            } else if p == ObjectType::SystemBinary.prolog() {
                let Some(v) = f5(r.rom, t + 5).and_then(|v| u16::try_from(v).ok()) else {
                    continue;
                };
                let Some((len, named)) = r.names.library_size(v) else {
                    continue;
                };
                if len < MIN_MENUS || named > 0 {
                    continue;
                }
                let menus = (0..len)
                    .filter(|&c| {
                        u16::try_from(c)
                            .ok()
                            .and_then(|c| r.names.object_index(v, c))
                            .is_some_and(|j| {
                                r.xlib_array(j).is_some()
                                    || r.prolog(j) == Some(ObjectType::List.prolog())
                            })
                    })
                    .count();
                if menus * 2 >= len {
                    consider(&mut best, menus, Source::Library(v));
                }
            }
        }
    }
    best.map(|(_, s)| s)
}

fn consider(best: &mut Option<(usize, Source)>, score: usize, s: Source) {
    if best.as_ref().is_none_or(|(b, _)| score > *b) {
        *best = Some((score, s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(n: &mut [u8], at: usize, value: usize, width: usize) {
        for i in 0..width {
            n[at + i] = ((value >> (4 * i)) & 0xF) as u8;
        }
    }

    fn put_rel(n: &mut [u8], at: usize, target: usize) {
        put(n, at, (target + 0x100000 - at) & 0xFFFFF, 5);
    }

    fn put_hex(n: &mut [u8], at: usize, hex: &str) -> usize {
        for (i, c) in hex.chars().enumerate() {
            n[at + i] = c.to_digit(16).unwrap() as u8;
        }
        at + hex.len()
    }

    fn ptr(a: usize) -> String {
        (0..5)
            .map(|i| format!("{:X}", (a >> (4 * i)) & 0xF))
            .collect()
    }

    fn xlib(lib: usize, cmd: usize) -> String {
        format!("29E20{}", body(lib, cmd))
    }

    fn body(lib: usize, cmd: usize) -> String {
        (0..3)
            .map(|i| format!("{:X}", (lib >> (4 * i)) & 0xF))
            .chain((0..3).map(|i| format!("{:X}", (cmd >> (4 * i)) & 0xF)))
            .collect()
    }

    fn string(s: &str) -> String {
        let mut h = format!("C2A20{}", ptr(5 + 2 * s.len()));
        for b in s.bytes() {
            h.push_str(&format!("{:X}{:X}", b & 0xF, b >> 4));
        }
        h
    }

    /// Library `id` at `base`: names by command number, and each
    /// command's object (hex, placed from `base + 0x400` on, 0x100 apart,
    /// each after its XLIB body). Returns the objects' addresses.
    fn library(
        n: &mut [u8],
        base: usize,
        id: usize,
        names: &[Option<&str>],
        objects: &[String],
    ) -> Vec<usize> {
        let (hash, link) = (base + 0x30, base + 0x300);
        let body_at = hash + 10;
        let mut at = body_at + 85;
        let mut entries = vec![0; names.len()];
        for len in 1..=16 {
            for (c, name) in names.iter().enumerate() {
                let Some(name) = name else { continue };
                if name.len() != len {
                    continue;
                }
                if read_field(n, body_at + 5 * (len - 1), 5) == Some(0) {
                    put_rel(n, body_at + 5 * (len - 1), at);
                }
                entries[c] = at;
                put(n, at, len, 2);
                for (i, b) in name.bytes().enumerate() {
                    put(n, at + 2 + 2 * i, usize::from(b), 2);
                }
                put(n, at + 2 + 2 * len, c, 3);
                at += 2 + 2 * len + 3;
            }
        }
        put_rel(n, body_at + 80, at);
        for (c, &e) in entries.iter().enumerate() {
            let f = at + 5 * c;
            put(n, f, if e == 0 { 0 } else { f - e }, 5);
        }
        let end = at + 5 * names.len();
        assert!(end < link);
        put(n, hash, 0x02A4E, 5);
        put(n, hash + 5, end - hash - 5, 5);
        put(n, link, 0x02A4E, 5);
        put(n, link + 5, 5 + 5 * names.len(), 5);
        let mut addrs = Vec::new();
        for (c, o) in objects.iter().enumerate() {
            let obj = base + 0x400 + 0x100 * c;
            put_hex(n, obj - 6, &body(id, c));
            assert!(o.len() < 0xF0);
            put_hex(n, obj, o);
            put_rel(n, link + 10 + 5 * c, obj);
            addrs.push(obj);
        }
        put(n, base, id, 3);
        put_rel(n, base + 3, hash);
        put_rel(n, base + 13, link);
        addrs
    }

    /// Library 2 with MENU (a program around `menu_body`) and SIN, COS,
    /// TAN (programs).
    fn builtins(n: &mut [u8], menu_body: &str) -> Vec<usize> {
        let prog = "D9D20B2130".to_string();
        library(
            n,
            0x100,
            2,
            &[Some("MENU"), Some("SIN"), Some("COS"), Some("TAN")],
            &[
                format!("D9D20{menu_body}B2130"),
                prog.clone(),
                prog.clone(),
                prog,
            ],
        )
    }

    #[test]
    fn a_list_of_definitions_as_on_the_48sx() {
        let mut n = vec![0u8; 0x8000];
        let list = 0x4000;
        let a = builtins(&mut n, &ptr(list));
        let (sin, cos, tan) = (a[1], a[2], a[3]);
        // Menu 1: SIN and a key { "TRIG" menu 2 }; menu 2: COS TAN and a
        // labelled key { "T" TAN } again; menus 3-24: empty lists.
        let mut at = put_hex(&mut n, list, "47A20");
        let def1 = |m2: usize| {
            format!(
                "47A20{}47A20{}{}B2130B2130",
                ptr(sin),
                string("TRIG"),
                ptr(m2)
            )
        };
        let m2_at = at + def1(0).len();
        at = put_hex(&mut n, at, &def1(m2_at));
        at = put_hex(
            &mut n,
            at,
            &format!(
                "47A20{}{}47A20{}{}B2130B2130",
                ptr(cos),
                ptr(tan),
                string("T"),
                ptr(tan)
            ),
        );
        for _ in 3..=24 {
            at = put_hex(&mut n, at, "47A20B2130");
        }
        put_hex(&mut n, at, "B2130");
        let names = NameTable::build(Model::Hp48sx, &n);
        let m = read(Model::Hp48sx, &n, &names).unwrap();
        assert_eq!(m.len(), 25);
        assert_eq!(m.get(1).unwrap().commands, ["SIN"]);
        assert_eq!(m.get(1).unwrap().submenus, [("TRIG".to_string(), 2)]);
        assert_eq!(m.get(2).unwrap().commands, ["COS", "TAN"]);
        let paths = m.paths(&[(1, "MATH".into())]);
        assert_eq!(paths[&2], "MATH TRIG");
        assert_eq!(paths[&3], "MENU 3");
        let p = m.placement(&paths);
        assert_eq!(p["TAN"], ["MATH TRIG"]);
        assert_eq!(p["SIN"], ["MATH"]);
        // The current-menu pointer on the 48SX is the definition itself.
        struct Image<'a>(&'a [u8]);
        impl Memory for Image<'_> {
            fn nibble(&self, a: u32) -> Option<u8> {
                self.0.get(a as usize).copied()
            }
        }
        assert_eq!(m.number(m2_at as u32, &Image(&n)), Some(2));
    }

    #[test]
    fn a_menu_library_as_on_the_48gx() {
        let mut n = vec![0u8; 0x10000];
        // MENU's code names library #A9 by a system binary.
        builtins(&mut n, "119209A000");
        // Library #A8: key objects. #A9: menu n = command n.
        let keys = library(
            &mut n,
            0x8000,
            0xA8,
            &[None, None],
            &[
                format!("47A20{}D9D20{}B2130B2130", string("SUBM"), xlib(0xA9, 2)),
                format!("47A20{}{}B2130", string("C"), xlib(2, 2)),
            ],
        );
        let _ = keys;
        let array = |items: &[String]| {
            let body: String = items.iter().map(|x| x[5..].to_string()).collect();
            format!(
                "8E920{}29E2010000{}{}",
                ptr(25 + 6 * items.len() - 5),
                ptr(items.len()),
                body
            )
        };
        let mut defs = vec!["47A20B2130".to_string(); 22];
        defs[1] = array(&[xlib(2, 1), xlib(0xA8, 0)]);
        defs[2] = array(&[xlib(0xA8, 1), xlib(2, 3)]);
        library(&mut n, 0xA000, 0xA9, &vec![None; 22], &defs);
        let names = NameTable::build(Model::Hp48gx, &n);
        let m = read(Model::Hp48gx, &n, &names).unwrap();
        assert_eq!(m.len(), 22);
        assert_eq!(m.get(1).unwrap().commands, ["SIN"]);
        assert_eq!(m.get(1).unwrap().submenus, [("SUBM".to_string(), 2)]);
        assert_eq!(m.get(2).unwrap().commands, ["COS", "TAN"]);
        // The current-menu pointer designates an XLIB name of #A9.
        let mut ram = n.clone();
        put_hex(&mut ram, 0xF000, &xlib(0xA9, 2));
        struct Image<'a>(&'a [u8]);
        impl Memory for Image<'_> {
            fn nibble(&self, a: u32) -> Option<u8> {
                self.0.get(a as usize).copied()
            }
        }
        assert_eq!(m.number(0xF000, &Image(&ram)), Some(2));
    }

    #[test]
    fn garbage_finds_no_menus() {
        let mut n = vec![0u8; 0x8000];
        for (i, x) in n.iter_mut().enumerate() {
            *x = ((i * 7919) >> 3) as u8 & 0xF;
        }
        let names = NameTable::build(Model::Hp48sx, &n);
        assert!(read(Model::Hp48sx, &n, &names).is_none());
        // MENU whose code points at itself and at garbage: bounded, none.
        let mut n = vec![0u8; 0x8000];
        let a = builtins(&mut n, &format!("{}{}", ptr(0x500), ptr(0x7FF0)));
        let _ = a;
        let names = NameTable::build(Model::Hp48sx, &n);
        assert!(read(Model::Hp48sx, &n, &names).is_none());
        assert!(read(Model::Hp42s, &n, &NameTable::default()).is_none());
    }
}
