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
//!   interrupts are enabled (wiki: emulators/emu48 SP16, SP31).

mod hardware;
mod lcd;

use std::fmt;

pub use hardware::Hardware;
pub use lcd::{LCD_HEIGHT, LCD_WIDTH, Lcd};

use crate::cpu::{Cpu, Event};
use crate::error::Error;
use crate::io::{IoRegisters, Key};
use crate::modules::{Ram, Rom};
use hardware::KEY_IN_MASK;

/// Timer clock rate (wiki: hardware/timers).
const TICKS_PER_SECOND: u64 = crate::io::timers::TICKS_PER_SECOND as u64;
/// Timer ticks between two keyboard polls: 8 ticks at 8192 Hz ≈ 1 ms.
const SCAN_TICKS: u32 = 8;
/// Upper bound for one SHUTDN time skip, so tick counts stay in `u32`.
const MAX_SKIP_TICKS: u64 = 1 << 28;
/// Time a single [`Machine::step`] may skip while shut down: one second.
const STEP_SKIP_TICKS: u64 = TICKS_PER_SECOND;

/// A supported calculator model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Model {
    /// HP48 SX: 256 KB ROM, 32 KB built-in RAM.
    Hp48sx,
}

impl Model {
    /// Size of the packed system ROM image in bytes (two nibbles per byte).
    pub fn rom_bytes(self) -> usize {
        match self {
            Model::Hp48sx => 256 * 1024,
        }
    }

    /// Built-in RAM size in nibbles (32 KB).
    pub fn ram_nibbles(self) -> usize {
        match self {
            Model::Hp48sx => 0x10000,
        }
    }

    /// CPU clock in Hz (wiki: hardware/hp48sx).
    pub fn clock_hz(self) -> u32 {
        match self {
            Model::Hp48sx => 2_000_000,
        }
    }
}

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
    model: Model,
    /// CPU stopped by SHUTDN.
    shutdown: bool,
    /// Total CPU cycles elapsed (including time skipped in SHUTDN).
    cycles: u64,
    /// Fractional timer ticks, in units of 1/clock_hz tick.
    tick_acc: u64,
    /// Timer ticks since the last keyboard poll.
    scan_acc: u32,
    /// OR(IN[8:0]) at the last keyboard poll.
    key_level: bool,
    /// ON was pressed and the interrupt is not delivered yet.
    on_edge: bool,
    /// A timer interrupt edge is waiting for delivery.
    timer_irq: bool,
    /// A keyboard interrupt edge is waiting for delivery.
    key_irq: bool,
}

impl Machine {
    /// Build a powered-on `model` from its packed ROM image. The image must
    /// have exactly [`Model::rom_bytes`] bytes.
    pub fn new(model: Model, rom_packed: &[u8]) -> Result<Self, Error> {
        let expected = model.rom_bytes();
        if rom_packed.len() != expected {
            return Err(Error::RomSize {
                expected,
                actual: rom_packed.len(),
            });
        }
        let hw = Hardware::new(Rom::from_packed(rom_packed), Ram::new(model.ram_nibbles()));
        let mut m = Self {
            cpu: Cpu::new(),
            hw,
            model,
            shutdown: false,
            cycles: 0,
            tick_acc: 0,
            scan_acc: 0,
            key_level: false,
            on_edge: false,
            timer_irq: false,
            key_irq: false,
        };
        m.reset();
        Ok(m)
    }

    /// Hardware reset: CPU registers, memory controller (all chips
    /// unconfigured) and I/O registers back to power-on; RAM contents and
    /// held keys are kept.
    pub fn reset(&mut self) {
        self.cpu.reset();
        self.hw.mc.reset();
        self.hw.io = IoRegisters::new();
        self.hw.set_out(self.cpu.regs.out);
        self.shutdown = false;
        self.scan_acc = 0;
        self.key_level = false;
        self.on_edge = false;
        self.timer_irq = false;
        self.key_irq = false;
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

    /// Run until the cycle counter has advanced by at least `n`.
    pub fn run_cycles(&mut self, n: u64) -> Result<(), Halt> {
        let end = self.cycles.saturating_add(n);
        while self.cycles < end {
            self.step_within(end - self.cycles)?;
        }
        Ok(())
    }

    /// Press `k`. Pressing ON raises the (non-maskable) ON interrupt.
    pub fn key_down(&mut self, k: Key) {
        if k == Key::On && !self.hw.keyboard.on_pressed() {
            self.on_edge = true;
        }
        self.hw.keyboard.press(k);
    }

    /// Release `k`.
    pub fn key_up(&mut self, k: Key) {
        self.hw.keyboard.release(k);
    }

    /// The nibble at `addr` through the current memory mapping, without
    /// side effects.
    pub fn peek(&self, addr: u32) -> u8 {
        self.hw.peek(addr)
    }

    /// The current LCD contents.
    pub fn lcd(&self) -> Lcd {
        Lcd::render(&self.hw.io, |a| self.peek(a))
    }

    /// One step; a shut-down CPU skips at most `budget` cycles (>= 1).
    fn step_within(&mut self, budget: u64) -> Result<u32, Halt> {
        if self.shutdown {
            return Ok(self.sleep(budget));
        }
        let s = self.cpu.step(&mut self.hw);
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
            }
            // RTI re-enters the handler while ON is held (wiki:
            // emulators/emu48 SP8).
            Some(Event::Rti) if self.hw.keyboard.on_pressed() => self.cpu.interrupt(),
            Some(Event::Rti) | None => {}
        }
        self.advance(u64::from(s.cycles));
        match halt {
            Some(h) => Err(h),
            None => Ok(s.cycles),
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
    }

    /// Shut-down step: wake if a wake condition holds (returns 0, no
    /// instruction taken), otherwise skip time to the next timer event,
    /// the end of `budget`, or the next keyboard poll while a key is held.
    fn sleep(&mut self, budget: u64) -> u32 {
        if self.wake_condition() {
            self.shutdown = false;
            self.deliver_interrupts();
            return 0;
        }
        let mut ticks = u64::from(self.hw.io.timers.ticks_until_event()).min(MAX_SKIP_TICKS);
        if self.hw.keyboard.read_in(KEY_IN_MASK) != 0 {
            ticks = ticks.min(u64::from(SCAN_TICKS));
        }
        let cycles = self.ticks_to_cycles(ticks.max(1)).min(budget).max(1);
        self.advance(cycles);
        u32::try_from(cycles).unwrap_or(u32::MAX)
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
        self.cycles += c;
        self.tick_acc += c * TICKS_PER_SECOND;
        let mut ticks = self.tick_acc / hz;
        self.tick_acc %= hz;
        let elapsed = ticks;
        while ticks > 0 {
            let t = u32::try_from(ticks).unwrap_or(u32::MAX);
            self.hw.io.tick(t);
            ticks -= u64::from(t);
        }
        self.poll_interrupts(elapsed);
    }

    /// Latch interrupt edges after `ticks` timer ticks, and deliver them
    /// unless the CPU is shut down (then they wake it first).
    fn poll_interrupts(&mut self, ticks: u64) {
        if self.hw.io.timers.take_interrupt() {
            self.timer_irq = true;
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
        let any = self.timer_irq || self.on_edge || self.key_irq;
        self.timer_irq = false;
        self.on_edge = false;
        self.key_irq = false;
        if any {
            self.cpu.interrupt();
        }
    }
}

#[cfg(test)]
mod tests;
