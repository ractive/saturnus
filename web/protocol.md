# saturnus front-end protocol, version 1

The page (`web/`) talks to the emulator only through a **backend** object,
and the backend talks to its host in the messages below. Three hosts speak
it:

| Host | Backend | Transport |
| --- | --- | --- |
| Browser | `WorkerBackend` (`backend.js`) | `postMessage` to `worker.js`, which runs the wasm core |
| Tauri app | `TauriBackend` (`backend.js`) | `invoke("command", {msg})` and the `saturnus` event (`crates/saturnus-tauri`) |
| `saturnus run` | `saturnus ctl`, `curl`, any HTTP client | HTTP/1.1 on 127.0.0.1, see [HTTP](#http) below (`crates/saturnus-cli/src/control`) |

The Tauri app and `saturnus run` share one implementation of the host
side, the machine thread in `crates/saturnus-drive/src/runner.rs`.

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

`v` is the protocol version; a host refuses another major version with an
error reply. `id` is optional: with an `id` the host answers with exactly
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
| `hello` | | `{protocol: 1, host: "worker" \| "tauri", models: ["48sx", ...]}` | Handshake; the models this host runs. The host also sends its current `status`, `keys` and `frame` again (before the reply), so a page that was reloaded while the machine kept running shows it at once. In the browser a reload starts a new Worker, and with it a new, empty machine; the Tauri app's machine outlives the page, so after a reload it is still running. |
| `skin` | `model` | the skin JSON (`crates/saturnus-host/src/skins`, with `letters` and `typing`) | Static data for drawing a model before and after boot. |
| `layout` | `model` | `{columns, rows, keys: [{name, label, alpha?, row, x, w}]}` | The model's keys in rows with their labels: the native hosts' `model` result carries it (`saturnus ctl model` lists the key names); the page draws skins (`skin`) instead. |
| `boot` | `model`, then `rom` (*bytes*) and `romName` (Worker); nothing more for Tauri, which asks for the ROM in a file dialog | `{model, romName}` or `null` (dialog cancelled) | Builds the machine from the ROM and starts running. `model` is a preference: a ROM that only fits another model boots that model. |
| `keyDown` | `key` | | Queues a press of the key (script name, as in `Key::name`), held until `keyUp`. Wakes a sleeping machine. An error for a key the model does not have, or before a ROM is booted. |
| `keyUp` | `key` | | Releases the newest held press of that key, once it was down at least 60 emulated ms (nothing if none is held). Errors as `keyDown`. |
| `keyUpAll` | | | Releases every held key (the window lost the focus). |
| `typeLetter` | `letter` (one character) | `true` if the model can type it | Types the letter through the model's alpha mode, lowercase through its shift (see `web/README.md`, Keyboard). |
| `typeKeys` | `keys` (array of names) | | Full presses of the keys, one after the other (the 38G's space is `["shift", "2"]`). All or nothing: an entry that is not a key name of the model, or no booted ROM, is an error and presses nothing. |
| `releaseAll` | | | Releases every key and drops the queue at once. |
| `setSpeed` | `speed`: `"1"`, `"2"`, `"4"` or `"max"` | | Emulated time per wall time; at `max` as fast as the host can while staying responsive. |
| `pause` | `paused` (boolean) | | The Run/Pause switch. |
| `reset` | | | Hardware reset (RAM kept); releases the keys and runs. |
| `saveState` | (Tauri: none; it shows a save dialog) | Worker and HTTP: `{state` (*bytes*)`, cycles}`; Tauri: `{path}` or `null` | The whole machine state, bound to model and ROM. The `WorkerBackend` keeps it in IndexedDB, one slot per model. |
| `loadState` | `state` (*bytes*, Worker and HTTP, at most 4 MiB); nothing for Tauri, which shows an open dialog | `{}` or `null` (cancelled) | Restores a saved state of the same model and ROM; releases the keys. |
| `visibility` | `hidden` (boolean) | | The page is hidden: a computing machine stops as an animation frame would; a sleeping one still keeps time. |
| `stats` | | `{cycles, emulatedMs, workMs, ticks, wakes, memoryLooks, memoryMs, loop, owedMs, nowMs}` | Counters for tests: `workMs` is the host's busy wall time, `ticks` its run passes, `wakes` its wakes from sleep, `memoryLooks` its looks at the user memory for `memoryChanged` and `memoryMs` the wall time they took, `owedMs` the emulated time owed to the wall clock (unpaid, plus the current sleep), `nowMs` the host's clock. `emulatedMs + owedMs` grows with wall time times the speed. |

### ROM slots

Every host but HTTP remembers the ROM of each model, so the user chooses
it once: the Worker keeps the ROM's bytes in the browser's IndexedDB
(database `saturnus-roms`), the Tauri app keeps the file's path in its
settings file (`settings.json` in the platform's config directory for
`ch.ractive.saturnus`, mode 0600 on Unix). Paths never cross to the page:
a slot tells only the file's name. `saturnus run` takes its ROM on the
command line and does not serve these commands.

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
| `romSlots` | | `{slots, offers, lastModel, bootLast, remembered, note}` | The slots: `slots` one per model in `hello`'s order, `{model, fileName, revision, state}` with `state` `empty`, `ready`, `missing` (Tauri: the file is gone) or `changed` (its content is no longer what was chosen); `offers` the files that may be a model's ROM, `{id, models, fileName}`; `lastModel` the model booted last; `bootLast` whether the page boots it when it opens; `remembered` whether the host keeps the slots beyond this page or app run (`false` when the browser refuses to store); `note` why not, or `null`. |
| `bootModel` | `model` | the slots, plus `booted` (`boot`'s result) and `notice` | Boots `model` from its remembered ROM after checking it is still there and unchanged. An error when it is not (the page reports it and, in the app, asks for the file again); nothing else boots in its place. |
| `chooseRom` | `model`, and `files` (Worker: `[{name, rom` (*bytes*)`}]`, one or more) or `offer` (an offer's `id`); Tauri without `offer`: nothing more, it asks in a file dialog | the slots, plus `booted` (`boot`'s result or `null`), `notice` (what else was found or offered, and why a file was not taken) and `bootError` (why the boot failed, or `null`: the files are remembered all the same, so a failed boot is not the command's error); `null` if the dialog was cancelled | Identifies the files (Tauri: the chosen one and those beside it), assigns them to their slots, remembers them and boots `model` if it got a ROM, else the first model a chosen file went to. With `offer`, takes that offered file as `model`'s ROM and boots it. |
| `forgetRom` | `model` (optional) | the slots | Forgets `model`'s ROM, or every ROM and the last model without it: the Worker deletes the bytes from IndexedDB, the Tauri app the paths from its settings (the files stay). Saved states are not touched. |
| `romSettings` | `bootLast` (boolean) | the slots | Whether the last model boots when the page opens. |

`boot` stays as it was (a ROM given once, not remembered). The Worker
handles one command after the other, in the order they came, so a key
sent while a ROM command waits for IndexedDB follows it.

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
`eval`, `transfer`.

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
  before its first key and holds the frames until it is done; the
  `status` with `busy: false` comes before the next `frame`. A shorter
  one (a command name) shows as it is typed. The Web Worker serves other
  messages meanwhile but refuses key commands (`keyDown`, `keyUp`,
  `typeLetter`, `typeKeys`) and `boot`, `bootModel` and `chooseRom`
  with an error, and ignores
  `keyUpAll`; `releaseAll` stops a send (its reply is an error). The
  native hosts take one command at a time, so nothing comes in during a
  send there.
- Typing runs at once in emulated time. The ROM's own work after each
  key sets the pace: 2.4-3.7 characters per second of emulated time on
  the 48SX, 3.6-5.5 on the 48GX, 7-10 on the 49G (a program full of
  shifted characters, plain text); in wall time 140-230, 240-380 and
  460-680 characters per second in the browser, about 1.5 times that
  natively. Bounded by 30 s of wall time; HTTP stops it as a `keyScript`
  when the request is withdrawn.

## Events (host to page)

`{"type": "frame", ...}`; a backend dispatches each as a DOM event of that
type with the message as `detail`.

| Event | Fields | When |
| --- | --- | --- |
| `frame` | `width`, `height`, `pixels`, `annunciators`, `contrast`, `contrastRange` | After a boot, and whenever the pixels, annunciators or contrast changed since the last frame, at most about 60 per second. |
| `keys` | `down` (array of key names) | Whenever the set of keys down in the machine changed (typed letters included), for drawing pressed keys. |
| `status` | `model`, `romName`, `running`, `halted` (message or `null`), `speed`, `loop` (`"frame"`, `"sleep"` or `"stopped"`), `busy` (a long send is typing, see [Typing](#typing)) | Whenever one of them changed. |
| `error` | `message` | A command without `id` failed, or a key the machine refused. |
| `memoryChanged` | | After `watchMemory`: the user memory (a variable anywhere under HOME, the current directory, the stack's levels, a flag) is no longer what it was at the last event or at `watchMemory`; the page reads again. Also when it became readable or unreadable. At most one per 250 ms. |

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
  `contrastRange`: the model's usable `[low, high]`.

## Pacing

All hosts follow the same rules (the Worker in `worker.js`, Tauri and
`saturnus run` in `crates/saturnus-drive/src/runner.rs`):

- While the CPU computes or keys are queued, the host runs emulated time
  equal to the elapsed wall time times the speed, in slices of at most
  10 emulated ms with the key queue fed after each; time a pass cannot
  fit into its wall-time budget is dropped, as a real calculator never
  runs in bursts.
- While the CPU sleeps in SHUTDN with nothing queued, the host stops and
  sets a timer for the next timer event. On waking (the timer, a key)
  it runs all emulated time that passed, up to 12 hours, cheaply,
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
  reported when it ends), at most every 100 ms, and not within 250 ms of
  the last event; a look that is due while the host sleeps is made by a
  timer. A look hashes HOME and the stack's pointers
  (`UserMemory::change_counter`); it is not a run pass, and an idle
  calculator with a watching page still sleeps (the 48's ROM wakes twice
  a second, so two looks a second, about 0.05 ms each).
- `keyScript` and the typing commands are the exception: they run at
  once in emulated time, as fast as the host can (a calculator waiting
  for keys costs nearly nothing), and the clock follows the wall clock
  again from where they left it. Meanwhile the machine thread does
  nothing else, so `saturnus run`'s serial bridge waits too: do not send
  keys during a Kermit transfer.
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
| 413 | A body over the cap: 256 KiB of JSON, 4 MiB of state. |
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
already started it, a `keyScript` or a send is stopped at its next
slice (within about 50 emulated ms; the presses before that took effect,
and every key is released) and the 504 says so; any other command is
short and finishes, and its normal reply is sent. A refused connection
(a 503) never queued anything. Connections that have not yet sent a
valid head count against a separate budget of 16 (the oldest is dropped
for a new one), so idle connections cannot hold the 8 request slots.
