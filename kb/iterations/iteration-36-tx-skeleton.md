---
type: iteration
title: "Iteration 36: saturnus-tx skeleton: Web Serial, connect, list, get and put"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus-tx
  - web
  - kermit
branch: iter-36/tx-skeleton
---

# Iteration 36: saturnus-tx skeleton

The first usable saturnus-tx ([[docs/saturnus-tx]]): a page at
<https://ractive.ch/saturnus-tx/> that connects to a 48S/SX, 48G/GX or
49G in `SERVER` over Web Serial (or to the emulated one in the page),
finds the speed and the model, shows the calculator's tree and list, and
gets and puts files by the file input and downloads. One pane; the
second pane and the local folder are [[iterations/iteration-37-tx-two-panes]].

## Before it starts

- hptx-core can be used from wasm (the `hptxwasm` work): no blocking
  transport, `default-features = false` drops `serialport` and saturnus,
  and a crates.io version exists (else the owner decides on a `deny.toml`
  git exception; [[docs/saturnus-tx]], open question 1).

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

- [ ] Move the shared web code to `web/shared/` (components, theme, tokens, graphic, text forms; `el` and `MODEL_TITLES` untangled into `shared/dom.js` and `shared/models.js`); `site.sh`, `saturnus-tauri`'s `build.rs` and its `frontend` test follow; saturnus's web tests and overflow audit pass; first commit, no behaviour change
- [ ] saturnus front-end protocol: `serialWrite {bytes}` and event `serial {bytes}` on every host (Worker, native runner), refused while a hidden transaction runs and the other way round; `web/protocol.md`; the protocol script and its expected output extended (state machine, native runner, Worker)
- [ ] `crates/saturnus-tx`: connect and detect (sync, speed and parity search, model, flag -35 kept and restored), list, get, put (binary, text, string), the file-name mapping (Unicode, `%XX`), FINISH; unit tests over recorded traces, failure cases included
- [ ] `just tx-traces DIR`: records the traces from saturnus (48SX, 48GX, 49G), committed
- [ ] `crates/saturnus-tx-web`: bindings over saturnus-tx and the preview parts of saturnus-objects, a `Link` given by JavaScript; `web/tx-build.sh` (wasm-pack into `web/tx/pkg`)
- [ ] `web/tx.html`, `web/tx/`: links (`WebSerialLink`, `EmulatorLink`, `FakeLink`), the device pane (`<device-tree>`, `<device-list>` as shared components over a `Device` interface), connect dialog, settings dialog, preview, toasts and confirms from `web/shared/`
- [ ] The protocol in a dedicated Worker; measure a 30 KB get in a hidden tab for five minutes (throttling) and record the result in the design doc
- [ ] Tests: `node --test web/test/tx-*.test.mjs` (pure modules, fake links); the page in headless Chrome with `FakeLink` (connect, list, put with replace, overflow at the five widths); `just tx-e2e DIR` against the emulated 48SX, 48GX and 49G
- [ ] Site and deploy: `web/tx-site.sh` (tx site with its own `pwa/` manifest and icons, the import check), a CI job (`wasm-tx`: both wasm builds, the tx tests) and the FTP upload to `httpdocs/saturnus-tx/` in `pages.yml` or `pages-tx.yml`
- [ ] Docs: `web/README.md` (shared folder, tx page), [[docs/ci]], [[docs/releasing]] (the web page section), [[docs/architecture]] (the new crates), CHANGELOG
- [ ] Owner: on the 48SX and the 49G in Chrome or Edge with a USB serial cable: connect without typing settings, the model shown right, list, get a program and a GROB, put them back under new names, the preview of both; then the same against the emulated 48GX in Firefox or Safari

## Acceptance

- The four flows (connect, list, get, put) work against the emulated 48SX,
  48GX and 49G in `just tx-e2e`, byte for byte round trips.
- A hidden tab does not break a transfer (measured, written down).
- saturnus's page and desktop app behave as before the move; all of
  saturnus's tests pass unchanged.
- `hyalo lint` clean; `just gates` green.
