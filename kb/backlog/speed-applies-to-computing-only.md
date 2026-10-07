---
title: "Speed setting applies only while the calculator computes"
type: backlog
date: 2026-10-07
status: completed
priority: high
tags:
  - backlog
  - saturnus
---

# Speed setting applies only while the calculator computes

Owner (2026-10-07): "When you increase the clock speed, the calculator
turns off automatically very quickly - probably because the wall-time for
the calculator also runs faster."

The speed setting scales all emulated time, including the time the CPU is
shut down waiting for a key or a timer, so the 48's auto-off (about ten
minutes on its own clock) comes after 2.5 real minutes at 4x and seconds
at Max (sleep runs at 60x there).

Fix: while the CPU is shut down, emulated time runs at 1x regardless of
the setting, in the Worker (`web/worker.js`) and the shared runner
(`crates/saturnus-drive/src/runner.rs`); the setting applies only to
running passes. The calculator's clock then drifts only by the time spent
computing fast. Tests: idling 30 s at 4x and at Max keeps emulated time
equal to wall time; a busy loop at 4x still runs four times as fast.

## Outcome (2026-10-07)

Done in the Worker and the native runner alike: the sleep runs at 1x
(`MAX_RATE` is gone), a pass's own share stops where the CPU goes to
sleep, and the wall time it leaves belongs to the sleep. Idle 10 s on the
48SX: emulated time equals wall time to 0.1 % at 4x and at Max
(`idles_in_real_time_at_any_speed` in `crates/saturnus-tauri/tests/runner.rs`);
computing still runs 4.0x at 4x and about 37x at Max (`keeps_real_time`,
3 s, release). `web/test/worker-speed.test.mjs` checks the Worker's rule
with a fake core.
