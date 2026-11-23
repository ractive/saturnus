---
title: "AI tools: the display as text on the key-driven models"
type: backlog
date: 2026-11-24
status: planned
priority: medium
tags:
  - backlog
  - saturnus
  - mcp
---

# AI tools: the display as text on the key-driven models

On the 42S, 27S, 17BII, 17B, 19BII, 38G, 39G and 40G, `read_display`,
`press_keys` and `run_function` give as text only the annunciators; what
the display shows (x and y on the 42S, a result on the 17BII, HOME's
history on the 38G) comes only as the image. Seen on 2026-11-24 through
the tab-mcp extension: `run_function "SIN"` on the 42S returned
`{"ran":"SIN","keys":"SIN","annunciators":[]}`, and `x: 0.5000` was
there only in the PNG. An AI app that does not read images, or keeps its
context small (WebMCP's default is text), cannot see a result. On the
48SX, 48GX and 49G the stack is read from memory, so they have it.

## Idea

Read the LCD's dots as characters. Each of these displays draws its text
in a fixed dot-matrix font, so a glyph table can be made on the emulator
itself: show known characters (the ROM's own character set, typed or
listed in ALPHA), record each one's dot pattern per model, then match
the display's rows against the table. The result goes into the tools'
text as `display` (lines as shown), e.g. `"y: 0.0000\nx: 0.5000"`, with
inverse menu labels and unknown cells marked rather than guessed.

- One mechanism for all eight models: one glyph table per display
  family (42S; the Pioneers 27S, 17B, 17BII, 19BII; the 38G, 39G, 40G).
- No memory layout needed, so no clean-room question: the patterns are
  the emulator's own output.
- Gives what is shown, rounded as shown (`0.5000` with FIX 4). Exact
  values need [[backlog/ai-42s-stack-from-memory]].
- Check the cost per call (the frame is already there for the PNG).

Done when the tools' text on each key-driven model carries `display` for
the stack screen, menus and messages, tested against frames with known
content per model, and the instructions and descriptions say so.
