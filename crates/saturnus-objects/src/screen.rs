//! What has the keyboard: the stack (the ROM's top-level outer loop), an
//! open command line, a halted program, or an application's own loop (an
//! input form, a choose box, a message box, the graphics screen), read
//! from RAM without pressing a key; the display only where RAM gave no
//! sign (wiki: hardware/ui-state).
//!
//! The signals, observed on the 48SX (ROM J), 48GX (ROM R) and 49G (ROM
//! 2.10 and 1.19-6) while the ROM waits for a key:
//!
//! - The RPL return stack (between the saved `HeapStart` and the saved B):
//!   its bottom entries are the system outer loop's own frame; while the
//!   stack has the keyboard, the frame right under the key wait's four
//!   entries is that same outer-loop frame. Any other loop (a
//!   parameterized outer loop: forms, choose and message boxes, the
//!   graphics screen, `INPUT`, `WAIT`, the interactive stack; the 48's
//!   equation writer) puts its own frame there. A halted program leaves
//!   its frames on the return stack with a new outer loop above them, so
//!   each halted program adds one more copy of the outer-loop frame.
//! - The temporary environments (RPLMAN 13.1): input forms bind named
//!   temporaries (`IF` on the 48GX, `IfMenu`, `CurrentField`, ... on the
//!   49G), the 49G's choose box binds `NbElm` and others.
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
    /// Another loop reads the keys with the command line open: a program
    /// waits for input (INPUT), or an editor uses the line (the 49G's
    /// equation writer).
    Input,
    /// Another loop has the keyboard (the interactive stack, the equation
    /// or matrix writer, WAIT), or the state could not be read.
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
        let in_control = self.top(KEY_WAIT + 2)? == outer;
        Some((in_control, copies.saturating_sub(1)))
    }
}

/// The names of the temporaries bound in the newest environments, down to
/// and including the first that binds `SavedUI` (the parameterized outer
/// loop's saved state: what the innermost loop bound); only the newest
/// environment when none does. Names lose a leading `'`.
pub fn innermost_names(mem: &dyn Memory, layout: &ScreenLayout) -> Result<Vec<String>> {
    let mut envs: Vec<Vec<String>> = Vec::new();
    let mut a = field(mem, layout.environments_ptr, 5)?;
    for _ in 0..MAX_ENVIRONMENTS {
        let link = field(mem, a, 5)?;
        if link == 0 {
            break;
        }
        let mut names = Vec::new();
        let mut p = a + 10;
        let mut n = 0;
        while p + 10 <= a + link && n < MAX_BINDINGS {
            let name = field(mem, p, 5)?;
            if let Ok(Object::LocalName { value }) = decode_at(name, mem) {
                let value = value.trim_start_matches('\'');
                if !value.is_empty() {
                    names.push(value.to_string());
                }
            }
            p += 10;
            n += 1;
        }
        let saved = names.iter().any(|n| n == "SavedUI");
        envs.push(names);
        if saved {
            return Ok(envs.concat());
        }
        a = a.wrapping_add(link) & 0xFFFFF;
    }
    Ok(envs.into_iter().next().unwrap_or_default())
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
    let names = innermost_names(mem, layout)?;
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
