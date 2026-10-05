---
type: iteration
title: "Iteration 17: Control API over HTTP and `saturnus ctl`"
date: 2026-10-05
status: planned
tags:
  - iteration
  - saturnus
branch: iter-17/control-api
---

# Iteration 17: Control API over HTTP and `saturnus ctl`

Read first: `web/protocol.md` (iteration 11; the command and event shapes
this API reuses), `crates/saturnus-cli/src/` (the `run` command and its
serial bridge), `crates/saturnus-drive/src/` (key scripts, idle wait,
pacer), `crates/saturnus-objects/src/ram.rs` (memory reads),
`crates/saturnus-mcp/src/emulator.rs` (what the keystroke tools did; the
crate is retired in iteration 18), `kb/docs/ci.md`, `deny.toml`.

## Context (owner, 2026-10-05)

Confirmed by the owner in this repository's session, after the hptx
session relayed it: no MCP server on either side. saturnus is "an
emulator that serves a serial port and a control API". Agents and scripts
drive a running emulator over HTTP or through `saturnus ctl`; calculator
operations (ls, get, put, run) against a running saturnus go through the
hptx CLI over the serial port, exactly as against hardware.

## Design

- `saturnus run` stays a foreground process: no daemon, no instance files,
  no `ls`/`kill`/`doctor`. Ctrl-C ends it. It serves the serial port on
  TCP as today and, new, the control API. At start it prints both
  endpoints. A busy port is refused with a message naming the listener
  (pid and command where the platform tells us). Fixed default ports,
  chosen in this iteration outside 4848-4852 and 4860-4869 and documented;
  `--control <addr>` and `SATURNUS_CONTROL` select another one for a
  second instance; `--no-control` turns the API off.
- Transport: HTTP/1.1 with JSON bodies, bound to 127.0.0.1 only. Small
  blocking dependencies (a tiny_http-class server); no tokio, no TLS. The
  machine stays on its own thread (the one the serial bridge paces); HTTP
  handlers send it the same commands the Worker and the Tauri runner take.
  Reuse, do not copy: the command handling shared with
  `crates/saturnus-tauri/src/runner.rs` moves to where both can use it.
- One protocol: request and response bodies are the command and event
  shapes of `web/protocol.md`, so the API is the third adapter (Worker,
  Tauri, HTTP). Additions the HTTP host needs go into that document, not
  into a second schema.
- Endpoints (names fixed in this iteration and written into
  `web/protocol.md` and the README): `screen` (JSON rows, or `image/png`
  by `Accept`), `keys` (press, release, a key script), `type` (text),
  `mem` (read nibbles; write nibbles), `cycles`, `info` (model, ROM
  revision and hash, speed, display on, serial endpoint, protocol
  version), `snapshot` (get returns the state as bytes; put loads bytes),
  `model`. Reads from `saturnus-objects` (`tree`, `stack`, `flags`) where
  the model has them.
- Security. Loopback alone is not a boundary (a web page in the user's
  browser can send requests to 127.0.0.1; DNS rebinding makes a hostile
  name resolve there):
  - a per-user token file, created on first run with mode 0600 (on
    Windows: the user's profile directory with the default ACL, and say
    so), 256 bits from the OS random source; every request carries it as
    `Authorization: Bearer`; comparison in constant time; a missing or
    wrong token is 401 with no detail;
  - `Host` must be the bound address or `localhost` with the bound port,
    otherwise 421; a request with an `Origin` header is refused (403)
    unless it is the loopback origin of the API itself; no CORS headers
    are ever sent, and `OPTIONS` is refused;
  - changing state needs `POST`/`PUT`; `GET` never changes anything;
  - bodies are capped (a state file's size plus margin), memory
    reads and writes are bounded to the model's address space and to a
    maximum length per request;
  - the server never takes a file path: snapshots travel as bytes, the
    ROM is the one given on the command line;
  - the token is never printed, logged or put into error messages; `run`
    prints the token file's path.
- `saturnus ctl <screen|keys|type|mem|snapshot|info|cycles|model>` is the
  client: it reads the token file, talks to the default or given
  endpoint, prints text for people and `--json` for scripts, writes
  snapshots and PNGs to files it is given, and exits non-zero with the
  API's error message.

## Tasks

- [ ] Shared command handling: one module that the Tauri runner and the
  HTTP host both use (machine thread, pacer, key queue, frames).
- [ ] The HTTP server in `saturnus run` with the endpoints above; ports,
  `--control`, `SATURNUS_CONTROL`, `--no-control`; startup lines; busy
  port message.
- [ ] Token file, Host and Origin checks, method rules, size caps; unit
  tests for each refusal (no token, wrong token, foreign Host, foreign
  Origin, oversized body, out-of-range memory, GET on a mutating
  endpoint).
- [ ] `saturnus ctl` with all subcommands, `--json`, exit codes.
- [ ] `web/protocol.md`: the HTTP mapping (method, path, body, status
  codes) next to the Worker and Tauri mappings.
- [ ] Tests without a ROM: the server against a machine on a ROM of
  zeros (info, mem, snapshot round trip, refusals). ROM-gated e2e: `run`
  a 48SX, `ctl keys`, `ctl screen` equals the golden, `ctl type`,
  snapshot get/put restores the screen, the serial port still answers
  while the API is used. Windows: the tests must pass on the CI runner
  (token file location, port handling).
- [ ] `cargo deny check` stays clean without new advisory ignores; new
  dependencies listed in the Outcome with their licences.
- [ ] README: the control API, `ctl`, the security model in plain words,
  and how an agent uses it; `kb/docs/` page for the API's security
  rules.

## Acceptance criteria

- [ ] With `saturnus run --model 48sx --rom ...` running, `saturnus ctl
  keys "2 ENTER 3 +"` followed by `saturnus ctl screen` shows 5, and
  `curl` without the token gets 401.
- [ ] A request with a foreign `Host` or `Origin` header is refused even
  with a valid token.
- [ ] `just gates` passes on the three CI platforms.

## Outcome

(to be written)
