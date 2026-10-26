---
title: The display
description: The LCD controller of the Saturn calculators: the 131 × 64 screen, display memory in RAM, the main and menu areas, margins and scrolling, contrast and the annunciators.
section: saturn
order: 3
---

The 48SX, the 48G series, the 49G and the 38G, 39G and 40G share one display: 131 × 64 pixels, drawn by a controller in the Clarke or Yorke chip. The picture lives in ordinary RAM, and the controller reads it 64 times a second.

## At a glance

| | |
|---|---|
| Pixels | 131 × 64, one bit each |
| Refresh | 64 frames a second, one row every 1/4096 s |
| Display memory | ordinary RAM, anywhere in the address space |
| Areas | a main area at the top and a menu area under it, each from its own address |
| Contrast | 5 bits, 0 to 31; higher is darker |
| Annunciators | six: left shift, right shift, alpha, alert, busy, transmitting |
| Chips | the controller in the Clarke or Yorke, two SED1181 column drivers, an Epson panel |

The Pioneer models with the Lewis chip (42S, 27S, 17B, 17BII) have a display of 131 × 16 pixels, and the 28S and 19BII one of 137 × 32. This page is about the 131 × 64 display.

## Display memory

There is no separate video memory. The controller fetches each row from RAM as it refreshes it, so a program draws by writing bits to RAM, and the picture changes on the next refresh.

A row of 131 pixels takes 34 nibbles, 17 bytes, with the last bits unused. Within each nibble, the lowest bit is the leftmost pixel. This is also the layout of a graphics object (a GROB) on the calculator, so the system can show a GROB by pointing the display at it.

While the controller fetches a row, the CPU waits. A row takes about 22 microseconds out of every 244. That slows every program down, and turning the display off makes the 48G about 13% faster. With the display off the keyboard does not respond either.

## Two areas

The screen shows two bitmaps, one under the other:

- **The main area** starts at an address in the hardware registers and runs for a number of rows that the system sets.
- **The menu area** takes the rows that are left, from an address of its own.

Normally the main area has 56 rows and the menu area 8: the stack and the command line above, the six menu labels under them. A program that wants the whole screen sets the main area to all 64 rows. The main area cannot be empty.

On every model the ROM draws the six menu labels in the menu area as boxes 21 pixels wide with a pixel between them, 22 pixels apart. saturnus places the drawn menu keys under them by that measure.

## Margins and scrolling

The main area can be larger than the screen. Two registers make that work:

- **The left offset** shifts the main area left by 0 to 7 pixels. For more, the start address moves on by two nibbles, 8 pixels.
- **The right margin** is a number of nibbles added at the end of each row. With it, each screen row is one row of a wider bitmap, so a GROB wider than the screen can be shown and scrolled sideways by moving the start address.

The right margin is signed. A negative margin, with the start address at the last row, draws the bitmap upside down.

The start address must be even: the controller fetches whole bytes. To start at an odd nibble, a program uses a left offset of 4 to 7 pixels instead.

The menu area has neither: it is always 131 pixels wide, from its address.

A register reports which row the controller is fetching, counting down from 63 at the top to 0 at the bottom. Programs that flip between two pictures to show grey levels wait for that counter, so that they flip between frames and the picture does not tear.

## Contrast

Contrast is a 5-bit value, 0 to 31; a higher value is darker. On the 48 series, holding ON and pressing + or − changes it. The ROM keeps it within a range: 3 to 19, according to the [Voyage book](https://literature.hpcalc.org/community/hp48gx-voyage.pdf). The 38G changes it with ON and a second key as well.

After a cold start, the 48SX in saturnus sets its contrast to 11, the 48GX, 38G and 49G to 14, the 39G and 40G to 12.

## Annunciators

Above the pixels sits a row of six symbols, driven from two nibbles of the hardware registers. Their meanings, from the [48G's user's guide](https://literature.hpcalc.org/community/hp48g-ug-en.pdf):

| Annunciator | Shows |
|---|---|
| Left shift | the left shift key was pressed |
| Right shift | the right shift key was pressed |
| Alpha | alpha mode, letters on the keys |
| Alert | an appointment is due, or the batteries are low |
| Busy | the calculator is not ready for new input; it keeps up to 15 keys meanwhile |
| Transmitting | data goes out to another device, on the wire or the infrared |

A separate bit turns all annunciators on or off, independently of the display. The ROM keeps a copy of the annunciator bits in RAM and updates the shift and alpha symbols outside the keyboard routine.

## Other registers

A few bits in the display registers are for HP's own tests. One stops the row scan, so every row is drawn on the same line of the panel; left on for long, it can damage the LCD. Programs leave these bits alone, and an emulator only stores them.

The 48's card port 1 carries the display's data stream on four pins: a clock, a line sync and two data lines. HP's overhead projector display for the classroom plugged in there. The 38G and the 49G connect to an overhead projector through their 10-pin connector.

## Sources

- [Introduction to Saturn Assembly Language, 3rd edition (2005)](https://www.hpcalc.org/details/1693), by Gilbert Fernandes and Eric Rechlin, chapter 68 by Christoph Gießelink: the refresh and the stall, the two areas, the left offset and the right margin, the row counter, the register names.
- [Voyage au centre de la HP 48 G/GX, by Paul Courbis and Sébastien Lalande](https://literature.hpcalc.org/community/hp48gx-voyage.pdf): the speed with the display off, the contrast and its range, the scan bit that can damage the LCD, the odd start address, the upside-down picture.
- [Guide to the Saturn Processor, by Matthew Mastracci](https://www.hpcalc.org/details/1753): the display and annunciator registers, the overhead projector signals on port 1, the column drivers and the panel.
- [HP48SX Hardware Registers Document, by Taplin](https://www.hpcalc.org/hp48/docs/programming/hdwreg.txt): the display registers on the 48SX.
- [HP48SX Keyboard Input, by Joe Ervin (input.zip)](https://www.hpcalc.org/details/1720): the annunciators updated outside the keyboard routine, their copy in RAM.
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: the pixel order of a GROB (8.18), the 49G and the overhead projector (2.14).
- [HP 48G Series User's Guide](https://literature.hpcalc.org/community/hp48g-ug-en.pdf): what the annunciators mean (1-2).
- [HP 38G User's Guide](https://literature.hpcalc.org/official/hp38g-ug-en.pdf): contrast with ON and a second key.
- [Hewlett-Packard Journal, June 1996 (hpj-38g.zip)](https://www.hpcalc.org/hp38/docs/hpj-38g.zip) and [Finseth's HP calculator data: HP 49G](https://www.finseth.com/hpdata/hp49g.php): the 10-pin connectors.
