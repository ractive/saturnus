---
type: backlog
title: XModem e2e on the 49G is flaky under load
date: 2026-10-05
status: planned
priority: medium
tags:
  - backlog
  - satx
---

# XModem e2e on the 49G is flaky under load

Seen 2026-10-05 in CI on a kb-only PR (#14, code identical to main): the
satx-core `xmodem_round_trip` scenario on the 49G failed with `timed out:
too many retries` after 286 s, while the 48SX run and the six Kermit
scenarios passed. The implementation agents saw the same failure locally
only when the machine's load average was above about 8, and never on a
rerun alone. The scenario types `XRECV`/`XSEND` on the emulated keyboard
through `docker exec calc-keys` after a fixed delay; under load the
calculator is not ready when the keys arrive, or the keys arrive after the
sender's start window.

## Options

- Make the typing robust: wait for the LCD to settle (`calc-screen` shows
  the stack) before typing, and retry the keyboard command once if no start
  character arrives within a few seconds.
- Run the two CI emulators one after the other instead of side by side.
- Mark the scenario as retried once in CI only.

## 2026-10-05, later

In CI the emulated 49G was dead after the satx-core suite in three runs in
a row (PRs #14 and #16): the CLI script's first `info` got no answer, and
retyping SERVER via `calc-keys` did not revive it. That is the emulator
wedge, not a missed SERVER. CI now gives the CLI script a fresh 49G
container after the core suite; the local scripts keep the SERVER
restart-and-retry on the first command. Root cause still unknown; the
core suite's last scenario is the XModem round trip with its keyboard
SERVER restart, so the ON press during a settling exchange ("Port Not
Available" state, see the quirks checklist) is the leading suspect.

## Status 2026-10-12

Not seen in the last eight `e2e.yml` runs (2026-10-11 to 2026-10-12): the
49G job passed in all of them, the core suite's `xmodem_round_trip`
included. The fresh 49G before the CLI script still hides whether the
wedge after the core suite happens, so the root cause stays open; the one
failure in those runs was the silent 48SX of
[[backlog/satx-ci-emulator-flakes]].

## Status 2026-10-13

The wedge after the core suite was measured. CI now probes the calculator
with a Kermit I packet after the satx-core suite. A silent one gets an
"Oracle wedged" warning annotation with the probe output and the log.
Results:

- In nine branch runs of PR #154 the 49G answered every time: 38307432427,
  38308793404, 38310181200, 38311432717, 38312597484, 38313817112,
  38315002236, 38316188583 and 38317347551.
- Locally it answered in 4 of 4 runs of the core suite against a fresh 49G.
- The core suite's `xmodem_round_trip` passed in all of these.

The wedge has not been seen since 2026-10-05. CI still restarts the 49G
before the CLI script. In the one run without that restart (38311432717)
the script failed in its aborted-command resync, see
[[backlog/satx-ci-emulator-flakes]]. That is too little to call it related
or unrelated, so the restart stays. Next step: keep watching the
annotation, and drop the restart once the annotation has stayed silent and
the resync failure is understood. The XSEND start window (typed keys
lost under load) is unchanged.
