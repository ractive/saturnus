---
title: "Iteration 4: UART, timers accurate enough for Kermit"
type: iteration
date: 2026-10-04
status: planned
branch: iter-4/uart-timers-accurate-enough-for-kermit
tags:
  - iteration
  - saturnus
---

# Iteration 4: UART, timers accurate enough for Kermit

Read first: wiki `hardware/uart`, `protocols/kermit-hp`.

## Tasks

- [ ] UART registers, 255-byte receive buffer, baud from IOPAR, TX/RX
  interrupts, the 11.375-bit-times-per-byte timing, overrun on inter-byte
  gaps, LPB loop-back.
- [ ] Serial API on `Machine`; `saturnus-cli --serial tcp:PORT` bridging like the
  container does, so hptx's e2e suite runs unchanged.

- [ ] Carried over from iteration 2: route the UART TX/RX interrupt sources
  through `Machine::poll_interrupts` (iteration 2 routes only timers, ON and
  the keyboard scan), and replace the instant-transmit stub in
  `io/registers.rs`.
- [ ] Carried over from iteration 2: TIMER1/2 run off the approximate SASM
  cycle counts, so emulated time drifts against real hardware. Measure the
  drift and fix it enough for Kermit timing.
- [ ] Carried over from iteration 3: once Kermit works, send a machine-code
  probe that reads #80000, #C0000 and #D0000 (empty CE1/CE2/NCE3) and, with
  two RAM cards configured to overlap, tells CE1 from CE2; run it on
  saturnng and on saturnus, and answer the wiki questions
  `bus-priority-ce1-ce2` and the open-bus value (now 0).

## Acceptance criteria

`hptx ls/get/put/run` pass against saturnus over TCP and
  in-process (hptx gets a `saturnus` transport behind a feature flag).
