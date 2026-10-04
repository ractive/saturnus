---
title: "Iteration 4: satx CLI"
type: iteration
date: 2026-10-04
status: planned
branch: iter-4/satx-cli
tags:
  - iteration
  - satx
---

# Iteration 4: satx CLI

## Tasks

- [ ] Commands: `ports`, `info`, `ls`, `get`, `put`, `rm`, `mkdir`, `mv`,
  `run`, `screenshot`, `backup`, `restore`, `settings`, `finish`, and
  offline `object inspect`, `object convert` (binary <-> ASCII where the
  object is text-representable), `grob to-png`.
- [ ] `--port` accepts a device path or `tcp://`; `SATX_PORT` env var; auto-pick
  when exactly one USB serial port exists.
- [ ] Output per the conventions in `CLAUDE.md`. `--json` is implied when piped.
- [ ] Transfer mode: `get`/`put` default to binary and rely on satx-core to
  set or check the HP's mode first (the fresh 48SX is in ASCII mode and
  mangles binary data, iteration 2); `--ascii` opts into ASCII transfer.
- [ ] Host-command length: `run`, `mv`, `settings` and friends go through
  `C` packets limited to 77 encoded bytes (`kermit-proto`
  `StartError::TooLong`); surface satx-core's split-or-error behaviour with
  a clear message, never a truncated command. Names and server text arrive
  as HP-charset bytes and are shown translated (satx-core, iteration 3).

## Acceptance criteria

the M3 e2e scenarios re-expressed as a shell script driving the
  binary (one script, run in the same CI job); `satx --help` reads well for a
  human; an agent can do `satx ls --json | jq`.
