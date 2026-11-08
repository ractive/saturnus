# filer-proto

A sans-IO client and server of the Filer protocol of the HP 100LX and
200LX palmtops: the link their built-in Filer speaks in server mode over
the serial cable. HP never published it; this crate follows the calculator
wiki's pages `protocols/filer-hplx` and `hardware/hp100lx-200lx`
(<https://github.com/ractive/calculator-knowledgebase>) and is checked
against BSD Filer v0.3 as a black box.

The crate never touches a port, a clock or a file. The caller reads and
writes the link, supplies the current `Instant` and keeps the files. It
builds for `wasm32-unknown-unknown`.

- `codec`: the frame (three SYN, DLE STX, SOH, command, counter,
  direction, revision, data with DLE doubling, DLE ETX, CRC-16/ARC low byte
  first and never doubled) and a deframer that resynchronises.
- `Request`, `Response`, `Code`: the 28 commands, their request data and
  reply layouts; `FsError` and `error::status` the error codes with their
  meanings.
- `Client`: one request at a time with HP's timing: 2 s for CONNECT and
  5.9 s for everything else to the first byte of the reply, three sends of
  the same frame, CONNECT up to six times, the counter advanced only on a
  matching reply.
- `Session`: what a file manager does (`Job`: list, get, put, rename,
  delete, make and remove directories, the drives), as the request
  sequences HP's own client sends.
- `Server` over a `FileSystem` (`MemoryFs` in memory): the palmtop's side,
  for tests, with its counter rules (a repeated counter gets the previous
  reply without running the command again).

## The seam

- `Session::start(now, job)` begins a job (`Job::Connect` first).
- `Session::handle_input(now, bytes)` feeds bytes read from the link.
- `Session::handle_timeout(now)` when `Session::next_timeout()` is reached.
- `Session::poll_output(now)` hands out the next frame; write it whole.
- `Session::poll_event()` yields progress and one `Finished` per job.
- `Session::disconnect(now)` queues one DISCONNECT, not awaited.

`satx lx` (the `satx` crate) drives it over a serial port or TCP.

## Licence

MIT. See `LICENSE`.
