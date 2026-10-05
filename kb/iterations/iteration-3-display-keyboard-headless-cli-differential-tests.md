---
title: "Iteration 3: Display, keyboard, headless CLI, differential tests"
type: iteration
date: 2026-10-04
status: completed
branch: iter-3/display-keyboard-cli-differential
tags:
  - iteration
  - saturnus
---

# Iteration 3: Display, keyboard, headless CLI, differential tests

Read first: wiki `hardware/display`, `hardware/keyboard`,
`questions/display-start-address-taplin`.

## Context from earlier iterations (2026-10-05)

- The CLI crate is `saturnus-cli` with binary `saturnus` (decision log); the
  core crate must stay I/O-free, so ROM loading, PNG and TCP live in the CLI.
- The container's screen dump format is produced by
  `~/devel/hptx/emulator/calc-screen.sh`; match it exactly so the diff script
  can compare text files.
- Iteration 2 already landed much of the display and keyboard plumbing:
  `io/registers.rs` (DON, bit offset, contrast, DISP1CTL, LINENIBS,
  LINECOUNT, DISP2CTL, annunciator nibbles), `io/keyboard.rs` (`Key` enum
  for all 49 keys with `matrix()`, `name()`, `from_name()`; ON on IN bit
  15), `machine/lcd.rs` (`Lcd::render` from the bitmaps, `to_text` with
  `#`/`.`), `machine/mod.rs` (`Machine::{new, reset, step, run_cycles,
  key_down, key_up, peek, lcd}`), and `examples/boot.rs` as a bring-up tool.
  What is missing for this iteration: the row counter running at 4096 Hz
  with its readback, the refresh stall, a framebuffer type that carries the
  annunciators and contrast, the card-detect path, and save/load state.
- The iteration 2 child reported "no container runtime", which was wrong:
  Docker is Rancher Desktop at `~/.rd/bin/docker` (put `~/.rd/bin` on PATH).
  The oracle image is built from `~/devel/hptx/emulator/` (`docker build -t
  hp49g-emu .`, downloads the ROMs itself), run with `-e MODEL=48sx`, and
  driven with `docker exec <name> calc-keys ...` / `calc-screen` (TUI pixel
  character is a block, one char per pixel; trailing blanks stripped). Its
  README documents the key map. Normalise both sides to the same text form
  before diffing.
- The ROM only sees a key after about 10 ms of debouncing (five identical
  2 ms samples), so a scripted key press must be held for tens of
  milliseconds of emulated time; the container waits for the LCD to settle
  between keys.
- The disassembler (`saturnus::cpu::disassemble`) is available for a
  `saturnus disasm` subcommand and for trace output when debugging boot.

## Tasks

- [x] Display controller: row refresh at 4096 Hz ticks, start address, offset,
  line count, menu area, annunciators, contrast (range per
  `questions/contrast-range-48gx`).
- [x] Full keyboard matrix including ON and the shift keys (Mastracci has them
  swapped; the wiki page is settled).
- [x] `saturnus-cli`: load ROM, run, press keys from a script, dump the screen
  as text (same 131x64 text form as the container's `calc-screen`) and PNG,
  save/load state.
- [x] `scripts/diff-vs-saturnng.sh`: same key script on both, compare text
  screens. Three scenarios: boot, `6 7 * ENTER`, a menu walk.

- [x] Carried over from iteration 2: the boot scenario of the diff script
  also covers iteration 2's open acceptance criterion ("Try To Recover
  Memory?", NO, "Memory Clear" over the empty stack must match saturnng).
- [x] Carried over from iteration 2: card-detect interrupt source and card
  status register (#10F); today the ports are always empty and #10F reads 0.
- [-] Carried over from iteration 2: check the inferred hardware behaviour
  against saturnng and record the results in the wiki: the open-bus value for empty CE1/CE2/NCE3
  (now 0), SHUTDN with OUT = 0 (the SASM "cold start" case, not modelled),
  CE2-above-CE1 priority (`bus-priority-ce1-ce2` in
  `docs/open-hardware-questions`).

`[-]` = partly done: SHUTDN with OUT = 0 was observed in saturnng (wakes
through #0000F); the open-bus value and the CE2/CE1 priority are not
observable with the stock ROM and move to iteration 4, where Kermit can
deliver a machine-code probe.

## Acceptance criteria

- [x] The three scenarios (boot, arith, menu) match saturnng pixel for
  pixel; two more (alpha, offon) match as well.

## Outcome

### Core crate (`crates/saturnus`)

Landed:

- **Display row counter** in `io/registers.rs`: one row per 4096-Hz tick
  (two 8192-Hz timer ticks), counting 63 down to 0 and wrapping; switching
  DON on restarts it from the LINECOUNT value. The #128-#129 readback
  returns the row with M32 and DA19 preserved. The counter keeps running
  while DON is clear (inferred; nothing is drawn then).
- **Refresh stall** in `Machine`: with DON set every instruction costs 13%
  more time (Voyage's measured on/off difference), accumulated exactly in
  hundredths of a cycle. `Machine::cycles()` therefore counts emulated
  time, not instructions. SHUTDN time is not stretched.
- **Framebuffer**: `Machine::framebuffer()` returns `Framebuffer { pixels:
  Lcd, annunciators: Annunciators, contrast }`. Annunciators honour AON
  (#10C bit 3) and are dark while TIMER2 is stopped (Emu48 SP19).
  `to_text()` is unchanged; `annunciator_line()` names the lit ones.
  `Model::contrast_range()` gives the ROM's ON+/ON- range (3-19).
- **Keyboard**: the existing matrix matched the wiki cell by cell; a
  table-driven test now checks all 49 keys, including the shift keys, ON on
  IN bit 15 for any OUT, and the ignored buzzer bit (OUT bit 11).
- **Card detect**: `Machine::insert_card(Port, &[u8])`, `remove_card`,
  `card_image`. RAM cards sit behind CE1 (port 1) and CE2 (port 2), are
  writable, mirror modulo their power-of-two size (1-128 KB). #10F reports
  presence and write enable when #10E bit 3 (ECDT) is set and reads 0
  otherwise. A card change with detection on sets SMP (#10E bit 1); while
  SMP is set HST.MP is forced on and a non-maskable interrupt is raised on
  its rising edge. Ports stay empty by default; goldens unchanged.
- **Save/load state** in `state.rs`: `Machine::save_state()` and
  `Machine::load_state()`, hand-written versioned binary format bound to
  the model and a ROM checksum. Parser rejects truncated, out-of-range and
  trailing input and leaves the machine untouched on error. Round trip
  tested on a synthetic ROM and on the booted 48SX ROM (e2e).

Learned:

- The 48SX ROM J programs contrast 11 at boot, which is Emu48's 48SX
  "reset" value: the value comes from the ROM, not from the hardware.
- The ROM reaches "Memory Clear" at about 3.4 M cycles with the stall
  model; the goldens did not move.
- #10F pairs its bits by chip select: bits 0 and 2 (present, write
  enable) are the CE1 card (port 1), bits 1 and 3 the CE2 card (port 2),
  as ROM J's size test (#09A18-#09A63) reads them. Mastracci's order, which
  the first version followed, made `2 PVARS` say "Port Not Available"; the
  card scenario of the diff script caught it.
- Open: whether the row counter runs while DON is clear; whether #10E bit 0
  (SWINT) raises an interrupt when written (stored only); Voyage's "keyboard
  inactive with the display off" is not modelled; the exact stall pattern
  per row.

### CLI and differential tests (`crates/saturnus-cli`, `scripts/`)

Landed:

- **`saturnus` binary** (crate `saturnus-cli`, clap, anyhow, png):
  `run` (ROM size check, `--load`, `--cycles`, `--keys` script, `--screen`
  as `.txt` or `.png`, `--annunciators`, `--save`, `--trace N`, `-v`),
  `disasm --rom --at --count`, and `rom fetch` (confirmation prompt or
  `--yes`, system `curl` with its own user agent, `unzip` or `tar`, size and
  SHA-256 check, existing verified file kept). SHA-256 is a small local
  implementation, not a dependency.
- **Key scripts**: one action per line (`press KEY [ms]` or a bare key
  name, `down`, `up`, `wait MS`, `wait-idle [cap]`), key names from
  `Key::name()`, emulated milliseconds. `press` holds 60 ms and then waits
  for idle: LCD unchanged for 300 ms while the CPU is in SHUTDN, cap 10 s.
- **`scripts/diff-vs-saturnng.sh`**: one shared `keys.txt` per scenario,
  translated to saturnng TUI keys; the oracle runs with `MODEL=48sx
  AUTOSTART=0 CARDS=0` and waits for a stable LCD between keys. Both
  screens are normalised to `Lcd::to_text()` from the raw tmux pane (the
  plain `calc-screen` drops blank rows), and the annunciator row is
  compared too. Prints the first differing row and column. On a difference
  it replays once on a fresh oracle and reports both runs.
- **Results**: all five scenarios match pixel for pixel and by
  annunciators: `boot` ("Memory Clear" over the empty stack), `arith`
  (`6 ENTER 7 * ENTER`), `menu` (MTH, NXT, softkey A), `alpha`, `offon`
  (OFF, 2 s, ON). Save, load and replay give the same screen as a direct
  run.

Learned:

- saturnng's instruction log (`--debug-implementation`) gives a second,
  finer oracle. The cold boot to "Try To Recover Memory?" executes the same
  instructions in both emulators, apart from iteration counts of a loop
  that polls #00138.
- The oracle's TUI once delivered the NO key twice; the screen matched
  saturnus pressing F twice. Hence the replay on a difference.
- While the calculator is off, saturnng's TUI keeps drawing a display
  bitmap and the busy annunciator; saturnus shows a dark LCD because the
  ROM clears DON. OFF-state screens are not comparable, so no scenario ends
  there.
- Carry-over checks (wiki `hardware/memory-controller`,
  `hardware/card-ports`, `hardware/interrupts`, dated 2026-10-05): the 48SX
  OFF path executes SHUTDN at #04377 with OUT = #000, and saturnng wakes
  it through #0000F like saturnus, not by jumping to zero. The open-bus
  value of empty CE1/CE2/NCE3 and the CE2-above-CE1 priority are **not
  observable** with the stock ROM: it never reads the empty windows and
  never overlaps CE1 with CE2. The wiki lists the experiments that would
  settle them (a machine-code probe sent over Kermit; two cards). That task
  stays open.
- Open: `--trace` steps a shut-down CPU one cycle at a time (slow, but
  exact); the `del` key has no TUI mapping in the diff script.
- **Cards in the CLI** (added after the first review): `run --card1 FILE
  --card2 FILE` inserts packed RAM-card images after `--load`; a missing
  file becomes a zeroed 128 KB card. `--card-writeback` writes the images
  back at the end; a saved state already holds them, so `--save` alone
  does not. Scenarios take an optional `config` (`ORACLE_CARDS`,
  `SATURNUS_CARD1`, `SATURNUS_CARD2`).
- **`card` scenario fails** (oracle with its default card, saturnus with a
  zeroed 128 KB card, then `2 PVARS`). The oracle shows `{ }` and the free
  bytes; saturnus shows "PVARS Error: Port Not Available" with a card in
  either port. Cause, from both instruction traces and the ROM code
  between #09A18 and #09A63: the ROM reads #10F and RAM-tests the card
  flagged by bits
  1 (present) and 3 (write) at #C0000+#3FFFB, the CE2 window. It then tests
  the card flagged by bits 0 and 2 at #80000, the CE1 window. On the 48SX
  the ROM calls the CE2 card port 2. saturnus pairs the bits the other way
  (wiki: bit 1 = port 1), so the ROM's write/read-back hits an empty window
  and the card is dropped. saturnng's card file "port1" answers at the
  address #C0000 and the ROM sizes it as 128K. A scratch build with the #10F bit
  pairs swapped (bits 1/3 for the CE2 card, 0/2 for the CE1 card) matches
  the oracle pixel for pixel and shows `1 PVARS` = `{ }` 131072 for a CE1
  card. The fix belongs in the core (`io/registers.rs` CARD_P1/P2
  constants and the wiki's #10F table for the SX); the diff script exits 1
  until then.
- A RAM card present at power-on is not merged into user memory on the
  48SX in either emulator (`MEM` unchanged, 30269 bytes after Memory
  Clear); it shows up as an independent port.
