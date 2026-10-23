---
title: Kermit and XModem
description: Moving files between a real HP calculator and a computer with Kermit and XModem: which model speaks which, starting the server, binary and ASCII transfers, the settings that matter, what to check when a transfer fails, and the programs on the computer.
section: transfer
order: 2
---

The HP calculators move files over their serial port with Kermit. The 48G series, the 49G and the 39G and 40G also speak XModem. Both sides need the same speed and the same transfer mode; most failed transfers come from a mismatch there. For how Kermit works, see the [Kermit Protocol Manual](https://www.kermitproject.org/kproto.pdf) by Frank da Cruz.

## Which calculator speaks which

| Model | Kermit | XModem | Who starts a transfer |
|---|---|---|---|
| HP 48S, 48SX | yes, with a server | no | the computer, with the calculator in SERVER |
| HP 48G, 48G+, 48GX | yes, with a server | XSEND and XRECV, binary only | either |
| HP 49G | yes, with a server | yes, and a server, XSERV | either |
| HP 38G | as a client only | no | the calculator, SEND and RECV |
| HP 39G, 40G | as a client only | for binary aplets | the calculator |
| HP 42S, 27S, 17B, 17BII, 19BII, 28S | no serial port | | |

## Kermit

Bill Catchings and Frank da Cruz designed Kermit at Columbia University in 1981. The calculator talks at 9600 baud, 8 data bits, no parity, 1 stop bit and no flow control.

### Starting the server

A Kermit **server** waits for commands from the **client**: send a file, take a file, list a directory, run a command. Run the 48 and the 49G as servers, with the computer as the client; the Kermit Project says the 48's own client commands are not reliable.

| Model | Keys |
|---|---|
| HP 48S, 48SX | type `SERVER` and press ENTER |
| HP 48G series | right-shift, then → (the right arrow) |
| HP 49G | right-shift and → quickly; with a pause between them they start XSERV, the XModem server |

The display then says "Awaiting Server Cmd.".

The 38G, 39G and 40G have no server. The calculator is the client and the computer plays its "disk drive", a folder of aplets: choose SEND or RECV in the Aplet Library, then "Disk drive…".

## XModem

XModem goes back to Ward Christensen's MODEM program of 1977. It sends one file in checked blocks, without its name.

- **48G series**: `XSEND` and `XRECV` with the variable's name. Binary only, 128-byte blocks with a checksum and no CRC; the computer's program must fall back to the checksum.
- **49G**: 128-byte and 1 KB blocks, checksum or CRC, and noticeably faster than Kermit. It also has an XModem server, XSERV.
- **39G and 40G**: binary aplets, from "HP39/40 (Wire)".

## Binary and ASCII

Flag −35 chooses the form a variable travels in: set for binary, clear for ASCII. On the 48G series and the 49G, FMT in the TRANSFER form sets it; on the 49G the form is under APPS, I/O FUNCTIONS.

**Binary** is the object as it sits in memory, after a header: `HPHP48-` and a letter on a 48, `HPHP49-` and a letter on a 49G. It is exact and faster. Both sides must treat the file as binary.

**ASCII** is the object as text, as the editor shows it, after a header such as `%%HP: T(3)A(D)F(.);`: the translation mode, the angle mode and the fraction mark. The calculator reads the text back packet by packet, so a syntax error stops the transfer at once, and an arbitrary text file cannot be received in ASCII mode.

A 48 and a 49G can be cabled together with the 49G's adapter. Set FMT to ASCII on both; only objects you created yourself transfer this way.

### Translation modes

The translation mode says what happens to line ends and to characters such as →, « » and Σ:

| Mode | Calculator to computer | Computer to calculator |
|---|---|---|
| 0 | nothing translated | nothing translated |
| 1 | a line end becomes CR LF | CR LF becomes a line end |
| 2 | as 1, and characters 128 to 159 become codes like `\->` | as 1, and the codes become characters |
| 3 | as 1, and all characters 128 to 255 become codes | as 1, and all codes become characters |

Mode 1 is the default, mode 3 the safest for editing on a computer. Binary is never translated.

## The settings that matter

- **Speed**: 1200, 2400, 4800 or 9600 baud, 9600 by default. The computer must use the same.
- **Parity and pacing**: none, the defaults, so IOPAR starts `{ 9600 0 0 0 …`. The computer must use the same.
- **Binary or ASCII**: flag −35, above.
- **Overwrite or keep**: flag −36. Set, a received object replaces a variable of the same name; clear, the new one gets an extension like `.1`.
- **Translation mode**: 0 to 3, 1 by default.

These live in the variable IOPAR in HOME, apart from the flags; on the 48G series and the 49G the TRANSFER form sets them. Deleting IOPAR brings back the defaults.

## Tips

- Start the receiving side first for Kermit. For XModem it is the other way round: start the sender, then the receiver, with only a short gap.
- Sending a large file to the calculator by Kermit gets slower as it goes. Give the computer's program a timeout of 20 seconds and a pause of 100 ms before each packet.
- The calculator wants every control character prefixed. Kermit 95 needs `SET CONTROL PREFIX ALL`.

## When a transfer fails

- **The computer gets no answer**: the server is not running; look for "Awaiting Server Cmd.". A 38G, 39G or 40G starts the transfer itself.
- **Time-outs or errors from the start**: the speeds or the parity differ. Compare IOPAR or the TRANSFER form with the computer's setting.
- **A string that starts with `HPHP48-`**: a binary file went over as text and its line ends were changed. Send it again in binary on both sides; the HP 48 FAQ lists programs that repair such strings.
- **"Invalid Syntax"** on an ASCII file: the fraction mark in its `%%HP:` header is not the calculator's.
- **XRECV fails on a 48G**: before ROM revision R it could need twice the file's size free.
- **Still nothing**: the cable or the port, or flag −33, which chooses the wire or infrared. See [cables](cables.md); early 49Gs have a weak transmitter some PC ports cannot read.

## Programs on the computer

### A Kermit program

Any Kermit program works with the calculator as server. The Kermit Project's settings for C-Kermit and Kermit 95, with your serial port in place of `COM1`:

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

Set the file type to binary or text to match flag −35. `REMOTE DIRECTORY`, `GET` and `SEND` then talk to the server, and `FINISH` ends it.

### satx and Files

[satx](satx.md) is the project's command-line tool for the Kermit server, and for XModem on the 48G series and the 49G. [Files](files.md) in the app runs the same code in Chrome, Edge or the desktop app, and is the disk drive for a 38G, 39G or 40G. Conn4x, by Bill Graves, is a Windows program for the 49G and, with its XModem server library, the 48.

### HPComm and HPGComm

HPComm is a Windows program by Jean-Pierre "James" Bergamin, the author of saturnus, for moving files between a PC and an HP 48 or 49G. It shows the calculator's directories beside the PC's folders, as Windows Explorer does, and files move between them by drag and drop, over Kermit. Version 2.4 came out as open source in February 1999.

HP then approached the project. HPComm 3.0, written by James Bergamin and Mitch Davis of HP, became the software of HP's Graphing Calculator PC Connectivity Kit (F1897A) in 1999, for the 48 series and the new 49G, and HP agreed to release it under the GNU GPL. The last version, 3.0r4, came out in June 2000.

For the 38G, HPComm38 split off at the end of 1999 and shipped in HP's HP38G PC Connectivity Kit. Reworked for the 39G and 40G, it became HPGComm: version 2.0 in June 2001, 2.01 in July. It serves a folder as the calculator's "disk drive", shows the calculator's screen on the PC, and sends binary aplets to the 39G and 40G by XModem.

The dog on the splash screen and the program icon is Spot, drawn by [ishadyn](https://ishadyn.ch/illustrationen/) ([@ishadyn_comic](https://www.instagram.com/ishadyn_comic/) on Instagram).

The source code, the help and the hpcomm.org website are archived [on GitHub](https://github.com/ractive/hpcomm). HPComm 3.0r4, the kit's version, as well as HPComm 2.4 and HPGComm 2.0 are still on hpcalc.org.

## Sources

- [Kermit Protocol Manual, 6th edition, by Frank da Cruz](https://www.kermitproject.org/kproto.pdf): Kermit's origin and design, server mode.
- [HP-48 Kermit Hints and Tips, the Kermit Project](https://www.columbia.edu/kermit/hp48.html): the HP's defaults, control prefixing, ASCII and binary mode on the HP, the slowdown, the timeout and pause, the client commands, the settings for the computer.
- [HP 48G Series User's Guide](https://literature.hpcalc.org/community/hp48g-ug-en.pdf): the TRANSFER form, sending and receiving (27-8 to 27-11), XModem without CRC (27-14), IOPAR (27-15), the flags (appendix D).
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: the 49G's Kermit and XModem (2.14), binary files received as `HPHP48-` strings (6.11), the `%%HP:` header and the translation modes (6.12), the slowdown (6.13), the XRECV bug (6.14), which side to start first (6.15), the 48G's XModem (10.1), starting the server on the G series (4.19).
- [XMODEM/YMODEM Protocol Reference, edited by Chuck Forsberg](https://gallium.inria.fr/~doligez/zmodem/ymodem.txt): XModem's origin.
- [Circuit to correct the HP49G serial port signal (serial49.zip)](https://www.hpcalc.org/hp49/docs/misc/serial49.zip): XModem faster than Kermit on the 49G.
- [Conn4x help, by Bill Graves](https://www.hpcalc.org/details/5457): starting the 49G's Kermit server and XSERV, IOPAR's parity and pacing, the 48 with its XModem server library.
- [HP 38G User's Guide](https://literature.hpcalc.org/official/hp38g-ug-en.pdf) and [HP 39G/40G User's Guide](https://literature.hpcalc.org/official/hp39g40g-ug-en.pdf): sending and receiving aplets from the calculator, the disk drive (1-26, 16-5).
- [HP 49G User's Manual](https://literature.hpcalc.org/official/hp49g-um-en.pdf): transfers between a 49G and a 48 in ASCII, the TRANSFER form under APPS (A-2, A-3).
- [HP Graphing Calculator PC Connectivity Kit User Guide](https://literature.hpcalc.org/community/hp-graph-calc-connectivity.pdf): the 38G's transfers started and controlled from the calculator.
- [HPComm, the archive on GitHub](https://github.com/ractive/hpcomm): HPComm's and HPGComm's history, versions and dates, HP's Connectivity Kit, the GPL, Spot.
- [HPComm 3.0r4](https://www.hpcalc.org/details/3713), the version of HP's kit for the 48 and the 49G, [HPComm 2.4](https://www.hpcalc.org/details/3717), the 48's from before the kit, and [HPGComm 2.0](https://www.hpcalc.org/details/3715) for the 38G, 39G and 40G, on hpcalc.org: binary aplets by XModem on the 39G and 40G.
- [ishadyn, illustrations](https://ishadyn.ch/illustrationen/): the drawing of Spot.
