---
title: "satx on real hardware"
type: backlog
date: 2026-10-10
status: planned
priority: medium
tags:
  - backlog
  - satx
---

# satx on real hardware

satx's iteration 6 ([[satx/iterations/iteration-6-real-hardware]]), moved
here when satx joined this repository. A manual checklist for when a
USB-serial adapter is at hand: the 48SX and the 49G, every CLI command,
both protocols on the 49G, timing on long transfers, and the early 49G's
DTR/RTS behaviour. Every surprise goes into the wiki.

- [ ] Verify which prolog a real 39G/40G aplet uses (#026D5 is the Conn4x
  guess) and that the object walk matches it, and whether the 39G/40G wire
  protocol is Kermit or XModem; record both in the wiki. Needs a 39G or
  40G.
- [ ] XSERV on a 49g+/50g (`satx xserv` is implemented from the wiki only
  and unverified). Needs a 49g+ or 50g.
