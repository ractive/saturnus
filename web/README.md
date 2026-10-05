# saturnus web UI

A static page around the WebAssembly build of the saturnus core
(`crates/saturnus-web`). Plain HTML, CSS and an ES module; no framework or
bundler.

## Build and serve

```sh
cargo install wasm-pack          # once; needs the wasm32-unknown-unknown target
./build.sh                       # wasm-pack build --target web into ./pkg (gitignored)
python3 -m http.server 4860      # any static file server works
```

Open http://127.0.0.1:4860/. Browsers do not load ES modules or wasm from
`file://`, so a server is needed.

## Use

1. Choose the model, then the ROM file. The ROM is read in the page with the
   File API; it is never uploaded or stored. A ROM whose size fits only one
   model switches the model selector to it.
2. Keys: click or tap the drawn keys, or use the keyboard: digits,
   `+ - * /`, `.` or `,`, Space, Enter, Backspace, Delete (DEL), arrows,
   `'` (quote), `^` (power), Escape (ON), F1-F6 (menu keys).
3. Run/Pause stops emulated time. Reset is the hardware reset (RAM kept).
4. Save state stores the machine in IndexedDB, one slot per model. After a
   reload, pick the ROM again and press Load state. A state only loads with
   the ROM it was saved from.

Stored in the browser: the model choice (localStorage key `saturnus.model`)
and the saved states (IndexedDB database `saturnus`, store `states`).

## Timing

Each animation frame runs the wall time since the previous frame, capped at
100 ms (a hidden tab does not catch up), in 10 ms slices. Every key press is
held for at least 60 ms of emulated time, and presses typed faster than that
are queued with a 30 ms gap, so the ROM's debounce sees each one. A key you
keep holding (ON for a chord) does not block the next press.

`window.saturnus` exposes the emulator and `screenText()` (the LCD as `#`
and `.` lines) for debugging and automated checks.
