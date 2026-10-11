---
title: "saturnus-tx: moving files between calculators and a computer"
type: docs
date: 2026-10-10
status: draft
tags:
  - saturnus-tx
  - design
  - kermit
  - web
---

# saturnus-tx

A web page and a desktop app that move files between a real calculator on
a serial cable and the computer, in the spirit of HPComm (the owner's
1999 Windows program, `~/devel/hpcomm`): the calculator's directory tree
beside the computer's files, drag and drop between them. It lives at
<https://ractive.ch/saturnus/transfer/>. The protocol is this
repository's `satx-core` (with `kermit-proto` and `xmodem-proto`, the
library behind the `satx` command-line tool), compiled to WebAssembly;
the page is built from saturnus's own web components; saturnus itself can
be one of the devices. Owner decisions of 2026-10-10 in [[decision-log]].
Iterations: [[iterations/iteration-37-tx-skeleton]],
[[iterations/iteration-38-tx-two-panes]],
[[iterations/iteration-39-tx-aplets]],
[[iterations/iteration-40-tx-desktop]].

## What HPComm was, and what carries over

HPComm 3.0 (shipped by HP in the 1999 PC Connectivity Kit) had one window
in four parts: the PC's folder tree and file list above, the
calculator's HOME tree and variable list below (columns Name, Size,
Checksum), a toolbar with connect, refresh, screen capture, delete, cut,
copy, paste and the list views, and drag and drop between the halves
(README and `docs/screenshots/ss1.png` of that repository). HPGComm, its
sibling for the 38G, 39G and 40G, was a small window instead: the
calculator drives every transfer, so the PC side only picks a folder and
a serial port and waits ("press SEND or RECV on the calculator").

Carried over: the two places side by side, drag and drop, size and
checksum in the list, the two different shapes for the two families
(the 48/49 are servers the computer drives; the 38G/39G/40G are clients
the computer serves). Not carried over: the toolbar of 1999 (actions sit
on rows, in the "⋯" and context menus, as in saturnus's memory view),
cut and paste between the halves (drag, or "Copy to…"), the Windows-only
everything.

## Goals

- Get, put, rename, purge and new directory on a real 48S/SX, 48G/GX or
  49G over a serial cable, from Chrome or Edge with nothing installed,
  and from a desktop app on macOS, Windows and Linux built from the same
  web code.
- The 38G, 39G and 40G: send and receive aplets, the calculator driving
  the transfer, the computer serving a folder.
- Backup and restore of a whole calculator.
- See what a variable is before taking it: text, numbers, lists, GROBs as
  pictures, the size and checksum the calculator reports.
- An emulated calculator (saturnus in the same page) as a device, so a
  file goes from the emulator to a real calculator and back, and so CI
  has a calculator without hardware.
- Plain English in the interface, no protocol words unless asked for
  (no "Kermit", "packet", "IOPAR" in the main flows; the settings name
  them).

## Non-goals

- **The 42S.** saturnus emulates it, but it has no serial port (its IR
  only prints, one way, to the 82240 printer). The device list says so
  instead of offering it.
- The 49g+, 50g and later USB models: saturnus does not emulate them and
  their USB link is another protocol.
- **XSERV** (owner, 2026-10-10): Kermit works on every supported model,
  and XSERV belongs to the 49g+/50g. It can come back if the owner ever
  has one of those.
- Infrared transfers (calculator to calculator, IR printing).
- Flashing a 49G ROM (`ROMUPLOAD`): one wrong step and the calculator
  needs recovery. Out of scope for good.
- Editing objects in place (saturnus's palette editor). A later idea:
  get as text, edit with `rpl-editor`, put back.
- A terminal, a REPL, scripting: that is `satx`, the command-line tool on
  the same core.
- HP's name or logos in the product. "Works with the HP 48, 49, 38, 39
  and 40 calculators" in running text is fine ([[docs/clean-room-rule]]).

## Devices

A **device** is a calculator, real or emulated; a **folder** is a place
on the computer. A pane shows one **place**: a device or a folder.

| Model | Link | Who drives | Operations |
| --- | --- | --- | --- |
| 48S/SX | Kermit | the computer (calculator in `SERVER`) | list, get, put, rename, purge, new directory, backup, restore |
| 48G/GX | Kermit, XModem started on the calculator | the computer | as the 48SX; XModem as a faster put/get option typed on the calculator |
| 49G | Kermit, XModem (1k blocks) | the computer | as the 48GX |
| 38G | Kermit, calculator as client | the calculator (SEND, RECEIVE) | send aplets to a folder, receive aplets from a folder |
| 39G, 40G | Kermit as the 38G; XModem for binary aplets | the calculator | as the 38G; binary aplets by XModem (HPGComm's README: RECV, "HP39/40 (Wire)") |
| 42S | none | | not offered |

Facts: wiki `protocols/kermit-hp`, `protocols/server-commands`,
`protocols/xmodem-hp`, `protocols/xserv`, `protocols/iopar`,
`questions/hp38g-39g-transfer-protocol`, `hardware/hp39g-40g`.

Three kinds of link carry the bytes, all behind one JavaScript interface
(`Link`: `open(settings)`, `write(bytes)`, a stream of received bytes,
`close()`):

- **Web Serial** (`navigator.serial`): Chrome, Edge and other Chromium
  browsers on the desktop. The user picks the port in the browser's
  chooser (a click is required); `getPorts()` brings back a port the page
  was given before, so a second visit connects in one click.
- **Native serial** in the desktop app: the `serialport` crate behind two
  Tauri commands and an event, the same `Link` to the page.
- **The emulator**: saturnus's wasm core in its Worker, its emulated
  UART's bytes in and out through two new commands of the front-end
  protocol (`serialWrite`, event `serial`; [[iterations/iteration-37-tx-skeleton]]).
  The transfer code cannot tell it from a cable, which is what makes it
  the test double.

The 48/49 operations run the same code whatever the link; the emulator
is not special-cased. A later shortcut for the emulator (its memory read
straight from RAM, writes as saturnus's hidden transactions, no `SERVER`
needed) is the shared device layer below.

### Two devices at once

Either pane may show a device, so "emulator left, real calculator right"
is a normal layout. A copy between two devices is a get from one into
memory and a put to the other, never a byte bridge between the two
links, so each side keeps its own settings and its own errors. A 48 file
going to a 49G (or back) is warned about first, as HP's own kit did
(`protocols/xmodem-hp`: Conn4x warns on a header mismatch); the 49G reads
most 48 objects, the 48 reads few 49G ones.

## Layout

Two panes side by side, each with a tree on the left and a list on the
right, as the memory view's tree and list; on a narrow window (below
760 px, saturnus's breakpoint) the panes stack and one shows at a time
with a switch at the top. Each pane has a header naming its place
("HP 48GX on USB serial", "Emulated 49G", "Folder hp-files"), a button
to change it, and the status of the link (connected, busy with a
progress line, disconnected with why).

- **The list**: name, type, size in bytes as the calculator counts them,
  checksum (`#` and hex, as the calculator's BYTES shows it), and for a
  folder the modified date. Sort by column. Directories first.
- **Drag and drop** from one pane to the other, from the computer's file
  manager onto a pane (files and, in Chromium, folders), and keys for the
  same (Copy to the other pane, F5 as in two-pane file managers; Delete;
  F2 to rename). Dragging out to the desktop is Chromium-only
  (`DownloadURL`) and optional.
- **Preview** below or beside the list (the memory view's preview panel):
  the selected object's text or picture, its size, checksum and type.
- **Actions** on the row, in the "⋯" menu and the context menu
  (`components/menu.js`), the same set the memory view offers where it
  makes sense: Get (to the other pane's folder, or a download), Rename,
  Purge, New directory, Backup, Restore, Show as text.
- Confirmations through `components/confirm.js` (one modal before what
  cannot be undone: purge, replace, restore), results through
  `components/toast.js`, notes through `components/note.js`.
- Theme, tokens, type sizes, target sizes and the safe areas are
  saturnus's (`style.css` tokens); `prefers-color-scheme` and the same
  theme switch. No HP logo; the app's own icon.

### The local folder

Three implementations of one `Folder` interface (list, read, write,
rename, remove, make directory, a change notification where there is
one):

- **File System Access API** (`showDirectoryPicker`, read and write
  permission asked when first writing): Chrome and Edge on the desktop.
  The handle is kept in IndexedDB, so the next visit asks only for the
  permission again.
- **Native** in the desktop app: a folder chosen with Tauri's dialog,
  read and written through commands scoped to that folder (the webviews
  of Tauri, WKWebView and WebKitGTK, have no File System Access API).
- **Fallback, uploads and downloads**: Firefox and Safari (and Chrome on
  Android) have no directory picker. The pane is then a "Files" list held
  in the page (OPFS where present, memory otherwise): files come in by
  the file input or a drop, go out as downloads (one file, or a `.zip`
  for a directory or a selection). Web Serial is missing in exactly these
  browsers too, so this fallback serves the emulator-only use; a real
  calculator in them means the desktop app.

Web Serial and the File System Access API ship in the same browsers. So a
user with a cable in a browser always has a real folder; the fallback is
for looking at files and the emulator.

Never overwritten without asking: a name that exists in the target asks
"Replace X?" first (Cancel focused), on the folder side as on the
calculator side.

### File names

A variable becomes `NAME.hp` (binary transfer file, `HPHP48-x` or
`HPHP49-x` header) or `NAME.txt` (the calculator's text form, `%%HP:`
header) on the computer. HP characters outside ASCII become their
Unicode equivalents (`→`, `α`, `Σ`; `saturnus-objects`'s `charset`),
which every file system takes; the few a file system refuses
(`/ \ : * ? " < > |`) are written `%XX` and read back the same way. A
directory becomes a folder by default; "Get as one file" keeps it one
directory object (`.hp`), which is what a restore or another calculator
wants. A put takes the name from the file name without the extension
and asks for a new one if the calculator would refuse it (`satx-core`'s
`validate_name`).

## Flows

### Connect (48/49)

1. "Add a calculator": the dialog says what to do on the calculator,
   per model, with the keys drawn as saturnus draws them (48G/GX:
   right-shift then →; 48S/SX: type `SERVER`, ENTER; 49G: right-shift
   and → pressed quickly, wiki `protocols/xserv`), then "Choose the port"
   opens the browser's chooser.
2. The page opens the port at 9600 baud, 8 data bits, no parity, one stop
   bit, no flow control, and syncs (`satx-core`'s `Calculator::sync`)
   once the search below has heard the calculator.
3. **Speed and parity found by probing** (`saturnus-tx`'s search): at
   each speed (9600, 4800, 2400, 1200) a Kermit I packet, which the
   server only ACKs, goes out framed for no, odd and even parity 500 ms
   apart (bit 7 as the parity bit; a calculator with parity checks it on
   receive), and the page listens 6 s (the probes again halfway) for the
   ACK or an idle NAK. The packet heard also tells the parity (bit 7 of
   each byte). Probing, not only listening, because a server need not
   NAK while idle: saturnus's emulated server sends one NAK per server
   timeout and then waits (measured 2026-10-11, all three models). Then
   it says what it found ("The calculator talks at 2400 baud"). Manual
   settings stay available.
4. **Model**: `VERSION` as a host command (`satx-core`'s
   `Calculator::model`): no such command is the 48S/SX, `HP48-x` 1993 the
   48G/GX, a 1999 or later copyright the 49G. The 49G in algebraic mode
   is **not** switched to RPN: Kermit list, get and put work as they are
   (host commands are RPN inside the server; the 49G trace of
   `saturnus-tx`, `crates/saturnus-tx/traces/49g-session.trace`, runs the
   whole session on a fresh 49G, which is in algebraic mode). One side
   effect the page must say at disconnect: a server entered from
   algebraic mode leaves the stack packed in a list after FINISH
   ([[decision-log]], iteration 12a and "49G writes in algebraic mode").
5. **Transfer format**: binary (flag -35 set) for exact copies; the page
   remembers the calculator's setting and restores it when it
   disconnects, so the user's own transfers behave as before.
6. Then the HOME tree loads (`G D` per directory, opened on demand, never
   the whole tree at once: a listing on a 48SX at 9600 baud takes about a
   second per directory).

Disconnect sends FINISH (the server ends and writes IOPAR, see
`protocols/iopar`) and says "The calculator has left server mode."
A cable pulled mid-way is a disconnected pane with the reason, never a
hang: every operation has the session's timeouts.

### Transfer settings

The settings dialog (the pane's "⋯", "Connection settings…"): speed
(1200, 2400, 4800, 9600), parity (none, odd, even), "Find automatically"
(default on), and the calculator's own settings as read after connect
(IOPAR: speed, parity, checksum, translation). "Set the calculator to
9600 baud" stores IOPAR (reals on the 49G, `protocols/iopar`) and says
it takes effect at the next `SERVER`. Mark and space parity, which IOPAR
allows, are not offered: neither Web Serial nor the `serialport` crate
has them.

### List, get, put

- **List**: the tree and list of the current directory; directories
  open on demand; refresh by a button and after every change made from
  the page. Changes made on the calculator's keyboard while connected are
  not seen until refresh (the server is busy serving).
- **Get**: binary `.hp` by default; "Get as text" asks the calculator for
  its text form (flag -35 clear for the one transfer, then back), which
  is also how a program's source is shown.
- **Put**: a binary transfer file as is; a `%%HP:` text file in text
  mode (the calculator compiles it; a syntax error is its error, shown);
  any other file asks "Store it as a string?" (the calculator would).
  Into the directory the pane shows.
- One operation at a time per device (the link is half duplex); more
  queue with a visible count and can be cancelled before they start. A
  host command is sent once and never resent (`satx-core`'s rule: a
  resent command would run twice).
- Progress: bytes of total for get and put, from the session's packet
  events.

### Rename, purge, new directory

Host commands through `satx-core` (`rename`, `remove`, `mkdir`), the same
refusals as the memory view (HOME, reserved names, a name in use; a
non-empty directory is purged with `PGDIR` after a modal question
naming how many variables go with it).

### Backup and restore

- **Backup** (48/49): `satx-core`'s `backup` (ARCHIVE to `:0:`, get,
  purge the port object; `protocols/server-commands`), saved as
  `48GX-backup-2026-10-10-1432.hp`. It needs about as much free memory
  as HOME holds; the dialog shows the free memory (the `G D` header on
  the 48GX and 49G, `MEM` on the 48SX) and refuses early when it cannot
  fit, offering the alternative:
- **Copy everything to a folder**: the whole HOME tree as folders and
  `.hp` files, one get per variable. Slower, needs no free memory, and
  a folder is easier to look through than one archive.
- **Restore**: a backup file (or a directory object) put to `:0:` and
  RESTOREd: a modal question first ("Replace everything on the
  calculator?"). The calculator warm-starts and leaves server mode; the
  pane says so and offers to reconnect once `SERVER` runs again.

### Aplets (38G, 39G, 40G)

The calculator is the client: the pane is a folder served as the
"disk drive" the calculator expects. "Serve a folder to a 38G/39G/40G"
picks the folder and the port, and the page answers the calculator's I
and R packets (wiki `questions/hp38g-39g-transfer-protocol`: it asks
first for `HP38DIR.CUR` or `HP39DIR.CUR`, the directory file). SEND on
the calculator lands aplets in the folder; RECEIVE lists the folder's
aplets on the calculator. The model is told by the directory file it
asks for (38G against 39G/40G; the 39G and the 40G are not told apart,
and need not be). The directory file's format is **not known yet**; it
is the first task of [[iterations/iteration-39-tx-aplets]], found on the
emulated 38G and 39G and written to the wiki before any code.

Binary aplets on the 39G and 40G go by XModem: RECV on the calculator,
"HP39/40 (Wire)", then the page sends (HPGComm's README, which is
documentation, not code).

### Preview

- **GROBs** as pictures: decoded by `saturnus-objects` (`graphic`) and
  drawn by the memory view's `graphic.js` (scale, thumbnails, PNG
  export, copy).
- **Data objects** (reals, complex, strings, lists, arrays, tagged,
  units, binary integers) as text: `saturnus-objects`'s decoder and
  `decompile`, which needs no ROM for them.
- **Programs and algebraics** need the ROM's command names
  (`saturnus-objects`'s `NameTable`). Two ways: the calculator's own
  text ("Show as text" gets it in text mode, one extra transfer), or a
  ROM of the same model the user already keeps in the emulator: both
  pages are on ractive.ch, one origin, so the transfer page can read the
  emulator's ROM slots (read only) and decompile locally, with the
  calculator's text as the fallback.
- **Files** on the computer: the same decoding from their bytes; `%%HP:`
  text shown with the trigraphs as characters.
- PICT (the graphics screen) through `satx-core`'s `pict`: "Graphics
  screen" in the device's "⋯", shown as a GROB.

## Crates and folders

```text
crates/satx-core           (exists) the protocol: sessions, list, get,
                           put, host commands, backup, restore, PICT,
                           XModem, XSERV; gains a sans-I/O API and
                           features for its native-only parts (below)
crates/saturnus-tx         host-neutral, no I/O, builds for wasm32:
                           device operations over satx-core (connect and
                           detect, list, get, put, rename, purge, mkdir,
                           backup, restore), the speed and parity search,
                           the aplet "disk drive" server (iteration 39),
                           file names, the copy-between-devices plan;
                           unit tests over recorded traces
crates/saturnus-tx-web     wasm-bindgen over saturnus-tx and the parts of
                           saturnus-objects the preview needs; drives a
                           Link given by JavaScript; publish = false
crates/saturnus-tx-tauri   the desktop app (iteration 40): the transfer
                           page, native serial (`serialport`) and folder
                           access as Tauri commands; publish = false
web/tx.html                the transfer page (/saturnus/transfer/)
web/tx/                    its own modules and components (panes,
                           devices, links, folders, dialogs)
web/shared/                what both pages use: components (menu,
                           confirm, toast, note, icons, resize), dom
                           helpers, theme, tokens.css, graphic, the
                           object text forms, model titles and order
web/  (as now)             the emulator (/saturnus/emulator/), importing
                           from ./shared/
web/landing/               (iteration 36d) the landing page /saturnus/
```

Why this shape:

- **satx-core stays the one protocol library** for `satx` and the
  transfer page. What the page needs from it: an API that runs in a
  browser without blocking (today `Transport::read` waits with a timeout
  and `Session` and `Calculator` are synchronous; sans-I/O, driven by
  bytes and a clock, decided 2026-10-11 in [[decision-log]], with the
  blocking form kept as a thin wrapper for the CLI), and features for
  its native-only parts, off for the page: `serialport` and TCP behind
  the default feature `native`, the in-process emulator transport
  (`saturnus-core`, `saturnus-drive`) behind the opt-in feature
  `saturnus`, which `satx` turns on and which would otherwise pull the
  native emulator into the transfer wasm. Done as the first task of
  iteration 37 (open question 1, resolved).
- **saturnus-tx apart from its bindings** repeats the saturnus-host /
  saturnus-web split that already works: the logic is tested natively
  with `cargo test`, the bindings stay thin, and the desktop app can run
  the same logic natively if the IPC hop ever matters.
- **Its own wasm, not saturnus-web's.** The transfer page should not
  download the emulator (0.8 MB of wasm, plus the skins) to talk to a
  cable; the emulator's wasm is loaded only when the user adds an
  emulated device. Two packages, two `wasm-pack` builds, one CI job.
- **The desktop app runs the protocol in the page's wasm too**, and
  native code only moves bytes and files. One protocol path for web and
  desktop means one set of tests and identical behaviour; at 9600 baud
  the IPC hop (well under a millisecond) is irrelevant. saturnus chose
  the opposite (a native core in Tauri) for emulation speed, which the
  transfer app does not need. Revisit only if a measured transfer shows
  otherwise.
- **`web/tx.html` beside `web/index.html`**: both pages sit at the same
  depth, so `./shared/x.js` resolves the same in development (one static
  server over `web/` serves both) and in each assembled subsite
  (`site.sh` copies `tx.html` as `transfer/index.html`, with `tx/` and
  `shared/` beside it, as it copies the emulator into `emulator/`). A
  page folder outside `web/` would need its imports rewritten when
  assembled, or an import map, which module Workers do not honour. The
  emulator's files stay where they are: moving them to `web/emulator/`
  is churn with no user-visible gain.

### What moves to `web/shared/`

The move is mechanical but touches the emulator, so it is the first
commit of [[iterations/iteration-37-tx-skeleton]] after the `satx-core`
work, reviewed on its own: no behaviour change, all web tests and the
overflow audit green, the emulator page unchanged to the eye.

- Components: `menu.js`, `confirm.js`, `toast.js`, `note.js`,
  `icons.js` (and the SVG sprite, out of `index.html` into a shared
  file both pages inline at build time or fetch once).
- Modules: `theme.js`, `theme-boot.js`, `models.js`, `resize.js`,
  `radiogroup.js`, `disable.js`, `graphic.js`, the PNG helpers of
  `screenshot.js`, the text forms of `objects.js`.
- The tokens and base rules at the top of `style.css` into
  `shared/tokens.css`.
- Untangled on the way: `el` lives in `components/entry-view.js` (which
  pulls in the command reference) and `MODEL_TITLES` in
  `components/sat-calculator.js` (which pulls in the skin); `el` moves to
  `shared/dom.js`, `MODEL_TITLES` beside the model order in
  `shared/models.js`.
- Not moved: the palette, the editor (`rpl-editor.js`, `editor.js`
  depend on `commands.json`), the explorer. The transfer page does not
  need them yet.
- Follows from the move: `web/site.sh`'s page file rule takes
  `shared/**`, `saturnus-tauri`'s `build.rs` copies it, its test
  `frontend` still compares the two lists, the emulator's service worker
  precaches it (it lists what `site.sh` assembles).

### The shared device layer (later)

The transfer page's tree and list are new shared components
(`<device-tree>`, `<device-list>` in `web/shared/components/`) over a
`Device` interface (`list(path)`, `read(path, name, {text})`,
`write(path, name, bytes)`, `rename`, `purge`, `mkdir`, a change event).
Two implementations exist from the start: `KermitDevice` (any `Link`) and
the test fake. The memory view keeps its own tree and list for now: it
is 2400 lines with the stack, the flags, the editor and the reference
woven in, and rewriting it under the transfer page's first iterations
would put the emulator's most-used panel at risk for no user gain.

Later (a backlog item after iteration 40, not planned here): an
`EmulatorDevice` over the memory view's own reads and writes
(`memory.js`, `writes.js`: RAM reads, hidden transactions), and the
memory view's tree and list replaced by the shared components. Then the
emulator's memory view could show a real calculator through Web Serial,
and the transfer page could show the emulator instantly, without
`SERVER`, from RAM.

## URL and deploy

- **<https://ractive.ch/saturnus/transfer/>**, beside the emulator at
  `/saturnus/emulator/` and the landing page at `/saturnus/`
  ([[iterations/iteration-36d-site]]). Its service worker's scope is
  `/saturnus/transfer/`, its caches `saturnus:/saturnus/transfer/:<build>`
  in the emulator's naming, untouched by the kill switch at
  `/saturnus/sw.js` (which deletes only the `saturnus:/saturnus/:`
  caches).
- `web/site.sh` (which builds all of `/saturnus/` since 36d) gains the
  `transfer/` subsite: `tx.html` as its `index.html`, `tx/`, `shared/`,
  the transfer wasm package, saturnus-web's package and the Worker files
  the emulated device loads, its own marker, manifest, icons and
  `sw.js`, a BUILD hash over `transfer/` only, the import check. A
  `site.sh --list transfer` gives the desktop app its file set, as
  `--list` does for the emulator's. The one `pages.yml` deploy to
  `httpdocs/saturnus/` carries it.
- One origin, on purpose: the transfer page reads the emulator's kept
  ROMs (IndexedDB) for the emulated device and for program text, read
  only, tolerating a schema it does not know. Its own IndexedDB names,
  localStorage keys (prefix `saturnus.transfer.`) and cache prefix go
  into the storage-names module and its test (36d's
  `web/test/storage-names.test.mjs`), disjoint from the emulator's; the
  theme choice is the one key both read.
- The desktop app: [[iterations/iteration-40-tx-desktop]].

## Testing

Following [[docs/test-policy]]: fast tests in CI, ROM-gated tests locally,
hardware by the owner.

1. **Rust, ROM-free, CI** (`saturnus-tx`): every operation over
   `satx-core`'s in-memory transport replaying recorded traces,
   including the failure cases (a NAK storm, a cut link, `Insufficient
   Memory`, a refused name). The traces are recorded from saturnus by a
   `just` recipe gated on `SATURNUS_ROM_DIR`, and committed; they hold
   protocol bytes and listings, no ROM content.
2. **JavaScript, ROM-free, CI** (`node --test web/test/tx-*.test.mjs`):
   the pure modules (file names, the speed search over a fake link, the
   copy plan, which actions a row offers, the folder implementations over
   fakes), as `actions.test.mjs` does for the memory view.
3. **The page in headless Chrome, ROM-free, CI**: the transfer page with
   a scripted fake link (replies from the same traces): connect, list, a
   drag from folder to device, the replace question, the overflow audit
   at 360, 390, 430, 768 and 1280 px as `overflow.test.mjs`.
4. **The page against the emulator, ROM-gated** (`just tx-e2e DIR`): the
   transfer page with saturnus-web's emulator as the device over the new
   serial commands, 48SX, 48GX and 49G (38G and 39G from iteration 39):
   connect, detect, list, get, put, rename, purge, new directory, backup
   and restore, emulator to folder and back byte for byte. Never in CI
   (owner, 2026-10-10: fetching ROMs in CI is over-engineered): CI
   replays the recorded transfers (1 to 3), the emulator run is local,
   where the ROMs are.
5. **Native, ROM-gated** (iteration 40): `saturnus-tx-tauri`'s serial
   link over a pseudo-terminal pair (`serialport`'s `TTYPort::pair`,
   Linux and macOS) to saturnus-drive's runner with its serial bridge.
6. **Real hardware, the owner**: the owner has a 48SX, a 49G and a 38G
   (no 48GX, 39G or 40G), and the 38G's 10-pin cable, so the 38G is
   tested on hardware too. Each iteration ends with an owner checklist on
   them in Chrome and in the desktop app. The 48GX, 39G and 40G stay
   "tested on the emulator only" until someone with one reports.

## Clean room

The transfer app implements no emulator: its calculator knowledge is the
wiki's protocol pages and `satx-core` (this repository, MIT).
[[docs/clean-room-rule]] applies as it does to the emulator, and one more
source needs a rule:

- **HPComm, HPComm38 and HPGComm are GPL-2**, mainly the owner's code
  (Mitch Davis of HP and Colin Croft wrote parts), and saturnus is MIT.
  Owner (2026-10-10): their source is a documented exception, as Conn4x
  is in calculator-knowledgebase's rules (its CLAUDE.md lists both):
  facts only, no code, no routine names, no layouts copied. Protocol and
  file-format facts (the 38G/39G directory file, the XModem aplet steps)
  go into the wiki with a citation, and the app is built from the wiki;
  an implementing session does not open the source. Their READMEs, help
  pages, website and screenshots are documentation and may shape the
  look and the flows.
- HP's Connectivity Kit binaries: black box only.
- No HP name or logo in the product name, icon or chrome.

## Risks

- **satx-core is not wasm-ready today**: its transport blocks, its
  session is synchronous, and it always pulls `serialport` and the native
  emulator. Iteration 37 cannot ship before the non-blocking API and the
  features exist; changing a working CLI's core is also a risk to `satx`,
  so its own tests and e2e suite gate the change.
- **Web Serial exists in Chromium on the desktop only.** Not in Firefox
  or Safari, not on iOS; Chrome on Android is not relied on. The page
  says so plainly and points to the desktop app; the emulator still works
  everywhere.
- **Timing in the browser.** The protocol runs in a dedicated Worker so
  a background tab's timer throttling (one wake-up per second, and
  Chrome's intensive throttling after five minutes hidden) cannot stretch
  the session's pauses into the calculator's own timeouts; the port is
  chosen in the page (the chooser needs a click) and used in the Worker,
  either directly where `SerialPort` is exposed to workers or by
  transferring its streams. Iteration 37 measures a transfer in a hidden
  tab before deciding. The calculators' quirks are handled in
  `satx-core`: the 200 ms turnaround, a command dropped right after the
  final ACK, the idle NAKs to drain.
- **USB serial adapters.** Linux needs the user in `dialout` (`uucp` on
  Arch) for Chrome and the app alike; Windows refuses counterfeit
  Prolific chips with current drivers; the 38G needs its own 10-pin
  cable (the owner has one). The connect dialog's help names these.
- **The File System Access API** is Chromium-only (above); Tauri's
  webviews lack it (native folder access instead). Writing asks for a
  permission per visit.
- **The 38G/39G directory file's format is unknown** (wiki
  `questions/hp38g-39g-transfer-protocol`), and `kermit-proto` has no
  server side yet. Iteration 39 cannot start its code before both.
- **Backups need free memory**, restore warm-starts, and a 48SX with
  a large HOME may not be able to back itself up through `:0:`. The
  folder copy is the way out.
- **Two pages on one origin** share IndexedDB and localStorage: a schema
  change in the emulator's ROM store must not break the transfer page's
  read, and the transfer page must not write there. The storage-names
  test keeps the names apart.
- **Moving components** to `web/shared/` can break the emulator page or
  the desktop app's embedded file list; hence a separate,
  behaviour-neutral commit and the existing site and frontend tests.

## Open questions

None open. Both earlier questions are resolved:

Resolved: question 1, on 2026-10-11 ([[decision-log]], "satx-core goes
sans-I/O"): sans-I/O, as the first task of
[[iterations/iteration-37-tx-skeleton]], starting from draft PR 108.

Resolved: question 2, on 2026-10-11 ([[decision-log]], "the transfer
app's names"): "saturnus tx" in menus and window titles, bundle
identifier `ch.ractive.saturnus-tx`, Homebrew cask and Linux packages
`saturnus-tx`; the command line stays `satx`.

The app ships from the workspace's one tag, as decided on 2026-10-10
(consolidation).
