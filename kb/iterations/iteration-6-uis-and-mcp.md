---
title: "Iteration 6: UIs and MCP"
type: iteration
date: 2026-10-04
status: completed
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

- [x] `saturnus-mcp`: an MCP server (stdio) owning one `Machine` per
  session: tools `press_keys` (key script syntax), `type_text`, `screen`
  (PNG and text), `read_stack`, `send_object`/`receive_object` and
  `run_command` over Kermit through `hptx-core`, `save_state`/`load_state`,
  `reset`; paced in emulated time with an idle-wait like the CLI.
- [x] Web UI: `saturnus-web` WASM bindings of the core (wasm-bindgen) and a
  static page: LCD on a canvas with annunciators, a drawn keyboard per
  model from the key layout (no HP marks), ROM file picker, run in real
  time, save/load state in the browser; served by any static file server.
- [-] Deferred until photos exist: pixel-faithful skins from the owner's
  photographs; Tauri desktop wrapper.

## Acceptance criteria

- [x] An MCP client (e.g. the `mcp` inspector or a scripted stdio session)
  can boot a 48SX, press `6 ENTER 7 * ENTER`, read the stack as `42`, and
  fetch a PNG of the screen.
- [x] The web page runs the 48SX ROM in a browser at real time, keys work
  by mouse and keyboard, and a state survives a reload.

## Outcome

### Web UI

- `crates/saturnus-web` (cdylib + rlib, `wasm-bindgen` and `js-sys` only):
  `Emulator::new(model, rom)`, `run_ms`, `key_down`/`key_up` by script name,
  `release_all`, `keys()` (physical layout: `{columns: 30, rows, keys:
  [{name, label, row, x, w}]}`), `framebuffer()` (131 x 64, one byte per
  pixel, row-major, 1 = dark), `annunciators()`, `contrast()`,
  `contrast_range()`, `save_state`/`load_state`, `reset`, `cycles`,
  `emulated_ms`, `is_shutdown`, `clock_hz`, `model`; free functions
  `model_names()` and `rom_bytes(model)`. The physical layouts live in this
  crate (`layout.rs`), not in the core, with unit tests against the core's
  matrices. Release wasm: about 125 KB.
- `web/`: `index.html`, `app.js`, `style.css`, `build.sh`, `README.md`;
  `web/pkg/` is the gitignored wasm-pack output.
- Verified in headless Chrome 154 driven over the DevTools protocol (the
  Claude-in-Chrome extension was not connected): the ROM picked through the
  file input (`DOM.setFileInputFiles` with `roms/sxrom-j`), "Try To Recover
  Memory?" shown, NO and `6 ENTER 7 * ENTER` clicked on the drawn keys with
  real mouse events, 42 on levels 1 and 2; `8` Enter typed on the keyboard;
  Save state, page reload, ROM picked again, Load state: the LCD matches the
  saved one pixel for pixel. Status line at 100% speed throughout. The 49G
  ROM boots to its memory prompt with the 49G keyboard, dark theme checked.
- Speed: CPU-bound slices (not in SHUTDN) run about 35x (48SX), 45x (48GX)
  and 55x (49G) faster than real time in V8 (node 24, same wasm); an idle
  calculator costs under 2% of a frame.
- Open: the 49G case arrangement is not in the wiki and is unverified; the
  38G labels follow the wiki's inferred mapping; the ROM is not stored, so a
  reload needs the ROM picked again before Load state.

### MCP

- `crates/saturnus-drive` (new library): the CLI's key scripts, scripted
  session with idle wait, per-model boot and SERVER autostart, ROM loading
  and screen dumps, moved out of `saturnus-cli` so the CLI and the MCP
  server share them. CLI behaviour and tests unchanged; new:
  `boot_script(model)`, collected `wait-idle` warnings, scaled in-memory
  PNGs.
- `crates/saturnus-mcp` (binary `saturnus-mcp`, rmcp 3.5 on stdio,
  `hptx-core` pinned to hptx `1a6cec7`): tools `boot`, `press_keys`,
  `type_text`, `screen`, `start_server`, `stop_server`, `read_stack`,
  `run_command`, `send_object`, `receive_object`, `save_state`,
  `load_state`, `reset`, `status`; one session lock; Kermit over an
  in-process `Transport` on the owned machine (emulated time, see the
  decision log).
- Checked by hand against the ROMs: boot with autostart, `run_command
  "6 7 *"` and `read_stack` give 42 on the 48SX, 48GX and 49G (each Kermit
  call well under a second); ASCII `send_object`/`receive_object` with a
  `%%HP:` header round-trips a program on the 48SX and 49G; `type_text
  "abc Xy Q1 Z"` shows on the 48SX and 49G screens.
- Acceptance: `crates/saturnus-mcp/tests/e2e.rs` (gated on
  `SATURNUS_ROM_DIR`) drives the server through rmcp's client over an
  in-process duplex pipe: boot the 48SX (0.8 s), `press_keys "6 ENTER 7 *
  ENTER"` (1.5 s), `start_server` (2.4 s), `read_stack` gives `2: 42` and
  `1: 42` (0.2 to 0.4 s), `screen` PNG decodes to 131x64; 5.3 s in a debug
  build.
- Bug found and fixed in the core: the 48SX Kermit server never answered
  when the stack held values and some emulated time passed before SERVER
  (`6 ENTER`, a wait, SERVER). The UART kept RBF set with its request held
  and the ROM never read RBR: the ROM's interrupts-off path returned with
  RTN and the edge-latched request was lost. RTI now re-enters the handler
  while the UART request is still held (`machine/mod.rs`, regression test
  `hp48sx_kermit_server_hears_packets_right_after_a_nak`).
- Open: the 38G gets no letters from `type_text`; the calculator is frozen
  between tool calls, so its clock lags wall time.
