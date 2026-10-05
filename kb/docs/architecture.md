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

Status (2026-10-05): CPU, bus, modules, I/O registers and the 48SX machine
boot the 48SX ROM to the memory prompt (iterations 1-2).

## Goal

A Saturn calculator emulator that is a library first: no UI inside, an API of
step / run / press key / read framebuffer / serial bytes in and out / save
and load state. On top of it: a headless CLI for tests and agents, an MCP
server, and UIs (pixel-faithful skins from the owner's own photographs, a
modern web UI). hptx uses it in-process for its tests.

## Architecture

```text
saturnus        core
  cpu/          decoder, ALU (nibble fields, BCD and hex modes), registers,
                flags, RSTK, interrupts (wiki: hardware/saturn-cpu)
  bus/          memory controller: six controllers, daisy-chain CONFIG/
                UNCNFG/RESET/C=ID, size-as-mask, priority, 48GX bank latch,
                49G flash banking (wiki: hardware/memory-controller)
  modules/      rom, ram, io-ram (#100-#13F), card ports, flash (49G)
  io/           display controller, keyboard matrix (IN/OUT, even-address
                quirk), timers (TIMER1/2, 8192 Hz), UART, IR, CRC register
  machine/      per-model wiring: hp48sx, hp48gx, hp49g, hp38g, hp39g
  state/        save and load (RAM, registers, controller config)
saturnus-drive  key scripts, idle wait, pacer, autostart; runner: the
                machine thread of the native hosts (front-end protocol,
                pacing, key queue, frames), shared by Tauri and the CLI
saturnus-cli    `saturnus run` (batch: --keys, --screen, --save; or
                serving: serial bridge + control API until Ctrl-C),
                `saturnus ctl` (the API's client), disasm, rom fetch
saturnus-web    wasm bindings and the protocol's host pieces (Emulator,
                KeyQueue), used natively by saturnus-drive's runner
saturnus-tauri  desktop app: the page in web/ on the shared runner
saturnus-mcp    retired (iteration 18 deletes it): the control API replaces it
```

Public API sketch:

```text
let mut m = Machine::new(Model::Hp48sx, rom_bytes)?;
m.reset();
m.run_cycles(200_000);           // or m.run_until_idle(max)
m.key_down(Key::On); m.key_up(Key::On);
let fb: &Framebuffer = m.framebuffer();   // 131x64 bits + annunciators
m.serial_push(&bytes); let out = m.serial_drain();
let snap = m.save_state(); m.load_state(&snap)?;
```

## Milestones
