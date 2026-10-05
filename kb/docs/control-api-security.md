---
type: docs
title: Control API security
date: 2026-10-05
status: active
tags:
  - security
  - saturnus
---

# Control API security

The rules of the control API that `saturnus run` serves (iteration 17,
[[iterations/iteration-17-control-api]]), why each exists, and the test
that holds it. Code: `crates/saturnus-cli/src/control/` (`server.rs`,
`token.rs`, `http.rs`); the HTTP mapping is in `web/protocol.md`
("HTTP"). Any change to these rules is a decision-log entry.

## Threat model

The API changes the calculator's memory and state, so a caller must be the
user who started `run`. Loopback is not a boundary:

- Every local user and process can connect to 127.0.0.1.
- A web page in the user's browser can send requests to 127.0.0.1 (simple
  `POST`s need no preflight), and DNS rebinding can make an attacker's
  host name resolve to 127.0.0.1, so its pages count as same-origin for
  the browser.

Out of scope: a process running as the same user (it can read the token
file, or the emulator's memory, anyway), and other machines (the API
never listens anywhere but 127.0.0.1).

## Rules

| Rule | Why | Test |
| --- | --- | --- |
| Bind 127.0.0.1 only; `--control`/`SATURNUS_CONTROL` accept `PORT`, `127.0.0.1:PORT`, `localhost:PORT` and refuse any other host | No remote caller | `control::tests::control_addresses_are_loopback_only`, `tests/control.rs::run_refuses_other_addresses_and_keeps_batch_mode` |
| A per-user token file, 256 bits from the OS (`getrandom`), 64 hex digits; Unix: mode 0600 in a 0700 directory, a file with group or other bits refused; Windows: `%LOCALAPPDATA%\saturnus\control-token` under the profile's default ACL (no ACL set by saturnus) | Other local users cannot read it | `token::tests::created_once_then_read_back`, `malformed_and_exposed_files_are_refused_without_their_content` |
| Every request needs `Authorization: Bearer <token>`, compared in constant time; missing or wrong is 401 with the body `{}` and nothing else, before the route is looked at | Other users, and browsers (a page cannot add the header without a preflight, which is refused) | `server::tests::missing_or_wrong_tokens_get_401_without_detail` |
| `Host` must be `127.0.0.1:PORT` or `localhost:PORT` with the bound port, else 421 | DNS rebinding: a rebound page sends its own host name | `server::tests::foreign_hosts_and_origins_are_refused_with_a_valid_token` |
| Any `Origin` header other than `http://127.0.0.1:PORT` or `http://localhost:PORT` is 403, even with a valid token | Cross-site browser requests always carry one | the same test |
| `OPTIONS` is 405 and no response ever has a CORS header | No preflight succeeds, so no page reads an answer or sends the token | `server::tests::options_and_wrong_methods_are_refused` |
| `GET` never changes anything; keys, typing, `poke` and loading a state need `POST`/`PUT`; each endpoint takes only its own commands (no `boot`, `pause`, `reset`, `setSpeed` over HTTP) | Reads cannot be abused to write | the same test |
| Bodies capped: 256 KiB of JSON, 4 MiB of state (the 49G's 2.6 MB state plus margin), refused from `Content-Length` before reading; `Transfer-Encoding` refused; heads at most 16 KiB and 64 lines | Memory and time per request bounded | `server::tests::oversized_bodies_and_heads_are_refused` |
| `peek`/`poke` inside `#00000`-`#FFFFF` without wrap-around, at most 65536 nibbles | Bounded to the model's address space | `server::tests::memory_access_is_bounded` |
| The server never takes a file path (`path`, `romPath` refused by the machine thread); states travel as bytes, `ctl` does the file I/O, the ROM is the one on `run`'s command line | No file read or written on a caller's say | `server::tests::memory_access_is_bounded` (the `path` case), `saturnus-tauri` `files_come_from_the_host_and_are_capped` |
| The token is never printed, logged or put into an error; `run` prints the file's path; `Token`'s `Debug` is redacted | It would leak into terminals, CI logs, agent transcripts | `token::tests`, `tests/control.rs::ctl_talks_to_a_running_saturnus` (401 message) |
| Head within 5 s, body within 20 s, response write within 10 s; at most 8 connections, each on its own thread, more get 503 at once; handlers reach the machine only through its command channel | A slow or stalled client never holds the machine thread or the serial bridge | `server::tests::stalled_clients_do_not_block_others` |
| One request per connection, `Connection: close`; after the response the server drains at most 1 MiB for 1 s before closing | A refused client still reads its answer instead of a reset | (by construction) |

## Known limits

- The token is per user, not per instance: every `run` of the user shares
  it. Deleting the file makes a new one on the next start.
- A key script or typed text runs on the machine thread at emulated speed
  for up to 30 s of wall time; the serial bridge waits meanwhile. This is
  the caller's own doing (it holds the token), not a client stall.
- Windows: the file inherits the profile directory's ACL; saturnus checks
  no ACL there. A profile with a loosened ACL loosens the token too.
