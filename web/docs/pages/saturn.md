---
title: How the Saturn works
description: The hardware of HP's Saturn calculators for developers: the CPU, the memory and its controllers, the display, the keyboard, the timers and interrupts, the serial port and infrared.
section: saturn
---

The HP 48SX, the 48G series, the 49G and the 38G, 39G and 40G are built around HP's Saturn processor and one companion chip, the Clarke in the 48SX and the Yorke in the others. These pages describe that hardware for developers: for anyone who writes machine code for these calculators, builds an emulator or wants to know how saturnus works inside.

The pages describe the hardware, as HP's documents and the community's describe it, not the code in the ROM.

## The pages

- [The CPU](saturn/cpu.md): the registers and their fields, 4-bit nibbles and 64-bit registers, the instruction set in general terms and the clock of each model.
- [Memory](saturn/memory.md): the bus, the six memory controllers and how they map ROM, RAM and the card ports, and bank switching on the 48GX and the 49G.
- [The display](saturn/display.md): the 131 × 64 LCD, display memory in RAM, the main and menu areas, margins and scrolling, contrast and the annunciators.
- [The keyboard](saturn/keyboard.md): the key matrix on the CPU's ports, the hardware scan, debouncing, held keys and the ON key.
- [Timers and interrupts](saturn/timers.md): the two timers, keeping the time, the interrupt and its sources, masking, sleeping with SHUTDN.
- [The serial port and infrared](saturn/serial.md): the UART, the frame and the speeds, the line levels, the infrared link and which model has what.

## The family

| Model | Chip | Clock | RAM | ROM |
|---|---|---|---|---|
| HP 48SX | Clarke | about 2 MHz | 32 KB | 256 KB |
| HP 48G | Yorke | about 4 MHz | 32 KB | 512 KB |
| HP 48GX | Yorke | about 4 MHz | 128 KB | 512 KB |
| HP 38G | Yorke | about 4 MHz | 32 KB | 512 KB |
| HP 39G, 40G | Yorke | about 4 MHz | 256 KB | 1 MB |
| HP 49G | Yorke | about 4 MHz | 512 KB | 2 MB of flash |

The other calculators saturnus runs use the Saturn's older relative, the Lewis chip: the 42S, 27S, 17B and 17BII with one, the 28S and 19BII with two. Their memory is fixed, their display smaller and their only output an infrared printer port. These pages do not cover them; their model pages say more, such as [the HP 42S](42s.md).

## Further reading

The sources each page lists are online. The most useful ones to start with:

- [Introduction to Saturn Assembly Language, 3rd edition (2005)](https://www.hpcalc.org/details/1693), by Gilbert Fernandes and Eric Rechlin, with chapters on the hardware by Christoph Gießelink, the author of the Emu48 emulator.
- [Voyage au centre de la HP 48 G/GX](https://literature.hpcalc.org/community/hp48gx-voyage.pdf), by Paul Courbis and Sébastien Lalande, in French.
- [The HP 48 I/O Technical Interfacing Guide](https://www.hpcalc.org/hp48/docs/misc/io2.txt), HP's own description of the serial and infrared ports.
- [The HP 48 FAQ](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl.

## Sources

- [Finseth's HP calculator data](https://www.finseth.com/hpdata/hp48gx.php): the chips, clocks and memory of the 48 series and the [38G](https://www.finseth.com/hpdata/hp38g.php).
- [Wikipedia: HP Saturn](https://en.wikipedia.org/wiki/HP_Saturn): the chips and the models that use them.
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html): the 49G's memory (2.14).
- [Christoph Gießelink, 25 years of Emu48](https://hp.giesselink.com/25ann48.htm) and [Wikipedia: HP 39/40 series](https://en.wikipedia.org/wiki/HP_39/40_series): the 39G's and 40G's memory and their 49G design.
