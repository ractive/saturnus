---
type: iteration
title: "Iteration 50d: one document, router, home"
date: 2026-10-17
status: in-progress
tags:
  - iteration
  - web
  - saturnus-tx
branch: iter-50d/one-document
---

# Iteration 50d: one document, router, home

The two mountable views of [[iterations/iteration-50b-files-view]] and
[[iterations/iteration-50c-calculator-view]] go into one document at
`/saturnus/app/` (owner, 2026-10-17; [[decision-log]], "one app"). No
migration: the old subsites go without redirects.

## Design

- **Routes** (hash, no rewrite rules on ractive.ch): `#/` home,
  `#/calculator/<model>`, `#/files`, `#/files/<model>` (Files on that
  calculator). `?model=` and `?connect=` go. Back and forward through
  `hashchange`. The emulator's "Connected to saturnus tx · Show" chip
  (decision 2026-10-14) becomes a link to `#/files/<model>`.
- **Home**: the kept calculators (romstore), each with **Calculator** and
  **Files**, marked running when the hub has it (a new hub command
  `engines` listing the running models; `web/protocol.md` gets
  it); **New calculator**; **Connect a calculator** (Web Serial where
  there is one, else the line that says why). The installed app on a
  phone (`display-mode: standalone` and `pointer: coarse`) opens the
  last calculator; otherwise home.
- **One worker, two ports**: `hub.js` in a plain module Worker. The
  document makes a `MessageChannel` per view and transfers the port in;
  the worker calls `hub.connect(port)` per port. `join {who}` stays
  (`who` drives `peers`, speed, hidden, `broken`); the Web Lock for a
  page's life and the build check go, and with them
  `shared-worker.js`, `MIXED_BUILDS`, `canShare`. The connections live
  for the document; `leave` only on Files' explicit disconnect, so the
  calculator never stops because a view is hidden. The heartbeat stays
  (it catches a hung worker; `error` never fires for that). The
  auto-save lock stays.
- **HubDevice** runs over Files' port, unchanged otherwise.
- **One site**: `site.sh` builds `/saturnus/` (landing; the kill switch
  `sw.js` stays) and `/saturnus/app/` (one manifest, named and described
  as the one app; one precaching `sw.js`). **Kill switches for the old
  subsites**: `web/landing/sw.js` is scope-generic, so `site.sh` ships it
  as `emulator/sw.js` and `transfer/sw.js` too (removed with the first
  one on 2027-01-15, [[backlog/remove-kill-switch]]); otherwise the old
  apps keep running from cache and their hub writes auto-saves into the
  same database.
- **Storage names keep their values**: `saturnus`, `saturnus-roms`,
  `TRANSFER_DB`, `FOLDER_STORE`, `TRANSFER_OPFS_DIR` and every
  `saturnus.*` key; `storage-names.js` becomes one set and
  `storage-names.test.mjs` keeps pinning the values (the disjointness
  part goes). Kept ROMs and states must survive.
- **Landing**: its Transfer section and two buttons
  (`landing/index.html:47`, `:59`, `:190`, `:229`) become one app
  section and one button.
- **Names**: the app "saturnus"; the views "Calculator" and "Files".

## Tasks

- [x] One document at `web/index.html` with the router; both views mounted on first use and then hidden/shown
- [x] Hub over two transferred ports; `engines` command; SharedWorker, page lock, build check removed
- [x] Home page
- [x] From the review of 50c (PR 190): the calculator's error toast is closed when its view hides (its Escape handler must not act in Files); `status.js` `watchCleared` listeners gated on `shown()`; the shell un-hides a view's root before `setShown(true)` and re-places a running tour; load `app.js` lazily (its components define on import)
- [x] `site.sh`: landing + `app/`; kill switches at `emulator/sw.js` and `transfer/sw.js`; manifest; `tx.html` and `web/tx/pwa/` removed
- [x] `site-links.js` down to landing ↔ app; `storage-names.js` one set with values pinned
- [x] Landing page: one app section
- [x] Tests re-pointed, not deleted, where they carry coverage: `tx-page`, `tx-chrome`, `tx-hidden` at `#/files`; `hub`, `tx-shared`, `nav-page`, `site` rewritten; `tx-e2e*`, `tx-serve-e2e`, `protocol-script`, `killswitch` untouched. Tests of removed code (`site-links` tab messages, SharedWorker joins) deleted
- [x] `ci.yml` (the second web job and `tx-build.sh` steps go), `e2e.yml`, `pages.yml`
- [ ] Full web suite and `just gates` once; deploy
- [ ] Owner: on the Mac (Chrome) and a phone: home lists the kept calculators; Calculator → Files → back keeps the calculator running; a cable calculator connects in Files; the installed app opens where it should

## Acceptance

- One document at `/saturnus/app/`; the old subsites serve only their kill switches.
- Switching views never reloads the page and never stops the calculator.
- Kept ROMs and states from before are there.
- No SharedWorker, no tab messages.

## Out of scope

- The desktop app ([[iterations/iteration-51-one-desktop-app]]).
- Docs past what this touches ([[iterations/iteration-52-one-app-sweep]]).
