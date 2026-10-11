# Changelog

All notable changes to saturnus. The crates (`saturnus-core`,
`saturnus-objects`, `saturnus-host`, `saturnus-drive`, `saturnus`,
`kermit-proto`, `xmodem-proto`, `satx-core`, `satx`), the CLI archives,
the desktop app and the web page share one version.

## Unreleased

- **saturnus tx serves a folder to a 38G, 39G or 40G**: these calculators
  start every transfer themselves, so the page plays the "disk drive"
  they send aplets to and get them from ("Serve a folder to a 38G, 39G
  or 40G"): a folder, a cable or an emulated 38G, 39G or 40G (its
  display and keys shown), a waiting screen that says what to press,
  the transfers as they happen, and Stop. The directory files the
  calculator asks for are made from the folder, so a folder of aplets
  from anywhere works without "Initialize directory?". Binary aplets go
  to the 39G and 40G by wire too (RECV, "HP39/40 (Wire)"; tested on the
  emulator only). "Transfer Failed" is explained where it can be.
  `kermit-proto` gains a Kermit server (`Server`, the calculator as
  client) and `satx-core`'s `Model` knows the 38G and the 39G/40G.
- **saturnus tx: fast transfer (XModem) on the 48GX and 49G**: "Fast
  transfer to the other pane (XModem)…" on a variable's menu, "Fast
  transfer a file to it (XModem)…" in the calculator's "⋯" and "Fast
  transfer to the calculator (XModem)…" on a folder's `.hp` file. The
  page ends server mode (a 49G in algebraic mode switched to RPN), shows
  what to type (`'NAME' XRECV` or `'NAME' XSEND`, then ENTER), waits up
  to three minutes, and then offers to reconnect once `SERVER` runs again
  (the 49G back in algebraic mode, the transfer setting the user had
  kept). 1k blocks and HP's CRC on the 49G, 128-byte blocks and checksum
  on the 48GX; an emulated calculator can have the command typed for it.
  A name in use is refused before the server ends (the calculators do
  not overwrite by XModem). Tested on the emulated 48GX and 49G.
- **The emulator and saturnus tx link each other**: saturnus tx's header
  has an Emulator link and the emulator's controls panel a "Transfer
  files: saturnus tx" link (and a palette command, "Open saturnus tx");
  on both pages the saturnus brand goes to the landing page. Not in the
  desktop app.
- **The installed pages never cache a half-deployed build**: the service
  worker checks every file's SHA-256 (written by `web/site.sh`) before it
  caches a new version, and fails the install on any mismatch, so the
  browser keeps the old version and tries again later. A transfer page
  whose modules and wasm are from different versions now says "This
  page's files are from different versions. Reload the page." instead
  of `No request "pict"`; "Could not get the graphics screen" and
  "Could not download X as text" replace "was not got"; the right
  pane's start card says that the browser does not open the home
  folder, Desktop or Downloads themselves. The web tests' browsers download into their
  temporary profiles, not ~/Downloads.
- **saturnus tx: folders only in the right pane.** The left pane offers
  calculators only (on the cable or emulated); the right pane a folder on
  the computer (or the page's own files) or a calculator, so two
  calculators still copy to each other. The "Change" menu and the start
  cards say so.
- **saturnus tx has two panes**: each shows a calculator (on a cable or
  emulated) or a folder on the computer (picked in Chrome and Edge, its
  permission asked when it is chosen and again on "Allow" the next
  visit; elsewhere the page's own files), the calculator left and a
  folder right; stacked with a switch on a phone. Copy by dragging rows
  to the other pane, F5 or "Copy to the other pane", and drop files and
  folders from the file manager; a calculator directory becomes a folder
  of `.hp` files (or one file), a folder a directory. Between two
  calculators the copy goes through saturnus-tx's plan, its warnings
  (the other family, what has no text form, a directory in the way)
  shown before anything is sent. Rename (F2), purge or delete (Delete)
  and new directory on both sides; "Replace X?" before any name in use
  and the count of what a directory holds before it goes: nothing is
  replaced or removed without a question. Backup with the memory check,
  into the folder as `48GX-backup-2026-10-11-1432.hp`, "Copy everything
  to a folder" as the way without free memory; restore after "Replace
  everything on the calculator?", then the restart said and "Reconnect".
  The graphics screen (PICT) in the calculator's "⋯". A queue under the
  panes: one operation per place at a time, the rest waiting, visible
  and cancellable, the progress in bytes.
- **saturnus tx, the transfer page** (`/saturnus/transfer/`, `web/tx.html`):
  a calculator on a serial cable (Chrome and Edge, Web Serial) or one
  emulated in the page, as one pane with the directory tree, the list
  (name, type, size, checksum) and a preview (programs as the
  calculator's text, strings and data as text, GROBs as pictures). The
  connect dialog says per model how to start the server; the page finds
  the speed and parity by itself (settings for a fixed one), tells the
  model, and gets variables as `.hp` downloads or as text, and puts
  files chosen or dropped: binary and `%%HP:` text as they are, anything
  else as a string after asking, a name in use after "Replace X?".
  Browsers without Web Serial say so and still have the emulated
  calculator (a 48SX, 48GX or 49G from the ROMs the emulator keeps on the
  site, read only, or a ROM file). The protocol runs in its own Worker,
  so a hidden tab does not slow a transfer. The landing page links it.
- **crates/saturnus-tx-web**: the transfer page's wasm bindings
  (`web/tx-build.sh` into `web/tx/pkg`); not published.
- **The icon sprite is one file**, `web/shared/icons.svg`, fetched once
  by both pages (`loadIcons`); the emulator looks the same.
- **The device tree and list** (`<device-tree>`, `<device-list>`) are
  shared components over a `Device` interface (`web/shared/device.js`).
- **The site** (`web/site.sh`) builds `transfer/` beside `emulator/`, with
  its own manifest, service worker and build hash; `web/site.sh --list
  transfer` lists its files. CI's `wasm-tx` job builds both wasm packages
  and runs the transfer tests; `just tx-e2e DIR` runs the page against
  the emulated 48SX, 48GX and 49G.
- **The front-end protocol gives the page the serial port**
  (`web/protocol.md`, "The serial port"): `serialOpen`, `serialWrite`
  (*bytes*) and `serialClose`, and the `serial` event with what the
  calculator sent, on the Worker and the native hosts. The writes to the
  user memory (hidden Kermit transactions) are refused while the page
  holds the port, and the port cannot be taken while one runs. For the
  transfer page's emulated calculator.
- **satx-core gets a sans-I/O face for the browser**:
  - `Machine`, a sans-I/O face for every calculator operation (`Op`,
    `Reply`, `Progress`): fed bytes, the caller's time and link errors, it
    hands out bytes to write, progress events and results. It never
    blocks, reads a clock or touches a port, so it runs where bytes arrive
    by callback (a browser with Web Serial).
  - A `native` feature (default) for the serial port and TCP transports,
    and an opt-in `saturnus` feature for the in-process emulator
    (`saturnus://[MODEL@]ROM`). Without default features satx-core builds
    for `wasm32-unknown-unknown` and pulls in neither `serialport` nor
    saturnus; satx turns both on, so its binaries open `saturnus://`
    addresses as before.
  - A `time` module (`Instant`, `Duration`; `web_time` on wasm32) and a
    README with the embedder API.
  - `Session`, `Calculator` and `XmodemSession` are thin blocking loops
    over the same protocol code as `Machine`; their API and behaviour are
    unchanged. A link error ends the current transaction only; the next
    one tries the transport again, as before. `TcpTransport` and
    `SerialTransport` need `native`; without it `transport::open` refuses
    serial and TCP addresses with `Error::Address`.
  - `Error::Serial` holds a `SerialError` (the `serialport::Error` is
    `SerialError::inner`) instead of the `serialport::Error` itself, and
    exists in every build, so `native` stays additive: a match on `Error`
    compiles whichever crate turns `native` on. Its text is unchanged.
  - CI and `just wasm`: `cargo check` of satx-core for wasm32 and its
    clippy, both without default features.
- **No zooming in fullscreen on a phone**: a pinch beside the keys no
  longer enlarges and crops the calculator, a double tap no longer
  zooms, and a zoom made before fullscreen is undone on entering it
  (`web/nozoom.js`). Leaving fullscreen lets the page zoom again.

## 0.2.0 (2026-10-11)

- **Renames, with no compatibility for the old names.** The core library
  is now the crate `saturnus-core` (`use saturnus_core::...`), and the
  command-line tool is the crate `saturnus`: `cargo install saturnus`
  installs the `saturnus` command. On Linux the CLI's .deb and .rpm are
  the package `saturnus`, and the desktop app's are `saturnus-app`.
- **satx joins the project**: the transfer tool for real calculators
  (Kermit and XModem over a serial cable), with its protocol crates
  `kermit-proto` and `xmodem-proto` and its library `satx-core`, now
  lives in this repository (`crates/satx-cli`, the `satx` command).
- **One release, two command-line tools**: the tag `v0.2.0` releases
  both `saturnus` and `satx`: archives for each platform (satx's with
  shell completions, and for Intel Macs too), a .deb and an .rpm of each
  (packages `saturnus` and `satx`), Homebrew (`brew install
  ractive/tap/saturnus`, `ractive/tap/satx`) and Scoop. Each has its own
  checksums file on the release, `saturnus-SHA256SUMS` and
  `satx-SHA256SUMS` (0.1.0 had `SHA256SUMS`). On crates.io the nine
  crates `saturnus-core`, `kermit-proto`, `saturnus-objects`,
  `saturnus-host`, `saturnus-drive`, `saturnus`, `xmodem-proto`,
  `satx-core` and `satx` are all at 0.2.0: `cargo install saturnus`,
  `cargo install satx`.
- **Screen to a variable… and Screen to stack**: beside Copy screen and
  Save screen (the panel, the command palette, the display's menu), the
  display goes onto the calculator as a graphic (GROB, 131 × 64): the
  same pixels the calculator's LCD→ gives, without typing anything. A
  variable is named in the memory view's name field, in the current
  directory (a taken name asks before it is replaced); the stack gets it
  on level 1. 48SX, 48GX and 49G; elsewhere the buttons are off and say
  why. Also `saturnus ctl screen --to-var NAME` and `--to-stack`, and
  the protocol's `storeScreen`.
- **Saved states work the same in the browser and the desktop app**:
  Save state keeps one state per model at one click (in the browser, or
  in the app's data folder) and says which save it replaced; Load state
  asks first, naming the save's date and time. A "⋯" beside them saves
  the state to a `.sat` file and loads one, in both; the file also
  loads in `saturnus run --load`. A state of another model, or a file
  that is not a state, is refused in plain words. The app's earlier
  `.state` files still load.
- **The CLIs' macOS binaries are signed and notarised** with a
  Developer ID certificate, through release-workflows v0.2.4 (now
  v0.3.1).
- **The site**: <https://ractive.ch/saturnus/> is a landing page (what
  saturnus is, the seven calculators, what you can do, where to get it,
  satx coming soon, the knowledge base; `web/landing/`), and the emulator
  lives at <https://ractive.ch/saturnus/emulator/>, unchanged inside.
  Kept ROMs and states stay where they were. A small service worker at
  `/saturnus/sw.js` retires the one the old page registered (it would
  have kept serving the old emulator at `/saturnus/` from its cache):
  it deletes the old caches, unregisters and reloads; it is removed on
  2027-01-15. The emulator's storage names are listed in one module,
  `web/storage-names.js`, so no other page of the site may take them.
  Install the web app again from the new address.
- **The logo takes the model's colours**: the headers' and the case's planet and ring come from the running (else the chosen) model's skin, its shift keys mostly; the plain logo, now flat without the highlight, stays for no model, the favicon and the icons.
- **A short tour of the page**: "Show me around" (in Search and in
  About) rings one element at a time with a sentence or two beside it,
  Back, Next and Skip tour, and a count; Esc ends it. Three chapters,
  each also on its own in Search: Getting started (the panel, the ROMs
  and where to download them, Search, the keyboard shortcuts), The
  calculator (keys, paste, shift-click, Edit, screen images), and The
  memory view while a 48SX, 48GX or 49G runs. The tour only shows: it
  opens the panel and the memory view and closes them again, and never
  presses a key or writes to the calculator. On a first visit a quiet
  line offers it once; × dismisses it for good. On a phone the words are
  a sheet at the bottom.
- **Larger headings in the side panel** (Model, ROM, Speed, Display,
  Screen images, Theme).
- **The memory view, tidied** (UX audit, round 2):
  - Rename, New directory, Copy to and Move to put Cancel first and their
    action last, as the default button, like the dialogs.
  - A stack level has Edit and a "⋯" (Copy text), and a right-click,
    Shift+F10 or the Menu key on it opens the same menu, as on a variable.
  - The Flags tab says which setting a group of flags holds ("Now:
    Rectangular", "Now: Radians"), and the word size and digit count
    read as help, not as notes on the manuals. The plain texts and the
    settings come from the hardware wiki through `scripts/flags-json.py`.
  - On a model without a memory view (the 42S, the 38G, 39G and 40G) the
    view closes and its buttons go; the 48SX, 48GX and 49G bring them back.
  - Where the view takes the calculator's place (a phone, a window under
    1000 px), the page opens on the calculator.
  - Messages no longer give how long a change took ("Copied P to HOME ›
    D."); the first change that makes IOPAR in HOME says why it appeared.
  - With no calculator running, the view offers Choose ROM… as the
    display does.

- **One question before what cannot be undone**: Purge, Replace (Copy to,
  Move to), Remove ROM, Remove ROMs and Start fresh ask in the same
  small dialog, Cancel first, the action named on a button in the error
  colour; on a phone it is a sheet at the bottom.
- **The ROM table's rows act like the memory view**: a button that acts
  at once (Choose…, Change…, or Download… in the app) and a "⋯" for the
  rest (Choose a file…, Download again…, Remove…), lined up, so every
  row keeps one height. Remove… asks first and removes one model's ROM;
  a model running from it stops. "Forget ROMs…" is now "Remove ROMs…".
- **Graphics (GROBs) as pictures in the memory view**: a variable or a
  stack level holding one shows the picture, sharp at a whole scale in
  the LCD's colours, with its size ("Graphic · 131×64 · 1107.5 bytes")
  and its nibbles folded below; **Copy image** and **Save as image…**
  give it as a PNG four times its size in the Screen images look, and
  the Stack tab shows a thumbnail beside "Graphic 131 × 64". Objects read
  from the calculator carry the picture as `graphic` (width, height,
  rows).
- **Theme: System, Light or Dark**: a choice in the panel (and the
  command palette) that fixes the page's colours, or lets them follow
  the device as before; kept, applied before the page shows, and
  followed by the desktop app's title bar.
- **Buttons that cannot act are easier to read**, in both themes: their
  text is grey at about 4:1 instead of faded to under 3:1.
- **The memory view no longer jumps on a write**: a one-line status row
  is always there, with a hint for the tab, then "Setting flag -14…",
  then what came of it, which goes back to the hint after a few
  seconds (an error stays until the next click or key).
- **A file the 49G refuses as a circular reference** (`test.txt` holding
  `test`) is explained in plain words; the desktop app's ROM download
  question has a button that opens the file's hpcalc.org page.
- **An Edit button that is always there** (over the calculator, and in
  the phone's top bar): it edits what Cmd/Ctrl+E edits, says what in its
  tooltip, and says why when there is nothing to edit.
- **One small toolbar over the calculator**: Edit, Search and the memory
  view as icon buttons; Search's ⌘K shows on hover and in the palette.
- **The command line as a .deb and an .rpm** (Linux x86_64) on the
  releases page, `saturnus-v0.2.0-x86_64-linux.deb` and `.rpm`: the
  package `saturnus`, installing `/usr/bin/saturnus` next to the
  desktop app's package `saturnus-app`. The binary is statically linked
  (musl), so any glibc will do.
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
- **Copy to… and Move to…** in the memory view's menus copy or move a
  variable or a directory into another directory, on the calculator
  itself. A move removes the original only after the copy is checked. A
  taken name asks first. Also `saturnus ctl cp` and `ctl mv`.
- **A double-click edits**: on a variable in the memory view, or on a
  stack level, it opens the editor, as Edit does.

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
- **Search runs the action you name**: "save state", "start fresh" or
  "copy screen" and Enter run the action instead of typing the words
  into the calculator; the text to type comes after the actions.
- **Search finds Edit and Remove the ROM…**; an action that cannot run
  now is listed greyed with the reason, and Enter on it says why.
  Remove asks the ROM table's question.
- **A quieter Search list**: before you type, the screen images show
  once (their black-on-white twins come with a search), with no kind
  badges; descriptions are plain ("Copy the display as a picture") and
  name no keys on a phone.
- **The 49G's algebraic-mode notice** shows in the search only, not
  in the editor, in plain words.
- **Off buttons change their form**: in both themes a button that
  cannot act loses its fill and shadow and gets a very light border,
  its label grey but readable; the same for primary and icon buttons,
  toolbar tools, segments and menu items.
- **Off buttons look off**: the Edit icon is greyed when it cannot act,
  and a tap on it says why under it (phones show no tooltips). Shorter
  reasons: "Nothing to edit yet: the stack is empty."
- **One name per thing**: Search (not "Command palette") on phones and
  in the shortcuts dialog; "Copy screen" and "Save screen…" in the
  display's menu, with their keys; "Typing goes here" and "Back to the
  calculator (Esc)" in the memory view; "Other commands" in the
  reference tree; About names "Remove ROMs…" and "the author's" 48SX.
- **What came of an action shows where you look**: "State saved",
  "Loaded the state saved …", "ROM removed", a fullscreen refusal and
  the memory view's results while it is closed show in a small toast at
  the top of the calculator, with the panel hidden and on a phone too.
  It goes after four seconds; an error stays until you click it or its
  ×. The panel's bottom line now names only what runs ("HP 48SX ·
  sxrom-j").
- **Removing the running model's ROM says the calculator stops.**
- **Shorter ROM help**: the hints under the ROM table are a sentence or
  two; "Kept in this browser" shows only once a ROM is; a phone is told
  to choose the file, not to drop it; the app's hint says a downloaded
  ROM is kept in its data folder.

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
