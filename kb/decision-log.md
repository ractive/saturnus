---
title: Decision log
type: decisions
date: 2026-10-04
status: reference
tags:
  - decisions
  - saturnus
---

# Decision log

Decisions already made. Do not re-litigate; add a dated entry to change one.

## 2026-10-04

- **Name** `saturnus`, Latin for the god and the planet. Rejected: anything
  with "hp" in it (trademark exposure for a project that runs HP ROMs),
  `saturnite`, `kayamanu`, `encke`, `aerarium`.
- **License** MIT with `AI_NOTICE`; public repo; GitHub user `ractive`.
- **Language and layout** Rust, edition 2024. Crates `saturnus` (core: CPU,
  bus, modules, peripherals, machine configs; no I/O, no UI, WASM-able),
  `saturnus-cli` (headless runner), later `saturnus-mcp` and UIs.
- **Clean room** from documentation and ROM behaviour only; see
  [[docs/clean-room-rule]].
- **Target order** HP48SX (Clarke chip, no bank switching, 32 KB RAM), then
  HP48GX, HP49G, then HP38G/39G/40G.
- **Accuracy target** what the ROM needs, not cycle-exactness: TIMER2 running,
  the RAM magic word, module configuration on the 48S, battery and card
  switches on every interrupt; the ROM halts with "Clock corrupted" if TIMER2
  stops.
- **No emulator frameworks**: none exist for the Saturn; libretro is a
  front-end API for game consoles.

## 2026-10-04 (iteration 1)

- **Source precedence for CPU semantics**: HP's SASM manual
  (`raw/saturn-hardware/hp48-sdk-1993/SASM.TXT`) over the Fernandes/Rechlin
  tutorial over Gariepy and Mastracci. Where they disagree the code comment
  says so. Findings go back to the wiki page `hardware/saturn-cpu`.
- **Add/subtract constant**: always hexadecimal (SASM 2.7); single-nibble
  fields overrun circularly through all 16 nibbles, which reproduces the
  tutorial's examples. Pinned by a test; marked
  `TODO(questions/dec-mode-constant-bug)`.
- **A=IN / C=IN parity**: the even-address restriction is ignored; the
  failure mode on odd addresses is undocumented and the ROM always calls
  through CINRTN.
- **Undefined encodings** decode to `Instruction::Invalid`; the executor
  advances PC by the decoded length and reports an event instead of
  guessing hardware aliasing.
- **Opcode oracle**: the round-trip test against HP's `SASM.OPC` runs only
  when `SATURNUS_LITERATURE_DIR` points at the literature library. The file
  stays out of the repo (HP copyright).
- **Test code may use `unwrap`/`expect`**: `lib.rs` allows the clippy lints
  under `cfg(test)`; production code stays free of them.

## 2026-10-05 (iteration 2)

- **Interrupt maskability**: INTOFF masks only the 1 ms keyboard scan; the
  timers, ON and (later) UART/card sources are gated by their own enable
  bits and by the in-service flag (wiki: questions/interrupt-maskability).
- **Timer expiry**: the event is the counter's MSB going set (count through
  zero); control bit 3 (SRQ) is computed as MSB set and (INT or WAKE), not
  stored; the interrupt fires on the rising edge of MSB and INT. Inferred
  from Voyage and Emu48's change log; the 48SX ROM boots with it.
- **CRC follows data reads only**: `Bus::read_data` (DAT loads, PC=(A)/(C))
  feeds it; opcode fetches and I/O window reads do not.
- **CE2 above CE1** in access and UNCNFG priority (Giesselink), pending
  [[docs/open-hardware-questions]] `bus-priority-ce1-ce2`.
- **Open bus reads 0** for empty card ports and NCE3; the real value is
  undocumented.
- **CPU clock 2 MHz** for the 48SX; TIMER2 is derived from the approximate
  cycle counts, so emulated time drifts against a real calculator.
- **Golden screens** for e2e tests are text dumps of the LCD (`Lcd::to_text`,
  `#`/`.`); `SATURNUS_BLESS=1` rewrites them.

## 2026-10-05 (iteration 3)

- **Contrast reset value**: the contrast nibbles power on at 0 like every
  other I/O register; the ROM writes its own default (48SX ROM J writes 11,
  Emu48's listed 48SX reset value), so the emulator does not force one.
  `Model::contrast_range` reports the ROM's ON+/ON- range, 3-19 on the
  48SX (wiki: emulators/emu48 Display, questions/contrast-range-48gx).
- **Refresh stall model**: a flat 13% time surcharge on every instruction
  executed while DON is set (Voyage's measured on/off speed difference,
  wiki: hardware/display), not per-row bus contention. The tutorial's
  22-23 us per 244 us row would give about 10%; the measured figure wins.
  `Machine::cycles()` counts emulated time including stalls; SHUTDN time is
  not stretched.
- **Row counter while the display is off** keeps counting (inferred); it
  restarts from LINECOUNT when DON is set (wiki: emulators/emu48 SP30).
- **Card detect**: a card change with #10E bit 3 set latches SMP (#10E bit
  1); while SMP is set HST.MP is forced on, the CPU wakes from SHUTDN, and
  its rising edge raises a non-maskable interrupt. #10F reads 0 unless bit
  3 is set (wiki: hardware/card-ports, emulators/emu48 SP16/SP19,
  questions/register-10e-role). SWINT (#10E bit 0) is stored only.
- **#10F bit pairing** follows the chip select as ROM J uses it (code
  at addresses #09A18-#09A63, traced on saturnng and saturnus): bits 0 and 2 for the
  CE1 card (port 1), bits 1 and 3 for CE2 (port 2). Mastracci 4.3's order
  is wrong for the SX; the GX is unchecked (wiki: hardware/card-ports).
- **State format**: hand-written little-endian binary, magic `SATURNUS`,
  u16 version (1), model byte, FNV-1a checksum of the ROM nibbles, then all
  mutable state; layout documented in `crates/saturnus/src/state.rs`. No
  serde. The ROM itself is not saved, so a state loads only into a machine
  built from the same ROM. A format change bumps the version; old versions
  are rejected, not migrated, until a user needs that.
- **Key script format** (CLI): plain text, one action per line, `#`
  comments; `press KEY [ms]` (or a bare key name), `down KEY`, `up KEY`,
  `wait MS`, `wait-idle [CAP_MS]`; key names are `Key::name()`,
  case-insensitive; times are emulated milliseconds. A press holds 60 ms
  (well over the ROM's 10 ms debounce) and then waits for idle. Idle means
  the LCD unchanged for 300 ms while the CPU is in SHUTDN, capped (default
  10 s, a warning, not an error). The same file drives the saturnng oracle,
  so `down`/`up` are saturnus-only.
- **Screen text form**: exactly `Lcd::to_text()` (64 lines of 131 `#`/`.`,
  each ended by a newline, no border, no stripping). The diff script
  converts the oracle's raw tmux pane to this form instead of using
  `calc-screen`, which drops blank rows. Annunciators go to a separate
  one-line file (`Framebuffer::annunciator_line()`), so the text form
  stays the same as the container's pixel area.
- **PNG**: 131x64, 1-bit grayscale, dark pixel = black, no scaling and no
  annunciators, written with the `png` crate (no `image`).
- **ROM fetch** shells out to the system `curl` (its own user agent) and
  `unzip`, falling back to `tar`; SHA-256 is computed in the CLI without a
  crypto dependency. The checksum is enforced by `rom fetch` only; `run`
  checks the size, so other revisions of the same size still load.
- **Oracle flakiness**: the diff script replays a differing scenario once
  on a fresh oracle container (`ORACLE_RETRIES`) and reports every
  differing run. saturnus is deterministic and is not rerun.
- **CLI cards**: `--card1`/`--card2` take packed RAM-card images, inserted
  after `--load` and before the run; a missing file is created as a zeroed
  128 KB card. Card files are written back only with `--card-writeback`,
  never implicitly, because a saved state already contains the cards and a
  run should not change input files unless asked.

## 2026-10-05 (iteration 4)

- **UART model**: hardware only, one holding register and one shifter per
  direction; the 255-byte buffer is the ROM's (wiki: hardware/uart "Line
  behaviour"). Line time counts sixteenths of a bit (the receiver's 16x
  clock) from emulated cycles at the #10D baud rate. A frame is 11.375 bit
  times (182 sixteenths) in both directions; the host side is assumed to
  send like an HP 48, so pushed bytes arrive back to back one frame apart.
  RBF sets after the stop bit, 160 sixteenths in (io-guide 2.4.1);
  transmitted bytes reach the outbound queue at the same point.
- **TBR**: writing #117 (high nibble, the ROM writes low first) sets TBF;
  the byte moves to the shifter one sixteenth later, so TBF is briefly
  visible and the tx-empty edge is real.
- **UART interrupts**: USRQ is the level `SON and ((ERBZ and RBZ) or (ERBF
  and RBF) or (ETBE and not TBF))`; its rising edge interrupts. tx-empty
  follows the #110 register meaning (holding register empty), not
  Mastracci's transmit sequence (shifter done). Not masked by INTOFF,
  like the timers: Ervin 4.1.2 and Duchesne 2.1 against Mastracci 2.3
  (wiki: hardware/interrupts). A pending UART edge wakes SHUTDN.
- **SON clear, LPB, BRK**: clearing SON clears IOC, RCS, TCS, RBR, TBR and
  the shifters; RCS/TCS/TBR writes need SON (wiki: emulators/emu48). Bytes
  on the wire while SON is clear or LPB is set are lost (inferred). LPB
  feeds the transmitter into the receiver and nothing reaches the wire
  (inferred). BRK is a control bit ("send break", ROM J writes it at
  address #317A9); with LPB it reads back as one null byte with RER and RBF.
- **#118 USRQ at bit 0** is a placeholder: no source gives the bit and ROM
  J's handler polls IOC and RCS instead (traced boot, keys and a Kermit
  session: no read of #118). RCS bit 3 is stored; ROM J masks it off.
- **Serial API**: `serial_push` / `serial_drain` / `serial_pending` as
  contracted, plus `serial_baud`. The wire queues survive `Machine::reset`
  and are part of the saved state (format still version 1, unshipped).
- **TIMER2 pending read**: reads return #FFFFFFFF while a TIMER2 expiry
  has not been taken by the CPU (wiki: emulators/emu48 SP43); a write to
  TIMER2 ends it (inferred). In practice this only shows inside the
  handler.
- **Clock drift**: measured against ROM J's own clock display (flag -40),
  -0.7 ms over 600 s, below the 10 ms resolution of the test. The SASM
  cycle counts are good enough for timekeeping; no change to `cycles.rs`.
- **Serial bridge pacing** (CLI): `run --serial` keeps emulated time on
  wall-clock time at the model's clock rate (2 MHz), because Kermit
  timeouts on both ends are wall-clock. Slices of at most 1 ms of emulated
  time between socket polls; sleep when ahead; when more than 200 ms behind
  (stopped process, overloaded host) re-anchor instead of running a burst
  no real calculator would. Single-threaded with non-blocking sockets; the
  core stays free of I/O and threads.
- **Bytes with no client are dropped**: the bridge discards what the
  calculator sends while nobody is connected (the idle server's NAKs).
  The saturnng container's pty keeps them; hptx drains on connect either
  way.
- **`--autostart`** types the same keys as the saturnng container's
  `AUTOSTART` (NO at the boot prompt unless `--load`, then ALPHA ALPHA
  S E R V E R ENTER), then prints `serial bridged on tcp:HOST:PORT` on
  stdout as the readiness line. ENTER is held and released without
  waiting for idle, because the running server never idles in SHUTDN.
- **hptx in-process transport** lives in hptx (feature `saturnus`, a git
  dependency pinned to a saturnus commit), not here: saturnus exposes only
  the `Machine` serial API. It runs emulated time, not paced: `read` runs
  until the output goes quiet or the timeout's worth of emulated time has
  passed, and `write_packet` first replays the host's wall time spent
  outside the transport (at most 2 s), so hptx's turnaround pause reaches
  the calculator.
