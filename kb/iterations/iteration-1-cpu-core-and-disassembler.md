---
title: "Iteration 1: CPU core and disassembler"
type: iteration
date: 2026-10-04
status: completed
branch: iter-1/cpu-core-and-disassembler
tags:
  - iteration
  - saturnus
---

# Iteration 1: CPU core and disassembler

Read first: wiki `hardware/saturn-cpu`, `sources/saturn-tutorial`,
`sources/sasm-manual` (HP's own), `questions/dec-mode-constant-bug`.

## Tasks

- [x] Registers A, B, C, D (64-bit, nibble fields P, WP, XS, X, S, M, B, W, A),
  R0-R4, D0, D1, PC, P, ST, HST (XM, SB, SR, MP from LSB), carry, DEC/HEX
  mode, 8-level RSTK.
- [x] Decoder for the full opcode table; a disassembler in SASM syntax as a
  by-product (invaluable for debugging; HP's SASM.OPC table is the oracle).
- [x] Quirks from the wiki: A=IN / C=IN only at even addresses; constant
  add/subtract field overrun (decide the DEC-mode question by experiment
  later, record it); pointer and P arithmetic always hex; SB on shifts.
- [x] Tests: one test per instruction family with hand-computed results; a
  round-trip test decode -> disassemble over the whole opcode table.

## Acceptance criteria

every opcode in SASM.OPC decodes; `cargo test` < 2 s.

## Outcome (2026-10-04)

- `crates/saturnus/src/cpu/`: `regs` (register file, RSTK), `alu` (field
  arithmetic), `instr` + `decode` (full opcode table, undefined encodings
  become `Instruction::Invalid`), `disasm` (SASM syntax), `exec` (`Cpu::step`,
  interrupt entry, RTI), `bus` (`Bus` trait, `FlatMemory`), `cycles`
  (approximate counts from the SASM manual).
- Oracle: with `SATURNUS_LITERATURE_DIR` set, the decode/disassemble
  round-trip verifies 391 of the 485 records in HP's `SASM.OPC`; the other
  94 are assembler directives, pseudo-ops and macros. 69 tests, well under
  the 2 s budget.
- The A=IN / C=IN even-address quirk is deliberately not modelled (failure
  mode undocumented); the constant-overrun model and the other source
  disagreements are in the decision log and in the wiki page
  `hardware/saturn-cpu` ("Facts settled while building saturnus").
