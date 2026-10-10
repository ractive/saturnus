//! The UART behind the wired serial port (wiki: hardware/uart).
//!
//! One holding register and one shift register per direction, one baud
//! generator for both (wiki: hardware/uart "Line behaviour"). Time runs in
//! sixteenths of a bit time, the receiver's 16x clock, derived from CPU
//! cycles at the baud rate selected in #10D.
//!
//! A byte frame takes 11.375 bit times = 182 sixteenths in both directions:
//! start bit, 8 data bits, 2 stop bits and a 3/16-bit internal delay, which
//! is what the HP 48 transmits (wiki: hardware/uart, io-guide 2.4.3). The
//! host side of the emulated wire is assumed to send the same way, so a
//! burst of pushed bytes arrives back to back, one every 182 sixteenths.
//!
//! Receive (wiki: hardware/uart "Receive sequence", io-guide 2.4.1): the
//! start bit sets RBZ ("receiving", RCS bit 1); after the stop bit, 160
//! sixteenths in, the byte goes to RBR, RBZ clears and RBF sets; if RBF was
//! still set, RER (overrun) sets too. Reading RBR clears RBF; writing #113
//! clears RER.
//!
//! Transmit (wiki: hardware/uart "Transmit sequence"): writing the high
//! nibble of TBR (#117, the ROM writes low nibble first) sets TBF; at the
//! next 16x clock edge the byte moves to the shifter, TBF clears and TBZ
//! ("transmitting", TCS bit 1) sets; 182 sixteenths later TBZ clears, or the
//! next byte is loaded at once if TBF is set again. The byte reaches the
//! wire (the outbound queue) after its stop bit, 160 sixteenths in.
//!
//! IOC #110 bit 3 is SON. Clearing it clears IOC, RCS, TCS, RBR and TBR and
//! drops the bytes in the shifters; writes to RCS, TCS and TBR only take
//! effect while SON is set (wiki: emulators/emu48 "UART facts", SP21/SP42).
//! While SON is clear, or while LPB loops the transmitter back, bytes on the
//! wire still pass at line rate but the receiver ignores them (inferred:
//! with the port closed a real calculator loses them too).
//!
//! The interrupt request (USRQ) is the level
//! `SON and ((ERBZ and RBZ) or (ERBF and RBF) or (ETBE and not TBF))`,
//! re-evaluated on every UART event, RBR read and IOC/TCS/TBR write (wiki:
//! emulators/emu48 SP15/SP21/SP26); its rising edge is the interrupt.
//! Sources disagree on the tx-empty condition: #110 bit 2 is "interrupt
//! when the transmit buffer is empty" (Mastracci 4.5 register table), while
//! Mastracci's transmit sequence raises it when the shifter is done. The
//! register meaning (holding register empty, TBF clear) is followed.

use std::collections::VecDeque;

/// Baud rates for the #10D codes 0-7 (wiki: hardware/uart "Baud rate
/// codes"; the ROM offers only the even codes).
pub const BAUD_RATES: [u32; 8] = [1200, 1920, 2400, 3840, 4800, 7680, 9600, 15360];
/// One byte frame on the line: 11.375 bit times in sixteenths.
pub const FRAME_16THS: u16 = 182;
/// Sixteenths from the start bit to the end of the first stop bit, when a
/// received byte reaches RBR (io-guide 2.4.1) and a sent one the wire.
pub const BYTE_DONE_16THS: u16 = 160;

/// IOC (#110) bit 0: interrupt when a receive starts.
pub const IOC_ERBZ: u8 = 0x1;
/// IOC bit 1: interrupt when the receive buffer is full.
pub const IOC_ERBF: u8 = 0x2;
/// IOC bit 2: interrupt when the transmit buffer is empty.
pub const IOC_ETBE: u8 = 0x4;
/// IOC bit 3: SON, serial port on.
pub const IOC_SON: u8 = 0x8;
/// RCS (#111) bit 0: RBF, a byte waits in RBR.
pub const RCS_RBF: u8 = 0x1;
/// RCS bit 1: RBZ, receiving a byte.
pub const RCS_RBZ: u8 = 0x2;
/// RCS bit 2: RER, receive error (overrun, break).
pub const RCS_RER: u8 = 0x4;
/// RCS bit 3: undocumented; stored. The ROM masks it off after reading
/// RCS (ROM J at #003F5 and #02845).
const RCS_BIT3: u8 = 0x8;
/// TCS (#112) bit 0: TBF, TBR holds an unsent byte.
pub const TCS_TBF: u8 = 0x1;
/// TCS bit 1: TBZ, transmitting.
pub const TCS_TBZ: u8 = 0x2;
/// TCS bit 2: LPB, loop the transmitter back into the receiver (wiki:
/// questions/uart-lpb-bit).
pub const TCS_LPB: u8 = 0x4;
/// TCS bit 3: BRK, send a break. Mastracci 4.5 calls it "break received";
/// Emu48 SP26 "send break", and ROM J writes it (#317A9), so it is a
/// control bit here.
pub const TCS_BRK: u8 = 0x8;
/// BAU (#10D) bit 3: UCK, the UART clock, read-only.
pub const BAU_UCK: u8 = 0x8;
/// SRQ1 (#118) bit carrying USRQ. The position is undocumented (wiki:
/// hardware/io-ram, emulators/emu48 list only the name); ROM J's interrupt
/// handler polls IOC and RCS instead and was not seen reading #118.
/// Bit 0 is a placeholder (inferred).
pub const SRQ1_USRQ: u8 = 0x1;

/// Register offsets in the HDW window.
pub(crate) const BAU: usize = 0x0D;
pub(crate) const IOC: usize = 0x10;
pub(crate) const RCS: usize = 0x11;
pub(crate) const TCS: usize = 0x12;
pub(crate) const CRER: usize = 0x13;
pub(crate) const RBR_LO: usize = 0x14;
pub(crate) const RBR_HI: usize = 0x15;
pub(crate) const TBR_LO: usize = 0x16;
pub(crate) const TBR_HI: usize = 0x17;
pub(crate) const SRQ1: usize = 0x18;

/// A byte travelling over the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Frame {
    /// The byte.
    pub(crate) byte: u8,
    /// Sixteenths since its start bit, 0..=FRAME_16THS.
    pub(crate) pos: u16,
    /// The receiver saw the start bit (wire frames only).
    pub(crate) live: bool,
}

/// The UART: registers, shifters and the two ends of the emulated wire.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Uart {
    /// Baud code, #10D bits 0-2.
    pub(crate) bau: u8,
    /// IOC, #110.
    pub(crate) ioc: u8,
    /// RCS, #111.
    pub(crate) rcs: u8,
    /// TCS, #112.
    pub(crate) tcs: u8,
    /// Receive buffer register.
    pub(crate) rbr: u8,
    /// Transmit buffer register.
    pub(crate) tbr: u8,
    /// Fraction of a sixteenth elapsed, in units of 1/clock_hz sixteenth.
    pub(crate) acc: u64,
    /// Free-running 16x clock phase (0..16) for UCK.
    pub(crate) phase: u8,
    /// The inbound byte currently on the wire.
    pub(crate) wire: Option<Frame>,
    /// The byte in the transmit shifter.
    pub(crate) tx: Option<Frame>,
    /// Sixteenths a looped-back break has lasted, capped at
    /// [`BYTE_DONE_16THS`]; `None` without a looped-back break.
    pub(crate) brk: Option<u16>,
    /// Last evaluated interrupt request level (USRQ).
    pub(crate) irq_level: bool,
    /// Latched rising edge of the interrupt request.
    pub(crate) irq_edge: bool,
    /// Bytes waiting to be sent to the calculator.
    pub(crate) inbound: VecDeque<u8>,
    /// Bytes the calculator sent, not drained yet.
    pub(crate) outbound: Vec<u8>,
}

impl Uart {
    /// Power-on state: everything clear, empty wire.
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep the wire (queues and the inbound byte in flight) of `old`; used
    /// when the calculator is reset, which does not stop the host sending.
    pub(crate) fn keep_wire(&mut self, old: Uart) {
        self.inbound = old.inbound;
        self.outbound = old.outbound;
        self.wire = old.wire.map(|f| Frame { live: false, ..f });
    }

    /// Selected baud rate.
    pub fn baud(&self) -> u32 {
        BAUD_RATES[usize::from(self.bau & 7)]
    }

    /// Whether the serial port is on (IOC SON).
    pub fn is_on(&self) -> bool {
        self.ioc & IOC_SON != 0
    }

    /// Bytes pushed but not yet delivered to RBR (or dropped by a closed
    /// port): the queue plus a byte still in flight.
    pub fn pending(&self) -> usize {
        let in_flight = self.wire.is_some_and(|f| f.pos < BYTE_DONE_16THS);
        self.inbound.len() + usize::from(in_flight)
    }

    /// Queue bytes arriving on the wire.
    pub fn push(&mut self, bytes: &[u8]) {
        self.inbound.extend(bytes);
    }

    /// Drop the queued inbound bytes (the host went away); a byte already
    /// on the wire finishes. Returns how many were dropped.
    pub fn clear_inbound(&mut self) -> usize {
        let n = self.inbound.len();
        self.inbound.clear();
        n
    }

    /// Take the bytes transmitted so far.
    pub fn drain(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.outbound)
    }

    /// The nibble a CPU read at window offset `off` returns, without side
    /// effects. `off` must be one of the UART offsets.
    pub(crate) fn peek(&self, off: usize) -> u8 {
        match off {
            BAU => {
                // UCK is always set while the UART is off (wiki:
                // emulators/emu48 SP14/SP42); while on it shows the bit
                // clock, high in the first half of each bit (inferred).
                let uck = !self.is_on() || self.phase < 8;
                self.bau | if uck { BAU_UCK } else { 0 }
            }
            IOC => self.ioc,
            RCS => self.rcs,
            TCS => self.tcs,
            RBR_LO => self.rbr & 0xF,
            RBR_HI => self.rbr >> 4,
            // TBR reads back the last written byte (wiki: emulators/emu48
            // SP21).
            TBR_LO => self.tbr & 0xF,
            TBR_HI => self.tbr >> 4,
            SRQ1 if self.irq_level => SRQ1_USRQ,
            // #113 is write-only; SRQ1 without a request reads 0.
            _ => 0,
        }
    }

    /// CPU read: reading either RBR nibble clears RBF (wiki:
    /// hardware/uart, Voyage p. 197-198; emulators/emu48 SP65).
    pub(crate) fn read(&mut self, off: usize) -> u8 {
        let v = self.peek(off);
        if matches!(off, RBR_LO | RBR_HI) && self.rcs & RCS_RBF != 0 {
            self.rcs &= !RCS_RBF;
            self.update_irq();
        }
        v
    }

    /// CPU write of nibble `v` at window offset `off`.
    pub(crate) fn write(&mut self, off: usize, v: u8) {
        let v = v & 0xF;
        let on = self.is_on();
        match off {
            BAU => self.bau = v & 7,
            IOC => {
                if v & IOC_SON == 0 {
                    self.power_off();
                } else {
                    self.ioc = v;
                }
            }
            // RCS bits 0-2 are status, changed only by the receiver, an RBR
            // read and #113; bit 3 is stored.
            RCS if on => self.rcs = (self.rcs & !RCS_BIT3) | (v & RCS_BIT3),
            TCS if on => {
                let was_brk = self.tcs & TCS_BRK != 0;
                self.tcs = (self.tcs & (TCS_TBF | TCS_TBZ)) | (v & (TCS_LPB | TCS_BRK));
                let brk = self.tcs & TCS_BRK != 0;
                if brk && !was_brk && self.tcs & TCS_LPB != 0 {
                    // The receiver sees the line go to space: a start bit
                    // that never ends (io-guide 2.4.2).
                    self.brk = Some(0);
                    self.rx_start();
                } else if !brk {
                    self.brk = None;
                }
            }
            CRER => self.rcs &= !RCS_RER,
            TBR_LO if on => self.tbr = (self.tbr & 0xF0) | v,
            TBR_HI if on => {
                self.tbr = (self.tbr & 0x0F) | (v << 4);
                self.tcs |= TCS_TBF;
            }
            // RCS, TCS and TBR writes with SON clear, read-only RBR and
            // SRQ1: ignored.
            _ => {}
        }
        self.update_irq();
    }

    /// SON cleared: IOC, RCS, TCS, RBR and TBR clear and the shifters drop
    /// their bytes. The wire keeps running.
    fn power_off(&mut self) {
        self.ioc = 0;
        self.rcs = 0;
        self.tcs = 0;
        self.rbr = 0;
        self.tbr = 0;
        self.tx = None;
        self.brk = None;
        if let Some(f) = self.wire.as_mut() {
            f.live = false;
        }
    }

    /// The receiver is connected to the wire: port on, no loop-back.
    fn wire_connected(&self) -> bool {
        self.is_on() && self.tcs & TCS_LPB == 0
    }

    fn rx_start(&mut self) {
        self.rcs |= RCS_RBZ;
    }

    /// A byte (or a break, `error`) completed in the receiver.
    fn rx_complete(&mut self, byte: u8, error: bool) {
        self.rcs &= !RCS_RBZ;
        if self.rcs & RCS_RBF != 0 || error {
            self.rcs |= RCS_RER;
        }
        self.rbr = byte;
        self.rcs |= RCS_RBF;
    }

    /// Put the next inbound byte on an idle wire.
    fn start_wire(&mut self) {
        if self.wire.is_some() {
            return;
        }
        if let Some(byte) = self.inbound.pop_front() {
            let live = self.wire_connected();
            if live {
                self.rx_start();
            }
            self.wire = Some(Frame { byte, pos: 0, live });
        }
    }

    /// Move TBR into the idle shifter.
    fn load_tx(&mut self) {
        self.tx = Some(Frame {
            byte: self.tbr,
            pos: 0,
            live: true,
        });
        self.tcs = (self.tcs & !TCS_TBF) | TCS_TBZ;
        if self.tcs & TCS_LPB != 0 {
            self.rx_start();
        }
    }

    /// Sixteenths until the next event of a frame at `pos`.
    fn frame_event(pos: u16) -> u16 {
        if pos < BYTE_DONE_16THS {
            BYTE_DONE_16THS - pos
        } else {
            FRAME_16THS - pos
        }
    }

    /// Sixteenths until the next UART event, `None` when nothing is in
    /// flight. An idle wire with queued bytes starts at once (0).
    fn next_event(&self) -> Option<u16> {
        if self.wire.is_none() && !self.inbound.is_empty() {
            return Some(0);
        }
        let wire = self.wire.map(|f| Self::frame_event(f.pos));
        let tx = match self.tx {
            Some(f) => Some(Self::frame_event(f.pos)),
            // The byte moves to the shifter at the next 16x clock edge.
            None if self.tcs & TCS_TBF != 0 => Some(1),
            None => None,
        };
        let brk = self
            .brk
            .filter(|&p| p < BYTE_DONE_16THS)
            .map(|p| BYTE_DONE_16THS - p);
        [wire, tx, brk].into_iter().flatten().min()
    }

    /// CPU cycles (at `clock_hz`) until the next UART event, `None` when
    /// idle. Lets the machine skip time in SHUTDN without missing a byte.
    pub(crate) fn cycles_until_event(&self, clock_hz: u32) -> Option<u64> {
        let n = u64::from(self.next_event()?);
        let rate = 16 * u64::from(self.baud());
        Some(
            (n * u64::from(clock_hz))
                .saturating_sub(self.acc)
                .div_ceil(rate),
        )
    }

    /// Advance the line by `cycles` CPU cycles at `clock_hz`.
    pub(crate) fn advance(&mut self, cycles: u64, clock_hz: u32) {
        let hz = u64::from(clock_hz);
        self.start_wire();
        self.acc += cycles * 16 * u64::from(self.baud());
        let mut n = self.acc / hz;
        self.acc %= hz;
        self.phase = ((u64::from(self.phase) + n % 16) % 16) as u8;
        while n > 0 {
            let Some(next) = self.next_event() else {
                break;
            };
            let step = u16::try_from(n.min(u64::from(next.max(1)))).unwrap_or(1);
            self.run(step);
            n -= u64::from(step);
        }
        self.update_irq();
    }

    /// Advance every frame by `step` sixteenths (no frame passes an event
    /// strictly inside the step) and handle the events reached.
    fn run(&mut self, step: u16) {
        if let Some(mut f) = self.wire {
            f.pos += step;
            if f.pos == BYTE_DONE_16THS && f.live {
                if self.wire_connected() {
                    self.rx_complete(f.byte, false);
                } else {
                    self.rcs &= !RCS_RBZ;
                }
            }
            self.wire = (f.pos < FRAME_16THS).then_some(f);
        }
        match self.tx {
            Some(mut f) => {
                f.pos += step;
                if f.pos == BYTE_DONE_16THS {
                    if self.tcs & TCS_LPB != 0 {
                        self.rx_complete(f.byte, false);
                    } else {
                        self.outbound.push(f.byte);
                    }
                }
                if f.pos >= FRAME_16THS {
                    self.tx = None;
                    self.tcs &= !TCS_TBZ;
                    if self.tcs & TCS_TBF != 0 {
                        self.load_tx();
                    }
                } else {
                    self.tx = Some(f);
                }
            }
            None if self.tcs & TCS_TBF != 0 => self.load_tx(),
            None => {}
        }
        if let Some(p) = self.brk
            && p < BYTE_DONE_16THS
        {
            let p = p + step;
            if p == BYTE_DONE_16THS {
                // A break reads as a null byte with RER and RBF set; the
                // start filter keeps it from producing more bytes
                // (io-guide 2.4.2).
                self.rx_complete(0, true);
            }
            self.brk = Some(p.min(BYTE_DONE_16THS));
        }
        self.start_wire();
        self.update_irq();
    }

    /// Re-evaluate USRQ and latch a rising edge.
    fn update_irq(&mut self) {
        let ioc = self.ioc;
        let level = ioc & IOC_SON != 0
            && ((ioc & IOC_ERBZ != 0 && self.rcs & RCS_RBZ != 0)
                || (ioc & IOC_ERBF != 0 && self.rcs & RCS_RBF != 0)
                || (ioc & IOC_ETBE != 0 && self.tcs & TCS_TBF == 0));
        if level && !self.irq_level {
            self.irq_edge = true;
        }
        self.irq_level = level;
    }

    /// Return and clear a latched interrupt edge.
    pub fn take_interrupt(&mut self) -> bool {
        std::mem::take(&mut self.irq_edge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock at which one CPU cycle is one sixteenth of a bit at 9600
    /// baud, so the tests count line time directly in sixteenths.
    const HZ: u32 = 16 * 9600;

    /// A UART at 9600 baud (code 6) with SON and the given IOC enables.
    fn on(enables: u8) -> Uart {
        let mut u = Uart::new();
        u.write(BAU, 6);
        u.write(IOC, IOC_SON | enables);
        u.take_interrupt();
        u
    }

    fn run(u: &mut Uart, sixteenths: u64) {
        u.advance(sixteenths, HZ);
    }

    fn send(u: &mut Uart, byte: u8) {
        u.write(TBR_LO, byte & 0xF);
        u.write(TBR_HI, byte >> 4);
    }

    fn read_rbr(u: &mut Uart) -> u8 {
        u.read(RBR_LO) | (u.read(RBR_HI) << 4)
    }

    #[test]
    fn baud_codes_and_uck() {
        let mut u = Uart::new();
        assert_eq!(u.baud(), 1200);
        u.write(BAU, 0xE);
        assert_eq!(u.baud(), 9600);
        assert_eq!(u.peek(BAU), 0x6 | BAU_UCK, "UCK set while off");
        u.write(IOC, IOC_SON);
        // The bit clock: high for the first 8 sixteenths of a bit.
        let mut seen = Vec::new();
        for _ in 0..16 {
            seen.push(u.peek(BAU) & BAU_UCK != 0);
            run(&mut u, 1);
        }
        assert_eq!(seen.iter().filter(|&&b| b).count(), 8);
        assert_eq!(u.peek(BAU) & 7, 6);
        // A real 2 MHz clock: 9600 baud is 2_000_000 / 153_600 = 13.02
        // cycles per sixteenth.
        let mut v = on(0);
        v.push(&[0x55]);
        v.advance(13 * 160, 2_000_000);
        assert_eq!(v.rcs & RCS_RBF, 0);
        v.advance(4, 2_000_000);
        assert_eq!(v.rcs & RCS_RBF, RCS_RBF);
    }

    #[test]
    fn receive_timing_and_rbr_read() {
        let mut u = on(0);
        u.push(&[0xA5]);
        assert_eq!(u.pending(), 1);
        run(&mut u, 1);
        assert_eq!(u.peek(RCS), RCS_RBZ, "start bit: receiving");
        run(&mut u, 158);
        assert_eq!(u.peek(RCS), RCS_RBZ);
        assert_eq!(u.pending(), 1);
        run(&mut u, 1);
        assert_eq!(u.peek(RCS), RCS_RBF, "after the stop bit: RBF");
        assert_eq!(u.pending(), 0);
        assert_eq!(u.peek(RBR_LO), 0x5);
        assert_eq!(u.peek(RBR_HI), 0xA);
        assert_eq!(u.peek(RCS), RCS_RBF, "peek has no side effect");
        assert_eq!(read_rbr(&mut u), 0xA5);
        assert_eq!(u.peek(RCS), 0);
        // RBR keeps its value; RCS is read-only status.
        u.write(RCS, 0x7);
        assert_eq!(u.peek(RCS), 0);
        assert_eq!(read_rbr(&mut u), 0xA5);
    }

    #[test]
    fn burst_arrives_one_frame_apart_and_overruns() {
        let mut u = on(0);
        u.push(&[1, 2, 3]);
        run(&mut u, 160);
        assert_eq!(read_rbr(&mut u), 1);
        // The second start bit comes 182 sixteenths after the first.
        run(&mut u, 21);
        assert_eq!(u.peek(RCS), 0);
        run(&mut u, 1);
        assert_eq!(u.peek(RCS), RCS_RBZ);
        run(&mut u, 160);
        assert_eq!(u.peek(RCS), RCS_RBF);
        // Not read: the third byte overruns, RER, RBR holds the new byte.
        run(&mut u, 182);
        assert_eq!(u.peek(RCS), RCS_RBF | RCS_RER);
        assert_eq!(read_rbr(&mut u), 3);
        assert_eq!(u.peek(RCS), RCS_RER, "RER survives the RBR read");
        u.write(CRER, 0);
        assert_eq!(u.peek(RCS), 0);
        assert_eq!(u.pending(), 0);
    }

    #[test]
    fn transmit_timing() {
        let mut u = on(0);
        send(&mut u, 0x3C);
        assert_eq!(u.peek(TCS), TCS_TBF);
        assert_eq!(u.peek(TBR_LO) | (u.peek(TBR_HI) << 4), 0x3C);
        run(&mut u, 1);
        assert_eq!(u.peek(TCS), TCS_TBZ, "moved to the shifter");
        // Second byte waits in TBR.
        send(&mut u, 0x3D);
        assert_eq!(u.peek(TCS), TCS_TBF | TCS_TBZ);
        run(&mut u, 159);
        assert!(u.drain().is_empty());
        run(&mut u, 1);
        assert_eq!(u.drain(), vec![0x3C], "on the wire after the stop bit");
        run(&mut u, 22);
        assert_eq!(u.peek(TCS), TCS_TBZ, "second byte loaded back to back");
        run(&mut u, 182);
        assert_eq!(u.drain(), vec![0x3D]);
        assert_eq!(u.peek(TCS), 0);
    }

    #[test]
    fn writes_need_son_and_son_clear_resets() {
        let mut u = Uart::new();
        u.write(TCS, TCS_LPB);
        send(&mut u, 0x41);
        u.write(RCS, 0x8);
        assert_eq!((u.peek(TCS), u.peek(RCS), u.peek(TBR_LO)), (0, 0, 0));
        let mut u = on(IOC_ERBF);
        u.write(RCS, 0xF);
        assert_eq!(u.peek(RCS), 0x8, "RCS bit 3 is stored");
        u.push(&[7, 8]);
        run(&mut u, 160);
        send(&mut u, 0x41);
        run(&mut u, 2);
        assert_eq!(u.peek(TCS), TCS_TBZ);
        u.write(IOC, IOC_ERBF);
        assert_eq!(
            [IOC, RCS, TCS, RBR_LO, TBR_LO].map(|o| u.peek(o)),
            [0; 5],
            "IOC, RCS, TCS, RBR, TBR clear"
        );
        run(&mut u, 400);
        assert!(u.drain().is_empty(), "shifter dropped its byte");
        // The second byte passed the closed port and is gone.
        assert_eq!(u.pending(), 0);
        assert_eq!(u.peek(RCS), 0);
        assert_eq!(u.peek(BAU), 6 | BAU_UCK, "the baud code stays");
    }

    #[test]
    fn byte_in_flight_when_son_sets_is_lost() {
        let mut u = Uart::new();
        u.write(BAU, 6);
        u.push(&[1, 2]);
        run(&mut u, 50);
        u.write(IOC, IOC_SON);
        run(&mut u, 132);
        assert_eq!(u.peek(RCS), RCS_RBZ, "the next start bit is seen");
        run(&mut u, 160);
        assert_eq!(read_rbr(&mut u), 2);
    }

    #[test]
    fn loop_back() {
        let mut u = on(0);
        u.write(TCS, TCS_LPB);
        u.push(&[0x99]);
        send(&mut u, 0x42);
        run(&mut u, 1);
        assert_eq!(u.peek(RCS), RCS_RBZ, "the receiver sees our start bit");
        run(&mut u, 159);
        assert_eq!(u.peek(RCS), RCS_RBZ);
        run(&mut u, 1);
        assert_eq!(u.peek(RCS), RCS_RBF);
        assert_eq!(read_rbr(&mut u), 0x42);
        run(&mut u, 400);
        assert!(u.drain().is_empty(), "nothing on the wire");
        assert_eq!(u.peek(RCS), 0, "wire bytes are ignored");
        assert_eq!(u.pending(), 0);
        assert_eq!(u.peek(TCS), TCS_LPB);
    }

    #[test]
    fn looped_back_break() {
        let mut u = on(0);
        u.write(TCS, TCS_LPB | TCS_BRK);
        assert_eq!(u.peek(RCS), RCS_RBZ);
        run(&mut u, 160);
        assert_eq!(u.peek(RCS), RCS_RBF | RCS_RER);
        assert_eq!(read_rbr(&mut u), 0);
        run(&mut u, 5000);
        assert_eq!(u.peek(RCS), RCS_RER, "a long break is one byte");
        u.write(TCS, TCS_LPB);
        u.write(CRER, 0);
        assert_eq!(u.peek(RCS), 0);
        assert_eq!(u.brk, None);
    }

    #[test]
    fn interrupt_enables() {
        // Receive start.
        let mut u = on(IOC_ERBZ);
        u.push(&[1]);
        run(&mut u, 1);
        assert!(u.take_interrupt());
        assert_eq!(u.peek(SRQ1), SRQ1_USRQ);
        run(&mut u, 159);
        assert!(!u.take_interrupt());
        assert_eq!(u.peek(SRQ1), 0, "RBZ dropped");
        // Receive full; the RBR read re-evaluates the request.
        let mut u = on(IOC_ERBF);
        u.push(&[1, 2]);
        run(&mut u, 159);
        assert!(!u.take_interrupt());
        run(&mut u, 1);
        assert!(u.take_interrupt());
        read_rbr(&mut u);
        assert_eq!(u.peek(SRQ1), 0);
        run(&mut u, 182);
        assert!(u.take_interrupt(), "next byte, new edge");
        // Transmit empty: enabling it with TBR empty raises one at once.
        let mut u = on(0);
        u.write(IOC, IOC_SON | IOC_ETBE);
        assert!(u.take_interrupt());
        send(&mut u, 0x30);
        assert_eq!(u.peek(SRQ1), 0, "TBF set: request drops");
        run(&mut u, 1);
        assert!(u.take_interrupt(), "byte moved to the shifter");
        // No enables, no requests; SON off masks everything.
        let mut u = on(0);
        u.push(&[1]);
        run(&mut u, 200);
        assert!(!u.take_interrupt());
        let mut u = Uart::new();
        u.write(IOC, IOC_ETBE);
        assert!(!u.take_interrupt());
    }

    #[test]
    fn next_event_in_cycles() {
        let mut u = on(0);
        assert_eq!(u.cycles_until_event(2_000_000), None);
        u.push(&[1]);
        assert_eq!(u.cycles_until_event(2_000_000), Some(0));
        u.advance(1, 2_000_000);
        // 160 sixteenths at 13.02 cycles each, rounded up, minus the cycle
        // already run.
        let c = u.cycles_until_event(2_000_000).unwrap();
        assert_eq!(c, (160 * 2_000_000u64).div_ceil(153_600) - 1);
        u.advance(c - 1, 2_000_000);
        assert_eq!(u.peek(RCS), RCS_RBZ);
        u.advance(1, 2_000_000);
        assert_eq!(u.peek(RCS), RCS_RBF);
    }
}
