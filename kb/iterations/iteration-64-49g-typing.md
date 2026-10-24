---
type: iteration
title: "Iteration 64: the 49G types right after a boot and takes long programs"
date: 2026-10-24
status: planned
tags:
  - iteration
  - saturnus-host
  - ai
branch: iter-64/49g-typing
---

# Iteration 64: the 49G types right after a boot and takes long programs

Found by the modern-UI study (branch `explore/modern-ui`,
`kb/research/modern-ui-on-the-rom.md` section 2, 2026-10-18), never tracked,
not fixed since. Both hit the AI tools' `run_rpl` and the palette on the 49G.
Owner 2026-10-24: start it.

## The two problems

1. **A send right after a cold boot fails.** A `run` within about 3 seconds of
   a cold boot fails with "could not bring the keyboard to AlphaLocked (alpha
   false, alpha lock false, …)", and the next send fails the same way until a
   pause. Waiting for `status.loop === "sleep"` was not enough; 3 seconds was.
2. **A long program comes out wrong.** A 130-character
   `« … → a b n « … FOR … NEXT n →LIST … » » EVAL` sent in one `run` works on
   the 48GX; on the 49G the values land on the stack but so do stray objects
   (`*` as a command object, a nameless ROM object) and the line is left open in
   program entry mode. Suspects: the 49G's program-entry mode after `«`, or its
   `'` pair insertion inside the program.

## Design

- Reproduce both on today's main with ROM 2.10 and 1.19-6, as ROM-gated tests
  through the host (`crates/saturnus-host`, the typing engine
  `src/typing.rs`). Record what was observed; one that no longer reproduces is
  written down as such and gets its test anyway.
- Fix in the typing engine, not in the callers: the engine waits (in emulated
  time) until the keyboard answers, or refuses with a clear "the calculator is
  still starting" the callers can retry on; it types any program text the
  48GX takes so that the 49G reads it back the same.
- New facts about the 49G's editor go to the wiki (`hardware/command-line`).

## Tasks

- [ ] Reproduce both (ROM-gated tests, both 49G ROMs)
- [ ] Fix the send after a boot
- [ ] Fix the long program send
- [ ] Wiki findings, decision-log entry
- [ ] Owner: on a freshly started 49G, an AI app's first run_rpl with a program of several lines works
