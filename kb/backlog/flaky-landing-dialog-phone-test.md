---
type: backlog
title: The landing page's phone dialog test is flaky under load
date: 2026-10-28
status: planned
priority: low
tags:
  - backlog
  - web
---

# The landing page's phone dialog test is flaky under load

Seen 2026-10-27 in the full local web suite (`just web-test`'s set) for
PR #251: `web/test/landing-dialogs-page.test.mjs`, "each model card opens
its dialog; Esc, a click outside and × close it", measured the 28S
dialog's bottom at 850 instead of 844 at the emulated 390 × 844 phone.
The same test passed alone twice on that branch and once on main, so it
is timing, not the layout: the test measures after a fixed
`sleep(300)`, while the sheet may still be sliding in under load.

## Options

- Wait until the dialog's rectangle stops changing (two equal reads a
  frame apart), or until its animations have finished
  (`d.getAnimations().every(a => a.playState === "finished")`), instead
  of the fixed 300 ms.
- Turn the animation off in the test (`prefers-reduced-motion` emulation,
  if the landing page honours it).
