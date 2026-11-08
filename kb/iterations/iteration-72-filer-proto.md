---
type: iteration
title: "Iteration 72: filer-proto, a test server and satx lx for the HP 100LX/200LX"
date: 2026-11-08
status: planned
tags:
  - iteration
  - satx
  - hp100lx
branch: iter-72/filer-proto
---

# Iteration 72: filer-proto, a test server and satx lx

The owner has an HP 100LX with its original serial cable and wants Files to
reach it (2026-11-08). Its built-in Filer has a server mode whose protocol
is now known well enough to implement: [[research/hp100lx-filer]] (published
sources) and [[research/hp100lx-filer-hp-client]] (facts from HP's
Connectivity Pack), and the wiki's `protocols/filer-hplx` and
`hardware/hp100lx-200lx` pages. saturnus implements from those pages only
(clean-room: no source of LXTools, WFX-HPLX, BSD Filer or j200lx is
opened, no HP code is consulted; the facts there are in our own words).
Files comes next, in [[iterations/iteration-73-files-hp100lx]].

## Tasks

- [ ] **`crates/filer-proto`**, sans-IO and wasm like `kermit-proto` and
  `xmodem-proto`: the framing (three SYNs, DLE STX with the protocol
  revision byte, SOH, command, counter, direction, data with DLE doubling,
  DLE ETX, CRC-16/ARC low byte first and not DLE-doubled), a deframer that
  resynchronises, the request and status words, the commands (connect,
  disconnect, drive list, change drive and directory, listing, open, read,
  write, close, rename, delete, mkdir, rmdir, date and time, attributes),
  the error codes with their meanings, and a client state machine with HP's
  timing (2 s for CONNECT, 5.9 s otherwise, three sends of the same frame,
  a repeated counter answered with the previous reply, CONNECT up to six
  times). A server state machine as well, for tests. Unit tests from the
  wiki's example frames (their CRCs `10 BF` and `2D 7F`).
- [ ] **A test server (oracle):** BSD Filer v0.3 (a 1996 server emulator,
  freeware for non-commercial use) in a Docker image under `tools/`, its
  serial side on TCP through socat on a new port (4853, beside the
  saturnng oracle's 4848-4852), run as a black box like saturnng; it
  serves a test folder. Plus our own `filer-proto` server as a second,
  in-process peer for the cases BSD Filer lacks.
- [ ] **`satx lx`** (or a separate command if that reads better): `ls`,
  `get`, `put`, `rm`, `mkdir`, `rmdir`, `mv`, `drives`, over a serial port
  or `tcp://`; the speed set by the user (19200 by default, the FAQ's safe
  limit) and the palmtop's settings in the help.
- [ ] **e2e** against the oracle in Docker (a new CI job behind the `e2e`
  label, as the satx oracle) and the in-process server.
- [ ] Docs: `kb/satx`, the satx README, CHANGELOG.

## Acceptance

- Listing, get, put, rename, delete, mkdir and rmdir work byte for byte
  against BSD Filer and the in-process server; errors map to their
  meanings; a lost reply is resent with the same counter.
- `filer-proto` builds for `wasm32`. `just gates`, `hyalo lint` clean.
