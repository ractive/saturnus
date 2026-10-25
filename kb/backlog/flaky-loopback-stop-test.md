---
type: backlog
title: saturnus-drive's a_stopped_server_stops_accepting is flaky
date: 2026-10-25
status: planned
priority: low
tags:
  - backlog
  - ci
---

# saturnus-drive's a_stopped_server_stops_accepting is flaky

Seen 2026-10-25 on PR #230 (which did not touch the crate): the macOS
`test` job failed in `loopback::guard::tests::a_stopped_server_stops_accepting`
(crates/saturnus-drive/src/loopback/guard.rs), at the assertion that a new
connection to the stopped server's port is refused. A rerun passed.

The likely cause: the test stops the server, then connects to its old
ephemeral port. Other tests in the same binary bind ephemeral ports in
parallel, so one of them can take that port in between and accept.

Fix: check that the stopped server no longer answers (the connection is
refused, or accepted and closed without a response), or hold the port
another way, rather than asserting that the port is free.
