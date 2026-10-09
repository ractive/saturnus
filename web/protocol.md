# saturnus front-end protocol, version 1

The page (`web/`) talks to the emulator only through a **backend** object,
and the backend talks to its host in the messages below. Three hosts speak
it:

| Host | Backend | Transport |
| --- | --- | --- |
| Browser | `WorkerBackend` (`backend.js`) | `postMessage` to `worker.js`, which runs the wasm core |
| Tauri app | `TauriBackend` (`backend.js`) | `invoke("command", {msg})` and the `saturnus` event (`crates/saturnus-tauri`) |
| `saturnus run` | `saturnus ctl`, `curl`, any HTTP client | HTTP/1.1 on 127.0.0.1, see [HTTP](#http) below (`crates/saturnus-cli/src/control`) |

Every host runs **one implementation** of this protocol and its pacing:
the state machine `Engine` in `crates/saturnus-host/src/protocol/`, which
takes commands and the host's clock and gives replies, events and the
time it wants to be called again. The hosts are thin drivers around it:
the Worker (`worker.js`, through the wasm bindings' `Host`) with
`performance.now()` and one `setTimeout`; the Tauri app and `saturnus
run` on the machine thread of `crates/saturnus-drive/src/runner.rs`, with
`Instant` and its command channel. What only one host has stays with it:
the ROM slots' storage (IndexedDB in `romstore.js`, a settings file in
the Tauri app), files and dialogs, and the native commands (`screen`,
`info`, `model`, `peek`, `poke`, `keyScript`). The same command script
gives the same replies and events through the state machine, the native
runner and the Worker (`web/test/protocol-script.json`).

The page picks the Tauri backend when `window.__TAURI__` exists and the
Worker otherwise; nothing else in the page knows which host it runs on.
The host owns the machine, paces it against the wall clock (speed factor,
idle sleep while the CPU is in SHUTDN) and **pushes** events; the page
never polls.

Messages are JSON objects (the Worker passes them by structured clone,
which also carries the `Uint8Array` fields marked *bytes*; Tauri and HTTP
serialise them as JSON, where a *bytes* field is a base64 string, as the
`frame` event's `pixels`). Field names are camelCase.

## Commands (page to host)

`{"v": 1, "id": 7, "cmd": "keyDown", "key": "enter"}`

`v` is the protocol version; a host refuses another version (or none)
with an error reply. `id` is optional: with an `id` the host answers with exactly
one reply, without one it answers nothing (errors then arrive as `error`
events).

Reply: `{"type": "reply", "id": 7, "ok": true, "result": ...}` or
`{"type": "reply", "id": 7, "ok": false, "error": "message"}`.
**Files** are the host's business: the page never names a file. The
Tauri host refuses any message with a `romPath` or `path` field, chooses
the file of `boot`, `chooseRom`, `saveState` and `loadState` in a native dialog, and
reads at most 4 MiB (the largest ROM any model takes, an unpacked 49G;
the largest state, the 49G's, is 2.6 MB). The `TauriBackend` also adds
`session` (an id drawn per page load) and `seq` (0, 1, 2, ...) to every
message, and the app delivers the commands to its machine thread in
that order, whatever order its async tasks start in.

A host sends the events a command caused (a `status` after `pause`, a
`frame` after `loadState`) before that command's reply, so a caller that
has the reply also has the state it led to.

| Command | Fields | Result | Does |
| --- | --- | --- | --- |
| `hello` | | `{protocol: 1, host: "worker" \| "tauri", models: ["48sx", ...], version: "0.1.0"}` | Handshake; the models this host runs, and the release it was built from (`version`, the workspace's; the About panel shows it). The host also sends its current `status`, `keys` and `frame` again (before the reply), so a page that was reloaded while the machine kept running shows it at once. In the browser a reload starts a new Worker, and with it a new, empty machine; the Tauri app's machine outlives the page, so after a reload it is still running. |
| `skin` | `model` | the skin JSON (`crates/saturnus-host/src/skins`, with `letters` and `typing`) | Static data for drawing a model before and after boot. |
| `layout` | `model` | `{columns, rows, keys: [{name, label, alpha?, row, x, w}]}` | The model's keys in rows with their labels: the native hosts' `model` result carries it (`saturnus ctl model` lists the key names); the page draws skins (`skin`) instead. |
| `boot` | `model`, then `rom` (*bytes*) and `romName` (Worker); nothing more for Tauri, which asks for the ROM in a file dialog; Tauri also `fresh` (boolean) | `{model, romName}`, with `restored: true` or `restoreError` after a boot with a kept state ([Auto-save](#auto-save)); `null` (dialog cancelled) | Builds the machine from the ROM and starts running. `model` is a preference: a ROM that only fits another model boots that model. The Tauri app restores the state it kept for the model unless `fresh`, which forgets it; the Worker's `boot` is always cold (its kept states come with `bootModel`). |
| `keyDown` | `key`, `shift` (optional) | | Queues a press of the key (script name, as in `Key::name`), held until `keyUp`. Wakes a sleeping machine. With `shift` (`leftshift`, `rightshift`, or `shift` on a model with one), the press is a shifted function (the page's Ctrl/Option+click): when its turn in the queue comes, after every key queued before it has played, the shift is tapped first unless its annunciator is on (on a one-shift model, either). An error for a key the model does not have, a `shift` that is not one of its shift keys, or before a ROM is booted. |
| `keyUp` | `key` | | Releases the newest held press of that key, once it was down at least 60 emulated ms (nothing if none is held). Errors as `keyDown`. |
| `keyUpAll` | | | Releases every held key (the window lost the focus). |
| `typeLetter` | `letter` (one character) | `true` if the model can type it | Types the letter through the model's alpha mode, lowercase through its shift (see `web/README.md`, Keyboard). |
| `typeKeys` | `keys` (array of names) | | Full presses of the keys, one after the other (the 38G's space is `["shift", "2"]`). All or nothing: an entry that is not a key name of the model, or no booted ROM, is an error and presses nothing. |
| `releaseAll` | | | Releases every key and drops the queue at once. |
| `setSpeed` | `speed`: `"1"`, `"2"`, `"4"` or `"max"` (a string; another one is `"1"`) | | Emulated time per wall time while the CPU computes or keys are queued; at `max` as fast as the host can while staying responsive. While the CPU sleeps in SHUTDN, emulated time follows the wall clock at 1x at any speed (the ROM's clock, auto-off and cursor blink keep real time). |
| `pause` | `paused` (boolean) | | The Run/Pause switch. |
| `reset` | | | Hardware reset (RAM kept); releases the keys and runs. |
| `saveState` | (Tauri: none; it shows a save dialog) | Worker and HTTP: `{state` (*bytes*)`, cycles}`; Tauri: `{path}` or `null` | The whole machine state, bound to model and ROM. The `WorkerBackend` keeps it in IndexedDB, one slot per model, apart from the auto-saved one ([Auto-save](#auto-save)). |
| `loadState` | `state` (*bytes*, Worker and HTTP, at most 4 MiB); nothing for Tauri, which shows an open dialog | Worker and HTTP: `{}`; Tauri: `{path}` or `null` (cancelled) | Restores a saved state of the same model and ROM; releases the keys. |
| `saveFile` | `name` (the file name offered), `data` (*bytes*, at most 4 MiB) | `{path}` or `null` (cancelled) | Tauri only, answered by the app without the machine: a save dialog offers `name` (its last part, without a leading dot) and the bytes are written where the user chose. The page's screen images (`web/screenshot.js`); in the browser the `WorkerBackend` downloads them instead. |
| `visibility` | `hidden` (boolean) | | The page is hidden: in the browser a computing machine stops as an animation frame would; a sleeping one still keeps time. The native hosts keep running (a desktop window keeps computing behind others). On every host that keeps states, an unsaved change is saved at once, or as soon as the machine settles ([Auto-save](#auto-save)). The page sends it on `visibilitychange` and `pagehide`; the Tauri app also when its window closes. |
| `stats` | | `{cycles, emulatedMs, workMs, ticks, wakes, memoryLooks, memoryMs, loop, owedMs, nowMs}`, the same on every host | Counters for tests: `workMs` is the host's busy wall time (passes, wakes, sends, key scripts), `ticks` its run passes, `wakes` its wakes from sleep, `memoryLooks` its looks at the user memory for `memoryChanged` and `memoryMs` the wall time they took, `owedMs` the emulated time owed to the wall clock (unpaid, plus the current sleep), `nowMs` the host's clock. `emulatedMs + owedMs` grows with wall time times the speed. |

### ROM slots

Every host but HTTP remembers the ROM of each model, so the user chooses
it once: the Worker keeps the ROM's bytes in the browser's IndexedDB
(database `saturnus-roms`), the Tauri app keeps the file's path in its
settings file (`settings.json` in the platform's config directory for
`ch.ractive.saturnus`, mode 0600 on Unix). Paths never cross to the page:
a slot tells only the file's name. `saturnus run` takes its ROM on the
command line and does not serve these commands.

The images saturnus knows, with where hpcalc.org offers them, are one
table in `crates/saturnus-host/src/romid.rs` (`KNOWN`), which the CLI's
`rom fetch`, the app's `downloadRom` and the page's links (each slot's
`download`) all read. saturnus never hosts or proxies an image.

A ROM is **identified by its content**
(`crates/saturnus-host/src/romid.rs`, shared by every host): its SHA-256
against the images saturnus knows (exact: models and revision; the 39G
and 40G share one image), otherwise its size and form against the models
whose loader takes it (fits: the 512 KB 48GX and 38G images cannot be told
apart this way), otherwise unknown. A batch of files is assigned by one
set of rules: a file the user chose fills the slots of its exact models,
or the selected model if it fits it, or its only fitting model; one that
fits several others is offered. Files the Tauri app finds beside a chosen
one fill only empty slots (exact) or are offered (fits); unknown files are
left alone. The Tauri app looks at the regular files of the chosen file's
folder only (not below it, no links): the first 256 entries, at most 16
files of a ROM's size and 32 MiB, each read with the 4 MiB cap.

| Command | Fields | Result | Does |
| --- | --- | --- | --- |
| `romSlots` | | `{slots, offers, lastModel, bootLast, remembered, note}` | The slots: `slots` one per model in `hello`'s order, `{model, fileName, revision, state, download}` with `state` `empty`, `ready`, `missing` (Tauri: the file is gone) or `changed` (its content is no longer what was chosen) and `download` where hpcalc.org offers the model's image, `{file, size, url, page, revision}` (`url` the zip, `page` its page there), or `null` (the 42S); `offers` the files that may be a model's ROM, `{id, models, fileName}`; `lastModel` the model booted last; `bootLast` whether the page boots it when it opens; `remembered` whether the host keeps the slots beyond this page or app run (`false` when the browser refuses to store or the app could not write its settings file); `note` why not, or `null`. |
| `bootModel` | `model`, `fresh` (optional boolean) | the slots, plus `booted` (`boot`'s result) and `notice` | Boots `model` from its remembered ROM after checking it is still there and unchanged, with the state the host kept for it ([Auto-save](#auto-save)); with `fresh` cold, forgetting that state (the page's Start fresh). An error when the ROM is not there (the page reports it and, in the app, asks for the file again); nothing else boots in its place. |
| `chooseRom` | `model`, and `files` (Worker: `[{name, rom` (*bytes*)`}]`, one or more) or `offer` (an offer's `id`); Tauri without `offer`: nothing more, it asks in a file dialog | the slots, plus `booted` (`boot`'s result or `null`), `notice` (what else was found or offered, and why a file was not taken) and `bootError` (why the boot failed, or `null`: the files are remembered all the same, so a failed boot is not the command's error); `null` if the dialog was cancelled | Identifies the files (Tauri: the chosen one and those beside it), assigns them to their slots, remembers them and boots `model` if it got a ROM, else the first model a chosen file went to. With `offer`, takes that offered file as `model`'s ROM and boots it. |
| `downloadRom` | `model` (Tauri only) | as `chooseRom`; `null` if the user declined | Asks the user in a native dialog (what is downloaded, from where, whose ROM it is and under what terms hpcalc.org hosts it), downloads the model's image from hpcalc.org (`curl` with its own user agent, at most 8 MiB), checks its size and SHA-256 and only then stores it in `roms` in the app's data directory (one already there that verifies is kept), then takes it as if chosen in the dialog (the files beside it too) and boots it. A failure names the page to download it from by hand. The Worker does not serve it: a browser cannot fetch from hpcalc.org (no cross-origin header), so the page links to `download.page`. |
| `forgetRom` | `model` (optional) | the slots | Forgets `model`'s ROM, or every ROM and the last model without it: the Worker deletes the bytes from IndexedDB, the Tauri app the paths from its settings (the files stay). Saved states are not touched, but for the 49G's in the browser, which hold its flash: the `WorkerBackend` deletes both its slots, and the Worker writes no auto-saved 49G state until the 49G boots again. |
| `romSettings` | `bootLast` (boolean) | the slots | Whether the last model boots when the page opens. |

`boot` stays as it was (a ROM given once, not remembered). The Worker
handles one command after the other, in the order they came, so a key
sent while a ROM command waits for IndexedDB follows it. The ROM-slot
commands are checked by the state machine first (the version, and the
refusals during a send).

### The user memory, read-only

Every host reads the calculator's user memory straight from RAM
(`saturnus_objects::ram`): nothing is written, no key is pressed, the
calculator does not change mode. The 48SX, 48GX and 49G have such a
memory; on the other models (aplets, or the 42S) the read commands reply
with an error that says why, and `watchMemory` says so without an error.
Before the ROM has set up its memory (the "Try To Recover Memory?" prompt)
the reads reply with an error too.

| Command | Fields | Result | Does |
| --- | --- | --- | --- |
| `watchMemory` | `on` (boolean) | `{supported, reason}`: `supported` is `true`, `false` with the `reason` (a model without RPL memory), or `null` with no ROM booted | Starts or stops the `memoryChanged` events. Events tell of changes after this reply, so a page subscribes first and then reads. `hello` stops them (a reloaded page asks again). |
| `memoryTree` | | `{path, variables}`: `path` is the calculator's current directory (`["HOME", "A"]`), `variables` HOME's tree, each `{name, type, size, checksum, address, variables?}`, newest first | HOME's tree. `type` is the calculator's type name, `size` in bytes as BYTES reports it (may end in .5), `checksum` BYTES's, `variables` a directory's own. |
| `stack` | | the typed levels, level 1 first | The stack. |
| `flags` | | `{system, user, set}`: 64-flag words as 16 hex digits (two of each on the 49G) and the set flags' numbers | The flags. |
| `objectAt` | `address` (0 to #FFFFF) | the typed object | One variable's value (its `address` from `memoryTree`). An object too large to decode is an error ("more than 262144 objects ..."), not a partial answer. |

The objects' shapes are under [Typed objects](#typed-objects). The
objects `stack` and `objectAt` return carry the calculator's own text as
`text`, on the object and on every object inside it (a list's items, a
tagged object's object, an array's elements), each written as the
calculator writes it in that place and in the display mode the flags
select (`saturnus_objects::described`): a front end shows and copies
these texts and formats nothing itself. Where the host has no text for
an object (a graphic, a library, a program holding a ROM object its
tables do not name) `text` is absent, as are `source`, `unit` and `name`.

The native hosts (Tauri, HTTP; the machine thread in
`crates/saturnus-drive/src/runner.rs`) also take these commands; the
Worker answers them with "unknown command". A read command returns its
answer as the reply's `result`.

| Command | Fields | Result | Does |
| --- | --- | --- | --- |
| `screen` | `png` (boolean), `scale` (1-8) | the `frame` event's fields, plus `rows` (one string per pixel row, `#` dark, `.` light, as the CLI's `.txt` screens) and `displayOn`; with `png`: `{png` (*bytes*, a 1-bit PNG)`, width, height}` | The display now. |
| `info` | | `{protocol, host, model, romName, running, halted, speed, loop, displayOn, cycles}` and the host's own fields (HTTP: `romSha256`, `romRevision` or `null`, `serial` endpoint or `null`, `control` URL) | What runs, and where. |
| `model` | | `{model, clockHz, width, height, hasSerial, layout}` (`layout` as the `layout` command's result) | The running model. |
| `keyScript` | `script` (text) | `{emulatedMs, warnings}` | Runs a key script (README, "Key scripts"; a line may also hold several key names, and `+ - * / .` name those keys) at once in emulated time and replies when it is done; every key is released first. At most 64 KiB, 2000 lines, 10 minutes of emulated time and 30 s of wall time; a `wait-idle` that reaches its cap is a warning. |
| `peek` | `address`, `length` (nibbles) | `{address, nibbles}` (hex digits) | Reads memory through the current mapping, without side effects. |
| `poke` | `address`, `nibbles` (hex digits) | `{address, length}` | Writes memory as CPU writes would (ROM ignores them, I/O registers react). |

### Typed objects

`stack` and `objectAt` return objects as JSON with a `type` field (the
shapes of `saturnus-objects`' `Object`):

| `type` | Fields |
| --- | --- |
| `real` | `value` (a number; text such as `"1.5E-400"` beyond an f64) |
| `integer` | `value` (49G exact integer: a number up to 15 digits, else text) |
| `complex` | `re`, `im` |
| `string`, `name`, `local_name`, `character` | `value` |
| `binary` | `value`, `base` (`hex`, `dec`, `oct`, `bin`), `text` (`"# 2Ah"`) |
| `list` | `items` |
| `tagged` | `tag`, `object` |
| `unit` | `value`, `unit` (`"m/s^2"`) |
| `array` | `dims`, `items` (rows nested; also the 49G's symbolic matrices) |
| `program` | `source` (`"« 1 2 + »"`) |
| `algebraic` | `source` (`"'A+1'"`, with its quotes) |
| `command` | `name` (`"SIN"`; absent when the ROM's tables have none), `address` (the ROM address, absent for XLIB names), `library` and `command` (its XLIB numbers when known; an XLIB name without `name`, which the calculator shows as `XLIB 1234 5`, has only these) |
| `unknown` | `prolog`, `kind`, `nibbles`, `hex`, `truncated`, `source` |

`text` (on the objects of `stack` and `objectAt`, see above), `source`,
`unit` and `name` are the calculator's own text, from the ROM's
command tables (read from the loaded ROM on the first object read, and
again after the 49G's flash changed) and in
the display mode the flags select (number format, fraction mark, binary
base and word size). A command inside a list is `{"type": "command",
"name": "SIN", "address": 111788, "library": 2, "command": 81}` (the 48SX ROM J). Text longer than 65536 characters
ends in `…`.

`peek` and `poke` stay inside the 20-bit address space (`#00000` to
`#FFFFF`, no wrap-around) and move at most 65536 nibbles per command. A
message never names a file: any `romPath` or `path` field is refused by
every native host.

Reserved for later versions (unknown commands get an error reply):
`eval`.

### The user memory, written

Every host writes the calculator's user memory the way a PC does: through
the ROM's own Kermit server, so its memory manager stays consistent; RAM
is never written (`crates/saturnus-host/src/transfer.rs`, on the
`kermit-proto` client). One write is one **hidden transaction**: the host
types `SERVER` on the command line, waits for the server's first NAK,
runs the Kermit exchanges over the emulated serial port, ends the server
with `G F` and waits until the calculator shows its stack again. It runs
in emulated time as fast as the host can (at any speed setting): 11 to 20
s of emulated time, about 0.1 s of wall time natively and 0.15 to 0.25 s
in the browser. The 48SX, 48GX and 49G take writes; the other models
refuse them, as they refuse the reads.

| Command | Fields | Result | Does |
| --- | --- | --- | --- |
| `storeFile` | `dir`, `name`, and the file: `data` (*bytes*; Worker and HTTP; the Tauri app takes it too, from a file dropped on the page); Tauri without `data`: nothing, it asks in a file dialog and names the variable after the file | `{name, emulatedMs, keys}`, `name` as the calculator stored it; Tauri: `null` if the dialog was cancelled | Stores the file as variable `name` in directory `dir`. An HP binary file (`HPHP48-x`, `HPHP49-x`) travels in binary; anything else as text, converted from UTF-8 into the calculator's character set when it can be (a `%%HP:` header makes the calculator compile it, otherwise it is a string). At most 512 KiB. An existing name is replaced or kept with `.1` added, as the calculator's flag -36 says. |
| `fetchFile` | `dir`, `name` | `{name, size, emulatedMs, keys}` and the file: `data` (*bytes*); Tauri: `file` (the file's name) instead, written where its save dialog said, or `null` if cancelled | The variable as an HP binary file (the header the ROM writes, then the object, without the padding of the last packet): stored again it gives the same bytes back. |
| `purge` | `dir`, `name` | `{emulatedMs, keys}` | Purges the variable; a directory with everything in it (`PGDIR`). Refused for a directory that holds the current one. |
| `rename` | `dir`, `name`, `to` | `{emulatedMs, keys}` | `'name' RCL 'to' STO`, then the old name purged. Refused when `to` exists or the directory holds the current one. |
| `createDir` | `dir`, `name` | `{emulatedMs, keys}` | `'name' CRDIR` in `dir`: a new, empty directory. Refused when `name` exists in `dir`. |
| `changeDir` | `dir` | `{emulatedMs, keys}` | Makes `dir` the current directory. |
| `setFlag` | `flag` (-64 to 64, not 0; -128 to 128 on the 49G), `on` (boolean) | `{emulatedMs, keys}` | `SF` or `CF`. |
| `storeText` | `text`, and `dir` and `name` (a variable), or `level` (a stack level, 1 at the top); `was` (optional) | `{emulatedMs, keys, error?}`; `error` is why the calculator did not compile it (its own message, `"Invalid Syntax"`, or `"the text holds more than one object"`, `"the text holds no object"`), and then nothing changed | Compiles `text` on the calculator and puts the one object it gives in the variable (stored, created if new) or in place of the stack level (the levels around it keep their places). See [The palette's editor](#the-palettes-editor). |

- The read that goes with `storeText`, `editText` (`dir` and `name`, or
  `level`; result `{text, was}`), is served like the reads above, no key
  pressed: the object's text written so that it compiles back to the same
  object (every digit of a real whatever the display mode, binary
  integers at 64 bits, a tagged object as `:tag:object`). An object with
  no text form (a graphic, a library, a backup, a directory, code), a
  string holding `"` and text over 65536 characters are an error. `was`
  identifies the object itself, `"size:checksum"` (its size in nibbles,
  `BYTES`'s checksum in hex), which no display mode changes.
- `dir` is a path from HOME, `["HOME", "D"]` (`HOME` may be left out);
  without it the current directory. Names are plain global names (no
  digit or point first, no spaces, delimiters or operators); the
  directory must exist, and the variable too for `fetchFile`, `purge` and
  `rename`. These checks, and a refusal (the model, a command line being
  edited, flag -33 set on a 48), happen before
  any key is pressed.
- The calculator is left as it was: the stack (every transaction counts
  the levels first, and what a failed command left is dropped), the
  current directory (a write in another directory changes back), flag
  -35 (set for a binary transfer, cleared for text, then put back), the
  49G's algebraic mode (below) and
  the screen (what the server draws is never shown; the `frame` after the
  write is the stack's). As on a real calculator the server keeps its I/O
  settings in `IOPAR` in HOME, which it creates on first use; on the 49G
  the CAS may create `CASDIR`. The command line's history holds `SERVER`.
- A command line being edited refuses every write (finish or cancel it
  first); so does a 48 with flag -33 set (I/O over infrared).
- The 49G in algebraic mode (flag -95 set, as it boots): its server,
  entered from that mode, leaves the stack packed in a list, so a write
  types `CF(-95)` first (the echo the mode leaves is dropped with the
  backspace key), runs the server in RPN mode and types `-95 SF` once the
  server has ended. -95 is set again on every path: the write done, the
  calculator's refusal, a server that stopped answering, a stop. The
  keys add about 4 s of emulated time (a fraction of a second of wall
  time). `setFlag` alone types `SF(n)` or `CF(n)` without the server
  (`keys: true` in its result).
- A write is a send to the rest of the protocol (see [Typing](#typing)):
  it runs in turns, `busy` is raised in the `status` event for its whole
  length, its frames and keys are held, and the same commands are
  refused meanwhile, with "a transfer is in progress (releaseAll stops
  it)". `releaseAll` (or an HTTP client that gives up) stops it: a line
  typed in part is cancelled and a running server is ended with ON,
  pressed again while it still answers (the 49G's server takes ON in a
  transaction as the transaction's end and goes on serving). 30 s of
  wall time at most. A
  calculator error is the reply's error, naming the command
  (`'P' RCL 'SIN' STO: Invalid Syntax`); the cleanup still runs.
  `storeText`'s compile is the exception: the calculator's refusal of the
  text is the result's `error`, not the reply's.

### The palette's editor

The palette's editor (`web/editor.js`, `web/components/rpl-editor.js`)
edits three things and sends each back its own way, behind one function
(`saveEdit`):

- **A live command line** (`commandLine` gives its text and cursor):
  `replace`, which keeps the calculator in its edit, also inside `EDIT`
  and `VISIT`.
- **A variable or a stack level** (`editText` gives its text):
  `storeText`. One hidden transaction (for a short text 16 to 25 s of
  emulated time and 0.15 to 0.2 s of wall time natively, 0.25 s in the
  browser; 3500 characters take 47 to 66 s of emulated time and 0.55 s
  natively): the
  text, wrapped in `{ }`, travels as a string variable (`SATEDIT`, with a
  number added if the directory has one) in binary; the host command
  `'SATEDIT' RCL 'SATEDIT' PURGE STR→ DUP SIZE 2 MIN` compiles it (the
  list keeps anything in it from running) and the reply says the
  calculator's error or how many objects the list holds; for exactly one,
  `DROP 1 GET 'name' STO` stores it, or `DROP 1 GET n+1 ROLL DROP n
  ROLLD` puts it on level `n`. The string variable is purged before the
  compile, so a failed one leaves nothing; what it left on the stack is
  dropped. Refused before anything runs, because each could move where
  the calculator reads the wrapper's end: a `}` that closes more than the
  text opened (outside strings and `@` comments), a string left open
  (it would swallow the wrapper's `}`), and a `"` or `@` right after a
  word's character (the ROM starts a string or a comment there, mid-word:
  `X@ 1` is `X` and a comment; put a space before it).
- With `was` (the identity `editText` gave when the editor opened), the
  write is refused unless the object is still that one (a change of the
  display mode meanwhile does not count): a save never
  replaces what the editor did not show ("P changed on the calculator
  since it was opened: open it again").
- Why not keys: typing runs at 2.4-10 characters per second of emulated
  time and cannot type `;`, the backslash and other characters on the
  48SX; the Kermit path takes any character of the set, at a cost that
  hardly grows with the length (decision log, 2026-10-08).

## Typing

All three hosts (the Worker, Tauri and HTTP) type text into the
calculator's command line by key presses, on the 48SX, 48GX and 49G (the
other models refuse with an error). The engine is
`crates/saturnus-host/src/typing.rs`; how each character is typed, and
what it reads from RAM: wiki `hardware/command-line`.

| Command | Fields | Result | Does |
| --- | --- | --- | --- |
| `commandLine` | | `{active, text, cursor}` | The command line, read from RAM; no key is pressed. `active` is false when none is open (also while the 49G shows an error box); `text` is `""` then. `cursor` counts calculator characters from the start (`x̄` is one character and two UTF-16 units). |
| `insert` | `text` | `{typed, keys, emulatedMs, commandLine}` | Types `text` at the cursor, or starts a command line. |
| `run` | `text` (may be `""`) | as `insert`, plus `closed`, `error`, `running` | Types `text`, then presses ENTER and waits until the calculator settles (at most 30 s of emulated time). `closed`: no command line is open afterwards; `error`: the message the calculator showed (`"Invalid Syntax"`, `"DROP Error: Too Few Arguments"`) or `null`; `running`: still busy when the wait ended. The 49G's error box is dismissed (ATTN), which returns to the line, as the 48 leaves it open. |
| `replace` | `text` | as `insert` | Deletes every character of the command line being edited, staying in it (also inside `EDIT` and `VISIT`), then types `text`. An error if no line is open. |
| `typeText` | `text` | as `insert` | The same as `insert` (newline is now the calculator's newline; ENTER is `run`). |

- `text` is Unicode in the calculator's character set (ASCII, Latin-1,
  and `∡ x̄ ∇ √ ∫ Σ ▶ π ∂ ≤ ≥ ≠ α → ← ↓ ↑ γ δ ε η θ λ ρ σ τ ω Δ Π Ω ■ ∞` as
  characters 128-159), a string (anything else is an error), at most
  4096 characters, and refused when typing it is estimated to take more
  than 10 minutes of emulated time: each character's keys (by its
  method, CHARS navigation included) times the model's measured time
  per key (48SX 270 ms; 48GX 180 ms, 420 ms in CHARS; 49G 100 ms, 170 ms
  in CHARS). About 2200 plain characters fit on a 48SX; the cap keeps a
  send well inside the 30 s of wall time, so an accepted send is not cut
  off partway. A newline is the
  calculator's newline, not ENTER. Text with a character the model cannot
  type is refused before any key is pressed: the 48SX types 195 of the
  255 (no key gives the control characters other than newline, `;`, the
  backslash, the backquote, DEL, `∇ ▶ ■` and 23 Latin-1 signs); the 48GX
  and 49G type all of 1-255, some through their CHARS application; NUL
  is never typable (the editor refuses it).
- Every key is released first. The engine reads the line back after each
  character and stops with an error when it is not what it should be; a
  delimiter key that inserts a pair (`( ) [ ] { } « »`) is stepped over
  when the text closes it. Alpha lock, lowercase lock and a pending shift
  are as they were afterwards (`run`'s ENTER ends them, as on the
  calculator); typing a `√`-like character may leave the line in program
  entry mode. A line in replace mode (INS off) is refused.
- A send of more than 12 characters raises `busy` in the `status` event
  before its first key and holds the `frame`, `keys` and the key queue's
  `error` events until it is done; the `status` with `busy: false` comes
  before the next `frame`. A shorter one (a command name) shows as it is
  typed. A command's own error (one sent without an `id`) is not held:
  it is that command's answer.
- A send runs in turns between other messages, on every host. Meanwhile
  these commands are refused with the error "typing is in progress
  (releaseAll stops it)", the same list everywhere
  (`REFUSED_WHILE_TYPING` in `crates/saturnus-host/src/protocol/mod.rs`):
  the key commands `keyDown`, `keyUp`, `typeLetter`, `typeKeys`; another
  send (`insert`, `typeText`, `run`, `replace`); `boot`, `bootModel`,
  `chooseRom`, `reset`, `saveState`, `loadState`; the memory reads
  `memoryTree`, `stack`, `flags`, `objectAt`, `editText`; the writes
  `storeFile`, `fetchFile`, `purge`, `rename`, `createDir`, `changeDir`,
  `setFlag`, `storeText`; and the native
  `keyScript` and `poke`. `keyUpAll` is taken and does nothing; `releaseAll` stops
  the send (the send's reply is the error "cancelled", before
  `releaseAll`'s reply). Everything else is served (`hello`, `stats`,
  `commandLine`, `setSpeed`, `pause`, `watchMemory`, `romSlots`,
  `screen`, `info`, `peek`, ...). In the Tauri app a `bootModel`,
  `chooseRom` or `downloadRom` reaches the machine as a `boot`, which is
  refused: after `chooseRom` or `downloadRom` the files are remembered
  and the refusal is its `bootError`.
- Typing runs at once in emulated time. The ROM's own work after each
  key sets the pace: 2.4-3.7 characters per second of emulated time on
  the 48SX, 3.6-5.5 on the 48GX, 7-10 on the 49G (a program full of
  shifted characters, plain text); in wall time 140-230, 240-380 and
  460-680 characters per second in the browser, about 1.5 times that
  natively. Bounded by 30 s of wall time; HTTP stops it at its next turn
  when the request is withdrawn.

## Events (host to page)

`{"type": "frame", ...}`; a backend dispatches each as a DOM event of that
type with the message as `detail`.

| Event | Fields | When |
| --- | --- | --- |
| `frame` | `width`, `height`, `pixels`, `annunciators`, `contrast`, `contrastRange`, `contrastDefault` | After a boot, and whenever the pixels, annunciators or contrast changed since the last frame, at most about 60 per second. |
| `keys` | `down` (array of key names) | Whenever the set of keys down in the machine changed (typed letters included), for drawing pressed keys. |
| `status` | `model`, `romName`, `running`, `halted` (message or `null`), `speed`, `loop` (`"frame"`, `"sleep"` or `"stopped"`), `busy` (a long send is typing, see [Typing](#typing), or a write runs, see [The user memory, written](#the-user-memory-written)) | Whenever one of them changed. |
| `error` | `message` | A command without `id` failed, or a key the machine refused. |
| `memoryChanged` | | After `watchMemory`: the user memory (a variable anywhere under HOME, the current directory, the stack's levels, a flag) is no longer what it was at the last event or at `watchMemory`; the page reads again. Also when it became readable or unreadable. At most one per 250 ms. |
| `autoSaved` | `model`, `cycles` | The host has kept the machine's state in the model's auto slot ([Auto-save](#auto-save)). For tests and a page that wants to say so; the bytes stay with the host. |

`frame` fields:

- `width`, `height`: the pixel area, 131 x 64 on the 48 and 49 series and
  the aplet models; other models may differ, so the page sizes the display
  from the frame.
- `pixels`: base64 of the packed pixels: row-major from the top-left,
  each row starting on a byte (`ceil(width / 8)` bytes, 17 for 131),
  the leftmost pixel in the most significant bit, 1 = dark. 1088 bytes
  for 131 x 64.
- `annunciators`: `{leftshift, rightshift, alpha, alert, busy,
  transmit}` booleans, in strip order.
- `contrast`: the raw 5-bit contrast, 0-31, higher is darker;
  `contrastRange`: the model's usable `[low, high]`; `contrastDefault`:
  the value its ROM sets at power-on, which the page renders properly
  dark (about 0.9), fading towards `low`.

## Auto-save

The calculator keeps its state across a reload or a restart, as a real
one keeps its memory when turned off (iteration 27). The rules are the
state machine's (`crates/saturnus-host/src/protocol/autosave.rs`), the
same on every host that keeps states (`Engine::set_auto_save`: the Worker
and the Tauri app; `saturnus run` keeps none):

- A **change** is what the page or the outside did: a key (`keyDown`,
  `keyUp`, `typeLetter`, `typeKeys`), a send or a write (at its start and
  its end), `loadState`, `reset`, a native `poke` or `keyScript`, serial
  input. A machine that only keeps time (the clock, the cursor blink, the
  ROM's wakes) has not changed: an idle calculator is never written,
  which matters for the 49G, whose state carries its 2 MB flash. A boot,
  restored or cold, is not a change.
- The state is saved **5 s after the last change**, and at once when the
  page is hidden (`visibility`), but only once the machine has
  **settled**: no send or write in progress, the CPU asleep with no key
  down or queued, not halted. A save due while the machine computes or a
  write runs waits until it settles; in the browser a computation stops
  while the page is hidden, so its save waits until the page is shown
  again. Before any boot the host stores the machine it replaces, if it
  owes a save and settled (`Engine::save_now`), and only then reads or
  clears the slot: a reboot of the same model restores the newest state,
  and a fresh start never gets the old machine back.
- The state machine hands the state to its host (`Output::Save`), which
  keeps it in the model's **auto slot**, apart from the user's own saved
  state: the Worker in IndexedDB (`saturnus`/`states`, key
  `auto:<model>`, written by the Worker one after the other), the Tauri
  app in `states/<model>.auto.state` in its data folder. Then the page
  hears `autoSaved`.
- **On boot** (`bootModel`, `chooseRom`, `downloadRom`; the Tauri app's
  `boot`) the host gives the kept state to the state machine, which
  restores it before the machine runs a cycle: no "Try To Recover
  Memory?", the stack and variables as they were. A state that does not
  load (another ROM, another model, an older format) leaves the cold boot;
  `restoreError` says why, the host logs it, the page shows no error.
- **Start fresh** is `bootModel` with `fresh`: a cold boot, and the kept
  state is deleted. The user's saved state stays.

## Pacing

Every host runs the same rules, in the state machine
(`crates/saturnus-host/src/protocol/pacing.rs`); the hosts differ only in
the tuning defined there next to each other (`Pacing::WORKER`,
`Pacing::NATIVE`):

| | Worker | Native (Tauri, `saturnus run`) |
| --- | --- | --- |
| Pass period while computing | 1/60 s (an animation frame) | 1 ms (the serial bridge is served between passes) |
| Pass budget (1x-4x / Max) | 22 / 11 ms | 4 / 11 ms |
| Wake budget (visible / hidden) | 22 / 200 ms | 22 ms |
| `frame` events from passes | every pass | at most every 16 ms |
| A send's turn | 40 ms | 16 ms |
| Hidden page | passes stop while computing | ignored |
| Timer may fire early by | 1 ms (`setTimeout` truncates) | 0 |

- While the CPU computes or keys are queued, the host runs emulated time
  equal to the elapsed wall time times the speed (at most 100 ms of wall
  time per pass), in slices of at most 10 emulated ms with the key queue
  fed after each; time a pass cannot fit into its wall-time budget is
  dropped, as a real calculator never runs in bursts. A pass stops where
  the CPU goes to sleep with nothing queued; the rest of its wall time
  belongs to the sleep. At Max a pass runs up to 1000 emulated ms within
  its budget.
- While the CPU sleeps in SHUTDN with nothing queued, the host stops and
  sets a timer for the next timer event. On waking (the timer, a key)
  it runs all emulated time that passed at 1x, whatever the speed, up
  to 12 hours, cheaply,
  since the core jumps over SHUTDN; what does not fit the wake's budget
  stays owed and is paid first by the next passes.
- Keys are timed in emulated time (`crates/saturnus-host/src/host.rs`,
  shared by all hosts): each press is held at least 60 ms, presses are at
  least 30 ms apart, and a press waits for the ROM to go idle after the
  previous one (at most 300 ms).
- The user memory is looked at for `memoryChanged` on the machine's side,
  never by the page: only while a page watches, only when the machine ran
  since the last look, only while it is not computing (the ROM's
  structures are whole when it waits for a key; a long computation is
  reported when it ends), not while a send types, at most every 100 ms,
  and not within 250 ms of the last event; a look that is due while the
  host sleeps is made by a timer. A look hashes HOME and the stack's pointers
  (`UserMemory::change_counter`); it is not a run pass, and an idle
  calculator with a watching page still sleeps (the 48's ROM wakes twice
  a second, so two looks a second, about 0.05 ms each).
- `keyScript` and the typing commands are the exception: they run at
  once in emulated time, as fast as the host can (a calculator waiting
  for keys costs nearly nothing), and the clock follows the wall clock
  again from where they left it. A send runs in turns between other
  messages (see [Typing](#typing)); a `keyScript` holds the machine
  thread until it is done, so `saturnus run`'s serial bridge waits too:
  do not send keys during a Kermit transfer.
- `saturnus run` also serves its serial bridge from the machine thread
  between passes: bytes from the client wake a sleeping CPU at once, and
  while a client is connected the thread looks at the socket at least
  every millisecond.

## HTTP

`saturnus run` serves the protocol over HTTP/1.1 on 127.0.0.1 (default
port 4840; `--control PORT`, `SATURNUS_CONTROL`), one request per
connection. Every request carries `Authorization: Bearer <token>` from
the per-user token file and a `Host` of `127.0.0.1:PORT` or
`localhost:PORT`; the security rules are in
`kb/docs/control-api-security.md`. A `POST` body is a protocol command as
JSON (`Content-Type: application/json`; `v` may be left out, an `id` is
ignored); the response is the protocol's reply object,
`{"type": "reply", "ok": true, "result": ...}` or
`{"type": "reply", "ok": false, "error": "..."}`. Events are not sent:
HTTP clients ask (`screen`, `info`, `cycles`).

| Method and path | Command | Body / query | Response |
| --- | --- | --- | --- |
| `GET /v1/screen` | `screen` | `?scale=N` with `Accept: image/png` (or `?format=png`) | reply; `image/png` with `Accept: image/png` |
| `POST /v1/keys` | `keyScript`, `keyDown`, `keyUp`, `keyUpAll`, `typeKeys`, `releaseAll` | the command | reply (`keyScript` when the calculator is idle again) |
| `POST /v1/type` | `insert`, `run`, `replace`, `typeText` | the command | reply, when the send is done |
| `GET /v1/cmdline` | `commandLine` | | reply |
| `GET /v1/mem` | `peek` | `?address=N&length=N` (decimal, or hex after `0x` or `%23`) | reply |
| `POST /v1/mem` | `poke` | the command | reply |
| `GET /v1/snapshot` | `saveState` | | the state, `application/octet-stream` |
| `PUT /v1/snapshot` (or `POST`) | `loadState` | the state, `application/octet-stream`, at most 4 MiB | reply |
| `GET /v1/info` | `info` | | reply |
| `GET /v1/cycles` | `stats` | | reply |
| `GET /v1/model` | `model` | | reply |
| `GET /v1/stack`, `/v1/tree`, `/v1/flags` | `stack`, `memoryTree`, `flags` | | reply |
| `GET /v1/object` | `objectAt` | `?address=N` (as for `/v1/mem`; 0 to #FFFFF) | reply |
| `POST /v1/memory` | `storeFile`, `fetchFile`, `purge`, `rename`, `createDir`, `changeDir`, `setFlag`, `storeText`, `editText` | the command (`storeFile`'s `data` as base64; at most 687 KiB of JSON: a 512 KiB file in base64 and 4 KiB more) | reply, when the write is done (`fetchFile`'s `data` as base64) |
| `GET /v1/hello` | (none) | `?nonce=N` (64 hex digits), without the token | `{proof}`: HMAC-SHA-256 under the token of `saturnus control hello\nN\nPORT` (PORT the server's bound port), as hex; a client checks it before it sends the token (`kb/docs/control-api-security.md`) |

`GET` never changes anything; each endpoint takes only its own commands
(`boot`, `setSpeed`, `pause`, `reset` are not served over HTTP).

| Status | When |
| --- | --- |
| 200 | Done; the reply (or the PNG, or the state). |
| 400 | Malformed: not HTTP/1.x, a duplicated or folded header, `Transfer-Encoding`, bad JSON, a command not of this endpoint, a bad query. |
| 401 | No token or a wrong one; the body is `{}`, with no detail. |
| 403 | An `Origin` header other than the API's own (`http://127.0.0.1:PORT`, `http://localhost:PORT`). |
| 404 | No such endpoint. |
| 405 | `OPTIONS` (no CORS, ever), or a method the endpoint does not take; `Allow` lists them. |
| 408 | The head did not arrive within 2 s, or the body within 20 s. |
| 413 | A body over the cap: 256 KiB of JSON (687 KiB on `/v1/memory`), 4 MiB of state. |
| 415 | A body of the wrong `Content-Type`. |
| 421 | A `Host` other than `127.0.0.1:PORT` or `localhost:PORT`, or none. |
| 422 | The machine refused the command (the reply's `error`: out-of-range memory, a state of another ROM, a halted CPU, a key script error). |
| 431 | A head over 16 KiB or 64 header lines. |
| 503 | 8 authenticated requests already in progress, 8 commands already waiting for the machine ("the machine's queue is full"), or the machine thread is gone. Nothing ran; try again. |
| 504 | The machine did not take the command within 90 s: it **did not run and will not** (a retry is safe). Or, if it had started, the reply says it was stopped (see below). |

**What a 504 or a lost connection means.** Every command travels with a
ticket that the machine thread takes when it starts the command and the
server takes back when its caller gives up: after 90 s, or as soon as the
client closes its connection (looked at every 200 ms). Exactly one of the
two wins. If the server wins, the command never runs. If the machine had
already started it, a `keyScript` is stopped at its next slice (within
about 50 emulated ms) and a send or a write at its next turn (the presses
before that took effect, and every key is released; a write's server is
ended with ON) and the 504 says so; any other command is
short and finishes, and its normal reply is sent. A refused connection
(a 503) never queued anything. Connections that have not yet sent a
valid head count against a separate budget of 16 (the oldest is dropped
for a new one), so idle connections cannot hold the 8 request slots.
