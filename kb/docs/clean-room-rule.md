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

Non-negotiable. Existing emulators (Emu48, x48ng, saturnng, HP EMU) are GPL
community projects whose authors would not welcome an AI-written clone. Their
sources are at `~/devel/hp-emulator-refs/` for one purpose: learning hardware
facts.

Procedure: read to learn a fact, write the fact with a citation into the wiki
(`~/devel/hp-literature/`, page `emulators/<name>`), close the file, implement
from the wiki. Never have their source open while writing ours, never mirror
their structure, never copy a table. Facts about a chip are not copyrightable;
their expression is.

ROMs are never committed or shipped. `saturnus rom fetch` downloads from
hpcalc.org after a confirmation prompt, identifying as curl or Wget (the site
serves a gzip bomb to fake browser user agents) and verifies size and
checksum. No HP logos or wordmarks in any UI chrome; "emulates the HP 48SX"
in text is fine.
