---
title: "One Kermit host and one charset: saturnus-kermit onto satx-core"
type: backlog
date: 2026-10-10
status: planned
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
