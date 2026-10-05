//! The command line being edited and the state of the ROM's editor (entry
//! mode, alpha, shifts), read from RAM without pressing a key.
//!
//! The ROM keeps the text reversed and NUL-terminated just below its
//! temporary environments, with no length field; the cursor and the
//! editor's flags sit in system RAM. Locations per model were found by
//! observation (wiki: hardware/command-line).

use anyhow::{Context, Result, ensure};
use saturnus::{Machine, Model};
use serde::Serialize;

use crate::charset;
use crate::object::Memory;

/// Most characters read (the 49G's RAM could hold more, but no line the
/// ROM edits is that long; more means the pointers are not the ROM's).
pub const MAX_CHARS: usize = 65_536;

/// Where a model's ROM keeps its editor in RAM (wiki:
/// hardware/command-line). `*_ptr` fields hold 5-nibble pointers; the
/// others are nibbles whose bits are listed with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorLayout {
    /// Pointer just past the text's first character (the temporary
    /// environments, `local_vars` in the 1991 address list).
    pub text_end_ptr: u32,
    /// Pointer past the data stack, the lowest the text may reach.
    pub stack_end_ptr: u32,
    /// The cursor, 5 nibbles, in characters from the start.
    pub cursor: u32,
    /// Bit 1 insert mode, bit 2 algebraic entry.
    pub entry: u32,
    /// Bit 1 a command line is open, bit 2 lowercase lock.
    pub line: u32,
    /// Bit 0 left shift, bit 1 right shift, bit 2 alpha.
    pub keys: u32,
    /// Bit 0 alpha lock.
    pub alpha_lock: u32,
    /// Bit 0 program entry.
    pub program: u32,
    /// The message nibble: on the 48 bit 0 is set while the header shows a
    /// message (an error); on the 49G it, or the nibble after it, reads
    /// #F while an error box has the keyboard.
    pub message: u32,
    /// Errors show in a box (49G), not in the header (48).
    pub message_box: bool,
    /// The byte the 49G's CHARS application opens on (it remembers the
    /// last one); `None` where CHARS always opens on 128 (48GX) or does
    /// not exist (48SX).
    pub chars_position: Option<u32>,
}

impl EditorLayout {
    /// 48SX (ROM J).
    pub const HP48SX: EditorLayout = EditorLayout {
        text_end_ptr: 0x70583,
        stack_end_ptr: 0x7057E,
        cursor: 0x70704,
        entry: 0x70685,
        line: 0x70687,
        keys: 0x706C4,
        alpha_lock: 0x70793,
        program: 0x70794,
        message: 0x7068B,
        message_box: false,
        chars_position: None,
    };
    /// 48GX (ROM R).
    pub const HP48GX: EditorLayout = EditorLayout {
        text_end_ptr: 0x80702,
        stack_end_ptr: 0x806FD,
        cursor: 0x80882,
        entry: 0x80803,
        line: 0x80805,
        keys: 0x80842,
        alpha_lock: 0x80911,
        program: 0x80912,
        message: 0x80809,
        message_box: false,
        chars_position: None,
    };
    /// 49G (ROM 2.10).
    pub const HP49G: EditorLayout = EditorLayout {
        text_end_ptr: 0x80702,
        stack_end_ptr: 0x806FD,
        cursor: 0x80F61,
        entry: 0x80EC2,
        line: 0x80EC4,
        keys: 0x80F01,
        alpha_lock: 0x80FF0,
        program: 0x80FF1,
        message: 0x80EC8,
        message_box: true,
        chars_position: Some(0x818CF),
    };

    /// The layout of `model`, or `None` for the aplet models and the 42S.
    pub fn of(model: Model) -> Option<EditorLayout> {
        match model {
            Model::Hp48sx => Some(Self::HP48SX),
            Model::Hp48gx => Some(Self::HP48GX),
            Model::Hp49g => Some(Self::HP49G),
            Model::Hp38g | Model::Hp39g | Model::Hp40g | Model::Hp42s => None,
        }
    }

    /// The layout of `model` or an error naming what the model lacks.
    pub fn require(model: Model) -> Result<EditorLayout> {
        Self::of(model).with_context(|| {
            format!(
                "the {} has no RPL command line to read or type into (48SX, 48GX and 49G only)",
                model.name().to_uppercase()
            )
        })
    }
}

/// The editor as the ROM left it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Editor {
    /// A command line is open (also an empty one, and in `EDIT`).
    pub active: bool,
    /// The text as HP bytes, empty when no line is open.
    pub text: Vec<u8>,
    /// The cursor in characters, 0 before the first.
    pub cursor: usize,
    /// Insert mode (off: typing replaces the character at the cursor).
    pub insert: bool,
    /// Algebraic entry mode (after `'`).
    pub algebraic: bool,
    /// Program entry mode (after `«`, or ENTRY).
    pub program: bool,
    /// Lowercase lock.
    pub lowercase: bool,
    /// Alpha is on (for one key, or locked).
    pub alpha: bool,
    /// Alpha is locked.
    pub alpha_lock: bool,
    /// The left shift is pending.
    pub left_shift: bool,
    /// The right shift is pending.
    pub right_shift: bool,
    /// The ROM shows an error: in the header (48) or in a box (49G).
    pub message: bool,
}

/// The `commandLine` protocol answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommandLine {
    /// A command line is open.
    pub active: bool,
    /// Its text (empty when none is open).
    pub text: String,
    /// The cursor in calculator characters (0 before the first).
    pub cursor: usize,
}

fn nibble(mem: &dyn Memory, addr: u32) -> Result<u8> {
    mem.nibble(addr)
        .with_context(|| format!("#{addr:05X} is not readable"))
}

fn field(mem: &dyn Memory, addr: u32, len: u32) -> Result<u32> {
    let mut v = 0;
    for i in (0..len).rev() {
        v = (v << 4) | u32::from(nibble(mem, addr + i)?);
    }
    Ok(v)
}

fn bit(mem: &dyn Memory, addr: u32, b: u8) -> Result<bool> {
    Ok(nibble(mem, addr)? & (1 << b) != 0)
}

/// Read the editor through `layout`.
pub fn read(mem: &dyn Memory, layout: &EditorLayout) -> Result<Editor> {
    let active = bit(mem, layout.line, 1)?;
    let mut text = Vec::new();
    if active {
        let end = field(mem, layout.text_end_ptr, 5)?;
        let floor = field(mem, layout.stack_end_ptr, 5)?;
        ensure!(
            floor < end,
            "the command line pointers (#{floor:05X}, #{end:05X}) are not the ROM's"
        );
        let mut a = end;
        while a >= floor + 2 {
            a -= 2;
            let b = field(mem, a, 2)? as u8;
            if b == 0 {
                break;
            }
            ensure!(
                text.len() < MAX_CHARS,
                "the command line is longer than {MAX_CHARS} characters"
            );
            text.push(b);
        }
    }
    let cursor = field(mem, layout.cursor, 5)? as usize;
    let keys = nibble(mem, layout.keys)?;
    let message = if layout.message_box {
        !active && (nibble(mem, layout.message)? == 0xF || nibble(mem, layout.message + 1)? == 0xF)
    } else {
        bit(mem, layout.message, 0)?
    };
    Ok(Editor {
        message,
        active,
        cursor: if active { cursor.min(text.len()) } else { 0 },
        text,
        insert: bit(mem, layout.entry, 1)?,
        algebraic: bit(mem, layout.entry, 2)?,
        program: bit(mem, layout.program, 0)?,
        lowercase: bit(mem, layout.line, 2)?,
        alpha: keys & 4 != 0,
        alpha_lock: bit(mem, layout.alpha_lock, 0)?,
        left_shift: keys & 1 != 0,
        right_shift: keys & 2 != 0,
    })
}

/// The editor of `machine` (48SX, 48GX, 49G).
pub fn editor(machine: &Machine) -> Result<Editor> {
    read(machine, &EditorLayout::require(machine.model())?)
}

/// The command line of `machine`: open or not, its text and cursor.
pub fn command_line(machine: &Machine) -> Result<CommandLine> {
    let e = editor(machine)?;
    Ok(CommandLine {
        active: e.active,
        text: charset::decode(&e.text),
        cursor: e.cursor,
    })
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
    }

    #[test]
    fn reads_the_reversed_text_and_the_flags() {
        let l = EditorLayout::HP48SX;
        let mut ram = Ram::default();
        // "ABCDEF" as the 48SX keeps it (observed: 645444342414 at #7FED7).
        ram.put(l.text_end_ptr, 0x7FEE3, 5);
        ram.put(l.stack_end_ptr, 0x7FEB3, 5);
        for (i, b) in b"FEDCBA".iter().enumerate() {
            ram.put(0x7FED7 + 2 * i as u32, u32::from(*b), 2);
        }
        ram.put(l.cursor, 4, 5);
        ram.put(l.line, 3, 1);
        ram.put(l.entry, 3, 1);
        ram.put(l.keys, 4, 1);
        let e = read(&ram, &l).unwrap();
        assert!(e.active && e.insert && !e.algebraic && e.alpha && !e.alpha_lock);
        assert_eq!(e.text, b"ABCDEF");
        assert_eq!(e.cursor, 4);
        // Closed: the stale text is not reported.
        ram.put(l.line, 1, 1);
        let e = read(&ram, &l).unwrap();
        assert!(!e.active && e.text.is_empty() && e.cursor == 0);
    }

    #[test]
    fn the_text_stops_at_the_stack() {
        let l = EditorLayout::HP48GX;
        let mut ram = Ram::default();
        ram.put(l.text_end_ptr, 0x90010, 5);
        ram.put(l.stack_end_ptr, 0x9000C, 5);
        ram.put(0x9000C, 0x4242, 4);
        ram.put(0x9000E, 0x41, 2);
        ram.put(l.line, 2, 1);
        assert_eq!(read(&ram, &l).unwrap().text, b"AB");
        ram.put(l.stack_end_ptr, 0x90020, 5);
        assert!(read(&ram, &l).is_err());
    }

    #[test]
    fn aplet_models_have_none() {
        assert!(EditorLayout::of(Model::Hp38g).is_none());
        let err = EditorLayout::require(Model::Hp42s).unwrap_err();
        assert!(err.to_string().contains("42S has no RPL command line"));
    }
}
