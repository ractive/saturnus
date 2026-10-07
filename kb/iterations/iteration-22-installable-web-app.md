---
type: iteration
title: "Iteration 22: The web app installable on phones (manifest, offline, touch)"
date: 2026-10-06
status: planned
tags:
  - iteration
  - saturnus
branch: iter-22/installable-web-app
---

# Iteration 22: The web app installable on phones (manifest, offline, touch)

Owner (2026-10-06): "What would it mean to create an iOS and Android
app?" Decided: first make the existing web page installable (a PWA);
native Tauri apps for iOS and Android are a later decision, after using
this. "App" means the web page here; the desktop app is unaffected.

Read first: `web/index.html`, `web/app.js`, `web/worker.js`,
`web/components/sat-calculator.js` (pointer handling of the keys),
`web/style.css` (the layouts below 1000 px and 760 px from iteration 12),
`web/romstore.js` (IndexedDB), `web/site.sh` and `pages.yml` (what ships),
`kb/iterations/iteration-10-web-design.md` and `iteration-12-memory-explorer.md`
(layout decisions).

## Design

- **Manifest** (`web/manifest.webmanifest`): name, short name, the
  saturnus logo as icons (maskable and plain, generated from
  `web/logo.svg` at build time or committed as PNGs made from it),
  `display: standalone`, portrait orientation preferred, theme and
  background colours from the page's palette, `start_url` relative so it
  works under `/saturnus/` on both hosts. Apple's meta tags for the home
  screen (`apple-mobile-web-app-capable`, status bar style, touch icon).
- **Service worker** (`web/sw.js`): precache the page's files and the
  wasm package at install, versioned by a build hash written by
  `web/site.sh` (so a new deploy replaces the cache; no stale wasm with
  new glue JS); serve from cache, update in the background, tell the page
  when a new version is ready; cache nothing else (the page makes no
  network requests; ROMs stay in IndexedDB). Registered only over HTTPS
  or localhost. The desktop app's embedded page must not register it
  (the app has no network).
- **Touch**: keys through pointer events with press-and-hold semantics
  (the key queue's hold time), no 300 ms delay, no text selection or
  callout on long press, no double-tap zoom on the calculator; the
  palette, explorer and controls usable by touch (hit targets at least
  44 px); a two-finger or edge gesture is not needed.
- **Layout on phones**: the calculator fills the width in portrait with
  safe-area insets (notch, home indicator); the side panel and the layer
  as the existing narrow layouts; landscape shows the calculator alone
  at full height; the keyboard shortcut hints hidden where there is no
  keyboard.
- **Lifecycle**: when the page goes to the background the emulator keeps
  idle-sleeping as today (no work while the CPU is shut down); on return
  the wake catch-up applies as today; a wake lock (Screen Wake Lock API,
  where available) while the calculator runs a long computation so the
  screen does not sleep mid-program; `visibilitychange` releases held
  keys.
- **Storage**: ROMs and states are already in IndexedDB; the page asks
  for persistent storage (`navigator.storage.persist()`) once a ROM is
  kept, and shows whether it was granted (iOS evicts storage of pages not
  used for 7 days unless installed or persisted).
- **What stays out**: push, background sync, share targets, native file
  handles beyond the picker.

## Tasks

- [ ] Manifest, icons, Apple meta tags; `web/site.sh` ships them and
  writes the build hash.
- [ ] Service worker with versioned precache and update notice; not
  registered in the desktop app.
- [ ] Pointer and touch handling on the keys and the components; hit
  targets; no zoom or selection on the calculator.
- [ ] Phone layouts with safe areas, portrait and landscape.
- [ ] Wake lock and persistent storage.
- [ ] Verification: Lighthouse's installability checks pass on the
  served page; headless Chrome with a phone viewport and touch
  emulation (keys, palette, explorer); offline reload after install;
  on the owner's iPhone and an Android device by hand (install from
  Safari and Chrome, boot a ROM, use it, kill and reopen, offline).

## Acceptance criteria

- [ ] On an iPhone and an Android phone, "Add to Home Screen" installs
  the page; it opens full screen, boots the remembered ROM, works
  offline, and keys respond to touch without delay.
- [ ] A new deploy replaces the cached version on the next open.
- [ ] `just gates` passes.

## Outcome

(to be written)
