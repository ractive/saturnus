//! What has the keyboard: the stack (the ROM's top-level outer loop), an
//! open command line, a halted program, or an application's own loop (an
//! input form, a choose box, a message box, the graphics screen, the 48's
//! EquationWriter, the interactive stack, an object edited on the command
//! line, the MatrixWriter), read from RAM without pressing a key; the
//! display only where RAM gave no sign (wiki: hardware/ui-state).
//!
//! The signals, observed on the 48SX (ROM J), 48GX (ROM R) and 49G (ROM
//! 2.10 and 1.19-6) while the ROM waits for a key:
//!
//! - The RPL return stack (between the saved `HeapStart` and the saved B):
//!   its bottom entries are the system outer loop's own frame; while the
//!   stack has the keyboard, the sixth entry from the top (under the key
//!   wait's four entries and the calling loop's own return into it) is
//!   that same outer-loop frame. Any other loop (a
//!   parameterized outer loop: forms, choose and message boxes, the
//!   graphics screen, `INPUT`, `WAIT`, the interactive stack; the 48's
//!   equation writer) puts its own frame there. A halted program leaves
//!   its frames on the return stack with a new outer loop above them, so
//!   each halted program adds one more copy of the outer-loop frame.
//! - The temporary environments (RPLMAN 13.1): input forms bind named
//!   temporaries (`IF` on the 48GX, `IfMenu`, `CurrentField`, ... on the
//!   49G), the 49G's choose box binds `NbElm` and others. The 48SX's and
//!   48GX's EquationWriter binds `ploc`, `FontGauge` and others: newest
//!   in entry mode, under one environment binding only `SavedUI` in
//!   selection mode, under `SavedUI` and `SAVEDDEPTH EXITFCN EnvOK`
//!   (48SX: `EXITFCN EnvOK`) in scrolling mode. The 49G's equation writer
//!   binds only unnamed temporaries and edits on the command line: it
//!   reads as INPUT does.
//! - The types of the objects bound to unnamed temporaries (observed on
//!   48SX J, 48GX R, 49G 2.10 and 1.19-6):
//!   - The interactive stack binds a command and three system binaries
//!     (48, under `SavedUI`), or eight objects: a system binary, any, a
//!     command, a system binary, a command, a system binary, a string, a
//!     program (49G).
//!   - An object edited on the command line (▼, EDIT, VISIT, the
//!     interactive stack's EDIT, the 49G's TOOL EDIT) binds a command, a
//!     system binary, a command and a string, as `INPUT` does; the
//!     environment under it (under its `SavedUI` on the 48) binds the
//!     edited object alone. Under `INPUT` there is a named one, a system
//!     binary (its options) or the stack's own (a command).
//!   - The 48's MatrixWriter binds 22: a system binary, any, a command, a
//!     system binary, then others, under `SavedUI`; the 49G's names `_IDX`
//!     and `_TAB2`.
//! - The display: the stack display's GROB pointer equals the PICT
//!   pointer while the graphics screen shows.
//! - The 48GX's choose and message boxes bind only unnamed temporaries,
//!   so they are told apart on the display: a framed box with a two-dot
//!   shadow, a choose box with a dark (inverted) row inside it.

use saturnus_core::{Machine, Model};
use serde::Serialize;

use crate::cmdline::{self, EditorLayout};
use crate::error::Error;
use crate::object::{Memory, Object, decode_at};
use crate::prolog::ObjectType;
use crate::prolog::ObjectType::{Program, String as Str, SystemBinary as Bint};

type Result<T, E = Error> = std::result::Result<T, E>;

/// Most return-stack entries believed (more means the pointers are not
/// the ROM's).
const MAX_DEPTH: u32 = 4096;
/// Most temporary environments walked.
const MAX_ENVIRONMENTS: usize = 64;
/// Most temporaries read per environment.
const MAX_BINDINGS: u32 = 512;
/// Entries the key wait itself puts on the return stack, above the loop
/// that called it.
const KEY_WAIT: u32 = 4;
/// The entry from the top (1-based) that holds the calling loop's frame:
/// under the key wait's entries and the loop's own return into the wait.
const LOOP_FRAME: u32 = KEY_WAIT + 2;

/// Where a model's ROM keeps what the screen state is read from (wiki:
/// hardware/ui-state). The `*_ptr` fields hold 5-nibble pointers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenLayout {
    /// The bottom of the return stack (`HeapStart` in the 1991 address
    /// list).
    pub return_bottom_ptr: u32,
    /// The saved B, the return stack's top (`FramePtr`).
    pub return_top_ptr: u32,
    /// The newest temporary environment (`local_vars`).
    pub environments_ptr: u32,
    /// The GROB the display shows below the header (`Stk_56line_GROB_ptr`).
    pub shown_grob_ptr: u32,
    /// PICT (`graph_GROB_ptr`).
    pub pict_ptr: u32,
    /// Return-stack entries at the stack with no program halted.
    pub base_depth: u32,
    /// Which entry from the bottom (1 = the bottom one) is the outer
    /// loop's frame.
    pub outer_frame: u32,
}

impl ScreenLayout {
    /// 48SX (ROM J): the 1991 address list's locations.
    pub const HP48SX: ScreenLayout = ScreenLayout {
        return_bottom_ptr: 0x7056F,
        return_top_ptr: 0x70574,
        environments_ptr: 0x70583,
        shown_grob_ptr: 0x7055B,
        pict_ptr: 0x70565,
        base_depth: 8,
        outer_frame: 3,
    };
    /// 48GX (ROM R): the 48SX's plus #1017F.
    pub const HP48GX: ScreenLayout = ScreenLayout {
        return_bottom_ptr: 0x806EE,
        return_top_ptr: 0x806F3,
        environments_ptr: 0x80702,
        shown_grob_ptr: 0x806DA,
        pict_ptr: 0x806E4,
        base_depth: 8,
        outer_frame: 3,
    };
    /// 49G (ROM 2.10): the 48GX's addresses; one more entry at the bottom.
    pub const HP49G: ScreenLayout = ScreenLayout {
        base_depth: 9,
        outer_frame: 4,
        ..Self::HP48GX
    };

    /// The layout of `model`, or `None` where there is none (as
    /// [`EditorLayout::of`]).
    pub fn of(model: Model) -> Option<ScreenLayout> {
        match model {
            Model::Hp48sx => Some(Self::HP48SX),
            Model::Hp48gx => Some(Self::HP48GX),
            Model::Hp49g => Some(Self::HP49G),
            Model::Hp38g
            | Model::Hp39g
            | Model::Hp40g
            | Model::Hp42s
            | Model::Hp27s
            | Model::Hp17bii
            | Model::Hp17b
            | Model::Hp28s
            | Model::Hp19bii => None,
        }
    }
}

/// What has the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// The stack, no command line open, no program halted.
    Stack,
    /// The stack with a command line open.
    CommandLine,
    /// The stack, with one or more programs halted (HALT, PROMPT).
    Halted,
    /// An input form (48GX, 49G).
    InputForm,
    /// A choose box.
    ChooseBox,
    /// A message box (MSGBOX, the 49G's error box).
    Message,
    /// The graphics screen (PICTURE, PVIEW, a plot).
    Graphics,
    /// The 48SX's or 48GX's EquationWriter in entry or selection mode.
    /// Its working objects lie on the stack above the user's levels while
    /// it is open.
    EquationWriter,
    /// The 48SX's or 48GX's EquationWriter in scrolling mode (▼ on an
    /// algebraic at the stack, or PICTURE in the writer): ENTER does
    /// nothing there, CANCEL returns to the writer.
    EquationView,
    /// The interactive stack (▲ at the stack; the 49G's HIST): ▲▼ move
    /// its pointer, the menu keys act on the level it points at, ENTER or
    /// CANCEL leaves and keeps what was done.
    InteractiveStack,
    /// An object edited on the command line inside another loop (▼ on a
    /// stack level, EDIT, VISIT, the interactive stack's EDIT, the 49G's
    /// TOOL EDIT): ENTER puts the edited object back, CANCEL restores the
    /// original. The object is off the stack meanwhile.
    ObjectEdit,
    /// The MatrixWriter (◀MATRIX, the 49G's MTRW, ▼ on a matrix), also
    /// with a cell being typed ([`Screen::command_line`]).
    MatrixWriter,
    /// Another loop reads the keys with the command line open: a program
    /// waits for input (INPUT), or an editor uses the line (the 49G's
    /// equation writer).
    Input,
    /// Another loop has the keyboard (WAIT, a catalog or browser), or the
    /// state could not be read.
    Unknown,
}

/// Where a [`Screen::kind`] came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// RAM.
    Memory,
    /// The display's dots.
    Display,
}

/// The `uiState` protocol answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    /// What has the keyboard.
    pub kind: Kind,
    /// Where `kind` came from.
    pub from: Source,
    /// The stack's own loop reads the keys (also with a command line open
    /// or a program halted).
    pub stack_in_control: bool,
    /// Programs halted (HALT, PROMPT), waiting for CONT or KILL.
    pub halted: u32,
    /// The editor's command line is open (at the stack, in a form's
    /// field, or for INPUT).
    pub command_line: bool,
}

fn field(mem: &dyn Memory, addr: u32, len: u32) -> Result<u32> {
    let mut v = 0;
    for i in (0..len).rev() {
        v = (v << 4) | u32::from(mem.nibble(addr + i).ok_or(Error::Unreadable(addr + i))?);
    }
    Ok(v)
}

/// The return stack as the ROM left it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReturnStack {
    /// The entries, top (most recent) first.
    pub entries: Vec<u32>,
}

impl ReturnStack {
    /// Read it through `layout`; `None` when the pointers are not the
    /// ROM's.
    pub fn read(mem: &dyn Memory, layout: &ScreenLayout) -> Result<Option<ReturnStack>> {
        let bottom = field(mem, layout.return_bottom_ptr, 5)?;
        let top = field(mem, layout.return_top_ptr, 5)?;
        if top <= bottom || (top - bottom) % 5 != 0 || (top - bottom) / 5 > MAX_DEPTH {
            return Ok(None);
        }
        let depth = (top - bottom) / 5;
        let mut entries = Vec::with_capacity(depth as usize);
        for i in 1..=depth {
            entries.push(field(mem, top - 5 * i, 5)?);
        }
        Ok(Some(ReturnStack { entries }))
    }

    /// Entry `i` from the top, 1-based.
    fn top(&self, i: u32) -> Option<u32> {
        self.entries.get(i.checked_sub(1)? as usize).copied()
    }

    /// Entry `j` from the bottom, 1-based.
    fn bottom(&self, j: u32) -> Option<u32> {
        let n = self.entries.len();
        self.entries.get(n.checked_sub(j as usize)?).copied()
    }

    /// Whether the stack's outer loop reads the keys, and how many
    /// programs are halted below it; `None` when the entries do not have
    /// the outer loop's shape at the bottom.
    pub fn control(&self, layout: &ScreenLayout) -> Option<(bool, u32)> {
        if (self.entries.len() as u32) < layout.base_depth || self.bottom(1)? != 0 {
            return None;
        }
        let outer = self.bottom(layout.outer_frame)?;
        if outer == 0 {
            return None;
        }
        let copies = self.entries.iter().filter(|&&e| e == outer).count() as u32;
        let in_control = self.top(LOOP_FRAME)? == outer;
        Some((in_control, copies.saturating_sub(1)))
    }
}

/// One temporary environment's bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Environment {
    /// The names it binds, in order, without a leading `'`.
    pub names: Vec<String>,
    /// The bindings with no name (`NULLLAM`, or a name that does not
    /// decode).
    pub unnamed: u32,
    /// The type of each bound object, every binding in memory order (named
    /// or not): `None` for one without an object prolog (a command: a
    /// primitive or a word in ROM).
    pub types: Vec<Option<ObjectType>>,
}

impl Environment {
    /// Whether it binds `name`.
    pub fn binds(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }

    /// The types of its objects when it binds no names, else `None`.
    fn unnamed_types(&self) -> Option<&[Option<ObjectType>]> {
        self.names.is_empty().then_some(self.types.as_slice())
    }

    /// Whether it binds `SavedUI` and nothing else (the 48's
    /// parameterized outer loop's saved state).
    fn only_saved_ui(&self) -> bool {
        self.names == ["SavedUI"] && self.unnamed == 0
    }
}

/// The temporary environments, newest first.
pub fn environments(mem: &dyn Memory, layout: &ScreenLayout) -> Result<Vec<Environment>> {
    let mut envs = Vec::new();
    let mut a = field(mem, layout.environments_ptr, 5)?;
    for _ in 0..MAX_ENVIRONMENTS {
        let link = field(mem, a, 5)?;
        if link == 0 {
            break;
        }
        let mut env = Environment::default();
        let mut p = a + 10;
        let mut n = 0;
        while p + 10 <= a + link && n < MAX_BINDINGS {
            let name = field(mem, p, 5)?;
            match decode_at(name, mem) {
                Ok(Object::LocalName { value }) if !value.trim_start_matches('\'').is_empty() => {
                    env.names.push(value.trim_start_matches('\'').to_string());
                }
                _ => env.unnamed += 1,
            }
            let object = field(mem, p + 5, 5)?;
            env.types
                .push(field(mem, object, 5).ok().and_then(ObjectType::from_prolog));
            p += 10;
            n += 1;
        }
        envs.push(env);
        a = a.wrapping_add(link) & 0xFFFFF;
    }
    Ok(envs)
}

/// The names of the temporaries bound in the newest environments, down to
/// and including the first that binds `SavedUI` (the parameterized outer
/// loop's saved state: what the innermost loop bound); only the newest
/// environment when none does. Names lose a leading `'`.
pub fn innermost_names(mem: &dyn Memory, layout: &ScreenLayout) -> Result<Vec<String>> {
    Ok(innermost(&environments(mem, layout)?))
}

fn innermost(envs: &[Environment]) -> Vec<String> {
    match envs.iter().position(|e| e.binds("SavedUI")) {
        Some(i) => envs[..=i].iter().flat_map(|e| e.names.clone()).collect(),
        None => envs.first().map(|e| e.names.clone()).unwrap_or_default(),
    }
}

/// What the environments above a scrolling EquationWriter bind (wiki:
/// hardware/ui-state).
const VIEW_NAMES: [&str; 4] = ["SavedUI", "SAVEDDEPTH", "EXITFCN", "EnvOK"];

/// The 48SX's or 48GX's EquationWriter from the environments, newest
/// first: [`Kind::EquationWriter`] when its own environment (`ploc`,
/// `FontGauge`) is the newest or only `SavedUI` ones are above it,
/// [`Kind::EquationView`] when the scrolling mode's are; `None` when it
/// is not open or another loop (a box, a form) is above it.
fn equation_writer(envs: &[Environment]) -> Option<Kind> {
    let at = envs
        .iter()
        .position(|e| e.binds("ploc") && e.binds("FontGauge"))?;
    let above = &envs[..at];
    if above.is_empty() {
        return Some(Kind::EquationWriter);
    }
    if above.iter().any(|e| e.unnamed > 0 || e.names.is_empty()) {
        return None;
    }
    let only = |allowed: &[&str]| {
        above
            .iter()
            .all(|e| e.names.iter().all(|n| allowed.contains(&n.as_str())))
    };
    if only(&["SavedUI"]) {
        return Some(Kind::EquationWriter);
    }
    if only(&VIEW_NAMES) && above.iter().any(|e| e.binds("EnvOK")) {
        return Some(Kind::EquationView);
    }
    None
}

/// The interactive stack from the newest environment (wiki:
/// hardware/ui-state): the 48's binds a command and three system
/// binaries under one binding only `SavedUI`; the 49G's eight objects.
fn interactive_stack(envs: &[Environment]) -> bool {
    let Some(types) = envs.first().and_then(Environment::unnamed_types) else {
        return false;
    };
    match types {
        [None, Some(Bint), Some(Bint), Some(Bint)] => {
            envs.get(1).is_some_and(Environment::only_saved_ui)
        }
        [
            Some(Bint),
            _,
            None,
            Some(Bint),
            None,
            Some(Bint),
            Some(Str),
            Some(Program),
        ] => true,
        _ => false,
    }
}

/// An object edited on the command line inside another loop (wiki:
/// hardware/ui-state): the newest environment binds a command, a system
/// binary, a command and a string, and the next one (past one binding
/// only `SavedUI`, on the 48) binds one unnamed object, the edited one.
/// `INPUT` binds the same four; under them it has a named environment, a
/// system binary (its options) or the stack's own (a command).
fn object_edit(envs: &[Environment]) -> bool {
    let Some(types) = envs.first().and_then(Environment::unnamed_types) else {
        return false;
    };
    if !matches!(types, [None, Some(Bint), None, Some(Str)]) {
        return false;
    }
    let mut below = &envs[1..];
    if below.first().is_some_and(Environment::only_saved_ui) {
        below = &below[1..];
    }
    below
        .first()
        .and_then(Environment::unnamed_types)
        .is_some_and(|t| matches!(t, [Some(edited)] if *edited != Bint))
}

/// The MatrixWriter from the newest environment (wiki:
/// hardware/ui-state): the 49G's names `_IDX` and `_TAB2`; the 48's binds
/// 22 unnamed objects, starting with a system binary, any, a command and a
/// system binary, under one binding only `SavedUI`.
fn matrix_writer(envs: &[Environment]) -> bool {
    let Some(newest) = envs.first() else {
        return false;
    };
    if newest.binds("_IDX") && newest.binds("_TAB2") {
        return true;
    }
    newest
        .unnamed_types()
        .is_some_and(|t| t.len() == 22 && matches!(t, [Some(Bint), _, None, Some(Bint), ..]))
        && envs.get(1).is_some_and(Environment::only_saved_ui)
}

/// A framed box with a two-dot shadow on the display (the 48GX's and the
/// 49G's message and choose boxes): `Some(true)` when a row inside it is
/// dark (a choose box's highlighted item or title rule), `Some(false)`
/// without, `None` when there is no such box.
pub fn framed_box(pixels: &[Vec<bool>]) -> Option<bool> {
    let h = pixels.len();
    let dark = |x: usize, y: usize| {
        pixels
            .get(y)
            .and_then(|r| r.get(x))
            .copied()
            .unwrap_or(false)
    };
    for (y0, row) in pixels.iter().enumerate().take(h.saturating_sub(4)) {
        let w = row.len();
        let mut x = 0;
        while x < w {
            if !dark(x, y0) {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < w && dark(x, y0) {
                x += 1;
            }
            let x1 = x - 1;
            if x1 < x0 + 30 {
                continue;
            }
            // The bottom border: the first row under the top whose dots
            // run from the left edge to the shadow.
            let Some(y1) =
                (y0 + 3..h.saturating_sub(1)).find(|&y| (x0..=x1 + 1).all(|x| dark(x, y)))
            else {
                continue;
            };
            let edges = (y0 + 1..y1)
                .all(|y| dark(x0, y) && dark(x1, y) && dark(x1 + 1, y) && !dark(x0 + 1, y));
            let shadow = (x0 + 1..=x1 + 1).all(|x| dark(x, y1 + 1)) && !dark(x0, y1 + 1);
            if !edges || !shadow {
                continue;
            }
            let inner = (x0 + 2..=x1.saturating_sub(2)).count().max(1);
            let banded = (y0 + 1..y1).any(|y| {
                (x0 + 2..=x1.saturating_sub(2))
                    .filter(|&x| dark(x, y))
                    .count()
                    * 10
                    >= inner * 9
            });
            return Some(banded);
        }
    }
    None
}

/// Read the screen state through the layouts, with `pixels` the display
/// for the boxes RAM does not name.
pub fn read(
    mem: &dyn Memory,
    layout: &ScreenLayout,
    editor: &EditorLayout,
    pixels: &[Vec<bool>],
) -> Result<Screen> {
    let command_line = cmdline::read(mem, editor)?.active;
    let unknown = Screen {
        kind: Kind::Unknown,
        from: Source::Memory,
        stack_in_control: false,
        halted: 0,
        command_line,
    };
    let Some(rs) = ReturnStack::read(mem, layout)? else {
        return Ok(unknown);
    };
    let Some((in_control, halted)) = rs.control(layout) else {
        return Ok(unknown);
    };
    let at = |kind, from| Screen {
        kind,
        from,
        stack_in_control: in_control,
        halted,
        command_line,
    };
    if in_control {
        let kind = if command_line {
            Kind::CommandLine
        } else if halted > 0 {
            Kind::Halted
        } else {
            Kind::Stack
        };
        return Ok(at(kind, Source::Memory));
    }
    if field(mem, layout.shown_grob_ptr, 5)? == field(mem, layout.pict_ptr, 5)? {
        return Ok(at(Kind::Graphics, Source::Memory));
    }
    let envs = environments(mem, layout)?;
    let names = innermost(&envs);
    let has = |n: &str| names.iter().any(|m| m == n);
    if has("IF") || has("IfMenu") || has("CurrentField") {
        return Ok(at(Kind::InputForm, Source::Memory));
    }
    if has("NbElm") {
        return Ok(at(Kind::ChooseBox, Source::Memory));
    }
    if has("PlotEnv") {
        return Ok(at(Kind::Graphics, Source::Memory));
    }
    match framed_box(pixels) {
        Some(true) => return Ok(at(Kind::ChooseBox, Source::Display)),
        Some(false) => return Ok(at(Kind::Message, Source::Display)),
        None => {}
    }
    // The 48's writer never has the command line open itself: an open
    // line above it (EDIT in selection mode) is an editor of its own.
    if !command_line && let Some(kind) = equation_writer(&envs) {
        return Ok(at(kind, Source::Memory));
    }
    // A cell typed into keeps the MatrixWriter's own environment newest.
    if matrix_writer(&envs) {
        return Ok(at(Kind::MatrixWriter, Source::Memory));
    }
    if !command_line && interactive_stack(&envs) {
        return Ok(at(Kind::InteractiveStack, Source::Memory));
    }
    if command_line && object_edit(&envs) {
        return Ok(at(Kind::ObjectEdit, Source::Memory));
    }
    if command_line {
        return Ok(at(Kind::Input, Source::Memory));
    }
    Ok(at(Kind::Unknown, Source::Memory))
}

/// The screen state of `machine` (48SX, 48GX, 49G).
pub fn screen(machine: &Machine) -> Result<Screen> {
    let model = machine.model();
    let editor = EditorLayout::require(model)?;
    let layout = ScreenLayout::of(model).ok_or(Error::NoCommandLine(model))?;
    read(machine, &layout, &editor, &machine.lcd().pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Ram(HashMap<u32, u8>);

    impl Memory for Ram {
        fn nibble(&self, addr: u32) -> Option<u8> {
            Some(self.0.get(&addr).copied().unwrap_or(0))
        }
    }

    impl Ram {
        fn put(&mut self, addr: u32, value: u32, len: u32) {
            for i in 0..len {
                self.0.insert(addr + i, ((value >> (4 * i)) & 15) as u8);
            }
        }

        /// A return stack, bottom entry first, at #71000.
        fn returns(&mut self, l: &ScreenLayout, bottom_first: &[u32]) {
            self.put(l.return_bottom_ptr, 0x71000, 5);
            for (i, e) in bottom_first.iter().enumerate() {
                self.put(0x71000 + 5 * i as u32, *e, 5);
            }
            self.put(l.return_top_ptr, 0x71000 + 5 * bottom_first.len() as u32, 5);
        }
    }

    const KEYS: [u32; 4] = [0x4204B, 0x420AF, 0x42190, 0x421C2];

    fn stack(extra: &[u32]) -> Vec<u32> {
        let mut v = vec![0, 0x38598, 0x385B6];
        v.extend_from_slice(extra);
        v.extend_from_slice(&KEYS);
        v
    }

    fn read_sx(ram: &Ram) -> Screen {
        read(ram, &ScreenLayout::HP48SX, &EditorLayout::HP48SX, &[]).unwrap()
    }

    #[test]
    fn the_return_stack_tells_the_stack_from_a_loop_and_counts_halts() {
        // The 48SX's return stack as observed (wiki: hardware/ui-state).
        let l = ScreenLayout::HP48SX;
        let mut ram = Ram::default();
        ram.put(l.shown_grob_ptr, 0x86907, 5);
        ram.put(l.pict_ptr, 0xCE017, 5);
        ram.returns(&l, &stack(&[0x385CA]));
        let s = read_sx(&ram);
        assert_eq!(
            (s.kind, s.stack_in_control, s.halted),
            (Kind::Stack, true, 0)
        );
        // A halted program: its frames, then a new outer loop.
        ram.returns(
            &l,
            &stack(&[0x385D4, 0x3A807, 0x40BC4, 0x143DE, 0x385B6, 0x385CA]),
        );
        let s = read_sx(&ram);
        assert_eq!(
            (s.kind, s.stack_in_control, s.halted),
            (Kind::Halted, true, 1)
        );
        // A parameterized outer loop (the interactive stack): no names, no box.
        ram.returns(
            &l,
            &stack(&[
                0x385D4, 0x40DBB, 0x43757, 0x4374D, 0x38AF5, 0x38B3B, 0x38B22,
            ]),
        );
        let s = read_sx(&ram);
        assert_eq!((s.kind, s.stack_in_control), (Kind::Unknown, false));
        // With the command line open in that loop: INPUT.
        ram.put(EditorLayout::HP48SX.line, 2, 1);
        ram.put(EditorLayout::HP48SX.text_end_ptr, 0x7FEE3, 5);
        ram.put(EditorLayout::HP48SX.stack_end_ptr, 0x7FEB3, 5);
        assert_eq!(read_sx(&ram).kind, Kind::Input);
        // The graphics screen: the shown GROB is PICT.
        ram.put(l.shown_grob_ptr, 0xCE017, 5);
        assert_eq!(read_sx(&ram).kind, Kind::Graphics);
        // Pointers that are not the ROM's.
        ram.put(l.return_top_ptr, 0x70FFF, 5);
        assert_eq!(read_sx(&ram).kind, Kind::Unknown);
    }

    #[test]
    fn named_temporaries_name_the_loop() {
        let l = ScreenLayout::HP48GX;
        let mut ram = Ram::default();
        ram.returns(&l, &stack(&[0x385D4, 0x38AF5, 0x38B3B, 0x38B22]));
        ram.put(l.shown_grob_ptr, 1, 5);
        // Local names: prolog #02E6D, length, characters.
        let lam = |ram: &mut Ram, at: u32, name: &str| {
            ram.put(at, 0x02E6D, 5);
            ram.put(at + 5, name.len() as u32, 2);
            for (i, b) in name.bytes().enumerate() {
                ram.put(at + 7 + 2 * i as u32, u32::from(b), 2);
            }
        };
        lam(&mut ram, 0x90000, "IF");
        lam(&mut ram, 0x90100, "SavedUI");
        // Two environments of one binding each, then the end.
        ram.put(l.environments_ptr, 0xA0000, 5);
        ram.put(0xA0000, 20, 5);
        ram.put(0xA000A, 0x90000, 5);
        ram.put(0xA0014, 20, 5);
        ram.put(0xA001E, 0x90100, 5);
        assert_eq!(innermost_names(&ram, &l).unwrap(), ["IF", "SavedUI"]);
        let s = read(&ram, &l, &EditorLayout::HP48GX, &[]).unwrap();
        assert_eq!((s.kind, s.from), (Kind::InputForm, Source::Memory));
        // A box opened over the form binds its own environment above:
        // the form's names are below its SavedUI.
        ram.put(l.environments_ptr, 0xA1000, 5);
        ram.put(0xA1000, 20, 5);
        ram.put(0xA100A, 0x90100, 5);
        ram.put(0xA1014, 0, 5);
        assert_eq!(innermost_names(&ram, &l).unwrap(), ["SavedUI"]);
    }

    /// A loop that is not the stack's (no box on the display), with the
    /// environments `envs`, newest first, each a list of bindings: a name,
    /// optionally `:` and the bound object's type (`bint`, `real`, `str`,
    /// `prog`, `arr`, `list`, `name`, `hxs`; else a command), or `:type`
    /// alone for an unnamed one (`""`: an unnamed command).
    fn in_loop(l: &ScreenLayout, envs: &[&[&str]]) -> Ram {
        let mut ram = Ram::default();
        ram.returns(l, &stack(&[0x385D4, 0x38AF5, 0x38B3B, 0x38B22]));
        ram.put(l.shown_grob_ptr, 1, 5);
        let mut names = 0x90000;
        let mut objects = 0xB0000;
        let mut a = 0xA0000;
        ram.put(l.environments_ptr, a, 5);
        for env in envs {
            let link = 10 + 10 * env.len() as u32;
            ram.put(a, link, 5);
            for (i, binding) in env.iter().enumerate() {
                let (name, ty) = binding.split_once(':').unwrap_or((binding, ""));
                // A local name: prolog #02E6D, length, characters.
                ram.put(names, 0x02E6D, 5);
                ram.put(names + 5, name.len() as u32, 2);
                for (j, b) in name.bytes().enumerate() {
                    ram.put(names + 7 + 2 * j as u32, u32::from(b), 2);
                }
                ram.put(a + 10 + 10 * i as u32, names, 5);
                names += 0x100;
                let prolog = match ty {
                    "bint" => 0x02911,
                    "real" => 0x02933,
                    "str" => 0x02A2C,
                    "prog" => 0x02D9D,
                    "arr" => 0x029E8,
                    "list" => 0x02A74,
                    "name" => 0x02E48,
                    "hxs" => 0x02A4E,
                    _ => 0x0312B,
                };
                ram.put(objects, prolog, 5);
                ram.put(a + 15 + 10 * i as u32, objects, 5);
                objects += 0x40;
            }
            a += link;
        }
        ram.put(a, 0, 5);
        ram
    }

    /// Opens the command line on [`in_loop`]'s RAM: its text ends where
    /// the environments start (the 48GX's text end pointer is theirs).
    fn open_line(ram: &mut Ram, e: &EditorLayout) {
        ram.put(e.line, 2, 1);
        ram.put(e.stack_end_ptr, 0x9FFD0, 5);
    }

    const WRITER: &[&str] = &["ploc", "FontGauge", "cx", "cy"];

    fn kind_gx(envs: &[&[&str]]) -> Kind {
        let l = ScreenLayout::HP48GX;
        read(&in_loop(&l, envs), &l, &EditorLayout::HP48GX, &[])
            .unwrap()
            .kind
    }

    #[test]
    fn the_equation_writers_environments_name_its_modes() {
        // Entry mode: the writer's own environment is the newest.
        assert_eq!(kind_gx(&[WRITER, &["SavedUI"]]), Kind::EquationWriter);
        // Selection mode: one environment binding only SavedUI above it.
        assert_eq!(
            kind_gx(&[&["SavedUI"], WRITER, &["SavedUI"]]),
            Kind::EquationWriter
        );
        // Scrolling mode: the 48GX's and the 48SX's.
        assert_eq!(
            kind_gx(&[
                &["SavedUI"],
                &["SAVEDDEPTH", "EXITFCN", "EnvOK"],
                WRITER,
                &["SavedUI"]
            ]),
            Kind::EquationView
        );
        assert_eq!(
            kind_gx(&[&["SavedUI"], &["EXITFCN", "EnvOK"], WRITER]),
            Kind::EquationView
        );
        // A message box over the writer binds unnamed temporaries: not
        // selection mode.
        assert_eq!(
            kind_gx(&[&["SavedUI"], &["", ""], WRITER, &["SavedUI"]]),
            Kind::Unknown
        );
        // Other names above the writer: not its own loop.
        assert_eq!(
            kind_gx(&[&["SavedUI"], &["SAVEDDEPTH", "EXITFCN"], WRITER]),
            Kind::Unknown
        );
        // A form opened over the writer names itself.
        assert_eq!(kind_gx(&[&["IF", "SavedUI"], WRITER]), Kind::InputForm);
        // Without the writer's names: nothing to tell.
        assert_eq!(kind_gx(&[&["SavedUI"]]), Kind::Unknown);
        // A command line open above the writer (EDIT in selection mode):
        // an editor of its own, not the writer.
        let l = ScreenLayout::HP48GX;
        let e = EditorLayout::HP48GX;
        let mut ram = in_loop(&l, &[&["SavedUI"], WRITER, &["SavedUI"]]);
        open_line(&mut ram, &e);
        let s = read(&ram, &l, &e, &[]).unwrap();
        assert_eq!((s.kind, s.command_line), (Kind::Input, true));
        let l = ScreenLayout::HP48GX;
        let ram = in_loop(&l, &[&["SavedUI", ""], WRITER]);
        assert_eq!(
            environments(&ram, &l).unwrap()[0],
            Environment {
                names: vec!["SavedUI".into()],
                unnamed: 1,
                types: vec![None, None],
            }
        );
    }

    /// Reads `envs` on the 48GX with the command line open or not.
    fn kind_with_line(envs: &[&[&str]], line: bool) -> Screen {
        let l = ScreenLayout::HP48GX;
        let e = EditorLayout::HP48GX;
        let mut ram = in_loop(&l, envs);
        if line {
            open_line(&mut ram, &e);
        }
        read(&ram, &l, &e, &[]).unwrap()
    }

    /// The bindings as observed (wiki: hardware/ui-state).
    const ISTACK_48: &[&str] = &["", ":bint", ":bint", ":bint"];
    const ISTACK_49: &[&str] = &[":bint", ":hxs", "", ":bint", "", ":bint", ":str", ":prog"];
    const LINE: &[&str] = &["", ":bint", "", ":str"];
    const SAVED: &[&str] = &["SavedUI:list"];
    const LAST_STACK: &[&str] = &["", ":real", ":real"];

    fn mw_48() -> Vec<&'static str> {
        let mut v = vec![":bint", ":hxs", "", ":bint"];
        v.resize(21, ":bint");
        v.push(":arr");
        v
    }

    #[test]
    fn the_interactive_stack_reads_from_its_bindings() {
        let k = |envs: &[&[&str]]| kind_with_line(envs, false).kind;
        assert_eq!(k(&[ISTACK_48, SAVED, LAST_STACK]), Kind::InteractiveStack);
        assert_eq!(k(&[ISTACK_49, LAST_STACK]), Kind::InteractiveStack);
        // The 48's without the loop's SavedUI under it: not told.
        assert_eq!(k(&[ISTACK_48, LAST_STACK]), Kind::Unknown);
        // Other types: the 49G's viewer starts with two commands.
        assert_eq!(
            k(&[&["", "", ":str", "", "", ":bint", ":bint", ""], ISTACK_49]),
            Kind::Unknown
        );
        // With a name among them: not the interactive stack.
        assert_eq!(
            k(&[&["x", ":bint", ":bint", ":bint"], SAVED]),
            Kind::Unknown
        );
        // A command line open over it is an edit or an editor.
        assert_eq!(kind_with_line(&[ISTACK_48, SAVED], true).kind, Kind::Input);
    }

    #[test]
    fn an_edited_object_is_told_from_input() {
        let k = |envs: &[&[&str]]| kind_with_line(envs, true).kind;
        // The 48: ▼ on a real, then VISIT (a name), then EDIT from the
        // interactive stack.
        assert_eq!(k(&[LINE, SAVED, &[":real"], LAST_STACK]), Kind::ObjectEdit);
        assert_eq!(k(&[LINE, SAVED, &[":name"], &[""]]), Kind::ObjectEdit);
        assert_eq!(
            k(&[LINE, SAVED, &[":real"], &[":bint"], ISTACK_48, SAVED]),
            Kind::ObjectEdit
        );
        // The 49G binds no SavedUI.
        assert_eq!(k(&[LINE, &[":str"], LAST_STACK]), Kind::ObjectEdit);
        // INPUT: from the command line ('est), with options (a system
        // binary), with local variables, from a menu key (the stack's own
        // environment, a command).
        assert_eq!(k(&[LINE, SAVED, &["est"], &[""]]), Kind::Input);
        assert_eq!(k(&[LINE, SAVED, &[":bint"], &["est"]]), Kind::Input);
        assert_eq!(k(&[LINE, SAVED, &["a:real"], &["est"]]), Kind::Input);
        assert_eq!(k(&[LINE, SAVED, &[""]]), Kind::Input);
        assert_eq!(k(&[LINE, &[""]]), Kind::Input);
        assert_eq!(k(&[LINE]), Kind::Input);
        // EDIT in the 48's EquationWriter: another SavedUI under it.
        assert_eq!(
            k(&[LINE, SAVED, SAVED, &["ploc", "FontGauge"], SAVED]),
            Kind::Input
        );
        // Without the command line open: not an edit.
        assert_eq!(
            kind_with_line(&[LINE, SAVED, &[":real"]], false).kind,
            Kind::Unknown
        );
    }

    #[test]
    fn the_matrix_writer_reads_from_its_bindings() {
        let mw = mw_48();
        let k = |envs: &[&[&str]], line| kind_with_line(envs, line);
        let s = k(&[&mw, SAVED, &[""]], false);
        assert_eq!((s.kind, s.command_line), (Kind::MatrixWriter, false));
        // A cell being typed: still the MatrixWriter.
        let s = k(&[&mw, SAVED, &[":arr"], LAST_STACK], true);
        assert_eq!((s.kind, s.command_line), (Kind::MatrixWriter, true));
        // Over the 48GX's statistics form: the form is under its SavedUI.
        assert_eq!(
            k(&[&mw, SAVED, &[""; 40], &["IF"], SAVED], false).kind,
            Kind::MatrixWriter
        );
        // The 49G's names.
        assert_eq!(
            k(
                &[&["", "", ":bint", "_IDX:str", "_TAB2", ":list"], &[""]],
                true
            )
            .kind,
            Kind::MatrixWriter
        );
        // 21 bindings, or without SavedUI under it on the 48: not told.
        assert_eq!(k(&[&mw[..21], SAVED], false).kind, Kind::Unknown);
        assert_eq!(k(&[&mw, &[""]], false).kind, Kind::Unknown);
    }

    fn boxed(band: bool) -> Vec<Vec<bool>> {
        let mut p = vec![vec![false; 131]; 64];
        let (x0, x1, y0, y1) = (17, 112, 15, 44);
        p[y0][x0..=x1].fill(true);
        for row in p.iter_mut().take(y1).skip(y0 + 1) {
            row[x0] = true;
            row[x1] = true;
            row[x1 + 1] = true;
        }
        p[y1][x0..=x1 + 1].fill(true);
        p[y1 + 1][x0 + 1..=x1 + 1].fill(true);
        if band {
            p[20][x0 + 2..x1 - 1].fill(true);
        }
        p
    }

    #[test]
    fn a_shadowed_frame_is_a_box() {
        assert_eq!(framed_box(&boxed(false)), Some(false));
        assert_eq!(framed_box(&boxed(true)), Some(true));
        assert_eq!(framed_box(&vec![vec![false; 131]; 64]), None);
        // A filled area is not a frame.
        assert_eq!(framed_box(&vec![vec![true; 131]; 64]), None);
    }
}
