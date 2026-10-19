---
type: iteration
title: "Iteration 41: one emulated calculator per model, shared by the emulator and saturnus tx"
date: 2026-10-11
status: completed
tags:
  - iteration
  - saturnus-tx
  - web
branch: iter-41/shared-calculators
---

# Iteration 41: shared calculators

Today saturnus tx boots a fresh emulated calculator from a kept ROM and
types SERVER into it, so it never sees what the user did in the
emulator (owner, 2026-10-11: "It doesn't make much sense to just boot
up an empty calc on tx"). Both pages live on the same origin, so they
can share one calculator per model: the emulator's auto-saved state
(`auto:<model>` in the `saturnus` database, web/states.js) becomes the
calculator both pages run. Several named calculators per model come
later, if at all.

## Design

- **One calculator per model.** The transfer page's "An emulated
  calculator…" lists the models whose ROM is kept, and boots the chosen
  one from its auto-saved state (a fresh boot only when there is none).
  It saves back to the same state, as the emulator does.
- **One page runs a calculator at a time**, through Web Locks
  (`navigator.locks`, lock name per model). The Worker that runs the
  calculator holds the lock and writes the state only while it holds it,
  so a page that lost the lock never writes again.
- **Handover**: a page that wants a calculator asks the holder over a
  BroadcastChannel; the holder saves, pauses ("In use by saturnus tx",
  or "In use by the emulator", with "Take it back") and releases. If no
  answer comes within a few seconds (a frozen background tab), the page
  takes the lock with `steal` and boots from the last auto-save.
- **Transfer mode**: before the transfer page hands a calculator back
  (or saves it for good), it leaves SERVER, so the emulator never gets
  it back waiting for a connection.
- **Versions**: the two pages are separate installed apps with their own
  caches, so they can run different builds. The saved state carries a
  format version; a page that meets a newer format says so ("Reload this
  page to use this calculator") instead of failing.

## Tasks

- [x] Web Locks and BroadcastChannel handover in a shared module (`web/shared/`), used by the emulator's Worker and saturnus tx's emulator Worker; the steal timeout; the lost-lock guard before every state write
- [x] Emulator page: hold the lock for the running model, the "In use by saturnus tx" state with "Take it back", reload of the newer state on taking it back
- [x] saturnus tx: emulated calculators from the shared states, leaving SERVER before saving or handing back
- [x] The state's format version and the "too new" message on both pages
- [x] Tests: unit tests for the handover and lock logic with fakes; one automated two-page test (the emulator holds the 48GX, saturnus tx takes it over, the state comes across). No simulation of frozen tabs, Android tab discards or mixed builds in CI: those go to the owner's check. If the two-page test is flaky or slow, keep one stable case and report.
- [x] Docs: [[docs/saturnus-tx]] ("Devices"), web/README.md, CHANGELOG, decision log

### Owner's checklist (by hand; `web/test/tx-shared.test.mjs` covers the 48GX handed over and back in two tabs)

Owner checks still open: [[backlog/owner-checks]]

## Acceptance

- A variable stored in the emulator shows up in saturnus tx's emulated
  calculator of that model, and one copied there shows up back in the
  emulator.
- Two pages never run the same calculator at once, and no page writes a
  state after it lost the lock.

## Out of scope

- More than one calculator per model.
- One live calculator shown by both pages at once (a SharedWorker): not
  in Chrome on Android, and the emulator's Worker would need rebuilding.
