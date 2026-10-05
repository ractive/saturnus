//! Everything outside the CPU on an HP48 SX, wired up as the CPU's [`Bus`]:
//! memory controller, system ROM, built-in RAM, I/O registers and keyboard.

use crate::bus::{Chip, MemoryController, Select};
use crate::cpu::Bus;
use crate::io::registers::{CARD_P1_PRESENT, CARD_P1_WRITE, CARD_P2_PRESENT, CARD_P2_WRITE};
use crate::io::{IoRegisters, Keyboard};
use crate::modules::{Ram, Rom};

/// A card port of the HP48 (wiki: hardware/card-ports).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Port {
    /// Port 1, behind chip select CE1.
    One,
    /// Port 2, behind chip select CE2.
    Two,
}

impl Port {
    /// Both ports, port 1 first.
    pub const ALL: [Port; 2] = [Port::One, Port::Two];

    /// The port with the user-visible number `n` (1 or 2).
    pub fn from_number(n: u8) -> Option<Port> {
        match n {
            1 => Some(Port::One),
            2 => Some(Port::Two),
            _ => None,
        }
    }

    /// The user-visible port number, 1 or 2.
    pub fn number(self) -> u8 {
        match self {
            Port::One => 1,
            Port::Two => 2,
        }
    }

    pub(crate) fn index(self) -> usize {
        match self {
            Port::One => 0,
            Port::Two => 1,
        }
    }
}

/// A plug-in memory card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// Card contents; accesses mirror modulo its (power-of-two) size.
    pub ram: Ram,
    /// Write enable as reported on the card-detect pins.
    pub writable: bool,
}

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
    /// Cards in port 1 (CE1) and port 2 (CE2); empty by default.
    pub(crate) cards: [Option<Card>; 2],
    /// Last value the CPU wrote to OUT (keyboard rows driven).
    pub(crate) out: u16,
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
            cards: [None, None],
            out: 0,
        }
    }

    /// The card in `port`, if any.
    pub fn card(&self, port: Port) -> Option<&Card> {
        self.cards[port.index()].as_ref()
    }

    /// Put `card` into `port` (or empty it with `None`) and report the new
    /// card-detect state to the I/O registers.
    pub(crate) fn set_card(&mut self, port: Port, card: Option<Card>) {
        self.cards[port.index()] = card;
        let status = self.card_pins();
        self.io.set_card_status(status);
    }

    /// Card-detect pin state in the #10F layout: bits 0/2 for the CE1 card,
    /// bits 1/3 for CE2, as ROM J reads them (wiki: hardware/card-ports).
    pub(crate) fn card_pins(&self) -> u8 {
        let mut s = 0;
        if let Some(c) = self.card(Port::One) {
            s |= CARD_P1_PRESENT;
            if c.writable {
                s |= CARD_P1_WRITE;
            }
        }
        if let Some(c) = self.card(Port::Two) {
            s |= CARD_P2_PRESENT;
            if c.writable {
                s |= CARD_P2_WRITE;
            }
        }
        s
    }

    fn card_for(&self, chip: Chip) -> Option<&Card> {
        match chip {
            Chip::Ce1 => self.card(Port::One),
            Chip::Ce2 => self.card(Port::Two),
            _ => None,
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
            Select::Chip {
                chip: chip @ (Chip::Ce1 | Chip::Ce2),
                offset,
            } => self.card_for(chip).map_or(OPEN_BUS, |c| c.ram.read(offset)),
            // HDW is handled by the callers; NCE3 is empty on the 48SX.
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
            Select::Chip {
                chip: chip @ (Chip::Ce1 | Chip::Ce2),
                offset,
            } => {
                let idx = if chip == Chip::Ce1 { 0 } else { 1 };
                if let Some(c) = self.cards[idx].as_mut().filter(|c| c.writable) {
                    c.ram.write(offset, nibble);
                }
            }
            // ROM, empty ports and write-protected cards ignore writes.
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
