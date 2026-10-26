---
title: The keyboard
description: How the Saturn calculators read their keys: the key matrix on the CPU's OUT and IN ports, the hardware scan and its interrupt, debouncing, held and released keys, and the ON key on its own line.
section: saturn
order: 4
---

The keys of a Saturn calculator are a matrix of wires on two of the CPU's own registers: OUT drives the rows, IN reads the columns. The chip scans the matrix by itself and interrupts the CPU when a key goes down. The ON key has a line of its own.

## At a glance

| | |
|---|---|
| HP 48SX, 48G series | 49 keys: 48 on 9 OUT lines × 6 IN lines, and ON |
| HP 49G | 51 keys: 50 on 8 OUT lines × 8 IN lines, and ON |
| ON key | its own pin, read as IN bit 15 whatever OUT holds |
| Hardware scan | every 1 ms, an interrupt on a key going down |
| Release | no interrupt: the system watches for it with a timer |
| Debouncing | in software, five equal samples 2 ms apart |
| Beeper | also on OUT, bit 11 |

## The matrix

Each OUT bit drives one row of keys. Each IN bit reports whether a column is connected to a driven row through a pressed key. To read one row, a program writes that row's bit to OUT and reads IN. Rows can be combined: with all nine bits set, IN is non-zero if any key is down.

The matrix of the 48 series, from the [Saturn assembly tutorial](https://www.hpcalc.org/details/1693). The column heads are the IN bits, the row heads the OUT bits, in hexadecimal:

| OUT | IN 20 | IN 10 | IN 08 | IN 04 | IN 02 | IN 01 |
|---|---|---|---|---|---|---|
| 100 | | B | C | D | E | F |
| 080 | | PRG | CST | VAR | ▲ | NXT |
| 040 | | STO | EVAL | ◀ | ▼ | ▶ |
| 020 | | COS | TAN | √x | yˣ | 1/x |
| 010 | | ENTER | +/− | EEX | DEL | ← |
| 008 | α | SIN | 7 | 8 | 9 | ÷ |
| 004 | left shift | MTH | 4 | 5 | 6 | × |
| 002 | right shift | A | 1 | 2 | 3 | − |
| 001 | | ' | 0 | . | SPC | + |

The keys are named as on the 48SX; A to F are the six menu keys. The 48G series has the same positions with its own labels. The 38G uses this matrix too, with 47 of its positions.

The 49G has a different keyboard and a different matrix, eight rows by eight columns. The 39G and 40G use the 49G's positions. Matrix tables for both are in the tutorial and in Sylvester's hardware note on the 49G keyboard.

The OUT lines are not only the keyboard's. On the 48GX's board they share pins with the address bus, and bit 11 drives the beeper: OUT set to 800 and back to 000 clicks it. OUT cannot be read back, so the system keeps a copy in RAM and restores OUT from it after each interrupt. A program that scans the keyboard itself, with interrupts on, has to update that copy before it writes OUT.

## The ON key

ON is wired to its own pin of the CPU. It shows as bit 15 of IN, whatever OUT holds, and its interrupt cannot be masked: INTOFF, which stops the keyboard interrupt, leaves ON working. Holding ON while pressing another key gives the 48's system functions:

| Keys | What they do |
|---|---|
| ON and + or − | darker or lighter display |
| ON and C | warm start: clears the stack and PICT, keeps memory |
| ON and A and F | cold start: offers "Try To Recover Memory?" |
| ON and B | cancels the other ON combinations, before ON is released |
| ON and D | interactive self-test |
| ON and E | looping self-test |
| ON and 4 | cancels repeating alarms |
| ON and 1 | screen dump |
| ON and SPC | coma mode: all clocks stop until ON, and the warm start log is cleared |

The other models have their own combinations: the 38G takes ON with a menu key to reset.

## The hardware scan

Unless the CPU has executed INTOFF, the chip itself scans the whole keyboard every millisecond, with no software involved, and raises an interrupt when a key goes down. Holding the key causes no further interrupts, and releasing it causes none.

So the operating system does the rest in its interrupt handler:

1. On the interrupt, it reads the matrix row by row to find the key.
2. It debounces: it samples the whole keyboard every 2 ms until five samples in a row agree. A keystroke therefore takes more than 10 ms to register.
3. It puts the key's code into a key buffer. While the calculator is busy, up to 15 keys wait there.
4. It sets the 16 Hz timer to come back every 1/16 s while any key is down, and so notices the release. Without that, two presses of the same key would look like one.

The shift and alpha annunciators are updated by the system outside this keyboard routine.

With the display turned off, the keyboard does not respond. And when SHUTDN has put the CPU to sleep, a key going down wakes it (see [Timers and interrupts](timers.md)).

## Emulating it

For an emulator, the timing matters more than the matrix:

- A simulated key press has to last long enough for the debouncing. In saturnus, a key held for 15 ms is sometimes lost on the 48SX; 25 ms is reliable on the 48SX, 48GX and 49G.
- A second press of the same key needs a gap, since the release is seen only at the next 1/16 s check. The 49G loses repeated keys with gaps under 70 ms.
- After some keys the system stays awake for a while: up to about 0.75 s on the 48SX after a function key, about 1 s on the 38G after ENTER. A key pressed in that time can be lost, so saturnus waits for the CPU to sleep before it sends the next key it has queued.

The [HP 48SX page](../48sx.md) and the [HP 49G page](../49g.md) describe the two keyboards as their owners see them.

## Sources

- [Introduction to Saturn Assembly Language, 3rd edition (2005)](https://www.hpcalc.org/details/1693), by Gilbert Fernandes and Eric Rechlin: the 48 and 49G matrices (p. 136-137).
- [HP48SX Keyboard Input: a guide for the ML programmer, by Joe Ervin (input.zip)](https://www.hpcalc.org/details/1720): the 1 ms hardware scan, no release interrupt, the debouncing, the timer that watches for release, ON in a column of its own.
- [Guide to the Saturn Processor, by Matthew Mastracci](https://www.hpcalc.org/details/1753): OUT and IN, combined rows, ON on its own pin, the OUT lines on the address bus.
- [HP48 S, SX, G, GX Interrupts: Tricks to Know, by Duchesne](https://sites.google.com/view/mnavarrodocs/home): OUT restored from its copy, ON combinations known to the handler, no interrupt while a key is held.
- [Voyage au centre de la HP 48 G/GX, by Paul Courbis and Sébastien Lalande](https://literature.hpcalc.org/community/hp48gx-voyage.pdf): the beeper bit, ON with interrupts off, the keyboard with the display off.
- [HP 49G Keyboard Hardware Note, by Sylvester (keyb49.zip)](https://www.hpcalc.org/hp49/docs/programming/keyb49.zip): the 49G's matrix.
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: the ON key combinations (4.6), by Dave Arnett, who worked on the G series.
- [HP 48G Series User's Guide](https://literature.hpcalc.org/community/hp48g-ug-en.pdf): up to 15 keys kept while busy (1-2).
- [HP 38G User's Guide](https://literature.hpcalc.org/official/hp38g-ug-en.pdf): the 38G's reset keys (9-8).
