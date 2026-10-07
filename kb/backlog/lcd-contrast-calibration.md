---
title: "LCD contrast: the ROM's default renders properly dark on every model"
type: backlog
date: 2026-10-07
status: completed
priority: medium
tags:
  - backlog
  - saturnus
---

# LCD contrast: the ROM's default renders properly dark on every model

Owner (2026-10-07): "Can it be that the screen of the 49G has less
contrast on purpose?"

`web/components/sat-calculator.js` `darkness()` maps the contrast register
linearly across the model's ON+/ON- range (`Model::contrast_range`: 3-19
on the 48SX, 9-24 on the 48GX/38G/49G/39G/40G, 15-31 on the 42S) to 30-100 %
pixel darkness. The 49G ROM's power-on contrast sits low in its range, so
the screen comes up washed out, unlike the real calculator.

Fix: per model, record the ROM's power-on contrast value (observe it on
each ROM after a cold start; write it to the wiki's model pages) and map
so that value renders at about 90 % darkness, with the fade concentrated
in the lowest part of the range (a curve, not a line), as a real LCD
behaves. The desktop app uses the same component. Test: the darkness at
each model's default is at least 0.85; ON+ and ON- still change it.

Also (owner, 2026-10-07): ON held with + is not reachable on every
keyboard layout (the backtick is a dead key behind Shift on Swiss German;
Esc is taken by the browser in fullscreen) nor on a trackpad or phone.
Add "Darker display" and "Lighter display" as palette actions and as a
small control in the side panel; each sends the ON + / ON - sequence
(press ON, tap + or -, release ON) through the key queue.

## Outcome (2026-10-07)

Power-on contrast after a cold start: 48SX 11, 48GX, 38G and 49G 14, 39G
(and 40G) 12, 42S 22, recorded on the wiki's model pages and as
`Model::default_contrast`, sent in each frame as `contrastDefault`.
`web/contrast.js` draws it at 0.9 darkness; below it a square root fades
to 0.15 at the range's low end, above it the pixels reach 1 and the unlit
pixels grey. "Darker display" and "Lighter display" (palette, and a
Display control under Speed in the panel) send ON + / ON - with keyDown
and keyUp, releasing ON after the machine let go of + or -. Checked in
headless Chrome on the 49G (14 → 0.53 before, 0.9 after) and the 48SX
(11 → 0.65 before, 0.9 after), with the buttons stepping the register.
