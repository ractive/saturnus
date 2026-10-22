---
title: Timers and interrupts
description: The Saturn's two hardware timers, how the calculator keeps the time with them, the single interrupt and its sources, what can and cannot be masked, and sleeping with SHUTDN.
section: saturn
order: 5
---

The Saturn calculators have two hardware timers and one interrupt. The timers keep the clock and the alarms, watch the keyboard and wake the CPU from sleep; the interrupt comes from the keys, the timers and the serial port. Between key presses the CPU sleeps.

## At a glance

| | |
|---|---|
| TIMER1 | 4 bits, counts down at 16 Hz |
| TIMER2 | 32 bits, counts down at 8,192 Hz |
| Time base | a 32 kHz crystal, which also clocks the CPU, multiplied up |
| Interrupt | one, not re-entrant; the CPU pushes PC on the return stack and jumps to a fixed address in the ROM |
| Maskable | the keyboard's interrupt, with INTOFF |
| Not masked by INTOFF | ON, the timers, the serial port |
| Sleep | SHUTDN, until a timer, a key or the serial port wakes the CPU |

## The two timers

Both timers sit in the hardware registers and count down by themselves.

**TIMER1** is a single nibble that counts down from 15 to 0 in steps of 1/16 s. The system uses it for the keyboard: while a key is down, it comes back every 1/16 s to see whether the key is still held (see [The keyboard](keyboard.md)).

**TIMER2** has 32 bits and counts down 8,192 times a second, from FFFFFFFF to 00000000 and round again. That is the calculator's clock. The command `TICKS` gives the time in the same unit, 1/8192 s.

Each timer has a control nibble with four bits:

| Bit | TIMER1 | TIMER2 |
|---|---|---|
| 0 | an extra function, not described | the timer runs |
| 1 | an interrupt when it passes zero | the same |
| 2 | wake the CPU from SHUTDN when it passes zero | the same |
| 3 | set when it has passed zero; the handler clears it | the same |

Only "coma" mode, entered with ON and SPC on the 48, stops TIMER2. If the interrupt handler finds TIMER2 stopped at any other time, it halts with "Clock corrupted".

## Keeping the time

The calculator does not count seconds in software. It keeps an offset in RAM, and the time is that offset plus what TIMER2 has counted since the system last loaded it. On each timer interrupt the handler adds TIMER2's count to the offset, checks the alarms and loads TIMER2 again:

- with 1 s while the clock is shown on the screen, so that it ticks,
- otherwise with the time to the next alarm, or one hour, whichever is sooner.

The handler counts its own instructions so that reloading TIMER2 does not lose time. Because TIMER2 is 32 bits wide, the system can miss its interrupts for about 72 hours (2³¹ ticks) without losing the time.

## The interrupt

There is one interrupt line into the CPU. When it fires, the CPU pushes the program counter on the return stack and jumps to a fixed address near the start of the ROM, where the system's handler starts. It also sets an "in service" flag. Until the handler returns with RTI, no other interrupt is taken, maskable or not: an interrupt that arrives meanwhile is kept and taken right after RTI.

What can raise it:

- a key going down, from the 1 ms hardware scan,
- the ON key,
- TIMER1 or TIMER2 passing zero, with the interrupt bit set,
- the serial port: a character received, or the transmit buffer empty (see [The serial port and infrared](serial.md)),
- the infrared receiver,
- a low battery,
- a card put in or taken out.

The handler finds out which by reading the hardware registers.

## Masking

INTOFF and INTON turn off and on the keyboard's interrupt, the 1 ms scan. They do not stop ON, the timers or the serial port. Joe Ervin and Duchesne say so, and the ROMs run in saturnus with only the keyboard masked; Mastracci's guide counts the timer and serial interrupts as maskable as well.

The system has its own way to turn off everything, ON included. It keeps a bit in the CPU's status register (ST bit 15) that says whether interrupts are allowed. When that bit is clear, the handler returns at once with an ordinary return instead of RTI, notes in ST bit 14 that an interrupt was missed, and leaves the CPU "in service". No further interrupt is taken until the system allows them again: it sets the bit, executes RSI, which makes the CPU look again at what is pending, and then RTI.

The handler needs one or two levels of the eight-level return stack. A machine-language program that calls subroutines seven or eight deep while interrupts are on (six is safe) loses its oldest return address, returns to address 0 and warm-starts the calculator.

## The handler's checks

Before anything else, the 48's handler checks that the calculator is healthy. On the 48SX, every memory controller must be configured; on the S and the G series TIMER2 must run, a test word in RAM must be intact and the clock's offset must pass its checksum. When a check fails, the calculator warm-starts and writes the reason to a log of the last four warm starts. The command `WSLOG` shows it, with codes such as low battery, time corrupt, RAM test word corrupted, device configuration abnormal and card pulled.

## Sleeping

SHUTDN stops the CPU in a low-power mode. It wakes when:

- a timer passes zero with its wake bit set,
- a key goes down,
- the serial receiver raises an interrupt.

The calculator spends most of its time there. After every key it finishes the work, arms TIMER2 for the next alarm or the next tick of the shown clock, and executes SHUTDN. Turned off with OFF, it sleeps the same way with the display off, and ON wakes it.

There is a deeper level as well. When the batteries are very low, the handler can put the machine into "deep sleep" to keep RAM alive. Coma mode, from ON and SPC, stops all clocked activity, the timers included; the battery then only holds the contents of RAM.

## Sources

- [Guide to the Saturn Processor, by Matthew Mastracci](https://www.hpcalc.org/details/1753): the two timers, their widths, rates and control bits, the interrupt sources, INTON and INTOFF, the ST bits of the system's handler.
- [HP48 S, SX, G, GX Interrupts: Tricks to Know, by Duchesne](https://sites.google.com/view/mnavarrodocs/home): what the handler does in which order, its checks, timekeeping with TIMER2 and its self-compensation, the return stack levels it needs, which interrupts are maskable.
- [HP48SX Keyboard Input, by Joe Ervin (input.zip)](https://www.hpcalc.org/details/1720): INTOFF stops only the keyboard, the 72 hours, re-enabling interrupts with RSI and RTI, deep sleep on low batteries.
- [Introduction to Saturn Assembly Language, 3rd edition (2005)](https://www.hpcalc.org/details/1693), by Gilbert Fernandes and Eric Rechlin: the in-service flag, no re-entry, the wake conditions of SHUTDN, the crystal.
- [Voyage au centre de la HP 48 G/GX, by Paul Courbis and Sébastien Lalande](https://literature.hpcalc.org/community/hp48gx-voyage.pdf): the timers' control bits, the values TIMER2 is loaded with, coma mode, RSI.
- [HP SASM reference](https://www.hpcalc.org/hp48/docs/programming/sasmhtml.zip): RSI.
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: ON and SPC and coma mode (4.6), the warm start log and its codes (8.13).
- [HP 48G Series Advanced User's Reference Manual](https://literature.hpcalc.org/community/hp48g-aur-en.pdf): `TICKS` and its unit.
