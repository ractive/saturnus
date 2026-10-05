# saturnus front-end protocol, version 1

The page (`web/`) talks to the emulator only through a **backend** object,
and the backend talks to its host in the messages below. Two hosts speak
it:

| Host | Backend | Transport |
| --- | --- | --- |
| Browser | `WorkerBackend` (`backend.js`) | `postMessage` to `worker.js`, which runs the wasm core |
| Tauri app | `TauriBackend` (`backend.js`) | `invoke("command", {msg})` and the `saturnus` event (`crates/saturnus-tauri`) |

The page picks the Tauri backend when `window.__TAURI__` exists and the
Worker otherwise; nothing else in the page knows which host it runs on.
The host owns the machine, paces it against the wall clock (speed factor,
idle sleep while the CPU is in SHUTDN) and **pushes** events; the page
never polls.

Messages are JSON objects (the Worker passes them by structured clone,
which also carries the `Uint8Array` fields marked *bytes*; Tauri
serialises them as JSON). Field names are camelCase.

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
| `keyDown` | `key` | | Queues a press of the key (script name, as in `Key::name`), held until `keyUp`. Wakes a sleeping machine. |
| `keyUp` | `key` | | Releases the newest held press of that key, once it was down at least 60 emulated ms. |
| `keyUpAll` | | | Releases every held key (the window lost the focus). |
| `typeLetter` | `letter` (one character) | `true` if the model can type it | Types the letter through the model's alpha mode, lowercase through its shift (see `web/README.md`, Keyboard). |
| `typeKeys` | `keys` (array of names) | | Full presses of the keys, one after the other (the 38G's space is `["shift", "2"]`). |
| `releaseAll` | | | Releases every key and drops the queue at once. |
| `setSpeed` | `speed`: `"1"`, `"2"`, `"4"` or `"max"` | | Emulated time per wall time; at `max` as fast as the host can while staying responsive. |
| `pause` | `paused` (boolean) | | The Run/Pause switch. |
| `reset` | | | Hardware reset (RAM kept); releases the keys and runs. |
| `saveState` | (Tauri: none; it shows a save dialog) | Worker: `{state` (*bytes*)`, cycles}`; Tauri: `{path}` or `null` | The whole machine state, bound to model and ROM. The `WorkerBackend` keeps it in IndexedDB, one slot per model. |
| `loadState` | `state` (*bytes*, Worker); nothing for Tauri, which shows an open dialog | `{}` or `null` (cancelled) | Restores a saved state of the same model and ROM; releases the keys. |
| `visibility` | `hidden` (boolean) | | The page is hidden: a computing machine stops as an animation frame would; a sleeping one still keeps time. |
| `stats` | | `{cycles, emulatedMs, workMs, ticks, wakes, loop, owedMs, nowMs}` | Counters for tests: `workMs` is the host's busy wall time, `ticks` its run passes, `wakes` its wakes from sleep, `owedMs` the emulated time owed to the wall clock (unpaid, plus the current sleep), `nowMs` the host's clock. `emulatedMs + owedMs` grows with wall time times the speed. |

Reserved for later versions (unknown commands get an error reply):
`eval`, `memoryTree`, `stack`, `objectAt`, `transfer`. A read command
returns its answer as the reply's `result`; a host that cannot answer
(an aplet model has no user memory) replies with an error.

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

Both hosts follow the same rules (the Worker in `worker.js`, Tauri in
`crates/saturnus-tauri/src/runner.rs`):

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
  shared by both hosts): each press is held at least 60 ms, presses are at
  least 30 ms apart, and a press waits for the ROM to go idle after the
  previous one (at most 300 ms).
