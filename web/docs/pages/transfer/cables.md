---
title: Cables
description: Which cable connects which HP calculator to a computer: the 48's 4-pin port and its wiring, the 10-pin connector of the 49G and 38G, HP's cables and adapters, USB serial adapters and building a cable yourself.
section: transfer
order: 1
---

To move files between a computer and a real calculator you need a serial cable and, on a computer of today, a USB serial adapter. The HP 48 series has one kind of connector, the 49G and the 38G another. The 42S, 27S, 17B, 17BII, 19BII and 28S have no serial port at all.

## Which models have a port

| Model | Connector | Cable |
|---|---|---|
| HP 48S, 48SX | 4 pins, top edge | HP 82208A (PC), 82209A (Macintosh) |
| HP 48G, 48G+, 48GX | 4 pins, top edge | the same cables |
| HP 49G | 10 pins | the cable of HP's Connectivity Kit; a 49G-to-49G cable in the box |
| HP 38G | 10 pins, as the 49G | the cable of HP's Connectivity Kit |
| HP 39G, 40G | the 49G's hardware | a PC adapter and a cable from calculator to calculator in the 40G's box |
| HP 42S, 27S, 17B, 17BII, 19BII, 28S | none | only infrared to HP's 82240 printer, one way |

Infrared from the 48 or the 39G to a computer is possible, but slow (2400 baud) and short-ranged, and saturnus does not use it.

## The HP 48's port

The 48 has four pins in a small recess on its top edge, spaced 2 mm apart. Looking at the calculator, from the outer edge inwards:

| Pin | Signal |
|---|---|
| 1 | shield |
| 2 | TX, data out of the calculator |
| 3 | RX, data into the calculator |
| 4 | signal ground |

The recess is not symmetrical: the pins are nearer the top of the calculator, which keeps a home-made plug from going in upside down.

HP sold two cables for it: the HP 82208A with a PC's 9-pin connector (and an adapter to 25 pins), and the HP 82209A for the Macintosh. HP's link kits came in two versions, at about $20 and $50; the dearer one added software and a manual, the cable was the same.

At the PC end the wiring is a null-modem: the calculator's TX goes to the PC's RX and back.

| Calculator | PC, 9-pin (DE-9) | PC, 25-pin (DB-25) |
|---|---|---|
| shield | shell | 1 |
| TX (out) | 2, RX | 3, RX |
| RX (in) | 3, TX | 2, TX |
| signal ground | 5 | 7 |

The calculator uses no other lines: no RTS, CTS, DTR or carrier detect. Programs on the computer must be told not to wait for them.

## Building a 48 cable

The HP 48 FAQ explains how to build one. You need a female 9-pin (or 25-pin) connector with its shell, some wire, and a plug with four sockets 2 mm apart: not the usual 0.1 inch (2.54 mm) spacing, which damages the calculator's pins.

The FAQ's tip for the plug is an old CD-ROM audio cable from a PC: one of its two ends has the 2 mm spacing and fits the 48. Cut off the other end and solder three wires to the PC connector as in the table above; the empty socket is the ground. Its harder way is four sockets taken from an IC socket strip, glued together at 2 mm.

## The 49G, 38G, 39G and 40G

The 49G's port has ten contacts and a different shape. In the box came a cable with a 10-pin plug at each end, for connecting two 49Gs, and a small cap that turns its 10-pin plug into the 48's 4-pin one, for connecting a 49G to a 48.

HP's Graphing Calculator PC Connectivity Kit of 1999 had one cable for the 48 series, the 49G and the 38G: a 9-pin plug for the PC at one end and, at the other, the plug of the 49G and 38G, with an adapter for the 48 series. The kit's guide warns not to use that adapter with the 49G or the 38G. The 38G's 10-pin connector also carries the signals for HP's overhead projector display.

The 39G and 40G are built on the 49G's hardware. The 40G, which has no infrared, came with a PC adapter and a cable from calculator to calculator. This page could not confirm from HP's documents that the 39G's and 40G's connector is the 49G's, so check the plug before you buy a cable for one.

Early 49Gs, with a serial number below 94xxxxxx, have a weak transmitter that some PC ports cannot read. Two community circuits fix it: one patches the calculator, the other is a small buffer in the cable, powered from the PC's RTS and DTR lines. The program on the computer then has to raise RTS and DTR; [HPComm](kermit-xmodem.md#hpcomm-and-hpgcomm), the program of HP's Connectivity Kit, did.

## USB serial adapters

A computer of today has no serial port, so the cable's 9-pin end goes into a USB serial adapter. Any adapter with a 9-pin male RS-232 plug works; the operating system shows it as a serial device:

| System | Device name |
|---|---|
| macOS | `/dev/cu.usbserial-…` |
| Linux | `/dev/ttyUSB0` |
| Windows | `COM3` or another COM port |

Two things to watch:

- **RS-232 levels, not logic levels.** The calculator talks RS-232: a negative voltage for a 1 bit, a positive one for a 0. A bare USB serial board with 3.3 V or 5 V pins idles at a positive voltage, which the calculator reads as data. Use an adapter with a real RS-232 plug.
- **The early 49G.** Its signal may be too weak for an adapter's receiver as well. The buffer described above helps; it needs a program that raises RTS and DTR when it opens the port, as satx does.

[satx](satx.md) finds the adapter by itself when it is the only USB serial port, and [Files](files.md) in the app asks Chrome or Edge for it.

## Sources

- [HP 48 I/O Technical Interfacing Guide](https://www.hpcalc.org/hp48/docs/misc/io2.txt), Hewlett-Packard: the HP 82208A and 82209A cables and their pinouts (2.1), the line levels.
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: the pinout of the 48's connector (6.17), HP's link kits (6.5), building a cable and the CD-ROM audio cable (12.1), the 49G's cable and adapter (2.14), the IR range (6.7).
- [HP Graphing Calculator PC Connectivity Kit User Guide](https://literature.hpcalc.org/community/hp-graph-calc-connectivity.pdf): the kit's cable and adapter, the warning about the 49G and the 38G.
- [Finseth's HP calculator data: HP 49G](https://www.finseth.com/hpdata/hp49g.php): the 10-pin connectors and the 10-to-4-pin cap in the 49G's box.
- [Hewlett-Packard Journal, June 1996 (hpj-38g.zip)](https://www.hpcalc.org/hp38/docs/hpj-38g.zip) and [Finseth's HP calculator data: HP 38G](https://www.finseth.com/hpdata/hp38g.php): the 38G's serial port, its connector and the overhead display.
- [Wikipedia: HP 39/40 series](https://en.wikipedia.org/wiki/HP_39/40_series): the 39G's and 40G's hardware, the 40G's serial port.
- [HP 39G/40G User's Guide](https://literature.hpcalc.org/official/hp39g40g-ug-en.pdf): the 40G's adapter and cable (16-5).
- [HP 49G User's Manual](https://literature.hpcalc.org/official/hp49g-um-en.pdf): connecting a 49G to a 48 with the adapter (A-2).
- [External serial buffer for the HP49, by Marcel Flipse (buf49.zip)](https://www.hpcalc.org/details/3009) and [Circuit to correct the HP49G serial port signal (serial49.zip)](https://www.hpcalc.org/hp49/docs/misc/serial49.zip): the early 49G's transmitter and the fixes.
- [HP-48 Kermit Hints and Tips, the Kermit Project](https://www.columbia.edu/kermit/hp48.html): no modem signals.
- [HP 42S Owner's Manual](https://literature.hpcalc.org/community/hp42s-om-en.pdf) and [HP 17BII Owner's Manual](https://literature.hpcalc.org/community/hp17bii-om-en.pdf): the infrared printer as the only output.
