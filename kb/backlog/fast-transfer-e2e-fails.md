---
type: backlog
title: The fast transfer's end-to-end test fails on the emulated 48GX and 49G
date: 2026-11-05
status: completed
priority: high
tags:
  - backlog
  - web
  - transfer
---

# The fast transfer's end-to-end test fails on the emulated 48GX and 49G

Seen 2026-11-05 after the 0.10.3 release: both tests of
`web/test/tx-e2e-xmodem.test.mjs` fail at the fast get, line 148, after
about 190 s. The dialog says "Nothing came from the calculator in time. Was
XSEND typed and ENTER pressed?" instead of "PRG came over".

It is not load: it failed the same way alone, with a load average of 7. It
is not 0.10.3 either: `v0.10.2`, built in a worktree, fails the same way.
The last pass on record is 2026-10-17. CI has no ROMs, so only `just
web-test-full` runs this test, and nothing has caught it.

Not known yet: whether a person's fast transfer with an emulated calculator
fails too, or only the test's way of doing it ("Type it for me" through the
page's dialog).

Real hardware, 2026-11-08 (the owner, 0.10.3): the fast transfer works on a
real 49G (ROM 1.18), both in Files in the desktop app and with `satx`. So
the protocol and the dialog's steps are fine on a calculator; what fails is
the emulated path, "Type it for me" typing XSEND into the emulated
calculator ([[iterations/iteration-68a-aplet-folder-and-add-calculator]]).

## Next

- Bisect from 2026-10-17 to `v0.10.2` on the first-parent history, with
  `--test-name-pattern 48gx` and a `just web` per step, in a worktree.
- Try the same steps by hand on an emulated 48GX in Files.

## 2026-11-08: passes with the one-key start

Iteration 68b replaced the typed `'PRG' XSEND` with a program left on
the stack and "Press the keys for me" (EVAL on the 48GX, right-shift
SYMB on the 49G, `typeKeys`). The test passes on both models again, in
about 30 s (48GX) and 40 s (49G); the traces
(`crates/saturnus-tx/traces/*-xmodem.trace`) are recorded from it again,
with no edits but the 49G's padding. One run of the 48GX test in ten
failed the old way ("Nothing came from the calculator in time") at the
first fast get, alone on the machine; the cause of that and of the old
failures (typing through `run`) was not found. The bisect is no longer
needed.
