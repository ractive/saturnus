---
title: How Saturnus is built
page_title: How Saturnus is built: an HP emulator in Rust and WebAssembly
description: How Saturnus, an emulator of HP calculators, is built: a Rust core that also runs as WebAssembly, a browser app, a Tauri desktop app, the clean-room method and the tests.
section: index
wiki_link: true
---

Saturnus is an emulator of twelve HP calculators, written in [Rust](https://www.rust-lang.org/). One core runs in the browser as [WebAssembly](https://webassembly.org/), natively in a desktop app and in two command-line tools. It was written from documentation and from the behaviour of the calculators' own ROMs, not from the code of other emulators. This page says how it is put together and where its limits are. The code is [on GitHub](https://github.com/ractive/saturnus) under the MIT licence.

## One core, several hosts

| | |
|---|---|
| The core | The crate `saturnus-core`: the Saturn CPU, the bus with its memory controllers, the display, the keyboard, the timers, the serial port and each model's wiring. No I/O, no threads, no dependencies. |
| The host code | `saturnus-host`: what every front end shares, as one state machine. Commands and a clock go in; replies, events and a deadline come out. |
| In the browser | The core compiled to WebAssembly, in a Web Worker behind a static page. |
| On the desktop | A [Tauri](https://tauri.app/) app: the same page, with the core linked natively on a thread of its own. |
| On the command line | `saturnus` runs a calculator without a window; `satx` moves files to and from real calculators. |

The core knows nothing about its host. It takes a ROM, runs a number of CPU cycles, takes key presses and serial bytes, and hands back the display:

```rust
use saturnus_core::io::Key;
use saturnus_core::{Machine, Model};

let rom = std::fs::read("gxrom-r")?;
let mut m = Machine::new(Model::Hp48gx, &rom)?;

// Two seconds of machine time: the 48GX runs at 4 MHz.
m.run_cycles(8_000_000).map_err(|h| format!("halted: {h:?}"))?;

m.key_down(Key::Enter)?;
m.run_cycles(200_000).map_err(|h| format!("halted: {h:?}"))?;
m.key_up(Key::Enter)?;

// The screen as text, one character per pixel.
println!("{}", m.lcd().to_text());
```

Because the core does no I/O and starts no threads, the same code builds for `wasm32` and for native targets.

What the hosts must agree on sits one layer up. The page talks to the calculator in one protocol of commands and events, and the pacing (how long to run before the next frame, when to sleep) is part of the same state machine, which has no clock, timer or thread of its own. The Web Worker and the desktop app's thread are thin drivers around it. One test script runs through the state machine with a fake clock, through the native runner and through the Worker with the real WebAssembly, and all three must produce the same transcript.

## What is emulated

The emulator is the hardware; the calculator is HP's ROM. Saturnus models the [Saturn CPU](saturn/cpu.md) with its 4-bit nibbles and 64-bit registers, the [memory controllers](saturn/memory.md) that map ROM, RAM and the card ports, the [display](saturn/display.md), the [key matrix](saturn/keyboard.md), the [timers and the interrupt](saturn/timers.md) and the [serial port](saturn/serial.md). Everything else is the ROM's own code: every command, menu and error message.

The target is what the ROM needs, not cycle exactness. Each instruction is charged the cycle count that HP's assembler manual gives, or on the 48GX and the later models the counts published for the 48G's processor, plus a 13% stall for the display refresh. On top of that comes a factor per model, between 1.20 and 1.34, fitted to [a benchmark timed on real calculators](https://www.hpmuseum.org/forum/thread-9750.html). The cause of that factor is not known.

| Model | Real calculator | Saturnus |
|---|---|---|
| HP 48SX, a sum of 1,000 terms | 95.5 s | 95.5 s |
| HP 48GX, a sum of 100 terms | 5.9 s | 5.9 s |
| HP 49G with ROM 2.10, 100 terms in a FOR loop | 5.5 s | 5.4 s |

The 38G takes the 48GX's factor and the 39G and 40G the 49G's. The 42S, 27S, 17B, 17BII, 28S and 19BII run at 1 MHz with no factor, because none of them has been timed against a real calculator. [The CPU](saturn/cpu.md) has more on the clocks.

Some hardware is left out. The infrared port is not modelled on any model, so nothing is beamed or printed. In the app the 48SX and 48GX have no memory cards; the `saturnus` command takes RAM card files.

## In the browser

[The app](../app/) is a static page: plain HTML, CSS and JavaScript modules, with no framework, no bundler and no server code. The core runs in a Web Worker at the calculator's speed (or 2×, 4×, Max), sleeps while the calculator's CPU sleeps, and sends the display to the page when it changes. Its two views, Calculator and Files, share one running calculator per model.

- The ROM you choose and the saved states stay in the browser's IndexedDB. The site has no server code to send them to.
- A service worker keeps every file of the app, each checked against its SHA-256, so the app works offline and can be installed on a phone's home screen.
- The memory view reads variables, the stack and the flags from the emulated RAM. Changes go the other way through the calculator's own Kermit server, over the emulated serial port, so the calculator makes each change itself.
- [Files](transfer/files.md) reaches a real calculator on a cable through [Web Serial](https://developer.mozilla.org/en-US/docs/Web/API/Web_Serial_API). It runs the same Kermit and XModem code as satx, compiled to WebAssembly.
- [AI apps](ai-apps.md) use the calculators over [MCP](https://modelcontextprotocol.io/), through a relay in the browser and a local address in the desktop app.

## On the desktop

The desktop app is a Tauri 2 app with the same page as its front end. The core is linked natively and runs on its own thread, so the calculator keeps running while the window is hidden. It speaks the page's protocol, so the page does not know which host it runs on. Files gets native serial ports and the computer's folders. Installers are built for macOS on Apple silicon and for Windows and Linux on x86_64 and ARM. The macOS app is signed and notarised; the Windows installers are not signed yet.

## On the command line

`saturnus` runs a calculator from its ROM without a window. It presses the keys listed in a script, saves the screen as text or as a PNG, saves and loads states, opens the calculator's serial port on a TCP port and serves a local control API. It is made for automated tests and scripts, not for using a calculator by hand.

[satx](transfer/satx.md) moves files between a computer and a real HP 48 or 49G by [Kermit and XModem](transfer/kermit-xmodem.md). Its protocol code is in crates of its own, `kermit-proto` and `xmodem-proto`, which do no I/O either: they take bytes and return bytes, and so also run in the browser.

## The clean-room method

Other emulators of these calculators exist, most of them under the GPL. Their source code is off limits to anyone writing Saturnus's code, human or AI, even to look up one fact. Only a reviewer who checks that Saturnus is independent of them may compare the two. Facts come from what is published: HP's manuals and journal articles, the hardware literature, the documentation and change logs of other emulators, forum posts by their authors, and runs of the ROMs themselves.

Each fact is first written down with its source in a public knowledge base, [calculator-knowledgebase](https://github.com/ractive/calculator-knowledgebase), and the code is written from there. A comment in the code names the page it relies on. Where the sources disagree, the comment says so and which one was followed.

One other emulator is used, as a black box: [saturnng](https://codeberg.org/gwh/saturnng) runs in a container with the same ROM and the same keys, and the two screens are compared pixel by pixel. That comparison found a real bug: the status bits of the 48SX's card ports pair with the chip-select lines, not with the port numbers one document gives.

Some answers were in no document. The 39G and the 40G share one ROM, which tells them apart by a single input line. Which line, and how a 40G holds it, was found by running the ROM, and [the 40G's page](40g.md) says so.

Saturnus ships no ROM: the ROMs are HP's software. `saturnus rom fetch` and the desktop app download a ROM from hpcalc.org after asking, and check its size and SHA-256.

## Tests

- The CPU's instructions are unit tests in the core: register state in, register state out.
- Tests that need a ROM run only where a folder of ROMs is set, and skip elsewhere. The project's CI has no ROMs, so these run on the developer's computer before a release.
- The differential tests replay scripted scenarios on Saturnus and on saturnng, for the 48SX, 48GX and 49G: starting up, arithmetic, menus, alpha entry, off and on, memory cards.
- satx's end-to-end tests transfer files with the emulated 48SX, 48GX and 49G over their emulated serial ports. The same suite runs against saturnng.
- The benchmark above is a test too, and so is a comparison of what the memory view reads from RAM with what the calculator itself reports over Kermit.
- The examples in the [command reference](commands.md) are not written by hand. A generator runs each one on the emulated calculator and records the result.
- The page's tests run in headless Chrome, driven over its DevTools protocol.

The other nine models have no other emulator to be compared with here. The 38G, 39G and 40G are tested against recorded screens, and the 38G against a real one where a result looked wrong; the 42S, 27S, 17B, 17BII, 28S and 19BII by starting, taking keys and passing their own self-tests. There is no fuzzing and there are no property tests yet.

## How it was written

Parts of the code were generated by AI under human supervision; the repository's [AI_NOTICE](https://github.com/ractive/saturnus/blob/main/AI_NOTICE) says so. The plans of each step and the decisions taken, with their reasons, are in the repository's `kb/` folder. The [decision log](https://github.com/ractive/saturnus/blob/main/kb/decision-log.md) is the place to look for why something is the way it is.

## Using the crates

The libraries are on crates.io, with their documentation on docs.rs:

- [saturnus-core](https://docs.rs/saturnus-core): the emulator core.
- [saturnus-objects](https://docs.rs/saturnus-objects): RPL objects and the user memory, read from a machine's RAM.
- [saturnus-host](https://docs.rs/saturnus-host): the front ends' protocol, typing, ROM identification.
- [kermit-proto](https://docs.rs/kermit-proto) and [xmodem-proto](https://docs.rs/xmodem-proto): the two transfer protocols.
- [satx-core](https://docs.rs/satx-core): satx as a library.

`cargo install saturnus satx` installs the two command-line tools. The repository's [README](https://github.com/ractive/saturnus/blob/main/README.md) covers building the app and the desktop app from source.

## Sources

- [The Saturnus repository](https://github.com/ractive/saturnus): the code, and its README on the models, the speed, the command line and the tests.
- [Architecture](https://github.com/ractive/saturnus/blob/main/kb/docs/architecture.md): the crates and what each one does.
- [The clean-room rule](https://github.com/ractive/saturnus/blob/main/kb/docs/clean-room-rule.md): what may be read and what may not.
- [Test policy](https://github.com/ractive/saturnus/blob/main/kb/docs/test-policy.md): which tests exist and when they run.
- [Decision log](https://github.com/ractive/saturnus/blob/main/kb/decision-log.md): the accuracy target, the timing and its calibration, one protocol for every host.
- [The page's protocol](https://github.com/ractive/saturnus/blob/main/web/protocol.md): the commands and events between the page and the calculator.
- [HP Museum forum: Summation based benchmark for calculators](https://www.hpmuseum.org/forum/thread-9750.html): the times on real calculators.
