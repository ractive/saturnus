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
the file of `boot`, `saveState` and `loadState` in a native dialog, and
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
| `skin` | `model` | the skin JSON (`crates/saturnus-web/src/skins`, with `letters` and `typing`) | Static data for drawing a model before and after boot. |
| `layout` | `model` | `{columns, rows, keys: [{name, label, alpha?, row, x, w}]}` | The plain button grid of a model. |
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
| `stats` | | `{cycles, emulatedMs, workMs, ticks, wakes, loop, owedMs, nowMs}` | Counters for tests: `workMs` is the host's busy wall time, `ticks` its run passes, `wakes` its wakes from sleep, `owedMs` the emulated time owed to the wall clock (unpaid, plus the current sleep), `nowMs` the host's clock. `emulatedMs + owedMs` grows with wall time times the speed. |

The native hosts (Tauri, HTTP; the machine thread in
`crates/saturnus-drive/src/runner.rs`) also take these commands; the
Worker answers them with "unknown command". A read command returns its
answer as the reply's `result`; a host that cannot answer (an aplet model
or the 42S has no RPL user memory) replies with an error.

| Command | Fields | Result | Does |
| --- | --- | --- | --- |
| `screen` | `png` (boolean), `scale` (1-8) | the `frame` event's fields, plus `rows` (one string per pixel row, `#` dark, `.` light, as the CLI's `.txt` screens) and `displayOn`; with `png`: `{png` (*bytes*, a 1-bit PNG)`, width, height}` | The display now. |
| `info` | | `{protocol, host, model, romName, running, halted, speed, loop, displayOn, cycles}` and the host's own fields (HTTP: `romSha256`, `romRevision` or `null`, `serial` endpoint or `null`, `control` URL) | What runs, and where. |
| `model` | | `{model, clockHz, width, height, hasSerial, layout}` (`layout` as the `layout` command's result) | The running model. |
| `keyScript` | `script` (text) | `{emulatedMs, warnings}` | Runs a key script (README, "Key scripts"; a line may also hold several key names, and `+ - * / .` name those keys) at once in emulated time and replies when it is done; every key is released first. At most 64 KiB, 2000 lines, 10 minutes of emulated time and 30 s of wall time; a `wait-idle` that reaches its cap is a warning. |
| `typeText` | `text` (at most 1000 characters) | `{emulatedMs, warnings}` | Types letters through alpha mode (as `typeLetter`), digits, space, `+ - * / .` and newline (ENTER) as plain presses, then waits until idle (at most 2 s). Text with a character the model cannot type is refused before any key is pressed. |
| `peek` | `address`, `length` (nibbles) | `{address, nibbles}` (hex digits) | Reads memory through the current mapping, without side effects. |
| `poke` | `address`, `nibbles` (hex digits) | `{address, length}` | Writes memory as CPU writes would (ROM ignores them, I/O registers react). |
| `memoryTree` | | `{path, variables}` | HOME's tree, read from RAM (48SX, 48GX, 49G). |
| `stack` | | the typed levels, level 1 first | The stack, read from RAM. |
| `flags` | | `{system, user, set}` | The flags, read from RAM. |
| `objectAt` | `address` | the typed object | One variable's value (its `address` from `memoryTree`). |

`peek` and `poke` stay inside the 20-bit address space (`#00000` to
`#FFFFF`, no wrap-around) and move at most 65536 nibbles per command. A
message never names a file: any `romPath` or `path` field is refused by
every native host.

Reserved for later versions (unknown commands get an error reply):
`eval`, `transfer`.

## Events (host to page)

`{"type": "frame", ...}`; a backend dispatches each as a DOM event of that
type with the message as `detail`.

| Event | Fields | When |
| --- | --- | --- |
| `frame` | `width`, `height`, `pixels`, `annunciators`, `contrast`, `contrastRange` | After a boot, and whenever the pixels, annunciators or contrast changed since the last frame, at most about 60 per second. |
| `keys` | `down` (array of key names) | Whenever the set of keys down in the machine changed (typed letters included), for drawing pressed keys. |
| `status` | `model`, `romName`, `running`, `halted` (message or `null`), `speed`, `loop` (`"frame"`, `"sleep"` or `"stopped"`) | Whenever one of them changed. |
| `error` | `message` | A command without `id` failed, or a key the machine refused. |
| `memoryChanged` | | Reserved: the user memory changed (later, with `memoryTree`). |

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
- Keys are timed in emulated time (`crates/saturnus-web/src/host.rs`,
  shared by all hosts): each press is held at least 60 ms, presses are at
  least 30 ms apart, and a press waits for the ROM to go idle after the
  previous one (at most 300 ms).
- `keyScript` and `typeText` are the exception: they run at once in
  emulated time, as fast as the host can (a calculator waiting for keys
  costs nearly nothing), and the clock follows the wall clock again from
  where they left it. Meanwhile the machine thread does nothing else, so
  `saturnus run`'s serial bridge waits too: do not send keys during a
  Kermit transfer.
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
| `POST /v1/type` | `typeText` | the command | reply, when idle again |
| `GET /v1/mem` | `peek` | `?address=N&length=N` (decimal, or hex after `0x` or `%23`) | reply |
| `POST /v1/mem` | `poke` | the command | reply |
| `GET /v1/snapshot` | `saveState` | | the state, `application/octet-stream` |
| `PUT /v1/snapshot` (or `POST`) | `loadState` | the state, `application/octet-stream`, at most 4 MiB | reply |
| `GET /v1/info` | `info` | | reply |
| `GET /v1/cycles` | `stats` | | reply |
| `GET /v1/model` | `model` | | reply |
| `GET /v1/stack`, `/v1/tree`, `/v1/flags` | `stack`, `memoryTree`, `flags` | | reply |

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
already started it, a `keyScript` or `typeText` is stopped at its next
slice (within about 50 emulated ms; the presses before that took effect,
and every key is released) and the 504 says so; any other command is
short and finishes, and its normal reply is sent. A refused connection
(a 503) never queued anything. Connections that have not yet sent a
valid head count against a separate budget of 16 (the oldest is dropped
for a new one), so idle connections cannot hold the 8 request slots.
