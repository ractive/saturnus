---
type: iteration
title: "Iteration 12d: The 49G's system flags from the Pocket Guide"
date: 2026-10-06
status: planned
tags:
  - iteration
  - saturnus
branch: iter-12d/49g-flags
---

# Iteration 12d: The 49G's system flags from the Pocket Guide

The 49G's Advanced User's Guide refers to the HP 49G Pocket Guide for
the list of system flags. On 2026-10-06 the owner photographed the
booklet's "System Flags" section (pages 76 to 79: flags -1 to -26, -27
to -61, -62 to -96, -97 to -120; the list ends at -120). The images are
in the literature library at `raw/manuals/hp49g-pocket-guide/` with a
README; they are the citation. The earlier fallback (reading the
calculator's own MODE > FLAGS browser) is no longer needed.

Read first: wiki `hardware/system-flags-49g` and
`questions/hp49g-system-flags`, `hardware/system-flags-48gx` (the form
the pages take), `scripts/flags-json.py`, `web/flags.json`,
`web/components/sat-explorer.js` (the flags panel and its basis line).

## Design

- Transcribe every listed flag from the four page images into the wiki
  page `hardware/system-flags-49g` as facts in our own words: flag
  number, the meaning when set, when clear, the default (the booklet
  marks the default state with a bullet), grouped by topic as the 48GX
  page is; cite "HP 49G Pocket Guide, p. 76-79 (owner's copy,
  photographs in raw/manuals/hp49g-pocket-guide/)". Where the page
  photo is hard to read, say so on the page rather than guess (a
  question entry), and the owner can check the booklet.
- Flags the booklet does not list (gaps in the numbering such as -4,
  -13, -30, -33, -34, -56, -75, -77, -78, -101, -102, -104, -107, -108,
  -112, -115, -118, and -121 to -128) are recorded as "not listed in the
  Pocket Guide"; the guide's own words elsewhere (-110 LARGE MATRICES in
  the AUG) may fill some.
- A new source page `sources/hp49g-pocket-guide` in the wiki; the
  question page is answered; the index and log updated.
- `scripts/flags-json.py` regenerates `web/flags.json`; the panel's
  basis line for the 49G names the Pocket Guide; no "unknown" wording
  for listed flags remains.

## Tasks

- [ ] Transcription of the four pages into the wiki page, with defaults
  and groups; the source page; the question page closed.
- [ ] `web/flags.json` regenerated; the panel's basis line.
- [ ] Verification in headless Chrome on the 49G: the flags panel shows
  the meanings; setting -40 and -117 by keys updates the rows.

## Acceptance criteria

- [ ] Every flag the Pocket Guide lists has its set and clear meaning in
  the panel; the unlisted ones are marked as such.
- [ ] `just gates` passes; `hyalo lint` clean in the kb and the wiki.

## Outcome

(to be written)
