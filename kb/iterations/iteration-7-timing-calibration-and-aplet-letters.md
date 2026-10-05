---
title: "Iteration 7: Timing calibration against published benchmarks, 39G/40G letters"
type: iteration
date: 2026-10-05
status: planned
branch: iter-7/timing-calibration-and-aplet-letters
tags:
  - iteration
  - saturnus
---

# Iteration 7: Timing calibration against published benchmarks, 39G/40G letters

Read first: wiki `questions/instruction-speed-vs-hardware` (the oracle table),
`sources/hpmuseum-summation-benchmark`, `hardware/saturn-cpu` ("Timing"),
`hardware/display` (refresh stall), `hardware/hp48gx` (clock), `emulators/emu48`
(G-series cycle counts, SP1), `hardware/hp39g-40g` ("Alpha letters").

## Context (2026-10-05)

- Real-hardware speed figures exist online: the HP Museum summation
  benchmark gives a 48SX at 95.5 s for n=1000 and a 48GX at 5.5 s (FOR/NEXT)
  or 5.9 s (sum function) for n=100, plus 49G ROM 2.10 figures. saturnus
  runs the same programs in 80.3 s (SX, scaled from n=100) and 3.62-3.72 s
  (GX): 16% and 35% too fast. The sums match digit for digit, so this is
  pure timing. Measured with `saturnus-mcp`'s `run_command` on the Kermit
  host-command path and TICKS, unpaced.
- Candidate causes, to be separated by experiment rather than fudged: SASM
  cycle counts (approximate, "+d" field forms) versus the ROM's instruction
  mix; the flat 13% display stall (the thread's Saturn-assembly post 165
  measured a 12% display cost on a G/GX, so the stall itself looks right);
  the Yorke clock (4 MHz nominal; the SX/GX ratio difference suggests the
  G series needs its own cycle counts, which Emu48 SP1 states).
- The 49G in-process Kermit host command (`Emulator::run_command`) timed
  out on both 49G ROMs while the TCP e2e suite passes on the 49G; find out
  why (server start, timing of the first packet, or a 49G-specific reply).
- The 39G/40G alpha letters are now in the wiki from HP's user's guide
  figure; `type_text` and the web keyboard can use them.

## Tasks

- [ ] A ROM-gated benchmark test per model that runs the summation program
  (n=100) through the Kermit host command and reports ticks; record the
  figures in the plan Outcome and the wiki question.
- [ ] Find the cause of the 48SX speed error (16%): instrument the
  executor to histogram executed instructions by cycle class over the
  benchmark, compare the SASM counts with any second source (Mastracci,
  Gariepy, the tutorial's Meta Kernel figures), test the display-stall
  model against the thread's 12% display-on/off measurement, and derive
  the correction with a stated cause. Apply it to `cpu/cycles.rs` or the
  stall model, not as a global factor, unless the evidence only supports
  a factor (then document it as calibration).
- [ ] Do the same for the 48GX (35%): a Yorke-specific cycle table or clock
  figure, with the cause stated; re-check the 49G against its 5.5 s figure
  once the in-process host command works.
- [ ] Fix the 49G `run_command` timeout in `saturnus-mcp`.
- [ ] 39G/40G alpha letters in `type_text` and in the web keyboard labels
  (wiki `hardware/hp39g-40g` "Alpha letters").
- [ ] Re-run the full differential suite and both hptx e2e paths after the
  timing change (Kermit pacing depends on it), and the clock-drift e2e.

## Acceptance criteria

- [ ] The benchmark test reproduces the real 48SX and 48GX times within 5%
  on both models, with the cause of each correction documented in the
  decision log and the wiki question answered or narrowed.
- [ ] `type_text "HELLO WORLD"` works on the 39G and 40G.
