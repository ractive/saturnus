---
type: iteration
title: "Iteration 23c: The deep review's open findings (public API before v0.1.0, one protocol, robustness)"
date: 2026-10-07
status: completed
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

- [x] medium, `web/worker.js:186`: Node tests for the Worker's pacing (pass, wake, owed time, memory watch, speed, hidden) with a fake core and fake timers; a cross-host test that drives the Worker (Node) and the runner (Rust, ROM-free with a zero ROM) through the same command script and compares the replies and events.
- [x] medium, `web/worker.js:186`: Write the pacing constants once (a JSON or Rust-exported table both read), so the two pacers cannot drift; or, if that costs more than it saves, a test that checks the constants match.
- [x] low, `web/worker.js:353`: During a frozen send the Worker posts no key or error events until the send ends, as the runner does.
- [x] low, `web/protocol.md:229`: List exactly the commands refused during a send (all built on requireEmu), the same for both hosts.
- [x] low, `web/protocol.md:67`: Bring the command table in line with the hosts (`loadState` reply, `stats` fields such as `rebases`), or the hosts in line with the table.
- [x] low, `crates/saturnus-drive/src/runner.rs:885`: Halt detection by a typed error (an enum variant), not by matching "CPU halted" in the message.

- [x] The state machine in `saturnus-host` with unit tests (pacing with a fake clock, every command, the refusals during a send); the Worker and the runner as thin drivers; the Node tests and the tauri runner tests pass unchanged or are reduced to driver tests; headless Chrome and the desktop self-test still pass.

## C. Hosts and robustness

- [x] low, `crates/saturnus-tauri/src/roms.rs:218`: The Tauri host reports `remembered: false` when the settings write failed, as the Worker does.
- [x] low, `crates/saturnus-tauri/src/lib.rs:297`: ROM-slot commands go through the command sequencer like every other command, so two in flight cannot interleave their library changes.
- [x] low, `crates/saturnus-drive/src/rom.rs:12`: One ROM-loading policy: the CLI reads ROM, state and card files with the size cap the runner uses (a read that stops at cap + 1), so `--rom /dev/zero` is refused.
- [x] low, `crates/saturnus-cli/src/main.rs:187`: Model names parsed in one place (`FromStr` for `Model` in the core, or one function in `saturnus-host`); the CLI, the web bindings and the runner use it.
- [x] low, `crates/saturnus-drive/src/session.rs:149`: Traced runs on a shut-down CPU use `idle_cycles()` instead of cloning the whole machine (the 49G's 4 M-nibble flash per call).

## D. Page and tests

- [x] low, `web/components/sat-controls.js:63`: The Speed radio group follows the ARIA radiogroup pattern: one tab stop, arrow keys move the selection. Moved to [[iterations/iteration-26-refinement-and-mobile]] (team lead, 2026-10-08) and done there (PR 43: `web/radiogroup.js`, `radiogroup.test.mjs`).
- [x] low, `crates/saturnus-cli/src/control/server.rs:1023`: Control-server unit tests that assert wall-clock bounds on a loaded machine: replace sleeps and 1 s bounds with event-based waits or generous bounds stated as such.

## Acceptance criteria

- [x] Every item fixed, or answered in the Outcome with the reason.
- [x] The public API of the five published crates listed in the Outcome (before and after); hptx's list from the decision log still available.
- [x] `just gates` and `just rom-tests` pass.

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

### Group B (one protocol, one implementation), with C's ROM-loading policy

The protocol and its pacing are `saturnus_host::protocol::Engine`, a
wasm-clean state machine: `command(clock, msg, bytes, reply_tag)` in,
`take_output()` (events, then the reply they precede) out, `deadline()`
for the one timer a host arms and `timer(clock)` when it fires; the host's
`Clock` is its only contact with time. `web/worker.js` (125 lines, was
641) drives it through the wasm bindings' `Host` with `performance.now()`
and `setTimeout`, and keeps the ROM slots in IndexedDB; the native runner
drives it with `Instant` and its channel, and keeps files, dialogs and the
native commands (`screen`, `info`, `model`, `peek`, `poke`, `keyScript`).
`runner.rs`'s file handling moved to `saturnus-drive::files`; the native
`Pacer` (`saturnus-drive::pacer`) is gone.

- Worker pacing tests and the cross-host test: the pacing now lives in
  Rust, so its tests are there, with a fake core and fake clocks
  (`crates/saturnus-host/src/protocol/tests.rs`: idle at every speed for
  both tunings, computing at 1x/2x/4x, Max then sleep at 1x, resuming at
  Max gains nothing, the 12 h catch-up cap, owed time paid by the
  passes, hidden, the browser timer's slack, the memory watch's rules).
  The Node tests test the Worker as a driver (`web/test/worker.test.mjs`:
  one timer at the deadline, replies by id, bytes apart, ROM commands
  checked first) and run the cross-host script with the real wasm
  (`web/test/protocol-script.test.mjs`); the same script runs through the
  engine with a fake clock and the native runner
  (`crates/saturnus-drive/tests/protocol_script.rs`); all three give
  `web/test/protocol-script.expected.json` (48 events and replies).
- Pacing constants written once: `Pacing::WORKER` and
  `Pacing::NATIVE` side by side in `protocol/pacing.rs`; the Worker has no
  constants of its own.
- No key or error events during a frozen send: the engine holds
  `frame`, `keys` and the key queue's errors on every host (a command's
  own error, sent without an `id`, still goes out: it is its answer).
- The refusal list is `REFUSED_WHILE_TYPING`, one list for every
  host, listed in `protocol.md` ("Typing"); sends run in turns between
  messages natively too, `releaseAll` cancels them there as well.
- The command table: `loadState`'s results per host, `stats` with the
  same fields everywhere (`rebases` went with the `Pacer`), `visibility`
  ignored natively, `setSpeed` takes a string.
- The state machine with unit tests (17), the thin drivers, the Node
  tests reduced to driver tests plus the cross-host script, the tauri
  runner tests unchanged and passing with the ROM, headless Chrome and
  the desktop self-test passing.
- C, falling out: the CLI reads ROM, state and card files with the
  runner's capped read (`--rom /dev/zero` is refused).

Differences between the hosts, resolved by the protocol: sends served
between messages natively (they blocked the thread); one refusal list
(the Worker's requireEmu set plus `keyScript` and `poke`, with one
message); frozen sends hold the same events; no memory looks during a
send (the Worker looked); field checks as the runner's (`speed`,
`keys`, `address`, the version message); the Worker caps states at
4 MiB; pass, wake and owed-time rules are the Worker's natively (passes
every 1 ms instead of the `Pacer`'s 1 ms slices; at most 100 ms of wall
time per pass instead of a 200 ms re-anchor). Kept and documented:
`visibility` does nothing natively, Tauri answers `{path}` for states.

Measurements, headless Chrome, 48SX (before / after): idle Worker work
0.4-0.7 / 0-0.5 ms per 10-20 s with 20 wakes per 10 s; idle at Max
emulated 10055.5 over 10056 ms / 10062.3 over 10062 ms; a program
redrawing the display: 29.4 / 29.2 frame events per second at 1x, 57.2 /
57.0 at Max, 60 animation frames per second throughout. Natively (tauri
runner test, 5 s each): idle 100.00% with 0 passes, computing 1x 99.99%,
4x 3.99, Max 6.8x; the desktop self-test 100.00% idle and 1x, 399.4% at
4x. Headless Chrome on the page: 48SX from the ROM slots, mouse keys,
`run`, a 122-character send (busy, a typed letter refused, then the
frame), speed, state saved and loaded, the memory view with
`memoryChanged`, the palette (SIN), reload booting the remembered ROM, no
console errors.

Not changed: a wake that finds the CPU computing at Max waits for the next
pass (16 ms in the browser) before running it, and that wall time is not
accounted; with real ROMs the timer work ends within the wake's run, so
no drift shows (above), but a fake core whose tick computes 2 ms loses
about 3% at Max. Left as it was in the Worker.

### Groups C and D (the remaining lows)

- `remembered` after a failed write: `Library` keeps `unsaved` when
  writing the settings file fails; `romSlots` then answers
  `remembered: false` with the note, as the Worker does when its store
  refuses. Unlike the Worker, the app tries again on the next change, and
  a write that succeeds makes it `true` again and clears the note.
  `protocol.md` says so. Test `roms::tests::a_failed_write_is_not_remembered`.
- ROM-slot commands in order: the sequencer has a third kind of slot,
  `Slot::Turn`. It is released when the earlier numbers have gone through
  and holds the later ones until its number is admitted again. A ROM
  command takes its turn and then does all its work: the library, the
  dialog, the boot (sent to the machine thread under the order lock, and
  only while the turn holds, so a page reloaded during the dialog never
  gets a boot it did not ask for; PR 44 review) and `booted`. After
  that it admits `Skip`.
  Two ROM commands can no longer interleave, and a boot's `lastModel`
  can no longer land after a later `forgetRom`. A later command still
  waits while a dialog is open, as before. Tests
  `order::tests::a_turn_holds_the_later_numbers`,
  `order::tests::a_reload_ends_a_held_turn`,
  `tests::a_boot_after_a_reload_is_not_sent`. The desktop ROM-slot
  self-test (`selftest-roms.js`, phases choose, hold38g and restart,
  missing) passes.
- Traced runs on a shut-down CPU: `is_shutdown() && idle_cycles().is_none()`
  (a wake condition holds) replaces the clone and probe step. Test
  `session::tests::a_traced_run_traces_the_first_instruction_after_a_wake`
  (a SHUTDN ROM woken by ON). It fails when the wake is removed. The
  research note's second row for this finding ("fixed, PR 33") was wrong:
  the clone was still there.
- Control-server tests without wall-clock bounds:
  `idle_unauthenticated_connections_do_not_block_others` has no 200 ms
  sleep. The server accepts in order, so the idle connections are pending
  before the request. Its 1 s bound became an order of events: the
  newest idle connection has no answer yet when the request is answered.
  The 500 ms read on the dropped one became 3 × `HEAD_TIMEOUT`, where a
  connection that was not dropped would get its 408 instead.
  `a_client_that_leaves_withdraws_its_command` waits until the handler
  has let go of the ticket (a generous 30 s, stated) instead of sleeping
  3 × `CLIENT_CHECK`. `authenticated_requests_are_capped` plays the
  machine thread: it takes the 8 queued commands (so every slot is held),
  gets 503 for the ninth, then answers the 8 (200), with no 500 ms sleep
  and no 3 s reply timeout. The 300 ms reply timeout of
  `timed_out_commands_never_run_and_the_queue_is_bounded` is a lower
  bound only and stays.
- Speed radiogroup: moved to [[iterations/iteration-26-refinement-and-mobile]] and done there (PR 43).

### Public API of the published crates

The four library crates are counted again with one method across all
three points: rustdoc JSON (`cargo +nightly rustdoc -- --output-format
json`), walked from the crate root. An entry is a public item at a public
path, with public fields, variants and inherent associated items. The
"before" point is main before PR 39 (`6e3f77c`), "after A" is PR 39
(`5909d26`), and "now" is this branch. These counts differ by a few
from group A's table, which used a slightly different walk. The fifth
published crate, `saturnus-cli`, has no library. Its API is the
`saturnus` command line, which 23c did not change apart from the capped
file reads.

| crate | before | after A | now | now, top level |
| --- | --- | --- | --- | --- |
| `saturnus` | 1323 | 193 | 193 | `Machine`, `Model`, `Lcd`, `Framebuffer`, `Annunciators`, `Halt`, `Port`, `Error`, the constants (`ADDR_MASK`, `LCD_*`, `CARD_*`, `NEW_CARD_BYTES`); `io::Key`; `disasm::{decode, Instruction}` |
| `saturnus-host` | 267 | 295 | 404 | `Emulator`, `Error`, `Result`, `AnnunciatorFlags`, `MemoryTree`; modules `host` (`Keyboard`, `KeyQueue`, `Frame`, `KeysDown`), `layout`, `protocol` (`Engine`, `Clock`, `Pacing`, `Speed`, `Event`, `Output`, `Reply`, `Status`, `Stats`, ...), `romid`, `sha256`, `skins`, `typing` |
| `saturnus-drive` | 102 | 106 | 97 | modules `autostart`, `files` (capped reads, atomic writes), `rom`, `runner` (`Runner`, `Request`, `Ticket`, `Sink`, `Hook`, `Service`), `screen`, `script`, `session` (`Session`, `Limits`, `Halted`); `pacer` and `runner::Speed` gone |
| `saturnus-objects` | 536 | 536 | 536 | untouched |

`saturnus-web` (`publish = false`) went from 56 to 40 to 11 (the one
`Host` binding the Worker drives). The core is item for item the same
as after group A, so hptx's list (decision log, iterations 15 and 16) is
all still public. `saturnus-host` grew by `protocol`, group B's engine.
