---
type: iteration
title: "Iteration 37: saturnus-tx skeleton: Web Serial, connect, list, get and put"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus-tx
  - web
  - kermit
branch: iter-37/tx-skeleton
---

# Iteration 37: saturnus-tx skeleton

The first usable saturnus-tx ([[docs/saturnus-tx]]): a page at
<https://ractive.ch/saturnus/transfer/> that connects to a 48S/SX, 48G/GX or
49G in `SERVER` over Web Serial (or to the emulated one in the page),
finds the speed and the model, shows the calculator's tree and list, and
gets and puts files by the file input and downloads. One pane; the
second pane and the local folder are [[iterations/iteration-38-tx-two-panes]].

## Before it starts

- The consolidation is done ([[iterations/iteration-36a-import]] to
  36d): `satx-core` is in this repository, the core is `saturnus-core`,
  and the site builds `/saturnus/` with the emulator in `emulator/`.
- The owner has answered open question 1 of [[docs/saturnus-tx]]
  (2026-10-11): sans-I/O, as this iteration's first task, from draft
  PR 108.

## What it does

- **Connect**: "Add a calculator", the per-model instructions to start
  the server, Chrome's port chooser, 9600 8N1, then the speed and parity
  search if nothing answers, `VERSION` for the model, flag -35 set for
  the session and restored at disconnect, FINISH on disconnect.
- **List**: HOME's tree, each directory listed when opened; name, type,
  size, checksum; refresh.
- **Get**: binary `.hp` as a download; "Get as text" as `.txt`.
- **Put**: a file chosen or dropped onto the list goes into the shown
  directory; binary, `%%HP:` text, or "Store it as a string?"; "Replace
  X?" before a name in use.
- **Preview**: GROBs as pictures, data objects as text, programs as the
  calculator's text (one extra transfer).
- **The emulated calculator** as a device: "Add an emulated calculator"
  loads saturnus-web's Worker with a ROM from the ROMs kept on this origin
  (or a ROM file), types `SERVER`, and the same code talks to it over its
  emulated serial port.
- Browsers without Web Serial: a plain message (use Chrome or Edge, or
  the desktop app later) and the emulated calculator still available.

## Tasks

- [ ] `satx-core` for the browser: a non-blocking API (sans-I/O, owner 2026-10-11) with the blocking one kept as a wrapper for `satx`; `serialport` and TCP behind the default feature `native`, the in-process emulator transport behind the opt-in feature `saturnus` (which `satx` turns on), both off for the page; the draft rebased onto main first; `cargo check -p satx-core --no-default-features --target wasm32-unknown-unknown` in CI; `satx`'s tests and e2e suite unchanged and green
- [ ] Move the shared web code to `web/shared/` (components, theme, tokens, graphic, text forms; `el` and `MODEL_TITLES` untangled into `shared/dom.js` and `shared/models.js`); `site.sh`, `saturnus-tauri`'s `build.rs` and its `frontend` test follow; the emulator's web tests and overflow audit pass; its own commit, no behaviour change
- [ ] saturnus front-end protocol: `serialWrite {bytes}` and event `serial {bytes}` on every host (Worker, native runner), refused while a hidden transaction runs and the other way round; `web/protocol.md`; the protocol script and its expected output extended (state machine, native runner, Worker)
- [ ] `crates/saturnus-tx`: connect and detect (sync, speed and parity search, model, flag -35 kept and restored), list, get, put (binary, text, string), the file-name mapping (Unicode, `%XX`), FINISH; unit tests over recorded traces, failure cases included
- [ ] `just tx-traces DIR`: records the traces from saturnus (48SX, 48GX, 49G), committed
- [ ] `crates/saturnus-tx-web`: bindings over saturnus-tx and the preview parts of saturnus-objects, a `Link` given by JavaScript; `web/tx-build.sh` (wasm-pack into `web/tx/pkg`)
- [ ] `web/tx.html`, `web/tx/`: links (`WebSerialLink`, `EmulatorLink`, `FakeLink`), the device pane (`<device-tree>`, `<device-list>` as shared components over a `Device` interface), connect dialog, settings dialog, preview, toasts and confirms from `web/shared/`
- [ ] The protocol in a dedicated Worker; measure a 30 KB get in a hidden tab for five minutes (throttling) and record the result in the design doc
- [ ] Tests: `node --test web/test/tx-*.test.mjs` (pure modules, fake links); the page in headless Chrome with `FakeLink` (connect, list, put with replace, overflow at the five widths); `just tx-e2e DIR` against the emulated 48SX, 48GX and 49G
- [ ] Site and deploy: `web/site.sh` builds the `transfer/` subsite (`tx.html` as `index.html`, `tx/`, `shared/`, both wasm packages, its own marker, manifest, icons, `sw.js` and BUILD hash, the import check; `--list transfer`); its storage names in the storage-names module and test; a CI job (`wasm-tx`: both wasm builds, the transfer tests); the one `pages.yml` deploy carries it
- [ ] Docs: `web/README.md` (shared folder, the transfer page), [[docs/ci]], [[docs/releasing]] (the web page section), [[docs/architecture]] (the new crates), CHANGELOG
- [ ] Owner: on the 48SX and the 49G in Chrome or Edge with a USB serial cable: connect without typing settings, the model shown right, list, get a program and a GROB, put them back under new names, the preview of both; then the same against the emulated 48GX in Firefox or Safari

## Acceptance

- The four flows (connect, list, get, put) work against the emulated 48SX,
  48GX and 49G in `just tx-e2e`, byte for byte round trips.
- A hidden tab does not break a transfer (measured, written down).
- The emulator page and the desktop app behave as before the move; all
  their tests pass unchanged, and so do `satx`'s.
- `hyalo lint` clean; `just gates` green.
