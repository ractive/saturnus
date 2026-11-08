---
title: "Files and the HP 100LX/200LX: the Filer protocol over a serial cable"
type: research
status: completed
date: 2026-11-08
tags:
  - hp100lx
  - serial
  - filer
  - files
  - satx
---
# Files and the HP 100LX/200LX (research, 2026-11-08)

Question from the owner: can Files (and satx) list, copy, rename and
delete files on his real HP 100LX over a cable, either inside the one app
or as a dedicated "100LX Files"? saturnus does not emulate the palmtop
(MS-DOS on an 80186-class CPU); only the cable protocols matter.

**Short answer: yes, and it fits Files well.** The palmtop's built-in
Filer has a server mode that speaks a small request/reply protocol
(binary-synchronous framing, CRC-16, 28 known commands, at most 2 KB of
data per packet). Third parties documented it well, it covers everything
a two-pane file manager needs (drives, directory listing with size, date
and attributes, get, put with or without overwrite, rename/move, delete,
mkdir, rmdir, timestamps, attributes), and a 1996 server emulator (BSD
Filer) still compiles today and can serve as a black-box oracle. The
recommendation is a sans-IO `filer-proto` crate plus a third device kind in
Files, in about three iterations plus one session with the real device.

Every fact below names its source. Sources are in [Sources](#sources),
with their licences. GPL and other open implementations were read only to
learn how the device behaves; nothing below is copied from them, and
nothing from them may be copied into saturnus (MIT). The facts here are
meant for a calculator-knowledgebase page on the 100LX/200LX (the owner
plans one); implementation should then work from that wiki page.

## 1. The Filer protocol

### Roles and how the palmtop is put into server mode

- Two machines running Filer can connect; the one that **receives** the
  connect request becomes the server, and the one that sent it (the
  client) drives everything. While it serves, the server can do nothing
  except end server mode. The client sees the server's disks as if they
  were local. (src: [BSD Filer readme], [BSD Filer man page])
- Each packet the client sends is answered by exactly one packet from the
  server; the server never speaks first (src: [j200lx Filer page];
  [Zampach protocol text]; behaviour of [BSD Filer source]). So Files would
  be the **client (master)** and the palmtop the **server**, the same shape
  as the 48's Kermit server.
- On the palmtop: start Filer, open MENU, Communications, Remote
  Settings, set the speed and the interface (COM1, the wire) and enable
  server mode (src: [lxbackup man page]; [Palmtop Paper CPACK article]
  gives the Connectivity Pack defaults as 9600 baud, COM1, server mode
  enabled). LXTools then needs nothing else on the palmtop, so with server
  mode enabled an open Filer answers a connect from the cable (src:
  [lxbackup man page]).
- The reverse direction exists too: the palmtop's Filer as **client**
  (MENU, Communications, Connect, F10; then Remote, F6) against a PC
  program that pretends to be a Filer server (src: [BSD Filer readme]).
- Switching to another application during a remote connection can break
  it ("Remote Connection Broken"), and the same happens with HP's own
  Connectivity Pack; leave Filer open and in front (src: [BSD Filer
  readme]).

### Physical layer

- 8 data bits, no parity, 1 stop bit, **no flow control** of any kind
  (neither RTS/CTS nor XON/XOFF); a three-wire cable (TX, RX, ground) is
  enough (src: [Zampach protocol text]; [Zampach WFX instructions],
  history of 1.01; LXTools 1.1b switched off RTS/CTS and carrier detect
  ([LXTools README])).
- Speeds: Zampach gives 2400 to 115200 baud ([Zampach protocol text]).
  The Filer's Remote Settings go up to 115200 for palmtop-to-palmtop links
  ([Palmtop Paper IR article]). lxbackup defaults to 38400 and suggests
  19200 when transfers fail ([lxbackup man page]); LXTools 1.1b added
  57600 and 115200 ([LXTools README]). BSD Filer defaults to 9600 and
  reports working runs at 9600 (cable) and 38400 (IR) ([BSD Filer
  readme]); its DOS port reports 57600 by IR and by cable with a 200LX
  ([LXFS readme]). **Contradiction:** the comp.sys.palmtops FAQ says the
  internal 8250 UART of a stock 100/200LX is good for 19200, and that
  higher rates only work for receiving, through a buffered UART on a
  PCMCIA card ([palmtops FAQ], "How fast is the highest speed"). Expect
  19200 or 38400 to be safe and anything higher to need a test on the
  owner's device.

### Packet framing

The framing is IBM binary-synchronous style (BSD Filer's author cites a
BSC text as his reference: [BSD Filer readme], references). Each packet,
in both directions:

1. Sync and start: `16 16 16` (three SYN), then `10 02` (DLE STX).
2. Header: `01` (SOH), the **command** byte, the **counter** byte, a
   **direction** byte (`01` from the client, `81` from the server), then
   `02` (STX).
3. The command's data (layouts below; all multi-byte numbers little-endian).
4. End: `10 03` (DLE ETX).
5. CRC, two bytes, low byte first.

- **Transparency:** inside the CRC-covered part every `10` byte is sent
  twice; the receiver drops the first of each pair (src: [Zampach
  protocol text]; [BSD Filer source] treats a DLE as an escape).
- **CRC:** CRC-16 with the reflected polynomial 0xA001 (that is,
  x^16+x^15+x^2+1, the "CRC-16/ARC" parameters: start value 0, input and
  output reflected, no final XOR). It covers the SOH, the command, the
  counter, the direction byte, the STX, every data byte (each counted
  once, before doubling) and the ETX. It does not cover the three SYN,
  the DLE STX in front, the doubled DLEs, or the DLE before ETX. (src:
  [Zampach protocol text] for the polynomial and the excluded bytes; the
  start value 0 and the exact coverage agree in [WFX source] and [BSD
  Filer source].)
- **Counter:** the client numbers its packets; it wraps from 0xFF to
  0x00, CONNECT and DISCONNECT always carry 0, and the server's reply
  repeats the command and the counter of the request (src: [Zampach
  protocol text]; [WFX source]). BSD Filer treats a request with the same
  counter as the previous one as a retransmission and, for a file read,
  rewinds and resends the same block ([BSD Filer source]), so the counter
  is what makes a retry safe.
- **Sync before the first packet:** Zampach's client sends four NUL
  bytes ahead of its first packet while not connected ([WFX source]).
  Whether the palmtop needs them is unknown.
- **Packet size:** at most 0x800 (2048) data bytes per data packet (src:
  [j200lx Filer page]; [Zampach protocol text]); the client chooses each
  block's length, up to that limit.

### Reply layout and errors

Every reply's data starts with a fixed run of status bytes that are all
zero on success: two bytes for CONNECT and DISCONNECT, four for the rest
(src: [Zampach example trace]; the [WFX source] client rejects a reply
whose status bytes are not zero). The meaning of non-zero values is not
documented anywhere read. The only error code seen is in BSD Filer, which
puts 0xD1 into the third status byte when it cannot change to a directory
([BSD Filer source]). BSD Filer also simply does not answer a command it
does not know, so the client times out ([BSD Filer source]). The real
palmtop's error replies (missing file, disk full, file exists, directory
not empty, read-only drive D:) must be observed on the device.

### Timeouts and retries

Nothing official. Zampach's client clears the port, sends, waits for a
reply (roughly one second plus a per-byte allowance) and tries three
times before giving up; after an error it waits 300 ms, clears the port
and sends CONNECT again ([WFX source]). LXTools 1.1b added retransmission
because high speeds produced errors ([LXTools README]). BSD Filer waits
up to 15 s for a byte ([BSD Filer source]).

### Commands

Command codes and their data, in my own grouping and words. "Name" means
a counted string: two-byte length, the characters (a DOS path such as
`C:\_DAT\FOO.TXT`), then two zero bytes. Request data, then reply data
after the status bytes. (src: [Zampach protocol text], which documents 13
more commands than PAL did ([S.U.P.E.R. index], WFX-HPLX entry), checked
against the recorded LX-to-LX session in [Zampach example trace]; names
in italics are BSD Filer's or j200lx's where they differ.)

| Code | Purpose | Request | Reply |
| --- | --- | --- | --- |
| 40 | connect | nothing | nothing |
| 41 | disconnect | nothing | nothing |
| 1B | list the drive letters | `00 00` | counted string of letters, then four zeros; Zampach says the usual answer is `ABCDE` |
| 07 | ask the current drive | `00 00` | drive letter, one zero |
| 11 | change drive | letter, one zero | nothing |
| 14 | volume label and free space | `00 00` | length 11, label (11 bytes), free bytes (4) |
| 06 | ask the current directory of a drive | letter, one zero | counted string (`C:\`), four zeros |
| 10 | change directory | name | nothing |
| 08 | does a name exist | name | one byte: 0 none, 1 file, 2 directory (no trailing backslash), one zero |
| 12 | start a listing (*wildcard*) | name with a pattern, e.g. `C:\*.*` | nothing |
| 09 | next listing entry | nothing | length (22), one entry, a "more" flag, three zeros |
| 0B | open a file for reading (*j200lx: get filename*) | name | nothing |
| 16 | prepare to read (*BSD Filer: transfer ready?*) | nothing | nothing; Zampach found it can be left out |
| 0C | read a block | `00 00`, wanted length (2) | counted block; a block shorter than asked for is the last, an empty one means end of file |
| 03 | create a file for writing; fails if it exists | name | nothing |
| 05 | create or overwrite a file | name | nothing |
| 15 | write a block | `00 00`, counted block | length written (2) |
| 17 | read the open file's date and time | `00 00` | DOS time (2), DOS date (2) |
| 18 | set the open file's date and time | `00 00`, DOS time (2), DOS date (2) | nothing |
| 13 | read the open file's position | `00 00` | position (4) |
| 0F | set the open file's position | four bytes (BSD Filer reads a two-byte "whence" in them), position (4) | nothing |
| 01 | close the open file | `00 00` | nothing |
| 0D | rename or move | old name, new name (each counted, each followed by two zeros) | nothing |
| 04 | delete a file | name | nothing |
| 0A | make a directory | name | nothing |
| 0E | remove an empty directory | name | nothing |
| 19 | read a file's attributes (file not open) | name | attribute byte, one zero |
| 1A | set a file's attributes | counted name, attribute byte, one zero | nothing |

Code 02 does not seem to exist ([Zampach protocol text]).

**Listing entry** (22 bytes): DOS attribute byte (0x10 directory, 0x20
archive, and so on), DOS time (2), DOS date (2), size (4), and a 13-byte
8.3 name ended by a zero, with leftover bytes after the zero (src:
[Zampach protocol text]; the junk after the terminator is visible in
[Zampach example trace]). The listing ends with a reply whose "more" flag
is 0; that reply repeats the previous entry and must be ignored (src:
[Zampach example trace]; [BSD Filer source] does the same). DOS packed
time: hours in bits 15-11, minutes 10-5, seconds/2 in 4-0; date: year
since 1980 in bits 15-9, month 8-5, day 4-0. Decoding one recorded
timestamp by hand (`32 6E 8E 31` in the trace) gives 14 December 2004,
13:49:36, which fits the trace's January 2005 date, so the order is time
then date.

**Recorded flows** (from the palmtop-to-palmtop session, [Zampach
example trace]):

- *Listing:* change drive, change directory, ask the current directory,
  start a listing with `C:\*.*`, then "next entry" until the flag is 0.
- *Put:* create the file (03), write blocks (15), set its date (18),
  close (01). The palmtop's own client therefore keeps dates on a copy.
- *Get:* open for reading (0B), prepare (16), read 0x800-byte blocks (0C)
  until a short or empty one, read the date (17), close (01).

### Contradictions between sources

- j200lx counts the SOH as part of the start sequence and calls the
  direction byte (`01`/`81`) a "counter" on send and a "status" on
  receive; the trace shows the counter in both directions, followed by
  the direction byte and STX ([j200lx Filer page] vs. [Zampach example
  trace]).
- j200lx shows a one-byte request for "data end" (01); the trace shows
  two zero bytes ([Zampach example trace]).
- j200lx reads the date and time as one 32-bit word and takes the
  seconds as the low six bits; in DOS format they are the low five bits
  times two ([j200lx Filer page] vs. [Zampach protocol text]).
- Zampach's text writes two zero status bytes in front of a listing
  entry; his trace and his code have four ([Zampach protocol text] vs.
  [Zampach example trace], [WFX source]).
- Zampach's text has two typos: the day of a date is taken from the time
  word, and the top byte of a file position is weighted 0x100000 instead
  of 0x1000000 ([Zampach protocol text]).
- Speeds: see the physical layer above.

## 2. What a client can do with it

| Need | Supported | How / limit |
| --- | --- | --- |
| Drives | yes | 1B lists letters; C: internal RAM disk, A: the memory card, D: the ROM disk (read-only) ([palmtops FAQ]; [200LX FAQ] shows C: and D:). Zampach's plugin "autodetects" drives, so the letter list probably includes letters with nothing behind them ([Zampach WFX instructions]). |
| List a directory | yes | name, size, date and time, DOS attributes, directories marked by the attribute. Whether `.` and `..` appear in subdirectories is unknown. |
| Wildcards | yes | DOS patterns in the listing request (`C:\*.*`); BSD Filer improved its wildcard handling, so patterns matter to the LX client ([BSD Filer readme], history). |
| Get | yes | blocks of up to 2 KB, with the file's date. |
| Put | yes | 03 refuses an existing name, 05 overwrites; set the date before closing. |
| Rename / move | yes | same drive only ([Zampach WFX instructions], limitations). |
| Delete | yes | files; directories with 0E only when empty (Zampach's API notes). |
| Make / remove directory | yes | 0A / 0E. |
| Dates kept | yes | 17/18 on the open file. |
| Attributes | yes | 19/1A (read-only, hidden, archive...). |
| Partial reads, resume | yes | 0F/13 seek and tell. |
| Long names | no | 8.3 only; LXTools treats names as case-insensitive and uses upper case on the palmtop ([lxbackup man page]); BSD Filer hides names that are not valid 8.3 or contain lower-case letters ([BSD Filer readme], limitations). |
| Speed | slow | at 19200 baud about 1.7 KB/s, so a 1 MB file takes about ten minutes (estimate: 2 KB blocks plus a reply each, no overlap). |

## 3. Kermit and XModem on the 100LX

- **DataComm** (built in) transfers with Kermit or XModem (src: [Palmtop
  Paper DataComm article], written for the 95LX). The 200LX FAQ adds
  YModem and ZModem for the 200LX's DataComm ([200LX FAQ], "What software
  can I use"); a ThinFax manual sets DataComm to ZModem on an LX
  ([ThinFax manual], via search, not read). Which of these the 100LX has
  is an owner check.
- **DataComm has a Kermit server mode:** MENU, Connect, Kermit Commands,
  then the server option; the other side then "gets" files by full DOS
  path (src: [Palmtop Paper Kermit steps]). Its subdirectories have to be
  fetched one by one.
- **Speed:** the 100LX's DataComm is notoriously slow as a terminal but
  "reasonably fast" for file transfers ([palmtops FAQ]).
- **MS-Kermit** runs on the palmtop as an alternative ([palmtops FAQ];
  [S.U.P.E.R. index]) and has a full Kermit server, but it has to be
  installed on the palmtop first.
- **Files' Kermit client cannot be reused as it stands.** `kermit-proto`
  is generic, but `satx-core`'s session and calculator layer are
  HP48-shaped: host commands are RPL (`C` packets), listings are parsed
  from the 48's `G D` text, and connect reads `VERSION` and sets flag -35
  ([[docs/saturnus-tx]], [[satx/docs/architecture]]). Over DataComm's
  server only get (R) and send (S) are certain; which REMOTE (`G`)
  commands it answers, and in what text format, is unknown. XModem
  carries one file without a name and is started by hand on the palmtop.
- **Verdict:** Kermit and XModem give "send a file" and "fetch a file by
  path", not a file manager. The Filer protocol gives the file manager:
  structured listings, drives, dates, rename, delete, directories, no
  extra software on the palmtop. Kermit stays a fallback the owner can
  already use with any terminal program.

## 4. Cable and hardware

The owner has HP's original 10-pin serial cable for the 100LX, so the
cable is settled. What matters for use:

- The 100LX introduced a small 10-pin connector with 2 mm pin spacing;
  HP's PC cable for it is the F1015A, which came with an adapter to the
  95LX's 4-pin plug (src: [Palmtop Paper serial article]; [palmtops
  FAQ]). It ends in a **female DB9 with the null modem built in**, ready
  for a PC's male DB9 port (src: [200LX FAQ], cable answer; [200LX.net
  kit page]; [Palmtop Paper CPACK article]). HP's cable table lists the
  PC-to-palmtop cable F1015-80002 as DB9 female to a 10-pin palmtop plug
  with "wide" keying ([holyjoe cable table]).
- **USB-serial adapter:** an ordinary adapter with a male DB9 (a PC-style
  DTE port, RS-232 levels: FTDI, Prolific, CH340 and so on) plugs straight
  into the cable. **No extra null modem and no gender changer.** Open the
  port at 8N1 with no flow control; DTR and RTS do not matter on a
  three-wire link (inference from the three-wire wiring above; LXTools
  ignores CTS and carrier detect, [LXTools README]).
- **Orientation:** the connector's bump and the HP logo face up; upside
  down, nothing connects (src: [HP tech support notes]).
- **The 38G cable must not be used.** The 38G/49G 10-pin connector has
  "small" keying and different wiring; the 48 cable with its 10-pin
  adaptor (1252-6635 or CONNE-80003) is for the 38G and 49G. The palmtop
  cable physically fits a 49G and destroys it ([holyjoe cable table]).
  Keep the two cables apart. The original HP48/95LX 4-pin cables cannot be
  converted for the 100LX ([Palmtop Paper serial article]).
- **Infrared:** the 100/200LX's IR runs Filer links to another palmtop at
  up to 115200 ([Palmtop Paper IR article]); a hobbyist got only 300 baud
  stable to an ESP32 ([Arduino IR thread]). Web Serial cannot drive IR,
  so it is out of scope, as it is for the calculators.
- **Self-test:** the palmtop's TEST mode (ESC held while pressing ON) has
  a "Wire serial port" check that should show "No loop" within about two
  seconds without a loopback plug ([HP tech support notes]).

## 5. Testing without the owner's device

The emulator cannot stand in (no 100LX emulation), so CI needs a test
double and development needs an oracle.

- **BSD Filer v0.3** (Dai Ishijima, 1996) emulates the palmtop's Filer
  **server**, which is exactly the side Files talks to. Its v0.3 runs on
  Linux as well as BSD and adds a `-root` option ([BSD Filer index]). The
  archive (`bsdfil03.lzh`, LHa, Shift-JIS comments, CR/LF) unpacks with
  macOS `tar`; after stripping the CRs the six C files **compiled and
  linked on macOS with `clang -std=gnu89`** without changes (checked
  2026-11-08 in the scratchpad; not run). In Docker: a Debian image with
  gcc in gnu89 mode, `socat` making a pseudo-terminal and bridging it to a
  TCP port, `bsdfiler -speed 38400 -root /srv /dev/pts/N`. Files'
  existing TCP link (or satx's `tcp://` transport) then reaches it the
  way satx reaches saturnng. Port: 4848-4852 belong to the saturnng
  containers and 4860-4869 to our own bridges, so the oracle needs a new
  port (owner's choice, for example 4853).
  - Limits as an oracle: it implements 23 of the 28 commands (not 05
    overwrite, 07 current drive, 0E remove directory, 19/1A attributes);
    unknown commands get no answer; delete renames to `.BAK` instead of
    deleting; only drive C:; minimal error replies ([BSD Filer source];
    [BSD Filer readme], limitations). It shows what the palmtop's
    **client** sends and expects, which is the subset that matters most,
    but it is a reimplementation, not HP's code, so it cannot settle
    error codes or the commands it lacks.
  - Licence: freeware, not open source in the OSI sense; free use and
    porting for free software and personal use, no commercial use,
    unmodified redistribution ([BSD Filer readme], disclaimer). Like the
    saturnng image, the oracle image is local only, never pushed, and
    the archive is fetched by the build, not committed.
- **W32 Filer** (Windows NT/95, freeware; [S.U.P.E.R. index]) and
  **WCEFiler** (Windows CE, "open source" per [WCEFiler page]) are further
  server emulators by Eiichiro Ito; neither helps on macOS/Linux.
- **HP's Connectivity Pack** (DOS) includes a desktop Filer that worked as
  either end against the palmtop ([Palmtop Paper CPACK article]; BSD Filer
  was tested with it as client, [BSD Filer readme]). Run in DOSBox with a
  serial port mapped to TCP, it would be HP's own server code and so the
  stronger oracle. It is a proprietary HP binary: only with the owner's
  own copy, kept outside the repository like the ROMs, and only if he
  wants it. Whether its desktop Filer can act as a *server* is
  unverified.
- **LXTools** (`lxcopy`, `lxdir`, `lxbackup` ...; GPL-2.0, maintained at
  [lxtools-ng]) is a **client**, so it cannot be an oracle for Files'
  client; it could test our own server (point 6) black-box.
- **In CI:** `filer-proto` gets a server state machine of its own (useful
  anyway, see the server-mode idea below), and the client is tested
  against it in process; the BSD Filer runs and the owner's device
  supply recorded traces. Do not copy Zampach's recorded trace into tests
  (it ships in a GPL package); record our own.

## 6. Fit into saturnus

Architecture today ([[docs/saturnus-tx]], [[satx/docs/architecture]]):
`kermit-proto` and `xmodem-proto` are sans-IO, `satx-core` adds the HP48
layer and transports, `saturnus-tx`'s `Device` drives one calculator
through `satx-core`'s `Machine` (HP48-specific: flag -35, RPL names,
speed and parity search), and the web page's `Link` (Web Serial, native
serial in the desktop app, the emulator) carries bytes.

Proposed:

1. **`crates/filer-proto`** (sans-IO, wasm, no I/O, no threads), same seam
   as `kermit-proto` (`start`, `handle_input`, `handle_timeout`,
   `poll_output`, `poll_event`, `next_timeout`): frame encoder/decoder
   (sync hunt, DLE doubling, CRC), typed requests and replies for the 28
   commands, a client that numbers packets, retries with the same
   counter and times out, and a server state machine over an abstract
   file store (the CI test double, and the base for a "serve a folder"
   mode).
2. **A palmtop device in `saturnus-tx`** beside the Kermit device and the
   aplet server, not inside `Device`: operations list (per drive), get,
   put (create first; on a clash ask, then overwrite), rename, delete,
   make and remove directory, set date; 8.3 name checks and upper case.
   Connect: try CONNECT at the chosen speed; optionally a search like the
   Kermit device's (CONNECT is harmless), fastest first. Block size
   adaptive (fall back from 2048 to smaller blocks after errors, since
   the palmtop has no flow control and a 1-byte-FIFO UART).
3. **Files UI:** a third kind in "Add a calculator": "An HP 100LX or
   200LX palmtop". The pane shows the drives as top-level folders (D:
   read-only), columns name, size, date; no checksum column; the
   connect sheet tells the user to open Filer with server mode on and
   shows the speed. Copy that says "calculator" becomes "device" where
   the palmtop appears. Web Serial: open with no flow control and a
   receive buffer of several KB (the default 255 bytes is small for 2 KB
   replies at high speed).
4. **satx:** a small `satx lx` subcommand group (`ls`, `get`, `put`,
   `mv`, `rm`, `mkdir`, `rmdir`) reusing satx-core's serial and TCP
   transports. Cheap once the crate exists, and it is what the e2e
   script against the BSD Filer container would drive. Gate an e2e test
   on an address variable the way `SATX_E2E_ADDR` does.
5. **Later, optional: serve a folder to the palmtop**, the reverse role
   (the palmtop's Filer as client, F6 Remote), like BSD Filer and like
   Files' aplet "disk drive" for the 38G. Only the real palmtop can test
   that direction.

**Recommendation: inside the one app**, not a dedicated "100LX Files".
The one-app decision of 0.6.0 ([[decision-log]]) already holds two very
different families (48/49 as servers, 38G/39G/40G as clients); the
palmtop is one more kind of device behind the same `Link`, panes, drag
and drop, desktop app and deployment. A separate app would duplicate the
shell, the desktop build, the cask and the deploy for one owner, and
contradict that decision. The only cost inside Files is wording
("calculator" in the add flow) and a drive level in the pane.

## 7. Effort and risks

**Size:** about three iterations, plus one session with the device.

1. `filer-proto`: framing, CRC, codec, client and server state machines,
   unit tests (each command, DLE in data and in the CRC, retries, end of
   listing, short last block).
2. Oracle and CLI: the BSD Filer Docker image with a TCP serial bridge,
   `satx lx` subcommands, an e2e test against the container, wiki page
   for the 100LX/200LX and its protocol.
3. Files: palmtop device in `saturnus-tx`, the third kind in the add
   flow, the drive level, overwrite question, desktop app; page tests
   against the in-process server.
4. With the owner (not a full iteration): real-device run, error codes,
   speed limit, fixes. Serving a folder to the palmtop would be one more
   iteration if wanted.

**Risks and unknowns that need the real device:**

- Error replies: their codes and where they sit; behaviour on a missing
  file, an existing name with 03, a full disk, a non-empty directory, a
  write to D:.
- The highest reliable speed (the FAQ's 19200 vs. the 115200 setting) and
  whether 2 KB blocks overrun the palmtop's UART at high speed.
- Whether the four NULs before the first packet are needed; whether the
  palmtop needs anything besides Filer open with server mode on; whether
  it stays awake (auto-off) while serving and what a connect does to a
  sleeping palmtop.
- What the drive list returns on the 100LX (all of `ABCDE`?), how an
  empty card slot answers, whether `.` and `..` are listed, whether
  hidden and system files are listed.
- Whether 16 ("prepare to read") is ever required.
- Whether the 95LX speaks the same protocol (LXTools has a separate 95LX
  package, [S.U.P.E.R. index]); out of scope unless the owner has one.
- Licence hygiene: three of the four detailed sources are GPL or
  non-commercial freeware; the implementation must come from the wiki
  page written from this note, with tests on our own recordings.

## What the owner needs to check by hand

1. **Cable:** the HP 10-pin cable (female DB9, null modem built in)
   straight into a USB-serial adapter with a male DB9. Nothing in
   between. Bump and HP logo up at the palmtop. Never the 38G cable or
   adaptor on the palmtop, never the palmtop cable on a 38G/49G.
2. **Filer settings:** Filer, MENU, Communications, Remote Settings:
   speed 19200 to start (later 38400, 57600, 115200), interface COM1
   (wire, not IR), server mode enabled; OK. Leave Filer open in front.
3. **First contact:** with BSD Filer or any test build, note what the
   palmtop's screen shows when the PC connects and disconnects.
4. **DataComm:** which transfer protocols its menu offers on the 100LX,
   and whether MENU, Connect, Kermit Commands has the server option.
5. **The unknowns above:** the error cases, the fastest speed that holds
   for a 1 MB file each way, auto-off while serving, the drive list with
   and without a card.

## Sources

Licences as stated by each source.

- [j200lx Filer page]: Rodrigo Serra, "Filer protocol", j200lx project,
  2002. <https://j200lx.sourceforge.net/Filer.html>. Project licence on
  SourceForge: LGPL-2.0; the page itself states none.
- [Zampach protocol text], [Zampach example trace], [Zampach WFX
  instructions], [WFX source]: Pavel Zampach, WFX-HPLX 1.2 (Total
  Commander plugin), 2005: `protocol/Filer protocol.txt` (Serra 2002,
  Zampach 2003-2005), `protocol/example.TXT` (recorded LX-to-LX session),
  `Instructions.html`, `source/Filer.pas`. GPL-2.0-or-later. Listed on
  S.U.P.E.R. as `wfx_hplx.zip`; the mizj.com copy is gone (404), read from
  the Wayback Machine:
  <http://web.archive.org/web/2010id_/http://mizj.com/wfx_hplx.zip>.
  This is the package S.U.P.E.R. describes as "the most complete
  description of the Filer communications protocol - 13 new commands
  compared with the PAL library".
- [S.U.P.E.R. index]: S.U.P.E.R. archive index, <http://mizj.com/>
  (entries WFX-HPLX, HPLXFILE 2.0 by the same author, LX Tools, LXTOOLS
  for 95LX, W32 Filer, J200LX, MS Kermit).
- [BSD Filer readme]: Dai Ishijima, BSD Filer v0.2 readme (Japanese),
  1996, <http://vvvvvv.wheel.gr.jp/~dai/software/bsdfil02-readme.html>.
  Freeware under the author's own terms (non-commercial, unmodified
  redistribution).
- [BSD Filer index]: the author's BSD Filer page with v0.3 (Linux
  support, `-root`, `-speed`),
  <http://vvvvvv.wheel.gr.jp/~dai/software/lx-filer.html>.
- [BSD Filer source], [BSD Filer man page]: `bsdfil03.lzh` from that page
  (`bsdfiler.h`, `emul.c`, `packet.c`, `crc.c`, `bsdfiler.man`), same
  terms. Read for behaviour only.
- [LXFS readme]: Dai Ishijima, LXFS4D v0.0 (DOS port of BSD Filer),
  <http://vvvvvv.wheel.gr.jp/~dai/software/lxfs4d00-readme.html>.
- [lxbackup man page]: Edward A. Falk, lxbackup(1), LXTools, GPL,
  <https://man.cx/lxbackup(1)> (credits Harry Konstas for working out the
  format and Andreas Garzotto for the LXTools functions).
- [LXTools README], [lxtools-ng]: LXTools 1.0a-1.1e README, GPL-2.0,
  <https://github.com/gonter/lxtools-ng>.
- [WCEFiler page]: HPC:Factor, WCEFiler 0.06 by Eiichiro Ito,
  <https://www.hpcfactor.com/scl/1322/Eiichiro_Ito/WCEFiler/version_0.06>.
- [palmtops FAQ]: Richard Cochran, comp.sys.palmtops HP100LX FAQ,
  <https://ftp.lip6.fr/pub/doc/faqs/hp/palmtops-faq>.
- [200LX FAQ]: 200LX Frequently Asked Questions,
  <https://mail.retroisle.com/others/hp95lx/Articles/hp200lx_faq.php>.
- [Palmtop Paper Kermit steps]: "Steps for Kermit file transfer",
  <https://www.shot.org/mirrors/hplx/www.palmtoppaper.com/ptphtml/17/pt17002f.htm>.
- [Palmtop Paper IR article]: "Using Infrared communications between HP
  Palmtops",
  <https://www.shot.org/mirrors/hplx/www.palmtoppaper.com/ptphtml/18/pt180033.htm>.
- [Palmtop Paper CPACK article]: Connectivity Pack defaults and Filer
  connection (from search results; pages under
  <https://www.shot.org/mirrors/hplx/www.palmtoppaper.com/ptphtml/17/pt17000b.htm>
  and <https://www.shot.org/mirrors/hplx/www.palmtoppaper.com/ptphtml/13/pt130049.htm>;
  the null-modem statement and the 9600/COM1/server-mode defaults were
  read in search summaries, not on the pages: re-check before the wiki).
- [Palmtop Paper serial article]: Mark Scardina, "Serial Port
  Communications ... Made Easy!",
  <https://www.shot.org/mirrors/hplx/www.palmtoppaper.com/PTPHTML/15/pt150056.htm>.
- [Palmtop Paper DataComm article]: "How do you use DataComm" (95LX),
  <https://palmtoppaper.com/ptphtml/1/ptp10048.htm> (from a search
  summary; re-check).
- [HP tech support notes]: "Connectivity Pack Notes from HP Tech
  Support",
  <https://www.shot.org/mirrors/hplx/www.palmtoppaper.com/PTPHTML/16/pt160048.htm>.
- [200LX.net kit page]: <https://200lx.net/connkit.htm>.
- [holyjoe cable table]: Joseph K. Horn, "The HP Cable Table",
  <https://holyjoe.net/hp/cable.htm>.
- [Arduino IR thread]:
  <https://forum.arduino.cc/t/building-a-device-to-communicate-via-infrared-with-an-hp-100lx-palmtop-computer/693343>
  (from a search summary).
- [ThinFax manual]:
  <https://www.manualsdir.com/manuals/329454/exp-computer-thinfax-lxm.html?page=26>
  (from a search summary, not read).

Not reached: the HP Museum threads on LX cables and the 95LX
(hpmuseum.org answered 403); the 100LX User's Guide (its DataComm and
Filer chapters would settle the speed list and DataComm's protocols).
