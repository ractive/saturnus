---
type: iteration
title: "Iteration 50: one app on the web"
date: 2026-10-17
status: planned
tags:
  - iteration
  - web
  - saturnus-tx
branch: iter-50/one-app-web
---

# Iteration 50: one app on the web

The emulator and saturnus tx become one app: one HTML document at
`/saturnus/app/`, a hash router, a home page that lists the
calculators, and the calculator and Files as views of it (owner,
2026-10-17; [[decision-log]], "one app"). No migration: the old
subsites, their service workers and their URLs go without redirects or
shims, and the site may be broken between steps of this iteration.

## Design

- **Routes** (hash, so ractive.ch needs no rewrite rules):
  `#/` home, `#/calculator/<model>` the emulator view, `#/files` tx
  (with `?connect=<model>` replaced by `#/files/<model>`: Files on that
  calculator). Back and forward work through `hashchange`.
- **Home**: the kept calculators (romstore, read directly), each with
  **Calculator** and **Files**, marked running when the worker has it;
  **New calculator** (add a ROM); **Connect a calculator** (cable: Web
  Serial where there is one, else the line that says why). On a phone
  (`(pointer: coarse)` and narrow) the installed app opens the last
  calculator directly; on a desktop it opens home.
- **Mountable views**: the emulator's and tx's start code become
  `mount(root)` / `unmount()` modules. Document- and window-level
  listeners move to the view's root or are gated on the view being
  shown (keys, paste, drop, dragover, resize; `sat-calculator`,
  `sat-controls`, `sat-palette`, `sat-shortcuts`, `sat-tour`,
  `tx/app.js`); tx's `beforeunload` guard only while a transfer runs.
  Styles scoped under each view's root (`#calculator-view`,
  `#files-view`); `tokens.css` and `components.css` shared. A hidden
  view keeps running (the calculator keeps its clock); Files loads
  lazily on first use.
- **One worker, not shared**: `hub.js` runs in a plain module Worker;
  the calculator view and Files are two connections into it (the
  HubDevice over that connection). Gone: `shared-worker.js`,
  `joinHub`'s lock and build check, `MIXED_BUILDS`, `canShare`. The
  auto-save lock stays (a second tab gets a calculator that does not
  save, `autoSaveOff`). Keep the heartbeat only if it catches something
  a Worker's `error` event does not; otherwise remove it.
- **One WebAssembly package**: `saturnus-tx-web` merges into
  `saturnus-web`; one `web/build.sh`, one `web/pkg/`. tx's private
  calculator (`tx/emulator.js`, a ROM file chosen in Files, driven over
  its serial port) stays as the cable path's honest test and the aplet
  server's target, reached from Files only, not listed on home.
- **One site**: `web/site.sh` builds `/saturnus/` (landing, kill
  switch `sw.js` kept until 2027-01-15) and `/saturnus/app/` (the app,
  one manifest, one precaching `sw.js`). The landing page links to the
  app. `storage-names.js` becomes one set of names (no disjointness
  check). `site-links.js` shrinks to the landing ↔ app link; the named
  tabs and tab messages go.
- **Names**: the app is "saturnus"; the views "Calculator" and "Files"
  (owner, 2026-10-14). "saturnus tx" goes from the UI.

## Tasks

- [ ] Router and shell (`web/app/` or `web/index.html` as the one document), home page
- [ ] Emulator start code as `mount`/`unmount`; listeners and styles scoped
- [ ] tx start code as `mount`/`unmount`; listeners and styles scoped; lazy load
- [ ] hub.js in a plain Worker; SharedWorker, join lock, build check removed; HubDevice on the in-app connection
- [ ] Merge `saturnus-tx-web` into `saturnus-web`; remove `web/tx-build.sh`, `web/tx/pkg`
- [ ] `site.sh`: landing + `/saturnus/app/`; one manifest, one service worker; `tx.html`, `web/tx/pwa/` removed
- [ ] `site-links.js`, `storage-names.js` cut down; tests of removed code deleted, not fixed
- [ ] `just`, `ci.yml`, `e2e.yml`, `pages.yml` follow the new layout
- [ ] Full `just web-test-full` once at the end; deploy
- [ ] Owner: on the Mac (Chrome) and a phone: home lists the kept calculators; Calculator → Files → back keeps the calculator running; a cable calculator connects in Files; the installed app opens where it should

## Acceptance

- One document at `/saturnus/app/`; `/saturnus/emulator/` and `/saturnus/transfer/` are gone from the deployed site.
- Switching between the calculator and Files never reloads the page and never stops the calculator.
- No SharedWorker, no tab messages, one WebAssembly package.
- `just gates` and the web suite green at the end (not per step).

## Out of scope

- The desktop app ([[iterations/iteration-51-one-desktop-app]]).
- Leftover docs and dead code past what this iteration touches ([[iterations/iteration-52-one-app-sweep]]).
