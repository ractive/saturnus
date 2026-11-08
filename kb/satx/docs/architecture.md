---
title: Architecture and goal
type: docs
date: 2026-10-04
status: active
tags:
  - architecture
  - satx
---

# Architecture and goal

Status (2026-10-05): `kermit-proto` client implemented (iteration 2);
`satx-core` implemented (iteration 3); `xmodem-proto` and `satx` (crates/satx-cli) are
still skeletons.

## Goal

`satx` lets a person or an AI agent list, fetch, store, run and back up
objects on an HP48/49 (later 38/39/40) over a USB-serial adapter, from macOS,
Windows and Linux. A GUI follows once the CLI is solid.

## Architecture

```text
kermit-proto   sans-IO Kermit state machine (generic, publishable)
xmodem-proto   sans-IO XModem (+1k, CRC, HP variants)
filer-proto    sans-IO Filer protocol of the HP 100LX/200LX (client,
               job layer, test server over a FileSystem)
satx-core      HP layer: server commands, object format, IOPAR, GROB, XSERV,
               transports (serial via `serialport`, TCP, in-memory),
               the palmtop (`filer::Palmtop`)
satx           the CLI (crates/satx-cli), binary `satx`
```

Sans-IO seam, the same for the protocol crates (as built in `kermit-proto`):

```text
fn start(&mut self, now: Instant, command: Command) -> Result<(), Busy>;
fn handle_input(&mut self, now: Instant, bytes: &[u8]);
fn handle_timeout(&mut self, now: Instant);
fn poll_output(&mut self, now: Instant) -> Option<Vec<u8>>;  // one packet, write atomically
fn poll_event(&mut self) -> Option<Event>;
fn next_timeout(&self) -> Option<Instant>;
```

Every call that can queue output takes `now`, so the state machine owns
retransmit deadlines and the inter-packet pause without reading a clock.
Kermit events: `FileStart`, `Data`, `Progress`, `FileEnd`, `ServerText`,
`Error`, `Done`. Names, file data and server text are raw bytes in the
calculator's character set; `satx-core` translates.

`satx-core` layers, bottom up:

```text
transport   Transport trait: write one packet in one write, read with a
            timeout. Serial (9600 8N1, no flow control, DTR and RTS on),
            TCP (tcp://host:port), in-memory (closure or trace replay).
session     Session: drives one kermit-proto Client over a transport,
            drains stale input for 0.5 s on connect, maps errors.
charset     HP character set <-> UTF-8; ASCII trigraphs (\->, \<<).
reply       parsers for G D listings and C stack text (all three models).
object      HPHP48-x/HPHP49-x headers, prolog table, object length walk,
            %%HP: ASCII header.
grob        GROB decode/encode, PNG export.
calc        Calculator: list, cd, get/put (sets flag -35), run, mkdir,
            remove, rename, IOPAR, screenshot, backup, restore.
```

Since iteration 14 (decision log 2026-10-10) the session, calc and xmodem
logic is async code on a crate-private mailbox (`link`) that never reads a
clock or touches a port. `machine::Machine` drives it sans-I/O (fed bytes
and time; for a browser with Web Serial, wasm32 without default
features); `Session`, `Calculator` and `XmodemSession` drive it over a
`Transport` with the system clock. Serial and TCP need `native`
(default), the in-process emulator `saturnus` (opt-in; satx turns it
on).

Host commands (`C`) return the stack as display text and leave their
results on the user's stack, so every internal query drops what it pushed;
names are checked in a `G D` listing before they are evaluated (decision
log, iteration 3).

Files cross the boundary as events and chunks. The only link property the
protocol must know is whether the link is 8-bit clean (Kermit prefixing);
baud, parity and port names belong to the transport. Discarding stale input
for about 0.5 s after connecting is the transport's job.

## The HP 100LX/200LX palmtops (iteration 72)

`satx lx` talks to the palmtop's built-in Filer in server mode, not to a
calculator: a request/reply protocol with binary-synchronous framing,
CRC-16/ARC and DOS paths (wiki: `protocols/filer-hplx`,
`hardware/hp100lx-200lx`). `filer-proto` has three layers with the seam
above: `Client` (one request at a time; HP's timing: 2 s for CONNECT,
5.9 s for other replies, three sends of the same frame, CONNECT up to six
times), `Session` (jobs: list, get, put, rename, delete, mkdir, rmdir, the
drives, as HP's client sequences them) and `Server` over a `FileSystem`
(`MemoryFs`) for tests. It builds for wasm32 and is what Files will use in
iteration 73. `satx_core::filer::Palmtop` drives a `Session` over a
`Transport` (serial at `--baud`, 19200 by default, or TCP) on the system
clock.

Two choices beyond HP's client, both for servers that do not give the
codes HP's does: a put without `--overwrite` and a rename first look for
the target with a listing of its name (BSD Filer's 0x03 overwrites
without a 217 and its 0x08 says 0 for a file), and a get takes the file's
time from that listing rather than from 0x17.

Tests: unit tests in `filer-proto` (the wiki's example frames, replies
recorded from BSD Filer, client timing on a fake clock, client against
server); `satx-core/tests/e2e/lx.rs` (a module of the e2e binary) over TCP against the in-process server
always and against `FILER_E2E_ADDR` when set; `scripts/satx-lx-e2e.sh`
for the CLI. The oracle is BSD Filer v0.3 in Docker
(`tools/filer-oracle/`, port 4853, `just lx-oracle-up`, `just lx-e2e`);
what it lacks is in its README. CI runs both in the `lx-e2e` job of
`e2e.yml`, behind the `e2e` label.

## Milestones
