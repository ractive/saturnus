---
title: Open hardware questions
type: docs
date: 2026-10-04
status: active
tags:
  - hardware
  - questions
  - saturnus
---

# Open hardware questions

From the wiki's `questions/` (open as of 2026-10-04):
`dec-mode-constant-bug`, `bus-priority-ce1-ce2`, `config-check-in-handler`,
`hp49g-bank-latch-bits`, `hp49g-flash-write`, `interrupt-maskability`,
`register-10e-role`, `display-start-address-taplin`, `contrast-range-48gx`,
`binary-odd-nibble-padding`. Each resolution goes back into the wiki with the
experiment that settled it.

Added 2026-10-05 (iteration 4): `instruction-speed-vs-hardware`. The
emulated speed of User RPL (SASM cycle counts at 2 MHz plus the 13% stall)
has no oracle; saturnng is not one. The experiment is a `TICKS 1 2000 START
NEXT TICKS SWAP -` run on a real 48SX; the owner has the hardware.

Updated 2026-10-05: `instruction-speed-vs-hardware` has an online oracle
now (HP Museum summation benchmark: saturnus is 16% fast on the SX, 35%
on the GX); calibration is iteration 7. The 39G/40G alpha letters came
from HP's user's guide figure and are no longer open.

