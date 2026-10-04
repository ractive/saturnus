# saturnus plan

Status: empty repo (2026-10-04). The wiki has the hardware documented;
nothing is built.

## Goal

A Saturn calculator emulator that is a library first: no UI inside, an API of
step / run / press key / read framebuffer / serial bytes in and out / save
and load state. On top of it: a headless CLI for tests and agents, an MCP
server, and UIs (pixel-faithful skins from the owner's own photographs, a
modern web UI). hptx uses it in-process for its tests.

## Architecture

```
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
saturnus-cli    `saturnus run --model 48sx --rom ... --serial tcp:4848
                --keys "..." --screen out.txt|png --cycles N`
saturnus-mcp    later: tools press_keys, screen, type, read_stack, transfer
```

Public API sketch:

```rust
let mut m = Machine::new(Model::Hp48sx, rom_bytes)?;
m.reset();
m.run_cycles(200_000);           // or m.run_until_idle(max)
m.key_down(Key::On); m.key_up(Key::On);
let fb: &Framebuffer = m.framebuffer();   // 131x64 bits + annunciators
m.serial_push(&bytes); let out = m.serial_drain();
let snap = m.save_state(); m.load_state(&snap)?;
```

## Milestones

### M1 CPU core and disassembler

Read first: wiki `hardware/saturn-cpu`, `sources/saturn-tutorial`,
`sources/sasm-manual` (HP's own), `questions/dec-mode-constant-bug`.

- Registers A, B, C, D (64-bit, nibble fields P, WP, XS, X, S, M, B, W, A),
  R0-R4, D0, D1, PC, P, ST, HST (XM, SB, SR, MP from LSB), carry, DEC/HEX
  mode, 8-level RSTK.
- Decoder for the full opcode table; a disassembler in SASM syntax as a
  by-product (invaluable for debugging; HP's SASM.OPC table is the oracle).
- Quirks from the wiki: A=IN / C=IN only at even addresses; constant
  add/subtract field overrun (decide the DEC-mode question by experiment
  later, record it); pointer and P arithmetic always hex; SB on shifts.
- Tests: one test per instruction family with hand-computed results; a
  round-trip test decode -> disassemble over the whole opcode table.
- Acceptance: every opcode in SASM.OPC decodes; `cargo test` < 2 s.

### M2 Bus, modules, first ROM boot (48SX)

Read first: wiki `hardware/memory-controller`, `hardware/io-ram`,
`hardware/interrupts`, `hardware/timers`, `hardware/hp48sx`,
`sources/hpregint-en` (Duchesne), `emulators/emu48`.

- Memory controller with the 48SX default map (ROM 256 KB, 32 KB RAM at
  #70000, I/O RAM at #100, CE1/CE2 card ports, NCE3 unused).
- I/O RAM registers with correct reset values; TIMER1 and TIMER2 at
  8192 Hz with the wrap-through-zero interrupt; the interrupt entry sequence
  (#0000F handler, ON key, timer, card and UART sources; INTON/INTOFF per
  `questions/interrupt-maskability`).
- Enough keyboard (IN/OUT scan) for the ROM's boot checks; display registers
  writable even before the display is drawn.
- Acceptance: the 48SX ROM reaches "Try To Recover Memory?" and, after
  pressing NO, the stack display state in RAM matches what the saturnng
  container shows for the same sequence (compare via framebuffer in M3, via
  RAM dump of the display area in M2). This is the milestone where most
  bugs surface; budget for it.

### M3 Display, keyboard, headless CLI, differential tests

Read first: wiki `hardware/display`, `hardware/keyboard`,
`questions/display-start-address-taplin`.

- Display controller: row refresh at 4096 Hz ticks, start address, offset,
  line count, menu area, annunciators, contrast (range per
  `questions/contrast-range-48gx`).
- Full keyboard matrix including ON and the shift keys (Mastracci has them
  swapped; the wiki page is settled).
- `saturnus-cli`: load ROM, run, press keys from a script, dump the screen
  as text (same 131x64 text form as the container's `calc-screen`) and PNG,
  save/load state.
- `scripts/diff-vs-saturnng.sh`: same key script on both, compare text
  screens. Three scenarios: boot, `6 7 * ENTER`, a menu walk.
- Acceptance: the three scenarios match pixel for pixel.

### M4 UART, timers accurate enough for Kermit

Read first: wiki `hardware/uart`, `protocols/kermit-hp`.

- UART registers, 255-byte receive buffer, baud from IOPAR, TX/RX
  interrupts, the 11.375-bit-times-per-byte timing, overrun on inter-byte
  gaps, LPB loop-back.
- Serial API on `Machine`; `saturnus-cli --serial tcp:PORT` bridging like the
  container does, so hptx's e2e suite runs unchanged.
- Acceptance: `hptx ls/get/put/run` pass against saturnus over TCP and
  in-process (hptx gets a `saturnus` transport behind a feature flag).

### M5 More models

- 48GX: 128 KB RAM, bank-switched port 2 with the byte-read latch quirk,
  DA19 polarity (wiki settled it against Mastracci and Voyage).
- 49G: 512 KB RAM, 2 MB flash with banking and the write-enable path
  (`questions/hp49g-bank-latch-bits`, `hp49g-flash-write` to resolve by
  experiment), different keyboard.
- 38G, 39G/40G: wiki pages are stubs; sources needed first (HP Journal 38G
  article is in raw/, not yet ingested).
- Acceptance per model: boot, differential screens, hptx e2e.

### M6 UIs and MCP

- `saturnus-mcp`: press_keys, screen (PNG), type_text, read_stack (via
  Kermit host command or RAM), send/receive object.
- Pixel-faithful skin per model from the owner's photographs of the real
  calculators (no third-party KML artwork); modern web UI via WASM build of
  the core; optionally Tauri. Decided when M4 is done.

## Open hardware questions to settle by experiment

From the wiki's `questions/` (open as of 2026-10-04):
`dec-mode-constant-bug`, `bus-priority-ce1-ce2`, `config-check-in-handler`,
`hp49g-bank-latch-bits`, `hp49g-flash-write`, `interrupt-maskability`,
`register-10e-role`, `display-start-address-taplin`, `contrast-range-48gx`,
`binary-odd-nibble-padding`. Each resolution goes back into the wiki with the
experiment that settled it.
