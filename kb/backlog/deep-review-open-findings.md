---
title: "Open findings of the 2026-10-07 deep review (batch 4 and the remaining lows)"
type: backlog
date: 2026-10-07
status: planned
priority: high
tags:
  - backlog
  - saturnus
---

# Open findings of the 2026-10-07 deep review

From [[research/deep-review-2026-10-07]]. Batch 4 must land before the `v0.1.0` tag, because it changes public APIs that publishing would freeze: narrow the core crate's public surface (modules and fields no host uses become private or crate-visible); finish typing `saturnus-host`'s results; automated tests for the Worker's pacing and a cross-host test that sends the same commands to the Worker and the native runner and compares the replies. The lows can go into the same PR where cheap.

18 open:

- [ ] medium, `crates/saturnus/src/machine/mod.rs:132`: The core crate's public surface is far wider than any host uses, and it is about to be frozen by the first crates.io release. Every module is `pub` (cpu::exec, cpu::alu, bus::controller, modules::flash, io::uart, ...). Machine::cpu and Machine::hw are pub fiel
- [ ] medium, `crates/saturnus-host/src/lib.rs:24`: saturnus-host is meant to be host-neutral, but it shapes its API around the wasm binding: the `_inner` methods return Result<_, String> and JSON as text, often built by hand with format!. Native callers then parse that JSON back: runner.rs json_of at :622, :62
- [ ] medium, `web/worker.js:186`: The Worker host's pacing, wake, owed-time and memory-watch logic (pass, scheduleNext, wake, pollMemory, setSpeed, setHidden) has no automated test, and no test runs the same commands against the Worker and the native runner. The divergences listed in this file
- [ ] low, `crates/saturnus-drive/src/runner.rs:885`: Whether a failed key script or typing run halted the CPU is decided by substring-matching the formatted error (`message.contains("CPU halted")`, here and in runner/typing.rs:81), not by error type.
- [ ] low, `crates/saturnus-host/src/lib.rs:284`: Emulator has three 'release everything' methods with different semantics. release_all() releases only the keyboard matrix and leaves the KeyQueue as it is. release_all_inner() (host.rs:447) also clears the queue, and release_keys() (host.rs:538) is an alias of
- [ ] low, `crates/saturnus/src/error.rs:29`: Public variant Error::Unsupported { model } is never constructed anywhere in the workspace. The enum is not #[non_exhaustive] (deliberately, per the decision log), so removing the variant after publishing is a breaking change.
- [ ] low, `crates/saturnus-drive/src/rom.rs:12`: saturnus-drive has two ROM-loading policies. rom::load, which the CLI uses (main.rs:354, :586), reads the whole file with std::fs::read and checks the size afterwards. The runner's boot (runner.rs:1018) caps the read with read_capped(max_rom_file()). The CLI's
- [ ] low, `web/worker.js:1`: The front-end protocol and its pacing rules are implemented twice: web/worker.js (about 25 commands, timing constants MAX_BUDGET_MS, WAKE_BUDGET_MS, MAX_BEHIND_MS, MEMORY_LOOK_MS, TYPING_STEP_MS, ...) and saturnus-drive runner.rs with runner/typing.rs (the sam
- [ ] low, `crates/saturnus-cli/src/main.rs:187`: Model-name parsing is spread over several places. Core has Model::name() but no FromStr. saturnus-host has model_from_name (Result<Model,String>). The CLI repeats every name in a mirror enum ModelArg with a hand-written From (saturnus-mcp's parse_model went with the crate in iteration 18; saturnus-refgen uses model_from_name).
- [ ] low, `crates/saturnus-drive/src/session.rs:149`: The traced `run()` arm clones the entire `Machine` (`let mut probe = self.machine.clone()`) on every call while the CPU is shut down, to test whether a step would wake it without time passing — and `wait_idle` (line 262) calls `run()` every 2 emulated ms of se
- [ ] low, `web/worker.js:353`: During a frozen (long) send the Worker still posts `keys` and `error` events on every tick, while the native runner sends nothing until the send ends. The page's skin animates the typed key presses in the browser but not in the desktop app.
- [ ] low, `web/protocol.md:229`: protocol.md lists keyDown, keyUp, typeLetter, typeKeys, boot, bootModel and chooseRom as the commands the Worker refuses during a send. requireEmu() refuses every command built on it: reset, saveState, loadState, memoryTree, stack, flags and objectAt.
- [ ] low, `web/protocol.md:67`: The command table does not match the hosts on several fields. Tauri's `loadState` returns `{path}`, not `{}`. `stats` from the native hosts carries `rebases`, which the Worker lacks and the doc omits. `visibility` is ignored by the native hosts, but the doc gi
- [ ] low, `crates/saturnus-tauri/src/roms.rs:218`: The Tauri host reports `remembered: true` whenever it has a settings path, even after the settings write failed and `note` says the ROMs cannot be remembered. The Worker sets remembered=false on the same failure.
- [ ] low, `crates/saturnus-tauri/src/lib.rs:297`: Tauri ROM-slot commands change the remembered-ROM library in rom_work before they pass the sequencer, so only their boots are ordered by `seq`. Two ROM commands in flight can mutate and read the library out of page order.
- [ ] low, `crates/saturnus-web/src/lib.rs:7`: There is dead binding surface and a stale crate doc. worker.js never calls the exported run_ms, key_down, key_up, release_all, keys(), skin() (method), framebuffer, lcd_height, annunciators, contrast, contrast_range, is_shutdown, clock_hz, typing() or the free
- [ ] low, `web/components/sat-controls.js:63`: The Speed radiogroup is four role=radio buttons, each a separate tab stop, with no arrow-key handling or roving tabindex, so it does not behave as the ARIA radiogroup pattern it announces.
- [ ] low, `crates/saturnus-cli/src/control/server.rs:1023`: Several control-server unit tests depend on wall-clock timing on a loaded machine. idle_unauthenticated_connections_do_not_block_others asserts a reply within 1 s after opening 48 connections. authenticated_requests_are_capped sleeps 500 ms and assumes all 8 t

Planned as [[iterations/iteration-23c-review-open-findings]].
