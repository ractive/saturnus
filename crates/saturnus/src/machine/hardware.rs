//! Everything outside the CPU, wired up per model as the CPU's [`Bus`]:
//! memory controller, the NCE1 device (system ROM), built-in RAM, the bank
//! latch, card slots, I/O registers and keyboard. What each chip select
//! drives comes from the model's [`HardwareProfile`].

use crate::bus::{Chip, MemoryController, Select};
use crate::cpu::Bus;
use crate::io::registers::{
    CARD_CE2_PRESENT, CARD_CE2_WRITE, CARD_OTHER_PRESENT, CARD_OTHER_WRITE,
};
use crate::io::{IoRegisters, Keyboard};
use crate::modules::{Nce1, Ram};

use super::model::{ChipRole, HardwareProfile, Model};

/// A card port of the HP48 (wiki: hardware/card-ports). Which chip select
/// serves it depends on the model: CE1 and CE2 on the 48SX, CE2 and NCE3
/// (banked) on the 48GX (see [`HardwareProfile`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Port {
    /// Port 1: CE1 on the 48SX, CE2 on the 48GX.
    One,
    /// Port 2: CE2 on the 48SX, NCE3 behind the bank latch on the 48GX.
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

/// Bank latch bits 0-4: the port 2 bank on the 48GX (nibble address bits
/// A1-A5 of the latching read).
const LATCH_BANK_MASK: u8 = 0x1F;
/// Bank latch bit 5: BEN (nibble address bit A6), port 2 enable on the
/// 48GX.
pub(crate) const LATCH_BEN: u8 = 0x20;
/// Nibbles per bank of a banked card (128 KB).
const BANK_NIBBLES: u32 = 0x4_0000;
/// ROM address lines below A19: with DA19 = 0 the 48GX ROM sees only these,
/// so its lower 256 KB repeat at #80000.
const LOWER_ROM_MASK: u32 = 0x7_FFFF;

/// Memory, I/O and keyboard of the machine, as seen by the CPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hardware {
    /// What each chip select drives on this model.
    pub(crate) profile: HardwareProfile,
    /// Chip-select decoding (wiki: hardware/memory-controller).
    pub mc: MemoryController,
    /// The NCE1 device (system ROM): answers every address no chip claims.
    pub nce1: Nce1,
    /// Built-in RAM on NCE2.
    pub ram: Ram,
    /// Built-in RAM behind CE1, CE2 and NCE3 where the profile says
    /// [`ChipRole::Ram`] (empty otherwise), in that order.
    pub(crate) chip_ram: [Ram; 3],
    /// The CE1 bank latch: nibble address bits A1-A6 of the last read in
    /// the CE1 window, where CE1 is a [`ChipRole::BankLatch`].
    pub(crate) latch: u8,
    /// The HDW register window, timers and CRC.
    pub io: IoRegisters,
    /// Pressed-key state.
    pub keyboard: Keyboard,
    /// Cards in port 1 and port 2; empty by default.
    pub(crate) cards: [Option<Card>; 2],
    /// Last value the CPU wrote to OUT (keyboard rows driven).
    pub(crate) out: u16,
}

/// Nibble address bits A1-A6 of a bank-latch window offset.
fn latch_bits(offset: u32) -> u8 {
    ((offset >> 1) & 0x3F) as u8
}

fn chip_ram_index(chip: Chip) -> Option<usize> {
    match chip {
        Chip::Ce1 => Some(0),
        Chip::Ce2 => Some(1),
        Chip::Nce3 => Some(2),
        Chip::Hdw | Chip::Nce2 => None,
    }
}

impl Hardware {
    /// Hardware of `model` in the power-on state around `nce1`: no chip
    /// configured, RAM zeroed, I/O registers cleared, latch 0, no key
    /// pressed, OUT = 0.
    pub fn new(model: Model, nce1: Nce1) -> Self {
        let profile = *model.hardware();
        let ram_for = |chip| match profile.role(chip) {
            ChipRole::Ram(n) => Ram::new(n),
            _ => Ram::new(0),
        };
        Self {
            profile,
            mc: MemoryController::new(),
            nce1,
            ram: Ram::new(model.ram_nibbles()),
            chip_ram: [ram_for(Chip::Ce1), ram_for(Chip::Ce2), ram_for(Chip::Nce3)],
            latch: 0,
            io: IoRegisters::new(),
            keyboard: Keyboard::with_layout(model.keyboard_layout()),
            cards: [None, None],
            out: 0,
        }
    }

    /// The model's hardware description.
    pub fn profile(&self) -> &HardwareProfile {
        &self.profile
    }

    /// The CE1 bank latch (nibble address bits A1-A6 of the last latching
    /// read; 0 on models without a latch).
    pub fn bank_latch(&self) -> u8 {
        self.latch
    }

    /// Clear the bank latch. CPU reset and SHUTDN do this (wiki:
    /// emulators/emu48 SP23, "SHUTDN also resets the GX bank-switch
    /// flip-flop, as does CPU reset").
    pub(crate) fn clear_latch(&mut self) {
        self.latch = 0;
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

    /// Card-detect pin state in the #10F layout. The bit pairs follow the
    /// chip select: bits 1 (present) and 3 (write) for the card on CE2,
    /// bits 0 and 2 for the other slot (CE1 on the 48SX, NCE3 on the
    /// 48GX). ROM J traces settled this for the SX; for the GX it matches
    /// Mastracci 4.3's port numbering (wiki: hardware/card-ports).
    pub(crate) fn card_pins(&self) -> u8 {
        let mut s = 0;
        for port in Port::ALL {
            let (Some(c), Some(chip)) = (self.card(port), self.profile.port_chip(port)) else {
                continue;
            };
            let (present, write) = if chip == Chip::Ce2 {
                (CARD_CE2_PRESENT, CARD_CE2_WRITE)
            } else {
                (CARD_OTHER_PRESENT, CARD_OTHER_WRITE)
            };
            s |= present;
            if c.writable {
                s |= write;
            }
        }
        s
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

    /// Whether `chip` currently takes part in decoding. Only the 48GX's
    /// NCE3 is ever inactive: its pin carries ROM A19 while DA19 = 1, and
    /// port 2 is selected only when DA19 = 0 and BEN = 1 (wiki:
    /// emulators/emu48 SP9; questions/da19-polarity). Mastracci 4.2 and
    /// Voyage p. 202 give the opposite DA19 polarity; the wiki settled on
    /// Emu48 and the tutorial, which this follows.
    fn chip_active(&self, chip: Chip) -> bool {
        chip != Chip::Nce3
            || !self.profile.nce3_shares_a19
            || (!self.io.da19() && self.latch & LATCH_BEN != 0)
    }

    fn select(&self, addr: u32) -> Select {
        self.mc.select_where(addr, |c| self.chip_active(c))
    }

    /// The nibble at `addr` through the current mapping, without side
    /// effects (no CRC update, no latching; I/O registers through
    /// [`IoRegisters::peek`]).
    pub fn peek(&self, addr: u32) -> u8 {
        match self.select(addr) {
            Select::Chip {
                chip: Chip::Hdw,
                offset,
            } => self.io.peek(offset),
            sel => self.read_memory(sel),
        }
    }

    /// Read through an already decoded `sel` with side effects: I/O
    /// registers through [`IoRegisters::read`], and a read in a bank-latch
    /// window latches its address.
    fn read_selected(&mut self, sel: Select) -> u8 {
        match sel {
            Select::Chip {
                chip: Chip::Hdw,
                offset,
            } => self.io.read(offset),
            Select::Chip { chip, offset } if self.profile.role(chip) == ChipRole::BankLatch => {
                // The window is aligned to at least #1000 nibbles, so the
                // offset carries the same A1-A6 as the address (wiki:
                // hardware/memory-controller: the latch stores nibble bits
                // A1-A6; only the window's first #80 nibbles are distinct,
                // Voyage p. 205-206). Every nibble read latches, so a byte
                // read at an even address latches its own bank.
                // Giesselink's three-nibble skew (tutorial p. 159-160) is
                // not modelled: ROM R latches with byte reads at #7F040+2n,
                // and with this exact latch "33 PVARS" with a 4 MB card
                // already gives "Invalid Card Data", as on saturnng
                // (scenario gx-card-p33).
                self.latch = latch_bits(offset);
                OPEN_BUS
            }
            sel => self.read_memory(sel),
        }
    }

    /// Read a non-HDW `sel`: RAM, a card, ROM or open bus.
    fn read_memory(&self, sel: Select) -> u8 {
        match sel {
            Select::Chip {
                chip: Chip::Nce2,
                offset,
            } => self.ram.read(offset),
            Select::Chip {
                chip: Chip::Nce3,
                offset,
            } if self.nce3_to_nce1() => self
                .nce1
                .nce3_read(offset, self.latch)
                .unwrap_or_else(|| self.read_role(Chip::Nce3, offset)),
            Select::Chip { chip, offset } => self.read_role(chip, offset),
            Select::Rom { addr } => self.nce1.read(self.rom_addr(addr), self.latch),
        }
    }

    /// Whether NCE3 accesses currently go to the NCE1 device: the 49G's
    /// flash write path, open while #11C bit 3 is set.
    fn nce3_to_nce1(&self) -> bool {
        self.profile.nce3_flash_path && self.io.lcr() & 0x8 != 0
    }

    /// Read `offset` in `chip`'s window through its role.
    fn read_role(&self, chip: Chip, offset: u32) -> u8 {
        match self.profile.role(chip) {
            ChipRole::Ram(_) => {
                chip_ram_index(chip).map_or(OPEN_BUS, |i| self.chip_ram[i].read(offset))
            }
            ChipRole::Card(p) => self.card(p).map_or(OPEN_BUS, |c| c.ram.read(offset)),
            ChipRole::BankedCard(p) => self
                .card(p)
                .map_or(OPEN_BUS, |c| c.ram.read(self.banked_offset(offset))),
            ChipRole::BankLatch | ChipRole::Empty => OPEN_BUS,
        }
    }

    /// Card offset for a banked window: the latch's bank bits pick the
    /// 128 KB bank (wiki: hardware/memory-controller, "port 2 address
    /// lines A17-A21"; emulators/emu48 SP27, port 2 mapped 128 KB at a
    /// time). Cards smaller than the addressed bank mirror.
    fn banked_offset(&self, offset: u32) -> u32 {
        u32::from(self.latch & LATCH_BANK_MASK) * BANK_NIBBLES + (offset % BANK_NIBBLES)
    }

    /// The address NCE1 sees: on the 48GX with DA19 = 0 the ROM loses A19,
    /// so the lower 256 KB repeat at #80000 (wiki: questions/da19-polarity,
    /// emulators/emu48 SP9).
    fn rom_addr(&self, addr: u32) -> u32 {
        if self.profile.nce3_shares_a19 && !self.io.da19() {
            addr & LOWER_ROM_MASK
        } else {
            addr
        }
    }
}

/// Value read from a configured chip with nothing behind it (empty card
/// slots, the bank latch, unused NCE3). Real hardware leaves the data bus
/// floating and the value is undocumented; 0 is what saturnng returns for
/// empty 48SX slots (wiki: hardware/memory-controller "Checked against
/// saturnng"; emulators/emu48 "Unmapped addresses read as an open data
/// bus").
const OPEN_BUS: u8 = 0;

impl Bus for Hardware {
    fn read_nibble(&mut self, addr: u32) -> u8 {
        let sel = self.select(addr);
        self.read_selected(sel)
    }

    /// Data reads feed the CRC generator, except reads of the I/O window
    /// itself (wiki: hardware/crc; Voyage: I/O RAM reads do not disturb it).
    fn read_data(&mut self, addr: u32) -> u8 {
        let sel = self.select(addr);
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
        let sel = self.select(addr);
        let Select::Chip { chip, offset } = sel else {
            // NCE1: ROM ignores writes.
            return;
        };
        if chip == Chip::Nce3
            && self.nce3_to_nce1()
            && self.nce1.nce3_write(offset, self.latch, nibble)
        {
            return;
        }
        match chip {
            Chip::Hdw => {
                self.io.write(offset, nibble);
                // Forward the #11C write-enable line to the NCE1 device.
                if offset & 0x3F == crate::io::registers::LCR as u32 {
                    let on = self.io.lcr() & 0x8 != 0;
                    self.nce1.set_write_enabled(on);
                }
            }
            Chip::Nce2 => self.ram.write(offset, nibble),
            _ => match self.profile.role(chip) {
                ChipRole::Ram(_) => {
                    if let Some(i) = chip_ram_index(chip) {
                        self.chip_ram[i].write(offset, nibble);
                    }
                }
                ChipRole::Card(p) => {
                    if let Some(c) = self.cards[p.index()].as_mut().filter(|c| c.writable) {
                        c.ram.write(offset, nibble);
                    }
                }
                ChipRole::BankedCard(p) => {
                    let off = self.banked_offset(offset);
                    if let Some(c) = self.cards[p.index()].as_mut().filter(|c| c.writable) {
                        c.ram.write(off, nibble);
                    }
                }
                // The 48GX latch is clocked by reads only; the 49G's also
                // by writes (profile `latch_writes`).
                ChipRole::BankLatch => {
                    if self.profile.latch_writes {
                        self.latch = latch_bits(offset);
                    }
                }
                ChipRole::Empty => {}
            },
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
