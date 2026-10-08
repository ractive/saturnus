---
type: iteration
title: "Iteration 29: Create a folder in the Variables view"
date: 2026-10-09
status: in-progress
tags:
  - iteration
  - saturnus
  - explorer
branch: iter-29/create-folder
---

# Iteration 29: Create a folder in the Variables view

Owner (2026-10-09): "A 'create folder' button in the 'Variables' view
would be great."

The Variables view already stores files, renames, purges and changes
directory ([[iterations/iteration-12b-explorer-writes]]) through the
engine's hidden Kermit transaction. Creating a directory is one more
write of the same kind.

## Design

- **Protocol**: `createDir {dir, name}` (`dir` defaults to the current
  directory, as for `purge` and `rename`); reply `{emulatedMs, keys}`.
  It is one of the `WRITE_COMMANDS`, so the Worker, the Tauri runner and
  `POST /v1/memory` serve it with no code of their own; the CLI gains
  `saturnus ctl mkdir NAME [--dir HOME/A]`.
- **On the calculator**: the transaction changes into `dir` when it is not
  current, sends the host command `'NAME' CRDIR`, and changes back, as
  `purge` does. Before anything runs the engine checks the name with the
  same plain-name rule as `rename` (`check_name`), that `dir` exists, and
  that nothing in `dir` has that name already ("NAME already exists in
  { HOME A }"). A command's name (`SIN`, `SUB`) is a plain name, which
  the calculator refuses itself ("Invalid Syntax"), as for `rename`.
- **38G/39G/40G**: they have no memory view (and no Kermit server
  transaction), so the button never shows there; the engine refuses the
  command like every other write.
- **Page**: "New directory…" next to "Store file…" in the Variables bar
  (the page and the calculator say directory everywhere). It opens an
  inline name field (the rename `edit-row`: field, "Create", "Cancel";
  Enter and Escape) under the bar for the directory shown; a
  name already in that directory is refused in the page with a message
  before anything is sent. Disabled by `writesOff()` like the other write
  buttons. The memory view follows by itself (`memoryChanged`), and the
  new directory is selected in the list.

## Tasks

- [x] Plan (this file).
- [x] Engine: `Op::CreateDir`, the `createDir` command in the protocol
  and `WRITE_COMMANDS`, refusals (bad name, taken name, missing
  directory).
- [x] CLI: `saturnus ctl mkdir`, `createDir` in `POST /v1/memory`.
- [x] Page: `createDir` in `backend.js` and `writes.js`; "New directory…"
  with its inline field in `sat-explorer.js`.
- [x] Docs: `web/protocol.md` (command table, write list, control API),
  `web/README.md` if it lists the writes.
- [x] Tests: protocol unit tests (missing fields; planning reads RAM, so
  its refusals are tested on the ROMs), ROM-gated
  `crates/saturnus-host/tests/transfer.rs` on 48SX, 48GX and 49G (create
  in HOME and nested, appears in the tree, duplicate and bad names
  refused, the calculator left in its directory), the CLI e2e through
  `saturnus ctl mkdir`, `web/test/writes.test.mjs`.
- [x] Gates: `just gates`, `just rom-tests`, `hyalo lint`.
- [x] Owner follow-up: the object marker in the Variables list looked
  like a checkbox; now a page with a folded corner (`.kind-obj`, CSS).
- [x] Owner follow-up: a double-click on a directory row did nothing
  (the first click draws the list again, so the browser fires no
  `dblclick`); the click's count opens it now
  (`web/test/explorer.test.mjs`).
- [ ] Owner: create a folder in the desktop app and in the browser.

## Outcome

Built 2026-10-09 on `iter-29/create-folder`.

- **Engine**: `Op::CreateDir` in `saturnus-host::transfer`, planned like
  `purge` (enter `dir`, `'NAME' CRDIR`, change back); `createDir` is one
  of `WRITE_COMMANDS` (and of `REFUSED_WHILE_TYPING`), so the Worker, the
  Tauri runner and `POST /v1/memory` serve it unchanged. `saturnus ctl
  mkdir NAME [--dir HOME/A]`.
- **Page**: "New directory…" in the Variables bar; the inline field
  refuses an empty or taken name under itself, the engine the rest. The
  new directory is selected once the list shows it.
- **Verified on the ROMs** (`crates/saturnus-host/tests/transfer.rs`
  `directories_created_through_the_kermit_server`, 48SX, 48GX, 49G): in
  the current directory, nested from HOME, in HOME and one level deeper
  while another directory is current; the stack, the flags, the screen
  and the current directory unchanged; taken names (a directory and a
  variable), `1A`, `A B`, an empty name and a missing directory refused
  before anything runs. 94-130 ms wall per write natively (release),
  11-17 s emulated. The CLI e2e creates `HOME/D/E` through `ctl mkdir`
  and gets the duplicate refused.
- **Browser** (headless Chrome, scratch only, not committed): 48SX, 48GX,
  49G at 1280 px and the 48GX at 390 px: the field takes the focus, `X`
  is refused in the page ("X already exists in HOME."), `NEWD` is
  created by Enter (0.13-0.15 s click to message), listed, in the tree
  and selected; `INNER` inside it while the calculator stays in HOME;
  `1A` refused by the host; Escape closes the field; no horizontal
  overflow. The 38G shows "No memory view", so no button.

**Found on the way**: a name that is a built-in command passes the
plain-name check and the calculator answers "Invalid Syntax" (`SUB` hit
this during the browser run, as `SIN` does for rename); reported as the
calculator's error, nothing changed.

**Owner follow-ups** (same branch): the variables' marker was a hollow
grey rectangle, which read as a checkbox; it is a filled page with its
top right corner folded (CSS, the icon sprite has no file icon). A
double-click on a directory row did nothing: the first click selects
the row and draws the list again, so the second click lands on a new
`<tr>` and Chrome fires no `dblclick` (seen over CDP: no event). The
list's click handler opens a directory on the second click
(`e.detail === 2`), as Enter does; on another variable it selects it,
as Enter does. `web/test/explorer.test.mjs` (headless Chrome, real mouse
events, the memory reads answered by the test, no ROM) fails without
the fix.

**Review of PR 58** (three findings, fixed): the New directory field
took the focus back on every render (search typing, row clicks, a
memory refresh); it now takes it once when it opens and keeps it, with
the caret, only when it had it. Rename and Purge looked up their field
page-wide and found this one; they look in the preview, and the two
editors close each other. A name the host or the calculator refused was
lost with the field; the field stays read-only while the write runs and
comes back with the name and the reason. Three tests in
`web/test/explorer.test.mjs`, each failing on the code before. The
Rename field had the same refocus on every render; it now keeps the
focus only when it had it (a fourth test).

**Not verified**: the desktop app's real window (the Tauri runner serves
the command through the shared engine, but nobody clicked it there).
