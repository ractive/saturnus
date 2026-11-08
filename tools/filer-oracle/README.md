# BSD Filer in Docker: an HP 100LX/200LX Filer server on TCP

This image runs BSD Filer v0.3 (Dai Ishijima, 1996), a Unix emulation of
the Filer **server** of the HP 100LX and 200LX palmtops, with its serial
side bridged to TCP port 4853 by socat. `filer-proto` and `satx lx` test
against it as a black box, the way satx tests against saturnng
(`kb/docs/clean-room-rule.md`).

## Licence and clean room

BSD Filer is freeware under its author's own terms, not open source: free
use and porting for free software and personal use, no commercial use,
redistribution only unmodified (its readme's disclaimer; see
`kb/research/hp100lx-filer.md`). Therefore:

- the build downloads the author's published archive
  (`http://vvvvvv.wheel.gr.jp/~dai/software/bsdfil03.lzh`) and checks its
  SHA-256 (`fdca9760...3404`, the archive of 2026-11-08);
- nothing of it is in this repository, and its source is not read for
  saturnus (MIT); only its behaviour on the wire is used;
- the image is local only: never pushed, cached or shipped.

The build compiles the C files as they are (GNU C89) with
`-include sys/vfs.h`, which Linux needs for `statfs`; the archive is not
changed.

## Build and run

```sh
docker build -t filer-oracle tools/filer-oracle
docker run -d -p 4853:4853 --name lx filer-oracle
satx lx --port tcp://localhost:4853 ls
```

or `just lx-oracle-up` and `just lx-oracle-down`. Port 4853 sits beside the
saturnng containers' 4848-4852; our own serial bridges use 4860-4869.

Every TCP connection gets its own pseudo-terminal and its own BSD Filer
process (`serve-one`), all serving `/srv/lx` as drive C:, so a client that
dies mid-session does not wedge the next one. BSD Filer's messages go to
`/tmp/bsdfiler.log` in the container (`docker exec lx cat
/tmp/bsdfiler.log`).

The served folder, made at start:

| Name | Contents |
| --- | --- |
| `HELLO.TXT` | 30 bytes of text, dated 2004-12-14 13:49:36 |
| `BYTES.BIN` | 5000 bytes, every value 0-255 in turn |
| `EMPTY.DAT` | empty |
| `SUB\NOTE.TXT` | 19 bytes of text |

## What it does and does not do

Observed on the wire (2026-11-08):

- Drive C: only, label `BSDFILER`; the frame, CRC and counter rules as
  the wiki describes them.
- No answer at all to 0x05 (overwrite), 0x07 (current drive), 0x0E
  (remove directory), 0x19 and 0x1A (attributes): the client times out
  after three sends (about 18 s) and ends the session.
- No error codes: a bad name, a missing file to rename or delete, an
  existing directory to make all answer 0. 0x03 on an existing file
  overwrites it (no 217); 0x08 answers 0 for a file.
- Delete renames to `NAME.BAK`, which is then hidden (not 8.3).
- A listing of, or an open in, a missing directory or file makes the
  process exit: the TCP connection closes.
- 0x18 (set time) is accepted but not kept; 0x17 gives no valid time.
- Unix names that are not valid 8.3 or contain upper case are not listed.
