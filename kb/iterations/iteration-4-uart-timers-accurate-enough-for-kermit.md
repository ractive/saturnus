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

## Acceptance criteria

`hptx ls/get/put/run` pass against saturnus over TCP and
  in-process (hptx gets a `saturnus` transport behind a feature flag).
