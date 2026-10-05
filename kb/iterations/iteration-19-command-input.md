---
type: iteration
title: "Iteration 19: Command input box, paste, and editing the command line in the app"
date: 2026-10-05
status: planned
tags:
  - iteration
  - saturnus
branch: iter-19/command-input
---

# Iteration 19: Command input box, paste, and editing the command line in the app

**Draft for discussion with the owner (2026-10-05). Not started; the
open questions at the end are to be settled first.** "App" means the web
page and the desktop app alike.

Read first: `web/protocol.md` (`typeLetter`, `typeKeys`, `typeText`,
`keyScript`), `crates/saturnus-web/src/host.rs` (the key queue),
`crates/saturnus-objects/src/ram.rs`, `kb/iterations/iteration-12c-rpl-decompiler.md`,
`kb/iterations/iteration-13a-command-data.md` (the per-model command
lists), wiki `hardware/keyboard` (the ROM's key buffer), `hardware/hp48-system-ram`.

## Context (owner, 2026-10-05)

- On the command reference panel: "If you click somehow on it or an icon
  besides, the command is entered in the calculator." (A stretch goal of
  iteration 13; it needs the typing engine of this iteration.)
- "It would be nice to have an input box in the app that lets you write
  commands easily and then send it to the calculator. It could even have
  code completion."
- "When editing something in the calculator, you may even 'sniff' it from
  memory, edit it in the app and then send it back to the calculator to
  replace what you are currently editing. Not sure if this is possible."

## What exists

- `typeText` types letters (through alpha), digits, space, `+ - * / .`
  and ENTER, at most 1000 characters, and refuses anything else. There is
  no mapping from an arbitrary RPL character (`« » { } [ ] ' " # → Σ π ∂
  ≤ ≥ ≠ √ ∫` and the rest of the calculator's character set) to the key
  presses that produce it on each model.
- Reads from RAM are in place (`saturnus-objects`): tree, stack, flags.
  The command line being edited is not read yet.
- The decompiler (iteration 12c) will give the text of objects on the
  stack and in variables.
- Kermit is not available to the page until `kermit-proto` is published
  (iterations 12b, 18). Everything here is designed to work with
  keystrokes alone; Kermit can replace the slow path later.

## Design (proposal)

1. **A typing engine for the whole character set.** Per model, a table
   from each character of the calculator's character set to the key
   sequence that types it in the command line (plain key, alpha, alpha
   with shifts, or the model's characters menu where there is no key),
   derived from the skins' key legends and verified on the ROM: type the
   character, read the command line back from RAM, compare. The table is
   generated and checked by a ROM-gated test, not written from memory.
   The engine keeps track of the entry state it creates (alpha lock,
   shifts) and leaves the keyboard as it found it. Line breaks are the
   calculator's newline, not ENTER, inside multi-line text.
2. **Reading the command line.** Where the ROM keeps the text being
   edited and the cursor (system RAM pointers, to be found as the HOME
   pointers were in 12a, per model), exposed as `commandLine` in the
   protocol: `{text, cursor, active}`. Read-only, no key sent.
3. **Sending text.** Text goes to the calculator as key presses at
   unlimited speed with a busy indication; the calculator parses it, so
   syntax errors are the calculator's own and show on its screen (and are
   read back for the box). Three verbs:
   - *Insert*: type the text at the cursor of the current command line
     (or start one).
   - *Run*: insert and press ENTER.
   - *Replace*: clear the command line being edited and type the new
     text, staying in the edit so that ENTER on the calculator commits it
     as usual. How to clear without leaving an `EDIT`/`VISIT` session is
     model-specific and must be established on the ROM.
4. **The input box.** A multi-line editor under or beside the calculator:
   RPL syntax highlighting (delimiters, numbers, strings, names,
   commands), completion from the model's command list (13a) with the
   stack effect and one-line description beside each proposal, and from
   the live variable names of the current directory and its parents
   (12). Enter runs, Shift+Enter inserts a line break, a history of sent
   lines, ASCII digraphs for characters a PC keyboard lacks (`<<` `>>`
   `->` and the transfer translations such as `\GS`), shown as the real
   characters. Focus rules as for the explorer: the box takes the
   keyboard only while it has focus.
5. **"Edit in the app".** With a command line active on the calculator, a
   button pulls its text into the box; *Replace* sends it back. For
   objects on the stack or in variables the same box edits the
   decompiled text (12c) and sends it back as `text` followed by the keys
   that store it (iteration 14 then becomes mostly this flow plus the
   explorer's affordances).
6. **Paste.** Pasting into the page with the calculator focused types
   the clipboard text through the same engine.
7. **Click to enter** from the reference panel (iteration 13): the
   command's name is inserted at the cursor through the same engine.

## Limits to be honest about

- Only what the calculator's command line can parse: no libraries, no
  backup objects, no binary objects without a text form.
- Speed: every character is one to four key presses and each press needs
  the ROM's debounce time. Rough estimate before measurement: a 1000
  character program is on the order of 100 s of emulated time, a few
  seconds of wall time at unlimited speed. The calculator's clock runs
  ahead by that much. A faster path (writing key codes into the ROM's own
  key buffer, the way its interrupt handler does) was already noted as a
  possible refinement in iteration 12; it is a RAM write and needs its
  own care. Kermit will be the fast path for large objects.
- The calculator's entry modes change what keys mean (algebraic entry
  after `'`, program entry, the 49G's algebraic operating mode). The
  engine types through alpha lock for letters, which is mode-independent,
  but operators and delimiters need checking in each mode.
- Models: 48SX, 48GX, 49G first. The 38G/39G/40G have a home command
  line that could take expressions later. The 42S types letters through
  ALPHA menus and is out of scope here.

## Tasks (to be fixed after the discussion)

- [ ] Research: command-line buffer and cursor in system RAM per model;
  how to clear it inside an edit session; key timing floor (how fast the
  ROM accepts presses); the key buffer as a faster path. Wiki pages.
- [ ] Character-to-keys tables per model, generated and ROM-verified.
- [ ] `typeText` for the full character set; `commandLine` read;
  `insert`/`run`/`replace` in the shared runner, the Worker and the
  control API (`saturnus ctl type` gains them).
- [ ] The input box component with highlighting, completion and history.
- [ ] Edit-in-the-app for the command line; for stack and variable
  objects once 12c is merged.
- [ ] Paste; click-to-enter from the reference panel.
- [ ] Verification on the three models: every character of the set
  round-trips; a multi-line program typed, run and read back; an `EDIT`
  session replaced from the box; measurements of typing speed.

## Open questions for the owner

1. Where does the box live: always visible under the calculator, a tab of
   the side layer next to explorer/flags/reference, or a pop-up (a
   shortcut opens it)?
2. Enter runs and Shift+Enter breaks the line, or the reverse (editor
   habit)?
3. Is typing by key presses at unlimited speed acceptable as the first
   send path, with its visible flicker on the calculator's screen while
   it types, or should the screen be frozen and a busy mark shown until
   it is done?
4. Should the box replace iteration 14's separate object editor, as
   proposed in point 5?
5. Is the key-buffer fast path wanted in this iteration, or only when
   typing proves too slow?
