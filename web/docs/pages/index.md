---
title: Saturnus docs
page_title: Saturnus docs: HP calculator models, commands and hardware
description: Docs for Saturnus, an emulator of classic HP calculators: the twelve models, every command, the Saturn processor, file transfer and how it is built.
section: index
---

Saturnus is an emulator of twelve classic HP calculators. It runs in the browser, as a desktop app and on the command line. These docs cover the models, their commands, how the Saturn processor inside them works, how to move files between a computer and a real calculator, and how Saturnus is built.

## Models

- [HP 48SX](48sx.md)
- [HP 48GX](48gx.md)
- [HP 49G](49g.md)
- [HP 38G](38g.md)
- [HP 39G](39g.md)
- [HP 40G](40g.md)
- [HP 42S](42s.md)
- [HP 27S](27s.md)
- [HP 17BII](17bii.md)
- [HP 17B](17b.md)
- [HP 28S](28s.md)
- [HP 19BII](19bii.md)

Each page tells the model's history, what makes it special and how Saturnus runs it, and answers the usual questions: where the ROM comes from, which computers and phones it runs on, and how files reach a real calculator.

## Command reference

[Every command of the 12 models](commands.md), with its stack effect, what it does and examples with their results from the emulated calculator. Search a model's commands by name or by what they do.

## How the Saturn works

[The hardware inside these calculators](saturn.md), for developers:

- [The CPU](saturn/cpu.md)
- [Memory](saturn/memory.md)
- [The display](saturn/display.md)
- [The keyboard](saturn/keyboard.md)
- [Timers and interrupts](saturn/timers.md)
- [The serial port and infrared](saturn/serial.md)

## Moving files to a real calculator

[For owners of a real HP calculator](transfer.md):

- [Cables](transfer/cables.md)
- [Kermit and XModem](transfer/kermit-xmodem.md)
- [satx](transfer/satx.md), the command-line tool
- [Files in the app](transfer/files.md)

## For AI apps

[AI apps](ai-apps.md): let Claude Code, Codex, Cursor, Claude or ChatGPT use the calculators, with the Saturnus skill and the Claude Code plugin.

## How Saturnus is built

[How Saturnus is built](how-it-is-built.md), for developers: the Rust core that also runs as WebAssembly, the browser app, the Tauri desktop app, the command-line tools, the clean-room method and the tests.

Every page is also available as Markdown: `/saturnus/docs/48gx/` as `/saturnus/docs/48gx.md`. [llms.txt](../llms.txt) lists them.
