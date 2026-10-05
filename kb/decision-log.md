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
