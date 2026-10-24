---
type: iteration
title: "Iteration 63: one Kermit client for the engine and satx-core"
date: 2026-10-24
status: in-progress
tags:
  - iteration
  - satx
  - saturnus-host
branch: iter-63/one-kermit-client
---

# Iteration 63: one Kermit client for the engine and satx-core

From [[backlog/one-kermit-client]]. Owner 2026-10-24: "Do it". Since
[[iterations/iteration-48-one-wire]] both run on one emulated wire
(`saturnus_host::wire`), but there are two Kermit clients:
`crates/saturnus-host/src/transfer` drives `kermit-proto`'s `Client` directly
(the writes users run), `crates/saturnus-kermit` drives satx-core's sans-I/O
`Machine` (refgen, the ROM-gated tests). Goal: one client, so the tests run the
Kermit client users run.

## Design

- Settle first, and record in the decision log: whether the engine takes
  satx-core's `Machine` (with its `set_emulated_link` mode) or saturnus-kermit
  moves onto the engine's `Exchange`, and what that costs the engine's turns
  (wasm, no threads, the Worker's pacing). Pick the smaller change that leaves
  one client; no shims, no backwards compatibility (the owner is the only user).
- The protocol crates (`kermit-proto`, satx-core's sans-I/O parts) keep building
  for wasm32.
- Users see no change: Files' writes and reads, the desktop's transfers, satx
  behave as before.

## Tasks

- [x] Decision: which client stays; decision-log entry
- [x] Fold the other one into it; remove the duplicate
- [ ] Tests: the transfer tests and the ROM-gated Kermit tests pass on the one client; satx e2e against saturnus (`just satx-e2e-saturnus 49g`)
- [ ] kb: backlog item completed
- [ ] Owner: a file sent and received in Files on the web and in the desktop app
