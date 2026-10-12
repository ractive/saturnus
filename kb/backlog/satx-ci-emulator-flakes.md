---
type: backlog
title: "CI emulator flakes: the silent 48SX and the 49G XModem start"
date: 2026-10-06
status: planned
priority: medium
tags:
  - backlog
  - satx
---

# CI emulator flakes: the silent 48SX and the 49G XModem start

Two failure shapes in the CI `e2e` job on 2026-10-06, both on code that
had passed CI before (for the 48SX shape the same suite minutes earlier in
a run the concurrency group then cancelled; for the XSEND shape main's run
after the merge):

1. **The 48SX container never answers.** Every one of the seven
   `satx-core` end-to-end tests fails with `Kermit(Timeout)` after its
   full retry budget (about 144 s each, 1007 s for the suite). Seen twice,
   each time on attempt 1 (PR #21 run 37394209709, PR #22 run
   37420562859); `gh run rerun <id> --failed` passed both times. The
   "Start HP 48SX and HP 49G" step passed each time and the container log
   ends with `Kermit server started (SERVER)` then `bridged` exactly like
   a healthy start, so the emulator booted but the Kermit server was not
   answering. The existing diagnostics gave no evidence: the
   "Screens and logs on failure" step's `calc-screen` dump of the 48SX is
   the same unreadable bitmap in healthy and failing runs (it works for
   the 49G only), and the boot log is identical to a healthy start.
2. **The 49G does not start XSEND in time.** `scripts/satx-e2e-cli.sh` fails at
   the XModem step with "the calculator did not start XSEND within 60 s"
   (PR #23 run 37421578335, not rerun: the PR was docs-only and was merged
   with it red). Related to `satx-flaky-xmodem-e2e.md` (the same
   typed-keys start window), but on a freshly restarted 49G, i.e. after
   that item's fresh-container mitigation. The failing step took 12
   minutes, of which the 60 s window was a small part: the rest was the
   script's cleanup (`restart_server`, then satx commands against a
   calculator possibly out of server mode).

## Options

- After the start step, probe each container with
  `satx --timeout 5 --retries 1 info`; if it does not answer, capture the
  `info` error, `docker logs` and a working screen dump (fix
  `tools/saturnng-oracle/calc-screen.sh` for the 48SX or dump the LCD another way) so
  the next occurrence yields a root cause, then type `SERVER` once (as
  `scripts/satx-e2e-cli.sh` already does for its first command) and, if still
  silent, recreate the container before the suite runs. The probe costs
  seconds and tells a dead calculator from a slow one; the current failure
  costs 17 minutes and a rerun.
- For the XSEND start: make the start timeout an environment variable in
  `scripts/satx-e2e-cli.sh` (it is a literal `--start-timeout 60` today) or
  retry the typed keys once (see the other item), and make the cleanup
  after a missed start bail out fast instead of running the full tail.
- A shorter Kermit budget for the core suite is not a CI knob: `tests/e2e.rs`
  uses `Options::default()`; it would need an `SATX_E2E_TIMEOUT`-style
  variable, and a shorter budget worsens the load flake the other item
  records. Prefer the probe.

Not a product bug: the same code passes locally and, for the 48SX shape,
on rerun.

## Status 2026-10-12

Shape 1 still occurs. Of the eight `e2e.yml` runs that ran since
2026-10-11 (seven pushes to main, one labelled PR), one failed: push run
38190346239 (the merge of PR #144), the 48SX job, all seven core tests
`Kermit(Timeout)`, 1007 s, while the 49G job of the same run passed and the
next push run passed. Same signature as before: the boot log ends with
`Kermit server started (SERVER)` and `bridged`, no "screen did not settle"
warning, and the 48SX screen dump is the usual unreadable bitmap (with the
busy annunciator on).

A lead, not confirmed: `tools/saturnng-oracle/entrypoint.sh` types the
boot answer and `SERVER` once `wait_stable` sees the screen unchanged for
1.5 s. On the 48SX that screen is the unreadable bitmap, so "stable" may
not mean "booted"; keys typed too early would be lost and leave the
calculator silent for the whole suite, which is what the failures look
like. Shape 2 (the 49G XSEND start) did not occur in these runs. Next step
is still the probe in Options (a `satx info` after the start step, retype
`SERVER` or recreate the container when silent); not done here, as it
needs CI runs to verify.

## Outcome (2026-10-13, PR #154)

Shape 1, the silent calculator after start-up: mitigated, cause partly
confirmed. The `wait_stable` lead holds in a wider form. A settled screen
does not prove the calculator took the keys: on a CPU-starved container
saturnng loses typed keys while the screen stays unchanged. Locally, with
`docker run --cpus 0.15`, 12 of 15 cold 48SX starts never answered Kermit
(some still at "Try To Recover Memory?", others with SERVER lost). The CI
failure's own screen (a garbage LCD with the busy annunciator) was not
reproduced, so it is not proven that CI hit the same path. Also corrected:
a healthy 48SX screen dump is readable ("Awaiting Server Cmd."), so only
the failing dump was garbage.

The fix is in `tools/saturnng-oracle/`. After SERVER, the entrypoint sends
a Kermit I packet to the wire port and bridges only when the ACK comes
back. A start without one is killed unsaved and repeated with slower
typing, four starts in all. After that the container exits with
`error: ... did not answer Kermit`. `wait-ready.sh` fails at once on that,
or after 300 s, instead of letting the suite run into 1007 s of timeouts.
`bridged` is now printed only once socat listens. Under starvation a client
that connected on the word had its connection closed, 1 of 8 starved 49G
starts.

Evidence:

- Locally, at normal load: 20 of 20 cold starts answered on the first try,
  for the 48SX and for the 49G. This was the final image.
- Locally, starved at `--cpus 0.15`: 48SX 8/8, 12/12 and 6/6 answered,
  often on the second or third start. 49G 10/10 answered after the listen
  fix.
- When every start fails (`--cpus 0.03`, `READY_ATTEMPTS=1`), wait-ready
  failed after 70 s and printed the error.
- CI `e2e.yml` was dispatched on the branch. All five runs on the final
  head f9928f2 passed: 38312597484, 38313817112, 38315002236, 38316188583
  and 38317347551. Earlier heads: 38307432427 and 38308793404 (7fd2dc8)
  and 38310181200 (530310b) passed. 38311432717 (530310b) failed in the
  CLI script, see below. In every CI start the calculator answered on its
  first start, so the retry path has not yet been exercised in CI.

The item stays open as a watch. Close it after two weeks of `main` without
a silent-start failure. A failure from now on shows in the start step as a
`warning: start N` line or a fast error with the log, not as
`Kermit(Timeout)` in the suite.

Shape 2, the XSEND start, did not recur and is not changed here. It is the
typed-keys window of [[backlog/satx-flaky-xmodem-e2e]].

New in run 38311432717: the 49G CLI script failed at "late reply of an
aborted command is skipped". `satx ls` after the killed REPL got
`not in step with the calculator: neither reply showed the sync marker`.
In that run the 49G had not been restarted after the core suite (that
commit had replaced the restart with a probe). It is the only such failure
in the nine branch runs and has not been seen with the restart. CI
restarts the 49G again, after the probe. Whether the inherited state
matters is open. If it recurs, it belongs with
[[satx/backlog/stale-reply-after-abort]].
