---
type: iteration
title: "Iteration 19: Typing engine for the full character set and reading the command line"
date: 2026-10-05
status: planned
tags:
  - iteration
  - saturnus
branch: iter-19/command-input
---

# Iteration 19: Typing engine for the full character set and reading the command line

The engine under the command palette ([[iterations/iteration-13-command-reference]])
and its editor mode ([[iterations/iteration-14-object-editor]]). No UI in
this iteration beyond paste. "App" means the web page and the desktop app
alike.

Read first: `web/protocol.md` (`typeLetter`, `typeKeys`, `typeText`,
`keyScript`), `crates/saturnus-web/src/host.rs` (the key queue),
`crates/saturnus-drive/src/runner.rs`, `crates/saturnus-objects/src/ram.rs`,
wiki `hardware/keyboard` (the ROM's key buffer), `hardware/hp48-system-ram`.

## Context (owner, 2026-10-05)

- "It would be nice to have an input box in the app that lets you write
  commands easily and then send it to the calculator. It could even have
  code completion." and "When editing something in the calculator, you
  may even 'sniff' it from memory, edit it in the app and then send it
  back to the calculator to replace what you are currently editing."
- Decided in the discussion that followed: the box is a command palette
  opened with Cmd+K (iteration 13), not a permanent field; Enter mimics
  the calculator's own keys (execute when no command line is open, insert
  when one is, Cmd+Enter for the opposite); while longer text is typed
  the calculator's screen is frozen with a busy mark; editing stored
  objects goes through the palette's editor mode (iteration 14). All
  sending is by key presses; Kermit is not needed.

## What exists

- `typeText` types letters (through alpha), digits, space, `+ - * / .`
  and ENTER, at most 1000 characters, and refuses anything else. There is
  no mapping from an arbitrary RPL character (`« » { } [ ] ' " # → Σ π ∂
  ≤ ≥ ≠ √ ∫` and the rest of the calculator's character set) to the key
  presses that produce it on each model.
- Reads from RAM (`saturnus-objects`): tree, stack, flags, and since
  iteration 12c the text of programs and algebraics. The command line
  being edited is not read yet.

## Design

1. **Typing engine for the whole character set.** Per model, a table from
   each character of the calculator's character set to the key sequence
   that types it in the command line (plain key, alpha, alpha with
   shifts, or the model's characters menu where there is no key), derived
   from the skins' key legends and verified on the ROM: type the
   character, read the command line back from RAM, compare. Generated and
   checked by a ROM-gated test, not written from memory. The engine
   tracks the entry state it creates (alpha lock, shifts) and leaves the
   keyboard as it found it. Line breaks inside text are the calculator's
   newline, not ENTER.
2. **Reading the command line.** Where the ROM keeps the text being
   edited and the cursor (system RAM pointers, found as the HOME pointers
   were in 12a, per model): `commandLine` in the protocol, returning
   `{active, text, cursor}`. Read-only, no key sent. `active` is what the
   palette's Enter rule needs.
3. **Sending.** Three verbs in the shared runner, the Worker and the
   control API: `insert` (type at the cursor, or start a command line),
   `run` (insert, then ENTER), `replace` (clear the command line being
   edited and type the new text, staying in the edit; how to clear
   without leaving an `EDIT`/`VISIT` session is model-specific and is
   established on the ROM). Text runs at unlimited speed in emulated
   time. Errors are the calculator's own: after a `run`, the reply says
   whether the command line closed and carries the error message the ROM
   shows, read from the screen or from RAM.
4. **Frozen screen while typing** (owner): a send of more than a few
   characters holds the last frame and raises a `busy` state in the
   protocol; frames resume when the calculator is idle again. A short
   send (one command name) shows as it is typed. The threshold is a
   constant, chosen from measurement.
5. **Paste.** Pasting into the page with the calculator focused sends the
   clipboard text through `insert`.

## Limits to be honest about

- Only what the calculator's command line can parse: no libraries, no
  backup objects, no binary objects without a text form.
- Speed: every character is one to four key presses and each press needs
  the ROM's debounce time. Rough estimate before measurement: a 1000
  character program is on the order of 100 s of emulated time, a few
  seconds of wall time at unlimited speed, and the calculator's clock
  runs ahead by that much. Measure it. A faster path (writing key codes
  into the ROM's own key buffer, as its interrupt handler does) is a RAM
  write and is only taken up if the measurement says typing is too slow
  (owner: wait for the measurement).
- Entry modes change what keys mean (algebraic entry after `'`, program
  entry, the 49G's algebraic operating mode). Letters go through alpha
  lock, which is mode-independent; operators and delimiters are checked
  in each mode.
- Models: 48SX, 48GX, 49G. The 38G/39G/40G home command line may follow
  later. The 42S types letters through ALPHA menus and is out of scope.

## Tasks

- [ ] Research and wiki: command-line buffer, cursor and "active" state in
  system RAM per model; clearing the command line inside an edit session;
  the key timing floor (how fast the ROM accepts presses).
- [ ] Character-to-keys tables per model, generated and ROM-verified
  (every character of the set round-trips through the command line).
- [ ] `typeText` for the full set; `commandLine`; `insert`, `run`,
  `replace`; the `busy` state and frozen frames; in the runner, the
  Worker and the control API (`saturnus ctl type` and a `ctl cmdline`).
- [ ] Paste in the page and the desktop app.
- [ ] Verification on the three models: the round trip of the character
  set; a multi-line program typed, run and read back equal (through the
  12c decompiler); an `EDIT` session replaced; typing speed measured and
  written into the Outcome with the decision on the key-buffer path.

## Acceptance criteria

- [ ] `saturnus ctl type` sends `« 1 2 + » EVAL` to a 48SX, 48GX and 49G
  and the stack shows 3; `ctl cmdline` returns the text and cursor of a
  half-typed command line without pressing a key.
- [ ] Every character of each model's set round-trips.
- [ ] `just gates` passes.

## Outcome

(to be written)
