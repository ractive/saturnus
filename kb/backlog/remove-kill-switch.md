---
title: "Remove the kill switch at /saturnus/sw.js on 2027-01-15"
type: backlog
date: 2026-10-11
status: planned
priority: low
tags:
  - backlog
  - web
---

# Remove the kill switch at /saturnus/sw.js on 2027-01-15

[[iterations/iteration-36d-site]] moved the emulator from `/saturnus/` to
`/saturnus/emulator/`. The service worker the old page registered is scoped
to `/saturnus/`, and a 404 on its script would not unregister it, so
`web/landing/sw.js` is shipped at `/saturnus/sw.js` in its place: it deletes
the old caches (`saturnus:/saturnus/:*`), unregisters itself and reloads the
pages it controls. By 2027-01-15 (the due date) every browser that visits
again has run it.

- [ ] Delete `web/landing/sw.js` and `web/test/killswitch.test.mjs`, and
  the lines of `web/site.sh` that copy it (the site's "kill switch" step)
- [ ] Drop the kill switch's prefix from `web/test/storage-names.test.mjs`
- [ ] Run `pages.yml`: the deploy deletes `/saturnus/sw.js` from the server,
  as the file is no longer in `site/` (the action syncs against
  `.ftp-deploy-sync-state.json`)
