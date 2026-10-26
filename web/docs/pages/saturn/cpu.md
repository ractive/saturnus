---
title: The CPU
description: The Saturn processor: its 4-bit data path, 64-bit registers and their fields, 20-bit addresses, the instruction set in general terms and the clock of each model.
section: saturn
order: 1
---

The Saturn is HP's own 4-bit processor. It works on nibbles, half a byte each, but its main registers hold 64 bits, so that one register holds a whole floating-point number. The same core runs the HP 48SX, the 48G series, the 49G and the 38G, 39G and 40G.

## At a glance

| | |
|---|---|
| Data path | 4 bits: one nibble per memory access |
| Addresses | 20 bits, counting nibbles: 1 M nibbles, 512 KB |
| Working registers | A, B, C and D, 64 bits (16 nibbles) each |
| Scratch registers | R0 to R4, 64 bits each |
| Data pointers | D0 and D1, 20 bits each |
| Return stack | 8 levels of 20 bits, in the CPU |
| Arithmetic | hexadecimal or decimal (BCD), switched by an instruction |
| Clock | about 2 MHz on the 48SX, about 4 MHz on the G series and the 49G |
| Chip | the 1LT8 core, inside HP's Clarke (48SX) or Yorke (48G, 38G, 39G, 40G, 49G) |

## A processor built for a calculator

The Saturn first appeared in the [HP 71B](https://en.wikipedia.org/wiki/HP-71B) of 1984, as the 1LF2 chip. A later version, the 1LK7, ran the 71B, the 18C and the [28C](../28s.md), and the 1LT8 core went into the 48SX, the 48G series and the 38G. In the 48SX the CPU sits inside the Clarke chip, together with the display controller, the timers, the serial port and the memory controllers; in the 48G series it sits inside the Yorke. The Pioneer calculators (17B, 27S, 42S) and the 28S use the Lewis, a chip of the same family with its own memory interface.

HP designed it around its decimal floating-point format. A number takes 16 nibbles: a sign nibble, 12 nibbles of mantissa and three nibbles of exponent with its sign. That is exactly one 64-bit register, and most instructions can work on a part of it (a field, below). In decimal mode the adder works in [BCD](https://en.wikipedia.org/wiki/Binary-coded_decimal), so the mantissa needs no conversion.

## Registers

The four working registers A, B, C and D do the arithmetic. They cannot all be combined freely: D pairs only with C, B with A or C, and only A and C exchange with the scratch registers, the data pointers and memory. The five scratch registers R0 to R4 can only be copied to and from A and C, or exchanged with them.

D0 and D1 hold addresses. Every memory read or write goes through one of them, and both can be moved forward or back by 1 to 16 nibbles in one instruction.

The other registers:

| Register | Width | What it does |
|---|---|---|
| PC | 20 bits | the program counter |
| P | 4 bits | points at one nibble of a working register, for the P and WP fields |
| RSTK | 8 × 20 bits | the return stack for subroutine calls |
| ST | 16 bits | program status flags; programs use the low 12, the operating system the top 4 |
| HST | 4 bits | hardware status: module missing, sticky bit, service request, module pulled |
| CARRY | 1 bit | set by an arithmetic overflow, and by a test that is true |
| IN | 16 bits | an input port, read only: the keyboard's columns |
| OUT | 12 bits | an output port, write only: the keyboard's rows and the beeper |

The return stack holds eight return addresses. A ninth call drops the oldest one, and a return from an empty stack reads zero, which sends the program to address 0. The interrupt system needs one or two levels of it, so a program that calls subroutines seven or eight deep while interrupts are on loses its oldest return address. Six levels deep is safe.

OUT cannot be read back. Software that changes it keeps a copy in RAM, so that the interrupt handler can put it back afterwards.

## Fields

An instruction that works on a working register names a field: the nibbles it touches. The fields follow the layout of a floating-point number.

| Field | Nibbles | Meaning |
|---|---|---|
| W | 15 to 0 | the whole register |
| S | 15 | the mantissa's sign |
| M | 14 to 3 | the mantissa |
| X | 2 to 0 | the exponent with its sign |
| XS | 2 | the exponent's sign |
| B | 1 to 0 | one byte |
| A | 4 to 0 | 20 bits: an address |
| P | P | the nibble P points at |
| WP | P to 0 | from nibble P down to nibble 0 |

So `A=A+B M` adds the mantissas of A and B and leaves the sign and the exponent alone. The A field is used so often for addresses that many instructions have a shorter form for it.

Values in memory are [little-endian](https://en.wikipedia.org/wiki/Endianness): the nibble at the lowest address goes into nibble 0 of the register. The same holds for constants inside instructions.

## The instruction set

Instructions are two or more nibbles long; one that loads a 16-nibble constant carries all 16 nibbles in the instruction. They fall into a few groups:

- **Arithmetic and logic** on a field: add, subtract, increment, decrement, negate, complement, AND, OR, and shifts by one nibble or by one bit. There is no multiply or divide: the ROM does those in software.
- **Moves and exchanges** between registers, and between A or C and memory through D0 or D1.
- **Loads of constants** into A, C, D0, D1 or P.
- **Tests** that compare two registers on a field, or test a status bit, and set CARRY. A test is followed by a branch offset, taken when the test is true.
- **Jumps and calls**: relative jumps of 2, 3 or 4 nibbles of offset, absolute ones of 5 nibbles, conditional jumps on CARRY, subroutine calls and several kinds of return.
- **Chip control**: CONFIG, UNCNFG, RESET and C=ID for the memory controllers (see [Memory](memory.md)), SHUTDN to sleep, INTON, INTOFF, RSI and RTI for the interrupts (see [Timers and interrupts](timers.md)), and the instructions that read IN and write OUT (see [The keyboard](keyboard.md)).

SETHEX and SETDEC switch the adder between hexadecimal and decimal. The mode applies only to register arithmetic. There is no instruction to read the mode back, and some instructions always count in hexadecimal whatever the mode: moving D0 or D1, changing P, and adding or subtracting a small constant.

The [49g+, 50g](https://en.wikipedia.org/wiki/HP_49/50_series) and later models run an emulated Saturn on an [ARM](https://en.wikipedia.org/wiki/ARM_architecture_family) processor. Their "Saturn+" adds multiply, divide and other instructions, which the real Saturn does not have.

## Speed

The 48SX's clock is about 2 MHz, the 48G series and the 49G run at about 4 MHz, from a 32 kHz crystal multiplied up. The clock does not tell the whole story:

- An instruction takes several cycles, more for memory access and jumps. HP's assembler manual gives approximate counts, and the counts in the Meta Kernel documentation for the 48G are higher. A register add on the A field takes about 7 to 8 cycles, a jump about 11 to 14.
- Some instructions take a cycle more or less depending on whether they start at an odd or an even address.
- The display controller reads its rows from RAM and stops the CPU while it does (see [The display](display.md)). Turning the display off makes the 48G about 13% faster.
- On the 48G series, the upper half of the ROM and the cards in port 2 are reached through bank switching, and the bus is still 4 bits wide. The clock doubled from the 48SX, but programs run only about 40% faster.

A benchmark on the [HP Museum forum](https://www.hpmuseum.org/forum/thread-9750.html) times the same sum of 1,000 terms on real calculators: 95.5 s on a 48SX, about 55 s on a 48GX and about 48 to 51 s on a 49G, whose ROM was tuned for speed.

| Model | Clock | Chip |
|---|---|---|
| HP 48SX | about 2 MHz | Clarke |
| HP 48G, 48G+, 48GX | about 4 MHz | Yorke |
| HP 38G | about 4 MHz | Yorke |
| HP 39G, 40G | about 4 MHz | Yorke |
| HP 49G | about 4 MHz | Yorke |
| HP 42S, 27S, 17B, 17BII | about 1 MHz | Lewis |

saturnus times every instruction with the published cycle counts and the display's stalls, then scales the 48SX, 48GX and 49G by factors measured against that benchmark, so that programs take as long as on those calculators. The 38G takes the 48GX's factor and the 39G and 40G the 49G's, and the other models run at an assumed speed: none of them was timed against a real calculator. See [the HP 48GX page](../48gx.md) for how that looks in the app.

## Sources

- [Introduction to Saturn Assembly Language, 3rd edition (2005)](https://www.hpcalc.org/details/1693), by Gilbert Fernandes and Eric Rechlin: the registers, the return stack, little-endian order, the instruction groups and their cycle counts, the odd and even addresses, the Saturn+ instructions, the clock and the crystal.
- [HP SASM reference, from HP's 48 development tools](https://www.hpcalc.org/hp48/docs/programming/sasmhtml.zip): the HST bits, which instructions always count in hexadecimal, the cycle counts.
- [Guide to the Saturn Processor, by Matthew Mastracci](https://www.hpcalc.org/details/1753): the chip versions and the companion chips, the registers, the fields, OUT and its copy in RAM.
- [Voyage au centre de la HP 48 G/GX, by Paul Courbis and Sébastien Lalande](https://literature.hpcalc.org/community/hp48gx-voyage.pdf): the arithmetic mode and how to test it, the speed with the display off.
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: the 48G's speed against the 48SX.
- [Finseth's HP calculator data](https://www.finseth.com/hpdata/hp48sx.php), with its pages on the [48GX](https://www.finseth.com/hpdata/hp48gx.php), [38G](https://www.finseth.com/hpdata/hp38g.php) and [49G](https://www.finseth.com/hpdata/hp49g.php): the Clarke and Yorke chips, the clocks, the 49G's tuned code.
- [Hewlett-Packard Journal, June 1991](https://www.hpcalc.org/hp48/docs/misc/hpj-48sx.zip) and [August 1994](https://www.hparchive.com/Journals/HPJ-1994-08.pdf): the 48SX's and the 48G series' processors and clocks.
- [HP Museum forum: Summation based benchmark for calculators](https://www.hpmuseum.org/forum/thread-9750.html): the times on real calculators.
- [Wikipedia: HP Saturn](https://en.wikipedia.org/wiki/HP_Saturn): the chips, their clocks (the Yorke at 3.68 to 4 MHz, the Lewis at 1 MHz) and the models that use them, the 71B as the first.
