---
type: iteration
title: "Iteration 23c: The deep review's open findings (public API before v0.1.0, one protocol, robustness)"
date: 2026-10-07
status: planned
tags:
  - iteration
  - saturnus
branch: iter-23c/review-open-findings
---

# Iteration 23c: The deep review's open findings

All 18 findings of [[research/deep-review-2026-10-07]] still open after PRs 32 and 33 (owner, 2026-10-07: "Plan all the open ones"). Groups A and B change public APIs and must land before the `v0.1.0` tag; C and D can follow. One agent, in group order; each group leaves the tree green. The finding texts with file, line and failure scenario are in the research note's table.

Read first: [[research/deep-review-2026-10-07]], [[iterations/iteration-23-deep-review]] (Outcome), `kb/decision-log.md` (iteration 15 and 16 entries: the core API hptx relies on; the `#[non_exhaustive]` decision), `web/protocol.md`, `CLAUDE.md`.

## A. Public API before v0.1.0 (must land before the tag)

- [ ] medium, `crates/saturnus/src/machine/mod.rs:132`: Make modules and fields no host uses private or `pub(crate)`; keep the API list in the decision log (iteration 16 and 15 entries) public and documented; accessors where hosts read fields; a test-only feature or `#[doc(hidden)]` where tests need internals. `cargo public-api` diff (or `cargo doc` item count) before and after in the Outcome.
- [ ] medium, `crates/saturnus-host/src/lib.rs:24`: Typed results everywhere JSON text is still built by hand (frames, events, status); the runner stops parsing JSON it produced itself; one error convention across the library crates (state which in the decision log).
- [ ] low, `crates/saturnus-host/src/lib.rs:284`: One release-everything method with documented semantics (matrix and queue); the others removed or renamed to say what they do.
- [ ] low, `crates/saturnus/src/error.rs:29`: Remove the unused `Error::Unsupported` variant (no host constructs it) before publishing freezes it.
- [ ] low, `crates/saturnus-web/src/lib.rs:7`: Remove the wasm bindings the Worker never calls; correct the crate doc.

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
- [ ] low, `crates/saturnus-drive/src/runner.rs:885`: Halt detection by a typed error (an enum variant), not by matching "CPU halted" in the message.

- [ ] The state machine in `saturnus-host` with unit tests (pacing with a fake clock, every command, the refusals during a send); the Worker and the runner as thin drivers; the Node tests and the tauri runner tests pass unchanged or are reduced to driver tests; headless Chrome and the desktop self-test still pass.

## C. Hosts and robustness

- [ ] low, `crates/saturnus-tauri/src/roms.rs:218`: The Tauri host reports `remembered: false` when the settings write failed, as the Worker does.
- [ ] low, `crates/saturnus-tauri/src/lib.rs:297`: ROM-slot commands go through the command sequencer like every other command, so two in flight cannot interleave their library changes.
- [ ] low, `crates/saturnus-drive/src/rom.rs:12`: One ROM-loading policy: the CLI reads ROM, state and card files with the size cap the runner uses (a read that stops at cap + 1), so `--rom /dev/zero` is refused.
- [ ] low, `crates/saturnus-cli/src/main.rs:187`: Model names parsed in one place (`FromStr` for `Model` in the core, or one function in `saturnus-host`); the CLI, the web bindings and the runner use it.
- [ ] low, `crates/saturnus-drive/src/session.rs:149`: Traced runs on a shut-down CPU use `idle_cycles()` instead of cloning the whole machine (the 49G's 4 M-nibble flash per call).

## D. Page and tests

- [ ] low, `web/components/sat-controls.js:63`: The Speed radio group follows the ARIA radiogroup pattern: one tab stop, arrow keys move the selection.
- [ ] low, `crates/saturnus-cli/src/control/server.rs:1023`: Control-server unit tests that assert wall-clock bounds on a loaded machine: replace sleeps and 1 s bounds with event-based waits or generous bounds stated as such.

## Acceptance criteria

- [ ] Every item fixed, or answered in the Outcome with the reason.
- [ ] The public API of the five published crates listed in the Outcome (before and after); hptx's list from the decision log still available.
- [ ] `just gates` and `just rom-tests` pass.

## Outcome

(to be written)
