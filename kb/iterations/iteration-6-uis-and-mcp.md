---
title: "Iteration 6: UIs and MCP"
type: iteration
date: 2026-10-04
status: planned
branch: iter-6/uis-and-mcp
tags:
  - iteration
  - saturnus
---

# Iteration 6: UIs and MCP

Read first: `kb/docs/architecture.md` (public API sketch, crate layout),
`kb/docs/clean-room-rule.md` (no HP logos or wordmarks in UI chrome, no
third-party KML artwork), `kb/decision-log.md` (iterations 4 and 5: serial
API, key layouts, models), `README.md` (CLI, key scripts, models).

## Context from iterations 1-5 (2026-10-05)

- The core (`saturnus` crate) is I/O-free and builds for wasm32: `Machine::new(model, rom)`,
  `run_cycles`, `key_down/key_up(Key) -> Result`, `has_key`, `framebuffer()`
  (131x64 pixels, annunciators, contrast), `save_state/load_state`,
  `serial_push/serial_drain/serial_pending/serial_baud`, `Model::{Hp48sx,
  Hp48gx, Hp49g, Hp38g}` with `keyboard_layout()` and `Key::{ALL, name,
  from_name, position(layout)}`. Emulated time is `cycles()` at the model's
  clock; the CLI's `serial.rs` has the wall-clock pacer and the per-model
  autostart (NO at the memory prompt, then SERVER).
- Kermit: hptx (`~/devel/hptx`, main) has `hptx-core` with `Calculator`
  (`ls`, `get`, `put`, `run`, `screenshot`, `backup`, stack reads via
  server commands) over a `Transport` trait (`write_packet`, `read` with
  timeout), and a `saturnus` feature with an in-process `SaturnusTransport`
  pinned to the iteration 4 saturnus commit; `kermit-proto` is the sans-IO
  protocol crate. A saturnus-side crate can depend on `hptx-core` by git
  (default features, so no second saturnus copy) and implement `Transport`
  over its own `Machine`.
- Photos: the pixel-faithful skins need the owner's own photographs of the
  real calculators, which are not available yet. This iteration ships the
  MCP server and a web UI with a generic, drawn keyboard per model; the
  photographic skins and Tauri are deferred until photos exist.
- Tooling on this machine: node 24 and npm; no wasm-pack/wasm-bindgen CLI
  yet (install with `cargo install` as needed); the `wasm32-unknown-unknown`
  target is installed.

## Tasks

- [ ] `saturnus-mcp`: an MCP server (stdio) owning one `Machine` per
  session: tools `press_keys` (key script syntax), `type_text`, `screen`
  (PNG and text), `read_stack`, `send_object`/`receive_object` and
  `run_command` over Kermit through `hptx-core`, `save_state`/`load_state`,
  `reset`; paced in emulated time with an idle-wait like the CLI.
- [ ] Web UI: `saturnus-web` WASM bindings of the core (wasm-bindgen) and a
  static page: LCD on a canvas with annunciators, a drawn keyboard per
  model from the key layout (no HP marks), ROM file picker, run in real
  time, save/load state in the browser; served by any static file server.
- [ ] Deferred until photos exist: pixel-faithful skins from the owner's
  photographs; Tauri desktop wrapper.

## Acceptance criteria

- [ ] An MCP client (e.g. the `mcp` inspector or a scripted stdio session)
  can boot a 48SX, press `6 ENTER 7 * ENTER`, read the stack as `42`, and
  fetch a PNG of the screen.
- [ ] The web page runs the 48SX ROM in a browser at real time, keys work
  by mouse and keyboard, and a state survives a reload.
