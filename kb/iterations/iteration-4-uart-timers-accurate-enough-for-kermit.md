---
title: "Iteration 4: UART, timers accurate enough for Kermit"
type: iteration
date: 2026-10-04
status: completed
branch: iter-4/uart-timers-kermit
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

- [x] UART registers, 255-byte receive buffer, baud from IOPAR, TX/RX
  interrupts, the 11.375-bit-times-per-byte timing, overrun on inter-byte
  gaps, LPB loop-back.
- [x] Serial API on `Machine`; `saturnus-cli --serial tcp:PORT` bridging like the
  container does, so hptx's e2e suite runs unchanged.

- [x] Carried over from iteration 2: route the UART TX/RX interrupt sources
  through `Machine::poll_interrupts` (iteration 2 routes only timers, ON and
  the keyboard scan), and replace the instant-transmit stub in
  `io/registers.rs`.
- [x] Carried over from iteration 2: TIMER1/2 run off the approximate SASM
  cycle counts, so emulated time drifts against real hardware. Measure the
  drift and fix it enough for Kermit timing.
- [-] Carried over from iteration 3: once Kermit works, send a machine-code
  probe that reads #80000, #C0000 and #D0000 (empty CE1/CE2/NCE3) and, with
  two RAM cards configured to overlap, tells CE1 from CE2; run it on
  saturnng and on saturnus, and answer the wiki questions
  `bus-priority-ce1-ce2` and the open-bus value (now 0).

`[-]` = partly done: the probe ran on both emulators (empty CE1/CE2/NCE3
read 0 everywhere, emulator against emulator); the overlapping-card half
cannot be run (saturnng loads no 48SX port-2 card, saturnus would echo its
own rule), so `bus-priority-ce1-ce2` stays open until a hardware test.

## Acceptance criteria

- [x] hptx's e2e suite (ls, get, put, run, screenshot, backup) passes
  against saturnus over TCP (`saturnus run --serial tcp:PORT --autostart`)
  and in-process (hptx feature `saturnus`, branch `iter-8/saturnus-transport`
  in hptx, PR after this one merges).

## Outcome

### Core

- `io/uart.rs` models the UART as hardware: BAU #10D (baud code, UCK),
  IOC #110 (enables, SON), RCS #111 (RBF, RBZ, RER), TCS #112 (TBF, TBZ,
  LPB, BRK), #113 clear-error, RBR #114-#115, TBR #116-#117, USRQ in SRQ1
  (#118). One holding register and one shifter per direction; the ROM keeps
  the 255-byte buffer. Line time runs in sixteenths of a bit from emulated
  cycles at the #10D baud rate; a byte frame is 11.375 bit times both ways,
  RBF sets after the stop bit (10 bits), overrun and a looped-back break
  set RER. IR registers #11A-#11D are plain storage.
- UART interrupts are the rising edge of USRQ (rx start, rx full, tx
  empty per IOC), routed through `Machine::poll_interrupts`, not masked by
  INTOFF, and they wake the CPU from SHUTDN; SHUTDN time skips stop at the
  next UART event.
- Serial API: `Machine::serial_push`, `serial_drain`, `serial_pending`,
  plus `serial_baud`. Pushed bytes arrive one frame apart; bytes arriving
  while SON is clear or LPB is set are lost, as on a real line. Reset keeps
  the wire queues. The state format carries the whole UART and both queues
  (still version 1, never shipped).
- Timers reviewed against the wiki: TIMER1 runs only while TIMER2 runs and
  both expire on their MSB going set (already so); new: TIMER2 reads as
  all ones (#FFFFFFFF) while its interrupt is pending (an expiry the CPU has not
  vectored for yet; a TIMER2 write ends it).
- Clock drift, measured by `hp48sx_clock_drift` (e2e): with flag -40 set,
  ROM J's displayed clock advanced 600 s in 600.0007 s of emulated time,
  -0.7 ms (-1.1 ppm), below the test's 10 ms resolution. No cycle-count
  change was needed.
- Kermit: `hp48sx_kermit_server_receives_a_file` (e2e) starts ROM J's
  SERVER from the keyboard and sends I, S, F, D, Z, B packets through the
  serial API at 9600 baud; every packet is ACKed and the received variable
  evaluates to 42.
- Tests: 10 UART unit tests, 1 TIMER2-pending test, 6 machine tests
  (echo program round trip at line rate, loop-back, interrupt routing past
  INTOFF, SHUTDN wake on a start bit, reset keeps the wire, the echo
  program's disassembly), the synthetic-ROM state round trip now carries
  UART traffic; 2 new e2e tests.
- Open: the USRQ bit position in register #118 (bit 0 is a placeholder;
  ROM J never read it in the traced boot, key and Kermit paths); RCS bit 3;
  how the wire behaves in loop-back; whether a pending UART request
  re-enters after RTI like a held timer request.

### Bridge and hptx

- `saturnus run --serial tcp:PORT|tcp:HOST:PORT|stdio` bridges the serial
  port after `--load`, cards, `--cycles` and the key script; `--autostart`
  answers the boot prompt with NO (not with `--load`) and types ALPHA
  ALPHA S E R V E R ENTER (S = SIN, E = softkey E, R = right arrow,
  V = square root on the 48SX), then prints `serial bridged on
  tcp:HOST:PORT`. One TCP client at a time, reconnects allowed, Nagle
  off. `--exit-on-disconnect`, `--serial-log FILE` (wire trace with
  emulated milliseconds); SIGINT/SIGTERM end the bridge and `--screen`,
  `--save` and card write-back still run.
- Backpressure (PR review): at most 2 KiB queued towards the UART, one
  bounded read per loop pass, no reads above that mark; bounded stdin
  channel; a new client waits until the previous one's last bytes have
  drained; `Machine::serial_clear_inbound` empties the queue when the
  bridge stops. A client sending 64 KiB blocks for 6 s (818 KB accepted by
  the kernel) left the bridge at 7.6 MB RSS throughout, 2047 bytes queued,
  and emulated time advancing at wall-clock rate.
- Pacing: 2 MHz of emulated cycles per wall-clock second, slices of at
  most 1 ms between socket polls, sleep when ahead, re-anchor when more
  than 200 ms behind. A 20 s `TICKS` measurement over the bridge gave
  8192 ticks per wall second.
- hptx e2e over TCP (`HPTX_E2E_ADDR=tcp://localhost:4850`): all six
  scenarios pass, 91 s for the suite, with no retransmission in the wire
  log. Per scenario (one `cargo test` each, so including the 0.5 s drain
  and startup): ls 24.2 s, get 11.6 s, put round trip 12.0 s, run 16.5 s,
  screenshot 17.5 s, backup 12.9 s. The saturnng container, same
  harness: 16.8, 7.8, 8.4, 11.4, 11.4, 8.7 s.
- hptx in-process (hptx branch `iter-8/saturnus-transport`,
  `saturnus:///path/sxrom-j`, emulated time runs ahead of wall time): all
  six pass, 17.5 s for the suite; per scenario 5.0, 2.4, 2.5, 3.5, 2.5,
  2.6 s.
- Finding for the core (not hardware-verified): ROM code runs slowly in
  emulated time. `TICKS 1 2000 START NEXT TICKS SWAP -` takes 48880 ticks
  (5.97 s) on saturnus; saturnng gives 145 ticks, which is implausibly
  fast, so it is no oracle. The timer is right (above), so the excess is
  in instruction cycle counts or wait states. The calculator takes
  100-360 ms from receiving a packet to answering it, which is why the
  TCP suite is slower than against saturnng; once, an I-packet sent 590 ms
  after a previous session's final ACK was ignored until the 5.2 s server
  timeout (kermit-probe run back to back; hptx's 200 ms turnaround never
  hit this in the suite).
- Probe (carry-over from iteration 3, partly done, task left open): a
  hand-assembled Code object (hptx put, binary) copies 16 nibbles each
  from the addresses #80000, #C0000, #D0000 and, as a control, #00000 into a 32-character
  string with `D1=(5)`, `C=DAT1 W`, `DAT1=C W`. With empty slots, saturnng
  (`CARDS=0`), saturnus over TCP and saturnus in-process all return
  `0000000000000000` for the three windows and `2369B108DADF1008` (the ROM
  image) for #00000. Recorded in the wiki (`hardware/memory-controller`)
  as emulator against emulator. The CE1/CE2 overlap half was not run: the
  saturnng container cannot load a 48SX port-2 card, and saturnus would
  only report the priority it implements, so `bus-priority-ce1-ce2` needs
  hardware.
