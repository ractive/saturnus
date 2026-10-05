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

## Context from iterations 1-3 (2026-10-05)

- Existing code: `io/registers.rs` holds the HDW window with a UART stub
  (`UART_TX_STATUS` reads with the busy bits cleared, RX reads 0); `io/timers.rs`
  has TIMER1/2 with expiry on the MSB going set; `machine/mod.rs` routes
  timer, ON, keyboard-scan and card interrupts in `poll_interrupts` and
  skips time during SHUTDN; `Machine::cycles()` is emulated time at 2 MHz
  including the 13% refresh stall. `saturnus-cli` has `run` with key
  scripts, `--save/--load`, cards; `scripts/diff-vs-saturnng.sh` is the
  screen oracle (Docker at `~/.rd/bin`).
- The 255-byte receive buffer is the ROM's, not hardware: the UART has one
  holding register per direction plus a shifter (wiki: hardware/uart, "Line
  behaviour"). Model the hardware; the ROM does the buffering.
- Serial API contract (core, so the CLI and hptx can be written in
  parallel): `Machine::serial_push(&mut self, bytes: &[u8])` queues bytes
  arriving on the wire, delivered to the UART at line rate (11.375 bit
  times per byte at the IOPAR baud, #10D); `Machine::serial_drain(&mut
  self) -> Vec<u8>` returns the bytes the calculator transmitted since the
  last drain; `Machine::serial_pending(&self) -> usize` is the undelivered
  count. Transmission takes line time too (the ROM busy-waits on the
  transmitting bit with a baud-indexed delay, wiki: hardware/uart "ROM
  handler behaviour").
- Real time: Kermit timeouts on both sides are wall-clock, so `saturnus run
  --serial tcp:PORT` must pace the machine to wall-clock time (2 MHz of
  emulated cycles per second, sleeping when ahead, catching up when behind),
  not run flat out.
- Timer drift: measure it with the ROM's own clock. Set flag -40 (clock
  display), read the shown time from the screen, run N emulated minutes,
  read again; the ROM's TIMER2 bookkeeping compensates its own code with
  cycle-counted instructions, so any drift exposes approximate cycle counts.
- hptx (`~/devel/hptx`): `hptx_core::transport::Transport` is the trait
  (`write_packet`, `read` with timeout); `open("tcp://host:port")` already
  exists; the e2e suite is `crates/hptx-core/tests/e2e.rs`, gated on
  `HPTX_E2E_ADDR`, run with `just e2e`; it expects the calculator already
  in Kermit server mode ("Awaiting Server Cmd."), which the saturnng
  container reaches by answering NO and typing ALPHA ALPHA S E R V E R
  ENTER (`~/devel/hptx/emulator/entrypoint.sh`). hptx has its own kb,
  iteration plans and PR discipline (CLAUDE.md there); the in-process
  transport is an hptx iteration with its own branch and PR.

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
