//! Saturn CPU: registers, decoder, ALU and instruction execution.
//! wiki: hardware/saturn-cpu

pub mod alu;
pub mod bus;
pub mod cycles;
pub mod decode;
pub mod disasm;
pub mod exec;
pub mod instr;
pub mod regs;

pub use bus::{Bus, BusCommand, FlatMemory};
pub use cycles::{CycleTable, cycles, cycles_g};
pub use decode::{Decoded, decode};
pub use disasm::disassemble;
pub use exec::{Cpu, Event, INTERRUPT_VECTOR, Step};
pub use instr::{Cmp, DatSize, Field, Instruction, OnTrue, Ptr, Reg, Scratch};
pub use regs::{ADDR_MASK, Mode, Registers, ReturnStack};
