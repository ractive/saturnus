---
type: iteration
title: "Iteration 38: saturnus-tx for the 38G, 39G and 40G (aplets, XModem), 49G extras"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus-tx
  - kermit
  - xmodem
  - 38g
  - 39g
branch: iter-38/tx-aplets
---

# Iteration 38: aplets for the 38G, 39G and 40G

On these calculators the calculator drives: SEND and RECEIVE to "a disk
drive (or a computer)" make it a Kermit client that first asks for the
directory file `HP38DIR.CUR` or `HP39DIR.CUR` (wiki
`questions/hp38g-39g-transfer-protocol`). saturnus-tx serves a folder as
that disk drive ([[docs/saturnus-tx]], "Aplets"). Binary aplets go to the
39G and 40G by XModem. For the 49G: XModem as the fast path, and XSERV
only if it proves itself.

## Before it starts

- **The directory file's format** is in the wiki, found on the emulated
  38G and 39G (saturnus's serial commands and a minimal server, as on
  2026-10-05), from the manuals, and from HPGComm only if the owner
  allows a facts-only reading of it ([[docs/saturnus-tx]], open
  question 4). No code before the wiki page.
- **A Kermit server side** in hptx (`kermit-proto` answering I, R, S, F,
  D, Z, B with the calculator as client), or in `saturnus-tx` if hptx
  will not take it; the owner decides where.
- hptx-core's `Model` knows the 38G, 39G and 40G, or saturnus-tx keeps
  its own model for them.

## Tasks

- [ ] Research: the directory file and an aplet's S/F/D packets on the emulated 38G and 39G, written to the wiki (`questions/hp38g-39g-transfer-protocol` answered, a new `protocols/aplet-transfer` page)
- [ ] Kermit server side (hptx or saturnus-tx, as decided) with unit tests over traces recorded from the emulated 38G and 39G
- [ ] saturnus-tx: "serve a folder": the directory file from the folder's aplets, receive (calculator SEND) into the folder, send (calculator RECEIVE) of the chosen aplets, model from the directory file asked for; XModem send of a binary aplet to the 39G/40G (RECV, "HP39/40 (Wire)"); "Transfer Failed" explained
- [ ] Web: "Serve a folder to a 38G, 39G or 40G": folder, port, the waiting screen with what to press on the calculator, the transfers as they happen, Stop; the emulated 38G and 39G as devices
- [ ] 49G and 48GX: "Fast transfer (XModem)" for get and put: tx ends the server, shows what to type (`'NAME' XRECV`, `XSEND`), waits, and offers to restart `SERVER` after (hptx's XModem session; 1k blocks on the 49G, checksum only on the 48GX)
- [ ] XSERV spike, at most a day: start XSERV on the owner's 49G and on the emulated 49G, try `V` and `L`; write the result to the wiki `protocols/xserv`; go or no-go for an XSERV device, decided by the owner
- [ ] Tests: unit tests over the new traces; `just tx-e2e DIR` with the emulated 38G and 39G: send an aplet to the folder, receive it back byte for byte; XModem put and get against the emulated 48GX and 49G
- [ ] Docs: [[docs/saturnus-tx]] (aplets as built), CHANGELOG
- [ ] Owner: on the 38G (its own cable): send an aplet to a folder, receive it back, receive an aplet made on the emulated 38G; XModem put and get on the 49G; the XSERV go or no-go

## Acceptance

- An aplet round trip calculator, folder, calculator works on the
  emulated 38G and 39G and on the owner's 38G.
- The 39G and 40G binary-aplet path is tested on the emulator; the
  page says it is untested on hardware until someone reports.
- XSERV is either working on the owner's 49G or recorded as not done,
  with the reason, in the decision log.
