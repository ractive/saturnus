//! Per-model machine wiring: CPU + [`Hardware`], time keeping, interrupt
//! sources, SHUTDN handling and the LCD.
//!
//! Time: the CPU reports a cycle count per instruction; every cycle advances
//! the 8192-Hz timer clock by `8192 / clock_hz` ticks (accumulated exactly
//! in integer arithmetic). Interrupt sources are sampled after each time
//! advance (wiki: hardware/interrupts):
//! - timer expiry edges (not masked by INTOFF; wiki:
//!   questions/interrupt-maskability),
//! - the ON key (non-maskable),
//! - the keyboard: a rising edge of OR(IN[8:0]) seen by the ~1 ms keyboard
//!   poll, which runs only while TIMER2 runs and only interrupts while
//!   interrupts are enabled (wiki: emulators/emu48 SP16, SP31),
//! - card detect: SMP (#10E bit 1) holds NINT low after a card change,
//!   which sets HST.MP and raises a non-maskable interrupt (wiki:
//!   hardware/card-ports, emulators/emu48 SP16/SP19; Duchesne counts card
//!   insertion and removal as non-maskable, wiki: hardware/interrupts),
//! - the UART: a rising edge of its interrupt request (rx start, rx full,
//!   tx empty, per the IOC enables; wiki: hardware/uart). Not masked by
//!   INTOFF, like the timers. Sources disagree: Mastracci 2.3 lists the
//!   UART as maskable, while Ervin 4.1.2 says INTOFF stops only keyboard
//!   interrupts and Duchesne 2.1 counts only keys as maskable (wiki:
//!   hardware/interrupts, questions/interrupt-maskability). Ervin and
//!   Duchesne are followed; ROM J's serial path executes INTOFF itself
//!   (#003FF) and INTON when done (#00485), which only matters for keys.
//!   A pending UART edge also wakes the CPU from SHUTDN (wiki:
//!   hardware/interrupts "SHUTDN and wake-up", emulators/emu48 SP14).
//!
//! Serial line: [`Machine::serial_push`] queues bytes on the wire; the UART
//! receives them at line rate (11.375 bit times per byte at the #10D baud
//! rate) as emulated time passes. Bytes the calculator transmits collect
//! for [`Machine::serial_drain`] as their stop bit ends.
//!
//! Display refresh stall: while DON is set the display controller fetches
//! one row per 4096-Hz tick from RAM and the CPU waits for the bus (wiki:
//! hardware/display). This is modelled as a flat slowdown, not per row:
//! every instruction executed with the display on costs 13% more time,
//! Voyage's measured speed difference between display on and off (wiki:
//! hardware/display "Voyage additions"). The tutorial's 22-23 us per 244 us
//! row would give about 10%; Voyage's figure is the measured one and is
//! used here. Approximate: real stalls depend on when an instruction hits
//! a row fetch. Time spent in SHUTDN is not stretched. The 42S has no
//! stall: its display RAM sits in the Lewis chip (inferred; wiki:
//! hardware/lewis).
//!
//! Calibration: before the stall, every instruction's table count is
//! multiplied by [`Model::cycle_scale_permille`], fitted to real-hardware
//! benchmark times (wiki: questions/instruction-speed-vs-hardware). The
//! tables alone run the ROMs 19-34% faster than the real machines with no
//! documented cause; see the decision log, iteration 7. SHUTDN time is not
//! scaled either: the timers run on the crystal.

mod hardware;
mod lcd;
mod model;
#[cfg(feature = "profile")]
pub mod profile;

use std::fmt;

pub use hardware::{Card, Hardware, Port};
pub use lcd::{Annunciators, Framebuffer, LCD_HEIGHT, LCD_HEIGHT_42S, LCD_WIDTH, Lcd};
pub use model::{ChipRole, HardwareProfile, Model};

use crate::cpu::regs::HST_MP;
use crate::cpu::{Cpu, Event};
use crate::error::Error;
use crate::io::{IoRegisters, Key};
use crate::modules::{Flash, Nce1, Ram, Rom};
use hardware::{FRAME_TICKS, KEY_IN_MASK};

/// Timer clock rate (wiki: hardware/timers).
const TICKS_PER_SECOND: u64 = crate::io::timers::TICKS_PER_SECOND as u64;
/// Timer ticks between two keyboard polls: 8 ticks at 8192 Hz ≈ 1 ms.
pub(crate) const SCAN_TICKS: u32 = 8;
/// Upper bound for one SHUTDN time skip, so tick counts stay in `u32`.
const MAX_SKIP_TICKS: u64 = 1 << 28;
/// Time a single [`Machine::step`] may skip while shut down: one second.
const STEP_SKIP_TICKS: u64 = TICKS_PER_SECOND;
/// Display refresh stall: extra time per instruction while DON is set, in
/// percent (wiki: hardware/display, Voyage p. 193 "about 13%").
const STALL_PERCENT: u64 = 13;
/// Denominator of the fractional instruction time carried between steps:
/// per-mille calibration ([`Model::cycle_scale_permille`]) times percent
/// stall.
pub(crate) const TIME_FRACTION: u64 = 1000 * 100;
/// The HDW register window in a ROM upload (nibble addresses).
const IO_WINDOW: std::ops::Range<usize> = 0x100..0x140;
/// Smallest card image in bytes (1 KB).
pub const CARD_MIN_BYTES: usize = 1024;
/// Largest card image any port of any model takes: 4 MB, 48GX port 2
/// (wiki: hardware/card-ports). Per port and model see
/// [`Model::card_max_bytes`].
pub const CARD_MAX_BYTES: usize = 4096 * 1024;
/// Size of a new, empty RAM card: 128 KB, the largest card every port of
/// every model with slots takes.
pub const NEW_CARD_BYTES: usize = 128 * 1024;

/// Why a run stopped before its cycle budget was used up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Halt {
    /// The CPU met an undefined opcode at `pc`; `nibbles[..len]` are the
    /// nibbles the decoder read. PC has already advanced past them.
    InvalidOpcode {
        /// Address of the opcode.
        pc: u32,
        /// Nibbles read by the decoder.
        nibbles: [u8; 8],
        /// Number of valid entries in `nibbles`.
        len: u8,
    },
}

impl fmt::Display for Halt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Halt::InvalidOpcode { pc, nibbles, len } => {
                write!(f, "invalid opcode at #{pc:05X}: ")?;
                for n in nibbles.iter().take(usize::from(*len)) {
                    write!(f, "{n:X}")?;
                }
                Ok(())
            }
        }
    }
}

/// A complete calculator: CPU, hardware and the clock that ties them.
#[derive(Clone, Debug)]
pub struct Machine {
    /// The Saturn CPU.
    pub cpu: Cpu,
    /// Memory, I/O and keyboard.
    pub hw: Hardware,
    pub(crate) model: Model,
    /// CPU stopped by SHUTDN.
    pub(crate) shutdown: bool,
    /// Total CPU cycles elapsed (including refresh stalls and time skipped
    /// in SHUTDN).
    pub(crate) cycles: u64,
    /// Fractional timer ticks, in units of 1/clock_hz tick.
    pub(crate) tick_acc: u64,
    /// Fractional instruction time (calibration and stall), in units of
    /// 1/[`TIME_FRACTION`] cycle.
    pub(crate) stall_acc: u64,
    /// Timer ticks since the last keyboard poll.
    pub(crate) scan_acc: u32,
    /// OR(IN[8:0]) at the last keyboard poll.
    pub(crate) key_level: bool,
    /// ON was pressed and the interrupt is not delivered yet.
    pub(crate) on_edge: bool,
    /// A timer interrupt edge is waiting for delivery.
    pub(crate) timer_irq: bool,
    /// A keyboard interrupt edge is waiting for delivery.
    pub(crate) key_irq: bool,
    /// A card-detect interrupt edge is waiting for delivery.
    pub(crate) card_irq: bool,
    /// A UART interrupt edge is waiting for delivery.
    pub(crate) uart_irq: bool,
    /// Checksum of the system ROM, binds saved states to it.
    pub(crate) rom_sum: u64,
    /// Executed-instruction profile (feature `profile`; not saved).
    #[cfg(feature = "profile")]
    pub profile: profile::Profile,
}

impl Machine {
    /// Build a powered-on `model` from its packed ROM image. The image must
    /// have exactly [`Model::rom_bytes`] bytes; for the 49G it is the 2 MB
    /// flash image (or the same unpacked, 4 MB), for the 39G and 40G the
    /// 1 MB ROM (or the same unpacked, 2 MB); see [`Model::accepts_rom_len`].
    pub fn new(model: Model, rom_packed: &[u8]) -> Result<Self, Error> {
        if model == Model::Hp49g {
            // The whole 2 MB flash image; an unpacked image (one nibble
            // per byte, 4 MB, as the 1.19-6 emulator ROM) is accepted too.
            let flash = if rom_packed.len() == 2 * model.rom_bytes() {
                Flash::from_unpacked(rom_packed)?
            } else {
                Flash::from_packed(rom_packed)?
            };
            let nce1 = Nce1::Flash(Box::new(flash));
            return Ok(Self::with_hardware(model, Hardware::new(model, nce1)));
        }
        if matches!(model, Model::Hp39g | Model::Hp40g) {
            return Self::new_aplet_49(model, rom_packed);
        }
        let expected = model.rom_bytes();
        if rom_packed.len() != expected {
            return Err(Error::RomSize {
                expected,
                actual: rom_packed.len(),
            });
        }
        let nce1 = Nce1::Rom(Rom::from_packed(rom_packed));
        Ok(Self::with_hardware(model, Hardware::new(model, nce1)))
    }

    /// The 39G or 40G from its 1 MB ROM image, packed (1 MB file) or
    /// unpacked (2 MB file, one nibble per byte: hpcalc's `rom.39g`). The
    /// image is an upload from a calculator and carries the I/O register
    /// window at #00100-#0013F, which is zeroed here as Emu48's Convert
    /// does (wiki: hardware/hp38g "ROM image", hardware/hp39g-40g); the
    /// CPU never reads those nibbles once HDW is configured.
    fn new_aplet_49(model: Model, image: &[u8]) -> Result<Self, Error> {
        let mut nibbles: Vec<u8> = if image.len() == 2 * model.rom_bytes() {
            image.to_vec()
        } else if image.len() == model.rom_bytes() {
            image.iter().flat_map(|&b| [b & 0xF, b >> 4]).collect()
        } else {
            return Err(Error::RomSize {
                expected: model.rom_bytes(),
                actual: image.len(),
            });
        };
        nibbles[IO_WINDOW].fill(0);
        let nce1 = Nce1::BankedRom(Rom::from_nibbles(nibbles));
        Ok(Self::with_hardware(model, Hardware::new(model, nce1)))
    }

    /// A powered-on `model` around already built hardware (its NCE1 device
    /// decides the ROM checksum that binds saved states).
    pub(crate) fn with_hardware(model: Model, hw: Hardware) -> Self {
        let rom_sum = crate::state::rom_checksum(hw.nce1.nibbles());
        let mut m = Self {
            cpu: Cpu::new(),
            hw,
            model,
            shutdown: false,
            cycles: 0,
            tick_acc: 0,
            stall_acc: 0,
            scan_acc: 0,
            key_level: false,
            on_edge: false,
            timer_irq: false,
            key_irq: false,
            card_irq: false,
            uart_irq: false,
            rom_sum,
            #[cfg(feature = "profile")]
            profile: profile::Profile::default(),
        };
        m.cpu.timing = model.cycle_table();
        m.reset();
        m
    }

    /// Hardware reset: CPU registers, memory controller (all chips
    /// unconfigured), bank latch and I/O registers back to power-on; RAM contents,
    /// cards, held keys and the serial wire's queues are kept.
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.hw.mc.reset();
        // Unlike the RESET instruction (which holds the picture, see
        // `HeldFrame`), a hardware reset also clears the I/O registers and
        // so DON: nothing is shown, and the next picture after DON comes
        // from the bitmaps the ROM sets up, not from before the reset.
        self.hw.held = None;
        self.hw.clear_latch();
        // The flash command interface returns to read array, its write
        // gate closes with #11C (cleared below).
        self.hw.nce1.reset();
        let old_uart = std::mem::take(&mut self.hw.io.uart);
        self.hw.io = IoRegisters::new();
        self.hw.io.uart.keep_wire(old_uart);
        self.hw.lewis.reset();
        // Card detection is off after reset, so this raises no event.
        let pins = self.hw.card_pins();
        self.hw.io.set_card_status(pins);
        self.hw.set_out(self.cpu.regs.out);
        self.shutdown = false;
        self.scan_acc = 0;
        self.key_level = false;
        self.on_edge = false;
        self.timer_irq = false;
        self.key_irq = false;
        self.card_irq = false;
        self.uart_irq = false;
        self.stall_acc = 0;
    }

    /// The emulated model.
    pub fn model(&self) -> Model {
        self.model
    }

    /// CPU cycles elapsed since construction.
    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    /// True while the CPU is stopped by SHUTDN.
    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }

    /// Execute one instruction, or, while shut down, skip time up to the
    /// next event (at most one second). Returns the cycles that elapsed
    /// (0 when the call only woke the CPU).
    pub fn step(&mut self) -> Result<u32, Halt> {
        let budget = self.ticks_to_cycles(STEP_SKIP_TICKS);
        self.step_within(budget)
    }

    /// As [`Machine::step`], but a shut-down CPU skips at most `budget`
    /// cycles (at least 1), so a caller can stop at its own next event.
    pub fn step_for(&mut self, budget: u64) -> Result<u32, Halt> {
        self.step_within(budget.max(1))
    }

    /// Run until the cycle counter has advanced by at least `n`.
    pub fn run_cycles(&mut self, n: u64) -> Result<(), Halt> {
        let end = self.cycles.saturating_add(n);
        while self.cycles < end {
            self.step_within(end - self.cycles)?;
        }
        Ok(())
    }

    /// Whether the model's keyboard has `k` (wiki: hardware/keyboard; the
    /// 48 and 49G matrices differ).
    pub fn has_key(&self, k: Key) -> bool {
        k.position(self.hw.keyboard.layout()).is_some()
    }

    /// Press `k`. Pressing ON raises the (non-maskable) ON interrupt. A key
    /// the model lacks (see [`Machine::has_key`]) is refused with
    /// [`Error::KeyNotOnModel`] and changes nothing.
    pub fn key_down(&mut self, k: Key) -> Result<(), Error> {
        self.check_key(k)?;
        if k == Key::On && !self.hw.keyboard.on_pressed() {
            self.on_edge = true;
        }
        self.hw.keyboard.press(k);
        Ok(())
    }

    /// Release `k`; a key the model lacks is refused as by
    /// [`Machine::key_down`].
    pub fn key_up(&mut self, k: Key) -> Result<(), Error> {
        self.check_key(k)?;
        self.hw.keyboard.release(k);
        Ok(())
    }

    fn check_key(&self, k: Key) -> Result<(), Error> {
        if self.has_key(k) {
            Ok(())
        } else {
            Err(Error::KeyNotOnModel {
                key: k.name(),
                model: self.model.name(),
            })
        }
    }

    /// The nibble at `addr` through the current memory mapping, without
    /// side effects.
    pub fn peek(&self, addr: u32) -> u8 {
        self.hw.peek(addr)
    }

    /// Whether the display is switched on: DON of the model's display
    /// controller (#100 bit 3 on the 48 family, DSPCTL bit 3 on the 42S's
    /// Lewis). The one place hosts ask.
    pub fn display_on(&self) -> bool {
        self.hw.display_on()
    }

    /// The current LCD pixels: 131x64, or 131x16 on the 42S. While an
    /// UNCNFG has taken the display bitmaps out of the memory map, the
    /// picture from before it (see `HeldFrame` in `hardware.rs`).
    pub fn lcd(&self) -> Lcd {
        if self.hw.profile().lewis {
            Lcd::render_lewis(&self.hw.lewis)
        } else {
            match &self.hw.held {
                Some(h) if self.hw.io.display_on() => h.lcd.clone(),
                _ => self.hw.render_lcd(),
            }
        }
    }

    /// The current display: pixels, annunciators and contrast. The
    /// annunciators are dark while TIMER2 is stopped (wiki: emulators/emu48
    /// SP19, "TIMER2CTRL's RUN bit also governs the annunciators") or AON
    /// is clear.
    pub fn framebuffer(&self) -> Framebuffer {
        let annunciators = if self.hw.profile().lewis {
            Annunciators::from_lewis(&self.hw.lewis)
        } else if self.hw.io.timers.t2_running() {
            Annunciators::from_bits(self.hw.io.annunciators())
        } else {
            Annunciators::default()
        };
        Framebuffer {
            pixels: self.lcd(),
            annunciators,
            contrast: self.hw.contrast(),
        }
    }

    /// Insert a writable RAM card into `port` (wiki: hardware/card-ports).
    /// `packed` holds two nibbles per byte, the even address in the low
    /// nibble, like a ROM image; its length must be a power of two from
    /// [`CARD_MIN_BYTES`] to [`Model::card_max_bytes`] for the port (128 KB
    /// on both 48SX ports and 48GX port 1, 4 MB on 48GX port 2). A card
    /// already in the port is replaced. With card detection enabled the
    /// ROM sees a card-detect interrupt.
    pub fn insert_card(&mut self, port: Port, packed: &[u8]) -> Result<(), Error> {
        let n = packed.len();
        let max = self.model.card_max_bytes(port);
        if max == 0 {
            return Err(Error::NoSuchPort {
                port: port.number(),
            });
        }
        if !n.is_power_of_two() || !(CARD_MIN_BYTES..=max).contains(&n) {
            return Err(Error::CardSize { actual: n, max });
        }
        let mut ram = Ram::new(n * 2);
        for (dst, &b) in ram.as_mut_slice().chunks_mut(2).zip(packed) {
            dst[0] = b & 0xF;
            dst[1] = b >> 4;
        }
        self.hw.set_card(
            port,
            Some(hardware::Card {
                ram,
                writable: true,
            }),
        );
        Ok(())
    }

    /// Remove the card from `port`; returns whether there was one.
    pub fn remove_card(&mut self, port: Port) -> bool {
        let had = self.hw.card(port).is_some();
        if had {
            self.hw.set_card(port, None);
        }
        had
    }

    /// The contents of the card in `port`, packed like
    /// [`Machine::insert_card`] takes them, or `None` for an empty port.
    pub fn card_image(&self, port: Port) -> Option<Vec<u8>> {
        self.hw.card(port).map(|c| {
            c.ram
                .as_slice()
                .chunks(2)
                .map(|p| p[0] | (p.get(1).copied().unwrap_or(0) << 4))
                .collect()
        })
    }

    /// Whether the model has a serial port (see [`Model::has_serial`]);
    /// false on the 42S. Hosts that bridge the serial line check this
    /// first.
    pub fn has_serial(&self) -> bool {
        self.model.has_serial()
    }

    /// Queue `bytes` arriving on the serial wire. The UART receives them
    /// one per 11.375 bit times at the baud rate in #10D, starting now,
    /// while emulated time runs. Bytes arriving while the port is off
    /// (IOC SON clear) or looped back (TCS LPB) are lost, as on the real
    /// line. On a model without a serial port ([`Machine::has_serial`]
    /// false: the 42S) this does nothing: the bytes are not queued,
    /// [`Machine::serial_pending`] stays 0 and nothing is ever received.
    pub fn serial_push(&mut self, bytes: &[u8]) {
        if self.has_serial() {
            self.hw.io.uart.push(bytes);
        }
    }

    /// The bytes the calculator transmitted since the last drain.
    pub fn serial_drain(&mut self) -> Vec<u8> {
        self.hw.io.uart.drain()
    }

    /// Pushed bytes not yet delivered to the UART's receive register.
    pub fn serial_pending(&self) -> usize {
        self.hw.io.uart.pending()
    }

    /// Drop pushed bytes that have not started on the wire yet, e.g. when
    /// the host on the other end disconnects; a byte already on the wire
    /// finishes. Returns how many were dropped.
    pub fn serial_clear_inbound(&mut self) -> usize {
        self.hw.io.uart.clear_inbound()
    }

    /// The serial baud rate the calculator selected (#10D).
    pub fn serial_baud(&self) -> u32 {
        self.hw.io.uart.baud()
    }

    /// One step; a shut-down CPU skips at most `budget` cycles (>= 1).
    fn step_within(&mut self, budget: u64) -> Result<u32, Halt> {
        if self.shutdown {
            return Ok(self.sleep(budget));
        }
        #[cfg(feature = "profile")]
        let (p_before, int_before, sel) = (
            self.cpu.regs.p,
            self.cpu.regs.in_interrupt,
            self.hw.select(self.cpu.regs.pc),
        );
        let s = self.cpu.step(&mut self.hw);
        #[cfg(feature = "profile")]
        self.profile.record(&s, p_before, int_before, sel);
        let mut halt = None;
        match s.event {
            Some(Event::InvalidOpcode { pc, nibbles, len }) => {
                halt = Some(Halt::InvalidOpcode { pc, nibbles, len });
            }
            Some(Event::Shutdown) => {
                // SHUTDN does not stop the CPU if a wake condition is
                // already present (wiki: emulators/emu48 SHUTDN). The
                // OUT = 0 "cold start" case of SASM is not modelled.
                if !self.wake_condition() {
                    self.shutdown = true;
                }
                // SHUTDN resets the 48GX bank latch (wiki: emulators/emu48
                // SP23), even when the CPU does not stop; not the 49G's
                // (see `HardwareProfile::shutdn_clears_latch`).
                if self.hw.profile().shutdn_clears_latch {
                    self.hw.clear_latch();
                }
            }
            // RTI re-enters the handler while a level request is held:
            // ON (wiki: emulators/emu48 SP8, "RTI re-enters at once if ON
            // is pressed, NINT or NINT2 is low") and the UART's USRQ,
            // unless the CPU already re-vectored for a pending interrupt.
            // USRQ is a level (wiki: emulators/emu48 "UART facts"); a
            // request whose edge vectored into a handler that returned at
            // once (ST bit 15 clear, RTN without RTI) must come back when
            // the ROM re-enables interrupts with RSI and RTI, or the
            // receiver stays full and every later byte overruns (ROM J,
            // AllowIntr at #010E8-#01113; wiki: hardware/uart "Facts
            // settled while building saturnus"). Treating USRQ like NINT
            // here is inferred.
            Some(Event::Rti)
                if (self.hw.keyboard.on_pressed() || self.hw.io.uart.irq_level)
                    && !self.cpu.regs.in_interrupt =>
            {
                self.cpu.interrupt();
            }
            Some(Event::Rti) | None => {}
        }
        self.note_vector();
        // Table cycles, times the model's calibration, times the refresh
        // stall while the display is on, carrying the fraction.
        let table = u64::from(s.cycles);
        // No stall on the 42S: its display RAM is inside the Lewis, and
        // the 13% is a 48 measurement (inferred; no Lewis figure).
        let percent = if self.hw.display_on() && !self.hw.profile().lewis {
            100 + STALL_PERCENT
        } else {
            100
        };
        self.stall_acc += table * u64::from(self.model.cycle_scale_permille()) * percent;
        let cycles = self.stall_acc / TIME_FRACTION;
        self.stall_acc %= TIME_FRACTION;
        #[cfg(feature = "profile")]
        {
            self.profile.stall_cycles += cycles - table;
        }
        self.advance(cycles);
        match halt {
            Some(h) => Err(h),
            None => Ok(u32::try_from(cycles).unwrap_or(u32::MAX)),
        }
    }

    /// Anything that ends SHUTDN: a timer with WAKE expired, a key seen on
    /// the driven rows, ON held, or an undelivered interrupt.
    fn wake_condition(&self) -> bool {
        self.hw.io.timers.wake()
            || self.hw.read_in_lines() != 0
            || self.hw.keyboard.on_pressed()
            || self.on_edge
            || self.timer_irq
            || self.key_irq
            || self.card_irq
            || self.uart_irq
            || self.hw.io.module_pulled()
    }

    /// Shut-down step: wake if a wake condition holds (returns 0, no
    /// instruction taken), otherwise skip time to the next timer event,
    /// the next UART event, the end of `budget`, or the next keyboard poll
    /// while a key is held.
    fn sleep(&mut self, budget: u64) -> u32 {
        if self.wake_condition() {
            self.shutdown = false;
            self.deliver_interrupts();
            return 0;
        }
        let cycles = self.cycles_until_event().min(budget).max(1);
        self.advance(cycles);
        u32::try_from(cycles).unwrap_or(u32::MAX)
    }

    /// Cycles a shut-down CPU can skip before something may change: the
    /// next timer event, the next UART event, or the next keyboard poll
    /// while a key is held. At least 1.
    fn cycles_until_event(&self) -> u64 {
        let mut ticks = u64::from(self.hw.io.timers.ticks_until_event()).min(MAX_SKIP_TICKS);
        if self.hw.keyboard.read_in(KEY_IN_MASK) != 0 {
            ticks = ticks.min(u64::from(SCAN_TICKS));
        }
        let mut cycles = self.ticks_to_cycles(ticks.max(1));
        if let Some(c) = self.hw.io.uart.cycles_until_event(self.model.clock_hz()) {
            cycles = cycles.min(c);
        }
        cycles.max(1)
    }

    /// While the CPU is shut down with nothing to wake it: the cycles it
    /// will skip before the next timer or UART event (or keyboard poll
    /// while a key is held), so a host can sleep for real instead of
    /// stepping. `None` while the CPU runs or a wake condition holds.
    pub fn idle_cycles(&self) -> Option<u64> {
        if self.shutdown && !self.wake_condition() {
            Some(self.cycles_until_event())
        } else {
            None
        }
    }

    /// CPU cycles until `ticks` more timer ticks have elapsed (rounded up).
    fn ticks_to_cycles(&self, ticks: u64) -> u64 {
        let hz = u64::from(self.model.clock_hz());
        (ticks * hz)
            .saturating_sub(self.tick_acc)
            .div_ceil(TICKS_PER_SECOND)
    }

    /// Advance time by `c` CPU cycles, then sample the interrupt sources.
    fn advance(&mut self, c: u64) {
        let hz = u64::from(self.model.clock_hz());
        self.cycles = self.cycles.saturating_add(c);
        self.tick_acc += c * TICKS_PER_SECOND;
        let mut ticks = self.tick_acc / hz;
        self.tick_acc %= hz;
        let elapsed = ticks;
        self.hw.io.uart.advance(c, self.model.clock_hz());
        while ticks > 0 {
            let t = u32::try_from(ticks).unwrap_or(u32::MAX);
            self.hw.io.tick(t);
            ticks -= u64::from(t);
        }
        if let Some(h) = &mut self.hw.held {
            h.ticks += elapsed;
            if h.ticks >= FRAME_TICKS {
                self.hw.held = None;
            }
        }
        self.poll_interrupts(elapsed);
    }

    /// Latch interrupt edges after `ticks` timer ticks, and deliver them
    /// unless the CPU is shut down (then they wake it first).
    fn poll_interrupts(&mut self, ticks: u64) {
        if self.hw.io.timers.take_interrupt() {
            self.timer_irq = true;
        }
        if self.hw.io.take_card_interrupt() {
            self.card_irq = true;
        }
        if self.hw.io.uart.take_interrupt() {
            self.uart_irq = true;
        }
        // MP is set whenever NINT is pulled low (wiki: hardware/interrupts).
        if self.hw.io.module_pulled() {
            self.cpu.regs.hst |= HST_MP;
        }
        // Keyboard poll every ~1 ms; one poll covers a long skip, since
        // keys only change between steps. Stopped TIMER2 stops the poll
        // (wiki: emulators/emu48 SP31).
        let acc = u64::from(self.scan_acc) + ticks;
        if acc >= u64::from(SCAN_TICKS) {
            self.scan_acc = (acc % u64::from(SCAN_TICKS)) as u32;
            if self.hw.io.timers.t2_running() {
                let level = self.hw.read_in_lines() & KEY_IN_MASK != 0;
                self.hw.io.set_key_down(level);
                if level && !self.key_level && self.cpu.regs.interrupts_enabled {
                    self.key_irq = true;
                }
                self.key_level = level;
            }
        } else {
            self.scan_acc = acc as u32;
        }
        if !self.shutdown {
            self.deliver_interrupts();
        }
    }

    /// Raise one CPU interrupt for all latched sources (the handler polls
    /// every source itself).
    fn deliver_interrupts(&mut self) {
        let any = self.timer_irq || self.on_edge || self.key_irq || self.card_irq || self.uart_irq;
        self.uart_irq = false;
        self.timer_irq = false;
        self.on_edge = false;
        self.key_irq = false;
        self.card_irq = false;
        if any {
            self.cpu.interrupt();
            self.note_vector();
        }
    }

    /// Tell the timers when the CPU has just entered the handler (by an
    /// interrupt, a pending re-entry on RTI or RSI), so a pending TIMER2
    /// interrupt counts as taken.
    fn note_vector(&mut self) {
        if self.cpu.regs.in_interrupt && self.cpu.regs.pc == crate::cpu::INTERRUPT_VECTOR {
            self.hw.io.timers.interrupt_taken();
        }
    }
}

#[cfg(test)]
mod tests;
