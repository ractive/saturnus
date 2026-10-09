---
type: iteration
title: "Iteration 12b: Explorer writes (store, fetch, purge, rename, flags)"
date: 2026-10-05
status: completed
tags:
  - iteration
  - saturnus
branch: iter-12b/explorer-writes
---

# Iteration 12b: Explorer writes (store, fetch, purge, rename, flags)

Split from [[iterations/iteration-12-memory-explorer]] on 2026-10-05: the
read-only layer could start at once; everything that changes calculator
memory waits here.

**Unblocked 2026-10-08**: `kermit-proto` is on crates.io (0.1.1), and
`crates/saturnus-kermit` already used it. The rule stays (decision-log
entry "control API replaces MCP"): no `hptx-core` in the page or in any
saturnus host. The hidden Kermit transaction is built on `kermit-proto`
plus the HP file header and text encoding in `saturnus-objects`;
keystrokes are the fallback where no Kermit server exists. Direct RAM
writes stay out, so the ROM's memory manager stays consistent.

## Tasks

- [x] Hidden Kermit transaction at unlimited speed in the shared runner
  and the Worker host: enter server mode, do the transfer, leave, restore
  the screen; a busy overlay in the page while it runs.
- [x] Explorer writes: store a file from the PC side, fetch a variable to
  a file, purge, rename, change directory.
- [x] Flags panel: toggling a flag (`SF`/`CF`) and re-reading it.
- [-] Desktop app: native drag and drop between the PC file pane and the
  calculator pane.
  Not done (2026-10-09): there is no PC file pane, and dragging a variable
  out of the webview needs a native drag source. Dropping files from
  Finder onto the memory view works; the owner checked it.
- [x] The control API gains the same write commands (one protocol).

## Acceptance criteria

- [x] Toggling the clock-display flag from the panel shows the clock on
  the LCD.
- [x] Dropping a file onto a directory stores it, and fetching a variable
  saves it as a file, each in under a second of wall-clock time at
  unlimited speed, with the calculator screen back afterwards.

## Outcome

Built 2026-10-08 on `iter-12b/explorer-writes`. Browser evidence (not
committed) is in the session's scratch directory `/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/iter12b/`:
`writes.mjs` (headless Chrome over the DevTools protocol, real mouse
events, a CDP file drop), `{48sx,48gx,49g}-report.json`, screenshots
`*-0{1..6}-*.png`, the fetched files in `dl-*`.

**What was built.**

- `saturnus-host::transfer` (new, wasm-clean; `kermit-proto` 0.1.1 is
  now a dependency of the published `saturnus-host`, with `web-time` on
  wasm32): a `Transfer` is planned from RAM reads (the tree, the path,
  the flags, the command line), then stepped in emulated time like a
  send: `SERVER` typed with the typing engine, the first NAK awaited,
  the Kermit exchanges (the link of `saturnus-kermit` rewritten as a
  steppable state machine on a plain `Machine`), `G F`, a wait until
  the screen is stable. Every transaction starts with an empty host
  command whose reply counts the stack; a failed command's leftovers are
  dropped (`n DROPN`), flag -35 is set per transfer and put back, a
  write in another directory changes back. A server that stops answering
  is ended with ON. `parse_stack` moved from `saturnus-kermit` to
  `saturnus-objects::transfer` (re-exported there).
- The engine: six commands (`storeFile`, `fetchFile`, `purge`,
  `rename`, `changeDir`, `setFlag`) run as a send in turns with
  `busy` raised and the frames held; the same refusals; `releaseAll`
  stops one. `web/protocol.md` has the section "The user memory,
  written".
- Native runner: `storeFile` reads the file the host chose (named after
  it), `fetchFile` writes the fetched bytes into the chosen file and
  replies `file`. Tauri: an open and a save dialog; the window's
  `dragDropEnabled` is off so files dropped on the page reach it as
  HTML5 drops (sent as base64 `data`). HTTP: `POST /v1/memory` (body cap
  687 KiB), `saturnus ctl store|fetch|purge|rename|cd|flag`.
- Page: `web/writes.js` (one write at a time, the store's `writing` and
  `writeMessage`); the explorer has "Store file…", drops on a directory
  of the tree or the list or on the pane, "Save as file", "Rename"
  (inline field), "Purge" (inline question; `PGDIR` for a directory),
  "Make current" and "Change to this one"; the flags panel's lamps and
  cells are buttons; a busy overlay over the layer while a write runs.

**Tests.** `saturnus-host` unit tests (transfer planning, DROPN after a
failed command, padding trimmed, refusals; engine refusals of the writes);
`crates/saturnus-host/tests/transfer.rs` (ROM-gated, all three models: a
text file stored into a directory, fetched in binary, stored again and
fetched back byte for byte, rename, purge, cd, flags 5 and -40 with a
transfer while the clock shows, a refused command line; stack, path,
flags and LCD compared); `crates/saturnus-cli/tests/e2e.rs`
`writes_through_the_control_api_on_three_models` (the same through
`saturnus ctl`, wall time under 1 s asserted in a release build);
`crates/saturnus-tauri/tests/runner.rs`
`writes_store_and_fetch_files_the_host_chose` (files from and to the
host, no frame while busy, the screen back); server tests for
`/v1/memory`; `web/test/writes.test.mjs` and the Worker's `data` bytes.

**Times** (release, native; per write, wall): store 113-130 ms, fetch
110-168 ms, others 87-115 ms, through `ctl` including its process
119-168 ms; 11 to 20 s of emulated time each. In Chrome (wasm Worker),
click to message: 160-290 ms on the 48SX, 48GX and 49G. A debug build
takes about 2 s per write.

**Found on the way.** The page took any file dropped on it as a ROM;
drops on the memory view now stop there. The server creates `IOPAR` in
HOME (purged, it comes back with `G F`), so it stays, as on a real
calculator. The 48SX with flag -40 set took transfers without trouble
(the iteration 9 note about `start_server` does not apply to this path).

**Not done or not verified.**

- Desktop drag and drop "between the PC file pane and the calculator
  pane" (unticked): there is no PC file pane, and dragging a variable out
  of the webview to the desktop needs a native drag source (a Tauri
  plugin, a new dependency for `deny.toml`). Dropping files from the
  desktop onto the memory view works through the webview's own drop
  (`dragDropEnabled: false`), but was not driven in the app's real
  window; the owner should try it.
- `saturnus-kermit` keeps its own blocking link; moving it onto
  `saturnus-host::transfer` would leave one Kermit driver.
- A name that is a command (`SIN`) passes the plain-name check and fails
  on the calculator ("Invalid Syntax"), reported as such.
- Safari, Firefox and the app's WKWebView were not driven.
