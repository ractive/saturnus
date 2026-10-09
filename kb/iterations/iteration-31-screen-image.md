---
type: iteration
title: "Iteration 31: The screen as an image"
date: 2026-10-09
status: in-progress
tags:
  - iteration
  - saturnus
  - web
branch: iter-31/screen-image
---

# Iteration 31: The screen as an image

Owner (2026-10-09): copy the calculator's current screen to the
clipboard as a PNG, or save it as a PNG file. The image is the whole LCD
as the calculator shows it, annunciators included; in the LCD's colours
by default, or black on white (a preference beside the display's), at 4×
with hard pixel edges. Two buttons in the panel, palette commands, a
shortcut, and a menu on the display (right-click, long press on a
phone). All seven models.

Read first: `web/components/sat-calculator.js` (`draw`, the annunciator
strip), `web/components/menu.js` (iteration 32), `web/bindings.js`,
`web/components/sat-controls.js`, `web/backend.js`,
`crates/saturnus-tauri/src/lib.rs` (the file dialogs).

## Design

- **The image** (`web/screenshot.js`, pure): the frame's pixels under the
  8-row annunciator strip, as the page draws the LCD: 131 × 72 (the 42S
  131 × 24, its own seven annunciators), scaled 4× nearest-neighbour to
  524 × 288 (524 × 96). The marks are drawn as pixels (the page draws
  font glyphs, which would blur and differ per system), each centred in
  its slot as on screen. Colours: the LCD's ink and background mixed at
  the frame's contrast exactly as `draw` does, or pure black on pure
  white. The PNG comes from a canvas (`toBlob`).
- **Copy**: `navigator.clipboard.write` with a `ClipboardItem` holding
  the PNG as a promise, made before any await, inside the click or key
  (Safari needs both). Where a browser has no `ClipboardItem`, does not
  support `image/png` or refuses the write, the image is saved instead
  and the status line says why.
- **Save**: the browser downloads it (`WorkerBackend.saveFile`); the
  desktop app answers a new `saveFile` command itself with a save
  dialog offering the name, and writes the bytes (the page names no
  path; at most 4 MiB). Name: `48gx-2026-10-09-0142.png`.
- **Where**: the panel's "Screen images" row under Display: the look as
  a radio pair (LCD colours / Black on white, kept as `screenLook`), and
  Copy screen / Save screen (one column in the desktop panel, two in the
  phone sheet). Palette: Copy screen and Save screen in the chosen look,
  and each in the other look. Keys (bindings, rebindable): Copy screen
  Alt+Shift+C, Save screen Alt+Shift+S (beside Alt+S for the speed and
  Alt+Shift+M). Not Mod+Shift+C, first asked for: the browsers' inspect
  element, which a page generally cannot take (the dialog warns about
  it); not Mod+S, the browser's save, nor Ctrl+Shift+S, Firefox's
  screenshot.
- **The display's menu** (`menu.js`): a right-click on the display, or a
  finger resting 500 ms on it (iOS sends no `contextmenu`; where a
  browser does, the open menu is not opened twice): Copy image, Copy
  image (black on white), Save image…, Save image (black on white)….
  Only while a ROM runs. A right-click elsewhere on the skin stays a
  held key, as before. No double-click action.

## Tasks

- [x] Plan and decision-log entry
- [x] `web/screenshot.js`: the image, colours, file name, clipboard write
- [x] Panel controls, palette commands, bindings, the display's menu
- [x] `saveFile`: browser download, Tauri save dialog
- [x] Icons `copy-screen`, `save-screen` in the sprite
- [x] Unit tests (`web/test/screenshot.test.mjs`)
- [x] Headless-Chrome tests (`web/test/screenshot-page.test.mjs`)
- [x] Screenshots: the menu, the panel, the phone sheet, exported images
- [x] Gates
- [ ] Owner: copy and save the screen in the desktop app and a desktop browser, and on a phone
