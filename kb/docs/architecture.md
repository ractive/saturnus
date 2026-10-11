---
title: Architecture and goal
type: docs
date: 2026-10-04
status: active
tags:
  - architecture
  - saturnus
---

# Architecture and goal

Status (2026-10-08): seven models (48SX, 48GX, 49G, 38G, 39G, 40G, 42S)
boot and take keys; the CLI with its control API, the web page and the
desktop app run on them; 0.1.0 is prepared (iterations 1-23).

## Goal

A Saturn calculator emulator that is a library first: no UI inside, an API of
step / run / press key / read framebuffer / serial bytes in and out / save
and load state. On top of it: a headless CLI for tests and agents (serial
port and control API), and UIs (pixel-faithful skins from the owner's own photographs, a
modern web UI). satx (`crates/satx-core`) uses it in-process for its tests.

## Architecture

```text
saturnus-core   core (crates/saturnus-core)
  cpu/          decoder, ALU (nibble fields, BCD and hex modes), registers,
                flags, RSTK, interrupts (wiki: hardware/saturn-cpu)
  bus/          memory controller: six controllers, daisy-chain CONFIG/
                UNCNFG/RESET/C=ID, size-as-mask, priority, 48GX bank latch,
                49G flash banking (wiki: hardware/memory-controller)
  modules/      rom, ram, io-ram (#100-#13F), card ports, flash (49G)
  io/           display controller, keyboard matrix (IN/OUT, even-address
                quirk), timers (TIMER1/2, 8192 Hz), UART, IR, CRC register
  machine/      the machine: model.rs (each model's wiring behind its
                chip selects), hardware.rs (the bus outside the CPU), lcd.rs
                (131x64 or the 42S's 131x16), profile.rs (timing profile)
  state/        save and load (RAM, registers, controller config)
saturnus-objects RPL objects and user memory (stack, HOME tree, flags)
                read from RAM, Kermit transfer files, no I/O, builds
                for wasm32
saturnus-drive  key scripts, idle wait, autostart, files (capped reads,
                atomic writes); runner: the machine thread of the native
                hosts, a thin driver of saturnus-host's protocol engine
                (channel, wall clock, files, the native-only commands),
                shared by Tauri and the CLI
saturnus        the CLI (crates/saturnus-cli): `saturnus run` (batch: --keys, --screen, --save; or
                serving: serial bridge + control API until Ctrl-C),
                `saturnus ctl` (the API's client), disasm, rom fetch
saturnus-host   the front ends' shared host code, no bindings, no I/O,
                builds for wasm32: protocol (the front-end protocol and
                its pacing as one state machine, Engine: commands and a
                clock in, replies, events and a deadline out), Emulator,
                KeyQueue, command-line typing, the writes to the user
                memory as hidden Kermit transactions (kermit-proto over
                the emulated serial port), layouts, skins, ROM
                identification; used by saturnus-web, saturnus-drive's
                runner, the CLI and the Tauri app
saturnus-web    wasm bindings: the Engine with the Worker's pacing and a
                JavaScript clock (Host), ROM identification; web/worker.js
                is a thin driver (messages, one timer, the ROM slots in
                IndexedDB)
saturnus-tauri  desktop app: the page in web/ on the shared runner
saturnus-refgen generates the command reference (names, menus,
                examples run on the emulator) into saturnus-cli/data;
                not published
saturnus-kermit Kermit host for the ROM-gated tests and saturnus-refgen:
                the serial port driven in process on emulated time,
                kermit-proto, typed eval and variables; not published
kermit-proto,   the sans-IO Kermit and XModem protocols, build for
xmodem-proto    wasm32
satx-core       satx's library: transports (serial, TCP, saturnus
                in process), Kermit sessions, objects, XModem; a
                sans-I/O Machine for hosts without blocking reads
satx            the transfer CLI (crates/satx-cli)
saturnus-tx     saturnus tx's device layer over satx-core's Machine:
                connect (the speed and parity search, the model, flag
                -35 kept and restored), list, get, put, disconnect, the
                file names; no I/O, builds for wasm32; not published
saturnus-tx-web wasm bindings of saturnus-tx and the preview parts of
                saturnus-objects for the transfer page (web/tx.html,
                web/tx/); the protocol runs in its own Worker and the
                page holds the link (Web Serial, or the emulated
                calculator's serial port); not published
```

The two web pages share `web/shared/` (tokens, components, the icon
sprite, the device tree and list); each has its own wasm package
(`web/pkg/` from saturnus-web, `web/tx/pkg/` from saturnus-tx-web), and
the transfer page loads the emulator's only for an emulated calculator
([[docs/saturnus-tx]]).

Public API sketch:

```text
let mut m = Machine::new(Model::Hp48sx, rom_bytes)?;
m.reset();
m.run_cycles(200_000);
m.key_down(Key::On); m.key_up(Key::On);
let fb: &Framebuffer = m.framebuffer();   // 131x64 bits + annunciators
m.serial_push(&bytes); let out = m.serial_drain();
let snap = m.save_state(); m.load_state(&snap)?;
```

## Milestones
