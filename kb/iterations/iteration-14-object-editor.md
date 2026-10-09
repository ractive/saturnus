---
title: "Iteration 14: The palette's editor mode (command line, stack objects, variables)"
type: iteration
date: 2026-10-05
status: completed
branch: iter-14/object-editor
tags:
  - iteration
  - saturnus
---

# Iteration 14: The palette's editor mode (command line, stack objects, variables)

Rewritten 2026-10-05 after the owner's decision to fold the object editor
into the command palette: one editor, text in and out by key presses, no
Kermit. Needs iteration 19 (typing engine, `commandLine`, `replace`),
iteration 13 (the palette), iteration 12 (the explorer) and iteration 12c
(the decompiler, merged or in review).

Read first: [[iterations/iteration-19-command-input]],
[[iterations/iteration-13-command-reference]],
[[iterations/iteration-12-memory-explorer]],
`crates/saturnus-objects/src/decompile.rs`.

## Context

- Owner (2026-10-05, the original wish): "When we can show the file
  system of the calculator, we may want to make the files directly
  editable in a nice rich text editor, maybe even with syntax
  highlighting."
- Owner (2026-10-05, the palette discussion): "When editing something in
  the calculator, you may even 'sniff' it from memory, edit it in the app
  and then send it back to the calculator to replace what you are
  currently editing." Editing stored objects goes through the same
  editor: yes.
- Mechanism: the text comes from RAM (the command line through
  `commandLine`; a stack level or a variable through the 12c decompiler),
  is edited in the palette grown to an editor, and goes back as key
  presses: `replace` for a live command line; for a stored object the
  text followed by the keys that put it where it came from (a variable:
  the text, the quoted name, `STO`; a stack level: drop and re-enter, or
  the ROM's own `EDIT` session driven by keys, whichever proves robust on
  the ROM). The calculator compiles the text, so a syntax error is the
  calculator's own, shown in the editor, and the original object stays as
  it was. The screen is frozen with a busy mark while it types.
- What this cannot edit: objects without a text form (libraries, backup
  objects, graphics). A pixel editor for graphics and a grid for matrices
  were in the earlier plan; they are out of this iteration and need a
  write path that is not the command line (iteration 12b).
- The earlier plan's Kermit ASCII round trip is not needed for text
  objects. It remains the faster path for large programs once
  `kermit-proto` is available (iteration 12b); the editor's transport is
  behind one function so it can switch.

## Tasks

- [x] Editor mode of the palette: grows to a multi-line editor on
  Shift+Enter, on an unclosed delimiter, or when text is pulled in; RPL
  syntax highlighting (delimiters, numbers, strings, names, commands from
  the 13a data), bracket matching, indentation of nested `« »` and
  structure words, completion as in the palette, ASCII digraphs shown as
  the real characters (`<<` `>>` `->` and the transfer codes), a history
  of sent text. Framework-free unless a code-editor library is clearly
  worth its weight; if one is proposed, say what it costs in size and
  licence and vendor it (no CDN).
- [x] "Edit in the app" for a live command line: a control beside the
  calculator when `commandLine.active`; pulls the text and cursor, sends
  it back with `replace`.
- [x] Edit buttons in the explorer and the stack view for objects with a
  text form: pull the decompiled text, save back by keys; a dirty
  indicator; save on Cmd/Ctrl+S; the calculator's error surfaced on a
  failed compile, the original intact.
- [x] Tests on the 48SX, 48GX and 49G: a program in a variable pulled,
  changed, saved, and read back equal to the edited text; a no-op edit
  leaves the object's checksum unchanged; a syntax error surfaces and
  leaves the object intact; a live `EDIT` session replaced.

## Acceptance criteria

- [x] Opening a program from the explorer shows highlighted RPL; editing
  and saving it changes the variable on the calculator, and the explorer
  shows the new size and checksum within a second of the save finishing.
- [x] With a command line open on the calculator, pulling it into the
  editor, changing it and sending it back leaves the calculator in the
  same edit with the new text.

## Outcome

Built 2026-10-08 on `iter-14/object-editor`, after iteration 12b's hidden
Kermit transaction. Decisions in `kb/decision-log.md`, "2026-10-08
(iteration 14: palette editor mode)"; for users in `web/README.md`,
"Editor mode"; the protocol in `web/protocol.md`, "The palette's
editor". Browser evidence (not committed) in the session's scratch
directory `/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/iter14/`:
`editor.mjs` and `phone.mjs` (headless Chrome over the DevTools
protocol, real mouse and key events in the page, calculator keys through
`window.saturnus.backend`), `{48sx,48gx,49g}-report.json`, screenshots
`*-NN-*.png` and `phone-dark-editor.png`.

**Transport (changed from the plan).** A stored object does not go back
by keys but through the Kermit server: `storeText`, one hidden
transaction. The text, wrapped in `{ }`, is sent as a binary string
variable (`SATEDIT`), compiled by one host command (`RCL`, `PURGE`,
`STR→`, `DUP SIZE 2 MIN`), and the single object it gives is stored in
the variable or put in place of the stack level. The calculator's parse
error comes back as the result's `error`; nothing changes then. Why: a
save takes 0.15 to 0.2 s of wall time natively and 0.25 s in Chrome on
all three models whatever the length (3500 characters in 0.55 s), where
typing runs at 2.4 to 10 characters per emulated second, cannot type `;`
or the backslash on a 48SX, and depends on the entry mode. A live
command line still goes back with `replace`. The page's one transport
function is `saveEdit` in `web/editor.js`.

**What was built.**

- `saturnus-objects`: `Settings::for_editing`, `decompile::edit_text`
  (every digit of a real, binaries at 64 bits, tags as `:T:obj`; refuses
  an object without text and a string holding `"`), `UserMemory::
  edit_text_at`.
- `saturnus-host`: `transfer::Op::StoreText` with `Target` (variable or
  level), the `Compile` step, `Emulator::edit_text`; the engine's
  `editText` (a read) and `storeText` (a write, with the optional `was`
  check), both in `REFUSED_WHILE_TYPING`, `storeText` in
  `WRITE_COMMANDS`. HTTP: both on `POST /v1/memory`; `saturnus ctl text
  NAME|--level N [--set TEXT]`.
- Page: `web/editor.js` (tokens, highlighting, bracket matching, what is
  open, indentation and layout, digraphs, completion, history, sessions,
  `saveEdit`), `web/components/rpl-editor.js` (a textarea over a
  highlighted `<pre>`), the palette's editor mode in `sat-palette.js`
  (Shift+Enter, Enter with a delimiter open, a pasted line break, or
  `openEditor(target)`; Cmd/Ctrl+S, Cmd/Ctrl+Enter, Alt+↑/↓, Format;
  the dirty mark and a second Escape to discard), Edit buttons in the
  explorer's variable and stack previews, "Edit line" over the calculator
  and a pencil in the phone's bar while a command line is open (read from
  RAM 250 ms after the screen changed), and the palette action "Edit the
  command line here". A pulled program is laid out by its structure.
  The pencil icon joined the sprite; syntax colours are tokens
  (`--syn-*`, light and dark) in `style.css`.
- Wiki: `protocols/server-commands`, "Compiling text through the server".

**Tests.** `crates/saturnus-host/tests/editor.rs` (ROM-gated, 48SX, 48GX,
49G): a program pulled, a no-op save with the checksum, the stack, flags
and screen unchanged and no `SATEDIT` left; an edit with `→`, `;` and a
string read back equal to the edited text; a multi-line edit read back
in the calculator's form; `Invalid Syntax` with the object, the stack and
the directory intact; two objects refused; a `}` refused before
anything runs; a 3500-character program; a variable in another
directory; no-op saves of ten kinds of object in FIX 3 with unchanged
checksums; a stack level replaced and a syntax error there; a live
`EDIT` session replaced with `replace`, still in the edit, and a save
refused while it is open. Unit tests: the compile reply (error, count,
leftovers dropped), the wrapper check, the temporary name, `edit_text`,
the engine's field refusals, the HTTP endpoint's command list.
`web/test/editor.test.mjs` (11 tests: tokens, highlighting, brackets,
open delimiters, indentation, layout, digraphs, completion, history,
sessions, the transport with a fake backend); `overflow.test.mjs` now
also checks the editor at every width and as a phone sheet with the
keyboard up.

**Verified in headless Chrome** (Worker, the real page) on the 48SX,
48GX and 49G: Edit on P in the explorer opens `« 1 2 + »` highlighted
(delimiters, numbers, `+` as a command); typing marks it unsaved; Cmd+S
says "Saved P in HOME in 0.24 s" (click to message 250 ms) and the
explorer already shows 28 bytes and the new checksum when the message
appears; `« 1 2 + 3 * ) »` gives "The calculator says: Invalid Syntax.
Nothing was changed." with P's checksum and the stack unchanged; Escape
warns once, then discards; level 2 edited from 42 to 43; with `1 2 « 3`
half typed, "Edit line" appears, pulls the text with the cursor at 7,
` 4 +` typed and sent back leaves the calculator in the same line
`1 2 « 3 4 +` with the cursor at the end; inside the ROM's EDIT (left
shift +/- on the 48, ↓ on the 49G) the line is replaced and ENTER puts
the new object on level 1; free text: `<<` Shift+Enter grows the
palette, `->` and `<<`/`>>` become `→`, `«`, `»`, Enter indents, `DU`
offers DUP, DUP2, DUPN (and DUPDUP on the 49G), Tab takes DUP, a `»`
typed first on its line moves out, Cmd+Enter runs it and the history
holds it. On a 390 px phone in dark mode the editor fills the sheet
with Format and Save under it (`phone-dark-editor.png`).

**Acceptance boxes** are ticked on that headless evidence.

**Review of PR 51** (fixed on the branch): text with a `"` or `@` right
after a word's character, or a string left open, is refused before
anything runs (ROM-gated: `X@ } 'P' PURGE {`, `A"B } 'P' PURGE {`,
`"} 'P' PURGE {` leave P as it was on all three models); the ROM's
reading of a mid-word `@` and `"` is in the wiki
(protocols/server-commands). `was` is the object's size and checksum,
so a change of the binary base between opening and saving no longer
refuses the save (ROM-gated). A failed read after a successful save
asks for a reopen instead of making every later save fail (web test).

**Not done or not verified.**

- The desktop app's webview (WKWebView) was not driven: Cmd+S there,
  and whether the app's menu takes it first, need the owner's hands
  (done 2026-10-09, below). Safari and Firefox were not run either.
- The 49G in algebraic mode: `storeText` is refused there with the way
  out (clear -95), as all of 12b's writes but `setFlag`.
- A variable whose name is a command (`SQ` on the 48SX) cannot be stored
  by name: the calculator says `Invalid Syntax` at the store, after the
  compile; nothing changes.
- The command-line watch reads RAM a quarter second after each frame;
  with the clock shown that is one read a second.

**Owner check (2026-10-09).** The owner confirmed editing works: Edit
from the memory view, Cmd+S and Edit line. Asked for one change: a save
that went through closes the editor (the button and Cmd/Ctrl+S alike),
one that did not keeps it open with the error and the text; the status
line says what was saved (decision log, 2026-10-09). And a shortcut:
Cmd/Ctrl+E edits the memory view's selected object, else stack level 1.
