---
type: iteration
title: "Iteration 12d: The 49G's system flags from the Pocket Guide"
date: 2026-10-06
status: completed
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

- [x] Transcription of the four pages into the wiki page, with defaults
  and groups; the source page; the question page closed.
- [x] `web/flags.json` regenerated; the panel's basis line.
- [x] Verification in headless Chrome on the 49G: the flags panel shows
  the meanings; setting -40 and -117 by keys updates the rows.

## Acceptance criteria

- [x] Every flag the Pocket Guide lists has its set and clear meaning in
  the panel; the unlisted ones are marked as such.
- [x] `just gates` passes; `hyalo lint` clean in the kb and the wiki.

## Outcome

- Counts: 103 flags listed and transcribed (91 table rows, ranges for
  the multi-flag fields -5..-10, -11..-12, -15..-16, -17..-18, -45..-48,
  -49..-50); 25 not listed (-4, -13, -30, -33, -34, -56, -75, -77, -78,
  -101, -102, -104, -107, -108, -112, -115, -118, -121 to -128), marked
  "Not listed in the Pocket Guide", status `unknown`; 0 unreadable. All
  four photographs were legible, so no question entry for a hard-to-read
  photo was needed.
- Defaults: every listed flag defaults to clear except -5..-10 (set,
  word size 64). The booklet's -25 entry names itself where -21 is meant,
  and its -90 default (clear) disagrees with the AUG p. 1-4 (set); both
  under Contradictions on the wiki page. -110 is stated in the User's
  Manual p. 8-12 (not the AUG); it agrees.
- Found in the browser check: ROM 2.10's cold start sets -5..-10, -11,
  -12, -17, -27, -34, -90, -95 and -128, so it starts in HEX, radians and
  algebraic mode, against the booklet's defaults, and uses two unlisted
  flags. Recorded on the wiki page ("Observed") and in the question,
  which is narrowed (still open) to the unlisted flags, the -90 default
  and the terse entries (-71, -86, -89, -93, -94, -120).
- Wiki: `hardware/system-flags-49g` rewritten (stub to draft), new
  `sources/hp49g-pocket-guide`, `questions/hp49g-system-flags` narrowed,
  `sources/hp49g-aug`, `index.md`, `log.md` and `raw/README.md` updated.
- Repo: the 49G basis line in `scripts/flags-json.py` names the Pocket
  Guide; `web/flags.json` regenerated (49G: 105 entries, 103 known, 25
  unknown); `--check` passes. No panel code changed: the unlisted flags
  fall into its "Without a documented meaning" grid.
- Verification: `just web-test` and `just gates` (with
  `SATURNUS_ROM_DIR`) pass; `hyalo lint` clean in the kb and the wiki.
  Headless Chrome on the 49G (port 4886, stopped afterwards): header "14
  of 128 system flags set" at the start, Flags tab 15 after; 91 described
  rows, none without a meaning, the 25 unlisted flags in the grid; MODE,
  +/-, OK switched to RPN and the -95 row followed; `40 +/- SF` and
  `117 +/- SF` by keys turned the -40 and -117 rows on within 23 ms each,
  and the calculator shows its clock.
