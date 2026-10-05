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
2. Keys: click or tap the keys of the drawn calculator, or use the keyboard: digits,
   `+ - * /`, `.` or `,`, Space, Enter, Backspace, Delete (DEL), arrows,
   `'` (quote), `^` (power), Escape (ON), F1-F6 (menu keys).
3. Run/Pause stops emulated time. Reset is the hardware reset (RAM kept).
4. Save state stores the machine in IndexedDB, one slot per model. After a
   reload, pick the ROM again and press Load state. A state only loads with
   the ROM it was saved from.

Stored in the browser: the model choice (localStorage key `saturnus.model`),
the view (`saturnus.view`, `skin` or `grid`) and the saved states (IndexedDB database `saturnus`, store `states`).

## Skins

The calculator is drawn as an SVG skin per model: 48SX, 48GX, 38G, 49G,
and 39G (the 40G shows the 39G drawing with its own name). The "Drawn
calculator" box switches between the skin and the plain button grid; both
press the same keys, by name, with the same timing, and the computer
keyboard works in both.

- **Data.** Each skin is Rust data in `crates/saturnus-web/src/skins/`
  (one file per model), handed to the page as JSON by `skin(model)` and
  `Emulator.skin()`. A unit test checks that every key of the model's
  matrix is drawn exactly once, that keys do not overlap, and that the
  alpha letters agree with the ROM and the wiki.
- **Measured.** Key rectangles, the case outline, the display window and
  the bezel come from the keyboard line drawings in HP's manuals, rendered
  as images and measured in pixels. The unit is 1/100 of the menu-key
  pitch of each figure; the origin is the case's top-left corner. The
  labels on and above each key are transcribed from the same figures.
- **Read off photographs.** Case, key and label colours. Nothing from any
  figure or photograph is reproduced: the page draws rounded rectangles,
  trapezoids and text.
- **Drawn freely.** Shadows, the pressed-key look, fonts, the saturnus
  logo where the HP logo was, the model name as plain text.

Which figure and photo each model comes from, and what is inferred, is in
the comment at the top of its file.

The LCD canvas sits in the drawn display window. The skin is sized so the
LCD gets the largest whole number of CSS pixels per LCD pixel (2 or more)
that fits the page width and the window height; on a short window it
stays at 2 and the page scrolls; only when even 2 is too wide (phones)
does the skin fill the width at a fractional scale. The canvas renders at
the device pixel ratio, so pixels stay square and sharp.

The skin keeps the calculator's colours (case, keys, LCD) in dark mode;
only the page around it follows the colour scheme.

## Timing

Each animation frame runs the wall time since the previous frame, capped at
100 ms (a hidden tab does not catch up), in 10 ms slices. Every key press is
held for at least 60 ms of emulated time, and presses typed faster than that
are queued with a 30 ms gap, so the ROM's debounce sees each one. A key you
keep holding (ON for a chord) does not block the next press.

`window.saturnus` exposes the emulator and `screenText()` (the LCD as `#`
and `.` lines) for debugging and automated checks.
