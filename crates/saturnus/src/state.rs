//! Save and load the complete mutable state of a [`Machine`].
//!
//! A small versioned little-endian binary format, written and parsed by
//! hand so the core crate stays dependency-free. Layout, in order:
//!
//! | Field | Encoding |
//! | --- | --- |
//! | magic | the 8 bytes `SATURNUS` |
//! | version | u16, currently 1 |
//! | model | u8 (0 = HP48 SX) |
//! | ROM checksum | u64, FNV-1a over the ROM nibbles |
//! | CPU | A B C D, R0-R4 (u64); D0 D1 PC (u32); P (u8); ST (u16); HST (u8); carry, mode (u8); OUT, IN (u16); RSTK 8 x u32 top first; INTON, in service, pending (u8) |
//! | memory controller | 5 chips in daisy-chain order: size flag + u32, base flag + u32, last u32 |
//! | RAM | nibble block |
//! | I/O | 64 register nibbles; TIMER1 u8, TIMER2 u32, control u8 x 2, TIMER1 phase u32, IRQ levels and edge (u8 x 3); CRC u16; row, row phase, line count u8; KDN u8; card pins u8; card edge u8 |
//! | keyboard | 9 row bytes, ON u8 |
//! | OUT | u16 |
//! | cards | per port 1, 2: present u8, then writable u8 and a nibble block |
//! | machine | shutdown u8; cycles, tick accumulator, stall accumulator u64; scan accumulator u32; key level, ON, timer, key, card edges u8 |
//!
//! A nibble block is a u32 nibble count followed by the nibbles packed two
//! per byte, low nibble first. Booleans are 0 or 1. Parsing checks every
//! length and range and returns [`Error::InvalidState`] instead of
//! panicking; trailing bytes are an error. A state only loads into a
//! machine of the same model built from the same ROM.

use crate::bus::MemoryController;
use crate::cpu::{ADDR_MASK, Mode, Registers, ReturnStack};
use crate::error::Error;
use crate::io::{IoRegisters, Keyboard, Timers};
use crate::machine::{CARD_MAX_BYTES, CARD_MIN_BYTES, Card, Hardware, Machine, Model, Port};
use crate::modules::Ram;

/// First bytes of every state.
const MAGIC: &[u8; 8] = b"SATURNUS";
/// Format version written by this library.
const VERSION: u16 = 1;
/// Largest cycle counter a state may carry (about 146 million years at
/// 2 MHz); larger values are corrupt and would overflow the counter.
pub(crate) const MAX_CYCLES: u64 = u64::MAX / 2;

/// Little-endian byte sink.
struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.buf.push(u8::from(v));
    }
    fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn opt_u32(&mut self, v: Option<u32>) {
        self.bool(v.is_some());
        self.u32(v.unwrap_or(0));
    }
    /// Nibble count (u32), then two nibbles per byte, low first.
    fn nibbles(&mut self, n: &[u8]) {
        self.u32(u32::try_from(n.len()).unwrap_or(u32::MAX));
        for pair in n.chunks(2) {
            let hi = pair.get(1).copied().unwrap_or(0);
            self.buf.push((pair[0] & 0xF) | (hi << 4));
        }
    }
}

/// Bounds-checked little-endian reader.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

type R<T> = Result<T, Error>;

impl<'a> Reader<'a> {
    fn err<T>(&self, reason: &'static str) -> R<T> {
        Err(Error::InvalidState {
            reason,
            offset: self.pos,
        })
    }
    fn take(&mut self, n: usize) -> R<&'a [u8]> {
        let slice = self
            .pos
            .checked_add(n)
            .and_then(|end| self.data.get(self.pos..end));
        match slice {
            Some(s) => {
                self.pos += n;
                Ok(s)
            }
            None => self.err("truncated"),
        }
    }
    fn array<const N: usize>(&mut self) -> R<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }
    fn u8(&mut self) -> R<u8> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> R<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> R<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn bool(&mut self) -> R<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => self.err("boolean not 0 or 1"),
        }
    }
    /// A u8 that must be `<= max`.
    fn small(&mut self, max: u8, reason: &'static str) -> R<u8> {
        let v = self.u8()?;
        if v > max { self.err(reason) } else { Ok(v) }
    }
    /// A u32 that must be `<= max`.
    fn u32_max(&mut self, max: u32, reason: &'static str) -> R<u32> {
        let v = self.u32()?;
        if v > max { self.err(reason) } else { Ok(v) }
    }
    fn addr(&mut self) -> R<u32> {
        self.u32_max(ADDR_MASK, "address above 20 bits")
    }
    fn opt_addr(&mut self) -> R<Option<u32>> {
        let some = self.bool()?;
        let v = self.addr()?;
        Ok(some.then_some(v))
    }
    /// A nibble block; its length must satisfy `len_ok`.
    fn nibbles(&mut self, len_ok: impl Fn(usize) -> bool) -> R<Vec<u8>> {
        let n = self.u32()? as usize;
        if !len_ok(n) {
            return self.err("unexpected nibble block length");
        }
        let bytes = self.take(n.div_ceil(2))?;
        let mut out = Vec::with_capacity(n);
        for &b in bytes {
            out.push(b & 0xF);
            out.push(b >> 4);
        }
        out.truncate(n);
        Ok(out)
    }
    fn finish(&self) -> R<()> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            self.err("trailing bytes")
        }
    }
}

fn model_code(m: Model) -> u8 {
    match m {
        Model::Hp48sx => 0,
    }
}

/// FNV-1a over the ROM nibbles; binds a state to its ROM. The machine
/// computes it once at construction.
pub(crate) fn rom_checksum(rom: &crate::modules::Rom) -> u64 {
    rom.as_slice()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, &n| {
            (h ^ u64::from(n)).wrapping_mul(0x0000_0100_0000_01B3)
        })
}

fn write_cpu(w: &mut Writer, r: &Registers) {
    for v in [r.a, r.b, r.c, r.d] {
        w.u64(v);
    }
    for v in r.r {
        w.u64(v);
    }
    for v in [r.d0, r.d1, r.pc] {
        w.u32(v);
    }
    w.u8(r.p);
    w.u16(r.st);
    w.u8(r.hst);
    w.bool(r.carry);
    w.u8(match r.mode {
        Mode::Hex => 0,
        Mode::Dec => 1,
    });
    w.u16(r.out);
    w.u16(r.inp);
    for &l in r.rstk.levels() {
        w.u32(l);
    }
    w.bool(r.interrupts_enabled);
    w.bool(r.in_interrupt);
    w.bool(r.interrupt_pending);
}

fn read_cpu(rd: &mut Reader) -> R<Registers> {
    // Struct literal fields evaluate in source order, matching write_cpu.
    let mut r = Registers {
        a: rd.u64()?,
        b: rd.u64()?,
        c: rd.u64()?,
        d: rd.u64()?,
        r: [rd.u64()?, rd.u64()?, rd.u64()?, rd.u64()?, rd.u64()?],
        d0: rd.addr()?,
        d1: rd.addr()?,
        pc: rd.addr()?,
        p: rd.small(15, "P above 15")?,
        st: rd.u16()?,
        hst: rd.small(15, "HST above 4 bits")?,
        carry: rd.bool()?,
        ..Registers::default()
    };
    r.mode = match rd.u8()? {
        0 => Mode::Hex,
        1 => Mode::Dec,
        _ => return rd.err("unknown arithmetic mode"),
    };
    r.out = rd.u16()?;
    if r.out > crate::cpu::regs::OUT_MASK {
        return rd.err("OUT above 12 bits");
    }
    r.inp = rd.u16()?;
    let mut levels = [0u32; 8];
    for l in &mut levels {
        *l = rd.addr()?;
    }
    r.rstk = ReturnStack::from_levels(levels);
    r.interrupts_enabled = rd.bool()?;
    r.in_interrupt = rd.bool()?;
    r.interrupt_pending = rd.bool()?;
    Ok(r)
}

fn write_mc(w: &mut Writer, mc: &MemoryController) {
    for c in &mc.chips {
        w.opt_u32(c.size);
        w.opt_u32(c.base);
        w.u32(c.last);
    }
}

fn read_mc(rd: &mut Reader) -> R<MemoryController> {
    let mut mc = MemoryController::new();
    for c in &mut mc.chips {
        c.size = rd.opt_addr()?;
        c.base = rd.opt_addr()?;
        c.last = rd.addr()?;
    }
    Ok(mc)
}

fn write_io(w: &mut Writer, io: &IoRegisters) {
    for &n in &io.regs {
        w.u8(n);
    }
    let t = &io.timers;
    w.u8(t.t1);
    w.u32(t.t2);
    w.u8(t.t1_ctrl);
    w.u8(t.t2_ctrl);
    w.u32(t.t1_phase);
    w.bool(t.t1_irq);
    w.bool(t.t2_irq);
    w.bool(t.irq_edge);
    w.u16(io.crc);
    w.u8(io.row);
    w.u8(io.row_phase);
    w.u8(io.line_count);
    w.bool(io.kdn);
    w.u8(io.card_status);
    w.bool(io.card_edge);
}

fn read_io(rd: &mut Reader) -> R<IoRegisters> {
    let mut io = IoRegisters::new();
    for n in &mut io.regs {
        *n = rd.small(15, "I/O register above 4 bits")?;
    }
    let mut t = Timers::new();
    t.t1 = rd.small(15, "TIMER1 above 4 bits")?;
    t.t2 = rd.u32()?;
    t.t1_ctrl = rd.small(7, "timer control above 3 bits")?;
    t.t2_ctrl = rd.small(7, "timer control above 3 bits")?;
    t.t1_phase = rd.u32_max(
        crate::io::timers::T2_TICKS_PER_T1_TICK - 1,
        "TIMER1 phase out of range",
    )?;
    t.t1_irq = rd.bool()?;
    t.t2_irq = rd.bool()?;
    t.irq_edge = rd.bool()?;
    io.timers = t;
    io.crc = rd.u16()?;
    io.row = rd.small(63, "display row above 63")?;
    io.row_phase = rd.small(1, "display row phase above 1")?;
    io.line_count = rd.small(63, "line count above 63")?;
    io.kdn = rd.bool()?;
    io.card_status = rd.small(15, "card status above 4 bits")?;
    io.card_edge = rd.bool()?;
    Ok(io)
}

fn write_cards(w: &mut Writer, hw: &Hardware) {
    for port in Port::ALL {
        match hw.card(port) {
            Some(c) => {
                w.bool(true);
                w.bool(c.writable);
                w.nibbles(c.ram.as_slice());
            }
            None => w.bool(false),
        }
    }
}

fn read_card(rd: &mut Reader) -> R<Option<Card>> {
    if !rd.bool()? {
        return Ok(None);
    }
    let writable = rd.bool()?;
    let nibbles = rd.nibbles(|n| {
        n.is_power_of_two() && (2 * CARD_MIN_BYTES..=2 * CARD_MAX_BYTES).contains(&n)
    })?;
    let mut ram = Ram::new(nibbles.len());
    ram.as_mut_slice().copy_from_slice(&nibbles);
    Ok(Some(Card { ram, writable }))
}

impl Machine {
    /// Serialize the complete mutable state (format in [`crate::state`]).
    /// The ROM is not included; [`Machine::load_state`] needs a machine
    /// built from the same ROM.
    pub fn save_state(&self) -> Vec<u8> {
        let mut w = Writer {
            buf: Vec::with_capacity(self.hw.ram.len() / 2 + 1024),
        };
        w.buf.extend_from_slice(MAGIC);
        w.u16(VERSION);
        w.u8(model_code(self.model));
        w.u64(self.rom_sum);
        write_cpu(&mut w, &self.cpu.regs);
        write_mc(&mut w, &self.hw.mc);
        w.nibbles(self.hw.ram.as_slice());
        write_io(&mut w, &self.hw.io);
        for &r in &self.hw.keyboard.rows {
            w.u8(r);
        }
        w.bool(self.hw.keyboard.on);
        w.u16(self.hw.out);
        write_cards(&mut w, &self.hw);
        w.bool(self.shutdown);
        w.u64(self.cycles);
        w.u64(self.tick_acc);
        w.u64(self.stall_acc);
        w.u32(self.scan_acc);
        w.bool(self.key_level);
        w.bool(self.on_edge);
        w.bool(self.timer_irq);
        w.bool(self.key_irq);
        w.bool(self.card_irq);
        w.buf
    }

    /// Replace the machine's state with one from [`Machine::save_state`].
    /// On error the machine is left unchanged.
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), Error> {
        let mut rd = Reader { data, pos: 0 };
        if rd.take(MAGIC.len())? != MAGIC {
            return Err(Error::InvalidState {
                reason: "not a saturnus state (bad magic)",
                offset: 0,
            });
        }
        if rd.u16()? != VERSION {
            return rd.err("unsupported state version");
        }
        if rd.u8()? != model_code(self.model) {
            return Err(Error::StateModelMismatch);
        }
        if rd.u64()? != self.rom_sum {
            return Err(Error::StateRomMismatch);
        }
        let regs = read_cpu(&mut rd)?;
        let mc = read_mc(&mut rd)?;
        let ram_len = self.hw.ram.len();
        let ram_nibbles = rd.nibbles(|n| n == ram_len)?;
        let io = read_io(&mut rd)?;
        let mut keyboard = Keyboard::new();
        for r in &mut keyboard.rows {
            *r = rd.small(0x3F, "keyboard row above 6 bits")?;
        }
        keyboard.on = rd.bool()?;
        let out = rd.u16()?;
        if out > crate::cpu::regs::OUT_MASK {
            return rd.err("OUT above 12 bits");
        }
        let cards = [read_card(&mut rd)?, read_card(&mut rd)?];
        let shutdown = rd.bool()?;
        let cycles = rd.u64()?;
        // Keep the counter far from u64::MAX so `advance` and `run_cycles`
        // cannot overflow or saturate within any plausible run.
        if cycles > MAX_CYCLES {
            return rd.err("cycle counter out of range");
        }
        let tick_acc = rd.u64()?;
        if tick_acc >= u64::from(self.model.clock_hz()) {
            return rd.err("tick accumulator out of range");
        }
        let stall_acc = rd.u64()?;
        if stall_acc >= 100 {
            return rd.err("stall accumulator out of range");
        }
        let scan_acc = rd.u32()?;
        if scan_acc >= crate::machine::SCAN_TICKS {
            return rd.err("scan accumulator out of range");
        }
        let key_level = rd.bool()?;
        let on_edge = rd.bool()?;
        let timer_irq = rd.bool()?;
        let key_irq = rd.bool()?;
        let card_irq = rd.bool()?;
        rd.finish()?;

        // Everything parsed: commit.
        self.cpu.regs = regs;
        self.hw.mc = mc;
        self.hw.ram.as_mut_slice().copy_from_slice(&ram_nibbles);
        self.hw.io = io;
        self.hw.keyboard = keyboard;
        self.hw.out = out;
        self.hw.cards = cards;
        self.shutdown = shutdown;
        self.cycles = cycles;
        self.tick_acc = tick_acc;
        self.stall_acc = stall_acc;
        self.scan_acc = scan_acc;
        self.key_level = key_level;
        self.on_edge = on_edge;
        self.timer_irq = timer_irq;
        self.key_irq = key_irq;
        self.card_irq = card_irq;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
