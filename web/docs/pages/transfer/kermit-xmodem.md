---
title: Kermit and XModem
description: The two transfer protocols built into HP's Saturn calculators: which model speaks which, the server and client roles, binary and ASCII transfers, translation modes, the settings in IOPAR, and how to set up a Kermit program on the computer.
section: transfer
order: 2
---

The HP calculators move files over their serial port with Kermit, a protocol from Columbia University. The 48G series, the 49G and the 39G and 40G also speak XModem. Both sides need the same speed and the same transfer mode; most failed transfers come from a mismatch there.

## Which calculator speaks which

| Model | Kermit | XModem | Who starts a transfer |
|---|---|---|---|
| HP 48S, 48SX | yes, with a server | no | the computer, with the calculator in SERVER |
| HP 48G, 48G+, 48GX | yes, with a server | XSEND and XRECV: 128-byte blocks, checksum, binary only | either |
| HP 49G | yes, with a server | 128-byte and 1 KB blocks, checksum or CRC | either |
| HP 38G | as a client only | no | the calculator, SEND and RECV |
| HP 39G, 40G | as a client only | for binary aplets | the calculator |
| HP 42S, 27S, 17B, 17BII, 19BII, 28S | no serial port | | |

## Kermit

Bill Catchings and Frank da Cruz designed Kermit at Columbia University in 1981. A packet is short and made of printable characters, apart from its start mark:

| Field | What it holds |
|---|---|
| MARK | Ctrl-A, the start of a packet |
| LEN | the length of the rest, as a character |
| SEQ | the packet's number, 0 to 63 |
| TYPE | one letter: S to start, F for a file name, D for data, Z for end of file, B for end of the batch, Y and N for yes and no, E for an error |
| DATA | the bytes, with control characters written as `#` and a letter |
| CHECK | a check over the packet: one character (type 1), two (type 2) or a 16-bit CRC in three (type 3) |

Every packet is answered with Y (received) or N (send again) before the next one goes. The two sides agree on the packet length, the timeout and the check type in the first exchange.

### Kermit on the HP calculators

- The calculator talks at 9600 baud by default, its maximum, with 8 data bits, no parity, 1 stop bit and no flow control.
- Packets are at most 94 characters, with no long packets and no sliding windows; a Kermit program falls back on its own.
- The check type is 3, the CRC, on most models; 1 and 2 also work.
- The calculator wants every control character prefixed. Most Kermit programs do this by default; Kermit 95 needs `SET CONTROL PREFIX ALL`.
- The calculator's Kermit is written in System RPL. A received file grows by copying all of it at each packet, so large transfers get slower as they go. A computer program may need a longer timeout, 20 seconds, and a pause of 100 ms before each packet.

### Server and client

A Kermit **server** waits for commands and does what the other side asks: send a file, take a file, list a directory, run a command. The **client** gives the commands.

The 48 and 49G are best run as servers, with the computer as the client. The Kermit Project's hints for the HP 48 say the 48's own client commands, such as REMOTE DIRECTORY, are not reliable. To start the server:

| Model | Keys |
|---|---|
| HP 48S, 48SX | type `SERVER` and press ENTER |
| HP 48G series | right-shift, then → (the right arrow) |
| HP 49G | right-shift and → quickly, without a pause; with a pause they start the XModem server |

The display then says "Awaiting Server Cmd.". The server also runs a command sent from the computer and returns the stack.

The 38G, 39G and 40G work the other way round. They have no server: the calculator is the client, and the computer plays the "disk drive" it expects, keeping aplets in one folder. You choose SEND or RECV in the Aplet Library on the calculator, then "Disk drive…". [Files](files.md) in the app can be that disk drive.

## Binary and ASCII

The calculator sends a variable in one of two forms. System flag −35 chooses which: set for binary, clear for ASCII. On the 48G series, FMT in the TRANSFER form sets the same thing.

**Binary** is the object as it sits in memory, after a header of 8 characters: `HPHP48-` and the ROM's letter on a 48, `HPHP49-` and a letter on a 49G. It is exact and faster. Both sides must treat the file as binary. A computer that converts line ends in a binary file damages it; the calculator then receives a string that starts with `HPHP48-` instead of the object. The FAQ lists small programs that repair such strings.

**ASCII** is the object as text, as the calculator shows it in the editor. On sending, the calculator turns the object into text and adds a header such as `%%HP: T(3)A(D)F(.);`. On receiving, it turns the text back into an object, packet by packet, so a syntax error stops the transfer at once. That also means an arbitrary text file cannot be received in ASCII mode.

The header records three settings, so that the text reads back the same:

| Letter | Setting | Values |
|---|---|---|
| T | the translation mode | 0 to 3, below |
| A | the angle mode | D degrees, R radians, G grads |
| F | the fraction mark | `.` or `,` |

A file with a fraction mark other than the calculator's gives "Invalid Syntax".

### Translation modes

The translation mode says what happens to line ends and to the calculator's own characters, such as →, « » and Σ:

| Mode | Calculator to computer | Computer to calculator |
|---|---|---|
| 0 | nothing translated | nothing translated |
| 1 | a line end becomes CR LF | CR LF becomes a line end |
| 2 | as 1, and characters 128 to 159 become codes like `\->` | as 1, and the codes become characters |
| 3 | as 1, and all characters 128 to 255 become codes | as 1, and all codes become characters |

Mode 1 is the default, mode 3 the safest for editing on a computer. A binary transfer is never translated.

## The settings: IOPAR

The calculator keeps its I/O settings in a list in the variable IOPAR in HOME, created the first time they are used. Deleting it brings back the defaults.

| Entry | Setting | Values | Default |
|---|---|---|---|
| 1 | speed | 1200, 2400, 4800, 9600 | 9600 |
| 2 | parity | 0 none, 1 odd, 2 even, 3 mark, 4 space; negative to send it but not check it | 0 |
| 3 | receive pacing | XON/XOFF when its buffer fills; not used by Kermit | 0 |
| 4 | transmit pacing | stop on XOFF; not used by Kermit | 0 |
| 5 | check type | 1, 2 or 3, when the calculator starts a send | 3 |
| 6 | translation mode | 0 to 3 | 1 |

Other settings are system flags: −33 chooses the wire or infrared, −35 binary or ASCII, and −36 whether a received object replaces a variable of the same name. With −36 clear, the calculator adds an extension like `.1` instead.

## XModem

XModem goes back to Ward Christensen's MODEM program of 1977. It sends a file in blocks: a start byte (SOH for 128 bytes, STX for 1 KB), the block number and its complement, the data and a check: a one-byte sum, or a 16-bit CRC. The receiver starts the transfer: NAK asks for checksums, `C` for the CRC. Each block is answered with ACK or NAK, and EOT ends the file. The last block is padded to its full size.

On the HP calculators:

- **48G series**: `XSEND` and `XRECV`, with the name of the variable. Binary only, 128-byte blocks with the checksum: the 48's XModem does no CRC, so a computer program that asks for one has to fall back. Before ROM revision R, XRECV could fail unless twice the file's size was free.
- **49G**: 128-byte blocks with the checksum, 1 KB blocks and 1 KB blocks with a CRC, and noticeably faster than Kermit. It also has XSERV, a command server on top of XModem, started with right-shift, a pause, and →.
- **39G and 40G**: binary aplets, from "HP39/40 (Wire)".

For XModem, start the sender first and then the receiver, with only a short gap. For Kermit it is the other way round: start the receiving side first.

## A Kermit program on the computer

Any Kermit program works with the calculator as server. The Kermit Project's settings for C-Kermit and Kermit 95, with the serial port's name in place of `COM1`:

```
SET MODEM TYPE DIRECT
SET PORT COM1
SET SPEED 9600
SET CARRIER-WATCH OFF
SET FLOW NONE
SET PARITY NONE
SET CONTROL PREFIX ALL
SET BLOCK 3
```

Set the program's file type to binary or text to match the calculator's flag −35. Then `REMOTE DIRECTORY`, `GET` and `SEND` talk to the calculator's server, and `FINISH` ends it. The project's own tool does all of this for you: see [satx](satx.md).

## Sources

- [Kermit Protocol Manual, 6th edition, by Frank da Cruz](https://www.kermitproject.org/kproto.pdf): Kermit's origin and design, the packet and its fields, the check types, prefixing, server mode.
- [HP-48 Kermit Hints and Tips, the Kermit Project](https://www.columbia.edu/kermit/hp48.html): the HP's defaults, the check types, control prefixing, no long packets, ASCII and binary mode on the HP, the slowdown, the timeout and pause, the client commands, the settings for the computer.
- [HP 48G Series User's Guide](https://literature.hpcalc.org/community/hp48g-ug-en.pdf): the TRANSFER form and its fields, Kermit from Columbia, sending and receiving (27-8 to 27-11), XModem without CRC (27-14), IOPAR (27-15), the flags (appendix D).
- [HP 48G Series Advanced User's Reference Manual](https://literature.hpcalc.org/community/hp48g-aur-en.pdf): IOPAR's entries and values (D-5, D-6).
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: the 49G's Kermit and XModem (2.14), binary files received as `HPHP48-` strings (6.11), the `%%HP:` header and the translation modes (6.12), the slowdown (6.13), the XRECV bug (6.14), which side to start first (6.15), the 48G's XModem (10.1), starting the server on the G series (4.19).
- [XMODEM/YMODEM Protocol Reference, edited by Chuck Forsberg](https://gallium.inria.fr/~doligez/zmodem/ymodem.txt): XModem's origin, blocks, checks and flow.
- [Circuit to correct the HP49G serial port signal (serial49.zip)](https://www.hpcalc.org/hp49/docs/misc/serial49.zip): XModem faster than Kermit on the 49G.
- [Conn4x help, by Bill Graves](https://www.hpcalc.org/details/5457): starting the 49G's Kermit server and XSERV.
- [HP 38G User's Guide](https://literature.hpcalc.org/official/hp38g-ug-en.pdf) and [HP 39G/40G User's Guide](https://literature.hpcalc.org/official/hp39g40g-ug-en.pdf): sending and receiving aplets from the calculator, the disk drive (1-26, 16-5).
- [HP 49G User's Manual](https://literature.hpcalc.org/official/hp49g-um-en.pdf): transfers between a 49G and a 48 in ASCII (A-2).
- [HP Graphing Calculator PC Connectivity Kit User Guide](https://literature.hpcalc.org/community/hp-graph-calc-connectivity.pdf): the 38G's transfers started and controlled from the calculator.
- [HPGComm 2.01](https://www.hpcalc.org/details/3715), the PC program for the 38G, 39G and 40G: binary aplets by XModem on the 39G and 40G.
