//! The Clarke/Yorke memory controller: address decoding only.
//!
//! Each chip select line except NCE1 (the system ROM) has a controller that
//! is set up by `CONFIG` instructions, torn down by `UNCNFG`, and queried
//! with `C=ID`. This module models those controllers; it holds no memory
//! contents (wiki: hardware/memory-controller, after Giesselink's
//! description in the Saturn tutorial p. 151-154).

use crate::cpu::ADDR_MASK;

/// A configurable chip select line. NCE1 (ROM) has no controller and is
/// therefore not a `Chip`: it answers every address no chip claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Chip {
    /// The memory-mapped I/O registers (64-nibble window).
    Hdw,
    /// NCE2: built-in RAM on the HP48.
    Nce2,
    /// CE1: card port 1 / bank switcher.
    Ce1,
    /// CE2: card port 2.
    Ce2,
    /// NCE3: further card / extension memory.
    Nce3,
}

/// Order in which `CONFIG` reaches the controllers: the first chip in this
/// list that is not fully configured takes the next `CONFIG`
/// (wiki: hardware/memory-controller).
pub const DAISY_CHAIN: [Chip; 5] = [Chip::Hdw, Chip::Nce2, Chip::Ce1, Chip::Ce2, Chip::Nce3];

/// Access priority, also used by `UNCNFG`: when windows overlap, the first
/// chip in this list wins. CE2 ranks above CE1 per Giesselink
/// (wiki: hardware/memory-controller); this ordering is still an open
/// question (wiki: questions/bus-priority-ce1-ce2).
pub const PRIORITY: [Chip; 5] = [Chip::Hdw, Chip::Nce2, Chip::Ce2, Chip::Ce1, Chip::Nce3];

/// HDW window: 64 nibbles, aligned to #40.
const HDW_MASK: u32 = 0xF_FFC0;
/// Size/address bits seen by the other controllers: A19-A12 only.
const SIZE_MASK: u32 = 0xF_F000;
/// Low byte that `C=ID` replaces with the chip's ID code.
const ID_ADDR_MASK: u32 = 0xF_FF00;
/// ID code reported by an unconfigured HDW controller.
const HDW_ID: u32 = 0x19;

/// Result of decoding an address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    /// A configured chip claims the address; `offset` is the address
    /// relative to the chip's window base.
    Chip {
        /// The selected chip.
        chip: Chip,
        /// Offset inside the chip's window.
        offset: u32,
    },
    /// No chip claims the address: NCE1 (ROM) answers at `addr`.
    Rom {
        /// The (20-bit masked) address presented to the ROM.
        addr: u32,
    },
}

/// Configuration state of one controller.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChipState {
    /// Size mask (unused for HDW).
    pub(crate) size: Option<u32>,
    /// Window base.
    pub(crate) base: Option<u32>,
    /// Raw C(A) value of the most recent accepted CONFIG; survives RESET.
    pub(crate) last: u32,
}

/// The memory controllers of all configurable chip select lines.
/// `Default` is the power-on state: every chip unconfigured.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryController {
    pub(crate) chips: [ChipState; 5],
}

fn index(chip: Chip) -> usize {
    match chip {
        Chip::Hdw => 0,
        Chip::Nce2 => 1,
        Chip::Ce1 => 2,
        Chip::Ce2 => 3,
        Chip::Nce3 => 4,
    }
}

impl MemoryController {
    /// A controller with every chip unconfigured.
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self, chip: Chip) -> &ChipState {
        &self.chips[index(chip)]
    }

    fn state_mut(&mut self, chip: Chip) -> &mut ChipState {
        &mut self.chips[index(chip)]
    }

    /// RESET: every chip becomes unconfigured (size and base cleared). The
    /// last CONFIG values are kept, since `C=ID` reports them afterwards.
    pub fn reset(&mut self) {
        for s in &mut self.chips {
            s.size = None;
            s.base = None;
        }
    }

    fn first_unconfigured(&self) -> Option<Chip> {
        DAISY_CHAIN.into_iter().find(|&c| !self.is_configured(c))
    }

    /// `CONFIG` with `value` = C(A). Goes to the first chip in
    /// [`DAISY_CHAIN`] that is not fully configured; ignored if all are.
    /// HDW takes one CONFIG (the address); the others take two (size mask,
    /// then address), and the address snaps down to a multiple of the size.
    pub fn config(&mut self, value: u32) {
        let Some(chip) = self.first_unconfigured() else {
            return;
        };
        let value = value & ADDR_MASK;
        let s = self.state_mut(chip);
        s.last = value;
        if chip == Chip::Hdw {
            s.base = Some(value & HDW_MASK);
        } else if let Some(mask) = s.size {
            s.base = Some(value & mask);
        } else {
            s.size = Some(value & SIZE_MASK);
        }
    }

    /// `UNCNFG` with `addr` = C(A): the first configured chip in
    /// [`PRIORITY`] whose window contains `addr` becomes unconfigured. No
    /// match is a no-op.
    pub fn unconfig(&mut self, addr: u32) {
        let addr = addr & ADDR_MASK;
        if let Some(chip) = PRIORITY.into_iter().find(|&c| self.contains(c, addr)) {
            let s = self.state_mut(chip);
            s.size = None;
            s.base = None;
        }
    }

    /// `C=ID`: the ID of the chip that would take the next `CONFIG`, or 0
    /// when all chips are configured.
    pub fn read_id(&self) -> u32 {
        let Some(chip) = self.first_unconfigured() else {
            return 0;
        };
        let s = self.state(chip);
        let size_next = s.size.is_none();
        let code = match (chip, size_next) {
            (Chip::Hdw, _) => return (s.last & HDW_MASK) | HDW_ID,
            (Chip::Nce3, true) => 0x01,
            (Chip::Nce2, true) => 0x03,
            (Chip::Ce1, true) => 0x05,
            (Chip::Ce2, true) => 0x07,
            (Chip::Nce3, false) => 0xF2,
            (Chip::Nce2, false) => 0xF4,
            (Chip::Ce1, false) => 0xF6,
            (Chip::Ce2, false) => 0xF8,
        };
        (s.last & ID_ADDR_MASK) | code
    }

    /// Decodes `addr` (masked to 20 bits): the first fully configured chip
    /// in [`PRIORITY`] whose window contains it, else the ROM.
    #[cfg(any(test, feature = "internals"))]
    pub fn select(&self, addr: u32) -> Select {
        self.select_where(addr, |_| true)
    }

    /// As [`MemoryController::select`], but a chip for which `active`
    /// returns false does not claim addresses, so they fall through to
    /// lower-priority chips and the ROM. Models use this for a chip select
    /// that is gated outside the controller (the 48GX's NCE3 shares a pin
    /// with ROM address line A19; wiki: questions/da19-polarity).
    pub fn select_where(&self, addr: u32, active: impl Fn(Chip) -> bool) -> Select {
        let addr = addr & ADDR_MASK;
        for chip in PRIORITY {
            if !active(chip) {
                continue;
            }
            if let Some((base, mask)) = self.window(chip)
                && addr & mask == base
            {
                return Select::Chip {
                    chip,
                    offset: addr & !mask & ADDR_MASK,
                };
            }
        }
        Select::Rom { addr }
    }

    fn contains(&self, chip: Chip, addr: u32) -> bool {
        self.window(chip)
            .is_some_and(|(base, mask)| addr & mask == base)
    }

    /// True when `chip` is fully configured (HDW: address given; others:
    /// size and address given).
    pub fn is_configured(&self, chip: Chip) -> bool {
        self.window(chip).is_some()
    }

    /// True when every chip is fully configured.
    #[cfg(test)]
    pub fn all_configured(&self) -> bool {
        self.first_unconfigured().is_none()
    }

    /// The `(base, mask)` window of `chip`, or `None` unless it is fully
    /// configured. An address is in the window when `addr & mask == base`.
    pub fn window(&self, chip: Chip) -> Option<(u32, u32)> {
        let s = self.state(chip);
        let base = s.base?;
        if chip == Chip::Hdw {
            Some((base, HDW_MASK))
        } else {
            s.size.map(|mask| (base, mask))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sx_bring_up() -> MemoryController {
        let mut mc = MemoryController::new();
        for v in [
            0x00100, 0xF8000, 0x70000, 0xC0000, 0x80000, 0xC0000, 0xC0000, 0xFF000, 0xD0000,
        ] {
            mc.config(v);
        }
        mc
    }

    #[test]
    fn sx_bring_up_sequence() {
        let mc = sx_bring_up();
        assert!(mc.all_configured());
        assert_eq!(mc.read_id(), 0);
        assert_eq!(
            mc.select(0x00105),
            Select::Chip {
                chip: Chip::Hdw,
                offset: 5
            }
        );
        assert_eq!(
            mc.select(0x70010),
            Select::Chip {
                chip: Chip::Nce2,
                offset: 0x10
            }
        );
        assert_eq!(mc.select(0x00140), Select::Rom { addr: 0x00140 });
        assert!(matches!(
            mc.select(0xD0000),
            Select::Chip {
                chip: Chip::Ce2,
                ..
            }
        ));
        assert_eq!(
            mc.select(0x80000),
            Select::Chip {
                chip: Chip::Ce1,
                offset: 0
            }
        );
        assert_eq!(mc.window(Chip::Nce3), Some((0xD0000, 0xFF000)));
    }

    #[test]
    fn config_ignored_when_all_configured() {
        let mut mc = sx_bring_up();
        let before = mc.clone();
        mc.config(0x12345);
        assert_eq!(mc, before);
    }

    #[test]
    fn c_eq_id_sequence() {
        let mut mc = MemoryController::new();
        assert_eq!(mc.read_id(), 0x00019);
        mc.config(0x00100);
        assert_eq!(mc.read_id(), 0x00003);
        mc.config(0xF8000);
        assert_eq!(mc.read_id(), 0xF80F4);
        mc.reset();
        assert!(!mc.is_configured(Chip::Hdw));
        assert_eq!(mc.read_id(), 0x00119);
    }

    #[test]
    fn address_snaps_down_to_size() {
        let mut mc = MemoryController::new();
        mc.config(0x00100);
        mc.config(0xF8000);
        mc.config(0x7A000);
        assert_eq!(mc.window(Chip::Nce2), Some((0x78000, 0xF8000)));
        assert_eq!(
            mc.select(0x7A001),
            Select::Chip {
                chip: Chip::Nce2,
                offset: 0x2001
            }
        );
    }

    #[test]
    fn hdw_window_aligned() {
        let mut mc = MemoryController::new();
        mc.config(0x0012F);
        assert_eq!(mc.window(Chip::Hdw), Some((0x00100, 0xFFFC0)));
        assert!(mc.is_configured(Chip::Hdw));
        assert!(!mc.is_configured(Chip::Nce2));
    }

    #[test]
    fn unconfig_removes_higher_priority_first() {
        let mut mc = sx_bring_up();
        // CE2 (#C0000-#FFFFF) overlaps NCE3 (#D0000-#D0FFF); CE2 ranks higher.
        mc.unconfig(0xD0000);
        assert!(!mc.is_configured(Chip::Ce2));
        assert!(mc.is_configured(Chip::Nce3));
        assert_eq!(
            mc.select(0xD0005),
            Select::Chip {
                chip: Chip::Nce3,
                offset: 5
            }
        );
        mc.unconfig(0xD0000);
        assert!(!mc.is_configured(Chip::Nce3));
        assert_eq!(mc.select(0xD0005), Select::Rom { addr: 0xD0005 });
        // No match: no-op.
        let before = mc.clone();
        mc.unconfig(0xD0000);
        assert_eq!(mc, before);
    }

    #[test]
    fn inactive_chip_falls_through() {
        let mc = sx_bring_up();
        // NCE3 (#D0000, 2 KB) under CE2: skipping CE2 exposes NCE3,
        // skipping both exposes the ROM.
        assert_eq!(
            mc.select_where(0xD0001, |c| c != Chip::Ce2),
            Select::Chip {
                chip: Chip::Nce3,
                offset: 1
            }
        );
        assert_eq!(
            mc.select_where(0xD0001, |c| !matches!(c, Chip::Ce2 | Chip::Nce3)),
            Select::Rom { addr: 0xD0001 }
        );
    }

    #[test]
    fn reconfig_after_unconfig_goes_to_freed_chip() {
        let mut mc = sx_bring_up();
        mc.unconfig(0x80000);
        assert!(!mc.is_configured(Chip::Ce1));
        assert_eq!(mc.read_id(), (0x80000 & 0xFFF00) | 0x05);
    }
}
