---
type: iteration
title: "Iteration 53: models and ROMs on the home page"
date: 2026-10-17
status: completed
tags:
  - iteration
  - web
  - desktop
branch: iter-53/roms-on-home
---

# Iteration 53: models and ROMs on the home page

Owner, 2026-10-17: the Calculator panel's Model and ROM section ("Model",
"ROM … Change…", "ROMs of every model (n of 12)", "Start the last model
when the page opens") belongs on the home page. To switch the calculator,
you go to the home page.

So the home page ([[iterations/iteration-50d-one-document]]) becomes the one
place where calculators are chosen, started, given a ROM and removed; the
Calculator view shows one calculator and links back home.

## Design

- Home lists the calculators with a kept ROM, as now (Files left, Calculator
  right). Each row gets a ⋯ menu: Change ROM…, Remove (what the ROM table's
  row menu does today, `web/romrows.js`).
- "New calculator" opens, on the home page itself, the models without a ROM:
  per model Choose… and, on the web, the Download link (with the
  `SOURCE_HINTS` text; on desktop the app's Download…). Choosing a ROM keeps
  it and opens that calculator (`#/calculator/<model>`). The ROM notices and
  offers (`romNotice`: a file of another model, several ROMs at once) show
  on the home page, where the ROM was chosen.
- On the home page too: "Start the last calculator when the app opens"
  (the `boot-last` pref, now deciding whether the app starts on `#/` or on
  the last calculator's route), the storage state with Keep permanently,
  and Remove ROMs….
- The Calculator panel loses the Model select and the whole ROM section. Its
  first group shows the running calculator's model and ROM name (read only)
  and a "Switch calculator" link to `#/`. Without a running calculator
  (`#/calculator` with no model, or a model with no ROM) the view says so
  and links home.
- The ROM operations go through the hub (web: `HubClient` `romSlots`,
  `chooseRom`, `forgetRom`, …, `web/hub.js` `ROM_COMMANDS`; desktop:
  `web/native-hub.js`), so the home page does not need the Calculator view
  mounted. Choosing a ROM for a model must not stop another running
  calculator.

## Tasks

- [x] Home: row ⋯ menu (Change ROM…, Remove), "New calculator" section with the models without a ROM, ROM notices and offers, storage state, Remove ROMs…, the start-last-calculator checkbox
- [x] Start route: the app opens on the last calculator only when the checkbox is on, else on `#/`
- [x] Calculator panel: Model select and ROM section removed; model and ROM shown read only with "Switch calculator"; the no-calculator state links home
- [x] Desktop: the same on the Tauri app (Choose… opens the file dialog, Download… fetches)
- [x] Tour, shortcuts, help texts and `web/README.md` follow (the tour's `roms` step moves or goes)
- [x] Tests: the affected e2e (`nav-page`, the ROM table tests) moved to the home page; dead code of the old section removed
- [x] `hyalo lint` clean

## Owner checks

- [ ] On the web: New calculator → Choose… a ROM → the calculator starts; ⋯ Change ROM… and Remove work on the home page; the Calculator panel has no ROM section
- [ ] On the desktop app: the same
