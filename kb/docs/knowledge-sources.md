---
title: Knowledge sources
type: docs
date: 2026-10-04
status: active
tags:
  - sources
  - saturnus
---

# Knowledge sources

- calculator-knowledgebase (<https://github.com/ractive/calculator-knowledgebase>,
  checked out at `~/devel/calculator-knowledgebase/`) is the LLM wiki. It is
  public: follow its CLAUDE.md (no private material, facts in our own
  words). Use `hyalo` from that directory.
  Hardware pages: `hardware/saturn-cpu`, `memory-controller`, `io-ram`,
  `interrupts`, `timers`, `uart`, `display`, `keyboard`, `crc`, `card-ports`,
  model pages `hp48sx`, `hp48gx`, `hp49g`. `questions/` lists open
  contradictions between sources; resolve them by experiment and answer them
  there. The opcode table is in `raw/saturn-hardware/` (HP's SASM.OPC and the
  Fernandes/Rechlin tutorial). Cite pages in code comments as
  `wiki: hardware/timers`.
- `~/devel/hptx/emulator/` runs saturnng in Docker with the serial port on TCP
  4848 and `calc-keys` / `calc-screen` to press keys and dump the LCD. It is
  the differential oracle: same ROM, same keys, same screen expected.
- `~/devel/hptx/` is the Kermit/XModem client that will talk to this emulator
  in-process and over TCP once the UART works.
- `~/devel/hp-emulator-refs/` holds GPL emulator sources, facts only (see the
  clean-room rule).
