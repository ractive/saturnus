---
type: iteration
title: "Iteration 68a: aplet folder, Add a calculator, kb notes"
date: 2026-11-08
status: planned
tags:
  - iteration
  - web
  - satx
branch: iter-68a/aplets-and-add-calculator
---

# Iteration 68a: aplet folder, Add a calculator, kb notes

The owner tested 0.10.3 on a real 48SX, 49G (ROM 1.18) and 38G on
2026-11-08 (Files in the desktop app, satx from Homebrew). Fast transfer
in Files works on the real 49G, and so do aplets both ways on the 38G.
These are the rough edges found along the way. Owner 2026-11-08: "do
everything, plan it and implement it". Files over XSERV is the next
iteration ([[iterations/iteration-69-files-over-xserv]]).

## Facts from the hardware session (2026-11-08)

- 49G ROM 1.18: right-shift → starts sometimes XSERV ("XModem server"),
  sometimes the Kermit server; it seemed to depend on the server used
  before, but not consistently. Typing `SERVER` always starts Kermit.
- EVAL on the 49G is right-shift SYMB (the owner's calculator; the 49G has
  no EVAL key of its own). The 48GX has an EVAL key.
- A program sent to the Kermit server as an object (`« 'EQ' XSEND »`)
  lands on the stack unevaluated; after FINISH, EVAL (right-shift SYMB)
  ran it and the XModem download arrived complete (real 49G).
- The 38G serve screen reads the folder once when serving starts
  (`web/tx/serve.js` `starting()`): a file put in the folder afterwards was
  not offered until serving restarted.
- After "disconnect" in Files, the desktop app (pid of saturnus-app) still
  had the USB serial port open twice (`lsof`); a little later it was free.
  The 38G serve screen held it until Stop.

Split from the session's plan: 68a has what does not touch the 49G traces that PR #259 re-records; [[iterations/iteration-68b-fast-transfer-keys]] has the rest.

## Tasks

- [ ] **Aplet folder read at each listing.** When the 38G/39G/40G asks for
  its directory file (the start of every RECV), the server asks the host
  for the folder's files and answers once they come, the way it already
  waits for a file's content (`ApletEvent::NeedFile` → `provide`). In
  `crates/saturnus-tx/src/aplets.rs` (`on_request`), the wasm wrapper
  (`crates/saturnus-web/src/tx/aplets.rs`), the worker/driver
  (`web/tx/aplet-worker.js`, `aplet-driver.js`) and `web/tx/serve.js`.
  Tests: Rust unit test (a file added between two listings is offered),
  the serve page test.
- [ ] **"Add a calculator" asks which kind.** "Serve a folder to a 38G, 39G
  or 40G" is a small link under two big buttons and easy to miss (owner).
  "Add a calculator" first asks: an HP 48 or 49 (today's flow), or an HP
  38G, 39G or 40G (the serve flow). Remove the separate link. Keep the
  emulated-calculator button as it is. Page tests updated.
- [ ] **kb and wiki.**
  - `kb/backlog/fast-transfer-e2e-fails.md`: fast transfer works on a real
    49G in Files and with satx (2026-11-08); only the emulated "Type it for
    me" path fails.
  - `kb/backlog/satx-real-hardware.md`: XSERV verified on a real 49G ROM
    1.18 (mem, ls, get, put, eval), 4 KB in 8.4 s against Kermit's 16.7 s.
  - `crates/satx-core/src/xserv.rs` module doc and `satx xserv --help`:
    no longer "unverified"/"not yet tested on a real calculator".
  - Reference: the 49G's EVAL key is right-shift SYMB (`saturnus ref EVAL`
    lists no 49G key today).
  - New backlog item `kb/backlog/49g-rom-1-18-image.md`: an emulator image
    from HP's `hp49g118.zip` `.flash` update (container: `HPROM`, 7 parts
    System, Part1-4, 6, 7; per part a length and a field with the bank in
    the top nibble and the nibble count; the parts' data does not start at
    the bank's offset 0; bank headers, boot code in bytes 0-#FF and CRCs
    as the wiki's custom-ROM page says); the zip is in `roms/`.

## Acceptance

- Each task's tests pass; `just gates` and `node scripts/check-links.mjs`;
  `hyalo lint` clean.
- The fast-transfer dialog and satx name the right key per model, checked
  on the emulated 48GX and 49G: that is 68b's work, copied here when the
  plan was split; nothing in 68a names a fast-transfer key. 68a's part is
  the reference: `saturnus ref EVAL` and Search name right-shift SYMB on
  the 49G.
