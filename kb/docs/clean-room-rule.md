---
title: Clean-room rule
type: docs
date: 2026-10-04
status: active
tags:
  - licensing
  - saturnus
---

# Clean-room rule

Non-negotiable. Existing emulators (Emu48, Emu42, jsEmu48, x48, x48ng,
x50ng, ui4x, saturnng, HP EMU) are mostly GPL community projects whose
authors would not welcome an AI-written clone.

Their source code is off limits to anyone implementing saturnus, human or
agent: no file of it is opened while working on this project, not even to
look up a fact. Copies kept locally (`~/devel/hp-emulator-refs/`) may be
opened only by a designated reviewer comparing saturnus against them for
independence (as in iteration 23), never by an implementing session.

Facts come from what the projects publish as documentation: manuals, KML
and skin documentation, change logs (Emu48's `CHANGES.TXT`), forum posts
by their authors; and from black-box runs (saturnng as an oracle: same ROM,
same keys, outputs compared). Each fact goes into the wiki
(calculator-knowledgebase, `~/devel/calculator-knowledgebase/`, page
`emulators/<name>` or the hardware page) with its citation, and saturnus
is implemented from the wiki. Never mirror another emulator's structure,
never copy a table. Facts about a chip are not copyrightable; their
expression is.

ROMs are never committed or shipped. `saturnus rom fetch` downloads from
hpcalc.org after a confirmation prompt, identifying as curl or Wget (the site
serves a gzip bomb to fake browser user agents) and verifies size and
checksum. No HP logos or wordmarks in any UI chrome; "emulates the HP 48SX"
in text is fine.
