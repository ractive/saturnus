# saturnus-tx

The device layer of saturnus tx, the transfer page and desktop app
(`kb/docs/saturnus-tx.md`): what a user does with an HP 48S/SX, 48G/GX or
49G in Kermit server mode, as whole requests over `satx-core`'s sans-I/O
`Machine`. No I/O, no threads, no clock of its own; it builds for
`wasm32-unknown-unknown` (a CI step). Not published (`publish = false`).

## The API

`Device` follows `satx_core::Machine`'s calling convention, one level up:

| Call | What it does |
|---|---|
| `Device::new(now, options, seed)` | A device for one link; `seed` should be random (the sync markers). |
| `start(now, request)` | Begin a `Request`: `Connect`, `List`, `Get`, `Put`, `Disconnect`. |
| `handle_input(now, bytes)` / `handle_timeout(now)` / `handle_link_error(now, err)` | Bytes, the time, a failed link. |
| `poll_action()` | `Action::Open(LinkSettings)` (reopen the port: speed, parity) or `Action::Write(bytes)`, in order. |
| `poll_event()` | `Listening`, `Found` (the search), `Progress` (Kermit). |
| `poll_result()` | The `Outcome` or a `TxError`. |
| `next_timeout()` | When to call `handle_timeout` at the latest. |
| `abort(now)`, `connection()` | Drop the running request; the connected calculator. |

- **Connect**: opens at 9600 baud, 8 data bits, no parity, and sends a
  Kermit I packet (the server only ACKs it) framed for no, odd and even
  parity 500 ms apart (bit 7 as the parity bit: a calculator with parity
  checks it), listening 6 s (the probes again halfway) for the ACK or an
  idle server's NAK; then 4800, 2400 and 1200. Probing rather than only
  listening: a server need not NAK while idle (saturnus's emulated server
  NAKs once per timeout, then waits). The packet heard tells the parity
  too: odd or even, the port is reopened with 7 data bits and that parity
  and Kermit asks for 8th-bit prefixing. Then the sync, `VERSION` (the
  model), flag -35 read and set for binary transfers. With `find` off,
  the given settings only. The 49G in algebraic mode is not switched to
  RPN (Kermit needs no switch); its server packs the stack in a list at
  FINISH.
- **List** a directory (`["HOME", "D1"]`), **Get** binary or text (flag
  -35 cleared for the one transfer and set again, also after a failure),
  **Put** a binary file, a `%%HP:` text file (text mode) or, with
  `as_string`, anything else as a string; a name in use is
  `TxError::NameClash` unless `replace`, which puts into an unused name
  (`SATXPUT1`) first and swaps it in with one command once stored, so a
  failed put leaves the old variable; a directory is never replaced.
- **Disconnect** puts flag -35 back as it was and sends FINISH, also when
  the flag fails; the session stays when FINISH fails.
- `names`: `NAME.hp` / `NAME.txt`, Unicode names kept, `/ \ : * ? " < > |`,
  `%`, control characters, a trailing dot or space and the first letter of
  a Windows device name (`AUX` as `%41UX.hp`) as `%XX`; `var_name`
  refuses `%XX` that is not UTF-8. Names that differ only in case collide
  on a case-insensitive file system. `FileKind::of` tells binary, text and
  other files apart.
- `trace`: the recorded traces' text form and `Replay`, which plays a
  trace's calculator on a fake clock.

## Traces

`traces/MODEL-session.trace` are the session of `tests/scenario` recorded
from saturnus in-process on the 48SX, 48GX and 49G by `just tx-traces DIR`
(the example `record-traces`, with the ROMs in DIR; it refuses to write a
trace whose transferred data, decoded from the Kermit packets, holds a
16-byte run of the ROM, the server's text replies aside).
`tests/traces.rs` replays them byte for byte and makes the failures from
them: no answer, the wrong speed, each parity, a NAK storm, a cut link,
refused names, a name in use, a failed replace, a failed or aborted text
get, a disconnect whose flag or FINISH fails.

## Licence

MIT, see `AI_NOTICE` and `LICENSE` in the repository.
