# Changelog

All notable changes to saturnus. The crates (`saturnus`,
`saturnus-objects`, `saturnus-host`, `saturnus-drive`, `saturnus-cli`),
the CLI archives, the desktop app and the web page share one version.

## 0.1.1 (unreleased)

- **Theme: System, Light or Dark**: a choice in the panel (and the
  command palette) that fixes the page's colours, or lets them follow
  the device as before; kept, applied before the page shows, and
  followed by the desktop app's title bar.
- **Buttons that cannot act are easier to read**, in both themes: their
  text is grey at about 4:1 instead of faded to under 3:1.
- **A file the 49G refuses as a circular reference** (`test.txt` holding
  `test`) is explained in plain words; the desktop app's ROM download
  question has a button that opens the file's hpcalc.org page.
- **The command line as a .deb and an .rpm** (Linux x86_64) on the
  releases page, `saturnus-v0.1.1-x86_64-linux.deb` and `.rpm`: the
  package `saturnus-cli`, installing `/usr/bin/saturnus` next to the
  desktop app's package `saturnus`. The binary is statically linked
  (musl), so any glibc will do.
  desktop app's package `saturnus`.
- **Cmd/Ctrl+E follows the keys**: with the memory view in use it edits
  its selection; after a click on the calculator or a key typed to it,
  the calculator's command line or stack level 1. A click on the
  calculator gives it the keys back, and while the memory view has them
  its indicator offers "Give back (Esc)".
- **Ctrl+click works in Firefox on macOS**, which sends it as a
  right-click without a pointer press; the shift glow is fainter and
  closer to the labels.
- **Plainer page texts**: the stage button is Search, the memory view's
  tab Reference; the palette offers "Change the ROM…" after the
  calculator's actions while it runs; greyed-out buttons say why; models
  are listed 48SX, 48GX, 49G, 38G, 39G, 40G, 42S.
- **Quick clicks are no longer lost**: a key pressed soon after another
  waits for the ROM as long as that model's ROM may stay busy after a
  key (850 ms on the 48SX, 600 on the 48GX, 550 on the 49G, 1100 on the
  38G; 300 as before on the 39G, 40G and 42S). The 48SX dropped a key in
  that time, so two quick Ctrl+clicks on √x could give √x instead of x².
- **Clearer help, messages and docs**: plainer wording in the CLI's help
  and errors, the desktop app's dialogs and messages, and the READMEs.
- **Memory view actions**: each preview has one main button (Edit, Open
  or Make current) and a "⋯" menu with the rest; right-click, Shift+F10,
  F2 and Delete work on rows and tree nodes. HOME and the directory shown
  in the tree get the same actions, Make current included. "Store file"
  and "New directory" are one "New" menu.
- **The divider between the directory tree and the list can be
  dragged**, and its width is remembered.
- **Shift-click**: with a mouse, Ctrl+click on a key is its left-shifted
  function and Option/Alt+click its right-shifted one (one-shift models:
  their shift); the shift is pressed only if it is not on when the key's
  turn comes. Holding Ctrl or Option/Alt lights the labels it reaches.
- **Screen images**: copy the calculator's display to the clipboard as a
  PNG, or save it as a file, annunciators included, at 4× with hard
  pixel edges; in the LCD's colours or black on white (a choice in the
  panel). Copy screen and Save screen in the panel, in the command
  palette and on keys (Alt+Shift+C, Alt+Shift+S), and a menu on the
  display (right-click, or a long press on a phone). Where a browser
  cannot copy images, the image is saved instead.
- **About names the release and the build**: "saturnus 0.1.0, build
  3f2a…" on the site (the service worker's build), "desktop app" in the
  app, so a deployment can be checked.
- **The no-ROM message fits every display**: on small displays (the
  42S's strip, a phone on its side) it gets smaller print, then its
  first sentence, then "No ROM" beside the button; a tap or click on it
  shows the whole message.
- **The 49G's memory writes work in algebraic mode**, the mode it starts
  in: storing a file, fetching, renaming, purging, creating a directory,
  making one current and saving the editor's text no longer ask for RPN
  mode first. The write switches to RPN for its own steps and back to
  algebraic mode afterwards, also when it fails or is stopped; the stack
  and the screen are as they were. This removes 0.1.0's known limit "On
  the 49G, writes other than flags need RPN mode (flag -95 clear)".
- **A write stopped halfway leaves the calculator at its stack**: a line
  typed in part is cancelled, and a server that kept running (the 49G's
  goes on serving when ON ends a transaction) is ended with ON again.

- **A first boot lands on an empty stack**: the page and the desktop app
  answer the 48SX's, 48GX's and 49G's "Try To Recover Memory?" NO by
  themselves (and dismiss the 49G's "Memory Clear"), so typing and the
  memory view work at once.
- **Plainer messages**: the palette says "The calculator is not ready
  for typing…" instead of an internal error; status messages are short
  sentences, an outcome goes after six seconds and an error with the
  next click or key; no browser error text for fullscreen.
- **Forget ROMs… asks first.**
- **The offer to keep a ROM on the device** waits until the calculator
  is idle and sits in the panel under the ROM, not over the keys.

## 0.1.0 (2026-10-09)

The first public release.

- **Seven calculators**: HP 48SX, 48GX, 49G, 38G, 39G, 40G and 42S,
  emulated from the Saturn CPU up (bus and memory-mapped modules,
  display, keyboard, timers, serial port, card ports, the 49G's flash).
  The 48SX, 48GX and 49G match the saturnng emulator's screens pixel for
  pixel in the differential scenarios and run at real-hardware speed
  within a few percent on the HP Museum summation benchmark.
- **`saturnus` command line** (`cargo install saturnus-cli`, archives,
  Homebrew, Scoop): run a ROM headless with key scripts, screen dumps
  (text or PNG) and saved states; serve the serial port for Kermit
  clients and an HTTP + JSON control API on 127.0.0.1 with `saturnus
  ctl` to drive it; a built-in command reference with examples run on
  the emulator (`saturnus ref`); a disassembler; `saturnus rom fetch` for
  the ROM images hpcalc.org offers for emulators, checked by size and
  SHA-256.
- **Web page** (<https://ractive.ch/saturnus/>): the core in
  WebAssembly with drawn calculator skins (lit cases and raised keys;
  the 39G and 40G in the colours of an HP 39g+), saved states, and ROMs
  kept in the browser. Nothing is uploaded. An empty ROM slot links to
  the hpcalc.org page of its image. The page can be installed on a
  phone (Add to Home Screen) and works offline after the first visit;
  when a new version is out while it is in use, it offers a Reload.
  After a ROM is kept,
  the page asks once whether the browser may store it permanently.
  Every view fits a phone screen. Fullscreen on a phone fills the screen
  with the display and keys (a search icon and a swipe down on the
  display open the palette); with a mouse it shows the whole calculator.
- **The calculator keeps its state** across reloads in the page and
  restarts of the app: it is saved a few seconds after the last change
  and when the page is hidden. "Start fresh" in the palette cold-boots
  with an empty memory.
- **Memory view** (48SX, 48GX, 49G): the calculator's variables,
  directories, stack and flags, read live from RAM. It also writes,
  through the calculator's own Kermit server, out of sight: store a file
  (button, or drop it on the view or a directory), save a variable as a
  file, rename, purge, make a directory current, create a new directory,
  set and clear flags. The stack and the screen are left as they were,
  and so is the current directory unless the write changes it. The same writes in the CLI: `saturnus ctl
  store`, `fetch`, `rename`, `purge`, `mkdir`, `cd`, `flag` and `POST
  /v1/memory`.
- **Object editor** in the palette (48SX, 48GX, 49G): edit a variable, a
  stack level or the command line being typed ("Edit line") as RPL text
  with highlighting, bracket matching, indentation and completion.
  Cmd/Ctrl+S saves and closes the editor; the calculator compiles the
  text, so a syntax error is its own message and nothing in the text
  runs. Cmd/Ctrl+E opens the editor on the selected variable or stack
  level 1. CLI: `saturnus ctl text`.
- **Typing into the command line** (48SX, 48GX, 49G): text becomes key
  presses, from a paste in the page and app, the command palette
  (Cmd/Ctrl+K: commands by name with their stack effects, the app's
  actions), `saturnus ctl type [--run|--replace]` and `POST /v1/type`;
  `ctl cmdline` reads the line being edited.
- **Keyboard shortcuts** dialog (Alt+K, the panel or the palette): ON,
  α, both shifts and the app's actions can be bound to other keys; the
  bindings follow the physical key, so they work with any keyboard
  layout. The side panel and the memory and commands layer can be
  resized. The Commands tab groups the ROM's menus under the manuals'
  categories.
- A file dropped on the calculator that is not a ROM is reported, with
  a pointer to the memory view.
- **Desktop app** for macOS (Apple silicon), Windows and Linux: the same
  page with the core linked natively, ROMs remembered by path. An empty
  ROM slot has a Download button that fetches the hpcalc.org image,
  checks it and boots it (not the 42S).
- **Libraries** on crates.io: the core `saturnus` (no dependencies,
  builds for `wasm32`), `saturnus-objects` (RPL objects, user memory read
  from RAM), `saturnus-host` (the front ends' shared host code: one
  protocol engine for the page and the app, skins, typing, Kermit
  transfers), `saturnus-drive` (key scripts, idle waits, screen dumps,
  ROM downloads).

No ROM is included. Clean-room: written from public documentation and
observed behaviour, not from other emulators' source code; parts were
generated by AI systems under human supervision (`AI_NOTICE`).
