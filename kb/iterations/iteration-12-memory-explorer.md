---
title: "Iteration 12: Memory explorer layer (variables, directories, transfers)"
type: iteration
date: 2026-10-05
status: completed
branch: iter-12/memory-explorer
tags:
  - iteration
  - saturnus
---

# Iteration 12: Memory explorer layer (variables, directories, transfers)

Split 2026-10-05: the research and the read API are [[iterations/iteration-12a-memory-read-api]];
this plan is the read-only UI layer (explorer and flags panel); the write
path is [[iterations/iteration-12b-explorer-writes]] and waits for
`kermit-proto` on crates.io. Iterations 11 and 12a are merged.

Read first: `crates/saturnus-mcp/src/object.rs` (the exact object decoder
from iteration 9), wiki `protocols/hp-object-format`, `hardware/hp48sx`
(system RAM addresses known so far: KeyBuf #704EA, OUT shadow #704C3,
display ghost registers), the SDK's RPL documentation
(`~/devel/hp-literature/raw/saturn-hardware/hp48-sdk-1993/{RPLMAN.TXT,RPL.TXT}`)
and HP's entry tables for the directory structures, `kb/decision-log.md`
(iteration 9: server-mode cost), `~/devel/hpcomm` (the owner's connectivity
kit; the two-pane PC/calculator explorer is the interaction model).

## Context (2026-10-05)

- Owner: besides the pure calculator view, "another layer on top that
  shows you some infos like all variables, or the current folder, maybe
  you can even navigate the folders outside the calculator", modern
  version of the HP Connectivity Kit / hpcomm two-pane UI. Asked whether
  this needs server mode or whether the emulator has a side channel.
- Design agreed: **reads** go straight to RAM (the emulator owns it): the
  directory tree from HOME, variables with name, type, size, checksum, the
  current path, the stack; decoded with the iteration 9 decoder; no mode
  switch, instant, refreshed when the directory area changes; available to
  the web page through the wasm bindings and to Tauri alike. **Writes**
  (store, purge, rename, send a file) go through a hidden Kermit
  transaction at unlimited speed (about 15 s emulated per round trip is
  about 0.2 s wall-clock), so the ROM's memory manager stays consistent;
  a later refinement may inject key codes into the ROM's key buffer for
  command-line operations. Direct RAM writes are out.
- Open research: the per-model locations of the directory roots and the
  current-path pointer in system RAM (48SX, 48GX, 49G; the 38G/39G/40G
  have aplets, not a HOME tree, treat them later), and how to detect a
  change cheaply (hash of the directory region or a ROM write hook).

- Write path (owner, 2026-10-05, with the retirement of saturnus-mcp):
  `hptx-core` is not compiled into the page or any saturnus host. The
  "hidden Kermit" of this plan is implemented with `kermit-proto` from
  crates.io (protocol only, no serial-port dependency) plus the HP file
  header and text encoding that live in `saturnus-objects`; keystrokes
  are the fallback where no Kermit server exists. Blocked on
  `kermit-proto` being published; the read-only parts are not.

## Tasks

- [x] Protocol: the read commands reserved in `web/protocol.md`
  (`memoryTree`, `stack`, `flags`, `objectAt`) and the `memoryChanged`
  event implemented in the Worker host and the shared runner (Tauri,
  HTTP), fed by `saturnus-objects`; the change counter is polled on the
  machine side, the page is told only when it moved (and at most a few
  times per second). Models without RPL memory (38G, 39G, 40G, 42S)
  answer with the existing refusal and the page hides the layer.
- [x] Explorer layer in the web page and the desktop app (a toggle; the
  calculator stays usable beside it): the HOME tree with the current
  directory highlighted, the variable list of the selected directory
  (name, type, size, checksum), a typed object preview, copy-to-clipboard
  of an object's text form, the stack as typed levels. A `<sat-explorer>`
  component on the shared store, following hpcomm's two-pane model
  (`~/devel/hpcomm`) where it applies to reading. Layout for desktop
  (side by side) and narrow screens (the layer over the calculator).
- [x] Flags panel (owner, 2026-10-05: "The UI could also handle all flags
  that you can set"), read-only in this iteration: system flags -1 to -64
  with their meaning per model and user flags 1 to 64, read live, grouped
  by topic with the current value (the 49G's second flag words included).
  The flag meanings are transcribed as facts, in our own words, into the
  wiki first (one page per model family, citing the user's guides), and
  shipped as data generated from those pages.
- [x] Verification in headless Chrome on the 48SX, 48GX and 49G: create a
  variable and a directory on the calculator, change directory, push
  objects, set a flag by keys: the explorer, the stack view and the flags
  panel follow within a second without any key being sent by the page;
  the calculator's speed and idle behaviour are unchanged with the layer
  open (0 run passes while idle; emulated time equals wall time).

## Acceptance criteria

- [x] With a 48SX running, the explorer shows the HOME tree, the
  variables, the stack and the flags live without server mode; creating a
  variable on the calculator shows up in the explorer within a second.
- [x] With the layer open and the calculator idle, the page still sleeps
  (no run passes), and `just gates` passes.

## Outcome

Built 2026-10-05 on `iter-12/memory-explorer`. Evidence (not committed) is
in the session's scratch directory
`/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/iter12/`:
headless Chrome over the DevTools protocol with real mouse and key events
(`lib.mjs`; `scenario.mjs`, `idle.mjs`, `design.mjs`), reports
`{48sx,48gx,49g}-report.json`, `{48sx,48gx,49g}-idle.json`,
`design-report.json`, `tauri-selftest.log`.

**Found first, and what it changed.** The read API of
`saturnus-objects` gave programs, algebraics, units and built-in
commands without text. Reported before any code was written; the lead
moved the decompiler to iteration 12c, which was merged (PR 23) before
this iteration's PR. The page was written against the shape 12c
delivers (`source`, `unit`, a command's `name`) and needed no change to
its preview code when the text arrived, only its wording: the "no text"
sentence remains for objects that really have none (a graphic, a library,
a backup; a program or expression holding something the ROM's tables do
not name; such a list shows grey placeholders and cannot be copied), and
an XLIB name of an unknown library shows as `XLIB n m`.

Text previews verified after the merge, by keys on the 48SX, 48GX and
49G (`previews.mjs`, `{48sx,48gx,49g}-previews.json`, screenshots
`*-21-text-{G,H,U,C}.png`): the program
`« → N « IF N THEN 1 N FOR I I NEXT ELSE 0 END » »` is shown in indented
lines and "Copy text" gives exactly that line, as the calculator's
display shows it; the algebraic `'A+2*B'`, the unit `9.81_m` and the list
`{ 1 SIN DUP }` (its commands named in the element list) likewise. The
older hand-made checks (`d-hand-*.png`) remain as the component-level
test of the same path.

**What was built.**

- Protocol (`web/protocol.md`, "The user memory, read-only"): `watchMemory`,
  `memoryTree`, `stack`, `flags`, `objectAt` and the `memoryChanged` event
  in the Worker and in the shared runner (Tauri, HTTP) alike; `stats`
  gained `memoryLooks` and `memoryMs`. New binding `memory_refusal()`.
  The looks happen on the machine's side (see the decision log); runner
  test `memory_changes_reach_a_watching_page`.
- `<sat-explorer>` (`web/components/sat-explorer.js`), `web/memory.js`
  (the reads, on the store), `web/objects.js` (text forms, program
  layout, previews, flag rows; 11 Node tests in `web/test/`, new gate
  `just web-test`, also in CI's `wasm` job). Three tabs: Variables (tree,
  list with name, type, size, checksum, preview, copy, search over all
  directories), Stack, Flags.
- Flag meanings: wiki pages `hardware/system-flags-48sx`,
  `hardware/system-flags-48gx`, `hardware/system-flags-49g` and the
  questions `hp48sx-system-flags`, `hp49g-system-flags`;
  `scripts/flags-json.py` writes `web/flags.json` from them (`just
  flags`; `--check` in `just lint` and CI). 48G/GX: 61 flags known, 3
  unused, from the User's Guide's appendix D. 48S/SX: 57 known and 7 unused, from
  appendix E of the Owner's Manual (added to the library during the
  iteration; a first version had carried the 48G meanings over as
  "assumed", which is gone). 49G: its two guides do not
  list the flags (they refer to the Pocket Guide, which the library
  lacks): 12 known, 116 shown without a meaning.
- `pages.yml` copies the three new page files.

**After review (PR 24).** The page formatted numbers and objects itself
and got three cases wrong (a name in a 49G array, a whole-number unit on
the 49G, a small real). Now every text the page shows or copies is the
host's: `saturnus_objects::described` puts the calculator's own `text`
on each object and on every object inside it, `stack` and `objectAt`
return that, and `web/objects.js` keeps only layout. Checked by a unit
test of `described`, by the ROM oracle (`decompiler_matches_the_rom`
compares the texts of a stack holding `1.23456789012E-5`, `2_m`, `'QQ'`,
a list and, on the 49G, `[ 'QQ' ]` with the ROM's display) and by keys in
the browser on the 48SX and 49G (`previews.mjs`: copies `2_m`,
`1.23456789012E-5` and `[ 'A' ]` exactly). A failed object read is no
longer kept (`ObjectLoader`, tested), and a ROM booted while a read is in
flight is read for itself (`MemoryView.refresh`, tested with a fake
backend).

**Layout and focus** are in the decision log (2026-10-05, iteration 12).

**Verified in headless Chrome, by keys only** (drawn keys clicked with the
mouse, letters typed on the keyboard; on the 49G after switching it to
RPN through MODE). On each of the 48SX, 48GX and 49G: `42 'A' STO`,
`'D' CRDIR`, `D` (enter it), then in D a real, a string with `→` in it,
a list, a program and a matrix stored, numbers and a list pushed, user
flag 5 and a system flag (-17; -105 on the 49G) set and cleared. Every
step appeared in the tree, the list, the stack view or the flags panel;
the time from the last key until the page showed it:

| Model | after the key's release | after its press | after the calculator went idle |
| --- | --- | --- | --- |
| 48SX | 612 to 930 ms | 724 to 1042 ms | 2 to 13 ms |
| 48GX | 386 to 561 ms | 499 to 676 ms | 1 to 12 ms |
| 49G | 226 to 334 ms | 338 to 445 ms | 3 to 12 ms |

The view is as fast as the ROM: it follows within 13 ms of the moment
the calculator itself has finished the key. On the 48SX the ROM takes up
to 0.9 s for an ENTER of a typed command line at real speed, so "within
a second" holds from the key's release and misses by up to 42 ms from
its press.

The page sent no key of its own: per run the Worker received exactly as
many `keyDown` as keys were clicked (77, 79 and 94 on the three models),
as many `typeLetter` as letters were typed (23) and no `typeKeys`;
browsing, selecting, searching and copying in the layer sent none
(`design-report.json`: 0 while navigating, 0 for a digit typed with the
focus in the layer, 1 for the same digit after Escape). No console
error and no uncaught rejection in any run; no request outside the
page's own origin in the scenario runs (the others did not record
requests).

Previews from the calculator: real `3.5`, string `"HI → "` (5
characters), list of 3 by element, matrix as a 2 × 2 grid, program (at that time
without text, see above); copied texts `{ 1 2 3 }`,
`[ [ 1 2 ] [ 3 4 ] ]`, `"HI → "`, `3.5`. Hand-made: an indented program,
a 1000-element list cut at 200 with "and 800 more", a 30 × 12 matrix cut
to 24 × 8 with the count, a graphic with its nibbles behind a
disclosure, the decode budget's message as a readable refusal
(`d-decode-refusal.png`).

Idle and speed, 10 s each, layer closed then open (`*-idle.json`), the
same on all three models: idle 0 run passes, 20 wakes (the ROM's own),
emulated time equal to wall time (ratio 1.0000) both ways; with the layer
open 20 looks at the memory costing 0.3 to 0.7 ms in all, no event, no
read. Computing (`1 300000 START NEXT`): 0.9986 to 1.0006 of real time at
1x and 3.995 to 4.004 at 4x, open or closed, no looks while it computes.

38G and 42S: the layer explains itself ("No memory view for the HP 38G:
the 38G keeps aplets, not a HOME directory"; the 42S "has no RPL user
memory"), the three reads are refused with that reason
(`d-38g-explained.png`, `d-42s-explained.png`). Before the ROM has set
up memory the Variables tab says so (`03-48sx-prompt.png`).

Desktop app: built and run as `cargo tauri dev` runs it (the debug
binary, `SATURNUS_SELFTEST`), self-test extended
(`tauri-selftest.log`): window 1100 × 868, columns 252, 386 and 462 px,
the layer supported on the 48SX, the stack tab showed `1: 5`, then
`2: 5, 1: 7` 810 ms (first run) and 1028 ms (second run) after the
press of ENTER, the flags tab 6 of 64 set
and 45 described; idle with the layer open 0 passes and 10 looks in 5 s
(0.58 ms), 100.00% of real time; computing 100.00% and 400.00%.

Screenshots to look at: `48sx-12-dir-D.png`, `48sx-13-preview-{R,T,L,P,M}.png`,
`48sx-14-stack.png`, `48sx-16-flags-set.png` (RAD on the calculator and
-17 lit), the same names for `48gx-` and `49g-`; `d-light-flags.png`,
`d-light-flags-end.png`, `d-search.png`, `d-browse-home.png`,
`d-keys-in-panel.png`, `d-keys-list.png`; `d-dark-{1-vars,2-stack,3-flags}.png`;
`d-tauri-1100x900-*.png`, `d-medium-900x800-*.png`,
`d-phone-390x844-*.png`, `d-wide-1920x1080-*.png`.

**For the owner to look at by eye**: the app's real window (the
self-test reads the DOM, it takes no picture; WKWebView's fonts and the
clipboard there are unseen); whether a third column is what was meant
by "another layer on top"; the flags list's density; Alt+M as the way
into the layer, on a Swiss keyboard and in Safari and Firefox (only
Chrome was driven).

**Not verified or not done.**

- Safari, Firefox and the app's webview were not driven with real
  events; a touch screen not at all.
- No real object large enough to hit the decode budget was made by keys;
  the refusal's display was checked with the message put through the
  component, and a bad address through the real command.
- The 49G in algebraic mode shows its stack raw (12a's open point); the
  scenario ran in RPN.
- 39G and 40G were not booted for the explanation (same refusal path as
  the 38G).
- The HTTP control API does not serve `watchMemory` (it sends no events);
  its read endpoints are unchanged.
- Flag meanings: nothing for most of the 49G. Its Advanced User's Guide
  was read again in the OCR text: p. 2-1 says the full list is in the
  Pocket Guide, and the command reference names only -3, -103 and -105.
  Two ways on are in the wiki (`questions/hp49g-system-flags`): the
  Pocket Guide, or the calculator's own MODE FLAGS list. The word
  size and digit-count fields show their bits, not a decoded number (the
  guides do not give the bit order).

**Deviations from the plan.** None left for object text (see above). The flags panel has no
"grouped second word" for the 49G: its 128 system flags are one list by
topic plus the undocumented ones as cells, and its 128 user flags one
grid. A Node test gate was added, which the plan did not ask for.
