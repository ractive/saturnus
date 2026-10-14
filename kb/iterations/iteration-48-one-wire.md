---
title: "Iteration 48: one emulated serial wire for the products and the tests, a decode error type, and satx's file names"
type: iteration
date: 2026-10-14
status: planned
branch: iter-48/one-wire
tags:
  - iteration
  - saturnus-host
  - satx
  - refactor
---

# Iteration 48: one emulated wire, a proper error type

From the code review of 2026-10-14 (findings A and C) and the transfer
review. No user-visible feature; the point is that the tests run the code
users run.

## The problem

Two implementations drive the emulated calculator's serial port on
emulated time:

- **`crates/saturnus-host/src/transfer.rs`** (2953 lines): `Wire`
  (`now`, `run`, `discard`; `rx`, `start`, `idle`), `Exchange` (a Kermit
  transaction on `kermit-proto`'s `Client`, in `Phase::Pump`,
  `Turnaround`, `Read`), `opens_transaction`, the probe and the server's
  end. This is what users run: the hidden writes of the emulator page,
  the Tauri app and `saturnus run`, in turns of the protocol `Engine`.
- **`crates/saturnus-kermit/src/link.rs` and `kermit.rs`**: `Link`
  (`read`, `write_packet`, `wait_for_nak`, `run`, `discard_input`, the
  read cap) and `Server` over satx-core's sans-I/O `Machine`
  (`set_emulated_link`), blocking. Used by `saturnus-refgen`
  (`catalog.rs`, `menus.rs`, `examples.rs`, `names.rs`) and the
  ROM-gated `crates/saturnus-kermit/tests/e2e.rs`.

The same rules live twice with their own constants, already drifting:

| Rule | saturnus-host `transfer.rs` | saturnus-kermit |
| --- | --- | --- |
| Shortest read wait | `MIN_WAIT` 10 ms | `MIN_WAIT` 1 ms (`kermit.rs`) |
| Read step | `READ_STEP_MS` 1 | `STEP` 1 ms |
| Quiet line | `QUIET_MS` 4 | `QUIET` 4 ms |
| Turnaround | `TURNAROUND_MS` 200 | `TURNAROUND` 200 ms |
| Busy time per read | `MAX_BUSY` 600 s | `MAX_BUSY` 600 s |
| Reply timeout, retries, idle wait | `TIMEOUT` 6 s, `RETRIES` 3, `IDLE_WAIT` 100 ms | the same, again |
| Server cap, finish settle | `SERVER_CAP_MS` 15 000, `FINISH_SETTLE_MS` 2 500 | `SERVER_CAP` 15 s, `FINISH_SETTLE_MS` 2 500 (`emulator.rs`) |

A fix to one (busy time not counted, the idle NAK crossing a packet) does
not reach the other, and the ROM-gated tests and the reference generator
prove the copy users do not run.

## Design

- **`crates/saturnus-host/src/wire.rs`** (public module, wasm-safe: no
  threads, no I/O, no wall clock): `pub struct Wire` with the rules
  above as one set of `pub const`s, and a **resumable** read: `Wire::step(
  &mut self, m: &mut Machine, budget_cycles) -> WireStep` (`Quiet(bytes)`,
  `Pending`, `Deadline`), plus `write_packet` (turnaround before a packet
  that `opens_transaction`), `wait_for_nak`, `discard`, `now`, the read
  cap. The engine's `Exchange` calls `step` in its turns, as today's
  `Phase::Read`; a blocking caller loops `step` until it is not `Pending`.
- **saturnus-kermit on top of it**: `Link` keeps its API (refgen and the
  e2e tests change no call) but holds a `saturnus_host::wire::Wire` and
  loops it; `kermit.rs`'s `MIN_WAIT`/`IDLE_WAIT`/`TIMEOUT`/`RETRIES` and
  `emulator.rs`'s `SERVER_CAP`/`FINISH_SETTLE_MS` come from
  `saturnus_host::wire`. saturnus-kermit gains a dependency on
  saturnus-host (it is `publish = false`).
- **One Kermit client on that wire?** Not in this iteration: `Exchange`
  speaks `kermit-proto`'s `Client` directly and saturnus-kermit speaks
  satx-core's `Machine` (decision log, 2026-10-12, "one Kermit client and
  one character set"). Folding those is the next step and is recorded as
  an open question, not done here.
- **Which value wins where they differ**: `MIN_WAIT` becomes one constant.
  The reference generator regenerates its output byte for byte today
  (`saturnus-refgen` `tests/regen.rs`, `--ignored`) at 1 ms; the products
  run 10 ms. Measure both: take 1 ms if the engine's turns stay within
  their budget (`stats` `workMs` in `web/test/writes.test.mjs` and
  `crates/saturnus-host/tests/transfer.rs`), else take 10 ms and
  regenerate the reference once, committing the diff with the reason.
- **Split `transfer.rs`** along the way into `transfer/` modules:
  `wire` (moved out to `crate::wire`), `exchange.rs` (`Exchange`,
  `Transcript`, `Phase`, `Failure`, `config`), `ops.rs` (`Op`, `Target`,
  the plans: `copy_plan`, `free_name`, `text_refusal`, `wrapped`,
  `cd_command`, `check_name`), `run.rs` (`Transfer`, `Running`,
  `EndPhase`, `CancelPhase`, `SettleState`, the 49G's algebraic mode),
  `emulator.rs` (the `impl crate::Emulator` block) and `tests.rs`. No
  behaviour change; the public items keep their paths
  (`saturnus_host::transfer::{MAX_FILE_BYTES, may_serve, store_bytes,
  trim_fetched, TEXT_VARIABLE, SCREEN_VARIABLE, ...}` re-exported).

### `saturnus-objects`: `DecodeError` (finding C)

- `anyhow` leaves `saturnus-objects`' public API (and its dependencies).
  `pub struct DecodeError { pub offset: usize, pub reason: Reason }` with
  a small `pub enum Reason` (truncated, unknown prolog, too deep, too
  many objects, bad size field, not a real, ...; the `bail!` sites in
  `prolog.rs` and `object.rs` give the list), `Display` and
  `std::error::Error`. `decode`, `decode_at`, `object_size`,
  `element_size`, `records`, `Record::at`, `Real::from_body`,
  `Integer::from_nibbles` return it.
- The other fallible functions (`ram.rs` `UserMemory::of`, `tree`,
  `stack`, `object_at`, ...; `cmdline.rs`; `Real::parse`,
  `Graphic::from_packed`, `transfer.rs`) return `saturnus_objects::Error`,
  an enum with `Decode(DecodeError)` and the few other cases (no RPL
  memory on this model, memory not set up yet, parse), so callers can
  match the "not set up yet" case the protocol reports instead of
  matching text.
- Callers: `saturnus-host` (`error.rs` gets `From<saturnus_objects::Error>`),
  `satx-core` (its `charset` wrappers), `saturnus-drive`,
  `saturnus-kermit`, `saturnus-refgen`, `saturnus-cli` keep `anyhow`
  themselves with `.context(...)`. saturnus-objects and satx-core are
  published: a breaking change for the next release (CHANGELOG says so).

### satx: one file-name mapping, a Kermit watchdog

- **Names**: `crates/saturnus-tx/src/names.rs` (`FileKind`, `file_name`,
  `var_name`, `folder_name`, `dir_name`, `backup_file_name`, `escape`,
  `unescape`) moves to `crates/satx-core/src/names.rs`; its `Format`
  (`crates/saturnus-tx/src/device.rs`, Binary or Text) moves with it and
  its `TxError` becomes `satx_core::Error`. saturnus-tx re-exports it. satx-cli's default
  paths use it: `satx get X` writes `X.hp` (binary) or `X.txt`
  (`--ascii`), `%XX`-escaped as tx writes them (today `get` writes a file
  named `X`, `crates/satx-cli/src/commands.rs` `fn get`); `satx put
  X.hp` names the variable `X` by `var_name` (today
  `util::name_from_file`, which does not undo the `%XX` escapes). The CLI's README and
  `--help` say so.
- **Watchdog**: satx-core's Kermit transaction loop
  (`crates/satx-core/src/session.rs`, `transact`) gets the no-progress
  watchdog XModem has (`crates/satx-core/src/xmodem.rs`, `run`:
  `stall_limit` from timeout × (retries + 2), reset on every sign of
  progress, an `Error` naming the stall), so a peer that keeps a
  transaction alive without progress (repeated packets, garbage at the
  right rate) ends it. Check what PR 173 already changed in `session.rs`
  and `kermit-proto` (a repeated packet now counts as a retry) and build
  on it; the watchdog is the backstop, sans-I/O, so the `Machine` path
  (wasm, saturnus-tx) gets it too.

## Tasks

- [ ] `saturnus_host::wire`: `Wire` with the resumable `step`, the one set of constants, unit tests on a fake machine (quiet line, busy time not counted, turnaround before an opening packet, deadline, read cap)
- [ ] `transfer.rs` split into `transfer/` modules on `crate::wire`; no behaviour change (`crates/saturnus-host/tests/transfer.rs`, `typing.rs`, `clock.rs` and the protocol script unchanged)
- [ ] saturnus-kermit's `Link`, `kermit.rs` and `emulator.rs` on `saturnus_host::wire`; their own constants removed; `MIN_WAIT` measured and decided
- [ ] `just rom-tests DIR` green; `saturnus-refgen`'s `tests/regen.rs --ignored` regenerates byte for byte, or the reference is regenerated once with the reason in the commit
- [ ] `saturnus-objects`: `DecodeError`, `Reason`, `Error`; `anyhow` out of its `Cargo.toml`; every caller compiles with context; `cargo deny check` and `just package` (the publish rehearsal) green
- [ ] satx-core `names` (moved from saturnus-tx, its tests with it); satx-cli `get`/`put` default names on it, with tests in `commands.rs` (`get X` writes `X.hp`, `--ascii` `X.txt`, an escaped name)
- [ ] satx-core Kermit watchdog with tests: a peer replaying the last packet and a babbling line end with the stall error within the limit (fake clock); the `Session` and `Machine` paths both
- [ ] `cargo clippy -p satx-core --no-default-features --target wasm32-unknown-unknown -- -D warnings` and the wasm builds of saturnus-host (`just web`) and saturnus-tx-web (`just tx-web`) green
- [ ] Docs: [[docs/architecture]] (the wire, who uses it), [[docs/test-policy]] (saturnus-kermit's tests now run the products' wire), `crates/satx-cli` README (default file names), CHANGELOG (Unreleased: breaking `saturnus-objects` and `satx-core` API), decision log (the `MIN_WAIT` value)

## Acceptance

- `rg "const (MIN_WAIT|IDLE_WAIT|TURNAROUND|QUIET|MAX_BUSY|STEP)" crates`
  finds them only in `crates/saturnus-host/src/wire.rs`.
- saturnus-kermit has no read/turnaround/quiet-line logic of its own; its
  e2e tests and the reference generator run `saturnus_host::wire`.
- No file of `crates/saturnus-host/src/transfer/` is over 1000 lines.
- `saturnus-objects` does not depend on `anyhow`; `decode` returns
  `Result<Object, DecodeError>` with the offset where it failed.
- `satx get X` writes `X.hp`; tx and satx name files with the same code.
- A Kermit peer that never makes progress ends satx's transaction within
  the watchdog limit.
- `just gates` green.

## Out of scope

- One Kermit client for the engine and satx-core (see open question 1).
- Any change to the protocol's JSON or to what users see.

## Open questions for the owner

1. Fold the engine's `Exchange` (kermit-proto `Client`) and satx-core's
   `Machine` into one client next, or keep two clients on one wire?
2. If `MIN_WAIT` at 1 ms costs the engine's turns, is a regenerated
   reference (TICKS shifted by milliseconds in the generated pages)
   acceptable?
3. `satx get X` now writes `X.hp` instead of `X`: fine as a plain
   change (no compatibility flag, per the no-backwards-compat rule)?
