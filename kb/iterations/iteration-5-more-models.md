---
title: "Iteration 5: More models"
type: iteration
date: 2026-10-04
status: planned
branch: iter-5/more-models
tags:
  - iteration
  - saturnus
---

# Iteration 5: More models

## Tasks

- [ ] 48GX: 128 KB RAM, bank-switched port 2 with the byte-read latch quirk,
  DA19 polarity (wiki settled it against Mastracci and Voyage).
- [ ] 49G: 512 KB RAM, 2 MB flash with banking and the write-enable path
  (`questions/hp49g-bank-latch-bits`, `hp49g-flash-write` to resolve by
  experiment), different keyboard.
- [ ] 38G, 39G/40G: wiki pages are stubs; sources needed first (HP Journal 38G
  article is in raw/, not yet ingested).

## Acceptance criteria

- [ ] Acceptance per model: boot, differential screens, hptx e2e.
