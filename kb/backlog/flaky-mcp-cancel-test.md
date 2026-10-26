---
type: backlog
title: saturnus-tauri's a_window_that_does_not_answer is flaky
date: 2026-10-26
status: planned
priority: low
tags:
  - backlog
  - ci
---

# saturnus-tauri's a_window_that_does_not_answer is flaky

Seen 2026-10-26 in the 0.10.0 release dry run (run 40253973128, the
`aarch64-apple-darwin` saturnus build): `mcp::tests::a_window_that_does_not_answer`
(crates/saturnus-tauri/src/mcp.rs) panicked with "the call was not
cancelled". The same commit passed in PR #246's CI, and a rerun of the
job passed.

The test writes a `tools/list` request to a slow window, sleeps 100 ms,
drops the connection, then waits up to 10 s for the call to be cancelled.
On a loaded runner the server may not have started the call within the
100 ms, so the drop can come before there is a call to cancel.

Fix: wait until the window has received the call (a flag set by the slow
window) before dropping the connection, instead of a fixed sleep.
Related: [[flaky-loopback-stop-test]].
