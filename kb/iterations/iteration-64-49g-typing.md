---
type: iteration
title: "Iteration 64: the 49G types right after a boot and takes long programs"
date: 2026-10-24
status: in-progress
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

## What was found (2026-10-24)

1. Reproduced on both 49G ROMs (and the 48GX, 48SX in their first
   ~400 ms): the send came while "Try To Recover Memory?" or the 49G's
   "Memory Clear" box still had the keyboard (the host answers them
   once they are up, about 3.4 s after a cold 49G boot); at 0 ms the
   1.19-6 RAM holds no command line at all. Fix: in the protocol
   engine, a send or write that comes while the question is pending
   waits in its turns (emulated time), answering the question itself,
   then runs; `status.starting` says so, the AI tools wait on it.
2. Not a typing fault: the text arrives exactly (it reads back as typed
   on all three ROMs). Evaluating the 146-point plot takes the 49G about
   38 s of emulated time (the 48GX about 8 s), longer than `run`'s 30 s
   wait; the reply said `running` and the stack read then showed the
   program's working objects. Fix: `run` waits up to 120 s; the AI
   tools no longer show the stack while a program runs.
3. Owner's ChatGPT test (2026-10-24, main before the fix): on a freshly
   started 49G a multi-line `« → n « … FOR … » » 'SSQ' STO …` typed in
   fully but gave "Invalid Syntax" at the FOR: the 49G boots in
   algebraic mode, which reads a program as algebraic. Line breaks and
   indentation do not matter (the same text in one line behaves the
   same). In RPN mode it is stored and runs as on the 48GX (ROM-gated
   test `the_owners_programs_need_rpn_mode_on_the_49g`). The AI tools
   already report `mode: "algebraic"` and tell the AI to run CF(-95);
   the test prompt forbade that workaround.

## Tasks

- [x] Reproduce both (ROM-gated tests, both 49G ROMs)
- [x] Fix the send after a boot
- [x] Fix the long program send
- [ ] (On hold, coordinator 2026-10-24: in its own commit, to keep or drop) run_rpl on a 49G in algebraic mode switches it to RPN first (CF -95, the echo dropped), says so (`switched`, `mode: "rpn"`), does not switch back; instructions and description say so; web tests and a ROM-gated test with the owner's text on both 49G ROMs (stored, `10 SSQ` → 385, `1 5 5 SQL` → { 1 4 9 16 25 })
- [x] Wiki findings, decision-log entry
- [ ] Owner: on a freshly started 49G, an AI app's first run_rpl with a program of several lines works
