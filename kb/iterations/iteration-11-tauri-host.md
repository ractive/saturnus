---
title: "Iteration 11: Tauri host with the shared front end"
type: iteration
date: 2026-10-05
status: in-progress
branch: iter-11/tauri-host
tags:
  - iteration
  - saturnus
---

# Iteration 11: Tauri host with the shared front end

Read first: `web/README.md`, `web/app.js` (the `KEYMAP`, the key queue, the
skin renderer), `crates/saturnus-web/src/{lib.rs,layout.rs,skins/}`,
`crates/saturnus-drive/src/serial.rs` (the wall-clock `Pacer`),
`crates/saturnus-cli/src/serial.rs`, `kb/decision-log.md` (iterations 6 and 8).

## Context (2026-10-05)

- Split on 2026-10-05: the web design pass, keyboard typing, shortcuts and
  the speed control moved to iteration 10 (`iteration-10-web-design`);
  this iteration is the Tauri host only and builds on that page.

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

## Architecture (agreed 2026-10-05)

Ports and adapters with a command/event protocol, Elm-style one-way data
flow in the page:

- The web side pushes too: the wasm core moves into a Web Worker that
  owns the machine, paces it (speed factor, idle stop in SHUTDN) and posts
  a `frame` message only when the LCD changed; Tauri's Rust thread does
  the same with `emit`. One protocol serves both hosts.
- Protocol, defined once in `web/protocol.md` (JSON messages, versioned):
  commands `boot`, `keyDown`, `keyUp`, `setSpeed`, `pause`, `reset`,
  `saveState`, `loadState`, later `eval`, `memoryTree`, `transfer`; events
  `frame` (packed LCD, annunciators, contrast), `status`, `memoryChanged`,
  `error`.
- Two adapters with the same interface: `WorkerBackend`
  (postMessage/onmessage; Comlink may be used for promise plumbing) and
  `TauriBackend` (`invoke`/`listen`). Nothing else knows which host it is.
- View: framework-free Web Components `<sat-calculator>` (skin + LCD),
  `<sat-controls>`, later `<sat-explorer>`, each given the backend and a
  shared store (`EventTarget`) fed by backend events; components render
  from the store and send commands only through the backend.
- hpcomm's patterns feed the later explorer: two-pane tree/list, drag and
  drop both ways, properties, overwrite/rename prompts, progress,
  screen capture with GROB conversion, backup/archive.

## Tasks

- [ ] Protocol document and the Worker: move the wasm core into a Web
  Worker; `WorkerBackend`; the page and components unchanged in behaviour
  (the iteration 10 design, speed control and idle logic carry over to the
  Worker).
- [ ] Web Components: `<sat-calculator>`, `<sat-controls>`, the store;
  `web/index.html` composes them.
- [ ] About page (owner, 2026-10-05: "an about page where all the
  literature and inputs we used are listed"): a `<sat-about>` panel with
  the project statement (clean room, MIT, AI notice, "not affiliated with
  HP; HP, HP48 and HP49 are trademarks of HP Inc."), the saturnus logo, and
  the complete list of sources: every page under `wiki/sources/` of the
  hardware wiki (title, authors, year, URL or archive location, what it
  was used for), the emulators used as black-box oracles (saturnng, with
  its licence and that no code was read), the HP Museum benchmark thread,
  the manuals used as skin references per model, the Intel datasheet,
  hptx and the ROM download policy. Generated at build time by a script
  that reads the wiki's source-page frontmatter into `web/about.json`
  (the wiki stays outside the repo; the JSON is committed and refreshed by
  the script), so the list never drifts from what was actually read. The
  same text in the README's Legal section where it is not already.

- [ ] Speed control in the Tauri host: the pacer thread takes the speed
  factor (1x, 2x, 4x, unlimited); the web control itself was done in
  iteration 10.
- [ ] `crates/saturnus-tauri`: a Tauri 2 app whose binary links the
  `saturnus` crate; the machine runs on a thread with the wall-clock
  pacer; commands `boot(model, rom_path)`, `key_down/up(name)`,
  `save_state/load_state(path)`, `reset`, `status`; a `frame` event with
  the packed framebuffer, annunciators and contrast whenever the LCD
  changed (and at least a few times per second while busy); native file
  dialogs for ROM and state; `web/` as the front end directory with a
  `TauriBackend` selected when `window.__TAURI__` exists.
- [ ] Packaging: `cargo tauri dev` and `cargo tauri build` documented in
  the README; the app icon from `web/logo.svg`; no HP marks. Owner
  (2026-10-05): "Windows and linux builds would be nice": a GitHub Actions
  matrix (macOS, Windows, Linux) with Tauri's official action producing
  the installers as release artifacts; the web page published to GitHub
  Pages from the same workflow.
- [ ] Verification: the web page unchanged in behaviour (headless Chrome
  run as in iteration 8); the Tauri app booting the 48SX from a file
  dialog, keys by mouse and keyboard, a state saved and loaded, on macOS
  (the owner's machine).

## Acceptance criteria

- [ ] `cargo tauri dev` opens the app, boots a 48SX from a chosen ROM,
  runs at 100% speed on its own thread, and a letter typed on the
  computer keyboard appears on the calculator; the same page served as
  the web UI still works.
