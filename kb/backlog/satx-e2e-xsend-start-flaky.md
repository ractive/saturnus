---
type: backlog
title: "satx e2e: the 49G oracle sometimes does not start XSEND"
date: 2026-11-08
status: planned
priority: medium
tags:
  - backlog
  - satx
---

# satx e2e: the 49G oracle sometimes does not start XSEND

On `main` after PR #262's merge (run 42523704138, 2026-11-08), the 49G
oracle's CLI script (`scripts/satx-e2e-cli.sh`) failed at

```text
satx get SATXXM -o … --protocol xmodem --start-timeout 60
error: get SATXXM: the calculator did not start XSEND within 60 s
```

That is the stacked route: satx leaves `« 'SATXXM' XSEND »` on the
stack and the script types EVAL through the oracle's `calc-keys`
(`SATX_E2E_CONTAINER`). The same test passed on PRs #261, #262 and
#264, so this is a flake, not a regression. The failing step took 14
minutes against 6-7.5 for a green one: after the failure the script's
cleanup ran satx commands against a calculator out of server mode.
It looks like shape 2 of [[backlog/satx-ci-emulator-flakes]] (the same
message on 2026-10-06, then on a freshly restarted 49G) and is related
to [[backlog/satx-flaky-xmodem-e2e]] (the typed-keys start window).

A second flaky case, on the emulated 48SX: two 20 s no-replies after a
STOF in a CLI script run, in the CI runs of PR #259's fixes (IOPAR and
VERSION read whole when the 49G cuts them).

Since 2026-11-08 the CLI script runs nightly and by hand only
(`e2e.yml` with `-f full=true`; [[docs/ci]], "What runs when"), so a
flake here no longer reds a push to `main`, but it reds the nightly.

## Next steps

- [ ] Rerun the full set on one commit several times
  (`gh workflow run e2e.yml --ref main -f full=true`) and count the
  XSEND-start failures, to measure the rate.
- [ ] On failure, keep the oracle's screen (`calc-screen`) and
  `/tmp/saturn.log` from the moment the 60 s run out, not only at the
  end of the job: did EVAL reach the calculator, is the program still
  on the stack, did XSEND start late?
- [ ] Depending on that: retype EVAL once, give the oracle a longer
  start window, or wait for the stack display before typing.
- [ ] The 48SX case: look at the run's log for the STOF before the two
  no-replies when it shows up again.
