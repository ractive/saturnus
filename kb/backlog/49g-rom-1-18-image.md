---
type: backlog
title: A 49G ROM 1.18 image for the emulator from HP's update file
date: 2026-11-08
status: planned
priority: low
tags:
  - backlog
  - roms
  - 49g
---

# A 49G ROM 1.18 image for the emulator from HP's update file

The owner's real 49G runs ROM 1.18 (2026-11-08) and answers XSERV; the
emulated 49G's ROM 2.15 has no XSERV. An emulated 1.18 would let the e2e tests and Files' fast transfer be checked
against the ROM the real calculator has
([[iterations/iteration-68a-aplet-folder-and-add-calculator]]).

HP's `hp49g118.zip` is in `roms/` (never committed). It holds a `.flash`
update file, not a whole-chip image, so an image has to be built from it.

## What is known (wiki: questions/custom-rom-for-saturn-calculators, section 3)

- The `.flash` container starts with the ASCII `HPROM` and the part count,
  7: System, Part1, Part2, Part3, Part4, Part6 and Part7 (no Part5, no FS).
- Per part: a length-prefixed name, a 32-bit little-endian length of the
  rest of the part, then (Part1 to Part7) a 32-bit field with the bank in
  its top nibble and the nibble count in the rest, then the nibbles packed
  two per byte. The System part's field does not follow that pattern.
- The parts' data does not start at the bank's offset 0 (no bank header at
  byte #100 of a part's data).
- The image also needs what the update file leaves out: the bank headers,
  the boot code in bytes 0-#FF and the CRCs, as the wiki's custom-ROM page
  describes them; HP's update files carry no boot sector.

## Next

- Work out where each part's data lands in its bank (the offset the part
  does not start at) and what the System part's field means, from the file's
  structure only, and write it back to the wiki.
- Build the image in a tool outside the core crate (a `tools/` script or an
  `xtask`), never committing the result; boot it in saturnus and record what
  differs from 2.15 in the wiki.
