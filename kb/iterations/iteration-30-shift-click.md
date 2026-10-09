---
type: iteration
title: "Iteration 30: Shift-click and the shift glow"
date: 2026-10-09
status: in-progress
tags:
  - iteration
  - saturnus
  - web
branch: iter-30/shift-click
---

# Iteration 30: Shift-click and the shift glow

Owner (2026-10-09): with a mouse, a shifted function takes two clicks
(the shift, then the key). Ctrl+click on a drawn key should press its
left-shifted function, Option+click (Alt+click on Windows and Linux) its
right-shifted one: on the 49G Ctrl+click NXT is PREV, Option+click NXT
is MENU. While Ctrl or Option is held, the labels it reaches should
light up on the skin.

Read first: `web/components/sat-calculator.js` (the pointer handler in
`drawKey`, `pressKey`/`releaseKey`, `onKeyDown`), `web/bindings.js`,
`web/components/sat-shortcuts.js`, `crates/saturnus-host/src/skins/mod.rs`
(labels: `left`, `right`, one shift uses `left`), `web/protocol.md`
(`keyDown`/`keyUp`: queued, held at least 60 emulated ms).

## Design

- **Which shift.** Ctrl is the left shift, Option/Alt the right shift.
  A model with one shift (38G, 39G, 40G, 42S: a key named `shift`) uses
  it for both. Exactly one of the two modifiers counts: Ctrl+Alt (AltGr
  on Windows), Shift or Cmd/Win with it is a plain click. Fixed, not a
  binding: the dialog lists them as fixed, like typing.
- **The presses.** The page sends one `keyDown` with `shift` (protocol.md)
  and `keyUp` on release. The host's key queue (`KeyQueue::press_shifted`)
  decides when the press's turn comes, after every key queued before it
  has played and the ROM has settled: it taps the shift unless its
  annunciator is on (a second press would cancel it), then presses the
  key, held as a click holds it. On a one-shift model either shift
  annunciator counts. A click on a shift key itself is a plain press.
- **Not from the frame.** The first version decided in the page from the
  last frame's annunciator; PR 64's review found it wrong for quick
  clicks (the frame lags the queue): two quick Ctrl+clicks, and ↰ then a
  quick Ctrl+click. Both are host unit tests and a ROM-gated page test
  (3, two quick Ctrl+clicks on √x: 81; ↰ then Ctrl+click: 6561).
- **Mac Ctrl+click** is a secondary click: `pointerdown` comes with
  `ctrlKey`, button 0 or 2, and a `contextmenu` the skin already
  prevents. The handler ignores the button (as it did), so it is one
  left-shifted press and nothing else.
- **Mouse only.** Touch and pen have no modifiers; nothing changes on
  phones. The key's tooltip names both combinations and their labels.
- **The glow.** While Ctrl or Option/Alt alone is held and the
  calculator owns the keys (no text field, select or editable element
  focused, no open dialog or the memory view on the event's path, no
  modal dialog open), the matching shift labels light up and the other
  labels dim. A pure state machine (`web/shiftclick.js`): lit 150 ms
  after the modifier goes down, so chords (Ctrl+K, Alt+L) never flash;
  any other key cancels it; cleared on the modifier's keyup, window
  blur, the page hidden, and any key or pointer event whose flags show
  the modifier is up. One-shift models light the one shift's labels for
  either modifier; keys without a label for that shift light nothing.
- **Lone Alt.** On Windows and Linux a lone Alt press and release
  toggles the menu bar (Firefox, the desktop app's WebView); its keyup
  is prevented while the calculator owns the keys.
- **Look.** Classes on the skin's SVG (`glow-left`, `glow-right`); the
  shift labels carry `shift-left`/`shift-right`. The lit labels get an
  SVG filter per shift colour: a light ink turns nearly white in a halo
  of its own colour, a dark ink (the 49G's) keeps its colour on a pale
  plate; the rest fade to 40 %, a short transition, none under
  `prefers-reduced-motion`.
- **Tests.** `web/test/shiftclick.test.mjs` (pure parts),
  `web/test/shiftclick-page.test.mjs` (headless Chrome, real mouse and
  key events, a stubbed backend; with `SATURNUS_ROM_DIR`, a booted 48SX
  takes Ctrl+click √x as x² and Alt+click as ˣ√y).

## Tasks

- [x] Plan and decision-log entry
- [x] `web/shiftclick.js`: modifier, shift per model, presses to send, glow state machine
- [x] Ctrl/Option+click in the skin's pointer handler; tooltips
- [x] The glow: classes, filter, CSS, document listeners, lone-Alt keyup
- [x] Keyboard shortcuts dialog: a fixed Mouse section
- [x] Unit tests (`web/test/shiftclick.test.mjs`)
- [x] Headless-Chrome tests (`web/test/shiftclick-page.test.mjs`)
- [x] Screenshots of the glow (48GX, 49G, 39G; light and dark), tuned
- [x] Gates
- [ ] Owner: Ctrl/Option+click and the glow in the desktop app and a desktop browser
