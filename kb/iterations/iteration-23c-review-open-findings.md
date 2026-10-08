---
type: iteration
title: "Iteration 23c: The deep review's open findings (public API before v0.1.0, one protocol, robustness)"
date: 2026-10-07
status: in-progress
tags:
  - iteration
  - saturnus
branch: iter-23c/review-open-findings
---

# Iteration 23c: The deep review's open findings

All 18 findings of [[research/deep-review-2026-10-07]] still open after PRs 32 and 33 (owner, 2026-10-07: "Plan all the open ones"). Groups A and B change public APIs and must land before the `v0.1.0` tag; C and D can follow. One agent, in group order; each group leaves the tree green. The finding texts with file, line and failure scenario are in the research note's table.

Read first: [[research/deep-review-2026-10-07]], [[iterations/iteration-23-deep-review]] (Outcome), `kb/decision-log.md` (iteration 15 and 16 entries: the core API hptx relies on; the `#[non_exhaustive]` decision), `web/protocol.md`, `CLAUDE.md`.

## A. Public API before v0.1.0 (must land before the tag)

- [x] medium, `crates/saturnus/src/machine/mod.rs:132`: Make modules and fields no host uses private or `pub(crate)`; keep the API list in the decision log (iteration 16 and 15 entries) public and documented; accessors where hosts read fields; a test-only feature or `#[doc(hidden)]` where tests need internals. `cargo public-api` diff (or `cargo doc` item count) before and after in the Outcome.
- [x] medium, `crates/saturnus-host/src/lib.rs:24`: Typed results everywhere JSON text is still built by hand (frames, events, status); the runner stops parsing JSON it produced itself; one error convention across the library crates (state which in the decision log).
- [x] low, `crates/saturnus-host/src/lib.rs:284`: One release-everything method with documented semantics (matrix and queue); the others removed or renamed to say what they do.
- [x] low, `crates/saturnus/src/error.rs:29`: Remove the unused `Error::Unsupported` variant (no host constructs it) before publishing freezes it.
- [x] low, `crates/saturnus-web/src/lib.rs:7`: Remove the wasm bindings the Worker never calls; correct the crate doc.

## B. One protocol, one implementation (owner, 2026-10-07: "Will you also reorganize the crate structure?")

Not a reorganisation of the crates, whose boundaries stand, but one move:
the command/event protocol and its pacing (speed, sleep and wake,
catch-up, typing sends with the frozen screen, memory watching, ROM
slots' boot) are implemented twice today, in `crates/saturnus-drive/src/runner.rs`
and in `web/worker.js`, and about a third of the open findings come from
that. Move them into `saturnus-host` as one wasm-clean state machine
(commands in; replies and events out; "run cycles, then wake me at time
T" as its only contact with the outside). The Worker becomes a thin
JavaScript shell feeding it messages and timers; the native runner a thin
thread feeding it channels and the wall clock; `runner.rs`'s file handling
moves into its own module. `web/protocol.md` describes the one
implementation. This replaces the patch-by-patch items below, which the
move must make true (each one checked in the Outcome):

- [ ] medium, `web/worker.js:186`: Node tests for the Worker's pacing (pass, wake, owed time, memory watch, speed, hidden) with a fake core and fake timers; a cross-host test that drives the Worker (Node) and the runner (Rust, ROM-free with a zero ROM) through the same command script and compares the replies and events.
- [ ] medium, `web/worker.js:186`: Write the pacing constants once (a JSON or Rust-exported table both read), so the two pacers cannot drift; or, if that costs more than it saves, a test that checks the constants match.
- [ ] low, `web/worker.js:353`: During a frozen send the Worker posts no key or error events until the send ends, as the runner does.
- [ ] low, `web/protocol.md:229`: List exactly the commands refused during a send (all built on requireEmu), the same for both hosts.
- [ ] low, `web/protocol.md:67`: Bring the command table in line with the hosts (`loadState` reply, `stats` fields such as `rebases`), or the hosts in line with the table.
- [x] low, `crates/saturnus-drive/src/runner.rs:885`: Halt detection by a typed error (an enum variant), not by matching "CPU halted" in the message.

- [ ] The state machine in `saturnus-host` with unit tests (pacing with a fake clock, every command, the refusals during a send); the Worker and the runner as thin drivers; the Node tests and the tauri runner tests pass unchanged or are reduced to driver tests; headless Chrome and the desktop self-test still pass.

## C. Hosts and robustness

- [ ] low, `crates/saturnus-tauri/src/roms.rs:218`: The Tauri host reports `remembered: false` when the settings write failed, as the Worker does.
- [ ] low, `crates/saturnus-tauri/src/lib.rs:297`: ROM-slot commands go through the command sequencer like every other command, so two in flight cannot interleave their library changes.
- [ ] low, `crates/saturnus-drive/src/rom.rs:12`: One ROM-loading policy: the CLI reads ROM, state and card files with the size cap the runner uses (a read that stops at cap + 1), so `--rom /dev/zero` is refused.
- [x] low, `crates/saturnus-cli/src/main.rs:187`: Model names parsed in one place (`FromStr` for `Model` in the core, or one function in `saturnus-host`); the CLI, the web bindings and the runner use it.
- [ ] low, `crates/saturnus-drive/src/session.rs:149`: Traced runs on a shut-down CPU use `idle_cycles()` instead of cloning the whole machine (the 49G's 4 M-nibble flash per call).

## D. Page and tests

- [ ] low, `web/components/sat-controls.js:63`: The Speed radio group follows the ARIA radiogroup pattern: one tab stop, arrow keys move the selection.
- [ ] low, `crates/saturnus-cli/src/control/server.rs:1023`: Control-server unit tests that assert wall-clock bounds on a loaded machine: replace sleeps and 1 s bounds with event-based waits or generous bounds stated as such.

## Acceptance criteria

- [ ] Every item fixed, or answered in the Outcome with the reason.
- [ ] The public API of the five published crates listed in the Outcome (before and after); hptx's list from the decision log still available.
- [ ] `just gates` and `just rom-tests` pass.

## Outcome

### Group A (public API), with C's model names and B's halt detection

Public API counted from rustdoc's JSON output (`cargo +nightly doc` with
`--output-format json`, walked from each crate root; `cargo public-api`
is not installed). An entry is an item at a public path: re-exports count
again, enum variants and public fields count too.

| crate | before | after | notes |
| --- | --- | --- | --- |
| `saturnus` | 1294 (647 distinct names) | 191 | 40 structs, 46 enums, 491 functions, 111 public fields, 12 public modules before; 5 structs, 5 enums, 59 functions, 14 fields, 2 modules (`io`, `disasm`) after; 98 of the 191 are `Key` (80) and `Model` (7) variants |
| `saturnus-host` | 261 | 289 | typed answers added (`Frame`, `KeysDown`, `Grid`, `GridKey`, `SkinView`, `Letters`, `SendResult`, `RunResult`, `AnnunciatorFlags`, `Error`'s variants); removed `model_from_name`, `annunciators_json`, `skin_json`, `layout_json`, `layout_of`, `model_for_rom_name`, `Shown`, `Error::message`, `Emulator::{release_all, take_frame, take_keys, invalidate, framebuffer, lcd_height, contrast, contrast_range, clock_hz}` |
| `saturnus-drive` | 95 | 99 | `session::Halted` added |
| `saturnus-web` | 56 | 40 | the 16 bindings the Worker never calls removed |
| `saturnus-objects` | 527 | 527 | untouched |

- Core: the notable removals are the modules `cpu` (decoder, ALU,
  `Cpu`, `Registers`, the instruction enums), `bus` (`MemoryController`,
  `Chip`), `machine` (`Hardware`, `HardwareProfile`, `ChipRole`, `Card`),
  `modules` (`Flash`, `Rom`, `Ram`, `Nce1`), `state`, the I/O chips in
  `io` (`IoRegisters`, `Timers`, `Uart`, `LewisIo`, `Keyboard`, `Layout`,
  `KeyPos` and their register constants), the fields `Machine::cpu`/`hw`,
  `Model::{hardware, cycle_table, keyboard_layout}`, `Key::{position,
  on_layout}`, `Lcd::{render, render_lewis, row_spans}`,
  `Annunciators::{from_bits, from_lewis}` and `Error::Unsupported`. Added:
  `Machine::{pc, contrast, poke, release_all_keys}`, `Model::{has_key,
  keys}`, `FromStr for Model` with `Error::UnknownModel`,
  `disasm::{decode, Instruction}`, and the constants at the root. hptx's
  list (decision log, iterations 15 and 16) is all still public.
- Tests that need internals: feature `internals` (the crate's
  dev-dependency on itself turns it on for `tests/e2e.rs`; the `boot`
  example requires it). `just lint` and CI's `clippy` job gained a
  `--lib --bins` clippy run, where the feature is off, so no host can use
  it unnoticed.
- Error convention (decision log): typed enums where callers branch
  (`saturnus::Error`, `saturnus_host::Error` with `Halted`, `Machine`,
  `Message`), `anyhow` where they only report (`saturnus-objects`,
  `saturnus-drive`, whose `Session` puts a typed `Halted` inside). The
  runner detects halts by `Error::Halted` and
  `downcast_ref::<session::Halted>()`; no "CPU halted" string matching
  remains.
- Answered, not changed: `Runner::handle` and its helpers keep
  `Result<Value, String>`, since the error is the protocol reply's
  message and group B replaces the runner; `saturnus-host`'s other
  public modules (`KeyQueue`, `sha256`, `base64`, `romid`'s JSON
  functions over `serde_json::Value`) were not narrowed, as they are
  typed already and B reshapes the host side; `Emulator` keeps its
  forwarding methods (`press`, `release`, ...) next to `queue()`, which
  the bindings use.
- wasm: the `.d.ts` that wasm-bindgen generates (built with types into a
  scratch directory) differs only by the removed members; every method
  and function the Worker calls has the same signature.
