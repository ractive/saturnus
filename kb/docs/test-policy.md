---
title: Test policy
type: docs
date: 2026-10-04
status: active
tags:
  - testing
  - saturnus
---

# Test policy

Tests must help without slowing the project down (lesson from ff-rdp and hyalo).

- Instruction tests are unit tests in the core crate: register state in, state
  out. Fast.
- ROM-gated integration tests run only when `SATURNUS_ROM_DIR` is set and
  skip otherwise: `tests/e2e.rs` of `saturnus-core`, `saturnus` (the CLI) and
  `saturnus-kermit` (`just e2e DIR`), `saturnus-host`'s `tests/typing.rs` and `tests/clock.rs`,
  `saturnus-tauri`'s `tests/runner.rs`, `saturnus-refgen`'s
  `tests/regen.rs` (its byte-for-byte regeneration is `--ignored`,
  minutes). `just rom-tests DIR` runs all of them; the release checklist
  does ([[docs/releasing]], step 4). The ROM-free integration tests
  (the CLI's `control.rs`, `reference.rs`) run in `just test`.
- The front-end protocol is one state machine (`saturnus-host`'s
  `protocol`), unit-tested with fake clocks (and a fake core for the
  pacing). One command script, `web/test/protocol-script.json`, runs
  through it, through the native runner
  (`saturnus-drive/tests/protocol_script.rs`) and through the Web Worker
  with the real wasm (`web/test/protocol-script.test.mjs`, `just
  web-test`, which builds `web/pkg` first); all three must give
  `protocol-script.expected.json` (`SATURNUS_BLESS=1` rewrites it from
  the state machine's run). A change to a reply or an event shows there.
- Differential tests against the saturnng container are a script, run
  manually or in a separate CI job, limited to a few scripted scenarios.
- `saturnus-kermit`'s ROM-gated tests and `saturnus-refgen` run the
  products' emulated wire (`saturnus_host::wire`) and Kermit client
  (`saturnus_host::transfer::Exchange`), each transaction run to its
  end, so they prove the turnaround, quiet-line, busy-time and timeout
  rules and the Kermit client users run ([[iterations/iteration-63-one-kermit-client]]).
  The wire's rules are unit-tested on a fake calculator in `wire.rs`.
- No fuzzing or property tests until a bug justifies one. Keep `just test`
  fast (seconds per crate in a warm build).

## Web tests

`web/test/*.test.mjs` (node:test, headless Chrome over the DevTools
protocol in `web/test/chrome.mjs`):

- `just web-test` is the fast default: every file but the `*e2e*` ones,
  three files at a time, about 4 minutes with `SATURNUS_ROM_DIR` set on
  the owner's Mac (it was 13 to 19 minutes with `tx-e2e` in it).
- `just web-test-full DIR` adds the `*e2e*` files (`tx-e2e`: the
  Files against the emulated 48SX, 48GX and 49G, about 5 minutes
  with its four tests side by side), run beside the rest: about 5.5
  minutes. `just tx-e2e DIR` runs that file alone.
- A file named `*e2e*` is full-suite only. Name a new file that way when
  it boots emulated calculators for minutes; anything that finishes in
  well under a minute belongs in the fast suite.
- One Chrome per test file: `session(t)` gives each test a page in a
  browser context of its own (fresh storage, caches and service
  workers), on the file's shared Chrome. Such a context is off the
  record: storage in memory, an incognito quota; a test that needs a
  real profile's storage (quota, `persist()`, large writes) uses
  `chrome()`, its own Chrome and profile. Wait for a condition (`until`,
  `settled`), not a fixed sleep, where the page offers one; the sleeps
  left wait out the page's own timers (a message's 4 s, the autosave's
  delay).

When to run what:

- While working, run only the test files a change touches (`node --test
  web/test/x.test.mjs`), and `tx-e2e` for one model
  (`--test-name-pattern 49g`) when the transfer path changed.
- The full suites (`just gates`, `just web-test-full DIR`) once, right
  before the PR. After review fixes, only the affected tests again.
- CI's run on `main` is the safety net. A failure that is already there
  on `main` is noted in the PR, not chased.
