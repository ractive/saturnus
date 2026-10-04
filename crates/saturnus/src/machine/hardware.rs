//! Everything outside the CPU on an HP48 SX, wired up as the CPU's [`Bus`]:
//! memory controller, system ROM, built-in RAM, I/O registers and keyboard.

use crate::bus::{Chip, MemoryController, Select};
use crate::cpu::Bus;
use crate::io::{IoRegisters, Keyboard};
use crate::modules::{Ram, Rom};

/// IN bits 0-8: the keyboard matrix columns (bit 15, ON, is separate).
pub(crate) const KEY_IN_MASK: u16 = 0x1FF;

/// Memory, I/O and keyboard of the machine, as seen by the CPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hardware {
    /// Chip-select decoding (wiki: hardware/memory-controller).
    pub mc: MemoryController,
    /// System ROM on NCE1: answers every address no chip claims.
    pub rom: Rom,
    /// Built-in RAM on NCE2.
    pub ram: Ram,
    /// The HDW register window, timers and CRC.
    pub io: IoRegisters,
    /// Pressed-key state.
    pub keyboard: Keyboard,
    /// Last value the CPU wrote to OUT (keyboard rows driven).
    out: u16,
}

impl Hardware {
    /// Hardware in the power-on state around `rom` and `ram`: no chip
    /// configured, I/O registers cleared, no key pressed, OUT = 0.
    pub fn new(rom: Rom, ram: Ram) -> Self {
        Self {
            mc: MemoryController::new(),
            rom,
            ram,
            io: IoRegisters::new(),
            keyboard: Keyboard::new(),
            out: 0,
        }
    }

    /// The OUT register as last written by the CPU.
    pub fn out(&self) -> u16 {
        self.out
    }

    /// Set the OUT register (power-on reset clears it).
    pub(crate) fn set_out(&mut self, out: u16) {
        self.out = out;
    }

    /// IN lines for the current OUT value.
    pub fn read_in_lines(&self) -> u16 {
        self.keyboard.read_in(self.out)
    }

    /// The nibble at `addr` through the current mapping, without side
    /// effects (no CRC update; I/O registers through [`IoRegisters::peek`]).
    pub fn peek(&self, addr: u32) -> u8 {
        match self.mc.select(addr) {
            Select::Chip {
                chip: Chip::Hdw,
                offset,
            } => self.io.peek(offset),
            sel => self.read_memory(sel),
        }
    }

    /// Read through an already decoded `sel`; I/O registers through
    /// [`IoRegisters::read`].
    fn read_selected(&mut self, sel: Select) -> u8 {
        match sel {
            Select::Chip {
                chip: Chip::Hdw,
                offset,
            } => self.io.read(offset),
            sel => self.read_memory(sel),
        }
    }

    /// Read a non-HDW `sel`: RAM, ROM or an empty port.
    fn read_memory(&self, sel: Select) -> u8 {
        match sel {
            Select::Chip {
                chip: Chip::Nce2,
                offset,
            } => self.ram.read(offset),
            // HDW is handled by the callers.
            Select::Chip { .. } => OPEN_BUS,
            Select::Rom { addr } => self.rom.read(addr),
        }
    }
}

/// Value read from a configured chip with nothing behind it (empty card
/// ports CE1/CE2, unused NCE3). Real hardware leaves the data bus floating
/// and the value is undocumented; 0 is a placeholder (inferred; wiki:
/// emulators/emu48 "Unmapped addresses read as an open data bus").
const OPEN_BUS: u8 = 0;

impl Bus for Hardware {
    fn read_nibble(&mut self, addr: u32) -> u8 {
        let sel = self.mc.select(addr);
        self.read_selected(sel)
    }

    /// Data reads feed the CRC generator, except reads of the I/O window
    /// itself (wiki: hardware/crc; Voyage: I/O RAM reads do not disturb it).
    fn read_data(&mut self, addr: u32) -> u8 {
        let sel = self.mc.select(addr);
        let is_hdw = matches!(
            sel,
            Select::Chip {
                chip: Chip::Hdw,
                ..
            }
        );
        let v = self.read_selected(sel);
        if !is_hdw {
            self.io.crc_update(v);
        }
        v
    }

    fn write_nibble(&mut self, addr: u32, nibble: u8) {
        match self.mc.select(addr) {
            Select::Chip {
                chip: Chip::Hdw,
                offset,
            } => self.io.write(offset, nibble),
            Select::Chip {
                chip: Chip::Nce2,
                offset,
            } => self.ram.write(offset, nibble),
            // ROM and empty ports ignore writes.
            Select::Chip { .. } | Select::Rom { .. } => {}
        }
    }

    fn read_in(&mut self) -> u16 {
        let v = self.read_in_lines();
        self.io.set_key_down(v & KEY_IN_MASK != 0);
        v
    }

    fn write_out(&mut self, out: u16) {
        self.out = out;
    }

    fn config(&mut self, addr: u32) {
        self.mc.config(addr);
    }

    fn unconfig(&mut self, addr: u32) {
        self.mc.unconfig(addr);
    }

    fn read_id(&mut self) -> u32 {
        self.mc.read_id()
    }

    fn reset(&mut self) {
        self.mc.reset();
    }

    /// RSI: any IN line currently high counts as a request (wiki:
    /// hardware/interrupts "RSI and the ST flags").
    fn interrupt_pending(&mut self) -> bool {
        self.read_in_lines() != 0
    }
}
