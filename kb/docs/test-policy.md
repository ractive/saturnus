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
  skip otherwise: `tests/e2e.rs` of `saturnus`, `saturnus-cli` and
  `saturnus-mcp` (`just e2e DIR`), `saturnus-host`'s `tests/typing.rs`,
  `saturnus-tauri`'s `tests/runner.rs`, `saturnus-refgen`'s
  `tests/regen.rs` (its byte-for-byte regeneration is `--ignored`,
  minutes). `just rom-tests DIR` runs all of them; the release checklist
  does ([[docs/releasing]], step 4). The ROM-free integration tests
  (`saturnus-cli`'s `control.rs`, `reference.rs`) run in `just test`.
- Differential tests against the saturnng container are a script, run
  manually or in a separate CI job, limited to a few scripted scenarios.
- No fuzzing or property tests until a bug justifies one. Keep `just test`
  fast (seconds per crate in a warm build).
