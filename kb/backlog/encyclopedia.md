---
title: "HP calculator encyclopedia from the wiki, on ractive.ch/saturnus"
type: backlog
date: 2026-10-20
status: in-progress
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

- 2026-10-21: the wiki is gathered material, not a published product: it stays as it is,
  ROM excerpts (instruction sequences, the 8-byte dump, routine walk-throughs) included,
  and needs no clean-up pass. The encyclopedia is written for readers from the wiki's
  facts and takes no ROM code. The fix pass below is dropped.

- 2026-10-21, form and URL (owner):
  - For both audiences: enthusiasts and collectors (models, history, moving files to a
    real calculator) and developers (how the Saturn works).
  - Under `/saturnus/docs/`, in strictly separate sections:

    | Section | URL | Source |
    |---|---|---|
    | Models (12) | `/saturnus/docs/48gx` | written from the wiki, the owner's photos |
    | Command reference | `/saturnus/docs/commands` (one dynamic page) | `web/commands.json` |
    | How the Saturn works | `/saturnus/docs/saturn/cpu`, `…/memory`, `…/display`, … | written from the wiki |
    | Moving files to a real calculator | `/saturnus/docs/transfer/cables`, `…/kermit`, `…/satx`, … | written from the wiki and the satx docs |

  - Every page also as Markdown beside its HTML (`/saturnus/docs/48gx` and
    `/saturnus/docs/48gx.md`), and `/saturnus/llms.txt` in the llmstxt.org format
    (title, summary, one link list per section; one command index per model rather than
    every command).
  - Sources are Markdown files in this repository; the site build renders HTML and
    publishes the Markdown as is.
  - The command reference is one dynamic page, not a page per command (owner: few
    people search for single commands): it reads `web/commands.json` in the browser,
    with a model picker, search, and a link per command through the address
    (`/saturnus/docs/commands#48gx/hms-`; readable slugs for names like `→LIST`, `%CH`,
    `∫`). It is also the app's ROM-free reference: the home page links to it. For LLMs,
    the build generates one Markdown file per model with all its commands and examples
    (`/saturnus/docs/commands/48gx.md`, 12 files), and `llms.txt` lists those.
  - No "Run it" links: the examples show input, the stack before and the display
    after, as the emulator gave them. Whoever runs a calculator uses Search (Cmd-K)
    there.
  - The command reference page shares the app's code: `web/reference.js` (lookup),
    `web/components/entry-view.js` (one command) and `web/commands.json`; read-only on
    the docs page, with "run" only in the app.
  - Tone: human, plain technical English as in technical documentation, no walls of
    text, no esoteric words. Model pages longer than the short ones on `/saturnus`, with
    history and anecdotes. A sample (48GX) settles tone and look before the others.
  - Look: the app's style; a model page takes that calculator's colours (as the logo and
    the model cards); section pages keep the neutral colours.
  - Build: a small script in the site build, no static site generator.
  - Sources: each page lists the online sources it used (HP's manuals, other sites),
    linked; no links into the wiki.
  - The app links to the docs from the home page, the command palette and the About
    panel. English only.
  - The encyclopedia takes no ROM code.

## Tasks

- [-] Fix pass on the wiki (dropped by the owner, 2026-10-21: the wiki stays gathered material)
- [x] Decide form, structure and URL of the encyclopedia
- [ ] Site build: Markdown sources → HTML plus the `.md` beside it, `/saturnus/llms.txt`, a docs index, navigation, the site's style; deployed with pages.yml
- [ ] Command reference: one dynamic page from `web/commands.json` (model picker, search, links per command), linked from the app's home page; one generated `.md` per model for `llms.txt`
- [ ] Sample model page (48GX), reviewed by the owner for tone and look
- [ ] The other model pages (11): history, specifications, what is special, how saturnus runs it, photos (no ROM section: the app's home page says where to get one, owner 2026-10-22)
- [ ] How the Saturn works: CPU, memory, display, keyboard, timers, interrupts
- [ ] Moving files to a real calculator: cables, Kermit, XModem, satx, Files
- [ ] Owner review of the written pages
