//! The two HP48 hardware timers (wiki: hardware/timers).
//!
//! TIMER2 is a 32-bit down-counter clocked at 8192 Hz; TIMER1 is a 4-bit
//! down-counter clocked at 16 Hz (once every 512 TIMER2 ticks). Both expire
//! when they count through zero, i.e. when their most significant bit becomes
//! set (wiki: questions/timer-expiry-semantics, wiki: emulators/emu48 SP8).
//!
//! While a TIMER2 interrupt is pending, reads of TIMER2 return #FFFFFFFF
//! (wiki: emulators/emu48 SP43). "Pending" is taken to mean: the expiry
//! edge happened and the CPU has not vectored to the handler since, which
//! in practice means the CPU was in service when TIMER2 expired. A write
//! to TIMER2 also ends it, so software reads back what it wrote (inferred;
//! the source does not say).

/// TIMER2 rate: ticks per second.
pub const TICKS_PER_SECOND: u32 = 8192;
/// Number of TIMER2 ticks per TIMER1 tick (TIMER1 runs at 16 Hz).
pub const T2_TICKS_PER_T1_TICK: u32 = 512;

/// Control bit 0: XTRA for TIMER1, TRUN (timer run) for TIMER2.
pub const CTRL_XTRA_OR_RUN: u8 = 1;
/// Control bit 1: raise an interrupt when the timer has expired.
pub const CTRL_INT: u8 = 2;
/// Control bit 2: wake the CPU from SHUTDN when the timer has expired.
pub const CTRL_WAKE: u8 = 4;
/// Control bit 3: service request ("timer needs service"), computed on read.
pub const CTRL_SRQ: u8 = 8;

/// State of TIMER1 and TIMER2 with their control nibbles.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Timers {
    /// TIMER1 value (4 bits).
    pub t1: u8,
    /// TIMER2 value (32 bits).
    pub t2: u32,
    /// TIMER1 control nibble, stored bits 0-2 only.
    pub t1_ctrl: u8,
    /// TIMER2 control nibble, stored bits 0-2 only.
    pub t2_ctrl: u8,
    /// TIMER2 ticks accumulated towards the next TIMER1 decrement (0..512).
    pub(crate) t1_phase: u32,
    /// Last evaluated interrupt level of TIMER1.
    pub(crate) t1_irq: bool,
    /// Last evaluated interrupt level of TIMER2.
    pub(crate) t2_irq: bool,
    /// Latched rising edge of either interrupt level.
    pub(crate) irq_edge: bool,
    /// A TIMER2 interrupt edge has not been taken by the CPU yet.
    pub(crate) t2_pending: bool,
}

impl Timers {
    /// Power-on state: both timers 0, stopped, no interrupts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether TIMER2 is counting (TRUN, control bit 0).
    pub fn t2_running(&self) -> bool {
        self.t2_ctrl & CTRL_XTRA_OR_RUN != 0
    }

    fn t1_msb(&self) -> bool {
        self.t1 & 0x8 != 0
    }

    fn t2_msb(&self) -> bool {
        self.t2 & 0x8000_0000 != 0
    }

    /// Advance both timers by `ticks` 8192-Hz ticks.
    ///
    /// Time is skipped in bulk up to the next MSB transition of either timer,
    /// where the interrupt levels are re-evaluated, so a large batch costs
    /// only a few steps and still catches every expiry.
    pub fn tick(&mut self, ticks: u32) {
        let mut remaining = ticks;
        while remaining > 0 && self.t2_running() {
            let step = remaining.min(self.ticks_until_event());
            self.advance(step);
            self.update_irq();
            remaining -= step;
        }
    }

    /// Advance by `n` ticks, assuming no MSB transition happens strictly
    /// before the last of them.
    fn advance(&mut self, n: u32) {
        self.t2 = self.t2.wrapping_sub(n);
        // TIMER1 only advances while TIMER2 runs (wiki: emulators/emu48 SP39).
        let total = self.t1_phase + n % T2_TICKS_PER_T1_TICK;
        let decrements = n / T2_TICKS_PER_T1_TICK + total / T2_TICKS_PER_T1_TICK;
        self.t1_phase = total % T2_TICKS_PER_T1_TICK;
        // Only the low 4 bits matter, so reduce the decrement count mod 16.
        self.t1 = (self.t1.wrapping_sub((decrements % 16) as u8)) & 0xF;
    }

    /// Re-evaluate both interrupt levels and latch a rising edge.
    fn update_irq(&mut self) {
        let t1_level = self.t1_msb() && self.t1_ctrl & CTRL_INT != 0;
        let t2_level = self.t2_msb() && self.t2_ctrl & CTRL_INT != 0;
        if t1_level && !self.t1_irq {
            self.irq_edge = true;
        }
        if t2_level && !self.t2_irq {
            self.irq_edge = true;
            self.t2_pending = true;
        }
        self.t1_irq = t1_level;
        self.t2_irq = t2_level;
    }

    /// CPU write of the TIMER1 value. Reloading the value it already holds
    /// keeps the running 1/16 s period; a different value restarts it
    /// (wiki: emulators/emu48 SP4/SP12).
    pub fn write_t1(&mut self, v: u8) {
        let v = v & 0xF;
        if v != self.t1 {
            self.t1_phase = 0;
        }
        self.t1 = v;
        self.update_irq();
    }

    /// CPU write of TIMER2 nibble `idx` (0 = least significant, 0..8).
    /// Counting continues.
    pub fn write_t2_nibble(&mut self, idx: u8, v: u8) {
        self.t2_pending = false;
        let shift = u32::from(idx & 7) * 4;
        self.t2 = (self.t2 & !(0xF << shift)) | (u32::from(v & 0xF) << shift);
        self.update_irq();
    }

    /// CPU read of TIMER2 nibble `idx` (0 = least significant, 0..8): #F
    /// while a TIMER2 interrupt is pending.
    pub fn read_t2_nibble(&self, idx: u8) -> u8 {
        if self.t2_pending {
            0xF
        } else {
            ((self.t2 >> (u32::from(idx & 7) * 4)) & 0xF) as u8
        }
    }

    /// The CPU entered the interrupt handler: a pending TIMER2 interrupt
    /// has been taken.
    pub(crate) fn interrupt_taken(&mut self) {
        self.t2_pending = false;
    }

    /// CPU write of the TIMER1 control nibble; bit 3 (SRQ) is read-only.
    pub fn write_t1_ctrl(&mut self, v: u8) {
        self.t1_ctrl = v & 0x7;
        self.update_irq();
    }

    /// CPU write of the TIMER2 control nibble; bit 3 (SRQ) is read-only.
    pub fn write_t2_ctrl(&mut self, v: u8) {
        self.t2_ctrl = v & 0x7;
        self.update_irq();
    }

    /// SRQ bit for a timer: expired and either INT or WAKE enabled.
    ///
    /// Computed on every read rather than stored (inferred from Voyage p.203
    /// and emu48's "control bits re-evaluated after every timer read or
    /// write"; wiki: emulators/emu48).
    fn srq(msb: bool, ctrl: u8) -> u8 {
        if msb && ctrl & (CTRL_INT | CTRL_WAKE) != 0 {
            CTRL_SRQ
        } else {
            0
        }
    }

    /// CPU read of the TIMER1 control nibble: stored bits 0-2 | computed SRQ.
    pub fn read_t1_ctrl(&self) -> u8 {
        self.t1_ctrl | Self::srq(self.t1_msb(), self.t1_ctrl)
    }

    /// CPU read of the TIMER2 control nibble: stored bits 0-2 | computed SRQ.
    pub fn read_t2_ctrl(&self) -> u8 {
        self.t2_ctrl | Self::srq(self.t2_msb(), self.t2_ctrl)
    }

    /// Return and clear the latched rising edge of a timer interrupt level.
    pub fn take_interrupt(&mut self) -> bool {
        std::mem::take(&mut self.irq_edge)
    }

    /// Whether either timer requests a wake-up from SHUTDN: expired with the
    /// WAKE bit set.
    pub fn wake(&self) -> bool {
        (self.t1_msb() && self.t1_ctrl & CTRL_WAKE != 0)
            || (self.t2_msb() && self.t2_ctrl & CTRL_WAKE != 0)
    }

    /// Ticks until the next MSB change of either timer, or `u32::MAX` when
    /// TIMER2 is stopped (then neither timer moves). Lets the machine skip
    /// time while the CPU is in SHUTDN.
    pub fn ticks_until_event(&self) -> u32 {
        if !self.t2_running() {
            return u32::MAX;
        }
        let t2 = if self.t2_msb() {
            // Drop from 0x80000000.. down to 0x7FFFFFFF.
            self.t2 - 0x7FFF_FFFF
        } else {
            // Count through zero into 0xFFFFFFFF.
            self.t2 + 1
        };
        let t1_steps = if self.t1_msb() {
            u32::from(self.t1) - 7
        } else {
            u32::from(self.t1) + 1
        };
        let t1 = t1_steps * T2_TICKS_PER_T1_TICK - self.t1_phase;
        t1.min(t2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running(int: bool) -> Timers {
        let mut t = Timers::new();
        let ctrl = CTRL_XTRA_OR_RUN | if int { CTRL_INT } else { 0 };
        t.write_t2_ctrl(ctrl);
        t
    }

    #[test]
    fn t2_expires_through_zero() {
        let mut t = running(true);
        t.t2 = 2;
        t.tick(2);
        assert_eq!(t.t2, 0);
        assert!(!t.take_interrupt());
        assert_eq!(t.read_t2_ctrl() & CTRL_SRQ, 0);
        t.tick(1);
        assert_eq!(t.t2, 0xFFFF_FFFF);
        assert!(t.take_interrupt());
        assert!(!t.take_interrupt());
        assert_eq!(t.read_t2_ctrl(), CTRL_XTRA_OR_RUN | CTRL_INT | CTRL_SRQ);
    }

    #[test]
    fn bulk_tick_fires_one_edge() {
        let mut t = running(true);
        t.t2 = 10;
        t.tick(1_000_000);
        assert_eq!(t.t2, 10u32.wrapping_sub(1_000_000));
        assert!(t.take_interrupt());
        assert!(!t.take_interrupt());
    }

    #[test]
    fn bulk_tick_matches_single_steps() {
        let mut a = running(true);
        a.t2 = 3000;
        a.t1 = 2;
        a.write_t1_ctrl(CTRL_INT);
        let mut b = a.clone();
        a.tick(5000);
        for _ in 0..5000 {
            b.tick(1);
        }
        assert_eq!(a, b);
    }

    #[test]
    fn t1_stops_without_trun() {
        let mut t = Timers::new();
        t.write_t1(5);
        t.tick(10_000);
        assert_eq!(t.t1, 5);
        assert_eq!(t.t2, 0);
    }

    #[test]
    fn t1_decrements_every_512_ticks() {
        let mut t = running(false);
        t.t2 = 0x7000_0000;
        t.write_t1(5);
        t.tick(511);
        assert_eq!(t.t1, 5);
        t.tick(1);
        assert_eq!(t.t1, 4);
        t.tick(512 * 4);
        assert_eq!(t.t1, 0);
        t.tick(512);
        assert_eq!(t.t1, 0xF);
    }

    #[test]
    fn t1_reload_same_value_keeps_phase() {
        let mut t = running(false);
        t.t2 = 0x7000_0000;
        t.write_t1(5);
        t.tick(300);
        t.write_t1(5);
        t.tick(212);
        assert_eq!(t.t1, 4);

        // A different value restarts the period.
        t.tick(300);
        t.write_t1(6);
        t.tick(212);
        assert_eq!(t.t1, 6);
        t.tick(300);
        assert_eq!(t.t1, 5);
    }

    #[test]
    fn ticks_until_event_simple() {
        let mut t = Timers::new();
        assert_eq!(t.ticks_until_event(), u32::MAX);
        t.write_t2_ctrl(CTRL_XTRA_OR_RUN);
        t.t2 = 100;
        t.t1 = 3;
        assert_eq!(t.ticks_until_event(), 101);
        t.t2 = 0x8000_0005;
        assert_eq!(t.ticks_until_event(), 6);
        t.t2 = 0x7000_0000;
        t.tick(12);
        // TIMER1 needs 4 decrements to wrap, minus 12 ticks of phase.
        assert_eq!(t.ticks_until_event(), 4 * 512 - 12);
    }

    #[test]
    fn wake_needs_wake_bit() {
        let mut t = running(false);
        t.t2 = 0;
        t.tick(1);
        assert!(!t.wake());
        assert_eq!(t.read_t2_ctrl() & CTRL_SRQ, 0);
        t.write_t2_ctrl(CTRL_XTRA_OR_RUN | CTRL_WAKE);
        assert!(t.wake());
        assert_eq!(t.read_t2_ctrl() & CTRL_SRQ, CTRL_SRQ);
        assert!(!t.take_interrupt());
    }

    #[test]
    fn enabling_int_on_expired_timer_is_an_edge() {
        let mut t = Timers::new();
        t.write_t1(0x8);
        assert!(!t.take_interrupt());
        t.write_t1_ctrl(CTRL_INT);
        assert!(t.take_interrupt());
        t.write_t1_ctrl(CTRL_INT | 0x8);
        assert_eq!(t.t1_ctrl, CTRL_INT);
        assert!(!t.take_interrupt());
    }
}

#[cfg(test)]
mod pending_tests {
    use super::*;

    #[test]
    fn t2_reads_all_ones_while_its_interrupt_is_pending() {
        let mut t = Timers::new();
        t.write_t2_ctrl(CTRL_XTRA_OR_RUN | CTRL_INT);
        t.t2 = 1;
        t.tick(5);
        assert_eq!(t.t2, 0xFFFF_FFFC);
        assert!(t.take_interrupt());
        assert_eq!(t.read_t2_nibble(0), 0xF);
        t.interrupt_taken();
        assert_eq!(t.read_t2_nibble(0), 0xC);
        assert_eq!(t.read_t2_nibble(7), 0xF);
        // A write ends it too.
        t.t2 = 1;
        t.tick(2);
        assert_eq!(t.read_t2_nibble(0), 0xF);
        t.write_t2_nibble(7, 0);
        assert_eq!(t.read_t2_nibble(0), 0xF, "the counter itself is #...F");
        assert_eq!(t.read_t2_nibble(7), 0);
        // TIMER1 expiry does not set it.
        let mut t = Timers::new();
        t.write_t2_ctrl(CTRL_XTRA_OR_RUN);
        t.t2 = 0x7000_0000;
        t.write_t1_ctrl(CTRL_INT);
        t.tick(512);
        assert!(t.take_interrupt());
        assert!(!t.t2_pending);
    }
}
