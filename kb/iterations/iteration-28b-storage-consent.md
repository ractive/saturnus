---
type: iteration
title: "Iteration 28b: Ask before persistent storage"
date: 2026-10-08
status: in-progress
tags:
  - iteration
  - saturnus
branch: iter-28b/storage-consent
---

# Iteration 28b: Ask before persistent storage

Owner (2026-10-08): Firefox shows a context-free "allow to permanently
save information?" prompt as the page loads, which could scare people
away. Wanted: the mobile-app pattern, asking in context, in our own
words, first.

Until now (iteration 22) the page called `navigator.storage.persist()`
on every load once a ROM was kept, until it got an answer. Firefox turns
that call into a prompt; Chrome answers silently.

Read first: `web/pwa.js`, `web/components/sat-controls.js` (the ROMs
panel), `web/romstore.js`, `web/store.js`, `web/app.js` (the
preferences), `kb/iterations/iteration-22-installable-web-app.md`.

## Design

- **Never on load.** At load the page reads `navigator.storage.persisted()`
  only, which never prompts, to show the state in the ROMs panel.
- **In context, in our words.** Right after the user picks, drops or
  downloads a ROM (or takes an offered one), and only if `persisted()` is
  not already true, a notice at the bottom of the page in its own style
  (the update notice's card, not a modal): "Keep this ROM on this device?
  Without this, the browser may delete it when space runs low, and you'd
  have to pick it again." with "Not now" and "Keep it".
- **Keep it** calls `persist()` at once in the click handler (a user
  gesture), before any `await`; Firefox shows its prompt then, Chrome
  answers silently. The outcome in one line with OK: "Kept on this
  device." or "The browser said no: it may still clear it when space runs
  low."
- **Not now** is kept in localStorage (`saturnus.storageAsk`, through the
  page's `prefs`, wrapped in try/catch like the other settings); the
  notice is not offered again.
- **The ROMs panel** shows the state once a ROM is kept, "Stored
  permanently." or "May be cleared when space runs low.", with a "Keep
  permanently" button whenever storage is not persistent, so the choice
  can be made later. It goes through the same `StorageChoice.keep()`.
- **Not in the desktop app** (`backend.romSource !== "file"`), nor where
  `navigator.storage.persist` is missing: no state line, no notice, no
  call.
- The logic is `StorageChoice` in `web/pwa.js` (testable under Node with
  fakes); its notice is `showStorageOffer`. `sat-controls` only says that
  a ROM was kept (`sat-rom-kept`) and that its button was pressed
  (`sat-keep-storage`); `app.js` wires both to the choice.

## Tasks

- [x] `StorageChoice` and `showStorageOffer` in `web/pwa.js` replace
  `persistWhenKept`; `storageOffer` in the store.
- [x] `sat-controls`: `sat-rom-kept` after a pick, drop, download or
  offer; the storage state and "Keep permanently" in the ROMs panel
  replace the old storage hint under the ROM.
- [x] `app.js`: the `storageAsk` preference, the choice wired to both
  events, `window.saturnus.storageChoice` for checks.
- [x] Styles: the notice shares the update notice's card; the outcome on
  one line beside OK.
- [x] Node tests (`web/test/pwa.test.mjs`): no `persist()` on load; the
  notice after a ROM is kept; Keep it calls `persist()` synchronously and
  shows the answer; Not now remembered; nothing in the desktop app or
  without `persist()`.
- [x] Browser checks in `web/test/overflow.test.mjs` (headless Chrome):
  no `persist()` on load, the notice inside the viewport at 390 px, Keep
  it, Not now remembered, the ROMs panel's button; the notice and the
  panel's state line in the overflow sweep at every width.
- [x] Screenshots at 390 px, light and dark, with a real ROM.
- [x] `web/README.md`, the decision log.
- [x] `just gates`.

## Acceptance criteria

- [x] No `persist()` call at page load (Node test and headless Chrome).
- [x] The notice appears after a ROM is kept, only when storage is not
  already persistent, and "Keep it" calls `persist()` inside the click.
- [x] "Not now" is remembered across loads; the ROMs panel still offers
  "Keep permanently".
- [x] Nothing of this in the desktop app or without `persist()`.
- [x] No horizontal overflow at 360 to 1280 px with the notice or the
  state line shown.
- [ ] The owner checks by hand on Firefox for Android: no prompt on load;
  after choosing a ROM the notice, then Firefox's prompt on "Keep it",
  and the outcome line matches the answer.

## Outcome

Done as designed. The page no longer calls `persist()` by itself: it
reads `persisted()` at load and asks only after the user keeps a ROM,
or from the ROMs panel. An update notice, if one is up at the same time,
covers the storage notice (z-index 6 under 7): the update is the more
urgent choice. Screenshots at 390 px were checked in headless Chrome
(the ask, the outcome, the panel's state line and button, light and
dark). The Firefox for Android check is the owner's.
