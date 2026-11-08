---
type: iteration
title: "Iteration 68b: fast transfer by one key, server hint, serial port"
date: 2026-11-08
status: completed
tags:
  - iteration
  - web
  - satx
branch: iter-68b/fast-transfer-keys
---

# Iteration 68b: fast transfer by one key, server hint, serial port

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

After PR #259 (it re-records the 49G traces this changes) and alongside [[iterations/iteration-68a-aplet-folder-and-add-calculator]].

## Tasks

- [x] **Fast transfer without typing the command.** Before ending server
  mode, Files sends `« 'NAME' XSEND »` (download) or `« 'NAME' XRECV »`
  (upload) as an object, so it lands on the stack, then ends server mode;
  the dialog says to press EVAL on a 48GX and right-shift SYMB on a 49G
  instead of typing the command. The emulated calculator's "Type it for
  me" presses the same keys. satx's `--protocol xmodem` does the same and
  prints the key instead of the command. Traces re-recorded.
- [x] **Server restart hint.** After a fast transfer the dialog says to type
  `SERVER` on a 49G (right-shift → may start XSERV); on a 48GX check what
  right-shift → does on the emulator and keep the shortcut only where it
  is reliable.
- [x] **Serial port released on disconnect.** Find why the desktop app kept
  `/dev/cu.usbserial-*` open after Files disconnected (two descriptors,
  `crates/saturnus-tauri/src/transfer.rs` `serial_close`/`close_all`) and
  fix it; a test where the platform allows.

## Acceptance

- Each task's tests pass; `just gates` and `node scripts/check-links.mjs`;
  `hyalo lint` clean.
- The fast-transfer dialog and satx name the right key per model, checked
  on the emulated 48GX and 49G.
