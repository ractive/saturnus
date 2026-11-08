---
title: "HP 100LX Filer protocol: what HP's Connectivity Pack shows"
type: research
status: completed
date: 2026-11-08
tags:
  - hp100lx
  - serial
  - filer
  - files
---

# HP 100LX Filer protocol: what HP's Connectivity Pack shows (2026-11-08)

Companion to [[research/hp100lx-filer]]. That note collects the published
sources and lists what they leave open. This page answers those open points
from a static analysis of HP's Connectivity Pack for the HP 100LX (the PC
programs), made on 2026-11-08. It gives facts only, in our own words: byte
values, timings, state changes. It contains no HP code, no decompiler
output, no routine names and no addresses.

**Certainty marks.** "Read" means seen directly in HP's code. "Inferred"
means concluded from usage or consistency, not traced end to end. Facts
about the palmtop come from HP's PC build of the Filer and are inferred
for the palmtop itself unless stated otherwise.

**What matters most:**

1. **The PC Filer holds both ends of the protocol**: the client and a
   complete Filer server. Its internal structure (including an empty stub
   for powering the 95LX's serial port) shows it is the palmtop Filer's
   code built for DOS (read). **Inferred:** the palmtop's built-in Filer
   server behaves like the PC Filer's server.
2. **The fifth header byte, which the published sources call "STX", is a
   protocol revision**: 2 normally, 1 for older peers. The client falls back
   to 1 by itself (read).
3. **The "00 00" in front of file requests is the file handle.** The server
   has 8 handle slots, and the reply to an open or create gives the slot
   number (read).
4. **The four "status bytes" are two words**: a protocol status word, then
   a result word (file-system errors). File errors are small positive
   numbers (200 to 222); protocol errors are negative words (FF38h to
   FF3Eh) (read).
5. **Command 16h is not "prepare to read".** It drops the most recent
   listing context and can always be left out (read).
6. **The two CRC bytes are never DLE-doubled** (read).
7. **No flow control, no FIFO, DTR and RTS on, 8N1.** Speeds 1200 to
   115200 (read).

## Method

- **Source:** HP's Connectivity Pack for the HP 100LX, the PC programs
  (the owner's copy from archive.org, item "HP100LX-Connect"). Analysed:
  the PC Filer (the program that holds the Filer client and server) and
  the communications kernel (the resident layer that plays the palmtop's
  System Manager on the PC: a UART driver and DOS file services). The
  redirector server and client of the pack were only classified and
  scanned for strings.
- **How:** static analysis only (disassembly and decompilation, plus a
  model of HP's CRC routine checked against CRC-16/ARC). Nothing was
  executed. The PC Filer carries debug information, so the roles of its
  parts are identified, not guessed.
- **The redirector** (the pack's "Redirector Server" and its client)
  speaks a **different** protocol: DLE STX frames with no SYN preamble and
  a byte-sum checksum. It is not the Filer protocol and plays no further
  part here (read).
- On the PC the Filer reaches the serial port and the disk only through
  the communications kernel. How the Filer's service calls map to the
  kernel's routines was matched by behaviour, not traced (the dispatcher
  sits in an overlay).

### Wire format, as the code builds and parses it (read)

- **Transmit:** three SYN (16h), DLE STX (10h 02h), the frame body with
  every 10h sent twice, DLE ETX (10h 03h), then the CRC low byte and the
  high byte. **The CRC bytes are sent raw, never doubled.** Example:
  CONNECT from a client is `16 16 16 10 02 01 40 00 01 02 10 03 10 BF`;
  the CRC's low byte is 10h here and goes out once.
- **Frame body:** request header `01, command, counter, 01, revision`.
  Reply header `01, command, counter, 81, revision`, then the 16-bit
  status word, then the command's data.
- **CRC:** CRC-16/ARC (reflected polynomial A001h, start 0, no final XOR).
  It covers every body byte once (a doubled DLE counts once) and the ETX.
  It does not cover the SYNs, the leading DLE STX or the DLE before ETX. A
  model of HP's table-free CRC routine agreed with CRC-16/ARC on 2,000
  random inputs.
- **Receive:** the receiver ignores everything until a SYN, accepts any
  number of further SYNs, then needs DLE then STX. **At least one SYN is
  required**; a DLE straight after idle line noise is not accepted. Inside
  a frame, DLE DLE is one data byte and DLE ETX ends the frame. DLE
  followed by anything else (DLE 05h included) drops the frame, and so
  does a body longer than 2,080 bytes (820h). The two bytes after DLE ETX
  are compared with the CRC, first with the low byte, then with the high
  byte. A mismatch drops the frame silently; error counts are kept
  internally and never sent.
- **Side effect (inferred):** if a frame is cut short and the next one
  starts while the receiver is still inside the first, the second frame's
  DLE STX counts as an illegal DLE sequence. Both frames are lost, and the
  sender's retry recovers.

## 1. Error replies

**Reply layout (read, the PC Filer's server):** after the 5-byte header,
every reply carries:

1. **Status word** (2 bytes): 0000h when the server accepted and ran the
   request. A non-zero value is a protocol-level refusal, and no further
   data follows.
2. **Result word** (2 bytes): the file-system outcome, 0 for success or a
   system error code.
3. The command's values (handle, drive letter, counted string, block and
   so on).

CONNECT and DISCONNECT replies carry only the status word. That is why the
published sources see "two status bytes" there and "four" elsewhere.

**HP's client looks only at the result word for ordinary commands**; it
reads the status word only after CONNECT (read).

### Protocol-level codes (status word, or client-internal) (read)

| Value | Signed | From | Meaning |
| --- | --- | --- | --- |
| FF3Bh | -197 | server | An idle server got a frame that is not a valid CONNECT (wrong command, counter not 0, or wrong direction byte). It answers with a CONNECT reply (command 40h, counter 0) with this status. A session also ends with this code when a frame lacks the leading 01h. |
| FF3Ch | -196 | server | Counter out of sequence: neither a repeat of the last counter nor the next one. Not run. The reply carries the server's last good counter. |
| FF3Dh | -195 | server | All 8 file slots in use when an open or create arrives. **The server leaves the session without replying.** |
| FF3Eh | -194 | server | Unknown command (outside 01h to 1Bh, or 40h/41h with a non-zero counter). Status only, no data. |
| (none) | | server | **Command 02h gets no reply at all**, although the server has already advanced its counter. |
| FF38h | -200 | client only | Timeout: the transmit queue made no progress for 5 s. |
| FF39h | -199 | client only | No reply, or no complete valid frame, in time (after all retries). |
| FF3Ah | -198 | client only | The reply's counter differs from the request's, or its direction byte is not 81h. **No retry.** |

The client shows every code from -200 to -97 as "Remote connection
broken". Before that, any transport-level failure makes the client send
DISCONNECT and mark the session closed (read).

### File-system codes (result word) (read for the PC; inferred for the palmtop)

The result word carries the System Manager's file error codes. On the PC
the communications kernel produces them, mostly by translating DOS errors
(read). The meanings come from lining up the DOS errors that map to each
code with the order of the system's error texts (inferred, but every DOS
error lands on a fitting text).

| Code | Hex | Meaning | Typical cause on the PC |
| --- | --- | --- | --- |
| 200 | C8h | invalid file or path name | a control character, a space, any of `"/[]\|<>+=;,`, or a colon not after the drive letter; `\`, `*` and `?` are allowed |
| 201 | C9h | cannot find system directory | |
| 202 | CAh | too many alarms | |
| 203 | CBh | **disk full** | a write wrote fewer bytes than asked |
| 204 | CCh | print queue error | |
| 205 | CDh | sharing or network error | DOS sharing and network errors |
| 206 | CEh | I/O device error | DOS device and media errors |
| 207 | CFh | internal software error | catch-all for DOS errors without a better match |
| 208 | D0h | **file does not exist** | DOS "file not found" |
| 209 | D1h | **directory (path) does not exist**, invalid drive | DOS "path not found", "invalid drive" |
| 210 | D2h | too many open files | |
| 211 | D3h | access denied ("file creation error") | read-only file, non-empty directory, creating a file over an existing directory |
| 212 | D4h | cannot delete the current directory | |
| 213 | D5h | cannot move a file to another drive | |
| 214 | D6h | **disk write-protected** | |
| 215 | D7h | drive not ready | |
| 216 | D8h | printer out of paper | |
| 217 | D9h | **file already exists** | create-new (03h) on an existing file |
| 218 | DAh | cannot create directory | |
| 219 | DBh | critical-error handler failure | |
| 220 | DCh | too many redirections | |
| 221 | DDh | duplicate redirection | |
| 222 | DEh | insufficient memory | |

DOS's "no more files" never reaches the wire as an error; it becomes the
end-of-listing flag (read). The D1h that BSD Filer puts in "the third
status byte" when a directory change fails is the low byte of result 209,
which agrees with this table. What the palmtop reports for a write to D:
(the ROM disk) depends on its disk driver: 214 or 211 (not determinable
from PC code).

### How HP's client reacts (read)

- **0:** success.
- **217 after create-new (03h):** asks "overwrite?"; on yes it repeats the
  copy with create-or-overwrite (05h).
- **211 after create-new (03h):** tests the name with 08h. If it is a
  directory, says a directory cannot be overwritten with a file; otherwise
  shows the error text.
- **214 after rename (0Dh):** shows the error, then reads the attributes
  (19h), clears the read-only bit and writes them back (1Ah).
- **Any other non-zero result:** a message box with the system text for
  that code, then the window is re-read.
- **During a copy:** a non-zero read or write result stops the loop; both
  files are closed and the partly written destination is deleted.

## 2. Timeouts and retries

**Client (read; times counted in PC clock ticks of 1/18.2 s):**

Each request is attempted up to **3 times**. Each attempt:

1. Discards both serial buffers (transmit and receive).
2. Sends the frame.
3. Waits until the transmit queue has drained: indefinitely while it keeps
   moving; after 5.0 s (91 ticks) without progress it gives up with -200,
   without retrying.
4. Waits for the **first byte** of a reply: **2.0 s (36 ticks) for
   CONNECT, 5.9 s (108 ticks) for every other command.**
5. Once a byte has arrived, the **whole frame** must arrive within a
   window that depends on the speed:

   | Speed | Window |
   | --- | --- |
   | 19200 and faster | 2.0 s (36 ticks) |
   | 9600 | 5.0 s (91 ticks) |
   | 4800 | 5.9 s (108 ticks) |
   | 2400 | 11.9 s (216 ticks) |
   | 1200 | 24.7 s (450 ticks) |
   | 300 | 79 s (1440 ticks) |

6. No first byte, or no complete valid frame (CRC errors included), is a
   failed attempt (-199), and the next attempt follows.

- **What is resent:** the identical frame, so the same command and **the
  same counter**. The counter advances only after a matching reply.
- **Reply check:** the reply's counter must equal the request's and its
  direction byte must be 81h; otherwise the call fails at once with -198,
  no retry. The reply's command and revision bytes are not checked.
- **No NAK, ACK or other negative acknowledgement.** A receiver drops bad
  frames silently; the only recovery is the sender's timeout.
- **Server (read):** a request with the **same counter as the last one**
  is a retransmission: the server resends its previous reply byte for byte
  and does **not** run the command again. A request with the next counter
  is run. Anything else gets status -196. This makes a resent read or
  write safe.
- **After three failures** the client sends DISCONNECT and drops the
  session.
- **No keep-alive.** The client sends nothing while idle. The server waits
  forever for the next frame, polling only for Ctrl+Break. All time limits
  belong to the client.

## 3. Connection start and end

**Opening (read):**

1. Open and set up the port (see 4). If a phone number is set, dial first
   (modem path, not relevant for a cable).
2. Send CONNECT: command 40h, counter 0, no data, revision 2. HP's client
   sends **no NUL bytes and no other preamble** besides the three SYNs.
   Zampach's four NULs are his own addition; the receiver does not need
   them, because it hunts for SYN.
3. CONNECT uses the normal request routine (3 attempts, 2 s each for the
   first byte). If that fails, the whole thing is repeated once: **up to
   6 transmissions, about 12 s**.
4. If the reply's status is FF3Bh on the first round, the client switches
   to **revision 1** and tries again (another up to 6 transmissions).
5. Success means status 0000h. The client's counter then moves to 1, so
   **the first real request carries counter 1**.

- **No speed negotiation or other handshake.** Both sides must already be
  set to the same speed.
- **Revision 1 (read):** against a revision-1 server the client does not
  send the drive-list request; it assumes C: is the only drive.
- **Server's view (read):**
  - While idle, a valid CONNECT (counter 0, direction 01h) is answered
    with status 0 and the server's revision (2 by default), and the
    session starts with all 8 file slots free and the counter at 0.
  - A CONNECT inside a session resets it: counter 0, all slots marked free
    (files left open are not closed), status 0. Here the server also takes
    over the client's revision byte: 1 means revision 1, anything else 2.
- **Palmtop not in server mode (read for the PC Filer; inferred for the
  palmtop):** the Filer looks for CONNECT only while server mode is on.
  Otherwise frames are ignored and nothing is sent back. The client sees
  about 12 s of silence, then "Cannot establish a remote connection". Even
  after a failed connect it sends one DISCONNECT.
- **Server mode on, no session open:** any frame is answered with a
  CONNECT reply (command 40h, counter 0) carrying FF3Bh, whatever the
  command. A client that sends a normal request to a server that has
  silently left the session (for example after Ctrl+Break on the palmtop)
  therefore gets a reply with the wrong counter; HP's client reports -198
  and disconnects.
- **Ending (read):** DISCONNECT is command 41h, counter 0, no data:
  `16 16 16 10 02 01 41 00 01 02 10 03 2D 7F`. It is sent **once**. The
  client **does not wait for or check the reply**; it pauses 1 s. The
  server does answer (status 0) and returns to normal Filer operation,
  still in server mode and ready for the next CONNECT.
- **Other ways a session ends on the server:** Ctrl+Break on the server
  (nothing sent), running out of file slots (FF3Dh, nothing sent), or a
  frame without the leading 01h. Each time the server closes all its open
  files.

## 4. Speeds and UART setup

**Speeds offered (read):** 1200, 2400, 4800, 9600, 19200, 38400, 57600,
115200, stored as UART divisors for a 1.8432 MHz clock (96, 48, 24, 12,
6, 3, 2, 1). The timeout table also knows 300 baud (divisor 384), which
the dialog does not offer.

**Line settings the Filer asks for (read):** no parity, 1 stop bit, 8
data bits; **no flow control** (XON/XOFF and CTS both off); interface
"wire".

**How the communications kernel programs the 8250/16450 (read):**

- At open it saves the UART's interrupt enable, line control, divisor and
  modem control registers and restores them at close. It sets the modem
  control register to 0Fh (**DTR, RTS**, OUT1 and OUT2 on), line control
  to 8N1, and enables the receive, transmit-empty, line-status and
  modem-status interrupts. COM1 is 3F8h on IRQ4, COM2 2F8h on IRQ3.
- **The FIFO is never touched**: the driver runs the UART as a plain 8250.
  **Speed aid:** at 19200 and above, after each interrupt the handler
  keeps polling the UART for a while so that bytes arriving back to back
  are not lost.
- **Flow control** exists in the driver (XON/XOFF, and RTS/CTS by watching
  CTS and dropping RTS) but is **off for the Filer**. DTR and RTS stay on
  throughout; CTS, DSR and DCD are ignored. A three-wire cable is enough,
  and holding DTR/RTS high from a USB adapter is harmless.
- Receive and transmit buffers hold 4,353 bytes each.
- **A second interface type** (inferred: the palmtop's infrared) switches
  on a half-duplex mode: after sending, the driver waits until the last
  bit is out, discards about 50 echoed bytes and then listens again. The
  hardware hook for it is empty in the PC build. Not relevant for the
  cable.

## 5. The drive list

- **Request (read):** command 1Bh with **no data**. The published "00 00"
  is not needed; the server ignores request bytes for this command.
- **Reply (read):** status 0000h, result word, a **count that includes a
  terminating zero**, the drive letters (one byte each, upper case, no
  separators), the zero byte, then 4 zero bytes. For drives A, C, D:
  count 4, `41 43 44 00`, `00 00 00 00`.
- **Letters only. There are no per-drive flags** (no read-only, removable
  or empty indication).
- **The PC server's list (read; inferred to be the service the server
  uses):** a drive from A to Z is listed when DOS knows it; of a phantom
  pair (A:/B: on a one-floppy PC) only the active one. **There is no media
  check**, so a drive with no disk is still listed. If the palmtop does the
  same, an empty card slot (A:) is listed and the error shows only when
  the client lists that drive (215 "drive not ready", inferred).
- **HP's client (read):** turns each letter into an "X:" entry, sorts them
  and shows them under "Available Drives" without probing them. Against a
  revision-1 server it uses C: alone.

## 6. Command 16h, the other commands, and HP's sequences

### What the request words really are (read)

- **File handle.** The first word of 01h, 0Ch, 0Fh, 13h, 15h, 17h and 18h
  is a **handle slot number (0 to 7)**. The reply to 03h, 05h and 0Bh
  returns that number in the word after the result. The first file opened
  in a session gets 0, hence the "00 00" in the traces.
- **The word after a name** is a parameter, not padding:
  - 03h/05h: file attribute (HP sends 0).
  - 0Bh: open mode (HP sends 0).
  - 12h: search attribute (HP sends 0; the PC server ignores it, see
    below).
  - 1Ah: the new attribute.
  - 04h, 08h, 0Ah, 0Dh, 0Eh, 10h, 19h: passed through, 0 from HP.
- **No data at all** for 07h, 09h, 14h, 16h and 1Bh from HP's client; the
  server ignores anything sent.

### Commands with corrected or new meanings (read)

- **16h: drop search context.** The server keeps a ring of 10 search
  contexts. 12h moves to the next context and records the pattern, 09h
  reads the next entry from the current context, and 16h steps back to
  the previous context. Its reply is always status 0, result 0, value 0,
  so **leaving it out is harmless**: the ring simply wraps after 10
  listings. HP's client sends 16h only after a one-entry lookup or a
  nested scan: after 12h + 09h on a source file's name when copying (for
  its size), and after recursive directory scans (copy or delete of a
  tree, the tree view). Never after an ordinary window listing. This fits
  the "0B, 16, 0C" in Zampach's palmtop trace: the palmtop's client
  evidently does the same size lookup.
- **12h starts a listing but finds nothing yet** (inferred from the PC
  server's search, which records "first" and then "next"). The first 09h
  does the actual first search, so errors such as a missing directory may
  show only on the first 09h.
- **09h reply (read):** status, result, length 22, the 22-byte DOS find
  record (attribute, time, date, size, 13-byte name), then a 4-byte
  "found" value: 1 when the entry is valid, 0 when there are no more. With
  0 the 22 bytes are left over from the previous entry; that is the
  repeated entry in the traces.
- **What a listing contains (read on the PC side):** the search always
  asks DOS for normal files and directories, whatever the request says.
  So normal, read-only and archive files and **subdirectories** are
  listed; **hidden and system files and the volume label are not**. "."
  and ".." are not filtered, so in subdirectories they appear as DOS
  returns them. The palmtop's own server should behave the same
  (inferred). HP's client shows at most 256 entries (inferred from its
  "Incomplete listing" message; the limit check was not traced).
- **02h:** not implemented; no reply (see 1). Commands above 1Bh: status
  FF3Eh.
- **0Fh/13h** (seek/tell): used only by the file viewer, never by copy.
- **15h reply:** the result word is 0 or an error. **The word after it has
  no defined value** in HP's PC server, so the published "length written"
  is not real. HP's client checks only the result word.
- **0Ch reply:** status, result, count, then count bytes.

### Sequences HP's client sends (read)

- **Connect:** 40h (see 3).
- **Opening a remote window:** 07h (current drive), 06h (that drive's
  current directory), then the change-directory sequence below.
- **Change directory:** 11h set drive, 10h change directory, 11h again,
  06h to read back the canonical path.
- **Drive information:** 11h, 14h (label and free bytes).
- **Drive list:** 1Bh.
- **Listing:** 12h with "path\pattern" (default pattern `*.*`), then 09h
  until the found value is 0. No 16h.
- **Get (palmtop to PC):**
  1. Create the local file.
  2. **0Bh** open the remote file (mode 0).
  3. **12h** with the remote file's name, **09h** once (size, for the
     progress display), **16h**.
  4. **0Ch** with length 800h, repeated **until a block of length 0**. A
     short block does not end the loop, so one extra empty read always
     follows.
  5. **17h** read the date and time.
  6. Set the local date and time, then **01h** close.
- **Put (PC to palmtop):**
  1. **03h** create-new (attribute 0); on 217 ask the user, then **05h**
     create-or-overwrite.
  2. **15h** blocks of up to 800h.
  3. **18h** with the local file's date and time.
  4. **01h** close.

  On a failure the partial file is deleted with 04h.
- **Rename:** 08h on the new name; if it exists, the client refuses
  ("duplicate filename"). Otherwise **0Dh** (old name, word 0, new name,
  word 0). For result 214 see 1.
- **Delete:** **0Eh** for a directory, **04h** for a file. Trees are walked
  with 12h/09h/16h first.
- **Make directory:** **0Ah**. **Disconnect:** 41h.

Every command from 01h to 1Bh except 02h, plus 40h and 41h, is sent by
HP's client somewhere.

## 7. The contradictions between sources

- **Counter and direction byte (read):** the trace's reading is right. The
  header is SOH, command, counter, direction (01h request, 81h reply),
  then the **revision** byte (2, or 1). A reply repeats the request's
  command and counter. j200lx's "counter/status" reading of the direction
  byte is wrong, and every published source misreads the revision byte as
  STX.
- **Data-end request (read):** the trace is right. j200lx's one-byte "data
  end (01)" is the close command (01h); its "00 00" is the two-byte handle
  word.
- **Date and time (read):** 17h returns, and 18h takes, a 4-byte value
  whose low word is the DOS time and high word the DOS date; on the wire
  (little-endian) time, then date. Standard DOS packing: seconds/2 in bits
  4 to 0, minutes in 10 to 5, hours in 15 to 11; day in bits 4 to 0, month
  in 8 to 5, years since 1980 in 15 to 9. j200lx's 6-bit seconds are wrong.
- **Status bytes before a listing entry (read):** four, the status word
  plus the result word, followed by the length word 22. Zampach's "two" is
  wrong; his trace and code are right.
- Zampach's two slips (day taken from the time word; the 0x100000 weight)
  are typos. The code uses ordinary DOS dates and plain 32-bit
  little-endian positions and sizes.

## 8. Auto-off

- **The client does nothing to keep the palmtop awake.** It sends no
  keep-alive and no traffic while idle (read, see 2).
- The PC Filer contains a routine that calls the palmtop BIOS's auto-off
  timer service, but **in the PC build nothing calls it** (read). A
  neighbouring routine that powers the serial port on the 95LX is an empty
  stub here, yet it is called on connect and on every server poll.
  **Inferred:** the palmtop's own Filer build fills these in, and the
  palmtop resets its auto-off timer itself while it serves, which would
  explain why HP's client never needed a keep-alive. Not provable from PC
  code; a real-device check is needed.
- Two system calls bracket the server session and the clean-up after a
  failed connect. Their purpose (for example holding off task switching or
  auto-off) could not be determined; their implementation sits in a part
  of the communications kernel that was not traced.
- **Suggestion for Files:** if the palmtop turns itself off during long
  idle periods, a harmless request every minute or so (07h, current drive)
  keeps the link alive without side effects; the server answers it like
  any request.

## What stays open

- Whether the palmtop's ROM server matches the PC Filer's server in every
  detail (error codes for the ROM disk D:, an empty card slot, hidden
  files). The code base is shared, but the palmtop runs its own System
  Manager.
- The highest speed a stock 100LX holds without a FIFO. HP's own PC driver
  needed extra polling at 19200 and above.
- Auto-off while serving (see 8), and what the system calls around opening
  and closing the port and around the server session do.
- The redirector's separate protocol was not analysed.
