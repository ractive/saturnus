---
title: "HP calculator encyclopedia from the wiki, on ractive.ch/saturnus"
type: backlog
date: 2026-10-20
status: planned
priority: medium
tags:
  - backlog
  - wiki
  - site
---

# HP calculator encyclopedia

A human-readable encyclopedia of the HP calculators, built from the public wiki
calculator-knowledgebase and published under <https://ractive.ch/saturnus/> (owner,
2026-10-20; the path below `/saturnus/` is open).

## State (2026-10-20)

The audit that had to come first is done: a Claude Managed Agent read the whole wiki at
commit 9db1840 against its clean-room rule. Setup, rerun steps and the report are in
`tools/wiki-audit/` (report: `tools/wiki-audit/audit-2026-10-20/audit.md` and
`findings.json`). 97 findings, 13 of high confidence; verdict: not ready for
republishing.

What blocks it:

1. ROM instruction listings and dumps on wiki pages (hardware/timers.md:115,
   questions/hp39g-40g-model-detection.md:35, an 8-byte dump in
   hardware/memory-controller.md, routine walk-throughs by address).
2. The owner's own photographs (38G, 39g+, 42S, the printed 49G Pocket Guide) cited as
   sources, and about twenty mentions of unreleased ROM images the owner holds. The wiki's
   CLAUDE.md forbids private material.
3. wiki/overview.md:21 says GPL emulator sources are "read for facts only", against the
   wiki's rule that they are never opened; the Conn4x pages name a routine and quote code
   comments.
4. Copied tables: the three system-flag pages, Intel datasheet tables, Mastracci's tables
   (Mastracci's terms say "no changes"), tables from the Saturn tutorial and Horn's Kermit notes.
5. 25 entries in sources/manifest.json have terms "not checked".

No images or other media in the wiki, no e-mail addresses, no local paths.

## Owner decisions

- 2026-10-20: the owner's own photographs may stay as sources. The wiki's CLAUDE.md says
  so since then; the audit's photograph findings (category 5) need no fix.
- 2026-10-20: the wiki no longer says who holds the ROM images it used (wiki commit
  290e7dd); the git history still does.

- 2026-10-21: copied tables: the rule is relaxed (wiki commit 67ffba3). Tables of plain
  facts may follow a source's table with the source cited. Rewritten in our own structure
  and words: Mastracci's tables (saturn-cpu, uart, timers, io-ram; his terms say "no
  changes"), the Emu42 PROBLEMS.TXT list (sources/emu42-problems, GPL) and the 49G Pocket
  Guide's flag list transcribed verbatim (system-flags-49g). The other category 3
  findings need no fix.

Open:

- Form and URL of the encyclopedia below `/saturnus/`.

## Tasks

- [ ] Fix pass on the wiki in the order the audit suggests: listings, dumps and private
  material; overview.md and the Conn4x pages; the tables named above;
  the source terms. As wiki PRs, then rerun the audit (`tools/wiki-audit/run.sh`)
- [ ] Decide form, structure and URL of the encyclopedia
- [ ] Build and publish it
