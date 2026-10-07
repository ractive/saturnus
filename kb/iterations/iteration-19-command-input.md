---
type: iteration
title: "Iteration 19: Typing engine for the full character set and reading the command line"
date: 2026-10-05
status: completed
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

- [x] Research and wiki: command-line buffer, cursor and "active" state in
  system RAM per model; clearing the command line inside an edit session;
  the key timing floor (how fast the ROM accepts presses).
- [x] Character-to-keys tables per model, generated and ROM-verified
  (every character of the set round-trips through the command line).
- [x] `typeText` for the full set; `commandLine`; `insert`, `run`,
  `replace`; the `busy` state and frozen frames; in the runner, the
  Worker and the control API (`saturnus ctl type` and a `ctl cmdline`).
- [x] Paste in the page and the desktop app.
- [x] Verification on the three models: the round trip of the character
  set; a multi-line program typed, run and read back equal (through the
  12c decompiler); an `EDIT` session replaced; typing speed measured and
  written into the Outcome with the decision on the key-buffer path.

## Acceptance criteria

- [x] `saturnus ctl type` sends `« 1 2 + » EVAL` to a 48SX, 48GX and 49G
  and the stack shows 3; `ctl cmdline` returns the text and cursor of a
  half-typed command line without pressing a key.
- [x] Every character of each model's set round-trips.
- [x] `just gates` passes.

## Outcome

- **Command line in RAM** (wiki: hardware/command-line, found by
  observation: typed text scanned for in RAM, system RAM diffed across key
  presses; no document gives it). The text is stored reversed and
  NUL-terminated just below the temporary environments: first character
  at `[local_vars] - 2` (#70583 on the 48SX, #80702 on the 48GX and 49G),
  no length field. Cursor (5 nibbles) at #70704 / #80882 / #80F61; the
  editor's nibbles: active and lowercase lock (#70687 / #80805 / #80EC4),
  insert and algebraic entry (#70685 / #80803 / #80EC2), shifts and alpha
  (#706C4 / #80842 / #80F01), alpha lock (#70793 / #80911 / #80FF0),
  program entry (#70794 / #80912 / #80FF1), message shown (48SX at
  address #7068B, 48GX at address #80809; on the 49G an error box
  makes the nibble at #80EC8 or #80EC9 read #F). Read by
  `saturnus_objects::cmdline` (`commandLine`).
- **Clearing inside EDIT/VISIT**: DEL for the characters after the
  cursor, backspace for those before; the session stays open and ENTER
  replaces the edited level (checked on all three).
- **Key timing** (measured): without waiting for SHUTDN between presses
  keys are lost even at 60/30 ms; with the wait, 25 ms holds were reliable
  (15 ms lost keys on the 48SX); a repeated key needs about 80 ms after
  its release (the ROM polls releases every 1/16 s; the 49G lost repeats
  below 70 ms). The engine holds 30 ms, waits for SHUTDN, then 20 ms
  (100 ms before the same key again). A garbage collection in the middle
  of a long 48GX line kept the ROM busy for over 5 s, so a key may keep it
  busy up to 30 s before typing gives up.
- **Character tables, generated** (`crates/saturnus-web/src/typing/*.tsv`,
  written by `tests/typing.rs` with `SATURNUS_TYPING_REGEN=1` from a survey
  of every key, plain and shifted, in alpha mode in immediate, algebraic
  and program entry (and the 49G's RPN mode), every key in program entry
  with alpha off, and the six accent keys after every letter, each read
  back from RAM). Committed rather than computed from the skins: most
  characters come from alpha-shifted keys whose characters are printed
  on no key, so the skins cannot give them, and a survey takes seconds per
  model. Coverage of characters 1-255: 48SX 195 (121 alpha, 8 pair keys,
  5 in program entry `^ √ ∫ Σ ∂`, 61 accented letters; 60 impossible:
  control characters but newline, `;`, backslash, backquote, DEL, `∇ ▶ ■`
  and 23 Latin-1 signs), 48GX 255 (60 through CHARS), 49G 255 (110 alpha,
  6 in program entry, 61 accented, 70 through CHARS). NUL never (the
  editor refuses it).
- **Engine** (`saturnus_web::typing::Job`, stepped in emulated time):
  per character the table's method, then the line is read back and
  checked; what a key inserts beyond the character is trimmed (spaces and
  `()` in program entry), a pair's closer is stepped over when the text
  closes it and deleted at the end otherwise. Modes are read from RAM:
  alpha and lowercase locks, pending shifts and replace mode; the
  keyboard is restored afterwards (the entry mode may stay in program
  entry). Round trip of every typable character, starting in immediate,
  algebraic and program entry (and RPN on the 49G): passes on the three
  ROMs (`every_character_round_trips`; the default debug run does
  immediate entry only, `SATURNUS_TYPING_FULL=1` all of them).
- **Protocol** (`web/protocol.md`, "Typing", one section for all hosts):
  `commandLine`, `insert`, `run` (adds `closed`, `error`, `running`),
  `replace`; `typeText` is now `insert` (newline is the calculator's
  newline). `status` has `busy`: a send of more than 12 characters holds
  the frames (Tauri runner test: no `frame` between the two statuses; the
  Worker likewise). HTTP: `POST /v1/type` takes the verbs, `GET
  /v1/cmdline`; `saturnus ctl type [--run|--replace]`, `ctl cmdline`.
- **Errors after run**: the 48 leaves a line that does not parse open and
  shows the message in the header, the 49G in a box (dismissed with ATTN,
  which returns to the line). No error number is stored for a parse
  error; the message is read from the string objects the ROM built in RAM
  to show it (a heuristic: new, printable, not part of the typed text, not
  a menu label; the command line goes first: "DROP Error: Too Few
  Arguments"). Checked for "Invalid Syntax" and "Too Few Arguments".
- **Verified** on the 48SX, 48GX and 49G: `ctl type --run "« 1 2 + »
  EVAL"` leaves 3 (49G after `CF(-95)`: its default algebraic mode does
  not parse RPN text); `ctl cmdline` reads `1 2 « 3` with the cursor at 5
  and the screen unchanged; a syntax error keeps the line with the
  message; `replace` inside `EDIT` then ENTER replaces the level; a
  multi-line program with `→`, `FOR`, `IF` and strings, typed and stored,
  decompiles (12c) to the sent text and runs. Paste in headless Chrome
  (Worker): a CRLF clipboard text arrives as the program with newlines,
  `busy` true then false, no frame while typing. Not checked: paste in
  the desktop app's webview (same page code).
- **Speed** (characters per second, plain text / a program with many
  shifted characters): emulated 3.7 / 2.4 (48SX), 5.5 / 3.6 (48GX), 10.4 /
  7.3 (49G); wall, native release 348 / 215, 572 / 378, 959 / 680; wasm in
  headless Chrome 227 / 137, 375 / 242, 675 / 458. Per key the ROM's own
  work dominates (268 ms per key on the 48SX, of which 50 ms are the
  engine's hold and gap), so the key-buffer fast path would save at most
  about a fifth. **Recommendation: do not build it.** A command name or a
  short program is typed within a frame or two; a 1000-character program
  takes about 7 s of frozen screen on a 48SX in the browser (4096
  characters would reach the 30 s limit there). If long pastes matter,
  writing the text into the edit buffer directly (the layout above) is
  the path that would be fast, a separate decision since it is not key
  presses.
- **Deviations**: `typeText` changed meaning (newline, full set) instead
  of keeping the old letters-and-digits typing; the MCP's `type_text`
  keeps its own map (aplet models). The cap is 4096 characters, not 1000,
  and a send estimated to need more than 10 minutes of emulated time is
  refused (review of PR 25: 4096 characters could not finish on a 48SX).
