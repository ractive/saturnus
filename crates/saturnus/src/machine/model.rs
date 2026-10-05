//! Supported calculator models and what each one wires behind its chip
//! selects.
//!
//! Every model has the same six controllers in the Clarke (48SX) or Yorke
//! (48GX, 49G) chip: NCE1, HDW, NCE2, CE1, CE2, NCE3 (wiki:
//! hardware/memory-controller "Controller model (Giesselink)"). What sits
//! behind them differs per model; [`HardwareProfile`] describes it and
//! [`super::Hardware`] decodes accesses through it. Power-on is the same on
//! every model: all controllers except NCE1 are unconfigured, the I/O
//! registers cleared, the bank latch (where fitted) cleared; the ROM
//! configures memory itself (wiki: hardware/memory-controller, tutorial
//! p. 99, 151; Voyage p. 87, 130).

use crate::bus::Chip;

use super::hardware::Port;

/// A supported calculator model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Model {
    /// HP48 SX: Clarke, 2 MHz, 256 KB ROM, 32 KB RAM, two 128 KB card
    /// ports on CE1 and CE2.
    Hp48sx,
    /// HP48 GX: Yorke, 4 MHz, 512 KB ROM (upper half through DA19),
    /// 128 KB RAM, CE1 bank latch, port 1 on CE2, banked port 2 on NCE3
    /// (wiki: hardware/hp48gx).
    Hp48gx,
    /// HP 38G: Yorke, 4 MHz, 512 KB OTP ROM, 32 KB RAM that the ROM puts
    /// at #F0000, no card ports (wiki: hardware/hp38g).
    Hp38g,
    /// HP49G: Yorke, 4 MHz, 2 MB flash on NCE1 banked by the CE1 latch,
    /// 512 KB RAM on NCE2, CE2 and NCE3, its own keyboard matrix (wiki:
    /// hardware/hp49g).
    Hp49g,
    /// HP 39G: the 49G board cut to a 1 MB mask ROM on NCE1, banked by the
    /// CE1 latch, and 256 KB RAM on NCE2; nothing on CE2 or NCE3; the
    /// 49G keyboard matrix with the 39G's labels (wiki: hardware/hp39g-40g).
    Hp39g,
    /// HP 40G: the 39G hardware without IR, running the same ROM, which
    /// tells the two apart by a strap (wiki: hardware/hp39g-40g,
    /// questions/hp39g-40g-model-detection).
    Hp40g,
    /// HP 42S: a Pioneer-series machine on the Lewis chip (1LR2), about
    /// 1 MHz, 64 KB ROM at #00000, 8 KB RAM at #50000, the Lewis display
    /// and register block at #40000, a 131x16 LCD, 37 keys, no card ports
    /// and no serial port (wiki: hardware/hp42s, hardware/lewis).
    Hp42s,
}

/// What a configurable chip select drives on a model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipRole {
    /// Nothing: reads return the open-bus value, writes are ignored.
    Empty,
    /// Built-in RAM of this many nibbles.
    Ram(usize),
    /// A card slot; the card in this port answers, if any.
    Card(Port),
    /// A card slot seen through the bank latch: the window shows one
    /// 128 KB bank of the card, selected by latch bits 0-4 (48GX port 2;
    /// wiki: hardware/memory-controller "Bank switching on the 48GX").
    BankedCard(Port),
    /// The bank-select latch: any read in the window latches nibble
    /// address bits A1-A6 (wiki: hardware/memory-controller; Mastracci
    /// 4.4, Teuwen 4, tutorial p. 158).
    BankLatch,
}

/// The per-model hardware description consumed by [`super::Hardware`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HardwareProfile {
    /// Device behind CE1.
    pub ce1: ChipRole,
    /// Device behind CE2.
    pub ce2: ChipRole,
    /// Device behind NCE3.
    pub nce3: ChipRole,
    /// NCE3 shares its Yorke pin with ROM address line A19, switched by
    /// DA19 (#129 bit 3): DA19 = 1 gives A19 to the ROM; DA19 = 0 mirrors
    /// the lower 256 KB of ROM at #80000 and lets NCE3 select its device
    /// only while the latch's BEN bit is set (48GX; wiki:
    /// questions/da19-polarity, emulators/emu48 SP9/SP16). The 49G's pin
    /// is always NCE3 (wiki: hardware/hp49g).
    pub nce3_shares_a19: bool,
    /// Writes in the bank-latch window latch their address as reads do.
    /// Not on the 48GX (Giesselink: its slot 2 write-protect line gates
    /// the latch); yes on the 49G (tutorial p. 163-165; wiki:
    /// hardware/hp49g).
    pub latch_writes: bool,
    /// While #11C bit 3 is set, NCE3 accesses go to the NCE1 device (the
    /// 49G's flash write path; wiki: hardware/hp49g "Controllers").
    pub nce3_flash_path: bool,
    /// SHUTDN clears the bank latch, as on the 48GX (wiki: emulators/emu48
    /// SP23). Not on the 49G: its ROM executes SHUTDN with the OS running
    /// from a switched bank and resumes there, so a cleared latch would
    /// pull bank 0 under the running code (ROM 2.15, 2.10 and 1.19-6 boot
    /// traces; wiki: hardware/hp49g).
    pub shutdn_clears_latch: bool,
    /// Largest card image per port (port 1, port 2) in bytes, 0 for a
    /// model without card slots.
    pub card_max_bytes: [usize; 2],
    /// I/O register bits the board holds high whatever the CPU writes, as
    /// (window offset, nibble mask): a model strap the ROM reads to tell
    /// boards with one ROM apart. The 40G reads #11A bit 3 (the IR
    /// receive sample) as 1: it has no IR receiver (see [`Model::Hp40g`]).
    pub io_strap: Option<(u8, u8)>,
    /// The Lewis chip's fixed memory map instead of the Clarke/Yorke
    /// controllers: ROM, the display and register block and RAM sit at
    /// fixed addresses and CONFIG does not move them (42S; wiki:
    /// hardware/lewis "Memory map").
    pub lewis: bool,
}

impl HardwareProfile {
    /// The role of `chip` (HDW is the I/O window and NCE2 the built-in RAM
    /// on every model; both report [`ChipRole::Empty`] here and are
    /// decoded separately).
    pub fn role(&self, chip: Chip) -> ChipRole {
        match chip {
            Chip::Ce1 => self.ce1,
            Chip::Ce2 => self.ce2,
            Chip::Nce3 => self.nce3,
            Chip::Hdw | Chip::Nce2 => ChipRole::Empty,
        }
    }

    /// The chip select a card port hangs on, if the model has that port.
    pub fn port_chip(&self, port: Port) -> Option<Chip> {
        [Chip::Ce1, Chip::Ce2, Chip::Nce3].into_iter().find(
            |&c| matches!(self.role(c), ChipRole::Card(p) | ChipRole::BankedCard(p) if p == port),
        )
    }
}

const KB: usize = 1024;

/// 48SX: CE1 = port 1, CE2 = port 2, NCE3 unused (wiki:
/// hardware/memory-controller "Modules on the HP48"). Cards up to 128 KB:
/// an SX port shows no more (wiki: hardware/card-ports, emulators/emu48
/// "an SX sees only the first 128 KB").
const HP48SX: HardwareProfile = HardwareProfile {
    ce1: ChipRole::Card(Port::One),
    ce2: ChipRole::Card(Port::Two),
    nce3: ChipRole::Empty,
    nce3_shares_a19: false,
    latch_writes: false,
    nce3_flash_path: false,
    shutdn_clears_latch: true,
    card_max_bytes: [128 * KB, 128 * KB],
    io_strap: None,
    lewis: false,
};

/// 48GX: CE1 = bank latch, CE2 = port 1, NCE3 = port 2 (wiki:
/// hardware/memory-controller, Mastracci 2.4). Port 1 takes 32 or 128 KB
/// cards, port 2 up to 32 banks of 128 KB (wiki: hardware/card-ports
/// "Ports as the OS sees them", Mastracci 4.3).
const HP48GX: HardwareProfile = HardwareProfile {
    ce1: ChipRole::BankLatch,
    ce2: ChipRole::Card(Port::One),
    nce3: ChipRole::BankedCard(Port::Two),
    nce3_shares_a19: true,
    latch_writes: false,
    nce3_flash_path: false,
    shutdn_clears_latch: true,
    card_max_bytes: [128 * KB, 4096 * KB],
    io_strap: None,
    lewis: false,
};

/// 38G: a 48G without card connectors (wiki: hardware/hp38g). Inferred,
/// not sourced: RAM on NCE2 as on the 48G, the CE1 bank latch and the
/// DA19/A19 pin as on the 48G (its 512 KB ROM needs A19), nothing behind
/// CE2 and NCE3 (wiki: questions/hp38g-memory-controllers).
const HP38G: HardwareProfile = HardwareProfile {
    ce1: ChipRole::BankLatch,
    ce2: ChipRole::Empty,
    nce3: ChipRole::Empty,
    nce3_shares_a19: true,
    latch_writes: false,
    nce3_flash_path: false,
    shutdn_clears_latch: true,
    card_max_bytes: [0, 0],
    io_strap: None,
    lewis: false,
};

/// 49G: NCE1 = 2 MB flash banked by the CE1 latch, which reads and
/// writes clock; CE2 and NCE3 = 128 KB RAM each (port 1); NCE3 is also the
/// flash write path while #11C bit 3 is set; the A19/NCE3 pin is always
/// NCE3; no card slots (wiki: hardware/hp49g "Controllers (Giesselink)",
/// questions/hp49g-ram-controllers).
const HP49G: HardwareProfile = HardwareProfile {
    ce1: ChipRole::BankLatch,
    ce2: ChipRole::Ram(2 * 128 * KB),
    nce3: ChipRole::Ram(2 * 128 * KB),
    nce3_shares_a19: false,
    latch_writes: true,
    nce3_flash_path: true,
    shutdn_clears_latch: false,
    card_max_bytes: [0, 0],
    io_strap: None,
    lewis: false,
};

/// 39G/40G: the 49G wiring minus what was cut (wiki: hardware/hp39g-40g,
/// "1 MB ROM and 256 KB RAM instead of 2 MB flash and 512 KB RAM"). The
/// CE1 bank latch is the 49G's, clocked by reads and writes, and SHUTDN
/// leaves it alone as on the 49G (inferred from the shared design). CE2
/// and NCE3, the 49G's ERAM, are empty and there is no flash write path
/// (inferred; the ROM's CONFIG sequence parks both, see
/// wiki: questions/hp39g-40g-memory-map).
const HP39G: HardwareProfile = HardwareProfile {
    ce1: ChipRole::BankLatch,
    ce2: ChipRole::Empty,
    nce3: ChipRole::Empty,
    nce3_shares_a19: false,
    latch_writes: true,
    nce3_flash_path: false,
    shutdn_clears_latch: false,
    card_max_bytes: [0, 0],
    io_strap: None,
    lewis: false,
};

/// 40G: the 39G wiring with #11A bit 3 held high.
const HP40G: HardwareProfile = HardwareProfile {
    io_strap: Some((0x1A, 0x8)),
    ..HP39G
};

/// 42S: the Lewis chip's fixed map; no chip select is configurable, no
/// card port, no bank latch (wiki: hardware/lewis). The CE/NCE roles are
/// unused.
const HP42S: HardwareProfile = HardwareProfile {
    ce1: ChipRole::Empty,
    ce2: ChipRole::Empty,
    nce3: ChipRole::Empty,
    nce3_shares_a19: false,
    latch_writes: false,
    nce3_flash_path: false,
    shutdn_clears_latch: false,
    card_max_bytes: [0, 0],
    io_strap: None,
    lewis: true,
};

impl Model {
    /// Every model, in declaration order.
    pub const ALL: [Model; 7] = [
        Model::Hp48sx,
        Model::Hp48gx,
        Model::Hp38g,
        Model::Hp49g,
        Model::Hp39g,
        Model::Hp40g,
        Model::Hp42s,
    ];

    /// Short lowercase name as the CLI and the oracle use it.
    pub fn name(self) -> &'static str {
        match self {
            Model::Hp48sx => "48sx",
            Model::Hp48gx => "48gx",
            Model::Hp38g => "38g",
            Model::Hp49g => "49g",
            Model::Hp39g => "39g",
            Model::Hp40g => "40g",
            Model::Hp42s => "42s",
        }
    }

    /// What the model wires behind its chip selects.
    pub fn hardware(self) -> &'static HardwareProfile {
        match self {
            Model::Hp48sx => &HP48SX,
            Model::Hp48gx => &HP48GX,
            Model::Hp38g => &HP38G,
            Model::Hp49g => &HP49G,
            Model::Hp39g => &HP39G,
            Model::Hp40g => &HP40G,
            Model::Hp42s => &HP42S,
        }
    }

    /// Range the ROM lets ON+ / ON- move the contrast in: 3-19 on the 48SX,
    /// 9-24 on the 48GX, 38G, 49G, 39G and 40G (wiki: emulators/emu48
    /// Display, from the KML 2.0 documentation; Voyage p. 193 agrees for
    /// the SX; wiki: hardware/hp39g-40g "Display"), 15-31 on the 42S (wiki:
    /// hardware/hp42s "Summary").
    /// Informational; the hardware register takes any value 0-31.
    pub fn contrast_range(self) -> std::ops::RangeInclusive<u8> {
        match self {
            Model::Hp48sx => 3..=19,
            Model::Hp42s => 15..=31,
            _ => 9..=24,
        }
    }

    /// Size of the packed system ROM image in bytes (two nibbles per
    /// byte): 256 KB SX, 512 KB GX (wiki: hardware/hp48gx), 2 MB 49G flash
    /// (wiki: hardware/hp49g), 1 MB 39G/40G mask ROM (wiki:
    /// hardware/hp39g-40g), 64 KB 42S (wiki: hardware/hp42s).
    pub fn rom_bytes(self) -> usize {
        match self {
            Model::Hp42s => 64 * KB,
            Model::Hp48sx => 256 * KB,
            Model::Hp48gx | Model::Hp38g => 512 * KB,
            Model::Hp49g => 2048 * KB,
            Model::Hp39g | Model::Hp40g => 1024 * KB,
        }
    }

    /// Whether a ROM file of `len` bytes has a size [`super::Machine::new`]
    /// accepts: the packed image, or for the 49G, 39G and 40G also the
    /// same unpacked (one nibble per byte, twice the size; hpcalc's
    /// `rom.39g` is in that form).
    pub fn accepts_rom_len(self, len: usize) -> bool {
        len == self.rom_bytes()
            || (matches!(self, Model::Hp49g | Model::Hp39g | Model::Hp40g)
                && len == 2 * self.rom_bytes())
    }

    /// Built-in RAM on NCE2 in nibbles: 32 KB SX, 128 KB GX (wiki:
    /// hardware/hp48gx), 256 KB 49G (wiki: hardware/hp49g, the IRAM half;
    /// the other 256 KB sit on CE2 and NCE3), 8 KB on the 42S (wiki:
    /// hardware/hp42s; the 32 KB upgrade is not modelled).
    pub fn ram_nibbles(self) -> usize {
        match self {
            Model::Hp42s => 2 * 8 * KB,
            Model::Hp48sx => 2 * 32 * KB,
            Model::Hp48gx => 2 * 128 * KB,
            Model::Hp38g => 2 * 32 * KB,
            Model::Hp49g | Model::Hp39g | Model::Hp40g => 2 * 256 * KB,
        }
    }

    /// CPU clock in Hz: 2 MHz on the SX (wiki: hardware/hp48sx), 4 MHz on
    /// the Yorke models (wiki: hardware/hp48gx "~4 MHz", hardware/hp49g),
    /// 1 MHz on the 42S (wiki: hardware/hp42s "about 1 MHz"; the Lewis
    /// RATE register's effect on the clock is not modelled).
    pub fn clock_hz(self) -> u32 {
        match self {
            Model::Hp42s => 1_000_000,
            Model::Hp48sx => 2_000_000,
            _ => 4_000_000,
        }
    }

    /// Cycle table: the SASM counts on the 48SX's Clarke, the Meta Kernel
    /// counts the Saturn tutorial gives for the G series on every Yorke
    /// model (wiki: hardware/saturn-cpu "Timing", emulators/emu48 SP1).
    /// The 42S's Lewis takes the SASM counts as the Clarke does (inferred:
    /// the same 1LT8 CPU core; no Lewis cycle table is documented).
    pub fn cycle_table(self) -> crate::cpu::CycleTable {
        match self {
            Model::Hp48sx | Model::Hp42s => crate::cpu::CycleTable::Sasm,
            _ => crate::cpu::CycleTable::MetaKernel,
        }
    }

    /// Calibration of instruction time, in per mille of the cycle table's
    /// count: the factor that makes the HP Museum summation benchmark take
    /// as long as on the real machines (wiki:
    /// questions/instruction-speed-vs-hardware,
    /// sources/hpmuseum-summation-benchmark). **Calibration, not a
    /// documented hardware figure**: with the SASM counts (48SX) and the
    /// Meta Kernel counts (Yorke models) plus the 13% display stall, the
    /// ROMs run n = 1000 in 75.4 s (48SX, real 95.5 s), 41.2 / 40.5 s
    /// (48GX sum / FOR, real 55 / 54 s) and 40.2 / 41.8 s (49G ROM 2.10,
    /// real 47.8 / 51.0 s). The 38G is taken as a 48G and the 39G and 40G
    /// as a 49G (inferred: same chip and memory types; no benchmark). The
    /// 42S runs the table counts at its documented clock, unscaled: no
    /// benchmark of a real 42S has been compared yet.
    pub fn cycle_scale_permille(self) -> u32 {
        match self {
            Model::Hp42s => 1000,
            Model::Hp48sx => 1267,
            Model::Hp48gx | Model::Hp38g => 1335,
            Model::Hp49g | Model::Hp39g | Model::Hp40g => 1205,
        }
    }

    /// Keyboard: the 48 matrix on the 48SX and 48GX, the same matrix with
    /// the 38G's labels on the 38G (wiki: hardware/hp38g "Keyboard"), the
    /// 49G's own matrix, and that matrix with the 39G's labels on the 39G
    /// and 40G (wiki: hardware/hp39g-40g "Keyboard"), and the 42S's own
    /// 6 x 7 matrix (wiki: hardware/hp42s "Keyboard").
    pub fn keyboard_layout(self) -> crate::io::Layout {
        match self {
            Model::Hp42s => crate::io::Layout::Hp42,
            Model::Hp48sx | Model::Hp48gx => crate::io::Layout::Hp48,
            Model::Hp38g => crate::io::Layout::Hp38,
            Model::Hp49g => crate::io::Layout::Hp49,
            Model::Hp39g | Model::Hp40g => crate::io::Layout::Hp39,
        }
    }

    /// Largest card image `port` accepts, in bytes; 0 when the model has no
    /// such slot.
    pub fn card_max_bytes(self, port: Port) -> usize {
        self.hardware().card_max_bytes[port.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_chips_follow_the_profile() {
        let sx = Model::Hp48sx.hardware();
        assert_eq!(sx.port_chip(Port::One), Some(Chip::Ce1));
        assert_eq!(sx.port_chip(Port::Two), Some(Chip::Ce2));
        let gx = Model::Hp48gx.hardware();
        assert_eq!(gx.port_chip(Port::One), Some(Chip::Ce2));
        assert_eq!(gx.port_chip(Port::Two), Some(Chip::Nce3));
        assert_eq!(Model::Hp49g.hardware().port_chip(Port::One), None);
    }

    #[test]
    fn model_facts() {
        assert_eq!(Model::Hp48gx.rom_bytes(), 524_288);
        assert_eq!(Model::Hp48gx.clock_hz(), 4_000_000);
        assert_eq!(Model::Hp48gx.card_max_bytes(Port::Two), 4 * 1024 * 1024);
        assert_eq!(Model::Hp48sx.card_max_bytes(Port::Two), 128 * 1024);
        for m in Model::ALL {
            assert!(m.contrast_range().end() < &32);
        }
        assert_eq!(Model::Hp39g.rom_bytes(), 1024 * 1024);
        assert!(Model::Hp40g.accepts_rom_len(2 * 1024 * 1024));
        assert!(Model::Hp49g.accepts_rom_len(4 * 1024 * 1024));
        assert!(!Model::Hp48gx.accepts_rom_len(1024 * 1024));
        assert_eq!(Model::Hp40g.hardware().io_strap, Some((0x1A, 0x8)));
        assert_eq!(Model::Hp39g.hardware().io_strap, None);
        assert_eq!(
            Model::Hp39g.keyboard_layout(),
            Model::Hp40g.keyboard_layout()
        );
        assert_eq!(Model::Hp42s.rom_bytes(), 65_536);
        assert_eq!(Model::Hp42s.clock_hz(), 1_000_000);
        assert_eq!(Model::Hp42s.card_max_bytes(Port::One), 0);
        assert!(Model::Hp42s.hardware().lewis);
        assert!(Model::ALL.iter().filter(|m| m.hardware().lewis).count() == 1);
        let names: std::collections::HashSet<_> = Model::ALL.iter().map(|m| m.name()).collect();
        assert_eq!(names.len(), Model::ALL.len());
    }
}
