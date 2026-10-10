---
type: iteration
title: "Iteration 36d: the saturnus site, landing page and emulator under /emulator/"
date: 2026-10-10
status: planned
tags:
  - iteration
  - saturnus
  - web
  - consolidation
branch: iter-36d/site
---

# Iteration 36d: the saturnus site

The third consolidation iteration ([[decision-log]], "2026-10-10
(consolidation)"). It comes before 36c: the site has to ship before the crates
are deleted. The install lines are added in 36c, once 0.2.0 is out.

| URL | What | Source |
|---|---|---|
| `https://ractive.ch/saturnus/` | landing page: what saturnus is, screenshots, links to the emulator, the desktop downloads, the knowledge base and the source, "Not affiliated with HP" | `web/landing/` (static HTML and CSS, no JS) |
| `https://ractive.ch/saturnus/emulator/` | the emulator | `web/`, unchanged inside (all its URLs are relative) |
| `https://ractive.ch/saturnus/transfer/` | saturnus tx, later | PR #103 |

No redirects, and no detection of installed apps.

## The service workers

- **The emulator's worker** registers `sw.js` with the scope `./`. Under the
  new URL that scope is `/saturnus/emulator/`, and its caches are named
  `saturnus:/saturnus/emulator/:<build>`. Neither `pwa.js` nor `sw.js`
  changes.
- **The old worker** is scoped to `/saturnus/`, and a 404 on its script does
  not unregister it. Left alone, it would keep serving the old emulator at
  `/saturnus/`.
- **The kill switch** at `/saturnus/sw.js` replaces it: about 15 lines with no
  fetch handler. On install it calls `skipWaiting()`. On activate it:
  - deletes the caches whose names start with `saturnus:/saturnus/:` (with
    the trailing colon, so the emulator's caches survive);
  - calls `self.registration.unregister()`;
  - calls `navigate(url)` on the window clients it controls.
- **Kept ROMs and states** live in the origin's IndexedDB. The move does not
  touch them.
- **The kill switch is removed on 2027-01-15.**

## Tasks

- [ ] `web/landing/`: `index.html`, CSS, 2–4 screenshots (the screenshot harness), the favicon. No HP logos. The footer says "Not affiliated with HP"
- [ ] `web/landing/sw.js`, the kill switch as above, shipped at `/saturnus/sw.js`. It is not in the emulator's FILES and not part of its BUILD hash
- [ ] Kill switch tests (fake `caches` and `clients`): only the `saturnus:/saturnus/:` caches go, unregister is called, and only controlled windows are navigated
- [ ] `web/test/storage-names.test.mjs`: the emulator's IndexedDB databases, localStorage keys and cache prefix, from one module, disjoint from those of every other page on the origin. PR #103 adds tx's names
- [ ] `web/site.sh <out>` builds all of `/saturnus/`:
  - `.htaccess` at the top, no redirect rules;
  - the landing page and `sw.js`;
  - `emulator/` with its own marker, manifest, icons, `sw.js` and a BUILD hash over `emulator/` only;
  - the import check run once per subsite;
  - `--list` stays the emulator's list, so the desktop app's `build.rs` and the `frontend` test do not change
- [ ] `web/test/site.test.mjs` and `pwa.test.mjs` for the new layout. A landing-page change leaves the emulator's BUILD unchanged
- [ ] `pages.yml`: one artifact and one FTPS deploy to `httpdocs/saturnus/`, as today
- [ ] Docs: README.md's "Web page" link points at `/saturnus/emulator/`. kb/docs/releasing.md, CHANGELOG
- [ ] Backlog item: remove the kill switch on 2027-01-15 (take it out of `site.sh`; the next deploy deletes `/saturnus/sw.js` from the server)
- [ ] `just gates` and `just web-test`, then /create-pr, /review-pr, /merge-pr
- [ ] Fix the wrong claim that the FTP deploy only adds and overwrites, in the `pages.yml` comment and in kb/docs/releasing.md ("Web page"). In fact, SamKirkland/FTP-Deploy-Action v4.4.0 keeps `.ftp-deploy-sync-state.json` in `httpdocs/saturnus/` and syncs against it. A file it deployed earlier that is no longer in `site/` is deleted on the next run (its log reads "Uploading … Deleting … Replacing"), and its `exclude` patterns are left out of both publishing and deleting
- [ ] Owner: run `pages.yml`. The old emulator files at the top of `httpdocs/saturnus/` (including `about.json`) are deleted by the deploy itself, because they are no longer in `site/`. `sw.js` is in the new tree (the kill switch), so it is replaced, not deleted
- [ ] Owner, once: list `httpdocs/saturnus/` on the server after that deploy. It holds only `.htaccess`, `.ftp-deploy-sync-state.json`, `index.html`, the landing files, `sw.js` and `emulator/`. Anything else never went through the action, so the sync state does not know it (for example a leftover of the first copy, ractive.ch PR 8). Delete it by hand
- [ ] Owner: on a browser that had the old page installed, open `/saturnus/` once. It shows the landing page, `/saturnus/emulator/` lists the kept ROMs, and exactly one worker is left, scoped to `/saturnus/emulator/`. Install the web app again from `/saturnus/emulator/`

## Acceptance

- The landing page is at `/saturnus/` and the emulator at
  `/saturnus/emulator/`, with the kept ROMs intact.
- The old worker is gone after one visit.
- The desktop app is unchanged.
