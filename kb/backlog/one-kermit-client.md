---
title: "One Kermit client: the engine's Exchange and satx-core's Machine"
type: backlog
date: 2026-10-14
status: completed
priority: low
tags:
  - backlog
  - satx
  - saturnus-host
---

# One Kermit client for the engine and satx-core

After [[iterations/iteration-48-one-wire]] both run on one emulated
wire (`saturnus_host::wire`), but two Kermit clients stay:
`crates/saturnus-host/src/transfer` speaks `kermit-proto`'s `Client`
directly (the hidden writes users run), saturnus-kermit speaks
satx-core's sans-I/O `Machine` (refgen, the ROM-gated tests).

Owner (2026-10-14): not in iteration 48; a later iteration folds them
into one client, so the tests run the same Kermit client users run too.
What to settle then: whether the engine takes satx-core's `Machine`
(and its `set_emulated_link` mode), and what that costs the engine's
turns.
