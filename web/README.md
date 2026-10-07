# saturnus web UI

A static page around the WebAssembly build of the saturnus core
(`crates/saturnus-web`). Plain HTML, CSS and ES modules; no framework or
bundler. The same page is the front end of the desktop app
(`crates/saturnus-tauri`, see the main README).

## Structure

The page never touches the emulator directly. It talks to a **backend**
in the command/event protocol of [`protocol.md`](protocol.md):

- `backend.js`: `WorkerBackend` runs the wasm core in a Web Worker
  (`worker.js`); `TauriBackend` sends the same commands to the desktop
  app's native core (`invoke`) and hears the same events (`listen`). The
  page picks Tauri when `window.__TAURI__` exists. Both have the same
  methods; the differences (a file input or a native file dialog for the
  ROM, IndexedDB or files for states) stay inside them.
- The host owns the machine, paces it against the wall clock and pushes
  events: `frame` (the packed LCD, annunciators, contrast) only when the
  display changed, `keys` when the keys down changed, `status` when the
  model, Run/Pause, a halt, the speed or the run loop changed.
- `store.js`: one `EventTarget` with the page's state, fed by the
  backend's events and the controls (one-way: event, store, components).
- `components/`: framework-free Web Components in the light DOM
  (`display: contents`, so `style.css` lays them out as before):
  `<sat-calculator>` (skin or button grid, LCD, pointer and computer
  keyboard), `<sat-controls>` (the panel's controls and status line),
  `<sat-about>` (the About panel), `<sat-explorer>` (the side layer:
  the memory view and the Commands tab), `<sat-palette>` (the command
  palette). They render from the store and act only through the backend.
- `romstore.js`: the Worker's ROM slots over IndexedDB (`romSlots`,
  `bootModel`, `chooseRom`, `forgetRom`, `romSettings`), identifying and
  assigning with the wasm core's `identify_rom` and `plan_roms`, the same
  rules the desktop app runs natively; tested by `web/test/` with a fake
  store, including a store that refuses.
- `memory.js`: the memory view's reads. While the layer is open it asks
  the host to watch the user memory and reads the tree, the stack and the
  flags into the store after each `memoryChanged`; it never polls.
  `objects.js`: the text forms and previews of calculator objects and the
  rows of the flags panel, pure functions tested by `web/test/`
  (`just web-test`, Node's test runner, no dependencies).
- `flags.json`: what each system flag means per model, generated from the
  hardware wiki by `scripts/flags-json.py` (`just flags`), as `about.json`
  is by `scripts/about-json.py`.
- `reference.js`: the command reference in the page: the lookup rules of
  `saturnus ref`, the palette's ranking, its Enter rule and the ROM menu
  tree, pure functions over `commands.json`; `palette.js`: the palette's
  model without the DOM (`PaletteModel`: query, rows, selection, the
  command line's state, what choosing sends) and the `ReferenceLoader`
  that fetches the data once; `components/entry-view.js`: one command's
  entry as DOM, shared by the palette and the Commands tab. Tested by
  `web/test/reference.test.mjs` with a fake backend.
- `commands.json` (541 KB, 101 KB compressed): the reference data folded
  from `data/commands/` by `scripts/commands-json.py` (`just commands`;
  `--check` in `just lint` and CI keeps it current). It lives in `web/`
  like `about.json` and `flags.json`, so `web/site.sh` ships it with the
  site and the desktop app embeds it with the rest of the page
  (`frontendDist`); it is fetched only when the palette or the Commands
  tab first opens.
- `app.js`: the composition root: picks the backend, connects it to the
  store and the components, and keeps the page chrome (side panel, sheet,
  fullscreen) and the preferences.
- The key queue (hold times, gaps, typed letters through alpha and the
  shifts) is Rust in `crates/saturnus-web/src/host.rs`, shared by the
  Worker (compiled to wasm) and the desktop app (native), so both hosts
  time keys identically in emulated time.

## Build and serve

```sh
cargo install wasm-pack          # once; needs the wasm32-unknown-unknown target
./build.sh                       # wasm-pack build --target web into ./pkg (gitignored)
python3 -m http.server 4860      # any static file server works
```

Open http://127.0.0.1:4860/. Browsers do not load ES modules or wasm from
`file://`, so a server is needed.

## Layout

The calculator is the page: it fills the window's height, or its width on a
narrow screen. On a wide window the controls sit in a side panel on the
left, which the `‹` button hides (a `☰` button brings it back; the choice is
remembered). Below 760 px the panel becomes a sheet that drops down from a
compact top bar, which also carries the fullscreen button.

**Fullscreen** shows the calculator alone, on a dark background, using the
browser's Fullscreen API; the `✕` in the corner or the panel's button leaves
it. Escape leaves fullscreen in every browser; in Chromium the page locks
Escape so it keeps working as ON, and holding Escape leaves instead. The
`` ` `` key is ON everywhere.

## Use

1. Choose the model, then its ROM file. The ROM is read in the page and
   kept in this browser's IndexedDB, per model, so you do not have to pick
   it again: selecting a model boots its ROM, and the last model boots
   when the page opens (a setting under "ROMs of every model"). Several
   files may be chosen at once, or dropped on the page; each is
   recognised by its content and goes to its model's slot (see
   `protocol.md`, "ROM slots"); one that could be the ROM of more than one
   model is offered with a button, an unknown one is named and left out.
   A ROM is never uploaded. "Forget ROMs" deletes them from the browser.
2. Keys: click or tap the keys of the drawn calculator, or use the keyboard
   (below).
3. Run/Pause stops emulated time. Reset is the hardware reset (RAM kept).
4. Save state stores the machine in IndexedDB, one slot per model. After a
   reload the model's ROM boots again; press Load state. A state only loads
   with the ROM it was saved from.
5. Speed: 1×, 2×, 4× or Max. Above 1× the calculator's clock runs fast too;
   at Max the page runs as much emulated time per animation frame as fits
   in about 11 ms of wall time, at most one emulated second per frame, so
   the page stays responsive. The setting is remembered.

Stored in the browser: the model (localStorage `saturnus.model`), the view
(`saturnus.view`, `skin` or `grid`), the speed (`saturnus.speed`), whether
the side panel is hidden (`saturnus.panel`), the saved states (IndexedDB
database `saturnus`, store `states`) and the ROMs (IndexedDB database
`saturnus-roms`: store `slots` with each model's file name, SHA-256 and
revision and the last model, store `images` with the bytes by SHA-256, so
the 39G and 40G share one copy). Forget ROMs empties both stores but the
settings record; saved states stay. Where the browser refuses to store (a
blocked or full storage) the panel says so, and the ROMs last until the
page is closed.

## Keyboard

| Keys | Calculator |
| --- | --- |
| `a`–`z`, `A`–`Z` | the letter, through the model's alpha mode; lowercase through its shift (the 39G/40G space is alpha + plus). Not on the 42S, which types letters from its ALPHA menus: its skin has no typing data |
| `Tab` | α (one press for the next key; twice for alpha lock on the 48 and 49G, where a third press unlocks; the 38G, 39G and 40G cancel on the second press) |
| `[`, `]` | left and right shift; `[` is the only shift on the 38G, 39G and 40G |
| `Esc`, `` ` `` | ON (see Fullscreen above) |
| `F1`–`F6` | the six menu keys (the top row Σ+ to XEQ on the 42S) |
| `0`–`9`, `.`/`,`, `+ - * /`, `^`, `'` | as printed |
| `Enter`, `Space`, `Backspace`, `Delete`, arrows | ENTER, SPC (shift + 2 on the 38G, which has no SPC key), ⬅, DEL, the cursor keys |

A typed letter is expanded into key presses when its turn in the key queue
comes: the alpha key unless the alpha annunciator is already on, the shift
for a lowercase letter (after alpha on the 48 and 49G, before it on the
aplet models), then the letter's key. Right after a letter the page pressed
alpha for, alpha is known to be off again (one-shot), so a run of letters
needs no waiting; otherwise the page waits for the ROM to go idle before it
reads the annunciator, which the 48SX ROM blinks while redrawing.

### Paste

Pasting (Ctrl+V, Cmd+V) while no text field or dialog has the focus
types the clipboard's text into the calculator's command line through the
protocol's `insert` (48SX, 48GX and 49G): any character the model can
type, by key presses, at unlimited speed in emulated time; `\r\n` becomes
the calculator's newline. While more than a few characters are typed the
screen keeps its last frame, dimmed, and the status line says "typing…".
Text with a character the model cannot type is refused, nothing pressed,
with the reason in the status line. The desktop app runs the same
handler; whether its webview delivers a paste event with no text field
focused is not checked yet (it is in Chrome).

## Command palette

**Cmd+K** (Ctrl+K; both work everywhere) or the **Commands** button over
the calculator opens the command palette: one input over the calculator,
which stays visible behind a dimmed backdrop. It is the command reference,
the way to send commands and text to the calculator and the entry to the
app's actions at once. While it is open the keys are its own; Escape
closes it and gives them back to the calculator.

Typing suggests, ranked: the running model's commands (an exact name
first, then names the query begins, then names containing it, then
descriptions and example text), your variables of the current directory
and its parents (read once when the palette opens, nearest directory
first), the ROM's menus (`PL` on a 48SX, which has no PLOT command, offers
its PLOT menu), the app's actions (ROM, Run/Pause, Reset, Save and Load
state, the speeds, the side layer's tabs, fullscreen, the view, the
panel, About). The lookup rules are `saturnus ref`'s: case does not
matter, the calculator's ASCII codes (`\->LIST`, `\.S`) and friendly
spellings (`->LIST`, `SIGMA+`) find the name, and a spelling several
commands share lists them all (`INT` on the 49G: INT, then ∫). Each
command row shows its stack effect and description; the selected row's
full entry is beside the list (under it on a narrow window): stack
effect, description, the models that have it, the ROM's menu and the
manual's key for it, the examples generated on the emulator as input →
result with **Try it**, and links into the manuals' pages.

Choosing: arrows and Enter, a click, or the number shortcuts on the first
nine rows (Ctrl+1–9 in browsers, Cmd+1–9 in the desktop app on a Mac;
each row shows its hint). **Enter mimics the calculator's keys**: with no
command line open the command is typed and executed; with one open its
name is inserted at the cursor (with the spaces the calculator would put
around it); Cmd/Ctrl+Enter does the opposite, and the footer says which
is which. A variable inserts its name (Cmd/Ctrl+Enter evaluates it).
Text that is not a single name (`13 4 ^`, `« 1 2 + »`, `30`) is offered
as "send as typed", first; a bare word that names nothing exactly can
still be sent, as the last row. Everything goes through the protocol's
`insert` and `run` (see Paste above for how typing works and when the
screen freezes). When a `run` leaves the command line open with an error
the palette stays open and shows the calculator's message.

Where it cannot do everything it says so under the input: no ROM running
(the selected model's reference can be read, nothing sent), a model
without a reference or a command line (38G, 39G, 40G, 42S: app actions
only), and the 49G in algebraic mode, where the RPN text of the examples
and command names does not parse (the action "Switch the HP 49G to RPN
mode" runs `CF(-95)`).

The **Commands** tab of the side layer is the same reference for
reading: the ROM's menus as a tree (roots in the order of the keys that
open them, `MENU n` for a menu no key opens), then the commands no menu
offers grouped by the key a manual names or our own group; the commands
of the selected menu with their stack effects; the entry below, with
"Try it". A menu row chosen in the palette opens the tab at that menu.
The About panel links the full manuals.

## Memory view

The **Memory** button (top right of the calculator; in the top bar on a
narrow screen) opens a layer with three tabs on the calculator's user
memory, read live and never written (48SX, 48GX, 49G; on the other models
the layer says why it has nothing to show):

- **Variables**: the directory tree of HOME on the left, the variables of
  one directory on the right (name, type, size and checksum as the
  calculator's BYTES gives them, newest first), the selected object below:
  a number or a name as text, a string, a list by element, a matrix as a
  grid, a directory as its listing; large objects are cut with a count.
  "Copy text" copies the object's text form. Browsing here is navigation
  in the page, not `cd` on the calculator: the calculator's own directory
  is marked "current", the view follows it until you browse elsewhere,
  and "Show it" returns. "Find a variable" searches every directory.
- **Stack**: the levels as the calculator has them, level 1 at the bottom,
  the selected level in full below.
- **Flags**: the system flags by topic with their current state and what
  that state means, the flags without a documented meaning and the user
  flags as cells. Read-only. The meanings come from `flags.json`; where
  the guides do not establish one, the panel says so (the 49G's guides
  describe only a few of its flags).

A program is shown as its text in indented lines (one structure word per
line, bodies one level in), an algebraic expression and a unit as the
calculator writes them, a list with its commands by name; "Copy text"
copies the calculator's own text, not the indented layout. Every text
shown or copied is the host's (`text` on each object, from the
decompiler in `saturnus-objects` with the ROM's own command names, in
the calculator's display mode); the page formats no number and no object
itself, it only lays the texts out. Objects that have no text (a graphic, a
library, a backup; a program or expression holding something the ROM's
tables do not name) show type, size and checksum with a sentence saying
so, an unknown object's nibbles behind a disclosure.

Layout: from 1000 px the layer is a third column beside the calculator
(400 to 640 px wide; the controls panel stays and can be hidden); below
that it lies over the calculator with a "‹ Calculator" button to go back;
below 760 px the top bar's Memory button toggles it. The choice and the
tab are remembered.

Keyboard: typing goes to the calculator unless the focus is inside the
layer. A mouse click on a row, a tab or a button does not take the focus;
a click into a search field, Tab from there, or **Alt+M** does (Alt+M
opens the layer if needed and moves the keys back when pressed again). A
line beside the tabs and a bar along the layer's edge say where the keys
go; Escape empties a search field, then returns the keys to the
calculator. Inside: arrows in the tabs, the tree (left and right fold),
the list (Enter opens a directory, Backspace goes up) and the stack.

Updates: the host looks at the memory when the calculator has run and
waits for a key again, and tells the page only if it changed (see
`protocol.md`, Pacing); the page then reads once. The view follows the
calculator within about 10 ms of the moment it goes idle, which on the
48SX is 0.6 to 0.9 s after a key is released (the ROM's own time) and
0.2 to 0.35 s on the 49G. While the calculator computes, the view keeps
its last state and says so.

The bindings underneath (`Emulator`, 48SX, 48GX, 49G; the 38G, 39G, 40G
and 42S throw, `memory_refusal()` gives the reason):

- `memory_tree()`: `{path, variables}`, the current directory and HOME's
  tree, each variable `{name, type, size, checksum, address, variables?}`
  (newest first; type, size and checksum as the calculator lists them).
- `stack()`: the typed levels, level 1 first (the shapes in
  `protocol.md`, "Typed objects"): programs, algebraics and units with the
  calculator's own text, commands with their names, read from the loaded
  ROM's command tables on the first call (a few tens of milliseconds on
  the 49G) and in the display mode the flags select.
- `flags()`: `{system, user, set}`, words as 16 hex digits.
- `object_at(address)`: one variable's typed value, the same way.
- `memory_changes()`: a counter (16 hex digits); poll it, for example once
  per frame, and re-read only when it moves.

Before the ROM has set up memory (right after power-on, or with no HOME
yet) the calls throw. In the protocol they are the read commands
`memoryTree`, `stack`, `flags` and `objectAt`, with `watchMemory` and the
`memoryChanged` event (`protocol.md`).

## Skins

The calculator is drawn as an SVG skin per model: 48SX, 48GX, 38G, 49G,
39G (the 40G shows the 39G drawing with its own name) and 42S. The 42S skin
was measured from photographs of the owner's calculator, not a manual
figure, and its LCD is 131x16 with seven annunciators. `annunciators()`
returns the same ten keys on every model (the 48's six, then `updown`,
`battery`, `g`, `rad`), so its shape is stable; on the 48 family the four
42S-only ones are always false. Before a ROM is loaded the canvas takes
its row count from the selected model's skin (`lcdRows`). The "Drawn
calculator" box switches between the skin and the plain button grid; both
press the same keys, by name, with the same timing, and the computer
keyboard works in both.

- **Data.** Each skin is Rust data in `crates/saturnus-web/src/skins/`
  (one file per model), handed to the page as JSON by `skin(model)` and
  `Emulator.skin()`, with the model's letter map and typing rules. Unit
  tests check that every key of the model's matrix is drawn exactly once,
  that keys do not overlap, that the alpha letters agree with the ROM and
  the wiki, that every model but the 42S types all 26 letters, that the
  six softkey labels the ROM draws sit over the six menu keys (on the 42S
  within half a key pitch: its display is narrower than its key row), and
  that the case ends shortly below the bottom key row.
- **Measured.** The 48SX, 38G and 49G were measured from the owner's
  straight-on photographs of his own units (case, key wells and caps, the
  plates and zones of the case, colours); the 48GX shares the 48SX's
  mould and takes its geometry. The 39G/40G, and the labels of every
  model, come from the keyboard line drawings in HP's manuals, rendered as
  images and measured in pixels. The unit is 1/100 of the menu-key pitch;
  the origin is the case's top-left corner.
- **Wells.** On the 48 and the 38G every cap sits in a dark recessed well
  a little larger than the cap (widest around the 48's light menu keys);
  the 49G's keys have a black outline. A key's rectangle is its cap; the
  well's margin is part of the cap's data.
- **Display window.** The window is the LCD's active area (131 x 72 pixels
  at square pixels, the annunciator strip included). Every ROM draws the
  six menu labels at a 22-pixel pitch from column 0 (wiki:
  hardware/display "Menu labels"), so the window is 5.95 menu-key pitches
  wide and placed so label *i* is centred over menu key *i*; the bezel
  around it follows.
- **Read off photographs.** Case, key and label colours, toned down from
  the photographs' exposure. Nothing from any figure or photograph is
  reproduced or stored: the page draws rounded rectangles, trapezoids and
  text.
- **Drawn freely.** The relief: three gradients defined once in the SVG
  (a light falling on each cap, a rim lit above and shaded below, a light
  on the case), a shadow under each cap; a pressed key moves down onto its
  shadow and darkens. Fonts, the saturnus logo where the HP logo was, the
  model name as plain text.

Which figure and photo each model comes from, and what is inferred, is in
the comment at the top of its file.

The skin scales to fill the stage's height (or width); it then shrinks by
up to 8% so that each LCD pixel is a whole number of device pixels and the
display stays crisp (below two device pixels per LCD pixel it is not
snapped). The canvas renders at the device pixel ratio.

The skin keeps the calculator's colours (case, keys, LCD) in dark mode;
only the page around it follows the colour scheme.

## Timing and idle behaviour

While the calculator computes, each animation frame runs the wall time
since the previous frame (times the speed; capped at 100 ms, so a hidden
tab does not catch up) in 10 ms slices. When the CPU sleeps in SHUTDN with
no key queued, the page stops animating: `Emulator.idle_ms()` says how long
the ROM sleeps until its next timer event, and the page sets one timer for
that moment instead of running frames. Nothing is drawn or written to the
DOM meanwhile, so an idle calculator costs no CPU. A key press or the timer
wakes it: the page first runs the emulated time that passed (cheap while
the CPU sleeps; it stops early if the ROM wakes), so the calculator's clock
keeps time, then animates again. If the ROM is still asleep after a timer
event (the 48 wakes for nothing every half second), the page sleeps on
without a frame. No counters are updated per frame; the status line changes
only on events.

Keys: every press is held for at least 60 ms of emulated time. A queued
press starts 30 ms after the previous release and once the ROM has gone
idle again (the 48SX ROM drops a key pressed while it is still handling the
last one, 70-230 ms); while the ROM stays busy, as in a running program, a
queued press waits at most 300 ms. A key you keep holding (ON for a chord)
does not block the next press.

All of this runs in the Worker (`worker.js`), not on the page's thread:
"animation frame" above is a pass on a ~60 Hz timer, which stops while the
page is hidden as an animation frame would (the page sends `visibility`);
the wake timer keeps running. The page draws a `frame` event on its next
animation frame. The desktop app follows the same rules on its machine
thread (`crates/saturnus-tauri/src/runner.rs`, with the CLI's wall-clock
`Pacer` while busy).

`window.saturnus` exposes the backend and the store, `screenText()` (the
LCD as `#` and `.` lines), `loop` (`frame`, `sleep` or `stopped`), the
speed setting, `stats()` (the host's cycles, emulated ms, busy wall time,
passes and wakes, and the emulated time it owes to the wall clock) and
`skinKey(name)` for debugging and automated checks.

## About

"About saturnus and its sources" in the panel opens `<sat-about>`: the
project statement (clean room, MIT, AI notice, no ROMs, not affiliated
with HP) and every source the emulator was built from, read from
`about.json`. That file is generated from the hardware wiki's source pages
(`~/devel/hp-literature`, outside this repository) by
`scripts/about-json.py` (or `just about`), which reads their frontmatter
through `hyalo`: title, authors, year, URL or archive location, and the
wiki pages that cite each source. Rerun it after the wiki gains a source;
the JSON is committed.
