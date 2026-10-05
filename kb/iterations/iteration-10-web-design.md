---
title: "Iteration 10: Web UI design pass, keyboard typing, speed control"
type: iteration
date: 2026-10-05
status: planned
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

- [ ] Geometry: softkey labels aligned with the menu keys on every model
  (measure the ROM's label positions on the LCD and the key centres; adjust
  bezel and LCD placement per skin; a test asserts the alignment within a
  tolerance).
- [ ] Key relief: subtle 3D on key caps (shared SVG defs; highlight,
  shadow, pressed state), the same treatment on all skins; case depth hint.
- [ ] Case proportions: no excess case below the bottom row on any skin.
- [ ] Idle page: remove the per-frame counters; stop the animation loop
  while the calculator is idle and restart on input/timer; measure CPU in
  the Chrome task manager or `performance` and record it (target: 0% when
  idle).
- [ ] Layout: full-height calculator; side panel for controls on wide
  screens, compact bar on narrow; fullscreen button; the Keyboard help
  panel updated.
- [ ] Keyboard typing: letters, α shortcut, shift shortcuts; a test for the
  letter map per model.
- [ ] Speed control 1x/2x/4x/unlimited, remembered; works with the idle
  logic.
- [ ] Screenshots before/after per model in the plan Outcome; decisions in
  `kb/decision-log.md`; `web/README.md` updated; `hyalo lint` clean.

## Acceptance criteria

- [ ] On every skin the six softkey labels sit above the six menu keys; keys
  show relief and a pressed state; no empty case below the keys.
- [ ] An idle calculator costs no CPU in the browser (no animation frames,
  no DOM updates) and wakes on a key or timer.
- [ ] The calculator fills the window height; controls in a side panel on
  desktop; fullscreen works; letters, α and shifts work from the keyboard;
  the speed control makes a plot visibly fast at unlimited.
