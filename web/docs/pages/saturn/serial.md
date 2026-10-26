---
title: The serial port and infrared
description: The Saturn calculators' UART: the frame, the speeds, the line levels of the wired port, the buffers and interrupts, the infrared link through the same UART, and which model has what.
section: saturn
order: 6
---

One [UART](https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter) in the Clarke or Yorke chip drives both the wired serial port and the infrared LED. It sends and receives one byte at a time, with a holding register in each direction, and the system's interrupt handler moves the bytes to and from a buffer in RAM. Kermit and XModem run on top of it (see [Kermit and XModem](../transfer/kermit-xmodem.md)).

## At a glance

| | |
|---|---|
| Frame | 1 start bit, 8 data bits, the lowest first, at least 1 stop bit |
| Speeds | 1200, 2400, 4800 and 9600 baud on the wire; 2400 on infrared |
| Parity | none, odd, even, mark or space, checked in software |
| Throughput | at most about 844 bytes a second at 9600 baud |
| Levels | RS-232 compatible, about ±3.5 V out, up to ±15 V in |
| Buffers | one byte each way in the UART, 255 bytes for input in RAM |
| Flow control | XON/XOFF in software, if asked for; no hardware handshake lines |
| Infrared | the same UART, half duplex, a pulse for each 0 bit |

## Which models have it

| Model | Wired serial | Infrared |
|---|---|---|
| HP 48SX, 48S | yes | two-way |
| HP 48G, 48G+, 48GX | yes | two-way |
| HP 38G | yes | two-way |
| HP 39G | yes | two-way, to another 39G |
| HP 40G | yes | none |
| HP 49G | yes | none |
| HP 42S, 27S, 17B, 17BII, 19BII, 28S | none | one way, to HP's 82240 printer |

The [HP 48 FAQ](https://www.hpcalc.org/hp48/docs/faq/48faq.html) says the 49G was sold without infrared because teachers in some countries feared the 48's infrared would be used to cheat. The 49G and the 40G come with a cable from calculator to calculator instead.

## The frame and the speeds

A frame is a start bit, eight data bits sent lowest first, and the stop bits. The HP 48 sends slightly more than two stop bits: 11.375 bit times for each byte, which limits it to about 844 bytes a second at 9600 baud. Its receiver needs only one.

The speed is a 3-bit code in the hardware registers. The eight codes give 1200, 1920, 2400, 3840, 4800, 7680, 9600 and 15360 baud, but the system offers only 1200, 2400, 4800 and 9600. The [HP 49G FAQ](https://www.hpcalc.org/hp49/docs/faq/) notes 15360 baud "internally", which no PC supports. Both directions share one baud generator.

Parity is not done by the UART. All eight bits go through it, and the system adds or checks a parity bit in software when it takes bytes from its buffer. So the calculator can send with parity and not check it on receive.

## The line

HP's [I/O technical interfacing guide](https://www.hpcalc.org/hp48/docs/misc/io2.txt) for the 48SX gives the electrical side:

- A mark (a 1 bit, and the idle line while the port is open) is a negative voltage; a space (a 0 bit) is a positive one. That is the [RS-232](https://en.wikipedia.org/wiki/RS-232) convention, at lower voltages than a PC's port uses.
- With the port closed, the transmitter is switched off and the line sits at 0 V. Opening the port puts it at mark.
- The transmitter swings at least ±3.0 V, typically 3.5 V, into 3 kΩ and 500 pF, measured at the end of a 1 m cable.
- The receiver takes +1 to +15 V as a space and −15 to +0.3 V as a mark, with an input impedance of 5 to 7 kΩ.
- Bit widths may be off by 2.5% either way.

HP's own cables connect the calculator straight to a PC's serial port. The line is not a 3.3 V or 5 V logic signal: such a board idles at a positive voltage, which the calculator reads as a space. [Cables](../transfer/cables.md) tells which cable fits which model.

Early HP 49G units, with a serial number below 94xxxxxx, have a weak transmitter that swings only about −3.6 V to +4 V into 3.3 kΩ and ramps instead of switching. Some PC ports misread it. The fixes are an internal patch or a small external buffer powered from the PC's RTS and DTR lines.

## Receiving and sending

The receiver times each frame at 16 times the baud rate, from the start bit's leading edge. A start bit has to stay at space for half a bit, or the frame is dropped as noise. Then:

1. A status bit says a character is coming; an interrupt can be enabled for that.
2. After the stop bit, the byte is in the receive register, a "buffer full" bit is set and, if enabled, an interrupt raised.
3. The handler reads the byte, which clears the bit, and puts it into the 255-byte input buffer in RAM.

If a byte arrives before the last one was read, the UART sets an error bit (overrun); a stop bit that reads 0 sets it as well (framing). A break, the line held at space for a whole frame, reads as a null byte with the error bit set.

Sending is the other way round: a write to the transmit register sets a "buffer full" bit, the UART moves the byte to its shift register and clears the bit, and when the byte is out it can raise "transmit empty". While a character goes out, the handler lights the transmitting annunciator.

One timing rule matters to anyone writing a sender. A gap between incoming bytes of between four frame times and four frame times plus 5 ms can cause overruns, and timer interrupts (the shown clock, alarms) or key presses stretch those 5 ms. So a computer sending to an HP 48 should send bytes back to back, or pause clearly between them.

## Infrared

The infrared link uses the same UART. A bit in the hardware registers switches it from the wire to the infrared LED and receiver; while infrared is in use, the wired transmitter stays at mark and its receiver is ignored.

- Only 2400 baud, with the same framing as the wire.
- A 0 bit is one pulse of light of 52 µs, a 1 bit or an idle line is darkness, at 940 nm.
- It is half duplex: light from the calculator's own LED reflects off the window, so receive interrupts are off while it sends.
- Noise pulses look like start bits, so stray bytes, often FF, appear between transmissions.
- The receiver works up to about 5 cm; the transmitter reaches several feet.

The system can also drive the LED directly, bit by bit, without the UART. That is how the 48 prints to the HP 82240 infrared printer, which uses its own one-way protocol. The Pioneer models (17B, 17BII, 27S, 42S) and the 28S and 19BII have only that: an infrared printer port and no serial link.

In the 49G, which has no infrared, the bit that turns the LED on became the flash memory's write enable (see [Memory](memory.md)).

## Self-test output

When the 48 runs its self-tests (ON with D or E), it also writes the results to the serial port at 9600 baud, 8 data bits, no parity, whatever the user's settings.

## Sources

- [HP 48 I/O Technical Interfacing Guide](https://www.hpcalc.org/hp48/docs/misc/io2.txt), Hewlett-Packard: the frame, the stop bits and the throughput, the line levels and tolerances, parity in software, the receiver's timing, overrun, framing and break, the input buffer, the gap rule, the infrared pulses, wavelength, range and half duplex.
- [Guide to the Saturn Processor, by Matthew Mastracci](https://www.hpcalc.org/details/1753): one UART for wire and infrared, the speed codes, the receive and transmit sequences, the infrared and LED registers.
- [Voyage au centre de la HP 48 G/GX, by Paul Courbis and Sébastien Lalande](https://literature.hpcalc.org/community/hp48gx-voyage.pdf): the four speeds the system offers, the status bits, the infrared mode and direct LED control.
- [HP48SX Hardware Registers Document, by Taplin](https://www.hpcalc.org/hp48/docs/programming/hdwreg.txt): the speed codes for 1200 to 9600 baud.
- [HP48 S, SX, G, GX Interrupts: Tricks to Know, by Duchesne](https://sites.google.com/view/mnavarrodocs/home): the handler and the transmitting annunciator.
- [External serial buffer for the HP49, by Marcel Flipse (buf49.zip)](https://www.hpcalc.org/details/3009) and [Circuit to correct the HP49G serial port signal (serial49.zip)](https://www.hpcalc.org/hp49/docs/misc/serial49.zip): the early 49G's weak transmitter and its fixes.
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: the 49G's serial port and missing infrared (2.14), the infrared range (6.7), self-test output (4.7).
- [HP 49G FAQ](https://www.hpcalc.org/hp49/docs/faq/), maintained by Eric Rechlin: 15360 baud used internally.
- [HP 39G/40G User's Guide](https://literature.hpcalc.org/official/hp39g40g-ug-en.pdf): infrared on the 39G, none on the 40G (16-5).
- [HP 42S Owner's Manual](https://literature.hpcalc.org/community/hp42s-om-en.pdf), [HP 17BII Owner's Manual](https://literature.hpcalc.org/community/hp17bii-om-en.pdf) and [HP 28S Owner's Manual](https://literature.hpcalc.org/community/hp28s-om-en.pdf): printing to the HP 82240 infrared printer as their only output.
- [HP-48 Kermit Hints and Tips, the Kermit Project](https://www.columbia.edu/kermit/hp48.html): no modem signals on the HP 48.
- [Hewlett-Packard Journal, June 1996 (hpj-38g.zip)](https://www.hpcalc.org/hp38/docs/hpj-38g.zip): the 38G's serial port and infrared.
- [Introduction to Saturn Assembly Language, 3rd edition (2005)](https://www.hpcalc.org/details/1693), by Gilbert Fernandes and Eric Rechlin: the LED bit as the 49G's flash write enable.
