---
type: iteration
title: "Iteration 25: Keyboard shortcuts dialog and rebindable keys"
date: 2026-10-07
status: planned
tags:
  - iteration
  - saturnus
branch: iter-25/keyboard-shortcuts
---

# Iteration 25: Keyboard shortcuts dialog and rebindable keys

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
