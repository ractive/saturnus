---
title: "Iteration 10: Tauri host with a shared front end, keyboard typing"
type: iteration
date: 2026-10-05
status: planned
branch: iter-10/tauri-and-keyboard
tags:
  - iteration
  - saturnus
---

# Iteration 10: Tauri host with a shared front end, keyboard typing

Read first: `web/README.md`, `web/app.js` (the `KEYMAP`, the key queue, the
skin renderer), `crates/saturnus-web/src/{lib.rs,layout.rs,skins/}`,
`crates/saturnus-drive/src/serial.rs` (the wall-clock `Pacer`),
`crates/saturnus-cli/src/serial.rs`, `kb/decision-log.md` (iterations 6 and 8).

## Context (2026-10-05)

- Discussed with the owner: the web page and a Tauri app share the whole
  front end (skins, LCD, controls, state handling) and differ only in the
  backend object the page talks to: the in-page wasm `Emulator` today, or
  Tauri commands and events with the core linked natively into the Tauri
  binary. Spawning the CLI from Tauri is rejected; loading the wasm page
  inside Tauri unchanged is rejected too (no native perks, core on the
  webview's main thread).
- Differences that the design must carry: in Tauri the core runs on its
  own Rust thread with the CLI's wall-clock pacer, and the front end
  receives frames by event when the LCD changed (131x64 is about 1 KB
  packed) rather than pulling every animation frame; native file dialogs
  for the ROM and states; a real serial port later.
- Keyboard typing (owner's wishes, 2026-10-05): letters typed on the
  computer keyboard should type on the calculator (α plus the key carrying
  the letter, lowercase through the model's shift), and the α key wants a
  keyboard shortcut; the shifts should get shortcuts too. Each skin already
  knows every key's letter (`alpha` in the skin data).

## Tasks

- [ ] Front-end backend interface in `web/app.js`: `Backend` with `boot`,
  `keyDown`, `keyUp`, `frame`/`onFrame`, `saveState`, `loadState`,
  `reset`, `status`; `WasmBackend` keeps today's behaviour; the page and
  the skins know nothing else.
- [ ] Keyboard typing: letters map to α plus the key carrying the letter
  on the current model (uppercase direct, lowercase through the shift the
  model uses; the 48 alpha-lock rule: one α for the next key only), a
  shortcut for α (proposal: `Tab`, since the page does not use it;
  `CapsLock` as alpha lock if the browser reports it reliably), shortcuts
  for left and right shift (proposal: `Shift+ArrowLeft`/`Shift+ArrowRight`
  or `[`/`]` as the saturnng TUI does), documented in `web/README.md` and
  the page's Keyboard panel; a test in `saturnus-web` for the letter map
  per model.
- [ ] `crates/saturnus-tauri`: a Tauri 2 app whose binary links the
  `saturnus` crate; the machine runs on a thread with the wall-clock
  pacer; commands `boot(model, rom_path)`, `key_down/up(name)`,
  `save_state/load_state(path)`, `reset`, `status`; a `frame` event with
  the packed framebuffer, annunciators and contrast whenever the LCD
  changed (and at least a few times per second while busy); native file
  dialogs for ROM and state; `web/` as the front end directory with a
  `TauriBackend` selected when `window.__TAURI__` exists.
- [ ] Packaging: `cargo tauri dev` and `cargo tauri build` documented in
  the README; the app icon from `web/logo.svg`; no HP marks.
- [ ] Verification: the web page unchanged in behaviour (headless Chrome
  run as in iteration 8); the Tauri app booting the 48SX from a file
  dialog, keys by mouse and keyboard, a state saved and loaded, on macOS
  (the owner's machine).

## Acceptance criteria

- [ ] `cargo tauri dev` opens the app, boots a 48SX from a chosen ROM,
  runs at 100% speed on its own thread, and a letter typed on the
  computer keyboard appears on the calculator; the same page served as
  the web UI still works.
