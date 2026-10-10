//! Disassembly for traces and listings: decode the instruction at an
//! address and render it in HP SASM syntax (see the CPU's disassembler
//! for the conventions). The instruction set itself stays internal; an
//! [`Instruction`] is only its length and its text.

use std::fmt;

use crate::cpu;

/// One decoded instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction(cpu::Decoded);

impl Instruction {
    /// Its length in nibbles, 1 to 21.
    pub fn nibbles(&self) -> u8 {
        self.0.len
    }
}

/// The SASM text, e.g. `A=A+B   W` or `GOTO    #0A3F2`; an undefined
/// encoding renders as `NIBHEX` of the nibbles read.
impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&cpu::disassemble(&self.0.instr))
    }
}

/// Decode the instruction at nibble address `pc`; `fetch` returns the
/// nibble at a 20-bit address (only its low 4 bits are used), addresses
/// wrap at 20 bits ([`ADDR_MASK`](crate::ADDR_MASK)). For a running
/// machine, `fetch` is [`Machine::peek`](crate::Machine::peek).
pub fn decode(fetch: impl FnMut(u32) -> u8, pc: u32) -> Instruction {
    Instruction(cpu::decode(fetch, pc))
}
