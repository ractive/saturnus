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

- **Connect**: opens at 9600 baud, 8 data bits, no parity, sends a Kermit
  I packet (the server only ACKs it) and listens 3 s (the probe again
  halfway) for that ACK or an idle server's NAK; then 4800, 2400 and 1200.
  The packet heard tells the parity too (bit 7 of each byte): odd or even,
  the port is reopened with 7 data bits and that parity and Kermit asks
  for 8th-bit prefixing. Then the sync, `VERSION` (the model), flag -35
  read and set for binary transfers. With `find` off, the given settings
  only.
- **List** a directory (`["HOME", "D1"]`), **Get** binary or text (flag
  -35 cleared for the one transfer), **Put** a binary file, a `%%HP:` text
  file (text mode) or, with `as_string`, anything else as a string; a name
  in use is `TxError::NameClash` unless `replace`, a directory is never
  replaced.
- **Disconnect** puts flag -35 back as it was and sends FINISH.
- `names`: `NAME.hp` / `NAME.txt`, Unicode names kept, `/ \ : * ? " < > |`,
  `%`, control characters and a trailing dot or space as `%XX`;
  `FileKind::of` tells binary, text and other files apart.
- `trace`: the recorded traces' text form and `Replay`, which plays a
  trace's calculator on a fake clock.

## Traces

`traces/MODEL-session.trace` are the session of `tests/scenario` recorded
from saturnus in-process on the 48SX, 48GX and 49G by `just tx-traces DIR`
(the example `record-traces`, with the ROMs in DIR; it refuses to write a
trace holding a 16-byte run of the ROM). `tests/traces.rs` replays them
byte for byte and makes the failures from them: no answer, the wrong
speed, the wrong parity, a NAK storm, a cut link, refused names and a name
in use.

## Licence

MIT, see `AI_NOTICE` and `LICENSE` in the repository.
