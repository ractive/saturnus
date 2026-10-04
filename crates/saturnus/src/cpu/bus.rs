//! The interface between the Saturn CPU and the rest of the machine.
//!
//! The CPU sees a 20-bit nibble address space (wiki: hardware/saturn-cpu
//! "Data path and address space") plus a handful of chip-interface bus
//! commands (CONFIG, UNCNFG, C=ID, RESET, SHUTDN, SREQ?, BUSCB/BUSCC/BUSCD)
//! and the IN/OUT registers. Who answers which address is decided by the
//! memory controller (wiki: hardware/memory-controller), which arrives in
//! iteration 2; this module only defines the trait and a flat test memory.

use super::regs::ADDR_MASK;

/// Bus commands without a dedicated trait method. SASM only says "issue bus
/// command B/C/D on the system bus" (src: SASM manual 6.11.3); their effect
/// on the HP48 is undocumented in our sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusCommand {
    /// BUSCB `8083`.
    B,
    /// BUSCC `80B`.
    C,
    /// BUSCD `808D`.
    D,
}

/// Everything the CPU talks to.
///
/// Addresses passed to the memory methods are 20-bit nibble addresses,
/// already masked by the CPU; nibble values are 0..=15 (the CPU masks
/// what it writes and what it reads). The chip-interface hooks default to
/// no-ops so a bare memory only needs `read_nibble`/`write_nibble`.
///
/// wiki: hardware/memory-controller (configuration and address decoding),
/// hardware/saturn-cpu ("Chip-interface instructions").
pub trait Bus {
    /// Read the nibble at a 20-bit address.
    fn read_nibble(&mut self, addr: u32) -> u8;

    /// Write a nibble (low 4 bits significant) at a 20-bit address.
    fn write_nibble(&mut self, addr: u32, nibble: u8);

    /// The 16-bit IN register (keyboard columns on the HP48), read by
    /// `A=IN`, `C=IN` and `RSI`. src: SASM manual 8 (A=IN).
    fn read_in(&mut self) -> u16 {
        0
    }

    /// `OUT=C` / `OUT=CS` changed the 12-bit OUT register (keyboard rows,
    /// speaker). src: SASM manual 8 (OUT=C, OUT=CS).
    fn write_out(&mut self, _out: u16) {}

    /// `CONFIG`: configure the next unconfigured chip with `addr` = C(A).
    /// src: SASM manual 8 (CONFIG).
    fn config(&mut self, _addr: u32) {}

    /// `UNCNFG`: unconfigure the chip at `addr` = C(A).
    /// src: SASM manual 6.11.3 (UNCNFG).
    fn unconfig(&mut self, _addr: u32) {}

    /// `C=ID`: the 5-nibble ID of the next chip to configure, loaded into
    /// C(A). src: SASM manual 8 (C=ID).
    fn read_id(&mut self) -> u32 {
        0
    }

    /// `RESET`: send the reset command on the system bus (unconfigures
    /// all chips). src: SASM manual 6.11.3 (RESET); wiki: hardware/saturn-cpu.
    fn reset(&mut self) {}

    /// `SHUTDN`: the CPU sends the shutdown bus command and stops its
    /// clock. src: SASM manual 8 (SHUTDN).
    fn shutdown(&mut self) {}

    /// `SREQ?`: poll the bus for service requests; the low 4 bits are
    /// latched into C nibble 0 (bit 0 display driver, 1 HP-IL mailbox, 2 card
    /// reader). src: SASM manual 8 (SREQ?); wiki: hardware/saturn-cpu.
    fn service_request(&mut self) -> u8 {
        0
    }

    /// BUSCB / BUSCC / BUSCD.
    fn bus_command(&mut self, _cmd: BusCommand) {}

    /// Whether an interrupt request line is active right now, as sampled by
    /// `RSI` ("consider any input line presently high as a new interrupt",
    /// src: SASM manual 8 (RSI); wiki: hardware/interrupts). The machine
    /// decides which sources count (IN bits, masking by INTOFF). Default:
    /// no request.
    fn interrupt_pending(&mut self) -> bool {
        false
    }
}

/// Size of the full Saturn address space in nibbles (2^20).
pub const ADDRESS_SPACE: usize = 1 << 20;

/// A flat, fully writable nibble memory with no chip interface. For tests
/// and the CLI's bare-CPU mode. Reads beyond the end return 0, writes
/// beyond it are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatMemory {
    nibbles: Vec<u8>,
}

impl Default for FlatMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatMemory {
    /// A zeroed memory covering the whole 1 M-nibble address space.
    pub fn new() -> Self {
        Self::with_size(ADDRESS_SPACE)
    }

    /// A zeroed memory of `size` nibbles (capped at the address space).
    pub fn with_size(size: usize) -> Self {
        Self {
            nibbles: vec![0; size.min(ADDRESS_SPACE)],
        }
    }

    /// Size in nibbles.
    pub fn len(&self) -> usize {
        self.nibbles.len()
    }

    /// True if the memory has no nibbles at all.
    pub fn is_empty(&self) -> bool {
        self.nibbles.is_empty()
    }

    /// Copy `data` (one nibble per byte, low 4 bits used) to `addr`,
    /// wrapping at 20 bits; nibbles beyond the end are dropped.
    pub fn load(&mut self, addr: u32, data: &[u8]) {
        for (i, &n) in data.iter().enumerate() {
            let a = addr.wrapping_add(i as u32) & ADDR_MASK;
            self.write_nibble(a, n);
        }
    }

    /// All nibbles, address 0 first.
    pub fn as_slice(&self) -> &[u8] {
        &self.nibbles
    }
}

impl Bus for FlatMemory {
    fn read_nibble(&mut self, addr: u32) -> u8 {
        self.nibbles
            .get((addr & ADDR_MASK) as usize)
            .copied()
            .unwrap_or(0)
    }

    fn write_nibble(&mut self, addr: u32, nibble: u8) {
        if let Some(slot) = self.nibbles.get_mut((addr & ADDR_MASK) as usize) {
            *slot = nibble & 0xF;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_memory_bounds() {
        let mut m = FlatMemory::with_size(16);
        m.write_nibble(3, 0x1A);
        assert_eq!(m.read_nibble(3), 0xA);
        m.write_nibble(100, 5);
        assert_eq!(m.read_nibble(100), 0);
        assert_eq!(m.len(), 16);
        let mut full = FlatMemory::new();
        full.load(0xF_FFFF, &[1, 2]);
        assert_eq!(full.read_nibble(0xF_FFFF), 1);
        assert_eq!(full.read_nibble(0), 2);
    }
}
