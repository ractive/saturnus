---
title: "One Kermit host and one charset: saturnus-kermit onto satx-core"
type: backlog
date: 2026-10-10
status: completed
priority: low
tags:
  - backlog
  - satx
  - saturnus
---

# One Kermit host and one charset: saturnus-kermit onto satx-core

Since satx joined this repository ([[iterations/iteration-36a-import]]),
two pieces exist twice.

- **The Kermit host.** `saturnus-kermit` drives an emulated calculator's
  serial port in process, on emulated time only, and talks to the ROM's
  Kermit server through `kermit-proto`; `satx-core`'s `Session` and
  `Calculator` do the same over a `Transport`, with the in-process saturnus
  transport on wall-clock time. Once `satx-core` has a transport on
  emulated time (deterministic, as the ROM-gated tests and
  `saturnus-refgen` need), `saturnus-kermit`'s `kermit`, `reply` and
  `semantic` modules can become calls into `satx-core`, and the crate keeps
  only the emulator side, or goes.
- **The character set.** `saturnus-objects`' `charset` module is the same
  table as `satx-core`'s `charset` (its doc comment says so). One of them
  should use the other; which way round depends on the publishing order
  (`satx-core` depends on saturnus crates, not the reverse), so the table
  probably belongs in `saturnus-objects` or a protocol crate.

## Outcome

Done on 2026-10-12 (branch `refactor/fold-kermit-duplicates`).

- **The Kermit host.** No new transport was needed: satx-core's sans-I/O
  `Machine` takes the caller's clock, so `saturnus-kermit`'s `kermit`
  module now drives it over its own `Link` on the link's emulated idle
  clock (deterministic as before). Host commands, `G D`, the path, `cd`,
  GET, SEND and `G F` are satx-core's operations; the module keeps only
  the host loop and the name check. Its `reply` module is gone (satx-core's
  listing parser, which also reads the 49G's wrapped header). The link
  stays (emulated time, busy time not counted, the turnaround, the read
  cap), and so do the emulator side and `semantic`, which is
  saturnus-specific (typed objects through `saturnus-objects`).
  satx-core's real-calculator habits had to be switched off for an
  emulator (`Machine::set_emulated_link`, new): it reads and drops input
  before a host command, which on an emulator runs the calculator for a
  millisecond and left the 49G silent after `saturnus-refgen`'s `DATE`
  example; and it sends a `C` only once, where on the idle clock a resend
  is safe (saturnus-kermit always resent). With the switch the ROM-gated
  suites pass and every catalog and examples file regenerates byte for
  byte, `TICKS` included. The read cap now arms only after the command is
  written.
- **The character set.** One table, in `saturnus-objects` (published
  before `satx-core`, wasm-clean); satx-core depends on it and its
  `charset` wraps it with its own error type. The stack-reply parser
  (`StackReply`, `parse_stack`) and the name check (`is_plain_name`, plus
  satx's Windows device names) were duplicated the same way and are
  `saturnus-objects`' too. No public API changed or broke; satx-core
  gains `Machine::set_emulated_link`, saturnus-objects' `charset` gains
  `named_chars`, `to_trigraphs` and a public `trigraph_at`.
- Left: `saturnus-host`'s hidden Kermit transaction (`transfer.rs`) is a
  third Kermit client on the emulated port, a steppable one inside the
  engine's turns, and satx-core's `object` module overlaps
  `saturnus-objects`' prolog walk; neither was in this item.
