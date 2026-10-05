//! The calculator's user memory read straight from RAM
//! (`saturnus-objects`' [`UserMemory`]): the HOME tree, the current
//! directory, the stack and the flags, without the Kermit server and
//! without running the calculator. wiki: hardware/hp48-system-ram.

use anyhow::Result;
use saturnus_objects::{Flags, Object, UserMemory, Variable};
use serde::Serialize;

use crate::emulator::Emulator;

/// HOME's tree and the current directory.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemoryTree {
    /// The current directory, e.g. `["HOME", "A"]`.
    pub path: Vec<String>,
    /// HOME's variables, newest first, with every sub-directory's.
    pub variables: Vec<Variable>,
    /// The change counter as 16 hex digits: it moves whenever a variable,
    /// the current directory, the stack or a flag changes.
    pub changes: String,
}

impl Emulator {
    /// Run `f` on the user memory of the paused machine.
    fn user_memory<T>(&self, f: impl FnOnce(&UserMemory<'_>) -> Result<T>) -> Result<T> {
        self.with_machine(|m| f(&UserMemory::of(m)?))?
    }

    /// HOME's tree and the current directory, from RAM. Valid in and out of
    /// server mode.
    pub fn memory_tree(&self) -> Result<MemoryTree> {
        self.user_memory(|u| {
            Ok(MemoryTree {
                path: u.current_path()?,
                variables: u.tree()?,
                changes: format!("{:016X}", u.change_counter()?),
            })
        })
    }

    /// The flags, from RAM.
    pub fn ram_flags(&self) -> Result<Flags> {
        self.user_memory(|u| u.flags())
    }

    /// The data stack, level 1 first, from RAM. Only meaningful with the
    /// Kermit server stopped: inside the server the ROM's saved stack is
    /// the server's own.
    pub fn ram_stack(&self) -> Result<Vec<Object>> {
        self.user_memory(|u| u.stack())
    }

    /// The change counter (see `UserMemory::change_counter`).
    pub fn ram_changes(&self) -> Result<u64> {
        self.user_memory(|u| u.change_counter())
    }
}
