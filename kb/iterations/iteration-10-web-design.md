---
title: "Iteration 10: Web UI design pass, keyboard typing, speed control"
type: iteration
date: 2026-10-05
status: completed
branch: iter-10/web-design
tags:
  - iteration
  - saturnus
---

# Iteration 10: Web UI design pass, keyboard typing, speed control

Read first: `web/README.md`, `web/{index.html,app.js,style.css}`,
`crates/saturnus-web/src/{lib.rs,layout.rs,skins/}` (skin data: case, bezel,
LCD rectangle, keys with labels and colours; one file per model),
`kb/decision-log.md` (iterations 6 and 8), `kb/docs/clean-room-rule.md` (no
HP logo or wordmark; the saturnus logo is ours).

## Context (2026-10-05)

Owner's review of the SVG skins ("already looks really nice") with these
points, verbatim where it matters:

- "The menu keys are not really aligned with the tabs on the screen. Maybe
  you can reduce the bezel?" The six softkey labels the ROM draws at the
  bottom of the LCD should sit directly above the six menu keys; today the
  bezel and key spacing put them off. Fix by geometry: LCD position, bezel
  width and menu-key pitch must line up, per model.
- "The keys look really flat. Add some slight 3D effect by adding some
  light reflections on them." Subtle gradients, a highlight edge and a
  soft shadow; pressed state darker and lower; keep it restrained (a real
  calculator, not a glossy toy) and keep SVG size small (shared `defs`).
- "The bottom has a lot of free space": on the 48SX skin the case
  continues far below the ON row. Real 48 cases end shortly below the
  bottom key row; shorten the case so the bottom margin matches the top
  and sides (the screenshot showed about a key-row height of empty case
  and a page gap).
- "Remove the emulated counter. It just burns CPU and doesn't let the
  page go into a sleep mode." The status line's running counters are
  updated every frame; remove them. More generally: when the calculator
  is idle (in SHUTDN with no pending key), the page must stop its
  animation loop and wake on input or on the next timer event, so an idle
  tab costs nothing; no per-frame DOM text updates.
- "The calculator should use the whole height of the screen. Especially
  in desktop mode, the menu that is now on top can be on the side. There
  should also be a fullscreen option to make the calculator fullscreen
  (in the browser window)." Layout: the skin scales to the viewport
  height (and width on phones); the controls (model, ROM, pause, reset,
  save/load, speed, drawn/grid toggle) move to a collapsible side panel on
  wide screens and a bottom sheet or top bar on narrow ones; a fullscreen
  button uses the Fullscreen API with the calculator alone, Escape leaves
  it (Escape is also ON: when in fullscreen, Escape leaves fullscreen
  first, say so in the Keyboard help, or pick another key for ON in
  fullscreen).
- Earlier wishes that belong to this pass: letters typed on the computer
  keyboard type on the calculator (α plus the key carrying the letter on
  the current model, lowercase through the model's shift; the 48's α rule:
  one α for the next key only, two for lock); a shortcut for α (proposal
  Tab; CapsLock as alpha lock if reported reliably); shortcuts for the two
  shifts (proposal `[` and `]` as the saturnng TUI uses, or Shift+arrows);
  a speed control 1x, 2x, 4x, unlimited (the core runs 35-55x real time
  when busy; unlimited = as many emulated ms per frame as fit, with a cap
  and yielding; the ROM's clock runs fast then, say so), remembered in
  localStorage.
- Design process: load the `frontend-design` skill before touching the
  page; take before/after screenshots per model in headless Chrome (the
  iteration 8 CDP script `scratchpad/cdp.mjs` may still exist, else write
  one); verify with real mouse and key events; keep the saturnus logo on
  the bezel; no HP marks.

## Tasks

- [x] Geometry: softkey labels aligned with the menu keys on every model
  (measure the ROM's label positions on the LCD and the key centres; adjust
  bezel and LCD placement per skin; a test asserts the alignment within a
  tolerance).
- [x] Key relief: subtle 3D on key caps (shared SVG defs; highlight,
  shadow, pressed state), the same treatment on all skins; case depth hint.
- [x] Case proportions: no excess case below the bottom row on any skin.
- [x] Idle page: remove the per-frame counters; stop the animation loop
  while the calculator is idle and restart on input/timer; measure CPU in
  the Chrome task manager or `performance` and record it (target: 0% when
  idle).
- [x] Layout: full-height calculator; side panel for controls on wide
  screens, compact bar on narrow; fullscreen button; the Keyboard help
  panel updated.
- [x] Keyboard typing: letters, α shortcut, shift shortcuts; a test for the
  letter map per model.
- [x] Speed control 1x/2x/4x/unlimited, remembered; works with the idle
  logic.
- [x] Screenshots before/after per model in the plan Outcome; decisions in
  `kb/decision-log.md`; `web/README.md` updated; `hyalo lint` clean.

## Acceptance criteria

- [x] On every skin the six softkey labels sit above the six menu keys; keys
  show relief and a pressed state; no empty case below the keys.
- [x] An idle calculator costs no CPU in the browser (no animation frames,
  no DOM updates) and wakes on a key or timer.
- [x] The calculator fills the window height; controls in a side panel on
  desktop; fullscreen works; letters, α and shifts work from the keyboard;
  the speed control makes a plot visibly fast at unlimited.

## Outcome

All screenshots are in the session's scratch directory
`/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/design/`
(not committed). Taken in headless Chrome 154 over the DevTools protocol
with real mouse and key events (`cdp-lib.mjs`, `before.mjs`, `after.mjs`
there); every model booted from its ROM, answered the memory prompt by a
drawn key, computed 6 × 7 by mouse, typed "Hello World" from the keyboard,
and exercised Tab, `[`, `]` and `` ` ``.

Before (the page of iteration 8, 1280 × 900 and the whole page at half
scale; phone at 390 × 844):
`before-{48sx,48gx,38g,49g,39g,40g}-desktop.png`,
`before-{...}-full.png`, `before-phone.png`.

After: `after-{48sx,48gx,38g,49g,39g,40g}-desktop.png` (1280 × 900),
`after-{...}-pressed.png` (crop at 2× while ENTER is held),
`after-{...}-typed.png` (the display after typing "Hello World"),
`after-speed-program.png` and `after-speed-panel.png`,
`after-fullscreen.png`, `after-panel-hidden.png`, `after-tall.png`
(1000 × 1500), `after-phone.png` and `after-phone-sheet.png` (390 × 844 at
2×, the bar's sheet open). Reports: `after-report-all.json` (all six
models), `after-report.json` (the 40G rerun and the 48SX idle and speed
measurements).

Per review point:

- **Menu keys under the labels.** The skin's `lcd` rectangle is now the
  display's active area (595 × 327 units, 131 × 72 pixels at square
  pixels) placed from the ROMs' label pitch (22 px, wiki hardware/display
  "Menu labels"); the bezel follows it. Unit test
  `softkey_labels_sit_above_the_menu_keys` (≤ 3 units); measured on screen,
  every label centre is within 0.6 CSS px of its key centre on all six
  models (before: up to 50 units off on the outer keys).
- **Key relief.** Three gradients in shared `defs` (cap light, lit/shaded
  rim, case light), one overlay path per key, the shadow under each cap;
  pressed keys move down 3 units, lose most of their shadow and darken.
  `after-*-pressed.png`.
- **Case bottom.** 48: 1413 → 1265 units (52 under the ON row, about the
  side margin); 38G 1505 → 1485, 49G 1482 → 1455, 39G/40G 1472 → 1435; a
  test keeps the bottom margin within 0.8–1.3 of the side margin.
- **Idle CPU.** Counters removed; the status line changes only on events.
  With the ROM in SHUTDN and nothing queued the page cancels its animation
  loop and sets one timer for the ROM's next timer event
  (`Emulator.idle_ms`, new `Machine::idle_cycles`). Measured on the 48SX
  over 5 s idle: 0 animation frames requested, 2.2 ms of main-thread task
  time (0.04% of a core; before: 300 frames, a status rewrite per frame).
  A key press returns the loop to frames and the ROM's clock catches up
  first; it sleeps again once idle. The 48's wake-for-nothing every 0.5 s
  (TIMER1 MSB) sleeps on without a frame.
- **Layout and fullscreen.** The calculator fills the stage's height
  (1500 px window: 1358 px tall skin after the snap to whole device
  pixels; 900 px window: 836 px), controls in a 252 px side panel that
  hides with `‹`; below 760 px a 48 px bar with a drop-down sheet, no
  horizontal scroll at 390 px. Fullscreen on the stage alone, verified
  entering by the panel button and leaving by the `✕`; Escape stays ON in
  Chromium through keyboard lock, `` ` `` is ON everywhere.
- **Keyboard letters and shortcuts.** Tab = α (annunciator on after one
  press, lock after two on the 48/49G, cancel on the aplet models), `[` and
  `]` = the shifts, Esc and `` ` `` = ON. "Hello World" typed correctly on
  the 48SX, 48GX, 49G, 39G and 40G; "HelloWorld" on the 38G (no space).
  Two queue changes were needed: queued presses wait for the ROM to idle
  (the 48SX ROM drops a key sent 30 ms after the previous one while still
  handling it), and alpha is tracked as spent after a letter the page
  pressed α for, because the ROM blinks the annunciator for ~250 ms while
  redrawing.
- **Speed.** 1×/2×/4×/Max, remembered in `localStorage`; the panel says the
  clock runs fast. A `1 300000 START NEXT` loop on the 48SX measured at
  1.0×, 4.0× and 50× emulated time per wall time (Max is bound by the
  one-second-per-frame cap), the page staying responsive (frames yield
  after 11 ms).

Open: CapsLock as alpha lock (macOS reports no reliable key-up); the
Fullscreen keyboard lock only exists in Chromium; the full-height fit
gives up to 8% of the height to keep LCD pixels whole; `type_text` in
`saturnus-drive` and the web page now follow the same "wait for SHUTDN"
rule but share no code.

### Fidelity pass from the owner's photos

The owner's second review ("the keys now look 3D; give the menu keys a
small black border like on the real calculator") came with his own
straight-on photographs of his 48SX, 38G and 49G (ten JPEGs, 1500 x 2000,
in `/Users/james/Downloads/HP Taschenrechner/`; not in the repository).
They are now the reference for those skins: positions measured in pixels
against the menu-key pitch (125 px = 100 units), colours sampled and toned
down from the sunlit exposure. Everything is still drawn by the page;
`Cap::well`, `Skin::well_fill`, `Skin::round`, `Panel::bow`,
`Mark::italic` and `AlphaStyle::Corner` are the new skin data.

- **48SX.** Every cap sits in a dark well (72 x 54 units; the cream menu
  caps are 54 x 29 in 72 x 47 wells, so the black border is widest
  there), A-F and the other letters to the right of the wells. The case is
  685 x 1520 (was 667 x 1265: the manual figure's rows were 89 units
  apart, the unit's are 100), caps 64 x 46 (1.39:1). The lighter plate
  runs from the top to under the menu keys with its step at 555; label
  tabs behind PRINT, I/O, MODES, MEMORY, LIBRARY and behind the labels of
  4-9; orange and light blue labels; a light blue right-shift cap (was a
  saturated blue); a blue-black rim; "48SX" and "SCIENTIFIC EXPANDABLE"
  in italics above the display; the LAST bracket over 2 and 3.
- **48GX.** The 48SX's geometry and wells (one mould) with its own
  colours and labels from the earlier references; the lighter plate ends
  at the step; tiles behind the number keys and the right shift moved
  with the keys. No photograph of a 48GX: its shades stay inferred.
- **38G.** Case 714 x 1500 (was 676 x 1485), navy top, charcoal keyboard
  panel, the teal zone from above the HOME row to the bottom (six rows:
  the brief said five, the photographs show MODES ... on teal), the teal
  stripe joining the six dark menu keys, wells around every key, cream
  cursor caps and a light turquoise shift cap, muted teal shift labels
  at size 20, salmon alpha letters off the lower right corners, CANCEL
  in khaki. Grid, labels and all 26 alpha letters on the photographs
  match the skin data and the ROM-verified letter map: no disagreement.
- **49G.** The manual figure's key grid matched the photographs; the keys
  moved up 35 units (cursor pad 49) and the display surround starts 34
  units from the top with a bowed lower edge, six tick marks and "49G"
  in italics. Colours: silver-blue face, stronger blue rim, slate menu,
  operator and cursor keys, charcoal number keys, taupe function keys
  with white labels (were beige with dark labels), black outlines, navy
  and dark red shift labels, white CANCEL.
- **38G space.** SPACE is printed above 2; on ROM A1.67 SHIFT then 2
  types a space, also between letters. The page's space key and
  `type_text` now use it ("Hello World" typed with key events on the
  38G); the tests and READMEs that said the 38G cannot type a space are
  corrected; wiki hardware/hp38g has the fact.

Verified as before in headless Chrome over the DevTools protocol with
real mouse events (`after2.mjs` in the scratch directory): 48SX, 48GX,
38G and 49G boot from their ROMs, the memory prompt is answered on the
drawn menu key, 6 x 7 by mouse shows 42, menu labels within 0.94 CSS px of
their keys at a 2000 px tall window. `after2-<model>-desktop.png`
(1280 x 900), `after2-<model>-skin.png` and
`after2-<model>-side-by-side.png` (skin next to the owner's photograph;
the 48GX next to the 48SX) are in the scratch directory.

Still different from the units, on purpose: the display window is the
ROM's 131 x 72 pixels at 595 units wide so the menu labels sit over the
menu keys (the real glass is 509 to 543 units wide), which thins the
frames beside it and shortens the 49G's ticks; the bottom margins follow
the "case ends shortly below the keys" rule (48: 74 units for 82; 49G: 61
for 83); the 38G's tilted top and the 49G's grips are drawn flat and
straight; the 49G's cursor keys are trapezoids and the glyphs around them
are left off; no HP logo, wordmark or series emblem. The 39G/40G skin was
not part of this pass (no photograph). `SATURNUS_ROM_DIR` runs fail in
`summation_benchmark_matches_real_hardware` for a missing
`rom-2.10.49g`, before and after this pass.
