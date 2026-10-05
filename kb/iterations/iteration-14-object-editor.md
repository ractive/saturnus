---
title: "Iteration 14: The palette's editor mode (command line, stack objects, variables)"
type: iteration
date: 2026-10-05
status: planned
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

- [ ] Editor mode of the palette: grows to a multi-line editor on
  Shift+Enter, on an unclosed delimiter, or when text is pulled in; RPL
  syntax highlighting (delimiters, numbers, strings, names, commands from
  the 13a data), bracket matching, indentation of nested `« »` and
  structure words, completion as in the palette, ASCII digraphs shown as
  the real characters (`<<` `>>` `->` and the transfer codes), a history
  of sent text. Framework-free unless a code-editor library is clearly
  worth its weight; if one is proposed, say what it costs in size and
  licence and vendor it (no CDN).
- [ ] "Edit in the app" for a live command line: a control beside the
  calculator when `commandLine.active`; pulls the text and cursor, sends
  it back with `replace`.
- [ ] Edit buttons in the explorer and the stack view for objects with a
  text form: pull the decompiled text, save back by keys; a dirty
  indicator; save on Cmd/Ctrl+S; the calculator's error surfaced on a
  failed compile, the original intact.
- [ ] Tests on the 48SX, 48GX and 49G: a program in a variable pulled,
  changed, saved, and read back equal to the edited text; a no-op edit
  leaves the object's checksum unchanged; a syntax error surfaces and
  leaves the object intact; a live `EDIT` session replaced.

## Acceptance criteria

- [ ] Opening a program from the explorer shows highlighted RPL; editing
  and saving it changes the variable on the calculator, and the explorer
  shows the new size and checksum within a second of the save finishing.
- [ ] With a command line open on the calculator, pulling it into the
  editor, changing it and sending it back leaves the calculator in the
  same edit with the new text.

## Outcome

(to be written)
