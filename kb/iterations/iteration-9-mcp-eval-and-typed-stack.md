---
title: "Iteration 9: MCP high-level tools, phase 1: eval and the typed stack"
type: iteration
date: 2026-10-05
status: planned
branch: iter-9/mcp-eval-and-typed-stack
tags:
  - iteration
  - saturnus
---

# Iteration 9: MCP high-level tools, phase 1: eval and the typed stack

Read first: `README.md` ("MCP server"), `crates/saturnus-mcp/src/{server.rs,emulator.rs,link.rs,keys.rs}`,
`crates/saturnus-drive/src/{session.rs,autostart.rs}`, hptx
`~/devel/hptx/crates/hptx-core/src/{calc.rs,reply.rs,object.rs,session.rs}`
(`Calculator::run` sends a Kermit host command and `parse_stack` returns
the display text per level; `parse_real`, `parse_list`, `parse_string`,
`parse_name` exist; `ObjectType` knows the prologs), wiki
`protocols/server-commands`, `protocols/kermit-hp`, `protocols/hp-object-format`,
`~/devel/hptx/kb/docs/calculator-quirks.md`, `kb/decision-log.md` (iterations
6 and 7: the session lock, call limits, the busy-time rule in `link.rs`).

## Context (2026-10-05)

- Decision (owner): add a semantic layer on top of the keystroke tools;
  first `eval` and a typed stack, later screen text, waits, modes, menus
  and named snapshots. The keystroke tools stay.
- The seam to hide: everything semantic goes through the ROM's Kermit
  server, which owns the keyboard while it runs; entering and leaving it
  costs about 2 s of emulated time each way (`start_server`/`stop_server`
  exist). Host commands return the stack as display text, one line per
  level, with `Error: ...` on failure; display text is lossy (number
  format, truncated strings, `{ ... }` lists on one line, programs as
  `<< ... >>`).
- Exact values come from the Kermit `get` of a variable in binary mode:
  the object with its prolog (`ObjectType`), from which reals, complex
  numbers, strings, names, lists, binary integers and programs can be
  decoded exactly (the object format is in the wiki). hptx decodes the
  header today, not the object body.
- Every tool runs under the session lock with the emulated-time budget
  and the wall-clock deadline of iteration 6; `link.rs` lets a busy
  calculator finish (10 min cap). The 49G evaluates integer literals
  symbolically for minutes; `eval` must not silently hang on that.
- Models: 48SX, 48GX, 49G have a Kermit server; the 38G has none and the
  39G/40G act as Kermit clients only. The semantic tools apply to the
  three 48/49 models; on the others they return a clear error.

## Tasks

- [ ] `eval {source, mode?}`: run RPL source (an expression, a command
  sequence or a `<< >>` program) and return the result typed. Enters
  server mode on demand (and leaves it again unless `keep_server` is set,
  so the screen shows the stack afterwards), runs the source as a host
  command (or, when it is too long for one `C` packet, stores it as a
  temporary program via `send_object`, evaluates it and purges it),
  catches the calculator's error message and returns it as a structured
  error, and never leaves temporary variables behind. Options: `levels`
  to return more than level 1; `timeout_ms` bounded by the iteration 6
  limits.
- [ ] Typed objects: an `Object` JSON model (real, complex, string, name,
  binary integer with base, list, program source, tagged, unit, array,
  unknown with prolog and hex) decoded exactly from the binary object
  fetched by Kermit `get`; `eval` and `stack` return it. The display text
  stays available as `display`. Decoding lives in a module that can move
  to hptx later; exact reals (12 digits, exponent, sign) are decoded from
  the BCD body per the wiki's object format, with tests against known
  encodings.
- [ ] `stack {levels?}`: the stack as typed objects without disturbing it
  (store each requested level into a temporary variable, fetch it,
  restore; or fetch via a single temporary list), plus `push {object}`
  and `pop`/`drop`, `clear_stack`.
- [ ] `get_var {name}` / `set_var {name, object}` / `list_vars` / `cd
  {path}`: variables by value, on top of hptx's `get`, `put`, `list`,
  `cd`.
- [ ] Server-mode management inside the tools: `eval`, `stack` and the
  variable tools enter the server if needed; `press_keys` and `type_text`
  leave it if needed; `status` reports the mode; a `keep_server` option
  avoids the 4 s round trip in batches. Document the cost.
- [ ] Tests: unit tests for the object decoder (hand-built objects for
  each type; round trip through `put`/`get` where possible), ROM-gated
  MCP e2e on the 48SX: `eval "2 3 +"` = real 5, `eval "'X^2' 3 'X' STO
  EVAL"`, a string, a list, a complex, a program result, an error
  (`eval "0 0 /"` returns the calculator's "Infinite Result"), and the
  same `eval` on the 48GX and 49G (reals and a list at least); a test
  that `press_keys` after `eval` works without an explicit `stop_server`.
- [ ] README: the semantic tools with examples; the plan Outcome; decisions
  under a dated iteration 9 heading; `hyalo lint` clean.

## Acceptance criteria

- [ ] An MCP client can boot a 48SX and call `eval "SIN(0.5)"` with no
  knowledge of keys or server mode, getting `{"type":"real","value":0.479425538604}`
  with the exact 12-digit mantissa, and `stack` after a few evals returns
  the typed levels.
- [ ] The same `eval` works on the 48GX and the 49G; on the 38G, 39G and
  40G it returns a clear "no Kermit server on this model" error.
