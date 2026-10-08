---
type: iteration
title: "Iteration 25: Side panels and keyboard (resizable panels, Commands tab fixes, shortcuts dialog, rebindable keys)"
date: 2026-10-07
status: in-progress
tags:
  - iteration
  - saturnus
branch: iter-25/panels-and-keyboard
---

# Iteration 25: Side panels and keyboard

"App" means the web page and the desktop app alike.

## Context (owner, 2026-10-07)

- The Keyboard section of the side panel is cut off: the two-column
  table does not fit the panel's width (screenshot, 2026-10-07).
- "What about showing the keyboard shortcuts in a popup/popover?"
- "What about being able to change the keyboard shortcuts (where it
  makes sense)? As we have seen, some keyboard layouts make it impossible
  to use certain combinations." (Swiss German: the backtick for ON is a
  dead key behind Shift; Shift+Esc opens Firefox's process manager; Esc
  is taken in fullscreen.)

## Side panels (owner, 2026-10-07, with screenshots)

- **Resizable**: the left panel and the right slide-in layer get a drag
  handle on their inner edge; widths remembered per browser and in the
  app's settings; minimums so no content is cut off.
- **Consistent closing**: both panels close with a chevron pointing to
  the edge they slide into (today the left has "<", the right "x").
- **Focus hint** "Keys go to the calculator. Alt+M moves them here." is
  cut off in the tab bar: a short indicator with the full text as a
  tooltip; the shortcut moves into the keyboard dialog.
- **Object preview** in Variables and Stack looks like an editable
  input but is read-only: style it as a calculator-text display panel;
  "Copy text" stays.
- **Commands tab**: (a) `MENU 21`, `23`, `59` are built-in menus no key
  opens (on the 48SX the second pages of MODES, MEMORY, UNITS): name them
  from the manuals where they say, else group under "Other menus" with a
  one-line explanation; (b) STAT appears twice because the tree mixes the
  ROM's menus and the manuals' categories: one tree of ROM menus, the
  manuals' placements only for commands without a ROM menu, under a
  separate heading; (c) search is a plain substring match ("INT" finds
  102): reuse the palette's ranker (`web/reference.js`), results ordered
  by relevance, name matches before description matches.

## Design

- **Shortcuts dialog** replacing the panel section: opened by a
  "Keyboard shortcuts" link in the panel, by a key of its own (a
  default chosen like the others below, rebindable), and from the Cmd+K
  palette. A table with room for the
  explanations, grouped: typing (letters, digits, operators as
  characters), calculator keys (ON, alpha, left and right shift), app
  (palette, row-number modifier, fullscreen, layers, speed, darker and
  lighter display from the contrast backlog item).
- **Rebindable**: the calculator keys and the app actions above.
  Typed characters stay as they are (a letter types itself). A binding is
  recorded by pressing the wanted key (combination) in the dialog; it is
  stored as the physical key (`KeyboardEvent.code` plus modifiers), so it
  works on any layout, and shown with the label the layout gives it.
  Warnings for a conflict with another binding and for known browser and
  OS shortcuts (Cmd/Ctrl+digit for tabs, Shift+Esc in Firefox, Esc in
  fullscreen, Cmd+Q/W). "Reset to defaults".
- **Defaults** chosen so every one is reachable without a dead key on the
  common layouts (US, UK, German, Swiss German, French): check each
  against those layouts and pick physical keys accordingly.
- **Storage**: per browser in `localStorage` (a per-viewer preference);
  in the desktop app the settings file of iteration 20.
- The palette's hints and the dialog always show the current bindings.

## Tasks

- [x] Side panels: resize handles and persistence, consistent closing,
  the focus hint, the read-only preview style.
- [x] Commands tab: unnamed menus, one tree, ranked search; Node tests
  for the tree building and the ranking.

- [x] The dialog, opened three ways; the panel section replaced by a link.
- [x] A bindings model (defaults, physical-key storage, conflict and
  reserved-shortcut checks) with Node tests; the page's key handling
  reads from it.
- [x] Recording UI, reset, persistence in both hosts.
- [x] Defaults checked against five layouts (documented in the Outcome).
- [x] Headless Chrome: rebind ON to another key, reload, it works; a
  conflict is warned; reset restores; the panel no longer overflows.

## Acceptance criteria

- [x] On a Swiss German layout every calculator key and app action is
  reachable by the defaults or after rebinding.
- [x] `just gates` passes.

## Outcome

- **Side panels**: handles on the inner edges of the controls panel and
  the memory view (drag, arrow keys on the focused handle, double-click
  for the default), widths in `localStorage` (`saturnus.panelWidth`,
  `saturnus.layerWidth`), minimums 236 px (panel) and 380 px (layer, which
  leaves the calculator 320 px). Both close with a chevron towards their
  edge (‹ and ›). The tab bar's focus line is a short indicator ("Keys:
  calculator" / "Keys: here") with the full sentence, and the current
  key, as its tooltip. The read-only previews (Variables, Stack: text,
  lists, directories, grids) sit on a tinted, sunk display panel without
  an input border; "Copy text" stays.
- **Commands tab**: `menuTree` builds one tree. A `MENU n` is named from
  the manuals' categories where at least half of its commands agree
  (this model's manual, else another's): on the 48SX `MODES (MENU 21)`
  under MODES, `MEMORY (MENU 23)` under MEMORY, `UNITS (MENU 59)` among
  the roots; on the 48GX 12 of 14 named, `MENU 87` and `MENU 104` under
  "Other menus"; on the 49G 9 of 21 named. Each numbered menu and the
  heading has a one-line note. The manuals' placements moved under the
  folded heading "Not in a ROM menu", so STAT is once among the roots.
  The search is `findCommands`, the palette's `search` without send rows:
  INT, ∫, INTVX on the 49G; ∫ first on the 48SX (no INT there). Node
  tests in `web/test/reference.test.mjs`.
- **Keyboard**: `web/bindings.js` (defaults, physical-key storage of the
  changes only, matching, layout labels, conflict / reserved / fixed /
  typing / dead-key warnings) with `web/test/bindings.test.mjs`; the
  calculator's ON, α and shift handling, the app's shortcuts (app.js), the
  palette's key, its row-number modifier and every shown hint read from
  it. `<sat-shortcuts>` replaces the panel's Keyboard section; opened by
  the panel's link, Alt+K and the palette's "Keyboard shortcuts" action.
  Bindings are recorded by pressing the key after **Add key**; reset per
  action and for all. The speed and darker/lighter display got bindings
  too (Alt+S, Alt+↑/↓).
- **Defaults against five layouts** (unshifted and shifted dead keys from
  the layouts' charts; Alt+letter is a combination the page receives by
  code): US and UK have none; German ^ (Backquote) and the acute and grave accents
  (Equal); Swiss German ^ and the grave accent (Equal) and ¨
  (BracketRight); French ^ and ¨ (BracketLeft). No default without a
  modifier may type a character the calculator maps either (review of PR
  40: German types + on BracketRight, Dvorak / and = on the brackets), so
  the old `` ` ``, `[` and `]` are gone from the defaults: ON Esc / Alt+O,
  α Tab, left shift Alt+L, right shift Alt+R, palette Mod+K, the rest
  Alt+letter, Alt+Enter, Alt+arrows. So on Swiss German every calculator
  key and app action is reachable by a default (the owner's backtick
  problem: ON is Esc or Alt+O). The tests check both tables (dead keys;
  characters on six layouts including Dvorak) for Mac and PC; headless
  Chrome confirms the German `+` key (BracketRight, key `+`) presses +
  again. Not checked: macOS Option
  layers produce characters for Alt+letter, but the page matches the code
  and prevents the default, so no character is typed.
- **Persistence in the desktop app**: `localStorage` there too (the
  webview keeps it across starts). The settings file of iteration 20 holds
  ROM settings only and the page has no command to write other settings;
  a generic "page preferences" command in `saturnus-tauri` would be needed
  to keep them in the file.
- **Headless Chrome** (`scratchpad/iter25/check.mjs`, port 4881): panel
  252 → 322 px and layer 605 → 725 px by dragging, the same after a
  reload; the 48SX Commands tab shows STAT once, no bare `MENU n`, `MENU
  21` with its note; "INT" on the 49G lists INT, ∫, INTVX first; Alt+K,
  the panel's link and the palette open the dialog; ON rebound to F9
  (Esc removed), after a reload F9 holds ON and Esc does nothing; Alt+M
  for ON shows the conflict on both rows; Reset restores the defaults;
  Esc cancels a recording and removing a key keeps the focus in its row; the panel at its 236 px minimum with the ROM table open has no
  element past its edge (scrollWidth = clientWidth). No console errors.
