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
- One integration test binary (`tests/e2e.rs`) that boots a ROM only when
  `SATURNUS_ROM_DIR` is set: boot to the memory prompt, a key sequence with a
  known screen, a Kermit exchange. A handful of scenarios, no more.
- Differential tests against the saturnng container are a script, run
  manually or in a separate CI job, limited to a few scripted scenarios.
- No fuzzing or property tests until a bug justifies one. Keep `cargo test`
  under a few seconds.
