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
- **Serial bridge backpressure** (CLI, after PR review): at most 2 KiB
  (`INBOUND_HIGH_WATER`) queued for the UART, one read of at most 1 KiB per
  loop pass; above the mark the bridge does not read the peer, so the
  kernel buffer and TCP flow control (stdio: a one-chunk `sync_channel`)
  throttle a sender faster than line rate, and the calculator keeps its
  time slices. A disconnected client's queued bytes still go out (they are
  usually its final ACK; discarding them cost every next session a 5 s
  timeout, suite 91 s to 249 s); the next client is accepted only once
  they have drained. `Machine::serial_clear_inbound` (core) drops the queue
  when the bridge stops, so a `--save` holds no stale wire bytes. A client
  that floods and then closes keeps the slot until its kernel-buffered data
  has drained at line rate.

## 2026-10-05 (iteration 5)

- **Per-model hardware description**: `Model::hardware()` returns a
  `HardwareProfile`, which names the device behind CE1, CE2 and NCE3
  (`ChipRole`: empty, RAM, card, banked card, bank latch). It also says
  whether NCE3 shares its pin with ROM A19, and gives the largest card
  per port. `Hardware` decodes through it, so no code branches on the
  model name. The six controllers and their priority are the same on every
  model (wiki: hardware/memory-controller, Giesselink). Power-on is the
  same too: everything unconfigured, latch 0.
- **NCE1 abstraction**: an enum `Nce1` (`Rom` today; the 49G adds its
  flash) with `read(addr, latch)` and `nibbles()`. It is an enum rather
  than a trait object so `Hardware` stays `Clone + Eq` and the core crate
  needs no boxing. Model wiring that only moves address lines, such as the
  GX's DA19, stays in `Hardware`.
- **48GX DA19**: DA19 = 1 gives A19 to the ROM. DA19 = 0 masks the ROM
  address to 19 bits, so the lower 256 KB repeat at #80000, and NCE3
  decodes only while BEN = 1. Otherwise NCE3 does not claim the address and
  it falls through to the ROM. Source: wiki questions/da19-polarity and
  emulators/emu48 SP9/SP16, against Mastracci 4.2 and Voyage p. 202.
- **48GX bank latch**: every nibble read in the CE1 window latches nibble
  address bits A1-A6. Bits 0-4 are the port 2 bank and bit 5 is BEN (wiki:
  hardware/memory-controller; Mastracci 4.4, Teuwen 4, tutorial p. 158).
  Peeks and writes do not latch. CPU reset and SHUTDN clear the latch
  (wiki: emulators/emu48 SP23). A read returns the open-bus value.
  Giesselink's three-nibble skew (tutorial p. 159-160) is not modelled,
  because ROM R latches with byte reads at #7F040+2n. With the exact
  latch, `33 PVARS` with a 4 MB card already gives "Invalid Card Data",
  as on saturnng (scenario `gx-card-p33`). The 4 MB-card quirk therefore
  needs no hardware skew to appear. Card images are laid out bank 0 first;
  Emu48 and saturnng port 2 files may place bank 0 differently (untested).
- **Card ports per model**: the 48SX takes cards up to 128 KB in both
  ports (CE1, CE2). On the 48GX, port 1 is CE2 with up to 128 KB, and
  port 2 is NCE3 with up to 4 MB, shown as 128 KB banks (wiki:
  hardware/card-ports, Mastracci 4.3). Every image must be a power of two
  of at least 1 KB, and smaller cards mirror inside a bank.
  `CARD_MAX_BYTES` is now the 4 MB limit over all ports;
  `Model::card_max_bytes(port)` is the per-port limit, and new cards are
  128 KB (`NEW_CARD_BYTES`). #10F bits follow the chip select on both
  models: bits 1 and 3 for the CE2 card, 0 and 2 for the other slot. This
  matches Mastracci's port names on the GX, and the `gx-card*` scenarios
  agree with saturnng.
- **Clock and contrast**: the 48GX runs at 4 MHz (wiki: hardware/hp48gx),
  with a contrast range of 9-24 (wiki: emulators/emu48, KML). The SX cycle
  counts are reused; Emu48 SP1 says the G series differs, which is not
  modelled yet.
- **State format version 2**: it adds the model code, the RAM behind
  CE1, CE2 and NCE3, the latch byte, and per-port card limits on load.
  Version 1 states are rejected.
- **Diff scenarios per model**: a scenario's `config` sets `MODEL`, which
  picks the ROM (`roms/gxrom-r` for `48gx`), `saturnus run --model` and the
  oracle's `MODEL`. `SATURNUS_CARDn_KB` sizes the card, so the oracle's
  4 MB GX port 2 can be matched.
- **38G as a configuration**: `Model::Hp38g` uses the 48G-style profile
  without card ports: CE1 bank latch, DA19 on the A19/NCE3 pin, CE2 and
  NCE3 empty, 32 KB on NCE2, 4 MHz, and the 48 key matrix with 48 key
  names. The controller wiring is inferred, not sourced (wiki:
  questions/hp38g-memory-controllers). ROM A1.67 accepts it: it configures
  NCE2 at #F0000 itself, boots to HOME, and takes key input. With no
  oracle, acceptance is golden screens recorded from saturnus (boot, OK,
  `6 * 7 ENTER`). State model code 3.
- **49G extension hooks**: the `Nce1` methods `set_write_enabled`
  (forwarded on every #11C write), `nce3_read`/`nce3_write` (tried first
  while the profile's `nce3_flash_path` is set and #11C bit 3 is on), and
  `state_blob`/`load_state_blob` (an NCE1 byte block in the state, loaded
  into a clone and committed with the rest). The profile flag
  `latch_writes` latches the 49G's CE1 on writes too (wiki:
  hardware/hp49g, tutorial p. 163-165). The state checksum binds to the
  NCE1 image as loaded.
- **49G flash chip**: `modules/flash.rs` models the Intel 28F160S5 from
  its datasheet (order 290609-004; wiki: sources/intel-28f160s5): 2 MB in
  32 erase blocks of 64 KB, read array, identifier, CFI query and status
  modes, program (bits only clear), block and chip erase, write to
  buffer, lock-bits with a WP# input, clear status. Every operation
  completes at once, so status always reads ready. The Saturn writes
  nibbles to a byte-wide chip: the even nibble is held, and the odd nibble
  of the same byte completes one byte cycle (low nibble first), so
  `DAT1=C B` at an even address is one command or data byte (assumption;
  wiki: questions/hp49g-flash-write). Writes reach the chip only while
  bit 3 of #11C is set; NCE3 then reads and writes the flash at the #40000
  view's bank. WP# defaults high: how the boot sector is protected is
  unknown.
- **49G bank latch bit order: Sousa, not Giesselink**: A1-A4 of the
  latching access pick the bank at #40000-#7FFFF, A5-A6 the bank at the
  low view #00000-#3FFFF (Sousa's `base + 2*n` / `base + #20*n`). The wiki had
  followed Giesselink's opposite assignment (A1-A2 low view, A3-A6 high
  view). Experiment: with Giesselink's order, ROM 2.15 runs into data
  within 1 M cycles, and ROMs 2.10 and 1.19-6 (original 49G boot sector)
  stop in their boot loader with "No System"; with Sousa's, all three boot
  to "Try To Recover Memory?" and 2.15 matches saturnng pixel for pixel.
  Reads and writes in the CE1 window both latch (`latch_writes`).
- **49G latch survives SHUTDN**: the GX's latch clear on SHUTDN (Emu48
  SP23) breaks the 49G: its OS executes SHUTDN while running from a
  switched low-view bank and resumes there, so a cleared latch puts bank 0
  under the running code (observed at #017E7 in the 2.15 boot). New
  profile flag `shutdn_clears_latch` (false on the 49G); CPU reset still
  clears the latch.
- **49G ROM image**: primary ROM 2.15 from `hp4950emurom.zip`, the
  saturnng oracle's image, although its readme labels it for the
  48gII/49g+/50g and its boot sector is not the original 49G one. ROM 2.10
  (`hp4950v210.zip`, original boot sector, also boots) is the documented
  fallback. `Machine::new` also accepts the unpacked 4 MB form (1.19-6).
- **One key set for all models**: `Key` holds every model's keys; a
  `Layout` (48 or 49G matrix, from `Model::keyboard_layout`) places them,
  and a key the model lacks is ignored. Keys with the same label share a
  name; the softkeys are `A`-`F` on both, with `f1`-`f6` as aliases. The
  alternative, a separate 49G key type, would have forced the CLI's key
  scripts and the autostart to route by model. The 49G's letter positions
  (APPS to the divide key = G to Z) were measured on saturnus by typing
  them with ALPHA locked, and scenario `49g-alpha` matches saturnng.
- **`wait-idle` needs a settled screen**: idle now also requires lit
  pixels or a switched-off display. The 49G boot spends seconds in SHUTDN
  timer waits with a blank screen, which the old rule took for idle; the
  saturnng container's `wait_stable` also waits for lit pixels.
- **49G acceptance**: goldens `49g-try-to-recover-memory`,
  `49g-memory-clear`, `49g-stack` (the last equals saturnng's screen); e2e
  boot, state round trip and Kermit file reception; diff scenarios
  `49g-boot`, `49g-arith`, `49g-menu`, `49g-alpha` match; hptx's e2e suite
  passes 6/6 against `saturnus run --model 49g --autostart` over TCP. The
  state format stays version 2: the flash goes into the existing NCE1 byte
  block (lock-bits, status, read mode, WP#, packed array).

## 2026-10-05 (iteration 6)

- **Web UI without a framework**: `web/` is plain HTML, CSS and one ES
  module loading the `wasm-pack --target web` output; no bundler, no npm
  dependencies, served by any static file server. The bindings crate
  `saturnus-web` depends only on `wasm-bindgen` and `js-sys`; structured
  values (`keys()`, `annunciators()`) are built as JSON in Rust and parsed
  with `JSON.parse`, so the logic stays unit-testable natively.
- **Web storage**: saved states go to IndexedDB (binary, one slot per
  model); localStorage keeps only the model choice. The ROM is never stored
  or uploaded: after a reload the user picks it again, and the core's ROM
  checksum refuses a state saved under another ROM.
- **Web pacing**: each `requestAnimationFrame` runs the wall time since the
  last frame, capped at 100 ms so a hidden or slow tab does not try to catch
  up, in 10 ms slices; `run_ms` carries fractional cycles so time stays
  exact at the model's clock.
- **Web key mapping**: drawn keys and the computer keyboard feed one queue.
  A press is held at least 60 ms of emulated time and queued presses start
  30 ms after the previous release, so fast typing is not lost in the ROM's
  debounce; a key still held by the user (ON for chords) does not block.
  Keyboard: digits, `+ - * /`, `.`/`,`, Space, Enter, Backspace, Delete =
  DEL, arrows, `'`, `^`, Escape = ON, F1-F6 = softkeys. Physical key
  layouts per model live in `saturnus-web`, not the core.
- **Crate split for host-side driving**: the CLI's key scripts, scripted
  session with its idle wait, per-model boot and SERVER autostart, ROM
  loading and PNG/text screen dumps moved into a new library crate
  `saturnus-drive`, used by both `saturnus-cli` and `saturnus-mcp`. It
  does file I/O, so it stays out of the wasm-clean core. The move kept
  the CLI's behaviour and tests; the session now also collects
  `wait-idle` warnings (echoed to stderr for the CLI, returned in tool
  replies for MCP), and `boot_script(model)` holds the answer to the first
  screen (NO; NO then OK on the 49G; OK on the 38G).
- **MCP SDK**: `rmcp` 3.5 (the official Rust SDK) with its tool macros and
  stdio transport, on tokio. The MCP crate sets `rust-version = "1.88"`
  because rmcp needs it; the rest of the workspace stays at 1.85. Argument
  schemas come from `schemars` derives. Tool failures are MCP tool errors
  (`isError`) with the whole message chain, never protocol errors.
- **hptx dependency**: `hptx-core` by git, pinned to hptx main
  `1a6cec7420ff540320c33607549eae10ecee76a1`, default features only, so no
  second, older saturnus is built. The server implements hptx's
  `Transport` over its own `Machine` (`link::MachineTransport`). An empty
  host command (`C` with no data) returns the stack unchanged; that is
  `read_stack`.
- **MCP time model**: the machine only runs while a tool runs, with no
  background thread. Keys run in emulated time with the CLI's idle wait.
  Kermit reads run emulated time in 1 ms steps until the reply has gone
  quiet for 4 ms or the client's timeout has passed in emulated time, then
  sleep out the rest of a timeout in wall time. Writes first replay the
  wall gap since the last call (at most 2 s), which covers hptx's 200 ms
  turnaround. The Kermit client times out after 6 s per packet with 3
  retries instead of hptx's 20 s and 5, so a calculator that left server
  mode fails a tool in about 24 s. Stale bytes are dropped at the start of
  every Kermit tool and after key scripts.
- **MCP session lock**: one tokio mutex around the emulator; every tool
  holds it and runs on a blocking thread, so a key script and a Kermit
  exchange never overlap. While the server runs, `press_keys` and
  `type_text` are refused (`stop_server` sends Kermit FINISH and waits
  2.5 s for the 48SX's lost-keys quirk). `load_state` and `reset` mark the
  server as stopped.
- **type_text map**: letters through alpha mode (one capital: ALPHA then
  the key; longer runs: ALPHA ALPHA, left shift before a lowercase letter,
  ALPHA to unlock), with the 48 letters on the six-key rows and Y, Z on
  +/- and EEX, and the 49G letters from wiki hardware/keyboard; digits,
  `. + - * /`, space, newline = ENTER. The 38G types no letters and no
  space (its alpha keys differ and that key is the comma). Checked on the
  48SX and 49G screens with `abc Xy Q1 Z`.
- **UART request is a level at RTI**: RTI re-enters the handler while
  USRQ is held, as it already did for a held ON key (wiki:
  emulators/emu48 "RTI re-enters at once if ON is pressed, NINT or NINT2
  is low"; treating USRQ like NINT is inferred). Fixes the 48SX Kermit
  server going deaf: a start bit's edge vectored while ST bit 15 was
  clear, ROM J's handler returned without RTI, its later RSI/RTI
  (AllowIntr, #010E8-#01113) found no new edge, and RBR stayed unread
  with RBF and RER set. Regression test
  `hp48sx_kermit_server_hears_packets_right_after_a_nak` (wiki:
  hardware/uart, "Facts settled while building saturnus (2026-10-06)").
- **MCP call limits**: a `press_keys`/`type_text` call may ask for at most
  10 minutes of emulated time (holds, waits and idle caps summed, checked
  before running) and 64 KiB / 2000 lines of script; every call also runs
  under a 15 minute wall-clock deadline checked every 50 ms of emulated
  time (`saturnus_drive::session::Limits`), so a client cannot wedge the
  session lock. `save_state` writes through a temp file and rename, never
  the session's ROM, and replaces a non-state file only with
  `overwrite: true`; `load_state` drops the Kermit client only after the
  state loaded. From the PR review of iteration 6.

## 2026-10-05 (iteration 5b)

- **39G/40G are 49G machines cut down**: `Model::Hp39g` and `Model::Hp40g`
  share one `HardwareProfile` apart from the strap below: the CE1 bank
  latch clocked by reads and writes, `shutdn_clears_latch` false, CE2 and
  NCE3 empty, no flash write path (`nce3_flash_path` false), 256 KB RAM on
  NCE2, 4 MHz (wiki: hardware/hp39g-40g; the latch and SHUTDN behaviour
  are inferred from the shared design). The ROM's cold start configures
  only HDW at #00100 and NCE2 as 256 KB at #80000; it configures CE1 as a
  4 KB window at #7E000 just long enough to latch (`CONFIG #FF000`,
  `CONFIG #7E0nn`, access, `UNCNFG`) and never touches CE2 or NCE3.
- **Banked mask ROM**: a new `Nce1::BankedRom` reads the 1 MB ROM through
  the 49G's two views with Sousa's latch bits and takes the bank modulo 8,
  so high-view banks 8-15 mirror 0-7 (inferred, as Emu48 mirrors small
  ROMs; wiki: hardware/hp39g-40g). The ROM runs with latch 9 (high bank
  9 = 1) and scans banks 0-7 with latches 0-7, which fits the mirror. No
  write path, empty state blob.
- **ROM image**: hpcalc's `rom.39g` (from `../hp39/pc/rom3940.zip`, the
  link on details 6739; the `hp39/pc/rom/` URL is a 404) is 1 MB unpacked,
  one nibble per byte, SHA-256
  `69220f42d5e90dd8825e7d1596d9eaca490ee6a7a52a3b8b96469a5f3d3f627f`. It
  carries the I/O registers of the calculator it was read from at the
  nibbles #00100-#0013F (`0 F D F 3 0 0 0 2 0 0 3 ...`, where the 48GX and
  38G images hold zeros); `Machine::new` zeroes those 64 nibbles, as Emu48's
  Convert does. The packed 1 MB form is accepted too
  (`Model::accepts_rom_len`), and `rom fetch --model 40g` fetches the same
  file.
- **40G strap = #11A bit 3**: the ROM has one RPL primitive (its code at
  nibble #66F39, bank 1) that reads #11A-#11B and returns TRUE when bit 3
  is set;
  its only caller, ROMPTR target #66F16 in the bank-1 library, picks one of
  two objects with it. On the 39G that bit is the IR receive sample
  (wiki: hardware/uart "IR"). With bit 3 held high the ROM shows a CAS
  label on menu key 6 at HOME, the 40G's look; with it low, the 39G's
  blank key. So the 40G profile holds #11A bit 3 high
  (`HardwareProfile::io_strap`, ORed into every read of that register);
  inferred: a 40G board without the IR receiver reads that line high. The
  40G's missing IR needs nothing else, since IR is not emulated.
- **Model-specific key labels**: `Layout::Hp38` (48 matrix) and
  `Layout::Hp39` (49G matrix) map 17 new `Key` variants (`plot`, `num`,
  `lib`, `math`, `home`, `xt`, `lparen`, `rparen`, `shift`, `comma`,
  `aplet`, `views`, `vars`, `ddx`, `ln`, `log`, `square`) and shared
  labels to the 48/49G key at the same place on the case, per the wiki
  tables. A shared label keeps its variant at a different position (SIN
  on the 38G is the 48's COS cell); a name means the key with that label
  on the model, so 48 names such as `mth` are refused on the 38G. One
  layout serves the 39G and 40G (the 40G's is assumed identical; wiki:
  hardware/hp39g-40g).
- **type_text on the aplet models**: the 38G has no alpha lock (a second
  A...Z cancels the first; observed on ROM A1.67), so each letter is
  A...Z then its key, a lowercase one SHIFT A...Z then its key; A-Z were
  checked on screen. The 39G and 40G type no letters: no source read gives
  their letter positions. None of the three types a space.
- **Acceptance without an oracle**: e2e goldens `39g-memory-clear`
  (cold boot), `39g-home`, `39g-six-times-seven`, `39g-warm-reset`
  (ON + menu key 3; HOME with an empty history display) and the memory
  clear chord (ON + menu keys 1 and 6) back to `39g-memory-clear`;
  `40g-home` (CAS label) and `40g-six-times-seven`. The differential
  script refuses MODEL=38g/39g/40g, since saturnng emulates none of them.
- **Transfer protocol is Kermit, calculator as client**: SEND to a disk
  drive on the 38G (LIB, SEND, second entry) and the 39G (APLET, SEND,
  third entry) first sends a Kermit I packet (`~* @-#Y3`: MAXL 94, TIME
  10, CR, `#` control quoting, 8-bit quoting on request, asking for the
  3-byte CRC) and then an R packet (GET) for `HP38DIR.CUR` /
  `HP39DIR.CUR`. Served as an empty file (S, F, Z, B, every packet ACKed
  by the calculator with the 1-byte check negotiated), the calculator
  then shows a "disk drive not prepared" error: it wants a directory file
  whose format is still unknown. Recorded in wiki
  questions/hp38g-39g-transfer-protocol; hptx needs a Kermit server mode
  that serves that file.
- **`Machine::step_for`**: `step` with a cap on the time a shut-down CPU
  skips, so tools (the boot example's `--io-trace`) can stop at their own
  next event without running past a SHUTDN wake.

## 2026-10-05 (iteration 7)

- **Benchmark oracle**: the HP Museum summation benchmark (wiki:
  sources/hpmuseum-summation-benchmark), run through `saturnus-mcp`'s
  Kermit host command and timed with TICKS. n = 1000 is the precise
  figure: n = 100 adds about 5% of start-up on the 48s, and on the 49G
  the first `Σ` adds about 2.2 s (the real n = 100 times show no such
  overhead, cause unknown). Before iteration 7, n = 1000: 48SX 75.4 s
  (real 95.5 s), 48GX 34.4 / 33.9 s sum / FOR (real 55 / 54 s), 49G ROM
  2.10 33.0 s sum (real 47.8 s).
- **Instruction profile** (cargo feature `profile` on `saturnus` and
  `saturnus-mcp`, off by default): counts and charged cycles per
  `Instruction` variant, opcode and DAT nibbles, taken branches and time
  per region. The 48SX and 48GX ROMs run nearly the same mix for the
  benchmark (13.1M vs 11.9M instructions, 137M vs 126M SASM cycles; the
  BCD digit loops of shifts, subtracts and GONC dominate). With one
  cycle table the GX could only be 2.2x the SX, but the real GX is 1.74x.
  So no single table fixes both, and the G series needs its own counts,
  as Emu48's change log says (wiki: emulators/emu48 SP1).
- **G-series cycle table**: the Yorke models (48GX, 49G, 38G, 39G, 40G)
  use the Meta Kernel counts the Saturn tutorial quotes for "the HP 48G,
  whose processor runs at about 4 MHz" (wiki: hardware/saturn-cpu
  "Timing"; `cycles::cycles_g`). These counts run about 0.5 cycle per
  opcode nibble above SASM, with longer jumps and DAT reads. The tutorial's
  rounding rule is implemented: `.5` rounds down at an even instruction
  address and up at an odd one, and the second count after a comma
  rounds by the parity of the address read. The 48SX keeps SASM. The
  table moves the 48GX to 41.2 / 40.5 s and the 49G to 40.2 / 41.8 s
  (sum / FOR, n = 1000). The GX's error relative to the SX falls from
  26% to 5%.
- **Display stall unchanged**: the flat 13% (Voyage) agrees with the
  thread's assembly measurement on a GX (post 165: 39.7 s on vs 34.8 s
  off, a 14% slowdown). Giesselink's 22-23 µs per 244 µs row gives about
  9% for every model, so nothing supports a clock-dependent stall.
- **Clock unchanged**: no source gives a measured clock. The HP Journal
  (June 1991) says the 48SX's CPU clock is multiplied from the 32 kHz crystal
  ("8-MHz CPU clock", the nominal 2 MHz cycle). The 1994 article gives
  the G series a "4-MHz bus rate", and Mastracci says "~4 MHz, varies
  with temperature". The 48 FAQ says the G/GX throughput is about 40%
  above the S/SX, not 2x, "due to various overheads (memory bank
  switching, etc.)". `clock_hz` stays 2 / 4 MHz.
- **Calibration factor** (`Model::cycle_scale_permille`): after the
  tables, every model is still 19-34% fast: 48SX 1.267, 48GX 1.336 /
  1.334 (sum / FOR), 49G 1.189 / 1.221. No documented cause explains
  this. A common factor that real instructions take longer than either
  table says fits the three models to about ±6%, but the 48GX and 49G
  share a chip and still differ by 11%, so the factor is per model:
  48SX 1.267, 48GX 1.335, 49G 1.205. The 38G is taken as a 48GX and the
  39G/40G as a 49G (inferred, no benchmark). The factor multiplies each
  instruction's table count before the stall, with the fraction carried
  (`stall_acc` now counts in 1/100000 cycle; old states still load).
  SHUTDN skips are not scaled, since the timers run on the crystal. After:
  48SX 95.50 s (n = 1000), 48GX 54.96 / 54.05 s (n = 1000) and 5.93 s
  (sum, n = 100), 49G 48.43 / 50.32 s (n = 1000) and 5.38 s (FOR,
  n = 100). The ROM clock-drift e2e still passes.
- **49G `run_command` "timeout"**: not a link fault. With exact integer
  literals the 49G's `0 1 100 FOR ... 3 INV ^` runs symbolically
  (`√EXP(√2)^(1/3)+...`) for minutes. That exceeded the client's 6 s x 4
  tries, and the next command then read the late reply. `1 2 +` answers
  in milliseconds on both 49G ROMs. The benchmark uses reals on the 49G
  (`0. 1. 100.`), and the MCP e2e now covers the 49G.
- **Busy calculators do not time out**: `link::MachineTransport::read`
  does not count a 1 ms step toward the reply timeout while nothing has
  come back and the CPU is not in SHUTDN. An idle server sits in SHUTDN
  at over 99% of samples on all four ROMs. The cap is 10 minutes of
  emulated time per read, plus the session's wall-clock limits. A
  host command that computes for minutes now returns its own reply. The
  n = 100 benchmark takes 70 ms of wall time instead of 6 s of
  retransmission.
- **39G/40G alpha letters are one row off in the wiki**: booting the 39G
  ROM, ALPHA then SIN types E, not A. The full map, by key function, is
  A-D on VARS MATH d/dx X,T,θ, E-I on SIN COS TAN ln log, J-N on x² x^y
  ( ) ÷, O-S on `,` 7 8 9 ×, T-W on 4 5 6 −, X-Z on 1 2 3, and space on
  plus; θ : ; are on 0 . (−). Each letter sits one key row above where
  the user's guide figure was read, so the letters are printed below
  their keys. The plain functions of the same matrix positions (SIN types
  `SIN(`) confirm the key map. A second ALPHA cancels alpha mode as on
  the 38G, so `type_text` types each letter as ALPHA then the key
  (lowercase SHIFT ALPHA then the key), and space as ALPHA then plus.
  E2e goldens `39g-hello-world` and `40g-hello-world` (the 40G menu has
  CAS). The web keyboard shows the letter on each 39G/40G key.

## 2026-10-05 (iteration 8)

- **Skin format: Rust data, served as JSON.** One file per model in
  `crates/saturnus-web/src/skins/` (48SX, 48GX, 38G, 49G, 39G; the 40G is
  the 39G drawing with its own name). The page gets it from `skin(model)`
  or `Emulator.skin()`. Rust rather than JSON files in `web/` because a
  native unit test then checks every skin against the model's key matrix
  (each matrix key drawn exactly once, nothing else), that keys do not
  overlap or cover the LCD, and that the drawn alpha letters agree with
  the ROM (39G/40G) and the wiki (48, 49G, 38G). A skin holds the case
  panels, the display window, the logo's place, free text and bracket
  lines, and per key its rectangle, outline (rounded key or cursor-pad
  trapezoid), cap colours, label, left and right shifted labels, alpha
  letter and text below. The plain button grid stays, behind a "Drawn
  calculator" checkbox (localStorage `saturnus.view`).
- **Measurement.** Keyboard line drawings rendered with `pdftoppm` at 300
  or 400 dpi; key outlines found as connected components of dark pixels
  and case and window edges from pixel profiles, all in a scratch
  directory, nothing stored. Unit: 1/100 of the figure's menu-key pitch,
  origin at the case's top-left corner. Figures: 48SX Owner's Manual vol.
  1 p. 26 (keys and labels of both 48s); 48G User's Guide p. 1-9 (the 48
  display and case top) and pp. 1-5, 2-3 (cross-check of the 48 grid);
  38G User's Guide inside cover; 49G User's Manual figure 1.1 (p. 1-2);
  39G/40G User's Guide p. 1-3. Colours and the 48GX's label colours from
  photographs (48SX manual cover, Wikimedia Commons 48GX, 49G and 38G
  photos, Thimet's 39G photo). The figures stop just below the 48's ON
  row, so the 48 case below it (200 units) is taken from the 48GX photo.
- **39G/40G label placement.** Shifted labels belong to the key below
  them and alpha letters to the key above; the skin draws the letters
  under each key's right corner, where the guide's figure and the
  photograph have them, with the letters the ROM types. Thimet's
  collection page says the same. The 40G's CAS appears only as a menu
  label on the display (drawn by the ROM), so no CAS is printed on the
  case.
- **Logo.** A flat planet with a tilted ring, our own drawing:
  terracotta disc, teal ring, the ring's back half behind the planet.
  `web/logo.svg` (also `web/favicon.svg`), on each skin where the HP logo
  was, in the page header and at the top of `README.md`. No red swoosh
  (Saturn cars) and no wordmark (Sega Saturn). The model name on the
  bezel is plain text without "HP" ("48SX", "49G").
- **LCD scale on the skin.** The skin is sized so the LCD gets the
  largest integer number of CSS pixels per LCD pixel, at least 2, that
  fits the page width and the window height. On a short window it stays
  at 2 and the page scrolls. Only when 2 is too wide does the skin fill
  the width at a fractional scale. The canvas renders at the device pixel
  ratio. In skin mode the LCD keeps its light colours in dark mode, like
  the case; only the page chrome follows the theme.

## 2026-10-05 (iteration 9)

- **Semantic MCP tools** (owner: eval and the typed stack first): `eval`,
  `stack`, `push`, `pop`, `drop`, `clear_stack`, `get_var`, `set_var`,
  `list_vars`, `cd`, all on the ROM's Kermit server through hptx-core.
  The keystroke tools stay. JSON results with the server state; a
  calculator error is a tool error with `{error, depth, display}`.
- **Server mode inside the tools**, superseding iteration 6's refusal:
  semantic tools press ON (clears a command line) and type `SERVER` when
  the server is not running, and send FINISH afterwards unless
  `keep_server`; `press_keys` and `type_text` stop a running server first
  and say so. The raw Kermit tools still need `start_server`. `status`
  reports `mode`. The 38G, 39G and 40G answer "no Kermit server on this
  model".
- **Exact values from binary GETs**: levels are copied with
  `n DUPN n →LIST` into a temporary variable (`SATRNTMP`, first free of
  four names after a `G D` check), fetched in binary, purged; the stack
  is untouched and the list keeps tags (`STO` would strip a top-level
  tag). The decoder (`saturnus-mcp/src/object.rs`, written to move into
  hptx-core, which needs no change today) decodes reals (12 digits,
  exponent, sign), 49G integers, complex, strings, names, binary
  integers, characters, lists, tagged, units (number), real and complex
  arrays; ROM pointers inside composites are read from the emulated
  memory (`Machine::peek`). Programs, algebraics, unit expressions and
  commands take their text from an ASCII GET walked in step with the
  decoded tree. Body layouts went into the wiki first
  (protocols/hp-object-format "Object bodies", from RPLMAN and objects
  observed in saturnus).
- **Reals in JSON**: a JSON number (12 digits survive an f64) unless the
  exponent is outside ±307, then calculator text (`"1.5E-400"`). 49G
  integers: a number up to 15 digits, else text. Binary integers carry
  the display base, read from the level's display text or from `#0`.
- **eval syntax**: the C text is parsed as RPN on the 48SX and 49G, so
  `SIN(0.5)` is `Invalid Syntax`; eval then drops the command-line string
  the calculator left and runs `'SIN(0.5)' EVAL`. A syntax error that is
  not an algebraic either leaves the stack unchanged. Source over one
  packet (77 encoded bytes) is sent as a string (binary PUT) and run with
  `STR→`, so both paths parse alike.
- **push**: RPL text in one host command when the object has text that
  fits; else a binary PUT (exact: strings holding `"`, local names,
  unknown objects by their hex) and `RCL`; else a string and `STR→`.
  Program and algebraic sources must be `« »` and quoted, so a push never
  runs commands. Binary files are exactly as long as the object: the 48SX
  stores a file with a trailing byte as a string. The header's ROM letter
  is not checked by the calculators.
- **eval time limit**: `timeout_ms` (default 60 s, 1 s to the link's
  10 min busy cap) is emulated time from the moment the calculator has
  received the whole command packet to the first reply packet that is
  not a NAK, so the link's turnaround, line time and the reply's transfer
  do not count; the ROM's own handling does (0.3-0.45 s for `1 2 +`,
  more with a deep stack, hence the 1 s floor). The link fails a read
  past it (`Core::set_read_cap`), the tool presses ON, which ends the evaluation
  and the server on both the 48SX and the 49G (wiki:
  protocols/server-commands), marks the server stopped, enters it again and drops level 1 if it is
  the evaluated text as a string (the 48SX ROM puts it back when ON hits
  during compilation; compared in HP characters, so `\GS` matches `Σ`),
  and returns an error that says whether it dropped it and names the
  49G's exact integer arithmetic. E2e on all three models. Other semantic
  commands: 60 s.
- **Turnaround in emulated time**: hptx's 200 ms wall-clock pause between
  transactions is off; the link runs 200 ms of emulated time before a
  packet that opens a transaction (S, R, I, G, C). The 48SX e2e went from
  43 s to 4 s (release); the summation benchmark is unchanged. A command
  that collides with the idle server's periodic NAK still costs
  kermit-proto's 1 s NAK grace in wall time.
- **Cost**: a Kermit transaction takes about 1.4 s of emulated time at
  the ROM's pace; an eval with the server kept is six transactions (6-9 s
  emulated, 0.07 s wall in release), entering and leaving add about 9 s
  and 5 s emulated (0.2 s wall in total). Agents batch with
  `keep_server`.
- **Review fixes (PR 10)**: the array decoder checks every count from
  the object (dimensions, row counts, element bytes) with checked
  arithmetic against the object's size before allocating; a crafted
  array is `unknown`, not a panic. The core mutex is taken through
  `link::lock`, which recovers a poisoned lock, so a bug that panics in
  one tool does not lock out the session. RPL text for push and set_var
  cannot break out of its quoting: names that fail hptx's
  `validate_name` and tags that are not plain tokens go in binary;
  units with whitespace or delimiters, commands that are not one token,
  and program or algebraic sources that are not exactly one balanced
  `« »` or `' '` group are refused. push and set_var also check that the
  depth grew by one or stayed.
- **Plan correction**: `0 0 /` gives `Undefined Result`; `Infinite
  Result` comes from `1 0 /`. The e2e tests both.

## 2026-10-05 (iteration 10)

- **Display window = the LCD's active area.** A skin's `lcd` rectangle is
  now the 131 x 72 canvas itself (64 rows plus the annunciator strip, square
  pixels), not a window the canvas is fitted into. Every ROM draws the six
  menu labels as 21-pixel boxes at a 22-pixel pitch from column 0 (wiki:
  hardware/display "Menu labels"), so the window is 595 units wide (5.95
  menu-key pitches) and placed so label i is centred over menu key i; the
  bezel widened to follow it. A unit test asserts the alignment within 3
  units (under one LCD pixel) on every model, and the page's measurement in
  the browser is within 0.6 CSS px. Before: the labels drifted by up to 50
  units on the outer keys (the owner's first review point).
- **Case ends below the keys.** The 48 case lost 148 units (1413 to 1265),
  the 38G 20, the 49G 27, the 39G/40G 37; a test keeps the margin under the
  lowest row between 0.8 and 1.3 times the side margin. The 49G and 39G
  bottom corner radii shrank with it.
- **Key relief from three shared gradients.** `defs` holds a light falling
  on each cap (white 0.26 to black 0.16), a rim lit above and shaded below
  (as a 2-unit stroke) and a light on the case; each key adds one overlay
  path over its coloured cap, so the SVG grows by one element per key. A
  pressed key moves 3 units down onto a lighter shadow and darkens 26%.
  No filters, since blur per key would cost paint time on every press.
- **Idle page costs nothing.** `Machine::idle_cycles` (and
  `Emulator::idle_ms`) report how long a shut-down CPU with no wake
  condition sleeps before its next timer or UART event. When the ROM is in
  SHUTDN with no key queued, the page cancels the animation loop and sets
  one `setTimeout` for that moment; on the timer or on a key it first runs
  the emulated time that passed in bulk (cheap while the CPU sleeps, stops
  early if the ROM wakes), so the ROM's clock keeps time, then animates.
  The 48 ROM's timer event every 0.5 s (TIMER1's MSB) wakes nothing, so
  the page sleeps on without a frame. The per-frame status counters are
  gone; the status line changes only on events. Measured in headless
  Chrome over 5 s idle on the 48SX: 0 animation frames and 2.2 ms of main
  thread task time (`Performance.getMetrics` TaskDuration, 0.04% of one
  core); before: 300 frames and a status line rewritten each frame. A key
  wakes it (loop back to frames) and it sleeps again once idle.
- **Speed control.** 1x, 2x, 4x multiply the wall time each frame runs;
  Max runs 10 ms slices until 11 ms of wall time or one emulated second
  per frame is spent, then yields. The sleep timer divides by the rate (60
  for Max). Stored in `localStorage` `saturnus.speed`; the panel says the
  calculator's clock runs fast above 1x. Measured: a `1 300000 START
  NEXT` loop on the 48SX runs at 1.0x, 4.0x and about 50x emulated time
  per wall time (Max is bound by the per-frame cap).
- **Layout.** The calculator fills the stage's height (or width on a
  narrow screen), shrinking by up to 8% so each LCD pixel is a whole number
  of device pixels (not snapped below 2); iteration 8's integer-scale rule
  with a scrolling page is superseded, since the owner asked for the whole
  window height. Controls live in a 252 px side panel on the left (hidden
  with `‹`, remembered); below 760 px a 48 px top bar with a drop-down
  sheet. Fullscreen uses the Fullscreen API on the stage alone; in
  Chromium the page locks Escape so ON keeps working (hold Escape to
  leave), elsewhere Escape leaves and `` ` `` is ON too. The page chrome
  is a warm grey desk with a paper-coloured panel and the logo's teal as
  the only accent; one typeface (Helvetica Neue / Arial, what the skins
  print with) and a monospace only for key caps and the status line.
- **Keyboard letters.** The skin JSON carries `letters` (letter to key,
  from the alpha letters drawn on the skin; the 39G/40G also map the
  space to plus) and `typing` (alpha key, the shift that makes lowercase,
  whether it goes before alpha, whether a second alpha locks). A typed
  letter is queued and expanded when its turn comes: alpha unless the
  alpha annunciator is on, the shift for lowercase, the key. Right after
  a letter the page pressed alpha for, alpha is known to be off (one-shot),
  so runs of letters need no annunciator reads; otherwise the page waits
  for the ROM to idle, because the 48SX ROM blinks the annunciator while
  it redraws the command line (up to 250 ms after a key). Shortcuts: Tab
  = alpha, `[` and `]` = the shifts (saturnng's TUI uses the brackets),
  Escape and `` ` `` = ON. CapsLock is not used: macOS reports no reliable
  key-up for it. A test checks all 26 letters per model map to distinct
  matrix keys.
- **Queued keys wait for SHUTDN.** The 48SX ROM drops a key pressed 30 ms
  after the previous one while it is still handling that one (70 to
  230 ms); iteration 6's fixed 30 ms gap lost letters. A queued press now
  starts once the ROM has gone idle, or after 300 ms while it stays busy
  (a running program), and a key the user holds still never blocks.
  Verified by typing "Hello World" on the 48SX, 48GX, 49G, 38G ("HelloWorld",
  it has no space) and 39G.
- **ROM directory.** `roms/` in the checkout was a symlink to itself when
  this iteration started; the browser checks used copies under the
  session's scratch directory and the 39G ROM fetched with `rom fetch
  --yes` there. Not fixed in the repo (gitignored, the owner's setup).
- **Fidelity pass from the owner's photographs.** The owner photographed
  his 48SX, 38G and 49G straight on (ten JPEGs in `~/Downloads/HP
  Taschenrechner/`, not in the repository); they replace the manual
  figures as the reference for those skins' geometry and colours, and
  the 48GX takes the 48SX's geometry (one mould). A key's rectangle is
  now its cap, and `Cap::well` is the margin of the dark well (48, 38G)
  or outline (49G) drawn around it. Where a photograph conflicts with a
  rule from the owner's earlier review, the rule wins and the difference
  is written in the skin file: the display window stays the ROM's 131 x
  72 pixels aligned to the menu keys (595 units wide; the real glass is
  509 to 543), and the case still ends within 1.3 side margins of the
  bottom row (the real cases have 82 to 85 units there, the skins 61 to
  85). The photographs' colours are toned down from their sunlit, red-lit
  exposure by eye. No HP mark is drawn, including the 38G's series
  emblem; "SCIENTIFIC EXPANDABLE" on the 48SX is a description and is
  drawn.
- **38G space is SHIFT 2.** SPACE is printed above the 2 key; ROM A1.67
  types a space with SHIFT then 2, also between letters (iteration 10's
  "the 38G has no space" was wrong). `type_text` and the page's space key
  use it (`Typing::space`); wiki: hardware/hp38g. The photographs agree
  with the ROM-verified 38G letter map on all 26 letters.

## 2026-10-05 (iteration 16)

- **Supply-chain policy** (`deny.toml`, checked by `cargo deny check` in
  `just lint` and in CI's `quality-gates` job): dependencies come from
  crates.io; a git dependency needs an explicit `[sources] allow-git` entry, and the
  only one is `https://github.com/ractive/hptx` (hptx-core, pinned by
  rev in saturnus-mcp). Unknown registries and git sources are denied.
  Advisories block the merge, with no ignores: upgrade the dependency;
  an advisory that cannot be fixed is the owner's call, recorded in
  `deny.toml` with the reason.
- **Licences**: the allow-list is exactly what the tree needs (MIT,
  Apache-2.0 for rmcp, Unicode-3.0 for unicode-ident), narrower than
  hyalo's. One crate-scoped exception: serialport (MPL-2.0), a
  non-optional dependency of hptx-core that saturnus never uses to open a
  port; it goes away if hptx-core makes serialport optional.
  Duplicate versions warn and do not fail.
- **Lockfile**: `Cargo.lock` is committed and CI builds with `--locked`,
  so CI tests the tree the lockfile describes.
- **CI** (`kb/docs/ci.md`): hyalo's structure, separate jobs `fmt`,
  `clippy`, `test` (ubuntu, macOS, Windows), `wasm` (wasm32 check of the
  core, `web/build.sh`, the web bindings' tests), `lint-kb` (diff-aware on
  PRs) and `lint-kb-full` (pushes to main), `quality-gates` (cargo-deny).
  `just gates` is the same sequence locally. No ROMs in CI: the e2e tests
  skip without `SATURNUS_ROM_DIR`.
- **Pinning**: third-party actions by commit SHA with the version in a
  comment, updated by Dependabot; first-party `ractive/release-workflows`
  (exact tag) and `ractive/setup-hyalo@v1` (major tag) by tag, ignored by
  Dependabot. hptx-core's rev is moved by hand.
- **Release** (`kb/docs/releasing.md`): `release.yml` calls
  `ractive/release-workflows@v0.2.1` for the `saturnus` CLI; no crates.io
  (the hptx git dependency rules it out), winget, AUR, Cloudsmith or
  deb/rpm. The shared workflow ships one binary, so saturnus-mcp is not
  in the archives, and it always runs the Homebrew and Scoop jobs on a
  real release (they need their token secrets).
- **Core publishable to crates.io**: first requested from the hptx side
  for hptx-cli, withdrawn the same day (hptx-cli will not be published);
  kept as an option, no publication planned. It must stay free of git
  and path-only dependencies; `cargo publish --dry-run -p saturnus` in CI
  and `just gates` enforces it. The package excludes `tests/` (golden
  files are ROM screen dumps). Other crates stay unpublished.
- **hptx-saturnus**: hptx moves its saturnus transport into an adapter
  crate `hptx-saturnus` in the hptx repository, driving the core's public
  `Machine` API (`serial_push`, `serial_drain`, `serial_pending`, a
  framebuffer accessor). `saturnus-mcp` keeps its own `MachineTransport`
  and must not depend on `hptx-saturnus` (it would bring a second copy of
  the core). Do not bump `saturnus-mcp`'s `hptx-core` pin until hptx's API
  PR has merged; expect to add `_ =>` arms on its enums then.
- **Core API relied on by hptx** (semver-relevant once `saturnus` is
  published): `Machine::new` (the ROM constructor) and
  `Machine::{serial_push, serial_drain, serial_pending, run_cycles,
  cycles, lcd, framebuffer, key_down, key_up, model, is_shutdown}`,
  `saturnus::io::Key`, `saturnus::Model`. `lcd()` (pixels only: 64 rows of
  131, top row first, leftmost column at index 0, `true` = dark) and
  `framebuffer()` (pixels, annunciators and contrast) both stay; they are
  not aliases.

## 2026-10-05 (iteration 12a)

- **System RAM locations** (wiki: hardware/hp48-system-ram): HOME,
  end of HOME, current directory, saved D1, stack end and the flag words
  per model. 48SX from the 1991 internals address list (ROM E), all
  confirmed on ROM J; 48GX and 49G found in saturnus by scanning system
  RAM for pointers to directory objects, diffing RAM across `SF`/`CF` and
  tracing the ROM's D1 restore after a key (#067F1). The 49G has two
  64-flag words of each kind; `RCLF` order is system 1, user 1, system 2,
  user 2.
- **`saturnus-objects` crate**: the typed object model and the decoder
  moved out of `saturnus-mcp` with their tests, plus its own prolog table,
  size walk and HP-charset decode, because `hptx-core` pulls `serialport`
  and does not build for wasm32. Dependencies: the core (for
  `impl Memory for Machine` and the per-model layout), serde, anyhow.
  What rides on Kermit stays in `saturnus-mcp::object` (binary file
  header, ASCII sources, encoder, RPL source text), which re-exports the
  model, so no caller changed.
- **Read API** (`saturnus_objects::ram`): `UserMemory` over any `Memory`
  and a `Layout`, with `tree()` (HOME's variables newest first, nested
  directories, name, `G D` type, size, checksum, address), `current_path()`
  (the directory whose object is at the context pointer, found in the
  tree), `stack()` (typed, binary integers in the base of flags -11/-12),
  `flags()` and `change_counter()`; free functions `memory_tree`,
  `current_path`, `stack_objects`, `flags`, `change_counter` take a
  `&Machine`. Nothing is written. HOME's records must end exactly at the
  end-of-HOME pointer and the stack must end in its 0 marker, otherwise
  the read is an error rather than a guess.
- **Size and checksum**: size is the whole variable record (object + 2n
  + 9 nibbles) over 2, checksum the hardware CRC of the object's nibbles,
  matching `G D` and `BYTES` (fixture from a 48SX in the unit tests).
- **Change counter**: FNV-1a over HOME's nibbles, the five system
  pointers, the stack entries and the flag words, instead of a write hook
  in the core: no core change, and HOME is the user's memory, so the
  cost is that of reading it (a few thousand nibbles for small trees, at
  most the 49G's 512K).
- **Valid at idle**: the saved D1 is the ROM's stack only while it sits in
  its outer loop; inside the Kermit server it is the server's own stack.
  Tree, path and flags read correctly in both. So the MCP gets
  `memory_tree` (path, tree, counter) and `flags`, which work without
  server mode and do not run the calculator; the RAM stack is API only.
- **Web**: `memory_tree()`, `stack()`, `flags()`, `object_at(address)`,
  `memory_changes()` on the wasm `Emulator`, JSON through serde_json (new
  dependency of `saturnus-web`); counter and flag words as hex strings
  because a JavaScript number loses bits past 2^53.
- **Test** (`ram_reads_match_kermit`, MCP e2e, all three models): build a
  tree, a current directory, a stack and flags over Kermit, read the
  oracle in server mode (typed stack, `RCLF`, `G D` in every directory
  through `cd`), stop the server, compare the RAM reads; then a `STO`
  must move the counter and 500 ms of idling must not. The 49G is first
  put in RPN mode and its server re-entered from RPN: a server entered
  from algebraic mode (the 49G's default) leaves its stack packed in a
  list after FINISH.
- **Observed, not fixed**: with system flag -40 (clock display) set,
  `start_server` on the 48SX gets no NAK within 15 s; the test sets -2
  instead. Cause not investigated (likely the idle wait in the scripted
  `SERVER` start while the clock redraws every second).

## 2026-10-05 (iteration 15)

- **The 42S is modelled.** Feasibility verdict after the research: the
  Lewis can be modelled from Garnier's 42S article, Hosoda's hardware
  notes, the Emu42 documentation and the ROM's own behaviour; the open
  points (wiki: questions/lewis-*) do not block boot, keys or the
  self-test. Facts and sources are in wiki: hardware/lewis and
  hardware/hp42s.
- **A fixed Lewis map behind `HardwareProfile::lewis`**, not the Clarke/
  Yorke daisy chain: ROM #00000-#1FFFF, the 1024-nibble display and
  register block at #40000, RAM from #50000. CONFIG, UNCNFG and C=ID still
  run on the (unused) controller; the 42S ROM never executes them. The ROM
  decodes only its own length, #20000 (the optional second ROM) and other
  unclaimed addresses read 0. **RAM 8 KB mirrored through #5FFFF**: with
  open bus above #53FFF the ROM's cold start produces a broken memory
  (`2 ENTER 3 +` says "Invalid Type"); with the mirror it sizes 8 KB and
  works (inferred: it detects the alias). The 32 KB upgrade is not offered.
- **The Lewis block reuses the 48 timers and CRC.** `io::lewis::LewisIo`
  holds the display RAM and plain register storage; the registers at the
  addresses #4030E/#4030F and #403F7/#403F8-#403FF map onto `Timers` as the 48's T1
  control, T2 control, TIMER1 and TIMER2 (Garnier gives the 8192 Hz
  countdown at #403F8; the control roles are inferred from how the ROM sets INT/WAKE and
  tests the run bit), #40304-#40307 onto the CRC accumulator, so the
  machine's interrupt and SHUTDN logic is shared. LPD #40308 reads 0
  (batteries good). DON is #40303 bit 3, the contrast #40301 plus #40303
  bit 1 (the ROM's reset value 22 is KML 2.0's documented reset contrast).
  RATE (#40300) is storage.
- **Display geometry:** `Lcd` keeps its 131-pixel rows and carries 16 of
  them on the 42S (`Lcd::render_lewis`, `LCD_HEIGHT_42S`); the 131x64
  models are untouched. Hosts read the height from the `Lcd`
  (`Lcd::height`, web `lcd_height()`); PNG and text dumps follow it.
  `Annunciators` gains `updown`, `battery`, `g` and `rad`; the 42S's
  shift, print and busy words report as `left_shift`, `transmitting` and
  `busy`, so the 48 models' annunciator lines are unchanged. #40210 lights
  all seven (Garnier). Nothing is lit while DON is clear (inferred).
- **Keys:** the shared `Key` set gains `sigmaplus`, `xeq`, `rcl`, `rdn`,
  `swap` and `rs`; `exit` names `on`. The 42S's top row keeps its labels
  (`sigmaplus` `inv` `sqrt` `log` `ln` `xeq`) and is its menu row, so the
  42S has no `a`-`f`. Matrix from the KML 2.0 OutIn table.
- **Timing: 1 MHz, the SASM cycle table, factor 1.000, no display stall.**
  No benchmark or oracle exists for a real 42S; the stall is a 48
  measurement and the Lewis has its display RAM on chip (inferred). The
  self-test's SPD step shows 08847-08974 at this clock; a photo of a real
  unit's SPD value would calibrate it (wiki: questions/lewis-clock-and-rate).
- **Self-test result: the owner's revision C image fails the ROM's CRC
  step** (computes #1BE8, wants #FFFF; DRAM and URAM pass; summary FAIL).
  Checked independently in Python; no CRC variant gives #FFFF. Either the
  1999 dump has bad bits or the Lewis CRC differs. Recorded as an e2e
  golden ("ROM 01BE8") so a fix to either side shows; settle with
  `LEWISCRC` or a fresh dump (wiki: questions/hp42s-rom-crc).
- **No `rom fetch` for the 42S**: HP never released the ROM; the CLI
  refuses with a pointer to `--rom`. `--autostart` and the semantic MCP
  tools refuse ("no Kermit server on this model"); letters cannot be typed
  with `type_text` (the 42S types them from ALPHA menus).
- **State format**: version stays 2; a 42S state appends the 1024-nibble
  Lewis block after the machine section. Model code 6.
- **Core API relied on by hptx (extends iteration 16's entry)**: `Model`,
  `Key` and `Annunciators` grow whenever a model is added. This iteration:
  `Model::Hp42s`; the keys `SigmaPlus`, `Xeq`, `Rcl`, `RollDown`, `Swap`,
  `Rs` (`Model::ALL` 7 entries, `Key::ALL` 80); the annunciator fields
  `updown`, `battery`, `g`, `rad` (`Annunciators::list()` now has 10
  entries). `lcd()` keeps rows of 131, top row first, and has 16 rows on
  the 42S. No `#[non_exhaustive]`: our own hosts keep exhaustive matches
  so a new model shows every place to touch; downstream code (hptx) uses
  wildcard arms and reads sizes from the values (`lcd().pixels.len()`,
  `list().len()`), never from constants. New: `Machine::display_on()`
  (the one model-aware answer), `Machine::has_serial()`/
  `Model::has_serial()` (false on the 42S, where `serial_push` is a
  documented no-op and `serial_pending` stays 0; the CLI's `--serial`,
  MCP `boot` with autostart and `start_server` refuse the 42S up front).
  The key name `exit` resolves to ON on every model: `Key::from_name` has
  no model, by design; a model without the key refuses it as any other.
  The web `annunciators()` JSON keeps all ten keys on every model (stable
  shape; the 42S-only ones are false elsewhere).
- **Research access**: the HP Museum article answered 403 to plain fetches;
  the Wayback Machine copy was used. The Emu42 source and the LEWISCRC
  source were not opened; its manual, PROBLEMS.TXT and changelog were read
  for facts only.

## 2026-10-05 (iteration 11)

- **One protocol, two hosts.** The page talks to the emulator only
  through a backend speaking `web/protocol.md` (version 1, JSON
  messages): commands `hello`, `skin`, `layout`, `boot`, `keyDown`,
  `keyUp`, `keyUpAll`, `typeLetter`, `typeKeys`, `releaseAll`,
  `setSpeed`, `pause`, `reset`, `saveState`, `loadState`, `visibility`,
  `stats`; events `frame`, `keys`, `status`, `error` (`memoryChanged` and
  the read commands `memoryTree`, `stack`, `objectAt` reserved). A
  command with an `id` gets exactly one reply. `WorkerBackend` posts to a
  Web Worker; `TauriBackend` sends the same messages through one Tauri
  command, `command(msg)`, and hears them on one event, `saturnus`
  (rather than one Tauri command per protocol command: the Rust side
  dispatches the same messages the Worker does, so the two cannot
  drift). The page picks Tauri when `window.__TAURI__` exists.
- **Frames are pushed, packed and change-detected**: `frame` carries
  `width`, `height` (16 on the 42S), base64 of the pixels packed one bit
  each (rows on byte boundaries, leftmost pixel in the MSB; 1088 bytes
  for 131 x 64), the annunciators and the contrast, and is sent only when
  one of them changed, at most about 60 per second. The page draws it on
  its next animation frame.
- **The key queue moved to Rust** (`crates/saturnus-web/src/host.rs`,
  `KeyQueue`), a line-by-line port of the page's iteration 10 queue
  (60 ms hold, 30 ms gap, 300 ms busy gap, letters through alpha and the
  shifts, the "alpha spent" shortcut, the 400 ms letter settle). Both
  hosts use it, the Worker as wasm, the Tauri thread natively, so key
  timing is identical; it is timed in emulated time, so it lives next to
  the machine, not in the page. The Tauri crate depends on
  `saturnus-web` for it and for the skins (the "web" crate is the front
  end's Rust side, compiled for both targets).
- **Worker pacing** keeps the page's iteration 10 rules and the wake
  fix exactly: a ~60 Hz pass timer replaces `requestAnimationFrame`
  (paused while the page is hidden, as animation frames were; the page
  sends `visibility`); the wake timer, the catch-up of all elapsed wall
  time capped at 12 h, the 22 ms (visible) / 200 ms (hidden) wake budget
  and the owed remainder are unchanged. Measured in headless Chrome: 30 s
  idle 48SX, 0 passes, 60 wakes, 0.3 ms of Worker time, emulated time
  equal to wall time to 0.1 ms; the Worker paused 5 s in the debugger
  (a late wake) still accounted 6505.7 ms over 6505.7 ms.
- **Tauri pacing**: the machine thread uses the CLI's `Pacer` (moved to
  `saturnus-drive::pacer`, with a speed factor, `rebase` and
  `instant_of`) while the CPU computes, catching up in 1 ms slices within
  a 4 ms pass budget; while it sleeps the thread blocks on its command
  channel until the next timer event, then runs the elapsed time as the
  Worker does. A sleep starts at the instant the pacer had the machine
  at, so busy/idle transitions lose no time.
- **App Nap**: macOS throttled the app to about 32% of real time while
  its window was not in front. The app opts out at start
  (`NSProcessInfo.beginActivityWithOptions`,
  `UserInitiatedAllowingIdleSystemSleep`, through `objc2-foundation`,
  already in Tauri's tree; no `unsafe`); with it 100.00% over 30 s.
- **Workspace and CI for Tauri**: `saturnus-tauri` is a workspace member
  but not a default member; CI's `clippy` and `test` pass `--exclude
  saturnus-tauri`, a `tauri` job installs webkit2gtk-4.1 and checks it on
  ubuntu (kb/docs/ci.md). Installers come from `desktop.yml` (manual,
  Tauri's action, macOS/Windows/Linux, artifacts; release upload only
  with a tag, in a separate `contents: write` job); the page goes to
  GitHub Pages through `pages.yml` (manual). Both wait for the owner's
  settings (kb/docs/releasing.md). The crate's `rust-version` is 1.88.
- **deny.toml additions, pending the owner's decision**: crate-scoped
  licence exceptions only (nothing added to the allow-list): MPL-2.0 for
  cssparser, cssparser-macros, dtoa-short, selectors (dom_query in
  tauri-utils and wry) and option-ext (dirs-sys); BSD-3-Clause for
  brotli, alloc-no-stdlib, alloc-stdlib; Zlib for foldhash;
  "Apache-2.0 WITH LLVM-exception" for target-lexicon (system-deps on
  Linux). No GPL, LGPL or AGPL in the tree. Advisories: the two
  quick-xml vulnerabilities and the time one were fixed by upgrading
  (`plist` 1.10.1, `time` 0.3.47); six "unmaintained" advisories with no
  maintained release are ignored by ID with a reason and a re-check date
  (proc-macro-error via gtk 0.18; unic-char-range, unic-common,
  unic-char-property, unic-ucd-version, unic-ucd-ident via urlpattern in
  tauri-utils). This bends iteration 16's "no ignores" rule and is the
  owner's call.
- **Web Components in the light DOM**: `<sat-calculator>`,
  `<sat-controls>`, `<sat-about>` render into their own children with
  `display: contents`, so the iteration 10 stylesheet and layout apply
  unchanged; a shared `Store` (`EventTarget`) is fed by the backend's
  events; components act only through the backend.
- **About panel**: `web/about.json` is generated by
  `scripts/about-json.py` from the wiki's 55 source pages through
  `hyalo` (title, authors, year, URL or archive location, the wiki pages
  citing each), plus our own text for the statement, the saturnng oracle,
  the skin references, hptx and the ROM policy. The wiki stays outside
  the repository; the JSON is committed.
- **Native dialogs from Rust**: the Tauri side opens the ROM and state
  dialogs (`tauri-plugin-dialog`, blocking, off the main thread) when a
  `boot`, `saveState` or `loadState` arrives without a path, so the page
  needs no Tauri JavaScript package and no dialog permission. States are
  plain files of `Machine::save_state`.
- **Review fixes (PR 17)**: the Tauri page can no longer name files:
  messages with `romPath` or `path` are refused, the host chooses the
  file in a native dialog and passes it beside the message, and reads are
  capped at 4 MiB (the largest ROM any model takes; the largest state,
  the 49G's, is 2.6 MB), cut off at the cap for devices like `/dev/zero`.
  The page's capability is `core:event:allow-listen` and `allow-unlisten`
  only. Async Tauri tasks may start out of order, so `TauriBackend`
  numbers its messages (`session`, `seq`) and a sequencer in the app
  delivers them in that order (a dialog holds back later commands until
  it is answered). `web/about.json` carries only public URLs and an
  `archived` flag; `scripts/about-json.py --check` (CI, `just lint`)
  fails on local paths and e-mail addresses. `desktop.yml` builds the
  release tag it attaches to and never overwrites release assets.

## 2026-10-05 (control API replaces MCP)

- **saturnus-mcp is retired; no MCP server** (owner, confirmed in this
  repository's session after the hptx session relayed it). saturnus is an
  emulator that serves a serial port and a control API: `saturnus run`
  in the foreground serves the serial port on TCP and an HTTP/1.1 + JSON
  API on 127.0.0.1 (token file, Host and Origin checks), `saturnus ctl`
  is its client, and calculator operations against a running saturnus go
  through the hptx CLI over the serial port as against hardware. Plans:
  `iteration-17-control-api`, `iteration-18-retire-mcp`.
- **Supersedes** two bullets of the iteration 16 entry: the
  `hptx-saturnus` adapter crate and the freeze of `saturnus-mcp`'s
  `hptx-core` pin. Once `saturnus-mcp` is gone nothing in saturnus
  depends on hptx: no git source in `deny.toml`, no MPL-2.0 exception for
  `serialport`. Until then the pin stays where it is (no bump). The list
  of core API that hptx relies on (iterations 16 and 15) stays valid:
  hptx's in-process transport keeps using `Machine` and will use
  `saturnus-drive`'s autostart.
- **One protocol, three adapters**: the HTTP API reuses the command and
  event shapes of `web/protocol.md` (Worker, Tauri, HTTP); additions go
  into that document.
- **Order of retirement**: the crate is deleted only after its ROM-gated
  Kermit tests have moved to tests that use `kermit-proto` from crates.io
  as a dev-dependency. Iteration 18 is blocked until that crate is
  published.
- **Web explorer and editor writes** (iterations 12 and 14): no
  `hptx-core` in the page. The control API cannot serve the browser page
  (the page runs the core as wasm and has no server), so the hidden
  Kermit path is built on `kermit-proto` plus `saturnus-objects`, with
  keystrokes as the fallback. The owner asked to be told if this changes
  iterations 12 to 14 materially: it changes their dependency, not their
  design.
- **crates.io**: the hptx side withdrew its request to publish the core
  (2026-10-05); the publish dry run stays as an option.
- **Second review round (PR 17)**: a page reload retires its session
  in the app's command sequencer; a late message from a retired session
  (a dialog left open across the reload) is refused instead of resetting
  the order, and commands the old page left parked are dropped with their
  reply channels, so no caller waits forever. `hello` makes the host send
  its status, keys and frame again, because the Tauri app's machine
  outlives a reload of its page (a browser reload starts a new Worker).
  State files are written to a temporary file beside the target, synced
  and renamed over it, so a failed write keeps the previous state.

## 2026-10-05 (iteration 17: control API)

- **`saturnus run` serves or runs a batch.** With `--screen`,
  `--annunciators` or `--save` and no `--serial` it stays the batch tool
  the differential script and the docs use; anything else serves in the
  foreground until SIGINT/SIGTERM: the serial bridge (default
  `tcp:127.0.0.1:4841` on models with a port; `--no-serial`) and the
  control API (`--no-control`). No daemon, no instance files. A CPU halt
  no longer ends a serving run (stderr and `info` report it).
- **Default ports: 4840 (control API) and 4841 (serial).** Outside
  4848-4852 (hptx's containers) and 4860-4889 (bridges and tests on the
  owner's machine). `--control PORT` / `SATURNUS_CONTROL` and `--serial
  tcp:PORT` select others for a second instance. A busy port is refused
  naming the listener: `lsof`, then `ss` (Linux), `netstat` + `tasklist`
  (Windows); where none answers the message says no pid is available.
- **Token file per user**: `$XDG_CONFIG_HOME/saturnus/control-token` or
  `~/.config/saturnus/control-token` on Linux and macOS (0600 file, 0700
  directory, exposed files refused), `%LOCALAPPDATA%\saturnus\
  control-token` on Windows (the profile's default ACL; no ACL code).
  `--token-file` / `SATURNUS_TOKEN_FILE` override it (the tests use
  temporary directories). macOS uses `~/.config` rather than
  `~/Library/Application Support` so the path has no space and one rule
  covers both Unixes. Rules and tests: [[docs/control-api-security]].
- **The shared runner lives in `saturnus-drive`** (`runner.rs`, moved from
  `saturnus-tauri` with its history), which now depends on
  `saturnus-web` (the protocol's host pieces, `Emulator` and `KeyQueue`;
  its wasm-bindgen parts are inert natively) and `serde_json`. The Tauri
  crate re-exports it as `saturnus_tauri::runner`; the CLI must not
  depend on Tauri. Host additions: a `Hook` the loop calls between passes
  (the CLI's serial bridge and Ctrl-C), `info` fields set by the host,
  `start` with a machine the host built. The protocol's existing shapes
  are unchanged; new native-host commands (`screen`, `info`, `model`,
  `keyScript`, `typeText`, `peek`, `poke`, `memoryTree`, `stack`, `flags`,
  `objectAt`) and `saveState`/`loadState` without a file (state as base64
  in JSON, as the Worker's bytes) are added to `web/protocol.md`.
- **Key scripts and typed text run synchronously in emulated time** on
  the machine thread (`Session`, as the CLI's batch scripts), bounded to
  10 minutes emulated and 30 s wall time, and reply when idle; the clock
  then follows the wall clock again. Deterministic like the goldens, and
  `ctl keys` returns when the screen is final. Cost: the serial bridge
  waits meanwhile (documented: no keys during a Kermit transfer).
- **HTTP written by hand, no server crate**: a small HTTP/1.1 subset
  over `std::net` (one request per connection, `Content-Length` only),
  for both the server and `ctl` (`control/http.rs`, about 300 lines with
  tests). A tiny_http-class crate was considered and not taken: the
  subset is small, and owning it keeps the refusal order (Host, Origin,
  token, route, method, size, all before a body byte is read), the
  per-phase deadlines and the connection cap in plain sight, with no
  dependency to audit. The only new dependency is **getrandom 0.4** (MIT OR
  Apache-2.0, already in the tree through Tauri; deps cfg-if, libc,
  r-efi on UEFI only) for the token. `cargo deny check` passes with no
  new ignore and no licence change.
- **Endpoints `/v1/<name>`**: `screen`, `keys`, `type`, `mem`,
  `snapshot`, `info`, `cycles`, `model`, `stack`, `tree`, `flags`; command
  errors are 422, malformed requests 400 (full table in
  `web/protocol.md`).
- **Review fixes (PR 18).** `run` serves only with `--serve`, `--serial`
  or `--control`; every other invocation finishes as before iteration 17
  (the first version served whenever no output flag was given, which
  broke `--cycles`/`--card-writeback` batch runs and left a 42S no way to
  serve and still write `--save`). `--serve` gives the serial bridge
  (127.0.0.1:4841) and the API (4840); `--serial` alone stays the bridge
  alone; output flags are written when a serving run stops. The serial
  bridge binds loopback unless `--serial-remote`, and refuses a new
  client whose first bytes start an HTTP request line (browser pages
  sending no-cors requests), forwarding nothing; `CONNECT` is left out
  (browsers cannot send it, and `C` starts XMODEM-CRC). A 504 now means
  the command did not run and will not: requests carry a `Ticket`
  (`saturnus_drive::runner`) that either the machine thread takes or the
  server withdraws (timeout, or the client closed), and a running key
  script is aborted through the session's abort flag; the queue to the
  machine is a `sync_channel` of 8 (503 when full). Connections still
  sending their head have 2 s and a budget of 16 (the oldest is dropped),
  separate from the 8 authenticated request slots. `keyDown`, `keyUp` and
  `typeKeys` now refuse unknown keys and a missing machine on all three
  hosts (the Worker too, via a new `has_key` binding); the page shows the
  `error` event in its status line.

## 2026-10-05 (iteration 13a)

- **Command list from the ROM's decompiler.** The names come from the
  ROM, not from a manual: every library number 0-7FF is probed with
  binary lists of XLIB names (library and command number, three nibbles
  each, 16 per library) sent over Kermit and fetched back as ASCII, so
  the calculator's decompiler prints each name from the library's name
  table, or `XLIB l n` when there is none. Libraries with names are then
  read in chunks of 512 until a chunk has none. Result: the 48SX has
  libraries 2 and 700 (397 names), the 48GX adds AB (517), the 49G 830
  (CAS libraries 221, 222, 788 and others, development 256, MASD 257).
  Names in two places (`+`, `∂`, `∫` have two numbers) are one entry
  with both `xlib` pairs.
- **Categories from the ROM's menus, crawled on the keyboard.** The 48SX
  has no command catalog; the categories are the built-in menus. Each
  `MENU` number 3-255 is shown page by page (`n.pp MENU`) and its labels
  read off the LCD with a font built from the ROM's own label drawing
  (`TMENU` of the characters 33-255). Plain keys are pressed in program
  entry mode, where a menu key types its command's name; a page whose
  batch navigated or did not compile is pressed key by key, and a lone
  structure word (IF, FOR) again with left-shift, which types the whole
  structure. Tabbed keys open submenus; `RCLMENU`, run from a program
  pushed beforehand, names the target. Toggles that act even in program
  entry mode (CLK, IR/W, XYZ, BAUD) are pressed until their labels are
  back. Each menu starts from a saved machine state. Menus are named by
  key path from the keyboard (the skin's key labels, matched against the
  first pages), breadth first; a command's category is the first menu
  that offers it, then `Keyboard` for keys that type it, then our own
  category in `reference.json` (43 commands on the 48SX, such as `DROP`,
  `LIST→`, the `*FIT` words). Numbers `MENU` does not know show the VAR
  menu and are dropped.
- **Clock workaround.** On the 48SX the TIME menus show the clock, and
  with the clock shown the emulated ROM stops taking keys after a few
  presses (reported to the lead as an emulator bug, not fixed here). The
  crawler therefore types only in the VAR menu, presses single keys on
  the menu under test and checks that each changed the screen below the
  status area. Every batch of runs starts from the saved state, and a
  batch that fails is redone one run at a time.
- **Deterministic Kermit link for generators.** The MCP link runs the
  machine for the client's wall time between packets (realistic for an
  agent), which made two crawls of the same ROM differ under load (one
  48GX menu, the 48SX TIME menus, through the clock bug).
  `Emulator::set_deterministic_link` turns that catch-up off (only the
  200 ms turnaround before a transaction runs); `saturnus-refgen` uses it,
  so its output depends only on the ROM and the inputs. Menus that still
  fail keep their labels and take their commands from labels that spell
  a command name (six unit menus on the 48GX and 49G).
- **Examples run from one booted state.** Each example (setup, input,
  run) starts from the same saved state with the Kermit server started
  fresh; results are the typed objects of `eval` (unknown objects
  without their nibbles), the display text and the error. On the 49G the
  state is RPN, CAS silent mode (flag -120, so the CAS switches modes
  instead of asking) and an empty stack. The stack effect is our
  notation, checked against the runs' arities and the manuals' stack
  diagrams.
- **Descriptions are ours.** Written from scratch by agents briefed
  never to copy or paraphrase; `scripts/check-similarity.py` rejects any
  run of six or more words shared with the manuals' text (48G AUR, 48G
  user's guide, 48SX owner's manual, OCR of the 49G Advanced User's
  Guide, the 49G user's manual's text layer decoded from its shifted
  font); the final set of 840 has none.
- **Manual deep links** point at the public copies on
  literature.hpcalc.org (page numbers differ between scans, so the local
  copies were not used for the 48 manuals). The AUR is indexed by its
  entry headings and running heads, the user's guides by their operation
  indexes, page labels mapped through the pages' own footers, and a page
  is kept only when it names the command. The 49G user's manual is not
  indexed: it has no operation index and its text layer uses a shifted
  font.
- **Tests.** The data regenerates byte for byte
  (`cargo test --release -p saturnus-refgen -- --ignored`, minutes); the
  default ROM-gated tests read the 48SX name tables and a sample of
  examples.
- **Lookup in the CLI, not MCP.** `saturnus ref <command>` embeds the
  files deflated by the CLI's build script (miniz_oxide, already in the
  dependency tree): 2.48 MB of JSON grow the release binary by 0.39 MB
  (2.69 to 3.08 MB) instead of 2.7 MB uncompressed; inflating and parsing
  takes a few milliseconds. It prints an entry as text or JSON.
- **Lookup rules (PR 22 review).** An exact name wins; then names in
  another case, the calculator's ASCII translation codes (the trigraphs
  of wiki protocols/hp-object-format, unambiguous since no name holds a
  backslash; a test derives every HP character in the catalogs and checks
  each name round-trips) and friendly spellings that are no other name
  (`∫` has none: `INT` is the 49G's INT). Several matches are listed, not
  picked. `--model` selects that model's category, examples, manual pages
  and whether a run confirmed the stack effect.
- **Crawl fixes (PR 22 review).** States live in a fresh owner-only
  temporary directory, never a predictable path in the shared one.
  Showing the PLOT, STAT, TVM and PRINT setup menus creates variables
  (PPAR, ΣPAR, N, PRTPAR), which changed the VAR menu the crawler returns
  to and failed those menus; the VAR menu is now also recognised by two
  equal presses that do not show the menu under test. Labels of menus
  that still fail and spell no command are kept as `unresolved`. Examples
  that fail in setup or input carry an effect like the others. An MCP `help` tool was built first
  and removed when MCP was retired; `saturnus-refgen` still drives the
  calculator through `saturnus-mcp`'s emulator and Kermit link (and its
  `set_deterministic_link`) until iteration 18 moves it.

## 2026-10-05 (fix: LCD noise during RAM remaps)

- **Cause**: the owner saw the whole 48SX screen flash to noise while
  busy. Not the transport: the core itself rendered it. ROM J resizes the
  built-in RAM at #0C0B2-#0C0FA (UNCNFG, then CONFIG with the size
  masks #F0000, #FC000 or #FE000); during those gaps of mostly 200 cycles (at most
  466) the bitmaps at #7097C and #70858 decode to the ROM, and
  `Machine::lcd` renders all 64 rows from it at once. A frame the runner
  happened to flush inside a gap was ROM code on every row (trace:
  pc #0C0B2, DON set, NCE2 unconfigured, line count 55). The 48GX's ROM R
  does the same at #72386-#72D6B (at most 1021 cycles); the 49G showed
  one 136-cycle gap during boot. Not tied to timers or interrupts (the
  SX gaps run with interrupts disabled).
- **Fix, in the core**: an UNCNFG or a RESET instruction (which
  unconfigures every chip and keeps DON) that changes how any nibble the
  display reads decodes keeps the picture rendered through the mapping
  from before it (`HeldFrame` in `machine/hardware.rs`). The decode
  compared is the first and last nibble of each of the 64 rows, main area
  for its line count and menu area for the rest; that is complete because
  a row is shorter than the smallest chip window (64 nibbles, aligned).
  The CONFIG that restores every row's decoding releases it, and so does
  one frame (128 ticks of 8192 Hz) without it. DON clear still shows a
  blank screen; a hardware reset clears the hold with DON. Every host (Tauri runner, Web Worker, CLI dumps, control API,
  MCP) reads frames through `Machine::lcd`, so all of them get the fix.
- **Hardware basis**: the controller fetches one row per 244 us (wiki:
  hardware/display), so a gap of about one row period reaches one or two
  rows for one 1/64 s frame on a real panel. Holding the picture differs
  from that by at most those rows; a full row-by-row refresh model was
  not needed for this. Whether the row fetches go through the chip
  selects at all is not documented: recorded as open in the wiki.
- **Not saved** in state files: a load shows the loaded mapping as it
  is.
- **Menu crawler removed; categories from the manuals** (owner's call,
  PR 22). The crawl (every MENU number on the keyboard, labels read with a
  font built from TMENU, RCLMENU for submenus, key-path names, toggles
  pressed back, a VAR-menu heuristic) was over-engineered for what it
  gives: it needed workarounds for the clock hang and for menus that
  create variables, took 5-15 minutes per model, and would have to move to
  the new Kermit path in iteration 18. It supersedes the "Categories from
  the ROM's menus" and "Clock workaround" bullets above. The ROM still
  gives the names (XLIB decompiling) and runs the examples.
  `scripts/manual-categories.py` reads the library's manual texts:
  48SX owner's manual operation index (48SX), 48G AUR "Keyboard Access"
  (48GX; the 48SX where its own manual is silent), 49G AUG "Access" lines
  (49G CAS commands). The scans read the hardware keys but not the menu
  labels, so a category is the menu key (`MTH`), or `Keyboard` when the
  key is the command itself, with the manual and page in
  `categories.json`. The 49G takes nothing from the 48 manuals: its
  keyboard differs, and over half of the AUR's statements disagreed with
  the 49G ROM's menus in a comparison with the last crawl. Unplaced
  commands carry our category (marked ours) or none.

## 2026-10-05 (iteration 12c: RPL decompiler)

- **Names from the loaded ROM, never committed.** `saturnus-objects`
  builds a `NameTable` from the ROM image (`Machine::rom_nibbles`, new in
  the core) by scanning it for library headers whose hash and link
  tables check out (layouts observed and written to wiki:
  protocols/rpl-libraries; RPLMAN p. 13 documents the XLIB body before
  each command). Scanning rather than following HOME's library list
  needs no per-model RAM table and finds the 49G's libraries in any
  flash bank; an indirection to a hash table in another bank is resolved
  by trying every bank and accepting a unique valid table. Counts: 48SX
  J 410 names, 48GX R 539, 49G 852; every name of iteration 13a's
  catalogs resolves. Build: 4 / 7 / 23 ms native release, 8 / 10 / 33 ms
  in wasm under node; 16 / 60 / 90 KiB.
- **A ROM pointer is resolved the ROM's way**: the six nibbles before
  the target give library and command, accepted only if that library's
  link table points back at the target (on the 49G: same offset in the
  bank). Pointers to programs, code and primitives are commands, never
  followed (fixes `SIN` decoded as a program); data constants are still
  decoded. Without a table the pointer is an unnamed command with its
  address.
- **Argument counts at run time**, not from a table: the four evenly
  spaced first objects of library 2's commands are CK1-CK4&Dispatch,
  calibrated by `+` being binary; the most common other one is CK0. The
  unit operators are found as the end marker most of the ROM's own unit
  objects end with, preceded by four empty lists. Operator names,
  precedence and the special forms (`∂`, `Σ`, `|`, `NOT`, `√`, `!`) are
  syntax in our code, checked against the ROM.
- **Oracle: the ROM itself.** The e2e test compiles objects on the
  calculator and compares our text from RAM with the ASCII transfer
  (normalised in one documented function: header, line breaks,
  indentation, unit quotes, the 49G's tag colon) and, for display modes
  the transfer ignores, with the server's stack display. 1194 cases
  (hand-written structure cases, generated algebraics, programs and
  numbers at `SATURNUS_ORACLE_SCALE=20`) plus the 828 sources of 13a's
  examples: no mismatch on the 48SX, 48GX and 49G. The default run keeps
  the generated part small (the suite runs in a debug build).
- **Shapes**: `command` gains `name`, `address`, `library` and `command`
  (it keeps `source` for the MCP's ASCII fill); an XLIB name of a library
  the tables do not hold has no `name`, so the MCP still fetches its text
  and sends it back in binary; `program`/`algebraic` `source` and `unit` `unit`
  are filled when a host passes the table (`UserMemory::with_names`; the
  web bindings and so the runner, `ctl stack` and the page build it on
  the first object read and rebuild it when `Machine::rom_generation`
  moved, i.e. after the 49G's flash was programmed, erased or loaded from
  a state). `NameTable::build` handles the 48SX, 48GX and 49G and gives
  an empty table for the other models. On a banked ROM a pointer's
  command is accepted only when every copy of its library that could be
  mapped there agrees. The 49G's symbolic matrices decode as `array`.
  Text is capped at 65536 characters with `…`; algebraic trees render
  without recursion.

## 2026-10-05 (command palette)

- **One palette instead of a reference panel, an input box and an object
  editor** (owner, in discussion). Cmd/Ctrl+K opens a command palette that
  suggests the model's commands (with stack effect and description), the
  user's variables and app actions while typing; it is the command
  reference for lookup, the way to send text to the calculator, and,
  grown to an editor, the way to edit the live command line and stored
  objects. Plans: `iteration-19-command-input` (typing engine for the
  full character set, `commandLine` read, `insert`/`run`/`replace`,
  frozen screen), `iteration-13-command-reference` (the palette),
  `iteration-14-object-editor` (its editor mode).
- **Enter mimics the calculator's keys**: with no command line open the
  chosen command is typed and executed; with one open its name is
  inserted at the cursor; Cmd/Ctrl+Enter does the opposite. This needs
  the command line's state read from RAM.
- **Number shortcuts** on the first nine rows, as in Alfred: Cmd+1..9 in
  the desktop app; Ctrl+1..9 in the browser, because Chrome and Safari
  reserve Cmd+digit for tabs.
- **Everything is sent as key presses**, at unlimited speed, with the
  calculator's screen frozen and a busy mark while more than a few
  characters are typed (owner: "freeze the screen while typing"). No
  Kermit, so none of this waits for `kermit-proto`. The faster key-buffer
  path is taken up only if measured typing speed is too slow.
- **Categories of commands**: from the manuals for PR 22 (iteration 13a;
  the key-pressing menu crawler is removed). A static decode of the ROM's
  own menu definitions was tried on the 48SX after iteration 12c (all 59
  numbered menus are the elements of one list in ROM; 334 of 397 commands
  placed, more accurately than the crawler) and is the intended per-ROM
  source once the list is located on the 48GX and 49G; the manuals then
  supply keyboard placement and the cross-check.
