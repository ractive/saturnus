---
type: backlog
title: The fast transfer's end-to-end test fails on the emulated 48GX and 49G
date: 2026-11-05
status: planned
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

## Next

- Bisect from 2026-10-17 to `v0.10.2` on the first-parent history, with
  `--test-name-pattern 48gx` and a `just web` per step, in a worktree.
- Try the same steps by hand on an emulated 48GX in Files.
