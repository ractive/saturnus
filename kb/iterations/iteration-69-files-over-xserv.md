---
type: iteration
title: "Iteration 69: Files over XSERV on the 49G"
date: 2026-11-08
status: completed
tags:
  - iteration
  - web
  - satx
branch: iter-69/files-over-xserv
---

# Iteration 69: Files over XSERV on the 49G

Files talks to every cable calculator through its Kermit server today. On
the 49G, XSERV (the XModem server HP's Conn4x used) is faster and does
not depend on the display: on a real 49G (ROM 1.18, 2026-11-08) a 4 KB
string took 8.4 s over XSERV and 16.7 s over Kermit, and the listing came
whole while Kermit's stack replies are cut at 20 characters. Owner
2026-11-08: Files should use XSERV; "do everything, plan it and implement
it". After [[iterations/iteration-68a-aplet-folder-and-add-calculator]] and [[iterations/iteration-68b-fast-transfer-keys]].

## Facts from the real 49G (ROM 1.18, 2026-11-08)

- `satx xserv` mem, ls, get, put and eval all worked; get and put were byte
  for byte (47 bytes and 4013 bytes). The first `ls` right after an `eval`
  once failed and worked on retry (the calculator was probably still
  running the PURGE); with a 2 s pause it worked.
- XSERV's listing sizes differ from the Kermit server's (IOPAR: XSERV 20,
  Kermit 29.5).
- A lone `M` byte sent to the Kermit server got no reply, and the Kermit
  server worked normally afterwards: probing for XSERV with `M` first is
  safe. The reverse (a Kermit packet to XSERV) was not tried.
- XSERV started from the Kermit server (`satx run XSERV`) fails with "Port
  Not Available": Files cannot switch a calculator from Kermit to XSERV.
- `SERVER` run through XSERV's E command starts the Kermit server, and
  Kermit's FINISH returns to XSERV: the Kermit server runs nested inside
  XSERV. Two round trips XSERV → Kermit → XSERV worked, the stack was
  untouched.
- XSERV cannot say the current directory (`satx xserv` goes back to HOME).
- Right-shift → starts XSERV or Kermit unpredictably; typing `XSERV` is
  reliable.
- The emulated 49G: `xserv.rs` says ROM 2.15 has no XSERV, but the 49G has
  XSERV since ROM 1.10 and an attempt with right-shift → on the emulator
  was inconclusive (the key may not have started it). Settle this first:
  type `XSERV` on the emulated 49G (2.15 and 2.10) and ask with `satx
  xserv mem` over `saturnus run --serial tcp:PORT`.

## Design

- **Detection.** On connect to a cable calculator, Files sends `M` and
  waits about 1.5 s for an XSERV reply frame; a reply means an XSERV
  session, silence means the Kermit server as today. Check on the emulated
  48SX and 48GX that their Kermit servers ignore the stray `M` as the 49G
  did. One shared function in satx-core.
- **XSERV session.** Listing `L`, download `G`, upload `P`, free memory `M`.
  Changes (purge, rename, new directory) and directory changes go as RPL
  through `E`, each followed by a fresh listing that confirms it, with a
  pause or retry for a calculator still busy. Files keeps the current path
  itself (XSERV cannot report it) and sets it with `E` (`HOME DIR …`).
  Every transfer is XModem, so the "Fast transfer (XModem)…" items go
  away in an XSERV session.
- **Kermit when needed.** For what XSERV cannot do (PICT, backup, restore,
  running RPL with the stack back, previews that need the Kermit server),
  Files sends `SERVER` through `E`, does the work over Kermit, and returns
  with FINISH. Restore warm-starts the calculator: say so, and expect to
  reconnect.
- **The connect instructions** for a 49G say to type `XSERV` (or `SERVER`
  for Kermit). Connected over Kermit, Files offers a hint that `XSERV` is
  faster (press ON, type `XSERV`).
- **Sizes.** One meaning of a variable's size in Files whatever the server;
  say in the kb which.
- **satx.** The detection in satx-core; `satx info` says which server
  answered; `satx ls`, `get` and `put` work against XSERV too (or say
  plainly to use `satx xserv`); `satx xserv` docs drop "not yet tested".

## Tasks

- [x] Settle XSERV on the emulated 49G (above); record in the wiki and fix
  the `xserv.rs` claim either way.
- [x] Detection in satx-core (probe, timeout, tests with scripted replies;
  the 48SX and 48GX Kermit servers on the emulator).
- [x] XSERV session in `crates/saturnus-tx` (device ops over XSERV, the
  Kermit nesting, events and progress as for Kermit), wasm and desktop.
- [x] Files UI: connect flow, the hint, no fast-transfer items under XSERV,
  directory navigation, messages.
- [x] satx CLI as above.
- [x] Tests: unit tests with scripted XSERV replies; the e2e on the
  emulated 49G if it runs XSERV; page tests; traces.
- [x] kb: `kb/docs/saturnus-tx.md` (Files over XSERV), the satx docs, the
  wiki's XSERV page (hardware facts above).
- [-] Owner check on the real 49G: connect in XSERV, list, download,
  upload, rename, purge, a directory, PICT and a backup (through the
  Kermit nesting).

## Acceptance

- On the emulated 49G (if it has XSERV) and on the real one: Files connects
  to XSERV without being told, lists, transfers both ways byte for byte,
  changes and navigates, and reaches PICT and backup through Kermit and
  back.
- The 48SX and 48GX connect exactly as before.
- `just gates`, `hyalo lint` clean.

## As built

- satx-core: the XSERV client over the link (`XservSession`, the
  Machine's `Xserv*` operations), `memory_reply` for the probe, `L`
  records sized as the Kermit server's; satx probes `M` while it drains
  at connect (`satx info` names the server, Kermit commands stop with a
  hint), `satx xserv quit`.
- saturnus-tx: the search probes `M` at each speed first; `xserv.rs`
  runs list, get, put, changes (RPL through `E`, checked by listing),
  directory walks and the Kermit nesting; disconnect sends `Q`.
- Files: the 49G's connect step types `XSERV`; the status names XSERV,
  or on a 49G on the Kermit server says XSERV is faster; no fast
  transfer under XSERV; the disconnect note; the console names XSERV.
- Traces recorded again with the probe (the page's at a real 48GX's
  speed: at Max every pause got an idle NAK).
- Tests: satx-core's client against scripted replies, a scripted XSERV
  calculator in `saturnus-tx/tests/traces.rs`, the whole session on the
  emulated 49G in `saturnus-tx/tests/xserv.rs`, the page with the fake
  calculator's `server: "xserv"`, the console.
- The facts for the wiki's XSERV page went to the main session (the
  wiki is written there).
