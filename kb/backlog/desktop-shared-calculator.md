---
type: backlog
title: Desktop tx on the desktop emulator's running calculator
date: 2026-10-17
status: deferred
priority: low
tags:
  - backlog
  - desktop
  - saturnus-tx
---

# Desktop tx on the desktop emulator's running calculator

On the web, saturnus tx uses the calculator running in the emulator tab
(the SharedWorker hub, iterations 46 and 47; "Files" connects to it). The
two desktop apps cannot: separate processes and webviews, so desktop tx
only boots a private copy from the emulator app's ROM (own memory).
Owner, 2026-10-17: build it only if it is not too complicated; never as a
Kermit link over a pseudo-terminal. Assessed as medium (about 1,100 lines
of code and 600 of tests across about 14 files), so deferred.

## Design sketch (from the assessment)

- **Transport:** loopback TCP, a per-launch 256-bit token in a bridge file
  (0600) in the emulator app's config folder, mutual HMAC challenge
  (the control API's `Token::proof`, moved into `saturnus-drive`), u32
  length-prefixed JSON frames capped at 1 MiB; HTTP bytes close the
  connection. Allowlist of memory commands only (`memoryTree`,
  `fetchFile`, `storeFile`, `rename`, `purge`, `createDir`, `backup`,
  `watchMemory`); one tx client at a time.
- **Emulator app:** the calculator already runs natively
  (`saturnus-drive` runner, the same `protocol::Engine` as the hub), so a
  bridge server relays `Request{file: None}` to `Machine.tx`, answering
  `hello`/`attach`/`leave`/`ping` itself. A `FanSink` routes
  `memoryChanged` to whoever watches; the engine's single watch flag needs
  an arbiter (the page's `hello`, boot or unload clear it).
- **tx app:** a bridge client (commands `emulator_calc`,
  `emulator_calc_open` with a Channel, `emulator_calc_request`,
  `emulator_calc_close`); the page gets a `DesktopHubClient` with
  `HubClient`'s surface, and `web/shared/hub-device.js` works unchanged.
- **"Files" on desktop:** push `connect` to a running tx, else launch it
  with `--connect <model>` (macOS `open -b`; elsewhere a `location.json`
  tx writes); focus per OS.

## Risks that made it "not small"

- Watch/`hello` interplay in the single-flag engine (tx goes silent after
  the emulator page reloads if wrong).
- Window launch and focus per OS (Windows foreground lock, Wayland).
- One model at a time on desktop (the runner has one machine).
- Quitting the emulator mid-copy kills the operation (on the web the
  SharedWorker outlives the tab): an owner decision.
- A second loopback listener: a security doc and decision-log entry.
