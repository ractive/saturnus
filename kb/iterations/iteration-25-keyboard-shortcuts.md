---
type: iteration
title: "Iteration 25: Side panels and keyboard (resizable panels, Commands tab fixes, shortcuts dialog, rebindable keys)"
date: 2026-10-07
status: planned
tags:
  - iteration
  - saturnus
branch: iter-25/keyboard-shortcuts
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

- [ ] Side panels: resize handles and persistence, consistent closing,
  the focus hint, the read-only preview style.
- [ ] Commands tab: unnamed menus, one tree, ranked search; Node tests
  for the tree building and the ranking.

- [ ] The dialog, opened three ways; the panel section replaced by a link.
- [ ] A bindings model (defaults, physical-key storage, conflict and
  reserved-shortcut checks) with Node tests; the page's key handling
  reads from it.
- [ ] Recording UI, reset, persistence in both hosts.
- [ ] Defaults checked against five layouts (documented in the Outcome).
- [ ] Headless Chrome: rebind ON to another key, reload, it works; a
  conflict is warned; reset restores; the panel no longer overflows.

## Acceptance criteria

- [ ] On a Swiss German layout every calculator key and app action is
  reachable by the defaults or after rebinding.
- [ ] `just gates` passes.

## Outcome

(to be written)
