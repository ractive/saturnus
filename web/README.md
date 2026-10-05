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

## Layout

The calculator is the page: it fills the window's height, or its width on a
narrow screen. On a wide window the controls sit in a side panel on the
left, which the `‹` button hides (a `☰` button brings it back; the choice is
remembered). Below 760 px the panel becomes a sheet that drops down from a
compact top bar, which also carries the fullscreen button.

**Fullscreen** shows the calculator alone, on a dark background, using the
browser's Fullscreen API; the `✕` in the corner or the panel's button leaves
it. Escape leaves fullscreen in every browser; in Chromium the page locks
Escape so it keeps working as ON, and holding Escape leaves instead. The
`` ` `` key is ON everywhere.

## Use

1. Choose the model, then the ROM file. The ROM is read in the page with the
   File API; it is never uploaded or stored. A ROM whose size fits only one
   model switches the model selector to it.
2. Keys: click or tap the keys of the drawn calculator, or use the keyboard
   (below).
3. Run/Pause stops emulated time. Reset is the hardware reset (RAM kept).
4. Save state stores the machine in IndexedDB, one slot per model. After a
   reload, pick the ROM again and press Load state. A state only loads with
   the ROM it was saved from.
5. Speed: 1×, 2×, 4× or Max. Above 1× the calculator's clock runs fast too;
   at Max the page runs as much emulated time per animation frame as fits
   in about 11 ms of wall time, at most one emulated second per frame, so
   the page stays responsive. The setting is remembered.

Stored in the browser: the model (localStorage `saturnus.model`), the view
(`saturnus.view`, `skin` or `grid`), the speed (`saturnus.speed`), whether
the side panel is hidden (`saturnus.panel`) and the saved states (IndexedDB
database `saturnus`, store `states`).

## Keyboard

| Keys | Calculator |
| --- | --- |
| `a`–`z`, `A`–`Z` | the letter, through the model's alpha mode; lowercase through its shift (the 38G has no space in alpha mode; the 39G/40G space is alpha + plus) |
| `Tab` | α (one press for the next key; twice for alpha lock on the 48 and 49G, where a third press unlocks; the 38G, 39G and 40G cancel on the second press) |
| `[`, `]` | left and right shift; `[` is the only shift on the 38G, 39G and 40G |
| `Esc`, `` ` `` | ON (see Fullscreen above) |
| `F1`–`F6` | the six menu keys |
| `0`–`9`, `.`/`,`, `+ - * /`, `^`, `'` | as printed |
| `Enter`, `Space`, `Backspace`, `Delete`, arrows | ENTER, SPC, ⬅, DEL, the cursor keys |

A typed letter is expanded into key presses when its turn in the key queue
comes: the alpha key unless the alpha annunciator is already on, the shift
for a lowercase letter (after alpha on the 48 and 49G, before it on the
aplet models), then the letter's key. Right after a letter the page pressed
alpha for, alpha is known to be off again (one-shot), so a run of letters
needs no waiting; otherwise the page waits for the ROM to go idle before it
reads the annunciator, which the 48SX ROM blinks while redrawing.

## Skins

The calculator is drawn as an SVG skin per model: 48SX, 48GX, 38G, 49G,
and 39G (the 40G shows the 39G drawing with its own name). The "Drawn
calculator" box switches between the skin and the plain button grid; both
press the same keys, by name, with the same timing, and the computer
keyboard works in both.

- **Data.** Each skin is Rust data in `crates/saturnus-web/src/skins/`
  (one file per model), handed to the page as JSON by `skin(model)` and
  `Emulator.skin()`, with the model's letter map and typing rules. Unit
  tests check that every key of the model's matrix is drawn exactly once,
  that keys do not overlap, that the alpha letters agree with the ROM and
  the wiki, that every model types all 26 letters, that the six softkey
  labels the ROM draws sit over the six menu keys, and that the case ends
  shortly below the bottom key row.
- **Measured.** Key rectangles, the case outline and the display window
  come from the keyboard line drawings in HP's manuals, rendered as images
  and measured in pixels. The unit is 1/100 of the menu-key pitch of each
  figure; the origin is the case's top-left corner. The labels on and
  above each key are transcribed from the same figures.
- **Display window.** The window is the LCD's active area (131 x 72 pixels
  at square pixels, the annunciator strip included). Every ROM draws the
  six menu labels at a 22-pixel pitch from column 0 (wiki:
  hardware/display "Menu labels"), so the window is 5.95 menu-key pitches
  wide and placed so label *i* is centred over menu key *i*; the bezel
  around it follows.
- **Read off photographs.** Case, key and label colours. Nothing from any
  figure or photograph is reproduced: the page draws rounded rectangles,
  trapezoids and text.
- **Drawn freely.** The relief: three gradients defined once in the SVG
  (a light falling on each cap, a rim lit above and shaded below, a light
  on the case), a shadow under each cap; a pressed key moves down onto its
  shadow and darkens. Fonts, the saturnus logo where the HP logo was, the
  model name as plain text.

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
event (the 48 wakes for nothing every half second), the page sleeps on
without a frame. No counters are updated per frame; the status line changes
only on events.

Keys: every press is held for at least 60 ms of emulated time. A queued
press starts 30 ms after the previous release and once the ROM has gone
idle again (the 48SX ROM drops a key pressed while it is still handling the
last one, 70-230 ms); while the ROM stays busy, as in a running program, a
queued press waits at most 300 ms. A key you keep holding (ON for a chord)
does not block the next press.

`window.saturnus` exposes the emulator, `screenText()` (the LCD as `#` and
`.` lines), `loop` (`frame`, `sleep` or `stopped`), the speed setting and
`skinKey(name)` for debugging and automated checks.
