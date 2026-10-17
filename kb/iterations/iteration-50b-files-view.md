---
type: iteration
title: "Iteration 50b: Files as a mountable view"
date: 2026-10-17
status: planned
tags:
  - iteration
  - web
  - saturnus-tx
branch: iter-50b/files-view
---

# Iteration 50b: Files as a mountable view

tx's start code becomes a module the one document can mount. Still
served as `tx.html` so the tx page tests keep running.

## Design

- `web/tx/app.js` resolves its `ui` by `document.getElementById` at
  import (`:41-54`) and starts, adds listeners and sets `window.tx` at
  import (`:338`, `:381`, `:793`). It becomes
  `mountFiles(root, {shown})`: elements looked up under `root`, nothing
  run at import. `tx.html` becomes a thin page that calls it.
- **Views are hidden, not unmounted**: once mounted, Files lives for the
  document. `shown()` (a function the shell passes; `tx.html` passes
  `() => true`) is checked first in every document- or window-level
  handler: `dragover`/`drop`, `beforeunload` (only while a transfer
  runs).
- tx's theme row goes; the theme is set in one place (the calculator's
  settings and, later, the shell).
- Styles under `#files-view` (`web/tx/style.css`).

## Tasks

- [ ] `mountFiles(root, {shown})`; `tx.html` a thin host
- [ ] Global handlers gated on `shown()`; theme row removed
- [ ] Styles scoped
- [ ] tx tests (`tx-page`, `tx-chrome`, `tx-hidden`, `tx-shared`) green against `tx.html`

## Acceptance

- `tx.html` works as before; nothing in `web/tx/` runs at import.
