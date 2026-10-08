---
type: iteration
title: "Iteration 22: The web app installable on phones (manifest, offline, touch)"
date: 2026-10-06
status: in-progress
tags:
  - iteration
  - saturnus
branch: iter-22/installable
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

- **Owner's additions (2026-10-07, after using fullscreen on a phone):**
  - Edge-to-edge fullscreen: crop the skin at the keyboard plate (drop
    the outer shell and its rounded border) and scale to the screen
    width; keep the logo and model band unless it costs a key row.
  - Command palette on touch screens: hide the keyboard-shortcut hint
    where the pointer is coarse; in fullscreen a small search icon in the
    top-left corner opposite the close button; swipe down on the display
    opens the palette. No floating button over the keys.

## Tasks

- [x] Manifest, icons, Apple meta tags; `web/site.sh` ships them and
  writes the build hash.
- [x] Service worker with versioned precache and update notice; not
  registered in the desktop app.
- [x] Pointer and touch handling on the keys and the components; hit
  targets; no zoom or selection on the calculator.
- [x] Phone layouts with safe areas, portrait and landscape; edge-to-edge
  fullscreen; the palette's touch triggers.
- [x] Wake lock and persistent storage.
- [x] Verification: Lighthouse's installability checks pass on the
  served page; headless Chrome with a phone viewport and touch
  emulation (keys, palette, explorer); offline reload after install.
- [ ] By hand on the owner's iPhone and an Android device (install from
  Safari and Chrome, boot a ROM, use it, kill and reopen, offline).

## Acceptance criteria

- [ ] On an iPhone and an Android phone, "Add to Home Screen" installs
  the page; it opens full screen, boots the remembered ROM, works
  offline, and keys respond to touch without delay.
- [x] A new deploy replaces the cached version on the next open.
- [x] `just gates` passes.

## Outcome

What changed (decision log, "iteration 22"): `web/pwa/` holds the
manifest (standalone, any orientation, relative URLs), the icons drawn
from `web/logo.svg` by `web/pwa/icons.sh` (plain, maskable, Apple's
touch icon; no HP marks), Apple's head tags and the service worker;
`web/site.sh` ships them at the site's top, inserts the tags into the
site's `index.html` and writes the build hash and the precache list in
front of the worker (37 files, the wasm package included). The worker
precaches one build into `saturnus-<BUILD>`, serves only those files and
the page's navigation from it, and waits as a new build; an untouched
page takes it at once, a page in use shows a notice (Reload, Later).
`web/pwa.js` registers it only over HTTPS or on localhost and never on
the Tauri host, and the app embeds none of `web/pwa/` (the Tauri test
`frontend` asserts it). Fullscreen is edge to edge (the case dropped,
the skin cropped to its face, on the case's colour; the 42S's logo,
which sits on the case above its plates, is dropped), with a page-level
fallback where the Fullscreen API is missing (the iPhone), a search icon
top left and a swipe down on the display for the palette. Keys: no
callout or context menu on a long press, each pointer's key released
once, `touch-action: none` in fullscreen, held keys released when the
page is hidden (the engine's `visibility` releases nothing; only a
window blur did). Wake lock after 5 s of computing; persistent storage
asked for once a ROM is kept, its answer under the ROM hint. The
`.htaccess` gained the manifest's MIME type; its CSP already allowed the
worker and the manifest (`worker-src 'self'`, `default-src 'self'`).

Already covered by iteration 26 and not redone: the phone layouts
(portrait and landscape), the tokens, 44 px targets on coarse pointers,
safe-area insets, `touch-action: manipulation` and no selection on the
calculator, the palette as a phone sheet, and the keyboard hints hidden
on coarse pointers. Iteration 24's `g.case`/`g.face` split is what the
edge-to-edge view hides and measures.

Verified in headless Chrome over the DevTools protocol (`pwa.mjs` in
the session's scratch directory
`/private/tmp/claude-501/-Users-james-devel-saturnus/92b88cf2-5ffa-4c95-ba06-e64134673bb8/scratchpad/iter22/`,
the built site served under `/saturnus/` on localhost, 390 x 844 at 2x
with touch emulation): the manifest parses without errors and
`Page.getInstallabilityErrors` (the check Lighthouse's installability
audit reads) reports none; the worker controls the page with one cache
of 37 files; a touch held 700 ms on the 48SX's F key holds it and lifting
releases it (pointer events `pointerdown`, `pointerup`,
`lostpointercapture`, no `contextmenu`, no selection); with the server
cut off a reload makes no request that is answered, and the 48SX boots
from its kept ROM; a rebuilt site with one changed byte is a new build
that the next, untouched open takes (one cache left), and a page in use
gets the notice and its Reload takes the next build; the palette opens
from the search icon and from a swipe on the display and not from a tap;
the page fallback enters and leaves; a 4000-iteration loop on the 48SX
holds the wake lock after 5 s and lets go when it ends; all seven
models' idle machines sleep (`loop` `sleep`). Lighthouse itself was not
run (not installed here).

Screenshots, look first at `fs-48sx-portrait.jpg` (edge to edge at 360,
390, 430), `fs-48sx-landscape.jpg`, `fs-models-390.jpg` (42S, 49G, 38G,
39G) and `fs-palette-fallback-update-offline.jpg` (the palette from
fullscreen, the iPhone fallback, the update notice, the page offline);
the single shots are in `shots/` (`fs-<model>-<w>x<h>.png`). Every key is
inside the screen at every size; the 48SX's keys are 41-49 px wide in
portrait (35 px in the normal phone layout), and 19-28 px in landscape,
where the calculator is height-bound.

Tests: `web/test/pwa.test.mjs` (where the worker may register, the wake
lock only after the delay and around a hidden page, persistent storage
asked once and its refusal shown, nothing asked in the app),
`web/test/site.test.mjs` (the precache list equals the built site's
files, every `site.sh --list` file in it, none of the installable files
in the app's list, the head tags and the manifest's icons present, the
same tree the same hash, a changed file a new one), and the Tauri
`frontend` test's new assertion. `web/test/overflow.test.mjs` stays
green. `just gates`, `cargo clippy -p saturnus-tauri --all-targets -- -D
warnings` and `cargo test -p saturnus-tauri -q` pass in the worktree.

The owner's steps by hand (the unticked boxes): publish the site (the
Pages workflow), then on the iPhone in Safari and on an Android phone in
Chrome: Add to Home Screen; open it (full screen, the icon and name
right); choose a ROM and see the storage line under the ROM hint; press
and hold keys (no delay, no callout, no zoom); fullscreen edge to edge,
the search icon and the swipe on the display; kill the app and reopen
(the remembered ROM boots); airplane mode and reopen (it still works);
after the next deploy, open it once and see the new version (an
untouched page reloads itself).
