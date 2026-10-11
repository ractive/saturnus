---
type: iteration
title: "Iteration 39: saturnus-tx for the 38G, 39G and 40G (aplets, XModem), XModem on the 48GX and 49G"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus-tx
  - kermit
  - xmodem
  - 38g
  - 39g
branch: iter-39/tx-aplets
---

# Iteration 39: aplets for the 38G, 39G and 40G

On these calculators the calculator drives: SEND and RECEIVE to "a disk
drive (or a computer)" make it a Kermit client that first asks for the
directory file `HP38DIR.CUR` or `HP39DIR.CUR` (wiki
`questions/hp38g-39g-transfer-protocol`). saturnus-tx serves a folder as
that disk drive ([[docs/saturnus-tx]], "Aplets"). Binary aplets go to the
39G and 40G by XModem. For the 48GX and 49G: XModem as the fast path.
No XSERV (owner, 2026-10-10: it belongs to the 49g+/50g).

## Before it starts

- **The directory file's format** is in the wiki, found on the emulated
  38G and 39G (saturnus's serial commands and a minimal server, as on
  2026-10-05), from the manuals, and from HPGComm's source, read for
  facts only (the owner's documented exception, [[docs/saturnus-tx]],
  "Clean room"). No code before the wiki page.
- **A Kermit server side**: `kermit-proto` answering I, R, S, F, D, Z
  and B with the calculator as client (its own module beside the
  client), used by `saturnus-tx`.
- `satx-core`'s `Model` knows the 38G, 39G and 40G, so `satx` gains them
  too.

## Tasks

- [x] Research: the directory file and an aplet's S/F/D packets on the emulated 38G and 39G, written to the wiki (`questions/hp38g-39g-transfer-protocol` answered, a new `protocols/aplet-transfer` page)
- [x] Kermit server side in `kermit-proto`, `Model` in `satx-core`, with unit tests over traces recorded from the emulated 38G and 39G
- [x] saturnus-tx: "serve a folder": the directory file from the folder's aplets, receive (calculator SEND) into the folder, send (calculator RECEIVE) of the chosen aplets, model from the directory file asked for; XModem send of a binary aplet to the 39G/40G (RECV, "HP39/40 (Wire)"); "Transfer Failed" explained
- [x] Web: "Serve a folder to a 38G, 39G or 40G": folder, port, the waiting screen with what to press on the calculator, the transfers as they happen, Stop; the emulated 38G and 39G as devices
- [x] 49G and 48GX: "Fast transfer (XModem)" for get and put: the page ends the server, shows what to type (`'NAME' XRECV`, `XSEND`), waits, and offers to restart `SERVER` after (`satx-core`'s XModem session; 1k blocks on the 49G, checksum only on the 48GX)
- [x] Tests: unit tests over the new traces; `just tx-e2e DIR` with the emulated 38G and 39G: send an aplet to the folder, receive it back byte for byte; XModem put and get against the emulated 48GX and 49G
- [x] Docs: [[docs/saturnus-tx]] (aplets as built), CHANGELOG
- [ ] Owner: on the 38G (the owner's 10-pin cable): send an aplet to a folder, receive it back, receive an aplet made on the emulated 38G; XModem put and get on the 49G

## Acceptance

- An aplet round trip calculator, folder, calculator works on the
  emulated 38G and 39G and on the owner's 38G.
- The 39G and 40G binary-aplet path is tested on the emulator; the
  page says it is untested on hardware until someone reports.
