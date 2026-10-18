---
title: "A modern UI on the ROM: design study and prototype"
type: research
date: 2026-10-18
status: active
tags:
  - research
  - saturnus
  - ui
  - web
  - 49g
  - 48gx
---

# A modern UI on the ROM: design study and prototype

Owner's idea: keep the 48GX or 49G ROM as the engine, but put a completely
different UI on it. Modern look, touch on the phone, keyboard on the
desktop, high-resolution rendering, a command palette as the main
paradigm; maybe no keys at all, a terminal where commands are typed and
the whole stack is drawn, graphs and the display rendered above 131 × 64.
And the harder question underneath: does it still make sense to use the
old ROM, or should one build a new calculator, given MATLAB and
Mathematica?

This note takes a position, says what the current core and front-end
protocol already allow, sketches three UI directions, recommends one, and
reports what a throwaway prototype of it (`web/lab/`, branch
`explore/modern-ui`, not merged) showed against the real ROMs.

## The position, in short

- **The ROM is the product; a new calculator is not.** What saturnus has
  that nothing else has is HP's own RPL system running exactly, with the
  49G's CAS, units, the 48's programs and libraries, and the behaviour the
  manuals describe. A calculator written from scratch competes with
  MATLAB, Mathematica, SageMath, Julia, Desmos and every phone app, and
  loses on day one. A modern UI *on the ROM* competes with nothing: it is
  "the HP 48/49, usable in 2026".
- **The ROM's output is the problem, not its engine.** 131 × 64 pixels,
  four stack lines, six soft keys and the key matrix are what date it. The
  host already reads the stack, the variables and the flags straight out
  of RAM as typed objects, types text into the command line and runs it.
  That is enough to draw the whole stack ourselves and to stop showing
  keys for ordinary work.
- **Recommended backend: the 48GX for the first iterations, the 49G as
  the goal.** The 48GX is RPN from the first key, its typing engine is
  settled, every command has a reference entry and examples; the 49G is
  the one with the CAS, exact integers and symbolic matrices, but boots
  in algebraic mode, and the prototype hit two typing-engine timing
  problems on it (below). Build on the 48GX, keep the 49G running in the
  same code from the start, and switch the default once its sends are
  as reliable.
- **Recommended direction: the terminal-and-stack UI.** One command line
  that is the palette, the keyboard and the input at once; the stack as a
  scrolling column of typed, high-resolution objects, level 1 at the
  bottom; graphs as cards the ROM computes point by point; the ROM's own
  screen as a card that appears only when the ROM is in a form, a menu or
  an app. Phone and desktop differ only in where the keyboard comes from.
- **It is feasible now, without a core change,** for everything the
  prototype does; the gaps are in the object decoder (units and
  algebraics come as text only, programs holding unnamed ROM objects have
  no text), in the 49G's typing right after a boot, and in anything that
  needs the ROM's interactive screens (the Equation Writer, PLOT's
  interactive cursor, the SOLVE forms, MATRIX editor), which stay the ROM's
  own pixels.

## 1. The ROM as backend, a new calculator, or MATLAB and Mathematica

### What the ROM uniquely buys

- **RPL, exactly.** The stack discipline, `→` local variables, lists as
  programs, `IFERR`, the whole command set (the reference counts 500-800
  commands per model) with HP's edge-case behaviour: flags, display modes,
  `LASTARG`, unit conversion and the units table, the 48's and 49's number
  formatting. None of this is specified anywhere well enough to reimplement
  faithfully; the ROM *is* the specification.
- **The 49G's CAS**: symbolic integration and differentiation, exact
  integers and rationals, `SOLVE`, series, polynomials, modular arithmetic,
  symbolic matrices. A credible CAS is years of work; this one exists and
  is, within its limits, trusted by the people who still use it.
- **Decades of programs and libraries**: hpcalc.org's archive, user
  libraries, the owner's own programs, course material. Everything runs
  unchanged because the ROM runs it.
- **The community and the manuals.** The AUR, the user's guides, the
  comp.sys.hp48 lore: every behaviour has a documented answer, which is
  exactly what makes the clean-room emulator checkable.
- **Fidelity as the test oracle.** Every feature of the new UI can be
  checked against the ROM's own screen (what our stack rendering says must
  be what the LCD says), and the emulator against saturnng. A new
  calculator has no oracle but itself.

### What it costs

- **Output is 131 × 64 and four lines.** The ROM draws its stack, menus,
  forms and plots into that. Anything we render ourselves must be decoded
  from RAM, which the host does for the stack, the variables and the flags,
  but not for forms, menus, the Equation Writer, PLOT's picture or
  `PICT` beyond a GROB read. Interactive apps stay the ROM's pixels.
- **Emulated speed.** The 48GX runs at 4 MHz Saturn speed with pacing
  against the wall clock; `max` speed lets a computation run as fast as
  the host can (the prototype's 187-point `SIN(X)·X` evaluation took
  under a second of wall time at `max`, about 6 s of emulated time). Fine
  for interactive use; not a numerical engine. Typing into the command line
  is 4-10 characters per second of emulated time, run as fast as the host
  can, so a 150-character program costs well under a second of wall time.
- **Objects are reachable only by decoding memory**, and only when the
  ROM is between commands (the memory reads are refused while a send
  types, and are meaningless while the ROM is mid-command). The decoder
  (`saturnus-objects`) covers the user-visible types; see section 3 for
  what it does not give.
- **Twelve-digit BCD reals, 64-bit binaries, no Unicode, no files, no
  network.** Modern expectations the ROM cannot meet; the UI can only
  wrap them (copy and paste, history, export of a plot as SVG).
- **One calculator per model per browser tab, single-threaded, with the
  ROM's own notion of "busy".** A long computation blocks the stack read
  until it ends; the UI has to show that honestly (the prototype shows
  "Running…" and reads the stack when `memoryChanged` fires).

### Who the audience is

Not MATLAB's or Mathematica's. Those are for people who need a numerical
or symbolic *environment*: large data, plots with publication quality,
notebooks, packages. The audience for a modern UI on the ROM is:

1. People who learnt the HP 48/49 and still think in RPN, on a phone or a
   laptop that has no HP. They want the calculator, not a skin of it, and
   not a notebook.
2. People with programs and libraries for it (the owner, with a 48SX, a
   38G and a 49G on the desk).
3. The curious, for whom "the 1990 calculator, but readable and typed" is
   the pitch; a new calculator of our own has no pitch for them.

A new calculator from scratch would be a weaker Desmos with RPN; the
market for that is a hobby. **Position: keep the ROM, change what people
see and touch.**

## 2. 48GX or 49G as the backend

| | 48GX | 49G |
| --- | --- | --- |
| Entry mode at boot | RPN | Algebraic (flag -95 set); `CF(-95)` first, and the mode's ENTER leaves an echo (the parsed line and `NOVAL`) on the stack that must be dropped (`web/protocol.md`, "The user memory, written"; the prototype drops it by the stack depth before and after) |
| Numbers | 12-digit reals | reals, exact integers (`5`, not `5.`), rationals, the CAS's symbolic objects |
| CAS | none | yes, the reason to want the 49G |
| Typing engine (`run`, `insert`) | settled; 3.6-5.5 characters/s emulated | 7-10 characters/s emulated, but see the problems below |
| Reference (`web/commands.json`) | complete with examples | complete with examples (some `MENU n` menus unnamed) |
| Boot | "Try To Recover Memory?" answered by the host | the same, plus "Memory Clear" cleared; then seconds in SHUTDN |
| ROM | 512 KB, one image | 2 MB, flash banking (the core handles it), the state 2.6 MB |
| Where the UI falls back to the LCD | forms (SOLVE, PLOT, I/O), the MatrixWriter, the EquationWriter | the same, plus the 49G's CAS error boxes and its APPS |

Two things the prototype found on the 49G, both outside the UI:

- A `run` within about 3 seconds of a cold boot fails with "could not
  bring the keyboard to AlphaLocked (alpha false, alpha lock false, …)",
  and once that has happened the next send fails the same way until a
  pause. Waiting until the machine sleeps (`status.loop === "sleep"`) is
  not enough; a 3-second wait is. The prototype waits and retries once;
  the real fix is in the host's typing engine (read the keyboard state
  again after a short emulated delay, or refuse with "the calculator is
  still starting" so the page can wait).
- The plot program (a 130-character `« … → a b n « … FOR … NEXT n →LIST
  … » » EVAL` sent in one `run`) works on the 48GX and not on the 49G: the
  values land on the stack but so do stray objects (`*` as a command
  object, a nameless ROM object) and the line is left open in program
  entry mode. Most likely the 49G's program-entry mode (entered by `«`,
  which the typing engine notes it may leave the line in) or its `'`
  pair insertion inside the program changes how the text is read back.
  Not investigated further here.

**Recommendation:** develop on the 48GX; run the whole test matrix on both
from the first iteration; make the 49G the default once the two problems
above are fixed in the host (one iteration of their own). The UI code is
model-neutral: the object shapes, the reference index and the protocol
are the same for both.

## 3. What is feasible with the current core and API

The front-end protocol (`web/protocol.md`) already has everything the
prototype needed; no core or host change was made.

### The stack, rendered by us

`stack` returns every level as a typed object with the calculator's own
text (`saturnus_objects::described`, in the display mode the flags
select), and `memoryChanged` (at most every 250 ms) says when to read
again. The prototype reads the stack and the command line after each
event and draws every level; a stack of 14 levels reads in a few
milliseconds.

| Type | Decoded today | Rendered by the prototype as |
| --- | --- | --- |
| real, integer (49G), complex, binary | value and text | large tabular numerals, the exponent lighter, the base suffix as the ROM writes it |
| string, name, local name, character | value and text | tinted text |
| list | items, nested, each with text | chips that wrap, `{ }` as delimiters, cut at 60 items |
| array | dims and items, nested | a grid; symbolic 49G matrices come with the items' text |
| tagged | tag and object | tag as a badge before the object |
| unit | value and the unit's text (`12_m/s`) | number with the unit as a suffix |
| program | source text (indented by `web/shared/objects.js` `indentRpl`) | a pre with structure words, delimiters and known commands tinted |
| algebraic | source text with quotes | the expression (`^` as superscript, `*` as a dot) and a Plot button when it mentions X |
| command (ROM pointer, XLIB) | name from the ROM's tables, address, library and number | its name |
| graphic (GROB) | width, height, packed rows up to 2048 × 2048 | a canvas at an integer multiple of the device pixel (3 CSS px a pixel on a 131-wide GROB, 2 above that, 1 above 300) |
| directory, library, backup, code, anything else | prolog, kind, nibbles, hex | kind and size |

What is missing for a UI that wants to do more than show text:

- **Algebraics as trees.** An expression comes as the ROM's own text,
  not as a structure, so typesetting (fractions, roots, integrals as the
  EquationWriter draws them) means parsing the text ourselves. For the
  48's expressions that is a small grammar (functions, operators, names,
  numbers); for the 49G's CAS output (`∫`, `Σ`, `∂`, `|`, `where`) a
  bigger one. The decoder could instead walk the object's prolog
  structure and return a tree; `saturnus-objects` has the object parsed
  already (`decompile.rs` builds `Node`s with precedence for the text), so
  exposing the tree is a modest change.
- **Units as structure.** The same: text only (`m/s^2`), no decomposed
  unit, so no conversion UI without the ROM's `CONVERT`.
- **Programs holding ROM objects the tables do not name** have no text;
  the UI shows a placeholder. Libraries' commands are named only when the
  library is in the ROM's tables (the 49G's are; a user library's are not).
- **Long reals beyond f64** come as text; fine for display, not for a
  graph axis without parsing.
- **Nothing is read while a send types or a transfer runs**, so the stack
  view goes stale for the length of a long program's typing (under a
  second) and of any long computation (as it should).
- **No `eval` command** (reserved in the protocol): running text is
  `run`, which types it key by key and presses ENTER. It is fast enough
  (a program of 150 characters in under a second of wall time) and keeps
  the ROM's own parser as the authority, but it cannot type a control
  character, a text longer than 4096 characters, or anything while the
  ROM is not on its stack display. A real `eval` through the hidden
  Kermit transaction (`storeText` compiles any text on the calculator in
  about a quarter of a second) is the path for long programs: compile
  into a temporary variable, then `run` its name.

### Typed entry with completion

`web/commands.json` and `web/reference.js` give an index per model and a
ranked `search` (exact name, codes such as `\->LIST`, friendly spellings,
prefix, substring, long name, description, examples, the ROM's menus). The
prototype's line uses it for an inline suggestion list (Tab completes the
word at the end of the line, arrows choose, Enter sends the line as
typed). The palette's Enter rule ("run with no command line open, insert
into an open one") does not apply: the line *is* the command line, and
the ROM's own line is shown underneath only when the ROM opened one
itself (a program typed with the keys, an `EDIT`). The variables of the
current directory (`memoryTree`) can join the suggestions as they do in
the app's palette; the prototype left them out.

### Graphs at the screen's resolution

Two ways, both available now:

1. **Evaluate through the ROM at as many points as the canvas needs**, the
   prototype's way: a program stores X, evaluates the expression with
   `→NUM` at n points, leaves the list, the page reads it off the stack
   (`stack`, the list's items as reals) and drops it. 187 points of
   `SIN(X)·X` on the 48GX: about 6 s of emulated time, under a second of
   wall time at speed `max` (the prototype sets `max` for the plot and
   restores the speed after). The ROM's own numerics (12 digits, its
   error behaviour, units) are preserved. Costs: the ROM is busy for the
   length of it, `X` is stored and purged (a user's `X` would be
   overwritten; use a name that cannot clash, or the 49G's local
   evaluation), and an expression that errors at a point aborts the
   program, with the ROM's `LASTARG` behaviour leaving arguments on the
   stack (the prototype reports the error; a production version brackets
   each evaluation with `IFERR` and a depth check).
2. **Plot from the decoded expression ourselves** (parse the text, evaluate
   in JavaScript): instant and resolution-independent, but a second
   numerical engine with its own disagreements with the ROM. Acceptable
   for a quick preview, not as the answer.

The ROM's own PLOT (131 × 64 into `PICT`) stays available: `PICT RCL`
gives the GROB, which the page draws crisp at 3 × 3 device pixels a pixel
(the prototype does this for any GROB on the stack, including `LCD→`'s
screen capture). Interpolating the picture is not worth it; drawing the
function again at the display's resolution is.

### What must stay the ROM's own screen

- Forms (`SOLVE`, `PLOT`, `I/O`, `MODES`, `TIME`), the MatrixWriter, the
  EquationWriter, `CHOOSE` boxes, `INPUT` and `PROMPT` in programs, the
  49G's APPS, the Interactive stack, `EDIT`/`VISIT`, the CAT and VAR
  menus, and any user program that draws.
- Menus (the six soft keys) in general: they are the ROM's state (which
  menu, which page) and the only way into some functions. The UI can
  offer the same functions as commands (`MENU n` is itself a command,
  and the reference knows every menu's commands), so for *typing* users
  the menus are redundant; for *touching* users they are the fallback.

Fallback design: the page watches `commandLine` and the stack after
every `run` and every `memoryChanged`; when the ROM has a command line
open that the page did not type, or a `run` reports `closed: false`
without an error, or a key press was sent, the page shows the LCD card
with the six soft keys and (on demand) the whole keyboard as plain
labelled buttons from the host's `layout`. The card goes away when the
line closes and the stack is the ROM's display again. The prototype does
the first two; "the ROM is in an app that is not a command line" (a
form, PLOT's picture) is not detectable from RAM today: a host-side
"what does the display show" (the ROM's display-mode flags, or the
system's `DispStatus` word) is the missing read. Until then the Screen
button is the user's.

## 4. Three UI directions

### A. Terminal and stack (recommended; the prototype)

One column. The stack as a scrolling list of rendered objects with level
numbers in a gutter and the type on the right, level 1 at the bottom,
larger; the lab's results (graphs) as cards under it; a short log of what
was run with the ROM's error text in red; the command line at the bottom
with the suggestion list above it. The ROM's screen as a card on the side
(desktop) or above the line (phone) when needed.

- Desktop: everything is the keyboard. ⌘K focuses the line, Tab
  completes, ↑/↓ history, Enter runs, Enter on an empty line is ENTER
  (DUP). Shortcuts for the stack verbs (⌘⌫ DROP, ⌘D DUP, ⌘S SWAP) are
  the next step; a click on a level puts its text in the line (edit), a
  drag reorders (ROLL).
- Phone: the line sits on the on-screen keyboard
  (`interactive-widget=resizes-content`); the stack scrolls above; tap a
  level to select it and get its verbs (DROP, DUP, SWAP, Edit, Copy) as a
  row of buttons; the suggestion list is five rows, each with the stack
  effect under the name. The system keyboard types everything the ROM
  takes; `«`, `→`, `'` and `"` as a small strip over the keyboard.
- Why this one: it is what the ROM already is (a command line and a
  stack) with the two constraints removed (four lines, keys). It needs no
  new read from the host. It works keyboard-only, which is what the
  owner asked for on the desktop, and the phone version is the same page
  with the system keyboard.

### B. Canvas-first touch UI

The stack as cards on a surface, objects you can drag onto each other
(drop a number on a number: a row of `+ − × ÷` appears; drop a list on a
program: `MAP`), pinch a graph to re-range it (the ROM evaluates again),
two-finger scroll through the stack, long press for the object's verbs.
Desktop gets the same with the mouse and the keyboard for entry.

- Strong on the phone, weak on the desktop (dragging numbers around is
  slower than typing). Needs the object tree for algebraics to be worth
  it (a dragged `SIN(X)` must know it is a function). High design cost;
  nothing of it exists.

### C. Palette-driven, keyless, in the existing app

Keep the Calculator view as it is (skin, keys, LCD) and make the palette
the primary surface: ⌘K opens it over the calculator, the stack is shown
*in the palette* as rendered objects, commands run from it, and the skin
is only there behind. The phone version hides the keys by default and
shows the palette sheet.

- Smallest step from today (the palette already does the search, the
  Enter rule, the editor, the actions); the memory view already renders
  the stack as a list. But the result is still the old calculator with a
  drawer, not a new UI; the owner's "I don't expect another calculator UI
  on top" argues against it.

**Recommendation: A**, built as its own view (`#/lab` or a new app route)
next to Calculator and Files, sharing the hub so the same calculator is
shown in both (switch to Calculator when the ROM is in a form, come back
when it is on the stack). C's pieces (the palette's search and editor
mode) are reused inside A, not the other way round.

## 5. What the prototype does and does not do

`web/lab/index.html`, `lab.js`, `lab.css` (branch `explore/modern-ui`).
A separate document under `web/`, so the app is untouched; it reuses the
hub Worker (`web/shared/calc-hub.js`, joining as `tx` and `attach`ing to
the model's kept calculator, like Files), the tokens, `web/shared/objects.js`
and `graphic.js`, `web/reference.js` and `web/commands.json`, and
`web/tx/kept-roms.js` to list the ROMs the app keeps.

Does:

- Boots a ROM file (or a ROM the app keeps) through the hub; the host
  answers the ROM's boot questions; a 49G in algebraic mode is switched to
  RPN by the lab (`CF(-95)`, the echo dropped by stack depth).
- One command line: suggestions from the reference while typing (name,
  stack effect, long name and description), Tab completion, history,
  Enter runs the text through `run`; the ROM's error is shown in red in
  the log and in the status; an empty Enter is ENTER.
- The whole stack from `stack` after every `memoryChanged`, every type
  rendered as in the table above; levels that changed light up; GROBs
  drawn crisp; a Plot button on algebraics in X.
- `:plot 'SIN(X)*X' -6.2832 6.2832`: the ROM evaluates at one point per
  3 CSS pixels of the card's width (147-187 points), the card is drawn at
  the device's resolution with axes; the list is dropped afterwards.
- The ROM's screen as a card (Screen, or `:screen`), at an integer
  multiple of the device pixel, with the annunciators as text, six soft
  keys, and on demand the whole keyboard as plain buttons from `layout`;
  shown by itself when the ROM has a command line open that the lab did
  not type.
- Phone layout at 390 px: the line on the keyboard, the stack above, the
  screen card with soft keys and the key grid; desktop at 1280 px: the
  stack column centred, the screen card at the right.
- Dark mode and the app's theme setting, through the shared tokens.

Does not:

- Edit a level in place, reorder levels, or act on a selected level
  (DROP, DUP, SWAP as buttons); no variables view, no flags, no
  directory; no copy and paste of objects; no editor mode for programs.
- Detect that the ROM is in a form or an app; the Screen button is manual
  for those. No menu labels read from the ROM (the soft keys are F1-F6).
- Handle the 49G's long program sends (section 2) or errors inside a plot
  gracefully.
- Any tests. The screenshot script lives in the session's scratch area
  and drives the page through `web/test/chrome.mjs`.

Measured on the owner's Mac, headless Chrome, 48GX ROM R: boot to a
readable stack about 4 s; each `run` of a short line 100-300 ms wall time;
the 187-point plot 0.8 s; the stack read and redraw after a change under
20 ms. The 49G: boot about 6 s, a 3-second wait before the first send.

## 6. A phased plan, if pursued

1. **Host fixes the study needs** (one iteration, Rust): the 49G's first
   send after a boot (typing engine reads the keyboard state again, or
   refuses with a message the page can wait on); the 49G's long program
   send; an `eval` command (compile through the hidden Kermit transaction,
   then run; `storeText` already does the compile) so programs of any
   length run without typing; a "display state" read (is the ROM on its
   stack display, in a command line, in a form or an app) so the page
   knows when to show the LCD. Risk: the display state has no documented
   RAM word; it may need a heuristic from the display's contents.
2. **The view** (one iteration, web): `#/lab` as a fourth route of the app
   on the shared calculator, the prototype's column rebuilt on the app's
   store and components (the palette's search, its editor mode for
   multi-line programs, the toast and confirm), level actions (select,
   DROP/DUP/SWAP/ROLL, Edit through `storeText`, Copy), variables as
   suggestions, the LCD card driven by the display state. Screenshot
   tests at 1280 and 390 in the fast suite. Risk: two views of one
   calculator typing at once; the hub already serialises sends and
   refuses the second, so the view must show "Calculator is typing".
3. **Objects as structure** (one iteration, Rust + web): the algebraic and
   unit trees from `saturnus-objects`' parsed nodes, typeset in the view
   (fractions, roots, powers, integrals as `∫`); matrices with proper
   alignment; the 49G's CAS objects. Risk: the 49G's CAS output has
   object kinds the decoder names but does not describe (`unknown` with a
   prolog); each needs a case.
4. **Graphs** (one iteration): `:plot` as a proper command with ranges
   picked from the expression, `IFERR` per point, pinch and drag to
   re-range (re-evaluated by the ROM), parametric and polar through the
   ROM's `PTYPE`, export as SVG and PNG; the ROM's own `PICT` beside it.
5. **Phone polish and the keyless default** (one iteration): the level
   verbs as a row, the character strip over the keyboard, the screen card
   as a sheet, the app's start on `#/lab` as an option.

Order 1 and 2 could swap: the view can ship on the 48GX with the Screen
button manual, and the host work follows. Four to five iterations in all
for something usable; the 49G as default depends on 1.

## 7. Open questions for the owner

1. Is "RPN first" right? The lab is an RPN terminal and switches the 49G
   to RPN. An algebraic entry mode (type `SIN(X)^2+1` and get a value)
   would be a different line, with the 49G's own algebraic mode or our
   own translation to RPN.
2. One app with a fourth view, or a second app? The plan assumes a route
   in the one app (owner's one-app decision of 2026-10-17); the prototype
   is a separate document only to keep the study isolated.
3. How much of the memory view belongs in the new UI (variables, flags,
   the reference)? The terminal direction suggests variables as
   suggestions and `:vars`, not a tree.
4. The 49G's first-send and long-send problems: fix in the host now
   (iteration 1 above), or stay on the 48GX for the study's next step?
5. Should the plot evaluate through the ROM only (faithful, slower, busy
   calculator) or also in JavaScript for live re-ranging (instant,
   second engine)? The note recommends the ROM only.
6. Naming: "lab" is the study's name; the view needs a real one
   ("Terminal", "Stack", "Workbench").
