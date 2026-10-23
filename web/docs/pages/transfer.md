---
title: Moving files to a real calculator
description: For owners of a real HP calculator: which cable to use, the Kermit and XModem protocols and their settings, the satx command-line tool, and Files in the app.
section: transfer
---

These pages are for owners of a real calculator who want to move programs and data between it and a computer: back up a calculator, load a program from the internet, or copy what you wrote in saturnus to the real thing.

You need a serial cable for your model, a USB serial adapter on most computers, and a program on the computer: Files in the app, in Chrome or Edge, or satx on the command line.

## The pages

- [Cables](transfer/cables.md): which cable fits which model, the 48's wiring, the 49G's and 38G's connector, USB serial adapters and building a cable yourself.
- [Kermit and XModem](transfer/kermit-xmodem.md): which calculator speaks which, binary and ASCII transfers, the settings that matter, what to check when a transfer fails, and the programs on the computer.
- [satx](transfer/satx.md): the project's command-line tool, with the commands that list, copy, run, back up and restore.
- [Files in the app](transfer/files.md): moving variables in the browser between a folder, an emulated calculator and a real one.

## Which models

| Model | Serial port | Transfers with |
|---|---|---|
| HP 48S, 48SX | yes | Kermit: satx, Files |
| HP 48G, 48G+, 48GX | yes | Kermit and XModem: satx, Files |
| HP 49G | yes | Kermit and XModem: satx, Files |
| HP 38G | yes | Kermit, started on the calculator: Files serves it a folder |
| HP 39G, 40G | yes | Kermit and XModem, started on the calculator: Files serves it a folder |
| HP 42S, 27S, 17B, 17BII, 19BII, 28S | no | none: their infrared port only prints |

The calculator's side of a transfer is built in: on the 48 and the 49G a Kermit server that the computer drives, on the 38G, 39G and 40G the SEND and RECV menus of the Aplet Library. How the port works inside is on [The serial port and infrared](saturn/serial.md).

## Sources

- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: Kermit on the 48 series, XModem on the 48G series and the 49G.
- [HP 38G User's Guide](https://literature.hpcalc.org/official/hp38g-ug-en.pdf) and [HP 39G/40G User's Guide](https://literature.hpcalc.org/official/hp39g40g-ug-en.pdf): transfers started on the calculator.
- [HP 42S Owner's Manual](https://literature.hpcalc.org/community/hp42s-om-en.pdf): the infrared printer as the 42S's only output.
