---
title: "Iteration 1: CPU core and disassembler"
type: iteration
date: 2026-10-04
status: planned
branch: iter-1/cpu-core-and-disassembler
tags:
  - iteration
  - saturnus
---

# Iteration 1: CPU core and disassembler

Read first: wiki `hardware/saturn-cpu`, `sources/saturn-tutorial`,
`sources/sasm-manual` (HP's own), `questions/dec-mode-constant-bug`.

## Tasks

- [ ] Registers A, B, C, D (64-bit, nibble fields P, WP, XS, X, S, M, B, W, A),
  R0-R4, D0, D1, PC, P, ST, HST (XM, SB, SR, MP from LSB), carry, DEC/HEX
  mode, 8-level RSTK.
- [ ] Decoder for the full opcode table; a disassembler in SASM syntax as a
  by-product (invaluable for debugging; HP's SASM.OPC table is the oracle).
- [ ] Quirks from the wiki: A=IN / C=IN only at even addresses; constant
  add/subtract field overrun (decide the DEC-mode question by experiment
  later, record it); pointer and P arithmetic always hex; SB on shifts.
- [ ] Tests: one test per instruction family with hand-computed results; a
  round-trip test decode -> disassemble over the whole opcode table.

## Acceptance criteria

every opcode in SASM.OPC decodes; `cargo test` < 2 s.
