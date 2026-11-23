---
title: "AI tools: read_stack for the 42S, from memory"
type: backlog
date: 2026-11-24
status: planned
priority: low
tags:
  - backlog
  - saturnus
  - mcp
  - 42s
---

# AI tools: read_stack for the 42S, from memory

`read_stack` exists for the 48SX, 48GX and 49G only: it reads their
stack from memory through the documented object formats. The 42S has no
such tool, so an AI app learns X, Y, Z and T only from the display, and
then rounded as shown (`0.5000` with FIX 4, where the register holds 0.5
to 12 digits). [[backlog/ai-display-text-key-models]] covers what is shown;
this item covers the exact values.

## How

No published memory map says where the 42S keeps its stack registers,
and other emulators' sources are off limits
([[docs/clean-room-rule]]). The facts can be found by observation, as the
42S's menu labels in RAM were (wiki: hardware/hp42s, "Menu labels in
RAM"): no ROM code read.

1. Put known values in X, Y, Z, T (1 2 3 4, then others, negative, large
   and small exponents) and compare RAM after each step: where the four
   registers are, and their order. LASTX and ALPHA the same way.
2. Their format: most likely the Saturn BCD real (sign, 12-digit
   mantissa, exponent) as on the 48 and 28S; check against the display.
3. Other types: complex numbers, ALPHA strings, matrices (probably a
   reference to a variable): find their marks, or report the type only.
4. Write the facts to the wiki (hardware/hp42s), then implement from it:
   `read_stack` for the 42S (`x`, `y`, `z`, `t`, `lastx`, `alpha`, each
   as text and type), its family's tool list, the instructions.

The other key-driven models (27S, 17B, 17BII, 19BII, 38G, 39G, 40G) each
need their own search; do them only when one is asked for.
