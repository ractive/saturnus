# saturnus web UI

A static page around the WebAssembly build of the saturnus core
(`crates/saturnus-web`, bindings over `crates/saturnus-host`). Plain
HTML, CSS and ES modules; no framework or
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
- The host owns the calculator, paces it against the wall clock and pushes
  events: `frame` (the packed LCD, annunciators, contrast) only when the
  display changed, `keys` when the keys down changed, `status` when the
  model, Run/Pause, a halt, the speed or the run loop changed.
- `store.js`: one `EventTarget` with the page's state, fed by the
  backend's events and the controls (one-way: event, store, components).
- `components/`: framework-free Web Components in the light DOM
  (`display: contents`, so `style.css` lays them out as before):
  `<sat-calculator>` (the skin, LCD, pointer and computer keyboard, the
  empty state without a ROM; its pure rules in `norom.js`, the contrast
  mapping and the ON + / ON - step in `contrast.js`, the fullscreen
  face's crop and scale in `edge.js`), `<sat-controls>` (the panel's controls and status line),
  `<sat-about>` (the About panel), `<sat-explorer>` (the memory view,
  with its Reference tab), `<sat-palette>` (the command
  palette), `<sat-shortcuts>` (the keyboard shortcuts dialog, over
  `bindings.js`: the rebindable keys, their defaults, matching, labels
  and warnings, tested by `web/test/bindings.test.mjs`), `<sat-tour>`
  (the tour, below, over the steps of `tour.js`). They render from
  the store and act only through the backend.
- `romstore.js`: the Worker's ROM slots over IndexedDB (`romSlots`,
  `bootModel`, `chooseRom`, `forgetRom`, `romSettings`), identifying and
  assigning with the wasm core's `identify_rom` and `plan_roms`, the same
  rules the desktop app runs natively; tested by `web/test/` with a fake
  store, including a store that refuses.
- `memory.js`: the memory view's reads. While the memory view is open it asks
  the host to watch the user memory and reads the tree, the stack and the
  flags into the store after each `memoryChanged`; it never polls.
  `writes.js`: the memory view's writes (store a file, also dropped on a
  directory; save a variable as a file; rename, purge, create a directory,
  change directory, set or clear a flag), one at a time, each a hidden Kermit transaction
  on the host (`protocol.md`, "The user memory, written"); the busy
  overlay and the result's message through the store. Tested by
  `web/test/writes.test.mjs` with a fake backend. `actions.js`: which
  actions the memory view offers for what it shows (the primary button,
  the "⋯" menu, the context menu), a pure function tested by
  `web/test/actions.test.mjs`; `components/menu.js`: the menu itself
  (placement, keys); `resize.js`: the drag, keys and double-click of
  every resize handle.
  `objects.js`: the text forms and previews of calculator objects and the
  rows of the flags panel, pure functions tested by `web/test/`
  (`just web-test`, Node's test runner, no dependencies).
- `flags.json`: what each system flag means per model, generated from the
  hardware wiki by `scripts/flags-json.py` (`just flags`), as `about.json`
  is by `scripts/about-json.py`; the multi-flag fields' plain texts and
  named settings come from the knowledge base's `hardware/system-flag-fields` page.
- `reference.js`: the command reference in the page: the lookup rules of
  `saturnus ref`, the palette's ranking, its Enter rule and the ROM menu
  tree, pure functions over `commands.json`; `palette.js`: the palette's
  model without the DOM (`PaletteModel`: query, rows, selection, the
  command line's state, what choosing sends) and the `ReferenceLoader`
  that fetches the data once; `components/entry-view.js`: one command's
  entry as DOM, shared by the palette and the Reference tab. Tested by
  `web/test/reference.test.mjs` with a fake backend.
- `commands.json` (541 KB, 101 KB compressed): the reference data folded
  from `data/commands/` by `scripts/commands-json.py` (`just commands`;
  `--check` in `just lint` and CI keeps it current). It lives in `web/`
  like `about.json` and `flags.json`, so `web/site.sh` ships it with the
  site and the desktop app embeds it with the rest of the page
  (`frontendDist`); it is fetched only when the palette or the Reference
  tab first opens.
- `app.js`: the composition root: picks the backend, connects it to the
  store and the components, and keeps the page chrome (side panel and
  memory view with their resize handles, sheet, fullscreen), the app's
  shortcuts and the preferences.
- `pwa.js`: the page as an installed app (below): registers the service
  worker and offers its updates, keeps the screen on through a long
  computation, offers persistent storage for the kept ROMs
  (`StorageChoice`, after a ROM is kept, never on load).
- The key queue (hold times, gaps, typed letters through alpha and the
  shifts) is Rust in `crates/saturnus-host/src/host.rs`, shared by the
  Worker (compiled to wasm) and the desktop app (native), so both hosts
  time keys identically in emulated time.

## Build and serve

```sh
cargo install wasm-pack          # once; needs the wasm32-unknown-unknown target
./build.sh                       # wasm-pack build --target web into ./pkg (gitignored)
python3 -m http.server 4860      # any static file server works
```

Open http://127.0.0.1:4860/. Browsers do not load ES modules or wasm from
`file://`, so a server is needed. Served from `web/` the page has no
service worker and no manifest (they are the site's, below); to try the
installed page, serve the site: `./site.sh /tmp/site && python3 -m
http.server 4860 -d /tmp/site` (localhost counts as secure).

## Installed page (offline)

The published site is installable ("Add to Home Screen", Chrome's
Install): `web/site.sh` adds what lives in `web/pwa/`:

- `manifest.webmanifest` (standalone, any orientation, relative URLs so it
  works under `/saturnus/`), the icons in `pwa/icons/` drawn from
  `logo.svg` by `pwa/icons.sh` (committed PNGs: plain, maskable, Apple's
  touch icon), and `pwa/head.html`, the manifest link and Apple's tags,
  inserted into the site's `index.html`.
- `sw.js`, the service worker: `pwa/sw.js` behind `BUILD` (a hash over
  every other file of the site) and `FILES` (their list), both written by
  `site.sh`. It precaches every file at install, fetched past the HTTP
  cache, into `saturnus:<scope path>:<BUILD>`, answers those files and the page's
  navigation from that cache and lets everything else through uncached.
  A new deploy is a new build: the browser finds it on the next open (or
  when the installed app returns from the background), installs it beside
  the old one and waits. A page nobody has touched yet takes it at once
  and reloads; a page in use shows a notice, as Reload restarts the
  calculator; otherwise the new build starts once every page has closed.
  The worker takes over only when the page asking is the only one open
  (other pages keep their build: taking over would hand them the next
  build's files mid-session); it claims pages only on the first install.
  This scope's older caches go when the new build takes over (another
  deployment on the same origin keeps its own), so the glue JS and the
  wasm always come from one build. `web/test/site.test.mjs` checks the
  list against the built site and the hash against a changed file;
  `web/test/sw.test.mjs` the take-over and the caches.
- Registered only over HTTPS or on localhost, never in the desktop app
  (which embeds none of `web/pwa/`; the Tauri test `frontend` checks).
- Persistent storage for the kept ROMs is asked for in context, never on
  load (Firefox turns `navigator.storage.persist()` into a prompt with no
  reason given). At load the page only reads `persisted()`. Right after
  the user picks, drops or downloads a ROM, and only if storage is not
  already persistent, a notice at the bottom asks "Keep this ROM on this
  device?"; "Keep it" calls `persist()` inside the click and shows the
  browser's answer in one line. "Not now" and a refusal are remembered
  (`saturnus.storageAsk` in localStorage) and the notice not shown again.
  The ROMs panel says whether the ROMs are stored permanently, with "Keep
  permanently" while they are not. Without it the browser may clear them
  when space runs low, and Safari after seven days without a visit unless
  the page is on the home screen. Not in the desktop app.
- While the calculator computes for more than 5 s (the run loop's
  `frame`, not asleep waiting for a key) the screen is kept on with the
  Screen Wake Lock API where there is one, and let go when it sleeps.
- Keys held by a finger, the mouse or the keyboard are released when the
  page goes to the background or loses the focus.

## Layout

The calculator is the page: it fills the window's height, or its width on a
narrow screen. On a wide window the controls sit in a side panel on the
left, which the `‹` button hides (a `☰` button brings it back; the choice is
remembered). Below 760 px the panel becomes a sheet that drops down from a
compact top bar, which also carries the palette, the memory view and
fullscreen as icon buttons.

On a wide window Edit, Search and the memory view's toggle are one
small toolbar of icon buttons in the top right of the stage. Search's
key (⌘K, Ctrl+K off a Mac; rebindable) shows under its icon on hover
and keyboard focus, in its tooltip "Search (⌘K)", and in the palette's
header beside esc.

The look is a small set of design tokens at the top of `style.css`
(colours for both schemes, type sizes, a 4 px spacing rhythm, radii,
three elevation levels, durations that go to zero under
`prefers-reduced-motion`, the target sizes), and every rule after them
combines tokens. Icons are one inline SVG sprite in `index.html`
(`components/icons.js` references them). Controls are 32 px tall with a
mouse and 44 px with a finger (`pointer: coarse`), list rows 26 and 40;
the page keeps out of the device's safe areas (the notch, the home
indicator). On a phone the About and keyboard shortcuts dialogs are full
sheets, and keyboard hints (`.kbd-hint`) are hidden where there is no
hover. `web/test/overflow.test.mjs` opens every view at 360, 390, 430,
768 and 1280 px in headless Chrome with touch emulation and fails on any
horizontal overflow (it skips without Chrome; `just web-audit` makes
that a failure).

**Fullscreen** shows the calculator alone, edge to edge: the case is
dropped and the skin cropped to its face (the plates, the window, the logo
and the keys), scaled to the screen's width or height, on the case's
colour. It uses the browser's Fullscreen API, or where the page cannot have
one (the iPhone) lays the calculator over the page, which in the installed
app is the whole screen. The `✕` in the top right corner or the panel's
button leaves it; the search icon in the top left corner, or a swipe down
on the display, opens the command palette. A finger held on a key holds
it; there is no context menu, selection or double-tap zoom on the
calculator. Escape leaves fullscreen in every browser; in Chromium the page locks
Escape so it keeps working as ON, and holding Escape leaves instead. The
`` ` `` key is ON everywhere.

## Use

First get the ROM. The ROMs are HP's software, hosted by hpcalc.org with
HP's permission for use with emulators; they are not part of saturnus,
and saturnus does not host or pass them on. A model without a ROM
shows the file name to expect and a link to its download page on
hpcalc.org (also as a Download link under "ROMs of every model", the
models in the order 48SX, 48GX, 49G, 38G, 39G, 40G, 42S): download the
zip there,
unzip it and drop the file on the page. The page cannot fetch it
itself, as hpcalc.org sends no cross-origin header; the desktop app
downloads it with one click after asking. The links come from the
table of known ROM files in `crates/saturnus-host/src/romid.rs` through
the wasm core's `rom_download` (`web/test/download.test.mjs`). The
42S ROM was never published: read it out of your own calculator.

1. Choose the model, then its ROM file. The ROM is read in the page and
   kept in this browser, per model, so you choose it once: choosing a
   model starts it, and the last model starts when the page opens (a
   setting under "ROMs of every model"). Several files may be chosen at
   once, or dropped on the page. saturnus recognises each file and gives
   it to its model. If a file fits more than one model, a button lets you
   choose. Unknown files are named and skipped (the rules:
   `protocol.md`, "ROM slots").
   Dropped files none of which is a ROM (`test.txt` dropped on the
   calculator) are named beside the ROMs and the controls come into
   view; when the running model takes files (48SX, 48GX, 49G) the message
   points to the memory view, which stores them on the calculator. The
   app takes ROMs only through Choose…, so there any file dropped outside
   the memory view gets that message.
   A ROM is never uploaded. In "ROMs of every model" each row has a
   button that acts at once and a "⋯" for the rest: Change… with Download
   again… (the app) and Remove… behind the ⋯ for a filled row; Download…
   with Choose a file… for an empty one in the app; Choose… alone for the
   42S and an empty row in the browser, which links to hpcalc.org beside
   it. Remove… asks first and removes that model's ROM (the
   39G's and the 40G's together when they share a file, the 49G's saved
   state with its ROM); a model that runs from it stops, and its display
   says there is no ROM (`unload`). "Remove ROMs…" (it asks first) removes
   them all, with the saved 49G states (which contain the ROM).
2. Keys: click or tap the keys of the drawn calculator, or use the keyboard
   (below). With a mouse, Ctrl+click presses left-shift, then the key;
   Alt+click (Option+click on a Mac) presses right-shift, then the key.
   The shift is skipped if it is already on. A model with one shift (38G,
   39G, 40G, 42S) takes either. While Ctrl or Alt alone is held and the
   calculator has the keys, the labels it reaches light up on the
   calculator. (`shiftclick.js`; the host's key queue decides about the
   shift, `keyDown` with `shift`.)
3. Screen images (`screenshot.js`): Copy screen and Save screen in the
   panel, their palette commands and shortcuts, and the display's menu
   (a right-click on it, a long press on a phone) take the display as a
   PNG, annunciators included, at 4× with hard pixel edges, in the LCD's
   colours at its contrast or black on white (the panel's "Screen
   images" choice; the menu offers both). Copy puts it on the clipboard;
   where a browser cannot copy images it is saved instead, and the
   status line says so. Save downloads it in the browser and asks where
   in the app (`saveFile`), as `48gx-2026-10-09-0142.png`.
   Theme (`theme.js`): System follows the device's light or dark
   setting, Light and Dark fix the page's colours (the panel's "Theme"
   choice, kept; the palette's "Theme: …" commands). A fixed theme is
   `data-theme` on `<html>`, set before the first paint by
   `theme-boot.js`; the installed page's bar colour and the desktop
   window's title bar follow it. The drawn calculator keeps its colours.
4. Buttons that cannot act yet (Reset, Save and Load state, the display
   and screen buttons) say why in their tooltip, such as "Start the
   calculator first" or "No saved state for this model". Reset restarts
   the calculator; its memory is kept. Pausing (stopping
   the calculator's clock) is the command palette's "Pause the
   calculator" / "Run the calculator"; the status line says "paused".
   Choosing a model without a ROM pauses the other model's calculator;
   choosing it again resumes it.
5. The calculator keeps its state, as a real one keeps its memory when
   turned off: about 5 s after the last key (once a computation, typing
   or a transfer has finished), and at once when the page is hidden, the
   page saves the calculator's state automatically. After a reload the
   model starts as it was left: the same stack, variables, mode and
   screen, no "Try To Recover Memory?". An idle calculator is never
   written (the 49G's state contains its 2 MB ROM, in flash memory). A
   kept state that does not fit the ROM (another ROM, an older format) is
   dropped quietly: the model starts with empty memory, and the console
   says why. The command palette's **Start fresh** restarts the model
   with empty memory and forgets the kept state, after asking in the
   page. The rules are in
   `protocol.md`, "Auto-save".
6. Save state keeps one saved state per model, apart from the automatic
   one, which never overwrites it. Load state loads it at any time. A
   state only loads with the ROM it was saved from. In the desktop app,
   Save state and Load state open a file dialog instead.
7. Speed: 1×, 2×, 4× or Max. Above 1× the calculator's clock runs fast too;
   at Max the page runs as much emulated time per animation frame as fits
   in about 11 ms of wall time, at most one emulated second per frame, so
   the page stays responsive. The setting is remembered.

Settings, ROMs and saved states stay in this browser; nothing is
uploaded. Where the browser refuses to store (blocked or full storage)
the panel says so, and the ROMs last until the page is closed. The
storage keys are listed under "Stored data" below.

## Keyboard

The panel's **Keyboard shortcuts** link, its own key (Alt+K, ⌥K on a
Mac) and the palette's action of that name open a dialog with every key.
Typed characters are fixed:

| Keys | Calculator |
| --- | --- |
| `a`–`z`, `A`–`Z` | the letter, through the model's alpha mode; lowercase through its shift (the 39G/40G space is alpha + plus). Not on the 42S, which types letters from its ALPHA menus: its skin has no typing data |
| `F1`–`F6` | the six menu keys (the top row Σ+ to XEQ on the 42S) |
| `0`–`9`, `.`/`,`, `+ - * /`, `^`, `'` | as printed |
| `Enter`, `Space`, `Backspace`, `Delete`, arrows | ENTER, SPC (shift + 2 on the 38G, which has no SPC key), ⬅, DEL, the cursor keys |

The calculator keys a computer has no key for and the app's actions are
shortcuts (`bindings.js`), changed in the dialog: **Add key**, then press
the key or combination; × removes one; "Reset" and "Reset to defaults"
go back. A shortcut is tied to the physical key, so it stays in the same
place on every keyboard layout. It is shown with the label your layout
gives that key in Chromium; other browsers show the US label. The
defaults (Mod is Cmd on a Mac, Ctrl elsewhere):

| Action | Default keys |
| --- | --- |
| ON | `Esc`, Alt+O |
| α | `Tab` (twice for alpha lock on the 48SX, 48GX and 49G, where a third press unlocks; the 38G, 39G and 40G cancel on the second press) |
| left shift | Alt+L; the only shift on the 38G, 39G and 40G |
| right shift | Alt+R |
| search | Mod+K |
| keyboard shortcuts | Alt+K |
| keys to the memory view and back | Alt+M |
| show or hide the memory view | Alt+Shift+M |
| fullscreen | Alt+Enter |
| next speed | Alt+S |
| darker, lighter display | Alt+↑, Alt+↓ |
| copy screen | Alt+Shift+C |
| save screen | Alt+Shift+S |
| search rows 1-9 | see Search (a modifier, chosen in the dialog) |

No default is a dead key on the US, UK, German, Swiss German or French
layouts. No default without a modifier types a character the calculator
uses on those layouts or on Dvorak. (`[` and `]`, the old shifts, are `+`
on German and `/`, `=` on Dvorak.) A key that is dead on one of these
layouts says so in the dialog. The dialog also warns
when a key is another action's (the first in the list wins), the
browser's or the system's (Cmd/Ctrl+digit for tabs, Alt+digit on Linux,
Shift+Esc in Firefox, Esc in fullscreen, Cmd+Q/W), one of the fixed
calculator keys, or one that types a character (with the layout's map,
one that types a character the calculator maps). While a row records,
Esc cancels; "Use Esc" records Esc. A binding works while the calculator
has the keys; the app's actions also from a search field when they have
Ctrl or Cmd, or Alt except on a Mac (where Option types letters), and
from inside an open dialog only the palette's and the dialog's own.

A typed letter is expanded into key presses when its turn in the key queue
comes: the alpha key unless the alpha annunciator is already on, the shift
for a lowercase letter (after alpha on the 48SX, 48GX and 49G, before it
on the 38G, 39G and 40G), then the letter's key. Right after a letter
the page pressed alpha for, alpha is known to be off again (one-shot),
so a run of letters
needs no waiting; otherwise the page waits for the ROM to go idle before it
reads the annunciator, which the 48SX ROM blinks while redrawing.

### Paste

Paste (Ctrl+V or Cmd+V) while no text field or dialog has the focus
types the clipboard into the calculator's command line (48SX, 48GX,
49G), by key presses, at full speed; `\r\n` becomes the calculator's
newline. While more than a few characters are typed the screen keeps its
last frame, dimmed, and the status line says "typing…". Text with a
character the model cannot type is refused before any key is pressed,
with the reason in the status line. The desktop app runs the same code;
whether its webview delivers the paste with no text field focused is not
tested yet (Chrome does).

## Search (the command palette)

**Cmd+K** (Ctrl+K elsewhere than a Mac; a binding, see Keyboard) or
**Search** (the magnifier, over the calculator or in the phone's bar)
opens the command palette, called Search on the page: one input over the calculator,
which stays visible behind a dimmed backdrop. It is the command reference,
the way to send commands and text to the calculator and the entry to the
app's actions at once. While it is open the keys are its own; Escape
closes it and gives them back to the calculator.

Typing suggests, ranked: the running model's commands (an exact name
first, then names the query begins, then names containing it, then
descriptions and example text), your variables of the current directory
and its parents (read once when the palette opens, nearest directory
first), the ROM's menus (`PL` on a 48SX, which has no PLOT command, offers
its PLOT menu), the app's actions (Edit, ROM, Run/Pause, Reset, Save and Load
state, the speeds, the memory view's tabs, fullscreen, the view, the
panel, Remove the ROM, About). An action that cannot run now (Edit with
nothing to edit, Remove with no ROM kept) is listed greyed, its reason
as its description; Enter on it says why. Before anything is typed the
list holds the actions, without the screen images' other-look twins
(a search finds them); on touch their descriptions name no keys. While a ROM runs, its action is "Change the HP 48GX
ROM…", after Pause, Reset and Save state; without one, "Choose the HP
48GX ROM…" comes first. The lookup rules are `saturnus ref`'s: case does not
matter, the calculator's ASCII codes (`\->LIST`, `\.S`) and friendly
spellings (`->LIST`, `SIGMA+`) find the name, and a spelling several
commands share lists them all (`INT` on the 49G: INT, then ∫). Each
command row shows its stack effect and description; the selected row's
full entry is beside the list: stack
effect, description, the models that have it, the ROM's menu and the
manual's key for it, the examples generated on the emulator as input →
result with **Try it**, and links into the manuals' pages.

On a phone (below 760 px) the palette is a sheet: the input at the top,
the list under it, and a tapped row opens its entry as a second step
with a way back and buttons for what Enter and Cmd/Ctrl+Enter would do
(Run, Insert; Run; Open in the Reference tab). On a phone the
list shrinks when the on-screen keyboard opens, instead of being covered.

Choosing: arrows and Enter, a click, or the number shortcuts on the first
nine rows: by default Cmd+1–9 in the desktop app on a Mac, Ctrl+1–9 in Mac
browsers (which reserve Cmd+digit for their tabs), Alt+1–9 in browsers
on Windows and Linux (which reserve Ctrl+digit); the modifier can be
changed in the Keyboard shortcuts dialog, and each row shows its
hint. The palette's key inside the open palette closes it. **Enter mimics the calculator's keys**: with no
command line open the command is typed and executed; with one open its
name is inserted at the cursor (with the spaces the calculator would put
around it); Cmd/Ctrl+Enter does the opposite, and the footer says which
is which. A variable inserts its name (Cmd/Ctrl+Enter evaluates it).
Text that is not a single name (`13 4 ^`, `« 1 2 + »`, `30`) is offered
as "send as typed", first, unless it names an action (each word begins
a word of its name, one of three letters or more: "save state", "start
fresh"), which then comes first and the send row after the actions; a bare word that names nothing exactly can
still be sent, as the last row. It is typed into the calculator key by
key (see Paste above for how typing works and when the screen freezes).
When a run leaves the command line open with an error
the palette stays open and shows the calculator's message.

Where it cannot do everything it says so under the input: no ROM running
(the selected model's reference can be read, nothing sent), a model
without a reference or a command line (38G, 39G, 40G, 42S: app actions
only), and the 49G in algebraic mode, where the RPN text of the examples
and command names does not parse (the action "Switch the HP 49G to RPN
mode" runs `CF(-95)`). The editor does not show that notice: saving
there sets the mode itself.

### Editor mode

The palette grows to a multi-line RPL editor: on **Shift+Enter**, on
Enter while a delimiter (`«`, `{`, `[`, `(`, `'`, `"`) is still open, on
a pasted line break, or when text is pulled in to edit. The editor is a
plain textarea over a highlighted copy of its text (no library):
delimiters, numbers, strings, comments, tags, structure words, the
running model's commands (from the reference) and your variables each
have their colour, and the bracket pair at the cursor is marked. Enter
indents the new line by what is open (`«`, `{`, `IF`, `CASE`, `FOR`,
`DO`, `WHILE`, ...), a closer typed first on a line moves it out, Tab
completes the word at the cursor from the commands and variables (the
strip under the text; ↑/↓ choose while it shows, Escape hides it) or
indents, Shift+Tab outdents, **Format** (Shift+Alt+F) lays the text out
by its structure. `<<`, `>>`, `->` and the translation codes (`\->`,
`\GS`, `\v/`, ...) become `«`, `»`, `→` and the calculator's characters
as they are typed or pasted (not inside strings or comments).
Alt+↑/↓ goes through the text sent before (the last 50, kept in this
browser).

What the editor holds goes back with Cmd/Ctrl+Enter (free text: Run or
Insert by the palette's Enter rule; Cmd/Ctrl+Shift+Enter the other) or
Cmd/Ctrl+S (pulled text):

- **Edit** (the pencil over the calculator and in the phone's bar) is always
  there and does what Cmd/Ctrl+E does (below): its tooltip names what it
  would edit ("Edit the command line (⌘E)", "Edit stack level 1 (⌘E)",
  "Edit PRG in HOME (⌘E)"), and it is off, saying why, when nothing can
  be edited (no calculator, the 38G, 39G, 40G and 42S, an empty stack
  with no command line, a write or typing running). On the calculator's
  command line (or Search's action "Edit the command line") it
  pulls the line and its cursor; **Send back** replaces it (`replace`),
  and the calculator is in the same edit with the new text, also inside
  EDIT and VISIT, which ▲ then VIEW opens on a stack level.
- **Edit** in the memory view, a variable's or a stack level's first
  button, pulls its text (a program laid out by its structure);
  **Save** has the calculator compile it and store it there (about a
  quarter of a second). The header
  marks unsaved changes; closing with them asks once. A save that went
  through closes the editor (Save, Send back and Cmd/Ctrl+S alike): the
  status line says what was saved, and the focus goes back to Edit when
  it was opened by keyboard. A syntax error is the calculator's own
  message under the text, and nothing changed; a refused save (an open
  string, the calculator busy) keeps the editor open with its text. An
  object without a text form (a graphic, a library) has no Edit.
  Cmd/Ctrl+E (rebindable) does the same where the keys are: after a
  click in the memory view or with the keys in it, for the object
  selected there; after a click on the calculator or a key typed to it,
  for its open command line, else stack level 1.

The **Reference** tab of the memory view is the same reference for
reading: one tree of the ROM's menus (roots in the order of the keys that
open them). A built-in `MENU n` no key opens is named after the
category at least half of its commands have in a manual (this model's,
else another's): "MODES (MENU 21)" under MODES on the 48SX (its second
page), "STAT (MENU 96)" among the roots on the 48GX; the others are under
"Other menus", with a line saying what they are. The commands no menu
offers come last, under the folded heading "Other commands", grouped
by the key a manual names or our own group. Then the commands of the
selected menu with their stack effects, and the entry below, with "Try
it". The search ranks as the palette does (names, then descriptions:
INT, then ∫, then INTVX on the 49G). A menu row chosen in the palette
opens the tab at that menu.
The About panel links the full manuals.

## Memory view

The **Memory** button (the panel icon top right of the calculator; in the top bar on a
narrow screen) opens the memory view, with tabs on the calculator's user
memory, read live (48SX, 48GX, 49G; on the other models the button is
not there and an open view closes, saying it works with those three; the
choice to have it open is kept for them). With no calculator running it
offers **Choose ROM…**, as the display does:

- **Variables**: the directory tree of HOME on the left, the variables of
  one directory on the right (name, type, size and checksum as the
  calculator's BYTES gives them, newest first), the selected object below:
  a number or a name as text, a string, a list by element, a matrix as a
  grid, a directory as its listing; large objects are cut with a count.
  Browsing here is navigation
  in the page, not `cd` on the calculator: the calculator's own directory
  is marked "current", the view follows it until you browse elsewhere,
  and "Show it" returns. "Find a variable" searches every directory.
- **Stack**: the levels as the calculator has them, level 1 at the bottom,
  the selected level in full below.
- **Flags**: the system flags by topic with their current state and what
  that state means, the flags without a documented meaning and the user
  flags as cells. A click on a flag sets or clears it; **Set only (n)**
  shows only the n flags that are set. A group of flags with named
  settings (coordinate system, angle mode, base, number format) says
  which one it holds ("Now: **Rectangular**"), worked out from its flags.
  The meanings come from `flags.json`; where the manuals do not say what
  a flag means, the panel says so.

What can be done with the thing shown is in its preview's head: one
button for the next step (**Edit** for an object or a stack level,
**Open** for a directory selected in the list, **Make current** for the
directory shown, HOME included) and a **⋯** menu with the rest: Copy
text, Save as file…, Store file here…, New directory here…, Copy to…,
Move to…, Rename…, and Purge… last (which still asks first). Copy to…
and Move to… open a picker of the directories in the preview; a taken
name asks before it is replaced. A directory has the same set
whether it was chosen in the list or the tree; a stack level has Edit
and a ⋯ too, even with Copy text alone in it. A right-click on a row, a
tree node or a stack level (or Shift+F10, or the Menu key) opens the same menu there,
the first step included; F2 renames, Delete asks to purge, Ctrl+C (⌘C)
copies the text. A double-click opens a directory, and opens any other
variable or a stack level in the editor, as Edit does (one without a
text form is only selected). The bar's **New** menu stores a file or creates a
directory in the directory shown; files can also be dropped on a
directory. Without writes (another host, a model without them) only
what reads is offered, and a single action left stands beside the
first instead of in a menu. The forms in the preview (Rename, New
directory, Copy to and Move to) put Cancel first and their action last,
as the default button, as the dialogs do. A change's message says what
was done, not how long it took (the console has that); the first one
that makes IOPAR in HOME says that the calculator made it, as a real one
does for its transfers. The divider between the tree and the list
can be dragged (or moved with the arrow keys) like the memory view's edge; a
double-click resets it, and it is not there on a phone.

A program is shown as its text in indented lines (one structure word per
line, bodies one level in), an algebraic expression and a unit as the
calculator writes them, a list with its commands by name; "Copy text"
copies the calculator's own text, not the indented layout. Every text
shown or copied is the host's (`text` on each object, from the
decompiler in `saturnus-objects` with the ROM's own command names, in
the calculator's display mode); the page formats no number and no object
itself, it only lays the texts out. A graphic (GROB) is shown as its
picture, at a whole scale (up to 4 times) that fits the preview, in the
LCD's colours, with its size in the head ("Graphic · 131×64 · …") and
its nibbles folded below; **Copy image** and **Save as image…** in its
menu give it as a PNG four times its size (less for a large one, so
the image stays within about 16 megapixels), in the look chosen under
Screen images, and on the Stack tab its level shows a thumbnail the
row's height beside "Graphic 131 × 64" (none above a megapixel). A
GROB with no pixels (`#0 #0 BLANK`) is shown by its nibbles. The host decodes the picture (`graphic` on the
object, web/protocol.md); `graphic.js` turns it into pixels, tested by
`web/test/graphic.test.mjs`. Objects that have no text (a library, a
backup; a program or expression holding something the ROM's tables do
not name) show type, size and checksum with a sentence saying so, an
unknown object's nibbles behind a disclosure.

Layout: from 1000 px the memory view is a third column beside the calculator
(400 to 640 px wide by default; the controls panel stays and can be
hidden); a handle on its inner edge, as on the controls panel's,
resizes it (drag, or the arrow keys on the focused handle; a double-click
goes back to the default), within minimums that keep every control
whole (236 px for the panel, 380 px for the memory view, which leaves the
calculator 320 px), and the widths are remembered. Both close with a
chevron towards their edge. Below
that it lies over the calculator with a "‹ Calculator" button to go back;
below 760 px the top bar's Memory button toggles it. The choice and the
tab are remembered, but where the view lies over the calculator the page
opens on the calculator.

Keyboard: typing goes to the calculator unless the focus is inside the
memory view. A mouse click on a row, a tab or a button does not take the focus;
a click into a search field, Tab from there, or **Alt+M** does (a
shortcut; it opens the memory view if needed and moves the keys back when
pressed again). While the keys are in the memory view, an indicator
beside the tabs ("Typing goes here", the full sentence as its tooltip)
with a **Back to the calculator (Esc)** button, and a bar along its edge, say so.
The keys go back to the calculator with that button, Escape (after it
empties a search field), Alt+M again, or a click or tap anywhere on the
calculator, which still presses the key clicked. Inside: arrows in the tabs, the tree (left and right fold),
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
- `memory_changes()`: a counter (16 hex digits); the host polls it (at
  most every 250 ms, `protocol.md`, Pacing) and the page re-reads only on
  a `memoryChanged` event.

Before the ROM has set up memory (right after power-on, or with no HOME
yet) the calls throw. In the protocol they are the read commands
`memoryTree`, `stack`, `flags` and `objectAt`, with `watchMemory` and the
`memoryChanged` event (`protocol.md`).

## Skins

The calculator is drawn as an SVG skin per model: 48SX, 48GX, 49G, 38G,
39G (the 40G shows the 39G drawing with its own name) and 42S. The 42S skin
was measured from photographs of the owner's calculator, not a manual
figure, and its LCD is 131×16 with seven annunciators. `annunciators()`
returns the same ten keys on every model (the 48SX's six, then `updown`,
`battery`, `g`, `rad`), so its shape is stable; on the other models the four
42S-only ones are always false. Before a ROM is loaded the canvas takes
its row count from the selected model's skin (`lcdRows`), and the skin
is the selected model's whether a ROM runs or not: without one, an empty
state over the LCD names the model (for the 42S, that its ROM must be
read out of one's own calculator) with a "Choose ROM…" button, and a
drawn or computer-keyboard key makes it pulse. The page's old view
setting (`saturnus.view`) is removed on load.

- **Data.** Each skin is Rust data in `crates/saturnus-host/src/skins/`
  (one file per model), handed to the page as JSON by `skin(model)` and
  `Emulator.skin()`, with the model's letter map and typing rules. Unit
  tests check that every key of the model's matrix is drawn exactly once,
  that keys do not overlap, that the alpha letters agree with the ROM and
  the wiki, that every model but the 42S types all 26 letters, that the
  six softkey labels the ROM draws sit over the six menu keys (on the 42S
  within half a key pitch: its display is narrower than its key row), and
  that the case ends shortly below the bottom key row.
- **Measured.** The 48SX, 49G and 38G were measured from straight-on
  photographs of the owner's own calculators (case, key wells and caps, the
  plates and zones of the case, colours); the 48GX shares the 48SX's
  mould and takes its geometry. The 39G/40G, and the labels of every
  model, come from the keyboard line drawings in HP's manuals, rendered as
  images and measured in pixels. The unit is 1/100 of the menu-key pitch;
  the origin is the case's top-left corner.
- **Wells.** On the 48SX, 48GX and 38G every cap sits in a dark recessed well
  a little larger than the cap (widest around the 48SX's light menu keys);
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
- **Drawn freely.** The relief, one light from the top left (decision
  log, "Skin depth"): gradients defined once in the SVG (a light falling
  on each cap, a rim lit above and shaded below, a light across the case,
  an edge lit on the top left and shaded on the bottom right), a shadow
  under each cap; a pressed key moves down onto its shadow and darkens.
  The case casts a shadow on the page and has a rounded outer edge and,
  where the plastic has one, a grain (`texture`, an `feTurbulence`
  filter blended `overlay`); every other panel is flat, raised (a thin
  lit edge) or sunk (a shaded edge) by its `relief`; the glass is sunk
  into its bezel by an inset shadow on the `.glass` element over the
  canvas. Fonts, the saturnus logo where the HP logo was, the model name
  as plain text.

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
event (the 48SX wakes for nothing every half second), the page sleeps on
without a frame. No counters are updated per frame; the status line changes
only on events.

Keys: every press is held for at least 60 ms of emulated time. A queued
press starts 30 ms after the previous release and once the ROM has gone
idle again (the 48SX ROM drops a key pressed while it is still handling the
last one, and stays awake up to about 750 ms after some keys); while
the ROM stays busy, as in a running program, a queued press waits at
most a time per model: 850 ms on the 48SX, 600 on the 48GX, 550 on the
49G, 1100 on the 38G, 300 on the 39G, 40G and 42S. A key you keep holding (ON for a chord)
does not block the next press.

All of this runs in the Worker, not on the page's thread, in the
protocol's state machine (`crates/saturnus-host/src/protocol/`, through
the wasm bindings), which `worker.js` feeds with messages and one timer:
"animation frame" above is a pass on a ~60 Hz timer, which stops while the
page is hidden as an animation frame would (the page sends `visibility`);
the wake timer keeps running. The page draws a `frame` event on its next
animation frame. The desktop app runs the same state machine on its
machine thread (`crates/saturnus-drive/src/runner.rs`), with passes every
millisecond (`protocol.md`, "Pacing").

`window.saturnus` exposes the backend and the store, `screenText()` (the
LCD as `#` and `.` lines), `loop` (`frame`, `sleep` or `stopped`), the
speed setting, `stats()` (the host's cycles, emulated ms, busy wall time,
passes and wakes, and the emulated time it owes to the wall clock) and
`skinKey(name)` for debugging and automated checks.

## Tour

"Show me around" (an action in Search, a button in About) is a short
tour of the page: one element at a time is ringed, with a bubble of one
or two sentences beside it (a sheet at the bottom on a phone), Back,
Next, "Skip tour" and a count ("3 of 7"). Esc ends it, and the focus goes
back where it was. Three chapters, each also a Search action of its own
("Tour: The calculator", found by a search): Getting started (no ROM
needed: the panel, the ROMs and where to get them, Search, the keyboard
shortcuts), The calculator (keys, paste, shift-click, Edit, screen
images, fullscreen), The memory view (offered while a 48SX, 48GX or 49G
runs). The last step of a chapter offers the next. On a first visit a
quiet line on the stage offers the tour; × dismisses it, and the answer
is kept.

The tour shows and never changes the calculator: it opens the panel, the
ROM list and the memory view's tabs for a step (app.js `tourHooks`) and
puts them back as they were when it ends; nothing is typed, stored or
purged. The steps are data in `tour.js` (`{id, anchor, title, text,
setup, when}`): an anchor selector, the words (`{key:palette}` becomes
the binding's label), what to open, and the conditions (host, pointer,
phone, Mac). A step whose anchor is missing, hidden or covered is
skipped and leaves the count. `web/test/tour.test.mjs` tests the steps
and the run; `web/test/tour-page.test.mjs` the page, and that every
step's anchor is seen in light and dark, at 1280 and 390 px, in the
browser and the app, so a change that breaks a step fails.

## About

"About saturnus and its sources" in the panel opens `<sat-about>`: the
project statement (clean room, MIT, AI notice, no ROMs, not affiliated
with HP) and every source the emulator was built from, read from
`about.json`. That file is generated from the hardware wiki's source pages
(calculator-knowledgebase, https://github.com/ractive/calculator-knowledgebase,
a separate repository checked out at `~/devel/calculator-knowledgebase`) by
`scripts/about-json.py` (or `just about`), which reads their frontmatter
through `hyalo`: title, authors, year, URL or archive location, and the
wiki pages that cite each source. The dialog folds that list
(Literature) and shows the ROM text as short paragraphs. Rerun the
script after the wiki gains a source; the JSON is committed.

## Stored data

What the page stores: the model (localStorage `saturnus.model`), the speed (`saturnus.speed`), whether
the side panel is hidden (`saturnus.panel`), whether the memory view is
open and its tab (`saturnus.layer`, `saturnus.layerTab`), the widths of
the side panel, the memory view and its directory tree
(`saturnus.panelWidth`, `saturnus.layerWidth`, `saturnus.treeWidth`), the changed keyboard shortcuts (`saturnus.keys`,
only the changes), whether the tour was taken or its offer dismissed
(`saturnus.tour`), the saved states (IndexedDB
database `saturnus`, store `states`: the user's under the model's name,
the auto-saved one under `auto:<model>`) and the ROMs (IndexedDB database
`saturnus-roms`: store `slots` with each model's file name, SHA-256 and
revision and the last model, store `images` with the bytes by SHA-256, so
the 39G and 40G share one copy). Remove ROMs empties both stores but the
settings record, and deletes both saved 49G states (they contain the
49G's 2 MB ROM); the other saved states stay.
