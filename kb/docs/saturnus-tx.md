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
on the computer. A pane shows one **place**: a device or a folder. The
left pane shows devices only (a calculator on the cable, or an emulated
one); the right pane shows a folder or a device (owner, 2026-10-11).

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

### Shared calculators

Since [[iterations/iteration-46-shared-worker]] there is one live
calculator per model on the origin, and both pages use it at once. An
emulated 48SX, 48GX or 49G started from a ROM the emulator keeps is the
emulator's own calculator of that model, running in the emulator's
SharedWorker (`web/shared-worker.js` over `web/hub.js`, opened by both
pages by the emulator folder's URL, `web/shared/calc-hub.js`). What the
user typed in the emulator is listed in saturnus tx at once, and what
saturnus tx copies there is in the emulator's memory and on its screen
as it happens. (Iteration 41 handed one calculator over between the
pages, one at a time; that handover is gone.)

- **Who owns what.** The hub owns the machines (one per model, made when
  a page first boots or attaches that model, running while any page
  shows it, saved by the engine's auto-save and once more when its last
  page leaves) and alone writes their states (`auto:<model>`). Each page
  is a connection: the machine it shows, its speed choice, whether it
  is hidden, whether it watches the memory. saturnus tx joins (`join`,
  its build), attaches (`attach` with the kept ROM; running already, it
  only shows it) and leaves (`leave`; a tab closed is noticed through
  its lifetime Web Lock).
- **HubDevice** ([[iterations/iteration-47-hub-device]]). saturnus tx
  works the emulator's calculator as a `HubDevice`
  (`web/shared/hub-device.js`) beside the `KermitDevice` of a cable or
  the private calculator: the same device interface (`list`, `read`,
  `write`, `rename`, `purge`, `mkdir`, `getTree`, `putTree`, `backup`,
  the `change`, `busy` and `progress` events), mapped onto the engine's
  memory commands (`memoryTree` once per `memoryChanged`, `fetchFile`
  with `text` for "Get as text", `storeFile` with `replace`, `rename`,
  `purge`, `createDir`, `backup`). They run as hidden transactions
  inside the engine, in emulated time, ordered with the emulator's keys
  and its own writes: nothing is typed on the calculator, its serial
  port is not used, and its screen never shows SERVER. A variable typed
  in the emulator shows in tx at once. A refusal (a command line open
  in the emulator, typing, another transfer, flag -33 on a 48) is said
  at once in tx's words ("The calculator is busy in the emulator:
  finish or cancel the command line there."), never waited out. It
  lives in `web/shared/` and takes any object with `request` and the
  protocol's events, so the emulator page could use it in a merged app.
- **Two hands.** Keys from both pages go to the calculator as they come;
  the memory commands of both pages run one after the other in the
  engine. The serial port of a shared calculator is nobody's:
  `serialOpen` and `serialWrite` from a page that shares it are refused
  ("This calculator is shared with another page, so its serial port
  cannot be opened here. Use a calculator of its own (a ROM file chosen
  in saturnus tx)."; owner, 2026-10-14).
- **What it does not offer** (owner, 2026-10-14): Restore (the
  emulator's Load state covers it; the pane's "⋯" says so), the
  graphics screen (the "+" above the emulator's display: screen image,
  Store screen) and the fast transfer (a hidden transaction takes 0.15
  to 0.25 s of wall time; nothing to speed up). No free-memory figure
  (the memory tree has none): "Back up" runs the engine's `backup`,
  which checks the room itself and refuses with the bytes needed and
  free. They stay for a cable and the private calculator.
- **The emulator's side.** While a tx page is on the calculator it
  shows, the emulator says "Connected to saturnus tx · Show" under its
  status line (the hub's `peers` event); Show brings tx's tab forward.
  There is no Disconnect there: tx disconnects in tx. A `.hp` (or
  `%%HP:` text) file dropped on the drawn calculator is stored in its
  current directory.
- **Never left in SERVER.** A calculator saved in SERVER (by an older
  build, or a page closed while its private calculator served) comes
  back at its stack: the hub ends the server when it boots a kept state.
- **Speed.** The calculator runs at the emulator page's speed (tx's
  when no emulator page shows it). The memory commands run as fast as
  the host can at any speed.
- **Versions.** Both pages and the SharedWorker must be one build:
  `web/site.sh` writes one build id (a hash of the worker and what it
  loads) into `shared/build.js` of both folders, and the hub refuses a
  page of another build ("This page's files are from different
  versions. Reload the page."); the emulator page then runs on its own
  in a plain Worker. A state whose format is newer than the
  engine's is not booted ("This calculator was saved by a newer version
  of saturnus. Reload this page to use it.").
- **A ROM file** chosen in the dialog starts a private calculator in
  saturnus tx's own Worker (`emulator-worker.js`) that keeps nothing, as
  in the desktop app (the two desktop apps are separate processes and
  share nothing).
- **No SharedWorker** (Samsung Internet): the emulator runs in a plain
  Worker (the same hub, one page) and saturnus tx says it cannot share
  the emulator's calculator in this browser.

On a cable and on the private calculator the 48/49 operations run the
same code whatever the link: the private calculator types SERVER and is
the honest test of the cable path. The emulator's own calculator is the
`HubDevice` above (its memory read straight from RAM, writes as
saturnus's hidden transactions, no `SERVER` needed).

### Two devices at once

Either pane may show a device, so "emulator left, real calculator right"
is a normal layout (a folder, by contrast, opens only in the right
pane). A copy between two devices is a get from one into
memory and a put to the other, never a byte bridge between the two
links, so each side keeps its own settings and its own errors. A 48 file
going to a 49G (or back) is warned about first, as HP's own kit did
(`protocols/xmodem-hp`: Conn4x warns on a header mismatch). As built
(iteration 38): a binary file with the other family's header is stored as
a string (`protocols/hp-object-format`), so a copy across families gets
each variable's text form and the target compiles it (`saturnus-tx`'s
`copy` plan); a folder's binary file of the other family is warned about
as arriving as a string.

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

As built (iteration 38, `web/tx/pane.js`, `app.js`): the preview sits
under each pane's list, not beside it, since two panes side by side
leave no room for a third column; a narrow pane (a container query, below
540 px) shows a path line instead of the tree. The pane's head has
"Change" (the place switcher: on the left a calculator on a cable or an
emulated one; on the right also a folder on this computer or, where no
folder can be picked, "Files in this browser"; then nothing). A pane
that shows the emulator's own calculator also has "Open in the
emulator" (iteration 45, below). No menu,
card or later visit opens a folder in the left pane; a kept folder
comes back in the right pane. The head also has "⋯" (a calculator: New directory, Back up, Restore from a backup file, Graphics
screen, Copy everything to a folder, the connection settings, Read
everything again, Disconnect; a folder: New folder, Add files, Download
everything as `.zip`, Read again, Close). A row's menu: Open, Copy to the
other pane (F5), Download and Download as text (a variable), Copy as one
file (a directory), Download or Download as `.zip` (a folder's), Restore
the calculator from this file (a folder's `.hp`, with a calculator in the
other pane), Rename (F2), Purge or Delete (Delete). Ctrl or Cmd and
Shift with a click select more than one row, Ctrl/Cmd+A all of them.
Everything that runs is a job of one queue (`web/tx/queue.js`), listed
under the panes ("Transfers") with its progress in bytes; one job per
place at a time, in the order asked, the waiting ones cancellable one by
one or all at once. A drop lands in the directory whose row it is on,
else the one shown. `DownloadURL` drags out only rows already previewed
(the bytes must be in the page when the drag starts).

### The local folder

Three implementations of one `Folder` interface (list, read, write,
rename, remove, make directory, a change notification where there is
one):

- **File System Access API** (`showDirectoryPicker`): Chrome and Edge
  on the desktop. The handles of the last 8 folders picked are kept in
  IndexedDB (store `folders`, key an id, value `{handle, name, used}`),
  so the next visit asks only for the permission again; they are the
  pane's "Recent folders" (below). As built: read and write permission is
  asked **up front**, when the folder is picked (`mode: "readwrite"`), and
  on the next visit the pane shows "Use the folder X again?" with
  "Allow" before anything is listed, not at the first write: a copy that
  stops half-way for a browser prompt would be worse than one question
  at the start.
- **Native** in the desktop app: the computer's files through the app's
  commands (the webviews of Tauri, WKWebView and WebKitGTK, have no File
  System Access API), shown at the home folder; "Open a folder…" is
  Tauri's dialog. What it may change: "The desktop app's folders" below.
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
calculator side. As built: a copy is planned first (`web/tx/
transfers.js`), and one question says everything it would warn about
(files that become strings, files or objects of the other family, what
has no text form, names taken by a directory, which are left out) before
anything is sent; then one "Replace X?" (or "Replace 3 files?" with the
list) for what is in use. A folder copied onto a folder of the same name
merges into it, its files in use asked about the same way; a directory
is never replaced. Where no folder can be picked, the right pane holds
the files in this browser from the start ("Files in this browser",
"Not on your disk. Use ⋯ → Download to save a file.", said again with
the first copy there: iteration 45, after the owner looked for a copy
in Downloads); the left pane never holds a folder.

**Recent folders** (iteration 49): the place switcher ("Show in this
pane") lists the last 8 folders the pane showed, newest first, one click
to reopen, and "Clear recent folders" under them. In Chrome and Edge they
are the picked folders' handles (above); a folder picked again is found
with `isSameEntry` and moved to the top, and reopening one asks the
browser's permission in that click, so it is one click when the browser
asks again on a new visit. In the desktop app they are the folders the
pane showed (the app keeps them; a walk down from the newest replaces it,
the home folder itself is not listed). Browsers without the picker
(OPFS or memory "Files") show no list.

### The desktop app's folders

Iteration 49, from the security review of 2026-10-14. The folder pane
browses everything the user's account may (owner, 2026-10-11 and
2026-10-14: `folder_list` and `folder_stat` are not confined), but the
app refuses, in Rust, whatever the page sends:

- **Changes and reads of a file's bytes outside the roots.** The roots
  are the home folder and every folder picked with "Open a folder…"
  (Tauri's dialog, opened from Rust: the page cannot add one). Picked
  folders are remembered across runs in `folders.json` in the app's own
  config folder (written only by Rust, mode 0600 on Unix), so a USB stick
  or a drive is picked once; mounted volumes are not roots by themselves.
  A picked folder that is not there (unplugged) is no root until it is
  back. `folder_read`, `folder_create`, `folder_write`, `folder_mkdir`,
  `folder_remove` and `folder_rename` canonicalise the nearest existing
  ancestor of the path (a link in home that points out of it is outside)
  and refuse with "… is outside the folders this app may change. Open it
  with "Open a folder…" first." A remove or rename is a change of the
  folder holding the entry, so that folder must be inside. A folder the
  page reports for Recent folders (`folder_used`) is never a root;
  "Clear recent folders" forgets the picked ones too (home alone again).
- **A recursive remove of the home folder or a folder directly in it**
  (`~/Documents`, `~/Library`, `~/.ssh`), whatever the roots: "this
  folder is not removed from here; remove it in the system's file
  manager"; nor a folder holding home (`/Users`). An empty one may
  still be removed one by one. A dangling link is never written through.
- **The aplet server's writes directly into the home folder**: the page
  marks them with an `x-serve: 1` header and the app refuses them there,
  as the page does (`HOME_REFUSED`). This guards against a bug in the
  page, not a hostile page, which would leave the header out.

### The emulator app's ROMs in the desktop app

Desktop tx has its own webview origin, so the browser's kept ROMs
(`saturnus-roms`) are not there. Instead `emulator_roms` reads the
emulator desktop app's `settings.json` (its config folder,
`ch.ractive.saturnus`, beside tx's; 64 KiB at most) and gives the page
`[{model, fileName, sha256}]` of the files that are there;
`emulator_rom(model)` gives the bytes (4 MiB at most), only when they
match the SHA-256 recorded there (the settings file lies in home, which
the page may write). No path reaches
the page. The page checks the bytes against the SHA-256 (a changed file
is left out, with a line in the console) and offers them under "Add an
emulated calculator" and for the aplet server; they run as private
calculators, as the two apps share none. Debug builds read both apps'
files from `SATURNUS_SETTINGS_DIR` when it is set.

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
  resent command would run twice). As built, the queue is the page's,
  over every place (a calculator and a folder alike): a copy is a job on
  both of its places, so a folder's write waits for the calculator's get
  before it; previews and listings go straight to the calculator's own
  one-request-at-a-time line between jobs.
- Progress: bytes of total for get and put, from the session's packet
  events.

### Rename, purge, new directory

Host commands through `satx-core` (`rename`, `remove`, `mkdir`), the same
refusals as the memory view (HOME, reserved names, a name in use; a
non-empty directory is purged with `PGDIR` after a modal question
naming how many variables go with it). As built: the page counts what a
directory holds from its listings (every level, directories included,
as saturnus-tx counts) before the question, and the purge sends that
count; saturnus-tx counts again and refuses (`notEmpty`) when the
directory changed meanwhile. A rename to a name in use is refused, never
a replace. A folder's rename, delete and new folder work the same way
(`folder.js`), a folder that is not empty asked about with its count.

### Backup and restore

- **Backup** (48/49): `satx-core`'s `backup` (ARCHIVE to `:0:`, get,
  purge the port object; `protocols/server-commands`), saved as
  `48GX-backup-2026-10-10-1432.hp`. It needs twice as much free memory
  as HOME holds, and 1024 bytes more (measured: ARCHIVE and the recalled
  copy each take HOME's size); the dialog shows the free memory (the
  `G D` header on the 48GX and 49G, `MEM` on the 48SX) and refuses early
  when it cannot fit, offering the alternative:
- **Copy everything to a folder**: the whole HOME tree as folders and
  `.hp` files, one get per variable. Slower, needs no free memory, and
  a folder is easier to look through than one archive.
- **Restore**: a backup file (or a directory object) put to `:0:` and
  RESTOREd: a modal question first ("Replace everything on the
  calculator?"). The calculator warm-starts and leaves server mode; the
  pane says so and offers to reconnect once `SERVER` runs again. The
  reconnect purges the copy RESTORE leaves in port 0 (`:0:SATXRS`).
- As built: "Back up…" reads the memory first and shows the free memory,
  what HOME holds and what a backup needs; "Back up" is off when it does
  not fit. The backup goes into the other pane's folder (else the
  downloads). "Copy everything to a folder" makes a folder named
  `48GX-HOME-2026-10-11-1432` there. A restore takes a file chosen from
  the calculator's "⋯", or a `.hp` row of the folder in the other pane;
  "Reconnect" on an emulated calculator types `SERVER` again itself.

### Fast transfer (XModem, 48GX and 49G)

As built (iteration 39, `saturnus-tx`'s `xmodem` module, `web/tx/
xmodem.js`): XModem is faster than Kermit on the 49G, but `XRECV` and
`XSEND` do not run inside the server (wiki `protocols/xmodem-hp`), so
the user types them. Offered on the 48GX and 49G only: "Fast transfer
to the other pane (XModem)…" (or "as a download") on a variable's menu,
"Fast transfer a file to it (XModem)…" in the calculator's "⋯", "Fast
transfer to the calculator (XModem)…" on a folder's `.hp` file (its
name asked first). One job of the queue:

1. Checks while the server answers: a binary file of the model's family
   (text and the other family go by the normal copy, which converts), a
   name not in use (refused, never replaced: the 48GX says "Name
   Conflict", the 49G stores `NAME.1`), the variable there for a get.
2. A 49G in algebraic mode is switched to RPN, the server ends (FINISH;
   from then on the device counts as out of server mode, even if the
   answer to FINISH is lost). A directory is refused for a fast get.
3. The dialog: "Server mode has ended. On the HP 49G, type this and press
   ENTER:" with `'NAME' XRECV` (or `XSEND`), notes (the RPN switch; on
   the 48GX a failed XRECV can leave an empty string; ON stops it), and
   "Waiting for the calculator (up to 3 minutes)…" with Cancel. An
   emulated calculator offers "Type it for me".
4. The bytes with progress: 1k blocks and HP's CRC on the 49G, 128-byte
   blocks and checksum on the 48GX (`satx-core`'s `XmodemOptions`).
5. Done: what came over and where, and how to start the server again
   (the model's keys), with "Later" and "Reconnect"; the pane says "The
   calculator left server mode" with Reconnect until then. The reconnect
   keeps the transfer mode the user had and puts a 49G back in
   algebraic mode (`-95 SF`).

Tested: the dialog and its flows with the fake calculator
(`web/test/tx-xmodem.test.mjs`), the replayed traces of the page against
the emulated 48GX and 49G (`crates/saturnus-tx-web/tests/xmodem.rs`,
`just tx-xmodem-traces DIR`), and the round trip byte for byte in `just
tx-e2e DIR` (`web/test/tx-e2e-xmodem.test.mjs`). Not yet on hardware: the
owner's 49G checklist.

### Aplets (38G, 39G, 40G)

The calculator is the client: the page serves a folder as the "disk
drive" it expects (wiki `protocols/aplet-transfer`, which answers
`questions/hp38g-39g-transfer-protocol`). As built (iteration 39):

- **Where.** "Serve a folder to a 38G, 39G or 40G": a link in the left
  pane's empty state, an entry of each pane's "Change" menu and a line
  in the "Add a calculator" dialog. It opens a screen over the panes
  (`web/tx/serve.js`, `serve.css`), in the spirit of HPGComm's small
  window: choose a folder (the one a pane shows is offered, or open
  one, or the files in this browser) and a calculator (on a cable, or an
  emulated 38G, 39G or 40G from a kept ROM or a ROM file), then "Start
  serving". The serving screen says what to press on the calculator
  (both families until the first directory file tells the model, then
  that one's), lists the transfers as they happen, newest first, and has
  "Stop serving". An emulated calculator shows its display and the keys
  a transfer needs (the six menu keys, the arrows, LIB or APLET, HOME,
  ON), since the calculator drives.
- **The protocol.** `kermit-proto`'s `Server` (the calculator as client:
  I, R, S, F, D, Z, B; G refused) under `saturnus-tx`'s `ApletServer`
  (`crates/saturnus-tx/src/aplets.rs`), in its own Worker
  (`web/tx/aplet-worker.js`, `aplet-driver.js`), at 9600 baud, 8N1.
- **The directory files.** The page answers `HP38DIR.CUR` and the
  directory file it names (`HP38DIR.000`; `HP39DIR.…` on the 39G and
  40G) itself, made from the folder: the entries of the copy the
  calculator wrote last whose files are still there, then the folder's
  other files of that family with a calculator header and an 8.3 name,
  titled from their header. So a folder of aplets from anywhere works
  at once, and the calculator never asks "Initialize directory?". The
  directory files the calculator writes after a SEND are kept in the
  folder, as HPGComm kept them.
- **Files.** What the calculator sends is written under its own name
  (`FUNCTION.000`); a file of that name in another case is replaced
  (the calculator asked "Replace existing aplet?" itself) only when it
  is the calculator's: a directory file, a file with a 38G or 39G
  header (one the server lists), or one received in this session. Any
  other file of that name stays: the server answers the calculator's F
  with an E packet ("File exists, not an aplet"), the calculator says
  the transfer failed, and the line says to rename the folder's file to
  receive the aplet. A renamed copy (`NAME-1`) was rejected: the
  calculator's directory file would still name the original, and the
  name would no longer be 8.3. The desktop
  app refuses to serve the home folder itself. What it asks
  for is read from the folder when it asks, in any case; a name the
  folder does not have is refused, and the line says so ("Transfer
  Failed" explained: the calculator then offers to get another).
- **The model** comes from the directory file asked for: `HP38DIR` the
  38G, `HP39DIR` the 39G or 40G (one ROM; not told apart, and need not
  be). `satx-core`'s `Model` gained `Hp38G` and `Hp39G` for it.
- **By wire (39G and 40G).** The folder's binary aplets (`HP39BinB`)
  are offered; "Send by wire" waits up to 2 minutes for the calculator's
  RECV, "HP39/40 (Wire)", and sends by XModem: 128-byte blocks with the
  checksum, padded with NUL (the calculator's own sender pads that way).
  A file that is not a binary aplet makes the calculator say "Transfer
  Failed" after taking every block, so only binary aplets are offered,
  and the line after a send says what that message would mean. Tested
  on the emulated 39G only; the page says so.
- **Tests.** Unit tests of the server and the directory files, replays
  of the emulated 38G's and 39G's recorded sessions (`just
  tx-aplet-traces DIR`: list L1, not an aplet, because a new aplet is a
  copy of one in ROM and traces hold no ROM bytes), the page's session
  over the 38G's recording (`web/test/tx-serve.test.mjs`), and `just
  tx-e2e DIR` (`web/test/tx-serve-e2e.test.mjs`): Function to the folder
  and back byte for byte on the emulated 38G and 39G, and on the 39G an
  aplet saved as XYZ, deleted, sent back by wire and sent to the folder
  again byte for byte.

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
  screen" in the device's "⋯", shown as a GROB in the pane's preview,
  with the variable preview's "copy to the other pane" button: PICT.hp
  into the other pane's folder through the copy of dropped files (its
  Replace question, the queue), or a variable of a name asked on the
  other calculator (a variable sent as PICT, the graphics screen's own
  name, becomes `PICT.1` on the 48SX); a download when the other pane
  shows nothing.
- As built, a folder's files preview the same way: a binary transfer
  file decoded, a `%%HP:` text shown as text, anything else said to be
  no calculator file; files above 256 KB only on a click. A 48SX or
  48GX file (the model from its header's ROM letter: A to J the SX, K
  to R the GX; wiki: protocols/hp-object-format) is decoded with that
  model's command names (`saturnus-tx-web`'s `FileNames`), from the ROM
  of an emulated calculator of the model in either pane or a ROM the
  emulator keeps; without one, the preview names the ROM it needs. A
  49G file's ROM pointers depend on the flash banks the calculator
  shows, which the file does not say: its programs need the calculator. A file the ROM names wrongly cannot be told: the header letter is trusted (the calculator writes its own).

## Seeing what happens

Iteration 45 ([[iterations/iteration-45-tx-progress-console]]), after
the owner clicked a picture on a 48SX and saw only a small "Reading…"
under the list for several seconds.

- **Progress where the click was.** A calculator preview opens at once
  with what it reads and from where ("Reading P from the 48SX…"), the
  bytes so far of the listing's size, a bar, and how long: on a cable
  first from the speed (a second, then the bytes at about a third of the
  line's rate, as the recorded traces show: each packet of at most 94
  bytes waits for its acknowledgement), then from the rate measured;
  said only at 2 s or more. An emulated calculator gets only the
  measured time (it may run faster than its baud rate). Behind another
  job it says what it waits for. The row being read, and the source and
  target rows of a copy, show a thin bar under the name; the pane's
  status line is the body size. Progress is drawn at most once per
  animation frame.
- **The cable console.** "Show the cable" in the page's header opens a
  panel under the panes: one line per Kermit packet or XModem block,
  newest last: the time, the pane (L, R, S for the serving screen), → to
  the calculator or ← from it, the packet type and sequence number, the
  length, plain words ("send-init", "data, 85 bytes (412 of 1,106
  bytes)", "nak: send 3 again", "block 1, 128 bytes", "ready, with HP's
  CRC"), the payload as text and hex (cut short). Red: a packet sent
  again, a NAK (an idle server's too), an error packet, a bad check, a
  packet cut off, a lost link. Pause holds the drawing (the log goes
  on), Clear empties it, Copy log puts it on the clipboard as text. Its
  look is its own on purpose (owner: "a bit of a matrix feeling"): green
  monospace on black in both themes, a faint glow on the newest line, no
  animation under reduced motion.
- **How it sees the bytes.** Every link a pane or the serving screen
  opens is wrapped (`web/tx/cable.js`, `TappedLink`) before the protocol
  gets it, so a cable through Web Serial, the emulated calculator and
  the desktop app's native serial look the same. The decoder follows
  kermit-proto's framing and quoting (the check type found by trying
  1, 3 and 2; the quoting from the init exchange it sees) and
  xmodem-proto's blocks; bytes outside a packet are shown as such.
- **Cost.** The decoder runs on the page's side over bytes the page
  relays anyway; the lines go into a ring buffer of 3,000; while the
  panel is closed nothing is drawn; while it is open the new lines are
  drawn once per frame. A decoder that throws never touches the link.
- **Open in the emulator.** A pane that shows the emulator's own
  calculator (shared, not one from a ROM file) has "Open in the
  emulator" next to Change. It opens the emulator in its own tab with
  `?model=<model>`; the emulator reads it once (and removes it from the
  address) and shows that model's calculator, the one saturnus tx goes
  on using ("Shared calculators"). Not in the desktop app, which has no
  emulator page. The page header's general "Emulator" link stays.
- **From the emulator.** The emulator's "Transfer files: saturnus tx"
  and its palette command "Open saturnus tx" open tx with
  `?connect=<model>` when the emulator runs a calculator tx can use: a
  shared one (a kept ROM, the SharedWorker) of a model with a serial
  port and a Kermit server (48SX, 48GX, 49G). tx reads the parameter
  once, removes it from the address and connects the left pane to that
  calculator as "Add an emulated calculator" does, with no question when
  the pane is empty or shows a folder (one connected to another
  calculator is replaced only after a question); if either pane shows it
  already, nothing happens. For the other models
  (the Pioneers, the 28S, the 19BII, the 38G, 39G, 40G) the link is the
  plain one.
- **Switching back and forth.** The two pages open each other in named
  tabs ("saturnus-tx", "saturnus-emulator"; each page names its own
  tab). A link first looks the tab up (`window.open("", name)`); when it
  shows the other page, that tab is brought forward and sent the request
  as a same-origin message (`connect` to tx, `show` to the emulator), so
  neither page loads again and both keep their state. Otherwise the URL
  with the parameter is loaded. A message to the one tab found, not a
  BroadcastChannel, so no other open tab acts on it; a page takes a
  request only from the tab that opened it or one it opened. Browsers find a
  named tab only among tabs that opened one another: a tab opened by
  hand gets a second one. Details: web/README.md, "The pages link each
  other"; `web/shared/site-links.js`.

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
  file). Decided in iteration 37: **fetched once**, not inlined at build
  time. The sprite is `web/shared/icons.svg`; `loadIcons()` in
  `shared/components/icons.js` fetches it and puts it at the top of the
  body, where every `<use href="#ic-…">` of either page finds its
  symbol. Inlining at build time would make the pages served from `web/`
  (development, the tests, `cargo tauri dev`) differ from the built ones;
  external `<use href="shared/icons.svg#ic-…">` references would need
  every icon call changed and depend on how the desktop app's webviews
  load SVG from their custom scheme. The one fetch is precached by each
  subsite's service worker and embedded by the desktop app; the icons
  appear a moment after the first paint, which the layout does not see
  (the icons have fixed sizes). `web/test/icons.test.mjs` checks that
  no page carries symbols of its own.
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
  only, tolerating a schema it does not know; it runs the emulator's
  calculators (the `saturnus` database's auto-saved states, one page at
  a time, see "Shared calculators"). Its own IndexedDB names,
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
   at 360, 390, 430, 768 and 1280 px as `overflow.test.mjs`. As built
   (`web/test/tx-page.test.mjs`): the recorded page session through a
   `FakeLink`, and the two panes with a fake calculator in place of the
   protocol Worker (`web/test/tx-fake-calc.js`, saturnus-tx's rules for
   names in use, non-empty directories and restarts) and a folder in
   memory: drags both ways and drops from outside, "Replace X?" on both
   sides with Cancel keeping the old, purge and delete with the count,
   rename and new directory, the copy plan's warning before anything is
   sent, the queue, backup and restore; the overflow audit on every
   view, both themes.
4. **The page against the emulator, ROM-gated** (`just tx-e2e DIR`): the
   transfer page with saturnus-web's emulator as the device over the new
   serial commands, 48SX, 48GX and 49G (38G and 39G from iteration 39):
   connect, detect, list, get, put, rename, purge, new directory, backup
   and restore, emulator to folder and back byte for byte. As built: also
   the graphics screen, the folder copy of HOME put back and copied
   again byte for byte, the backup restored onto a freshly booted
   calculator (the same names, sizes and checksums), and an emulated
   48GX to an emulated 49G with the plan's warning (about 15 minutes at
   1x for all of it). Never in CI
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
  the session's pauses into the calculator's own timeouts. Decided in
  iteration 37: the **page holds the port** and relays its bytes to the
  Worker by messages (`web/tx/protocol.js`); the Worker runs
  saturnus-tx's device and its timers (`protocol-worker.js`). Neither
  alternative fits the speed search, which reopens the port at other
  settings: a `SerialPort` cannot be posted to a Worker, and transferred
  streams die with the port's next open. The page's part is driven only
  by events (stream reads, messages), which background throttling does
  not delay. Measured in a hidden tab: "Hidden-tab measurement" below. The calculators' quirks are handled in
  `satx-core`: the 200 ms turnaround, a command dropped right after the
  final ACK, the idle NAKs to drain.
- **USB serial adapters.** Linux needs the user in `dialout` (`uucp` on
  Arch) for Chrome and the app alike; Windows refuses counterfeit
  Prolific chips with current drivers; the 38G needs its own 10-pin
  cable (the owner has one). The connect dialog's help names these.
- **The File System Access API** is Chromium-only (above); Tauri's
  webviews lack it (native folder access instead). Writing asks for a
  permission per visit.
- **The 38G/39G directory file's format** was unknown; found in
  iteration 39 on the emulated 38G and 39G and from HPGComm's source
  (wiki `protocols/aplet-transfer`). Real hardware is the owner's 38G;
  the 39G and 40G wire path stays untested on hardware until someone
  reports.
- **Backups need free memory**, restore warm-starts, and a 48SX with
  a large HOME may not be able to back itself up through `:0:`. The
  folder copy is the way out.
- **Two pages on one origin** share IndexedDB and localStorage: a schema
  change in the emulator's ROM store must not break the transfer page's
  read, and the transfer page must not write there. The storage-names
  test keeps the names apart. The one exception is the shared
  calculators' states, which the shared module writes under a lock.
- **Moving components** to `web/shared/` can break the emulator page or
  the desktop app's embedded file list; hence a separate,
  behaviour-neutral commit and the existing site and frontend tests.

## Hidden-tab measurement

Measured 2026-10-11 for [[iterations/iteration-37-tx-skeleton]]
(`web/test/tx-hidden.mjs`, headless Chrome 154 on macOS): the transfer
page with the emulated 48GX at 1x (gxrom-r), a 30 KB string (30 733
bytes as a binary transfer file) got once visible, then again and again
for six minutes with the tab hidden behind another one, then once
visible again. A chain of 10 ms timers on the page's main thread showed
the throttling: 5459 wake-ups a minute visible, 34 a minute in the first
minutes hidden, **1 a minute** after five minutes (Chrome's intensive
throttling).

| When | State | Get of 30 KB | Page timers |
| --- | --- | --- | --- |
| 0 s | visible | 104.7 s | 5459/min |
| 211 s | hidden | 104.8 s | 34/min |
| 315 s | hidden | 104.7 s | 1/min |
| 422 s | hidden | 106.9 s | 1/min |
| 527 s | hidden | 104.6 s | 1/min |
| 632 s | visible | 104.3 s | 5459/min |

No failure, no retry storm, the same time hidden as visible: the
protocol's timers run in its Worker, the page's part is driven by
messages and stream reads, and neither is throttled. So the design
stands (the page holds the port, the Worker runs the protocol). About
290 bytes a second is the emulated 48GX's Kermit server at 1x (the
30 KB put took 172 s); a real calculator's speed is the owner's
checklist on hardware.

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
