//! Instruction execution: the [`Cpu`] and its `step`.
//!
//! wiki: hardware/saturn-cpu; src: SASM manual sections 2, 6 and 8
//! (authoritative), Fernandes/Rechlin tutorial ch. 33-52, Gariepy's HP-28S
//! processor notes and Mastracci's Saturn guide as cross-checks.
//!
//! # Execution model
//!
//! `step` decodes at PC, advances PC past the instruction (mod 2^20) and
//! only then executes it. So a branch that writes PC wins, and every
//! instruction that observes PC (A=PC, APCEX, GOSUB's return address, the
//! address pushed on interrupt) sees the address of the next instruction:
//! "The program counter contains the 20-bit address of the next instruction
//! to be executed by the CPU. It is incremented by the chip after reading
//! each instruction" (Mastracci, Program Counter); APCEX/CPCEX "have the
//! effect of saving the PC of the next instruction" (Gariepy, SWAP Register
//! with PC). SASM 6.11.3 only says "Copy current PC into A(A)".
//!
//! # Flags
//!
//! Arithmetic (add/sub/inc/dec/neg/not, constants, C+P+1, D0/D1 +-, P+-)
//! sets carry from the ALU; logic, copies, exchanges, shifts and loads leave
//! it alone; tests set carry to their result (SASM 8 "Adjusts Carry", 2.6).
//! Shifts OR their lost-nibble/bit result into HST SB and never clear it
//! (SASM 2.6). See `alu.rs` for the per-operation details.

use super::alu;
use super::bus::{Bus, BusCommand};
use super::cycles::cycles;
use super::decode::decode;
use super::instr::{Cmp, DatSize, Instruction, OnTrue, Ptr, Reg};
use super::regs::{
    ADDR_MASK, HST_SB, HST_SR, HST_XM, Mode, Registers, field_get, field_set, get_nibble,
    set_nibble,
};

/// The interrupt vector: every interrupt jumps here (src: SASM manual 2.10;
/// wiki: hardware/interrupts).
pub const INTERRUPT_VECTOR: u32 = 0x0000F;

/// Something the machine around the CPU must react to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// SHUTDN executed: the CPU stops its clock until a wake-up condition
    /// (src: SASM manual 8 (SHUTDN); wiki: hardware/interrupts "SHUTDN and
    /// wake-up"). The machine decides when to resume; with OUT = 000 SASM
    /// says the PC is set to zero (cold start / system halt on the HP 48),
    /// which is also left to the machine.
    Shutdown,
    /// An undefined opcode at `pc`. PC has already advanced by `len`.
    /// `nibbles[..len]` are the nibbles the decoder read.
    InvalidOpcode { pc: u32, nibbles: [u8; 8], len: u8 },
    /// RTI executed (after the pending-interrupt re-entry, if any).
    Rti,
}

/// Result of one [`Cpu::step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    /// Approximate cycle count (see `cycles.rs`).
    pub cycles: u32,
    /// Event the machine must handle, if any.
    pub event: Option<Event>,
}

/// The Saturn CPU: register file plus the execution engine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cpu {
    pub regs: Registers,
}

/// Read `n` nibbles little-endian starting at `addr` (20-bit wrap).
fn read_nibbles(bus: &mut impl Bus, addr: u32, n: u32) -> u64 {
    (0..n).fold(0u64, |acc, i| {
        let nib = bus.read_nibble(addr.wrapping_add(i) & ADDR_MASK) & 0xF;
        acc | (u64::from(nib) << (4 * i))
    })
}

/// Nibble range transferred by a DAT instruction. The field form uses
/// `field.range(p)`; the count form moves nibbles 0..d-1 ("If fs = d, d
/// nibbles are transferred into the register starting at nibble 0", SASM 8
/// A=DAT0).
fn dat_range(size: DatSize, p: u8) -> (u8, u8) {
    match size {
        DatSize::Field(f) => f.range(p),
        DatSize::Nibbles(n) => (0, n.clamp(1, 16) - 1),
    }
}

impl Cpu {
    /// A CPU in the reset state (see [`Registers::reset`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the register file.
    pub fn reset(&mut self) {
        self.regs.reset();
    }

    fn reg(&self, r: Reg) -> u64 {
        self.regs.get(r)
    }

    fn set_reg(&mut self, r: Reg, v: u64) {
        self.regs.set(r, v);
    }

    fn ptr(&self, p: Ptr) -> u32 {
        match p {
            Ptr::D0 => self.regs.d0,
            Ptr::D1 => self.regs.d1,
        }
    }

    fn set_ptr(&mut self, p: Ptr, v: u32) {
        match p {
            Ptr::D0 => self.regs.set_d0(v),
            Ptr::D1 => self.regs.set_d1(v),
        }
    }

    /// Low 20 bits (A field) of a working register.
    fn reg_a(&self, r: Reg) -> u32 {
        (self.reg(r) & u64::from(ADDR_MASK)) as u32
    }

    fn set_reg_a(&mut self, r: Reg, v: u32) {
        let new = field_set(self.reg(r), 0, 4, u64::from(v & ADDR_MASK));
        self.set_reg(r, new);
    }

    fn or_sb(&mut self, sb: bool) {
        if sb {
            self.regs.hst |= HST_SB;
        }
    }

    /// Enter the interrupt handler now: push PC, set `in_interrupt`, jump to
    /// #0000F.
    fn vector(&mut self) {
        let pc = self.regs.pc;
        self.regs.push(pc);
        self.regs.in_interrupt = true;
        self.regs.set_pc(INTERRUPT_VECTOR);
    }

    /// Raise an interrupt (wiki: hardware/interrupts "CPU behaviour",
    /// "Interrupt-in-service flag"). If no interrupt is in service, push PC
    /// on RSTK, set `in_interrupt` and jump to #0000F. Otherwise set
    /// `interrupt_pending`, so the handler runs again right after its RTI.
    ///
    /// This does not look at `interrupts_enabled`: which sources INTOFF
    /// masks (keyboard only, per Ervin 4.1.2; ON and timers are not) is the
    /// machine's decision, so the caller filters maskable sources.
    pub fn interrupt(&mut self) {
        if self.regs.in_interrupt {
            self.regs.interrupt_pending = true;
        } else {
            self.vector();
        }
    }

    /// Execute exactly one instruction.
    pub fn step(&mut self, bus: &mut impl Bus) -> Step {
        let pc = self.regs.pc & ADDR_MASK;
        let d = decode(|a| bus.read_nibble(a & ADDR_MASK), pc);
        self.regs.set_pc(pc.wrapping_add(u32::from(d.len)));
        let p = self.regs.p;
        let (taken, event) = self.execute(&d.instr, pc, bus);
        Step {
            cycles: cycles(&d.instr, p, taken),
            event,
        }
    }

    /// Branch to `target` if `cond`; returns `cond`.
    fn go_if(&mut self, cond: bool, target: u32) -> bool {
        if cond {
            self.regs.set_pc(target);
        }
        cond
    }

    /// Return (pop RSTK into PC) if `cond`; returns `cond`.
    fn rtn_if(&mut self, cond: bool) -> bool {
        if cond {
            let a = self.regs.pop();
            self.regs.set_pc(a);
        }
        cond
    }

    /// Second half of every test: carry = result, then GOYES/RTNYES when
    /// true (src: SASM manual 2.6, 8 "?A=B ... Adjusts Carry"; wiki:
    /// hardware/saturn-cpu "tests set it when true and clear it when false").
    fn test(&mut self, result: bool, yes: OnTrue) -> bool {
        self.regs.carry = result;
        match yes {
            OnTrue::RtnYes => self.rtn_if(result),
            OnTrue::GoYes(target) => self.go_if(result, target),
        }
    }

    /// Execute a decoded instruction; PC already points past it. Returns
    /// whether a conditional branch/return was taken (for cycle counting)
    /// and any event.
    fn execute(
        &mut self,
        instr: &Instruction,
        pc: u32,
        bus: &mut impl Bus,
    ) -> (bool, Option<Event>) {
        use Instruction as I;
        let p = self.regs.p;
        let mode = self.regs.mode;
        let mut taken = false;
        let mut event = None;
        match *instr {
            // ----- returns -----
            I::RtnSxm => {
                self.regs.hst |= HST_XM;
                self.rtn_if(true);
            }
            I::Rtn => {
                self.rtn_if(true);
            }
            I::RtnSc => {
                self.regs.carry = true;
                self.rtn_if(true);
            }
            I::RtnCc => {
                self.regs.carry = false;
                self.rtn_if(true);
            }
            I::Rti => {
                // "Return and re-enable the interrupt system" (SASM 8 RTI). A
                // pending interrupt re-enters the handler at once (wiki:
                // hardware/interrupts, Mastracci 2.3).
                self.rtn_if(true);
                self.regs.in_interrupt = false;
                if self.regs.interrupt_pending {
                    self.regs.interrupt_pending = false;
                    self.vector();
                }
                event = Some(Event::Rti);
            }
            I::RtnC => taken = self.rtn_if(self.regs.carry),
            I::RtnNc => taken = self.rtn_if(!self.regs.carry),

            I::SetHex => self.regs.mode = Mode::Hex,
            I::SetDec => self.regs.mode = Mode::Dec,

            // ----- RSTK (SASM 8 RSTK=C, C=RSTK: A field only) -----
            I::RstkEqC => {
                let c = self.reg_a(Reg::C);
                self.regs.push(c);
            }
            I::CEqRstk => {
                let a = self.regs.pop();
                self.set_reg_a(Reg::C, a);
            }

            // ----- ST: C=ST/ST=C/CSTEX/CLRST act on bits 11-0 only (SASM
            // 6.11.1; wiki: hardware/saturn-cpu) -----
            I::ClrSt => self.regs.st &= 0xF000,
            I::CEqSt => {
                let c = field_set(self.regs.c, 0, 2, u64::from(self.regs.st & 0x0FFF));
                self.regs.c = c;
            }
            I::StEqC => {
                let x = field_get(self.regs.c, 0, 2) as u16;
                self.regs.st = (self.regs.st & 0xF000) | x;
            }
            I::CStEx => {
                let x = field_get(self.regs.c, 0, 2) as u16;
                self.regs.c = field_set(self.regs.c, 0, 2, u64::from(self.regs.st & 0x0FFF));
                self.regs.st = (self.regs.st & 0xF000) | x;
            }
            I::StClear { bit } => self.regs.st &= !(1u16 << (bit & 0xF)),
            I::StSet { bit } => self.regs.st |= 1u16 << (bit & 0xF),
            I::TestSt { bit, set, yes } => {
                let v = self.regs.st & (1u16 << (bit & 0xF)) != 0;
                taken = self.test(v == set, yes);
            }

            // ----- HST (SASM 8 CLRHST: "82x, where x is merely a mask") -----
            I::HsClear { mask } => self.regs.hst &= !(mask & 0xF),
            I::TestHs { mask, yes } => {
                // ?HS=0 n: "True if all bits corresponding to n are clear"
                // (SASM 6.8.4).
                taken = self.test(self.regs.hst & mask & 0xF == 0, yes);
            }

            // ----- P register (SASM 6.9: pointer arithmetic is always HEX) -----
            I::PInc => {
                let (v, c) = alu::p_inc(p);
                self.regs.set_p(v);
                self.regs.carry = c;
            }
            I::PDec => {
                let (v, c) = alu::p_dec(p);
                self.regs.set_p(v);
                self.regs.carry = c;
            }
            I::PSet { n } => self.regs.set_p(n),
            I::CPlusPPlus1 => {
                let (v, c) = alu::c_plus_p_plus_1(self.regs.c, p);
                self.regs.c = v;
                self.regs.carry = c;
            }
            I::CEqP { n } => self.regs.c = set_nibble(self.regs.c, n, p),
            I::PEqC { n } => self.regs.set_p(get_nibble(self.regs.c, n)),
            I::CpEx { n } => {
                let old = get_nibble(self.regs.c, n);
                self.regs.c = set_nibble(self.regs.c, n, p);
                self.regs.set_p(old);
            }
            I::TestP { n, eq, yes } => {
                let same = p == (n & 0xF);
                taken = self.test(same == eq, yes);
            }

            // ----- constants: LC/LA load from nibble P upward, wrapping
            // 15 -> 0; carry untouched (SASM 8 LC(m); wiki: hardware/saturn-cpu)
            I::LoadConst {
                reg,
                nibbles,
                value,
            } => {
                let mut r = self.reg(reg);
                for i in 0..nibbles.min(16) {
                    let nib = ((value >> (4 * u32::from(i))) & 0xF) as u8;
                    r = set_nibble(r, p.wrapping_add(i) & 0xF, nib);
                }
                self.set_reg(reg, r);
            }

            // ----- logic (carry not affected, SASM 8 A=A&B) -----
            I::And { dst, src, field } => {
                let (lo, hi) = field.range(p);
                let v = alu::and(self.reg(dst), self.reg(src), lo, hi);
                self.set_reg(dst, v);
            }
            I::Or { dst, src, field } => {
                let (lo, hi) = field.range(p);
                let v = alu::or(self.reg(dst), self.reg(src), lo, hi);
                self.set_reg(dst, v);
            }

            // ----- scratch registers (whole register or field) -----
            I::ScratchFromReg { ss, src, field } => {
                let i = ss.index();
                let v = self.reg(src);
                self.regs.r[i] = match field {
                    None => v,
                    Some(f) => {
                        let (lo, hi) = f.range(p);
                        alu::copy(self.regs.r[i], v, lo, hi)
                    }
                };
            }
            I::RegFromScratch { dst, ss, field } => {
                let s = self.regs.r[ss.index()];
                let v = match field {
                    None => s,
                    Some(f) => {
                        let (lo, hi) = f.range(p);
                        alu::copy(self.reg(dst), s, lo, hi)
                    }
                };
                self.set_reg(dst, v);
            }
            I::RegScratchEx { reg, ss, field } => {
                let i = ss.index();
                let (lo, hi) = field.map_or((0, 15), |f| f.range(p));
                let (r, s) = alu::exchange(self.reg(reg), self.regs.r[i], lo, hi);
                self.set_reg(reg, r);
                self.regs.r[i] = s;
            }

            // ----- data pointers -----
            I::PtrFromReg { ptr, src, short } => {
                // D0=A: nibbles 0-4; D0=AS: "The lower 4 nibbles of A are
                // copied into the lower 4 nibbles of Data pointer register
                // D0" (SASM 8 D0=AS). Carry not affected.
                let v = self.reg_a(src);
                let new = if short {
                    (self.ptr(ptr) & 0xF_0000) | (v & 0xFFFF)
                } else {
                    v
                };
                self.set_ptr(ptr, new);
            }
            I::PtrRegEx { ptr, reg, short } => {
                // AD0EX: nibbles 0-4; AD0XS: "Exchange the lower 4 nibbles
                // of A with the lower 4 nibbles of Data pointer D0" (SASM 8
                // AD0XS). Carry not affected.
                let d = self.ptr(ptr);
                let (top, mask) = if short {
                    (3u8, 0xFFFFu32)
                } else {
                    (4, ADDR_MASK)
                };
                let r = self.reg(reg);
                let new_r = field_set(r, 0, top, u64::from(d & mask));
                let new_d = (d & !mask) | ((field_get(r, 0, top) as u32) & mask);
                self.set_reg(reg, new_r);
                self.set_ptr(ptr, new_d);
            }
            I::PtrAdd { ptr, n } => {
                let (v, c) = alu::ptr_add(self.ptr(ptr), n);
                self.set_ptr(ptr, v);
                self.regs.carry = c;
            }
            I::PtrSub { ptr, n } => {
                let (v, c) = alu::ptr_sub(self.ptr(ptr), n);
                self.set_ptr(ptr, v);
                self.regs.carry = c;
            }
            I::PtrLoad {
                ptr,
                nibbles,
                value,
            } => {
                // D0=(2)/(4) replace only the low 2/4 nibbles, D0=(5) all
                // (wiki: hardware/saturn-cpu, tutorial p. 75-77).
                let mask = match nibbles {
                    2 => 0xFFu32,
                    4 => 0xFFFF,
                    _ => ADDR_MASK,
                };
                let new = (self.ptr(ptr) & !mask) | (value & mask);
                self.set_ptr(ptr, new);
            }

            // ----- memory: lowest address <-> lowest nibble of the field
            // (SASM 8 A=DAT0, DAT0=A); addresses wrap at 20 bits -----
            I::DatWrite { ptr, src, size } => {
                let (lo, hi) = dat_range(size, p);
                let base = self.ptr(ptr);
                let r = self.reg(src);
                for (i, n) in (lo..=hi).enumerate() {
                    let addr = base.wrapping_add(i as u32) & ADDR_MASK;
                    bus.write_nibble(addr, get_nibble(r, n));
                }
            }
            I::DatRead { dst, ptr, size } => {
                let (lo, hi) = dat_range(size, p);
                let base = self.ptr(ptr);
                let mut r = self.reg(dst);
                for (i, n) in (lo..=hi).enumerate() {
                    let addr = base.wrapping_add(i as u32) & ADDR_MASK;
                    r = set_nibble(r, n, bus.read_nibble(addr));
                }
                self.set_reg(dst, r);
            }

            // ----- jumps and calls (targets already absolute) -----
            I::Goc { target } => taken = self.go_if(self.regs.carry, target),
            I::Gonc { target } => taken = self.go_if(!self.regs.carry, target),
            I::Goto { target } | I::GoLong { target } | I::GoVLong { target } => {
                self.go_if(true, target);
            }
            I::Gosub { target } | I::GosubL { target } | I::GosbVL { target } => {
                // Return address = the instruction after the call (SASM 8
                // GOSUB), i.e. the already-advanced PC.
                let ret = self.regs.pc;
                self.regs.push(ret);
                self.go_if(true, target);
            }
            I::PcEqReg { reg } => {
                let a = self.reg_a(reg);
                self.regs.set_pc(a);
            }
            I::PcEqInd { reg } => {
                // PC=mem(A[A]) (SASM 8 PC=(A)).
                let a = self.reg_a(reg);
                let target = read_nibbles(bus, a, 5) as u32;
                self.regs.set_pc(target);
            }
            I::RegPcEx { reg } => {
                // Saves the address of the next instruction (Gariepy, SWAP
                // Register with PC); see the module docs.
                let next = self.regs.pc;
                let a = self.reg_a(reg);
                self.regs.set_pc(a);
                self.set_reg_a(reg, next);
            }
            I::RegEqPc { reg } => {
                let next = self.regs.pc;
                self.set_reg_a(reg, next);
            }
            I::Nop3 | I::Nop4 | I::Nop5 => {}

            // ----- chip interface -----
            I::OutCs => {
                // "The least significant nibble of the Output register is
                // loaded with the least significant nibble of the C register"
                // (SASM 8 OUT=CS).
                let out = (self.regs.out & 0xFF0) | u16::from(get_nibble(self.regs.c, 0));
                self.regs.set_out(out);
                bus.write_out(self.regs.out);
            }
            I::OutC => {
                self.regs.set_out(field_get(self.regs.c, 0, 2) as u16);
                bus.write_out(self.regs.out);
            }
            I::In { dst } => {
                // Low 4 nibbles = IN (SASM 8 A=IN, C=IN). The even-address
                // restriction is deliberately not modelled: its failure mode
                // is undocumented and ROM code goes through AINRTN/CINRTN
                // (wiki: questions/c-equals-in-even-address).
                let v = bus.read_in();
                self.regs.inp = v;
                let r = field_set(self.reg(dst), 0, 3, u64::from(v));
                self.set_reg(dst, r);
            }
            I::Uncnfg => bus.unconfig(self.reg_a(Reg::C)),
            I::Config => bus.config(self.reg_a(Reg::C)),
            I::CId => {
                let id = bus.read_id();
                self.set_reg_a(Reg::C, id);
            }
            I::Shutdn => {
                bus.shutdown();
                event = Some(Event::Shutdown);
            }
            I::IntOn => self.regs.interrupts_enabled = true,
            I::IntOff => self.regs.interrupts_enabled = false,
            I::Rsi => {
                // SASM 8 RSI: "causes CPU to consider any input line (ie
                // input register bits) presently high as a new interrupt. If
                // the CPU is presently in the interrupt routine it will wait
                // for an RTI before vectoring, otherwise the CPU will vector
                // immediately following the RSI instruction." Voyage p. 132
                // agrees (wiki: hardware/interrupts "RSI and the ST flags").
                // Modelled as re-arming level detection: the bus reports
                // whether a request is active (and applies any masking);
                // `interrupt()` then latches it as pending when in service,
                // or vectors now with the address after RSI on RSTK. Not
                // modelled: Ervin's "resets the keyboard interrupt state
                // machine" edge-detection detail (wiki: hardware/interrupts).
                if bus.interrupt_pending() {
                    self.interrupt();
                }
            }
            I::Reset => bus.reset(),
            I::BusCb => bus.bus_command(BusCommand::B),
            I::BusCc => bus.bus_command(BusCommand::C),
            I::BusCd => bus.bus_command(BusCommand::D),
            I::Sreq => {
                // C(0) = bus response; SR set if any chip requests service
                // (SASM 8 SREQ?). SR is only set, never cleared here.
                let v = bus.service_request() & 0xF;
                self.regs.c = set_nibble(self.regs.c, 0, v);
                if v != 0 {
                    self.regs.hst |= HST_SR;
                }
            }

            // ----- bits -----
            I::BitClear { reg, bit } => {
                let v = alu::bit_set(self.reg(reg), bit, false);
                self.set_reg(reg, v);
            }
            I::BitSet { reg, bit } => {
                let v = alu::bit_set(self.reg(reg), bit, true);
                self.set_reg(reg, v);
            }
            I::TestBit { reg, bit, set, yes } => {
                let b = alu::bit_get(self.reg(reg), bit);
                taken = self.test(b == set, yes);
            }

            // ----- shifts: SB is ORed in, carry untouched -----
            I::Slc { reg } => {
                let (v, sb) = alu::rol_nibble(self.reg(reg), 0, 15);
                self.set_reg(reg, v);
                self.or_sb(sb);
            }
            I::Src { reg } => {
                let (v, sb) = alu::ror_nibble(self.reg(reg), 0, 15);
                self.set_reg(reg, v);
                self.or_sb(sb);
            }
            I::Srb { reg, field } => {
                let (lo, hi) = field.map_or((0, 15), |f| f.range(p));
                let (v, sb) = alu::shr_bit(self.reg(reg), lo, hi);
                self.set_reg(reg, v);
                self.or_sb(sb);
            }
            I::Sl { reg, field } => {
                let (lo, hi) = field.range(p);
                let (v, sb) = alu::shl_nibble(self.reg(reg), lo, hi);
                self.set_reg(reg, v);
                self.or_sb(sb);
            }
            I::Sr { reg, field } => {
                let (lo, hi) = field.range(p);
                let (v, sb) = alu::shr_nibble(self.reg(reg), lo, hi);
                self.set_reg(reg, v);
                self.or_sb(sb);
            }

            // ----- arithmetic: carry from the ALU -----
            I::Add { dst, src, field } => {
                let (lo, hi) = field.range(p);
                let (v, c) = alu::add(self.reg(dst), self.reg(src), lo, hi, mode);
                self.set_arith(dst, v, c);
            }
            I::Sub { dst, src, field } => {
                let (lo, hi) = field.range(p);
                let (v, c) = alu::sub(self.reg(dst), self.reg(src), lo, hi, mode);
                self.set_arith(dst, v, c);
            }
            I::SubRev { dst, src, field } => {
                // dst = src - dst; nibbles outside the field keep dst's value.
                let (lo, hi) = field.range(p);
                let (v, c) = alu::sub(self.reg(src), self.reg(dst), lo, hi, mode);
                let v = alu::copy(self.reg(dst), v, lo, hi);
                self.set_arith(dst, v, c);
            }
            I::Inc { reg, field } => {
                let (lo, hi) = field.range(p);
                let (v, c) = alu::inc(self.reg(reg), lo, hi, mode);
                self.set_arith(reg, v, c);
            }
            I::Dec { reg, field } => {
                let (lo, hi) = field.range(p);
                let (v, c) = alu::dec(self.reg(reg), lo, hi, mode);
                self.set_arith(reg, v, c);
            }
            I::AddConst { reg, field, n } => {
                let (lo, hi) = field.range(p);
                let (v, c) = alu::add_const(self.reg(reg), n, lo, hi);
                self.set_arith(reg, v, c);
            }
            I::SubConst { reg, field, n } => {
                let (lo, hi) = field.range(p);
                let (v, c) = alu::sub_const(self.reg(reg), n, lo, hi);
                self.set_arith(reg, v, c);
            }
            I::Neg { reg, field } => {
                let (lo, hi) = field.range(p);
                let (v, c) = alu::neg(self.reg(reg), lo, hi, mode);
                self.set_arith(reg, v, c);
            }
            I::Not { reg, field } => {
                let (lo, hi) = field.range(p);
                let (v, c) = alu::not(self.reg(reg), lo, hi, mode);
                self.set_arith(reg, v, c);
            }
            I::Zero { reg, field } => {
                let (lo, hi) = field.range(p);
                let v = alu::clear(self.reg(reg), lo, hi);
                self.set_reg(reg, v);
            }
            I::Copy { dst, src, field } => {
                let (lo, hi) = field.range(p);
                let v = alu::copy(self.reg(dst), self.reg(src), lo, hi);
                self.set_reg(dst, v);
            }
            I::Exch { a, b, field } => {
                let (lo, hi) = field.range(p);
                let (va, vb) = alu::exchange(self.reg(a), self.reg(b), lo, hi);
                self.set_reg(a, va);
                self.set_reg(b, vb);
            }

            // ----- register tests (unsigned, SASM 6.8) -----
            I::TestCmp {
                op,
                lhs,
                rhs,
                field,
                yes,
            } => {
                let (lo, hi) = field.range(p);
                let (l, r) = (self.reg(lhs), self.reg(rhs));
                let result = match op {
                    Cmp::Eq => alu::eq(l, r, lo, hi),
                    Cmp::Ne => alu::ne(l, r, lo, hi),
                    Cmp::Gt => alu::gt(l, r, lo, hi),
                    Cmp::Lt => alu::lt(l, r, lo, hi),
                    Cmp::Ge => alu::ge(l, r, lo, hi),
                    Cmp::Le => alu::le(l, r, lo, hi),
                };
                taken = self.test(result, yes);
            }
            I::TestZero {
                op,
                reg,
                field,
                yes,
            } => {
                let (lo, hi) = field.range(p);
                let z = alu::zero(self.reg(reg), lo, hi);
                let result = if op == Cmp::Ne { !z } else { z };
                taken = self.test(result, yes);
            }

            I::Invalid { nibbles, len } => {
                event = Some(Event::InvalidOpcode { pc, nibbles, len });
            }
        }
        (taken, event)
    }

    fn set_arith(&mut self, r: Reg, v: u64, carry: bool) {
        self.set_reg(r, v);
        self.regs.carry = carry;
    }
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
