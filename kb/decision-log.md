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
  The saturnng container's pty keeps them; satx drains on connect either
  way.
- **`--autostart`** types the same keys as the saturnng container's
  `AUTOSTART` (NO at the boot prompt unless `--load`, then ALPHA ALPHA
  S E R V E R ENTER), then prints `serial bridged on tcp:HOST:PORT` on
  stdout as the readiness line. ENTER is held and released without
  waiting for idle, because the running server never idles in SHUTDN.
- **satx in-process transport** lives in satx (feature `saturnus`, a git
  dependency pinned to a saturnus commit), not here: saturnus exposes only
  the `Machine` serial API. It runs emulated time, not paced: `read` runs
  until the output goes quiet or the timeout's worth of emulated time has
  passed, and `write_packet` first replays the host's wall time spent
  outside the transport (at most 2 s), so satx's turnaround pause reaches
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
  `49g-boot`, `49g-arith`, `49g-menu`, `49g-alpha` match; satx's e2e suite
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
- **satx dependency**: `satx-core` by git, pinned to satx main
  `1a6cec7420ff540320c33607549eae10ecee76a1`, default features only, so no
  second, older saturnus is built. The server implements satx's
  `Transport` over its own `Machine` (`link::MachineTransport`). An empty
  host command (`C` with no data) returns the stack unchanged; that is
  `read_stack`.
- **MCP time model**: the machine only runs while a tool runs, with no
  background thread. Keys run in emulated time with the CLI's idle wait.
  Kermit reads run emulated time in 1 ms steps until the reply has gone
  quiet for 4 ms or the client's timeout has passed in emulated time, then
  sleep out the rest of a timeout in wall time. Writes first replay the
  wall gap since the last call (at most 2 s), which covers satx's 200 ms
  turnaround. The Kermit client times out after 6 s per packet with 3
  retries instead of satx's 20 s and 5, so a calculator that left server
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
  questions/hp38g-39g-transfer-protocol; satx needs a Kermit server mode
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
  `list_vars`, `cd`, all on the ROM's Kermit server through satx-core.
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
  satx-core, which needs no change today) decodes reals (12 digits,
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
- **Turnaround in emulated time**: satx's 200 ms wall-clock pause between
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
  cannot break out of its quoting: names that fail satx's
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
  (a running program; later per model, see "Key waits per model"), and a
  key the user holds still never blocks.
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
  only one is `https://github.com/ractive/saturnus` (satx-core, pinned by
  rev in saturnus-mcp). Unknown registries and git sources are denied.
  Advisories block the merge, with no ignores: upgrade the dependency;
  an advisory that cannot be fixed is the owner's call, recorded in
  `deny.toml` with the reason.
- **Licences**: the allow-list is exactly what the tree needs (MIT,
  Apache-2.0 for rmcp, Unicode-3.0 for unicode-ident), narrower than
  hyalo's. One crate-scoped exception: serialport (MPL-2.0), a
  non-optional dependency of satx-core that saturnus never uses to open a
  port; it goes away if satx-core makes serialport optional.
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
  Dependabot. satx-core's rev is moved by hand.
- **Release** (`kb/docs/releasing.md`): `release.yml` calls
  `ractive/release-workflows@v0.2.1` for the `saturnus` CLI; no crates.io
  (the satx git dependency rules it out), winget, AUR, Cloudsmith or
  deb/rpm. The shared workflow ships one binary, so saturnus-mcp is not
  in the archives, and it always runs the Homebrew and Scoop jobs on a
  real release (they need their token secrets).
- **Core publishable to crates.io**: first requested from the satx side
  for satx-cli, withdrawn the same day (satx-cli will not be published);
  kept as an option, no publication planned. It must stay free of git
  and path-only dependencies; `cargo publish --dry-run -p saturnus` in CI
  and `just gates` enforces it. The package excludes `tests/` (golden
  files are ROM screen dumps). Other crates stay unpublished.
- **satx-saturnus**: satx moves its saturnus transport into an adapter
  crate `satx-saturnus` in the satx repository, driving the core's public
  `Machine` API (`serial_push`, `serial_drain`, `serial_pending`, a
  framebuffer accessor). `saturnus-mcp` keeps its own `MachineTransport`
  and must not depend on `satx-saturnus` (it would bring a second copy of
  the core). Do not bump `saturnus-mcp`'s `satx-core` pin until satx's API
  PR has merged; expect to add `_ =>` arms on its enums then.
- **Core API relied on by satx** (semver-relevant once `saturnus` is
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
  size walk and HP-charset decode, because `satx-core` pulls `serialport`
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
- **Core API relied on by satx (extends iteration 16's entry)**: `Model`,
  `Key` and `Annunciators` grow whenever a model is added. This iteration:
  `Model::Hp42s`; the keys `SigmaPlus`, `Xeq`, `Rcl`, `RollDown`, `Swap`,
  `Rs` (`Model::ALL` 7 entries, `Key::ALL` 80); the annunciator fields
  `updown`, `battery`, `g`, `rad` (`Annunciators::list()` now has 10
  entries). `lcd()` keeps rows of 131, top row first, and has 16 rows on
  the 42S. No `#[non_exhaustive]`: our own hosts keep exhaustive matches
  so a new model shows every place to touch; downstream code (satx) uses
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
  shifts, the "alpha spent" shortcut, the 400 ms letter settle; the busy
  gap is per model since "Key waits per model"). Both
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
  the skin references, satx and the ROM policy. The wiki stays outside
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
  repository's session after the satx session relayed it). saturnus is an
  emulator that serves a serial port and a control API: `saturnus run`
  in the foreground serves the serial port on TCP and an HTTP/1.1 + JSON
  API on 127.0.0.1 (token file, Host and Origin checks), `saturnus ctl`
  is its client, and calculator operations against a running saturnus go
  through the satx CLI over the serial port as against hardware. Plans:
  `iteration-17-control-api`, `iteration-18-retire-mcp`.
- **Supersedes** two bullets of the iteration 16 entry: the
  `satx-saturnus` adapter crate and the freeze of `saturnus-mcp`'s
  `satx-core` pin. Once `saturnus-mcp` is gone nothing in saturnus
  depends on satx: no git source in `deny.toml`, no MPL-2.0 exception for
  `serialport`. Until then the pin stays where it is (no bump). The list
  of core API that satx relies on (iterations 16 and 15) stays valid:
  satx's in-process transport keeps using `Machine` and will use
  `saturnus-drive`'s autostart.
- **One protocol, three adapters**: the HTTP API reuses the command and
  event shapes of `web/protocol.md` (Worker, Tauri, HTTP); additions go
  into that document.
- **Order of retirement**: the crate is deleted only after its ROM-gated
  Kermit tests have moved to tests that use `kermit-proto` from crates.io
  as a dev-dependency. Iteration 18 is blocked until that crate is
  published.
- **Web explorer and editor writes** (iterations 12 and 14): no
  `satx-core` in the page. The control API cannot serve the browser page
  (the page runs the core as wasm and has no server), so the hidden
  Kermit path is built on `kermit-proto` plus `saturnus-objects`, with
  keystrokes as the fallback. The owner asked to be told if this changes
  iterations 12 to 14 materially: it changes their dependency, not their
  design.
- **crates.io**: the satx side withdrew its request to publish the core
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
  4848-4852 (satx's containers) and 4860-4889 (bridges and tests on the
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

## 2026-10-05 (fix/48sx-clock-hang)

- **TIMER2 reads are live**, superseding iteration 4's "TIMER2 pending
  read": a read returns the counter even after an expiry the CPU has not
  vectored for (#FFFFFFFF right after the wrap, then counting down). The
  frozen read deadlocked the ROM with the clock shown (flag -40, TIMER2
  reloaded every second): when TIMER2 expired while the handler was in
  service for a key, the handler waited at #009B9-#009BD for TIMER2's
  nibble 1 to change, which the frozen read never did, and the vectoring
  that would have ended the freeze cannot come while the handler loops.
  Keys were lost and the clock stopped. The 48SX ROM J and 48GX ROM R
  have the loop at #009A9; the 49G ROM 2.10 has the same code. wiki:
  hardware/timers "TIMER2 read during service".
- The flag is gone from `Timers`; the saved state keeps its byte (written
  as 0, ignored on load), so existing states still load.
- Tests: `io::timers::read_during_service_tests` (ROM-free), MCP e2e
  `clock_display_keeps_keys_and_time` (48SX, 48GX, 49G: clock on, ten
  digits, all on level 1, status line still changing), differential
  scenario `clock-keys` (48SX against saturnng).

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

## 2026-10-07 (iteration 13: the command palette)

- **Ranking** (`web/reference.js`, `search`): tiers, then a shorter name
  first within a tier. An exact name (the lookup rules of `saturnus ref`:
  the same letters in another case, a translation code such as `\.S`, a
  friendly spelling no other command has) beats everything; a friendly
  spelling another command owns (`INT` for `∫` on the 49G) comes right
  after it, so `INT` lists INT and then ∫. Then names the query begins
  (commands and the user's variables in one tier, a variable before a
  command of the same length), the ROM's menus whose last name the query
  begins (the 48SX has no PLOT command: `PL` offers its PLOT menu), a
  name the query begins with (`STO` for "store"), names containing the
  query, app actions, then descriptions and example text, where the
  order is alphabetical rather than by length so that `-` and `!` do not
  crowd the top. Measured in headless Chrome: 3-10 ms per keystroke for
  the search and the redraw of the list and the entry, the index built
  in 7-21 ms; the 840-command search alone under 0.1 ms in Node.
- **Enter rule**: a command and typed text mimic the keys (no command
  line open: `run`; one open: `insert` at the cursor, with the spaces
  the calculator would put around the name); a variable inserts its
  name, as the plan says, and Cmd/Ctrl+Enter does the opposite in both
  cases. The command line is read when the palette opens and after every
  send, never polled. Text that is not a single name (whitespace, or a
  delimiter or digit first) is offered as "send as typed" first; a bare
  token that names nothing exactly is sendable as the last row. "Try
  it" always runs the example's setup, input and command and keeps the
  palette open for the next one. A `run` that left the line open with
  an error keeps the palette open and shows the message; the palette
  closes after any other choice so the result is seen.
- **Data loading**: one file, `web/commands.json` (541 KB, 101 KB
  compressed), folded from `data/commands/` by `scripts/commands-json.py`
  (`--check` in `just lint` and CI), committed like `about.json` and
  `flags.json`, so `web/site.sh` ships it and the desktop app embeds it
  with the page without a change to either. Fetched once, when the
  palette or the Commands tab first opens; one index per model. The
  examples' typed stacks stay out (the page shows the display text); the
  key legends that place a command no manual mentions come from the
  skin the backend already serves.
- **Layout**: a modal `<dialog>` (the keys are its own while it is open,
  as `sat-calculator` already leaves dialogs alone; closing blurs so the
  keys return), 960 px wide and 620 px tall at most, 48 px from the top,
  the calculator visible behind a dimmed backdrop; the input across the
  top with the model's name, a state line under it only when something
  cannot be done, the list on the left (47%) and the entry on the right;
  below 760 px the entry goes under the list. Rows: name in the mono face
  with the stack effect after it, the description beneath, a kind badge
  for anything that is not a command, the number hint on the selected
  row and on hover. The 49G in algebraic mode (flag -95) gets a state
  line and an action "Switch the HP 49G to RPN mode" (`CF(-95)` through
  `run`), since the examples and command names are RPN text. The
  Commands tab reuses the memory view's two-pane shape (menu tree,
  command list, the entry below) and the layer's title follows the tab.
- **Number shortcuts**: Cmd+1..9 only in the desktop app on a Mac
  (`backend.host === "tauri"`), Ctrl+1..9 in Mac browsers, Alt+1..9 in
  browsers on Windows and Linux (Chrome and Firefox reserve Ctrl+digit
  for tabs there as Mac browsers reserve Cmd+digit; found in the review
  of PR 31), the hint on each row following; Cmd+K and Ctrl+K both open
  everywhere, and inside the open palette close it.

## 2026-10-06 (iteration 13b: menus from the ROM)

- **Static read of the ROM's own menu definitions** (`saturnus_objects::menus`,
  wasm-clean, under a millisecond per ROM): `MENU`'s code is followed
  through the unnamed programs it calls to either a list of at least 20
  definitions (48SX) or a system binary naming a library without command
  names whose commands are definitions (48GX, 49G: library #A9). One rule,
  no addresses compiled in (wiki: protocols/rpl-libraries, "Built-in
  menus"). A key that leads to a submenu offers the submenu, not the
  commands its shifted variants type (`IF`'s `IF THEN END`).
- **Menu names from the keyboard, observed without Kermit or the screen**:
  `saturnus-refgen menus` presses every legend of the model's skin (with
  its shift) after two baseline keys and reads the current menu from RAM
  (`Layout::menu_ptr`: the definition on the 48SX, an XLIB name of #A9 on
  the 48GX and 49G); a submenu takes its parent's name and its key's label
  (`MTH BASE BIT`). A menu no key opens is `MENU n`, as `n MENU` opens it:
  most of the 48GX's (its shifted keys open input forms; holding the shift
  did not give the menus in the emulator). The 49G's keys give soft menus
  with flag -117 set, which the step sets in RAM.
- **Data**: catalogs gain `menus` per command and `menu_keys`;
  `categories.json` keeps the manuals' statements and gains the ROM's
  `menus` (written by both the step and `scripts/manual-categories.py`,
  every level sorted so both write the same bytes); `reference.json` drops
  our editorial group for the 314 commands the ROM places on every model
  that has them. `saturnus ref` prints `menu:` (ROM), `key:` (manual) and
  `group:` only where neither exists.
- **Keyboard commands**: a command with no menu and no readable manual
  statement whose name is a key legend of the model (SIN, STO on the
  48SX; the scan lost the owner's manual's key line for SIN) shows `key:
  Keyboard (SIN key; ...)` from the skins' legends, not our group.
- **Coverage** (commands; from the ROM / from a manual only / key legend
  only / ours only / none): 48SX 397: 334 / 19 / 13 / 31 / 0; 48GX 517:
  402 / 58 / 14 / 43 / 0; 49G 830: 566 / 6 / 21 / 208 / 29.

## 2026-10-05 (iteration 12: memory view, read-only)

- **Layout**: the memory view is a layer of the page, not a second
  window and not part of the controls panel. From 1000 px it is a third
  column right of the calculator, 400 to 640 px wide (42% of the window
  between those): the calculator keeps its height at 1280 px and loses
  about a sixth of it in the desktop app's 1100 px window, where hiding
  the controls panel gives it back. Below 1000 px there is no room for
  both, so the layer lies over the calculator with a "‹ Calculator"
  button; below 760 px the top bar's Memory button toggles it. A wider
  cap was tried (the layer taking 6/11 of the room): at 1920 px its rows
  stretched over 900 px with the type at the far end, unreadable as rows.
  Three tabs (Variables, Stack, Flags) rather than everything at once:
  the column is too narrow for a tree, a list, a stack and 64 flags
  together, and each tab keeps hpcomm's shape where it applies (tree
  left, list right with name, size and checksum, the properties below).
- **Navigation is the page's**: browsing a directory never sends a key
  and never changes the calculator's directory. The view follows the
  calculator's directory while it shows it, stops following when the
  user browses elsewhere, and says where the calculator is with a way
  back. The tree lists directories only; variables are the list's.
- **Focus rule**: typing belongs to the calculator. A mouse click on a
  row, a tab or a button in the layer does not take the focus (as the
  page's buttons already blur after a click); the focus enters only by a
  click into a search field, by Tab from there, or by Alt+M, which was
  chosen because the calculator ignores Alt chords, Tab is its α key and
  F6 a menu key. Escape gives the keys back. Where the keys go is said
  in a line beside the tabs and by a bar along the layer's edge. The
  calculator's key handler decides by the event's path, not its target:
  a handler in the layer redraws the focused row before the document's
  listener runs, and the detached target had let arrow keys through to
  the calculator (found in the browser test).
- **Polling**: on the machine's side, in both hosts (`worker.js`, the
  shared runner), behind a `watchMemory` command so a closed layer costs
  nothing. A look happens only after the machine ran, only while it is
  not computing, at most every 100 ms; an event at most every 250 ms; a
  look that falls due while the host sleeps is made by a timer, not by a
  run pass. Looking only when idle was chosen over looking during a
  computation: the ROM's structures are whole only when it waits for a
  key, and a tree read in mid-flight fails or lies. A read that still
  fails while the calculator computes keeps what was shown and is
  repeated when it idles. Measured: the page follows 1 to 12 ms after the
  calculator goes idle; idle with the layer open is 0 run passes, 2 looks
  a second (the 48's own half-second wake), under 0.1 ms each.
- **Flag meanings**: data, not code, generated by `scripts/flags-json.py`
  from three wiki pages (`hardware/system-flags-48sx`, `-48gx`, `-49g`)
  whose tables carry flag, topic, name, the meaning of clear and of set
  in our own words, a status and the citation; the citation stays in the
  wiki. The script checks that every system flag is covered once and
  that nothing private leaks. What the guides do not establish is shown
  as such: the 49G's two guides refer to the Pocket Guide for the list
  (Advanced User's Guide p. 2-1), so 116 of its 128 flags have no meaning
  yet. The 48S/SX list was first carried over from the 48G guide and
  marked as assumed; the owner pointed to the Owner's Manual, and the
  list is now transcribed from its appendix E, with no "assumed" status
  left in the data or the page. The panel is read-only: lamps, not switches.
- **Text of programs**: the read API gives programs, algebraics, units
  and built-in commands without text (found at the start of this
  iteration; the decompiler is iteration 12c). The preview is written
  against the shape 12c delivers (`source`, `unit`, a command's `name`)
  and used it unchanged once 12c was merged; objects that still have
  no text show type, size and checksum with a plain sentence, and a list
  holding one shows grey placeholders and cannot be copied.
- **Tests of the page**: `web/objects.js` holds everything that can be
  tested without a DOM (text forms, program layout, previews, flag rows)
  and `web/test/` tests it with Node's own runner: no package, no
  dependency, one more gate (`just web-test`, the `wasm` job in CI).

## 2026-10-06 (iteration 19: typing engine)

- **The command line is read from RAM, not the screen**: text reversed and
  NUL-terminated below the temporary environments, cursor and editor
  flags in system RAM, per model by observation (wiki:
  hardware/command-line). `commandLine` presses nothing, so the palette's
  Enter rule can ask it at any time.
- **Characters by a generated, committed table**: per model the method of
  each of the 256 characters (alpha key, pair key, program entry key,
  accent, CHARS, none), written by a ROM-gated survey that reads every
  key's insert back from RAM. Committed because the alpha-shifted
  characters are printed on no key (the skins cannot give them) and a
  survey takes seconds; key names and characters are facts, no ROM bytes.
- **Typing is self-checking**: the engine reads the line after every
  character, trims what a key adds (program entry's spaces and `()`),
  steps over a pair's closer, and stops with an error on any mismatch
  instead of typing on blind. Alpha mode does most characters because it
  types the same in every entry mode; locks and shifts found at the start
  are restored.
- **Keys timed by the ROM, not a clock**: hold 30 ms, wait for SHUTDN,
  20 ms gap, 100 ms before the same key again (measured: faster loses
  keys). Independent of `KeyQueue`, whose interactive timing stays.
- **No key-buffer fast path** (owner: only if typing is too slow): the
  ROM's own work is about four fifths of each key's time, so writing the
  key buffer would save about a fifth. Measured 140-680 characters per
  second of wall time in the browser; long pastes, if they matter, would
  need a direct write of the edit buffer, which is a separate decision.
- **`typeText` became `insert`** (newline is the calculator's newline,
  ENTER is `run`), on all three hosts; a send of more than 12 characters
  sets `busy` and holds the frames.
- **Error messages from RAM by heuristic**: a parse error stores no
  number; the message is the new string the ROM built to show it.
  Accepted with its limit (a program that builds message-like strings and
  then fails may add them) rather than reading pixels.

## 2026-10-06 (iteration 20: remember the ROMs)

- **One place identifies a ROM**: `crates/saturnus-web/src/romid.rs`,
  wasm-clean, so the Worker runs the same code as the desktop app. The
  known images (SHA-256, models, revision) moved there from the CLI's
  `rom.rs`, with the CLI's SHA-256; the CLI's `revision` and `rom fetch`
  use them, and the page's model choice (`model_for_rom`) now runs on the
  same `fitting_models`. Result: exact, fits (by size and form: packed,
  or every byte a nibble) or unknown. The 512 KB 48GX and 38G images
  cannot be told apart by structure (the loader checks only size), so an
  unknown 512 KB file is offered for both, never assigned. The owner's
  42S dump is not in the list (no published image; the dump's own
  correctness is an open question, wiki: questions/hp42s-rom-crc), so it
  fits the 42S only: chosen, it is assigned; found beside another ROM, it
  is offered with a button.
- **One set of assignment rules** (`romid::plan`, exported to the Worker
  as `plan_roms`): a file the user chose fills the slots of its exact
  models (replacing what was there), or the selected model if it fits,
  or its only fitting model; one fitting several others is offered.
  Files found beside it fill only empty slots and only when exact;
  fits are offered; unknown ones are left alone. The 39G/40G image, being
  exactly right for both, fills both slots (the plan said "offered";
  assigning is not ambiguous here, and the notice names it).
- **Desktop app**: paths in `settings.json` in Tauri's `app_config_dir`
  (macOS `~/Library/Application Support/ch.ractive.saturnus`), written
  whole (temporary file, rename) with mode 0600 in a 0700 directory. A
  remembered file is checked by SHA-256 before every boot; missing or
  changed, the page is told and, when the user selected the model, the
  dialog opens again; at start it is only reported. The folder scan
  reads regular files of a ROM's size only (first 256 entries, 16 files,
  32 MiB, each with the 4 MiB cap; links not followed). The page learns
  file names and offer numbers, never a path; the ROM commands are
  answered in the Tauri layer (not the shared runner, which iteration 19
  changes) and boot through the runner's `boot` with the remembered path,
  in the page's order. Debug builds read `SATURNUS_SETTINGS_DIR` (and a
  temp file under the self-test hook) so tests never touch the user's
  settings.
- **Web page**: the bytes in a database of their own, `saturnus-roms`
  (not a version 2 of `saturnus`, which the page opens at version 1: an
  upgrade would have broken an open older page), slots by model and
  images by SHA-256 (the 39G and 40G share one copy). A write that fails
  aborts its transaction, so a slot never outlives its image. A refused
  or failing store (blocked storage, quota) leaves the slots in memory
  for the page's life with a note in the panel. The Worker now handles
  commands one after the other through a promise queue, as the ROM
  commands wait for IndexedDB.
- **Protocol**: `romSlots`, `bootModel`, `chooseRom` (files or an
  offer), `forgetRom`, and `romSettings` (the start-with-the-last-model
  switch, a fifth command the brief did not name). The HTTP control API
  does not serve them.

## 2026-10-06 (iteration 21: first release)

- **Published set**: crates.io gets `saturnus`, `saturnus-objects`,
  `saturnus-host`, `saturnus-drive`, `saturnus-cli`, in that order
  (`publish-crates` in `release.yml` and `publish-crates.yml`, `just
  package`); `saturnus-web`, `saturnus-tauri`, `saturnus-mcp` and
  `saturnus-refgen` are `publish = false`. One version for everything,
  `workspace.package.version`; internal dependencies are
  `[workspace.dependencies]` with path and version; the desktop app takes
  its version from its crate (none in `tauri.conf.json`).
- **New crate `saturnus-host`** below `saturnus-web` and
  `saturnus-drive`, holding the host-neutral code that lived in the wasm
  crate: the `Emulator` with its key queue (`host`), command-line typing
  (`typing`), layouts, skins, ROM identification and SHA-256. Chosen over
  moving it into `saturnus-drive`, which does file I/O and runs threads
  and would have needed feature gates to stay wasm-clean for the web
  crate. `saturnus-web` is now a thin `#[wasm_bindgen]` wrapper (a
  newtype around `saturnus_host::Emulator`, plus the free functions);
  the generated JavaScript API is unchanged (the `.d.ts` signatures are
  identical before and after; only doc comments differ; the wasm grew
  2%, 615 to 629 KB). Before the first publish the host crate's methods
  got idiomatic names (`new`, `run_ms`, `run_slice`, `stack`,
  `memory_tree`, `command_line`, `typing_result`, ...), returning
  `saturnus_host::Result` with a small `Error` (a message for the user)
  and typed answers (`MemoryTree`, `Flags`, `CommandLine`, JSON values
  where the answer is described JSON) instead of `Result<_, String>` and
  JSON text; the `_inner` names were the old wasm shim's (PR 29 review).
  `cargo tree -p saturnus-cli --target all` has no
  wasm-bindgen or js-sys. The fifth published crate deviates from the
  owner's four; the lead accepted it.
- **Command reference in the CLI crate**: `data/commands/` moved to
  `crates/saturnus-cli/data/commands/`, because `cargo package` cannot
  reach outside the crate and `build.rs` embeds it.
- **Package contents** by `include`: sources, README (each compiles as a
  doctest) and LICENSE (a copy of the root file, checked to match; a
  link would package as a stub from a checkout without symlinks); no `tests/` (goldens are screen dumps of HP's ROMs).
- **Publish rehearsal**: `cargo package --locked` of the five crates in
  one call (cargo 1.90+ verifies each against the packages before it
  through a temporary registry under `target/`; `just package` clears
  its cached copies first, as cargo would otherwise reuse a previous
  run's); about 19 s warm, so it runs in `just gates` and CI's quality-gates, replacing the core-only `cargo
  publish --dry-run`. `just doc` (and CI) builds the published crates'
  docs with `-D warnings`.
- **Off in the shared workflow**: winget (first submission is a manual
  PR to microsoft/winget-pkgs), Cloudsmith and deb/rpm (a Cloudsmith
  repository and package metadata first), AUR.
- **The desktop app embeds the page's files only**: `build.rs` copies
  them from `web/` with `web/site.sh`'s rule into `$OUT_DIR/app-site` and
  points Tauri's code generation there through `TAURI_CONFIG` (merged
  with what the Tauri CLI sets); `frontendDist` stays `../../web` for the
  CLI's existence check and `cargo tauri dev`. Rust, not the shell
  script, so the Windows build and a plain `cargo clippy` need nothing
  else. `web/site.sh --list` prints the selection and
  `crates/saturnus-tauri/tests/frontend.rs` compares the two.
- **`web/site.sh` checks the page's imports**: `pages.yml`'s fixed file
  list had missed `romstore.js` (iteration 20); the script now fails when
  a relative import or `new URL(...)` names a file not in the site.
- **Unsigned installers** for 0.1.0, labelled in the README and the
  release notes with the steps to open them (`xattr` on macOS, "More
  info, Run anyway" on Windows).

## 2026-10-07 (iteration 12d: the 49G's flags from the Pocket Guide)

- **49G flag meanings**: the source changed from "unknown" (the two
  guides defer to the Pocket Guide, which the library lacked) to the
  owner's printed HP 49G Pocket Guide, p. 76-79, photographed into the
  literature library and transcribed in our own words on the wiki's
  `hardware/system-flags-49g`: 103 flags with both states and defaults;
  the 25 the booklet leaves out stay marked as not listed.

## 2026-10-07 (iteration 23: owner requests on the page)

- **No plain button grid**: every model has a drawn skin, so the page's
  grid view, its "Drawn calculator" box and `saturnus.view` setting (an
  old value is removed on load) are gone. The protocol's `layout`
  command, the wasm `layout` binding and `saturnus-host`'s layout module
  stay: the native hosts' `model` result (`ctl model`'s key list) and the
  host's key names use them.
- **Pausing only in the palette**: the panel's Run/Pause button is
  removed; the `pause` command is unchanged everywhere.
- **The selected model is drawn without a ROM**, with an empty state over
  the LCD; choosing a model without a ROM pauses the running machine of
  another model, choosing that one again resumes it.

## 2026-10-07 (owner reports: speed and LCD contrast)

- **The speed applies to computing only**: at 2x, 4x and Max only the
  passes while the CPU computes or keys are queued run fast; a pass stops
  where the CPU goes to sleep, and the sleep follows the wall clock at 1x
  in the Worker and the native runner alike (no more 60x sleep at Max).
  The calculator's clock then drifts only by the time spent computing
  fast, and the auto-off and cursor blink keep real time at any speed.
- **The LCD is drawn from each model's power-on contrast**: observed after
  a cold start (48SX 11, 48GX/38G/49G 14, 39G/40G 12, 42S 22;
  `Model::default_contrast`, sent as the frame's `contrastDefault`), that
  value draws at 0.9 darkness, below it a square-root fade to 0.15 at the
  range's low end, above it up to 1 with the unlit pixels greying.
  "Darker display" and "Lighter display" (palette and panel) send ON + and
  ON - through the key commands, for layouts and devices that cannot hold
  ON.

## 2026-10-07 (deep review)

- **Whole-codebase review before v0.1.0** (owner's request): five
  reviewers in fresh contexts on the release tree; 46 findings (2 high);
  no plagiarism and nothing derived from the GPL emulators; no unsafe
  code. Fixed in PRs 32 and 33; the rest is iteration 23c, whose group A
  (public API) precedes the release. Record:
  `kb/research/deep-review-2026-10-07.md`.

## 2026-10-07 (iteration 18: saturnus-mcp retired)

- **saturnus-mcp is deleted**, with its `rmcp` tree, the `satx-core` git
  pin and `serialport`. `deny.toml` has no git source and no MPL-2.0
  exception outside the Tauri tree; Apache-2.0 left the allow-list (rmcp
  was its only user outside Tauri) and is scoped to the four Tauri crates
  that need it alone.
- **Kermit test host `saturnus-kermit`** (`publish = false`): the crate's
  Kermit coverage and `saturnus-refgen` moved onto it. It drives the
  machine's serial port in process and speaks Kermit through
  `kermit-proto` 0.1 from crates.io. A separate crate rather than a test
  directory because `saturnus-refgen` needs it as a normal dependency and
  the ROM-gated tests as their subject; not published because only this
  workspace uses it and its API follows the tests.
- **The link runs on emulated time only**: the client's clock is the
  calculator's idle emulated time while a reply is awaited (busy time does
  not count, at most 10 minutes per read), so no wall-clock catch-up or
  sleep remains and every exchange is deterministic; this replaces the old
  link's `deterministic` switch.
- **Host commands may be resent** (`first_packet_retries` left at
  `None`): `Some(0)` failed the decompiler oracle when the 48SX NAKed a
  `C` (its idle NAK crossing the packet) and `kermit-proto` resent after
  its stale-NAK grace. On this link a resend happens only after idle time
  without an answer, so the calculator never runs a command twice.
- **saturnus-objects gains `transfer`** (binary transfer files, sources
  from ASCII transfers, RPL source text for host commands, plain-name
  check) and `charset::encode_command` (trigraphs). No new dependency and
  nothing that runs unless called, so no feature flag.
- **Keystroke tests stay with the Kermit tests**: the host has key
  scripts, and what they prove (keys then the stack over Kermit, the 39G
  and 40G letters) needs the Kermit side or the aplet ROMs; the control
  API's e2e already covers screens as PNG and typing on the 48s and 49G.

## 2026-10-07 (iteration 25: side panels and keyboard)

- **Bindings are physical keys**: a shortcut is stored as
  `KeyboardEvent.code` plus modifiers (`Alt+KeyO`), never as the
  character, so a key keeps its place on every layout and a dead key's
  character cannot break it; the dialog shows the layout's label where
  the browser gives it (`navigator.keyboard.getLayoutMap`, Chromium), the
  US label elsewhere. Only ON, α, the two shifts and the app's actions
  are bindings; typed characters (letters, digits, operators, Enter and
  the arrows, F1-F6) stay fixed, because they must follow the layout's
  characters, not its physical keys.
- **Defaults avoid dead keys and typing keys**: every action keeps at
  least one default that is not a dead key unshifted or shifted on US,
  UK, German, Swiss German or French, and no default without a modifier
  types a character the calculator maps on those or on Dvorak. So the old
  `` ` ``, `[` and `]` are no longer defaults (dead on some layouts, and
  `]` is German `+`, `[` Dvorak `/`): ON is Esc and Alt+O, the shifts
  Alt+L and Alt+R (PR 40 review); checked by `web/test/bindings.test.mjs`. Alt+letter rather
  than F7-F12 because a Mac laptop's function keys are media keys
  without Fn.
- **A conflict is warned, not refused**: the first action in the list
  wins, and the dialog says so; reserved browser and system shortcuts are
  warned the same way. Refusing would block a user whose layout makes the
  "reserved" combination harmless.
- **Stored per browser in both hosts**: the panel widths and the
  bindings live in `localStorage` (`saturnus.panelWidth`,
  `saturnus.layerWidth`, `saturnus.keys`), also in the desktop app, whose
  webview keeps its storage across starts. The app's settings file
  (iteration 20) has ROM settings only and no general command to write
  page preferences; adding one is a Rust change left for later.
- **One menu tree**: a `MENU n` no key opens is named by the manual
  category most of its commands share (at least half, any model's manual),
  nested under the key menu of that name when there is one; the rest go
  under "Other menus"; the manuals' placements for commands without a
  ROM menu sit under their own folded heading instead of beside the ROM
  menus, so a name such as STAT appears once among the roots. The
  Commands tab's search uses the palette's ranking.

## 2026-10-07 (iteration 23c group A: public API before v0.1.0)

- **The core's public API is the `Machine` and what it takes and
  returns**: `Machine`, `Model`, `Port`, `io::Key`, `Lcd`, `Framebuffer`,
  `Annunciators`, `Error`, `Halt`, the constants `LCD_WIDTH`,
  `LCD_HEIGHT`, `LCD_HEIGHT_42S`, `CARD_MIN_BYTES`, `CARD_MAX_BYTES`,
  `NEW_CARD_BYTES`, `ADDR_MASK`, and `disasm::{decode, Instruction}` (an
  opaque instruction: its length and its SASM text). The modules `cpu`,
  `bus`, `machine`, `modules` and `state` are private; `io` exports only
  `Key`. satx's list (iterations 15 and 16) is unchanged and still public.
  New accessors where hosts read fields: `Machine::{pc, contrast, poke,
  release_all_keys}`, `Model::{has_key, keys}`, and `FromStr` for
  `Model` (the one place model names are parsed: CLI, Tauri, runner,
  bindings, refgen). `Machine::cpu`/`hw` are `pub(crate)`.
  `Error::Unsupported` (never constructed) is gone; `Error::UnknownModel`
  is new. Still no `#[non_exhaustive]` (iteration 15).
- **Internals for tests: feature `internals`**, not `#[doc(hidden)]`
  public items: it makes `Machine::cpu`/`hw` public and adds a hidden
  `saturnus::internals` module (bus chips, instruction types, LCD
  rendering) for the crate's ROM-gated `tests/e2e.rs` and the `boot`
  example (`required-features`). The crate enables it for its own tests
  through a dev-dependency on itself, which cargo strips when packaging.
  A build that includes the core's test targets therefore unifies it on
  for every crate, so `just lint` and CI's `clippy` job add a run without
  them (`--workspace --exclude saturnus --all-targets`, then
  `-p saturnus --lib`) in which it is off and neither a host nor a host's
  tests can lean on it.
- **The `profile` feature** keeps `Machine::profile` a public field; its
  types are re-exported at the root under the feature
  (`saturnus::{Profile, Bucket}`), since `machine` is private.
- **One error convention for the library crates**: a typed error enum
  where callers react to the kind of failure, `anyhow` where they only
  report it. The core keeps `saturnus::Error` (dependency-free) and
  `Halt`; `saturnus-host` has one `Error` enum (`Halted(Halt)`,
  `Machine(saturnus::Error)`, `Message(String)` for the rest) used by
  every public function, `typing` included; `saturnus-objects` and
  `saturnus-drive` use `anyhow::Result` with context, and the drive's
  `Session` returns a typed `session::Halted` inside it, so hosts tell a
  halt by `downcast_ref`, not by matching "CPU halted" in the text. No
  `Result<_, String>` in a library's public signature except the
  runner's protocol replies (`Runner::handle`), whose error is the
  reply's message by definition; group B replaces the runner.
- **Typed answers, JSON at the edge**: `saturnus-host` no longer builds
  JSON text. The `frame` and `keys` events are `host::Frame` and
  `host::KeysDown`, the skin `skins::SkinView`, the key grid
  `layout::Grid`, a send's reply `typing::SendResult`, the annunciators
  `AnnunciatorFlags`, all `Serialize` to the same JSON as before; the
  runner turns them into `serde_json::Value` without parsing its own
  output, the wasm bindings into JSON text or `JsValue`.
- **One "release everything"**: `Emulator::release_keys` lets go of the
  machine's keyboard (matrix and ON, `Machine::release_all_keys`) and
  clears the key queue at once; `KeyQueue::release_held` is the gentle
  one (each press released once held long enough, after a focus loss).
  `Emulator::release_all` (matrix only) is gone.
- **wasm bindings**: exactly the calls `web/worker.js` makes. Removed
  `run_ms`, `key_down`, `key_up`, `release_all`, `keys`, the `skin`
  method, `framebuffer`, `lcd_height`, `annunciators`, `contrast`,
  `contrast_range`, `is_shutdown`, `clock_hz`, `typing` and the free
  `rom_bytes`, `rom_fits`; the generated `.d.ts` of the rest is
  unchanged.

## 2026-10-08 (iteration 23c group B: one protocol, one implementation)

- **The protocol and its pacing are one state machine**,
  `saturnus_host::protocol::Engine` (wasm-clean: no clock, timer, thread
  or I/O of its own). A host feeds it commands (`command(clock, msg,
  bytes, reply_tag)`), delivers `take_output()` in order (events, then
  the replies they precede) and arms one timer for `deadline()`, which
  calls `timer(clock)`. The clock is the host's (`Clock::now_ms`):
  `performance.now()` through a JavaScript function in the Worker,
  `Instant` natively. The Worker (`web/worker.js`) and the native runner
  (`saturnus-drive`'s `runner.rs`) are thin drivers; what only one host
  has stays with it (IndexedDB and the ROM slots in the browser; files,
  dialogs and the native commands `screen`, `info`, `model`, `peek`,
  `poke`, `keyScript` natively, served around the engine through
  `answer`, `poke` and `exclusive`). Inside it, the pacing loop and the
  memory watch run on a small private `Core` trait, so their unit tests
  use a fake core with a fake clock; the commands are tested on a ROM of
  zeros.
- **The Worker's pacing rules are the rules** (protocol.md documented
  them; the native `Pacer` with its 1 ms slices and 200 ms re-anchor is
  gone, as is `saturnus-drive::pacer`). The hosts differ only in a tuning
  table defined once (`Pacing::WORKER`, `Pacing::NATIVE`): passes at
  60 Hz or every 1 ms (the serial bridge between them), pass budget 22
  or 4 ms, frames from passes unthrottled or every 16 ms, a send's turn
  40 or 16 ms, hidden pages pause the Worker only, and a 1 ms slack for
  browser timers that truncate their delay. Natively measured with the
  48SX: idle 100.00% with 0 passes, computing 1x 100.0%, 4x 399.6%.
- **Differences between the hosts, resolved by the protocol**: sends run
  in turns between other messages on every host, with one refusal list
  (`REFUSED_WHILE_TYPING`: key commands, another send, the boots, reset,
  states, the memory reads, `keyScript`, `poke`; message "typing is in
  progress (releaseAll stops it)"); `releaseAll` cancels a send natively
  too. A frozen send holds `frame`, `keys` and the key queue's `error`
  events on every host (a command's own error is its answer and goes
  out). No memory looks during a send. `stats` has the same fields
  everywhere (the native `rebases` is gone with the `Pacer`). Field
  checks are the native runner's on every host (`speed` must be a
  string, `typeKeys` names a missing array, `objectAt` a missing
  number); the Worker now caps a state at 4 MiB too. Kept as host
  differences, now documented: `visibility` changes nothing natively,
  Tauri's `loadState` and `saveState` answer `{path}`.
- **Cross-host test**: `web/test/protocol-script.json` through the
  engine with a fake clock, the native runner on its thread and the
  Worker with the real wasm in Node give the same transcript
  (`protocol-script.expected.json`; the runner's events in order and
  its replies by step, as they travel apart). `just web-test` builds the
  wasm package first.
- **wasm bindings**: `Host` (`command`, `drain`, `deadline`, `timer`,
  `check`, `boot`) replaces the `Emulator` binding; `model_names`,
  `identify_rom`, `plan_roms` stay for the ROM slots. Measured in headless
  Chrome on a 48SX, before and after: idle Worker work 0.4-0.7 ms and 20
  wakes per 10 s both; idle at Max 10055.5 / 10056 ms before, 10062.3 /
  10062 after; a program redrawing the display gives 29.4 / 29.2 frame
  events per second at 1x and 57.2 / 57.0 at Max, 60 animation frames
  per second throughout.

## 2026-10-08 (iteration 24: skin depth)

- **One depth rule for every skin, the data says what each panel is.**
  The owner found the cases flat next to the keys. The page now lights the
  whole drawing from the top left, as the keys already were: the case (the
  first panel) gets the shadow it casts on the page, a diagonal light
  across it, a rounded outer edge (a wide soft band and a crisp line, lit
  on the top left and shaded on the bottom right) and, where the real
  plastic has one, a grain; every other panel is drawn by its new
  `relief` (`flat`, `raised`: a thin lit edge; `sunk`: a shaded edge with
  its upper lip's shadow), and the glass is sunk into its bezel by an
  inset shadow on a `.glass` element over the canvas. The rule lives in
  `sat-calculator.js` (`drawDefs`, `drawPanel`, `drawEdge`, style.css
  `.skin .glass`); a skin only names its panels' reliefs and its
  `texture` (0-100; the 48s, 38G and 42S textured, the 49G's blue
  smooth). Edges are clipped inset strokes with one diagonal gradient
  each, so they stay crisp at every size; the grain is one
  `feTurbulence` over the case blended `overlay` (a mid-grey leaves the
  colour alone), composited into the case's shape. Measured in headless
  Chrome at 2x: a key press still paints at the frame rate (median 16.7
  ms, p95 16.8 ms over 30 presses) because the filtered path never
  changes; the contact shadow stays inside the stage's padding (no
  scrollbar). The case is its own `<g class="case">` so iteration 22's
  edge-to-edge fullscreen can drop it and keep the face.
- **The 42S's caps stand on skirts drawn as wells.** The photographs show
  every 42S cap on a black skirt a little wider than the cap; its skin
  now keeps the measured footprint as cap plus a 6-unit `well`, which
  the page draws as it draws the 48's wells. Its shifted labels take the
  photo's saturated orange (the most saturated pixels, not a median), its
  caps a darker brown, its glass the LCD's colour inside a narrower
  silver bezel (the unit's glass is taller than our square-pixel window).
- **The no-ROM message is back over the LCD.** The component's template
  closed `.skin` before the `.no-rom` element (a stray `</section>`), so
  its positioning rule never applied and the "Choose ROM…" state was off
  screen; found by the before screenshots, fixed with the template.

## 2026-10-08 (iteration 26: refinement and mobile)

- **One set of design tokens; every rule combines them.** The owner liked
  the theme and asked for refinement, not a new style. `web/style.css`
  now opens with the tokens (colours for both schemes, type sizes in one
  point steps, a 4 px spacing rhythm, four radii, three elevation levels,
  two durations and one easing, the target sizes, the safe-area insets,
  the layout widths) and nothing after them names a colour or a size of
  its own. Controls are flat (a border, no shadow); sheets and popovers
  take the second shadow, dialogs the third; focus is one 2 px accent
  outline everywhere; hover darkens a border and tints a row; every
  transition is a token that goes to zero under `prefers-reduced-motion`.
  The three `:root` blocks the file had grown (the memory view's and the
  palette's own variables) are folded into the one.
- **One icon set, drawn as an SVG sprite in index.html.** The glyph
  characters (`⤢ ☰ ‹ › ✕ × ⌨ ▸ ▾`) rendered differently per platform
  font; they are now `<symbol>`s stroked in the current colour
  (`components/icons.js` references them). The top bar on phones is the
  brand and four icon buttons (palette, memory view, fullscreen,
  controls), which is what fixed the 8 px horizontal overflow at 360 px:
  the labelled buttons never fit.
- **Targets: 44 px for controls with a finger, 40 px for list rows.**
  `--tap` is 32 px with a mouse and 44 px under `pointer: coarse`, on
  every button, select, tab, field and link; `--row` is 26 and 40 px for
  the tree, list, stack and flag rows. Rows at 44 px would make a tree of
  thirty menus a screen and a half tall; 40 px keeps a row tappable
  without that. The drawn keys of the skins are the calculator's own
  geometry and are outside this rule.
- **The palette is a sheet on phones, with the entry as a second step.**
  Below 760 px the dialog fills the screen: the input at the top, the
  list alone under it (rows of a finger's height with a chevron), and a
  tapped row opens its entry in place of the list with a way back and
  buttons for what Enter and Cmd/Ctrl+Enter would do (Run, Insert, Run
  this action, Open in the Commands tab). The sheet's height follows the
  visual viewport (`--vvh` from `visualViewport`, set while it is open),
  so the on-screen keyboard shortens the list instead of covering it; a
  shorter layout viewport in headless Chrome stands in for the keyboard
  in the test. Hover-selection and the keyboard hints are gone there;
  Escape and the keyboard still work for the few phones that have one.
- **Dialogs are full sheets on phones; the stage tools were drawn
  twice.** About and the keyboard shortcuts fill the screen below 760 px
  with a sticky head, a 44 px close button and the safe areas padded.
  The Commands and Memory buttons over the calculator appeared beside the
  bar's below 760 px because the rule hiding them came before their own
  `display: flex` in the file; the narrow layout is now the last section
  of the stylesheet so its rules win by order.
- **The Speed control follows the ARIA radio group pattern.** One tab
  stop (the checked radio), the arrows move the selection and wrap, Home
  and End go to the ends (`web/radiogroup.js`, pure and tested in Node;
  `sat-controls.js` wires it).
- **The overflow check is a web test that needs Chrome.**
  `web/test/overflow.test.mjs` serves `web/` itself, drives headless
  Chrome over the DevTools protocol with touch emulation, opens every
  view at 360, 390, 430, 768 and 1280 px in both schemes and fails on a
  page wider than the viewport or a container scrolling sideways (the
  trees and object tables may), then checks the palette sheet with the
  keyboard up. It skips without Chrome or the wasm package; `just
  web-audit` makes the skip a failure. CI's runners have Chrome, so it
  runs there as part of `web-test`.

## 2026-10-08 (iteration 22: installable web app)

- **The installable page's files are the site's only.** The manifest,
  the icons, Apple's head tags and the service worker live in `web/pwa/`
  and `web/site.sh` puts them at the site's top and into its
  `index.html`, as it already does with the CSP. The desktop app's
  `build.rs` embeds the top of `web/` and `components/`, so it never
  sees them; the Tauri test `frontend` asserts it, and `web/pwa.js`
  registers nothing on the Tauri host or over plain HTTP but on
  localhost. Served from `web/` in development the page has no worker,
  so a stale cache never hides an edit.
- **One build, one cache.** `site.sh` writes `BUILD`, a hash over every
  file it ships (names and contents, so the same tree gives the same
  hash), and `FILES`, their list, in front of `web/pwa/sw.js`. The
  worker precaches all of them past the HTTP cache into
  `saturnus:<scope path>:<BUILD>`, answers those and the page's
  navigation from that cache only, lets anything else through uncached,
  and drops its own scope's other caches when it takes over (another
  deployment on the origin, say a preview path, keeps its own). The glue JS and the wasm therefore never
  come from two builds. `web/test/site.test.mjs` compares `FILES` with
  the built site and checks the hash for a repeated and a changed build.
- **A new build waits, unless the page is untouched.** It installs in
  the background (the browser's check on navigation, and `update()` when
  the installed app returns from the background) and waits. A page with
  no touch or key yet takes it and reloads at once, which makes "a new
  deploy replaces the cached version on the next open" true with one
  quick reload; a page in use shows a notice with Reload and Later,
  because a reload restarts the calculator and loses what is not saved.
  Either way the worker takes over only when the asking page is the only
  window open, and claims pages only on the first install: activating
  would switch every open page to the new worker and its cache, so a tab
  in use would keep build N's code and fetch build N+1's lazy files
  (commands.json, about.json). With others open the page hears `others`
  and the new build waits until all have closed (PR 45 review).
- **Manifest orientation `any`, not `portrait`.** The manifest has no
  "preferred" orientation, only a lock, and iteration 26's landscape
  layout (the calculator alone at full height) would be locked out; the
  phone's own rotation lock stays the owner's choice.
- **Edge to edge in fullscreen, also on the iPhone.** Fullscreen hides
  the skin's `g.case` and crops the SVG to the face's bounding box (the
  plates, the window, the logo and model band, the keys) plus 4 units,
  on the case's colour; it scales to the width or height, whichever
  binds, with at most 3% lost to whole-pixel LCD snapping instead of 8%.
  The logo band stays: no model loses a key row to it (48SX at 360 x 780
  is height-bound at 343 px, every key inside). The crop is measured
  without the logo; a logo outside it (the 42S's, on the case above the
  plates) is dropped rather than leave a band of case above the face.
  The corner buttons sit on a dark translucent disc with a light ring,
  44 px on coarse pointers, so they read on every skin's bezel. Where the page cannot
  have the Fullscreen API (the iPhone) the button is no longer disabled:
  the stage is laid over the page instead (`fs-page`), which in the
  installed app is the whole screen. The close button stays top right;
  the palette's search icon sits top left; a downward swipe of 40 px on
  the display opens the palette; nothing floats over the keys.
- **Keys by touch.** Iteration 26 had covered `touch-action:
  manipulation`, no selection, 44 px targets, the hidden keyboard hints
  and the phone layouts; added here: no callout or context menu on a long
  press, each pointer's key released once (up, cancel or lost capture),
  `touch-action: none` on the calculator in fullscreen (nothing scrolls
  there, so a drag stays the key's), and every held key released when
  the page is hidden, as on a window blur (the engine's `visibility`
  command only changes pacing; it releases nothing).
- **Wake lock after 5 s of computing.** The Screen Wake Lock is asked
  for once the run loop has been `frame` for 5 s and let go when it
  sleeps or stops; every model's idle machine sleeps (checked on all
  seven ROMs), so an idle calculator never keeps the screen on.
- **Persistent storage asked once, the answer shown.** When the first
  ROM is kept in the browser the page calls `navigator.storage.persist()`
  (unless already persisted) and says under the ROM hint whether the
  browser keeps the ROMs for good or may clear them.

## 2026-10-08 (the web page only on ractive.ch)

- Owner: "Remove <https://ractive.github.io/saturnus/> from the CI pipeline
  and delete the files there. Only deploy on ractive.ch". `pages.yml`
  keeps the build and the FTPS job; the GitHub Pages upload and deploy
  jobs are gone, Pages is disabled for the repository (which deletes its
  site) and the `github-pages` environment removed. Links in the README,
  CHANGELOG, release notes and the workspace `homepage` point to
  <https://ractive.ch/saturnus/>. The meta-tag copy of the CSP stays in
  the site's `index.html` for hosts that send no headers.

## 2026-10-08 (iteration 20b: ROM download)

- **Direct download in the app, a link on the page.** The owner: "Do
  the direct linking. We will not have soooo many visitors." The desktop
  app downloads a model's ROM from hpcalc.org with one click after a
  native confirmation that says what is downloaded, from where, whose ROM
  it is and under what terms it is hosted; the web page cannot (no
  cross-origin header on hpcalc.org), so each empty slot links to the
  model's hpcalc.org page and names the file to expect, with the hint to
  unzip and drop it. Downloading on the user's machine is the same act as
  the user clicking the link; saturnus never hosts or proxies an image.
  No 42S download exists. Wording everywhere: HP's ROM, hosted by
  hpcalc.org with HP's permission for use with emulators; not part of
  saturnus.
- **One table.** `saturnus_host::romid::KNOWN` (pure data, wasm-clean)
  holds per image the SHA-256, models, revision, file name, size, zip URL
  and hpcalc.org details page; `romid::download(model)` is the first
  entry of the model (the 49G's is 2.15, the 2.10 fallback stays known
  but is not downloaded). The CLI's own `RomSource` table is gone. The
  page gets the data through the wasm core (`rom_download`), which the
  Worker puts on every slot as `download`, as the app does natively, so
  both hosts send the same field from the same table; a page-side JSON
  file would have needed its own sync check.
- **curl stays the HTTP client; the zip is read in Rust.** The CLI
  already ran the system `curl`; a Rust client with TLS (ureq or reqwest
  with rustls or native-tls) would add ring/aws-lc, webpki roots and
  licences outside `deny.toml`'s allow-list. The fetch moved to
  `saturnus_drive::fetch`: `curl -q` (no `.curlrc`, so no configured
  user agent or compression) with its own user agent and no
  Accept-Encoding (hpcalc.org serves a gzip bomb to fake browser agents),
  http/https only and https for redirects, `--max-filesize` and our own
  8 MiB read cap, timeouts, no console window on Windows. The zip's
  member is taken out in Rust (stored or deflated, through `miniz_oxide`,
  already in the tree, capped at the image's size), so `unzip`/`tar` are
  no longer needed and nothing touches the disk before the size and
  SHA-256 check; the image is then written whole (temporary file and
  rename).
- **Where and how the app keeps it.** `roms/` in Tauri's
  `app_data_dir` (debug builds: `SATURNUS_DATA_DIR`). A file there that
  verifies is kept without a download; one that does not is replaced
  (the folder is the app's own; the CLI refuses instead, its folder is
  the user's). The downloaded file is then taken exactly as a file chosen
  in the dialog (`Library::choose`, so the 39G image fills the 40G too
  and other downloads beside it fill empty slots), and the model boots,
  all inside the command's sequencer turn: the boot is sent only while
  `Sequencer::holds`. The library is not locked during the download. A
  failure is the command's error, naming the hpcalc.org page for a
  download by hand; a failed boot after it is `bootError`, as after
  `chooseRom`.

## 2026-10-08 (iteration 12b: explorer writes)

- **The hidden transaction lives in `saturnus-host`**, not in
  `saturnus-kermit`: the Worker, the native runner, the desktop app and
  the control API all run the engine, which is in the published,
  wasm-clean `saturnus-host`, and a published crate cannot depend on the
  unpublished `saturnus-kermit` (which also needs `saturnus-drive`).
  `kermit-proto` 0.1.1 is its new dependency (on wasm32 with
  `web-time`, MIT OR Apache-2.0, inside `deny.toml`'s list). The link is
  the same idea as `saturnus-kermit`'s (emulated time only, idle time is
  the client's clock, a turnaround before opening packets) as a
  steppable state machine, so a write runs in the engine's turns like a
  send; `saturnus-kermit` keeps its blocking link for now.
- **Server mode is entered by typing `SERVER`** with the typing engine
  (not by a keystroke script), and left with `G F` plus a wait for a
  stable screen; a server that stops answering is ended with ON. A
  command line being edited refuses the write, so the typing never lands
  in the user's line.
- **Fetch goes through Kermit too**, not straight from RAM: the file is
  the one the calculator itself sends (its own header letter, `J`, `R`,
  `C`), cut to the object's length, and stored again it gives the same
  bytes.
- **The calculator is left as it was**: an empty host command first
  counts the stack, a failed command's leftovers are dropped with
  `n DROPN`, flag -35 is set per transfer and put back, a write in
  another directory changes back, and no frame of the server's screen is
  sent (`busy` holds them, as for a long send). `IOPAR` stays: purging it
  before `G F` does not help, the server's end writes it again, and a
  real calculator has it after any transfer too.
- **Keys are the fallback only for the 49G in algebraic mode**, and only
  for flags (`SF(n)`/`CF(n)` typed, the echo dropped with the backspace
  key): a server entered from algebraic mode packs the stack in a list
  (iteration 12a). Its other writes are refused with the way out (clear
  -95, which the flags panel does by keys). The models without RPL memory
  refuse, as they refuse the reads.
- **Write commands are one per operation** (`storeFile`, `fetchFile`,
  `purge`, `rename`, `changeDir`, `setFlag`) rather than a generic
  `transfer`: each has its own checks before anything runs, and the HTTP
  API takes all six on one endpoint, `POST /v1/memory`. `rename` is
  `RCL`/`STO` then `PURGE` (`PGDIR` for a directory), so a failure keeps
  the old name.
- **Files stay the host's**: the Tauri app opens its own dialogs for
  `storeFile` and `fetchFile`, and writes the fetched bytes itself; the
  page never names a path. A file dropped on the app's page is sent as
  bytes (`dragDropEnabled: false`, so the webview delivers HTML5 drops),
  which keeps one drop path for the browser and the app.

## 2026-10-08 (39G/40G colours from the owner's photo)

- **The 39G/40G skin takes its colours from the owner's photo of an HP
  39g+**, not Thimet's 39G photo: light blue menu keys, cream function
  keys with black labels, dark blue digits, operators, ON and ENTER with
  white labels, a black ALPHA, an orange SHIFT, chrome cursor keys,
  orange-red shifted labels and SETUP bracket, a black display surround.
- **Contrast wins over the photo where they clash**: every label keeps
  4.5:1 (WCAG AA) against its cap or the face it is printed on, checked
  by the `hp39g_colours` test. Orange-red cannot reach 4.5:1 on the
  photo's lit mid blue, so the case is the photo's shaded blue, the
  shifted labels a lighter coral, the alpha letters light instead of the
  photo's dark navy, and SHIFT a deeper orange so its white label reads.

## 2026-10-08 (fullscreen fills the screen)

- Owner, on an Android phone: "In fullscreen mode you can still zoom a
  little bit more in and 'hide' the calculator bezel." Fullscreen now
  scales the face's core (the display window and, below it, the keys
  with their print) to the screen's width, or to the height the
  overlay buttons leave; the rim, the bezel's sides and the lettering
  above the window are cropped where the screen has no room for them,
  from the top first, so the keys keep the bottom of the screen. A face
  that fits whole is centred. The rule is `edgeLayout` in `web/edge.js`,
  unit-tested; `web/test/overflow.test.mjs` checks every model at
  360/390/430 px upright and on its side (keys on the screen and at
  least 84% of its width upright, the buttons over nothing).
- The search and close buttons keep 44 px and their corners; the face
  leaves them a 52 px band along the top, or, on a wide screen, room at
  the sides. They no longer cover the logo or the model's lettering.
- The "column of 3s" at the display's right edge in the owner's
  screenshot is the ROM's output, not a rendering fault: the four stack
  levels each held the real number 3, right-aligned. Read back from the
  screenshot, the pattern is the 5x9 digit 3 in columns 126-130 on rows
  16-24, 26-34, 36-44 and 46-54; `3 3 3 3` on a booted 48SX gives the
  same pixels in our frame, and the recovered-memory screen without them
  has nothing there.

## 2026-10-08 (iteration 14: palette editor mode)

- **A stored object goes back through the Kermit server, not by keys.**
  The plan put the editor's transport behind one function so it could
  switch once iteration 12b's hidden Kermit transaction existed; it does,
  and it is the more robust path on all three ROMs. Measured: a save is
  one transaction of 16 to 25 s of emulated time, 0.15 to 0.2 s of wall
  time natively (0.25 s from the key to "Saved" in Chrome), and 3500
  characters take 0.55 s; typing the same text runs at 2.4 to 10
  characters per second of emulated time (iteration 19), so 3500
  characters on a 48SX are past the ten-minute cap and refused, and the
  48SX cannot type `;`, the backslash or 58 other characters at all. Keys
  also depend on the entry mode the line is in; the server takes any
  character of the set as bytes. A live command line still goes back with
  `replace`, the only way to stay inside the calculator's edit.
- **The calculator compiles, and nothing in the text runs.** The text is
  wrapped in `{ }` and sent as a binary string variable (`SATEDIT`, or
  with a number where that name is taken); one host command recalls and
  purges it, `STR→`s it and asks `DUP SIZE 2 MIN`. A list is compiled but
  not evaluated, so a command in the text is stored as the command, as a
  PC transfer would, instead of running on the user's stack. The reply
  carries the calculator's own error (`Invalid Syntax`) or the number of
  objects; only exactly one is stored (`DROP 1 GET 'name' STO`) or put
  on its level (`n+1 ROLL DROP n ROLLD`). A failed compile is a result
  (`{error}`) rather than a failed reply, so the editor can show it as
  the calculator's message; the original object and the stack are left
  as they were (12b's leftover drop). What could move the wrapper's end
  is refused before anything runs: a `}` that would close it early, a
  string left open (it would swallow the closing `}`), and a `"` or `@`
  right after a word's character. The last is conservative: observed on
  all three ROMs, a mid-word `@` starts a comment and a mid-word `"` a
  string, as at the start of a word, but text like `X@ } 'P' PURGE {` is
  too easy to misread to accept (review of PR 51). The count is read by its first
  digit, so the reply is the same in any display mode (FIX 3 shows
  `1.000`).
- **The text to edit is not the display text.** `editText` decompiles
  in the display mode made fit for compiling again: every digit of a real
  (a no-op save in FIX 3 would otherwise lose digits), binary integers at
  64 bits, a tagged object as `:tag:object` (the stack shows `T: 5`,
  which does not parse). A no-op save keeps the checksum for reals,
  complex numbers, strings with newlines, algebraics, tagged objects,
  units, binaries, lists, arrays and programs (ROM-gated test). A string
  holding `"` has no text, as on the calculator.
- **A save names what it opened** (`was`, the object's size and
  checksum from `editText`, not its text, which follows the display
  mode: review of PR 51): the host refuses the write if the object is no
  longer the one the editor showed, so a
  variable changed on the calculator meanwhile is not overwritten
  silently, and a stack level that moved is not replaced by the wrong
  object.
- **Framework-free, no editor library.** A textarea with transparent
  text over a `<pre>` that shows the same text highlighted keeps the
  browser's own selection, undo (edits go through `insertText`), IME and
  on-screen keyboard; the RPL logic (tokens, brackets, indentation,
  layout, completion, digraphs, history) is about 500 lines of plain JS
  in `web/editor.js`, tested in Node, and the DOM part 250 in
  `web/components/rpl-editor.js`. A code-editor library (CodeMirror 6,
  MIT) would be a vendored dependency many times that size and would
  still need an RPL mode written for it; not proposed.
- **The palette grows to the editor only when the text needs lines**:
  Shift+Enter, Enter with a delimiter still open, a pasted line break, or
  pulled text. Growing on the first `'` or `"` typed would have broken the
  palette's quick `42 'ABC' STO` Enter.

## 2026-10-08 (ask before persistent storage)

- Owner: Firefox's context-free "allow to permanently save information?"
  prompt on page load could scare people away; ask in context, in our own
  words, first (the mobile-app pattern). The page no longer calls
  `navigator.storage.persist()` on load; it reads `persisted()` only.
  Right after the user keeps a ROM, and only if storage is not already
  persistent, a non-modal notice asks "Keep this ROM on this device?";
  "Keep it" calls `persist()` inside the click (Firefox prompts then),
  "Not now" is remembered in localStorage (`saturnus.storageAsk`). The
  ROMs panel shows whether the ROMs are stored permanently, with "Keep
  permanently" while they are not. Not in the desktop app. Iteration 28b.

## 2026-10-08 (no hung page tests)

- **The hang**: CI run 37823805294 (attempt 1) logged `Chrome did not
  start` in `web/test/overflow.test.mjs` after 15 s, then nothing until
  it was cancelled 30 minutes later, with an orphaned `chrome`, its
  wrapper's `cat`s and two `chrome_crashpad_handler`s. The launch threw
  outside the test's `try`, so that Chrome was never killed, and a live
  child process keeps the test file's process, and `node --test`, alive
  for good. Reproduced locally with a stand-in that never writes
  `DevToolsActivePort`: both tests fail, the run never ends.
- **The fix**: Chrome runs in its own process group and closing kills the
  group; a failed launch closes it before throwing; each test registers
  the close with `t.after` and the test's abort signal, so a failure or a
  timeout ends Chrome and the server; every DevTools call and the launch
  have their own timeout (30 s). A launch that fails now says why
  (exited or still running, Chrome's last stderr).
- **The 360 px flake** (`calculator at 360px light: main#stage.stage +8
  scrolls sideways`): the first reading after shrinking the viewport from
  390 px, taken before the calculator's ResizeObserver refitted the skin.
  Measurements are now taken after two frames, until two readings agree;
  no retries.
- **Backstops**: `timeout-minutes` on every CI job (about three times its
  usual length, more for cold caches) and `--test-timeout=300000` for
  `node --test` in `ci.yml` and `just web-test`.

## 2026-10-08 (iteration 27: the calculator keeps its state)

- Owner: a reload must not lose the stack and variables, as a real
  calculator keeps its memory when turned off. Agreed design: an
  automatic save into the model's own slot, only when the machine
  changed; about 5 s after the last change and at once when the page is
  hidden; never in the middle of a transfer, typing or a computation; the
  kept state restored on boot instead of a cold boot (no "Try To Recover
  Memory?"), a state that does not load falling back to a cold boot
  without an error dialog; a "Start fresh" palette command with an
  in-page confirmation; the desktop app the same, in its data folder; the
  manual Save state and Load state unchanged, in their own slot.
- **Change detection is a dirty mark, not a hash.** A hash of RAM and the
  CPU moves while the calculator idles (its clock, the cursor blink, the
  timers in the state), so it would write an idle calculator. The state
  machine marks a change on what the page or the outside does (keys,
  sends and writes at start and end, `loadState`, `reset`, `poke`,
  `keyScript`, serial input); a boot or a restore is none. "Settled" is
  the memory watch's own test: no send, the CPU asleep with no key down
  or queued, not halted.
- **The rules live in the state machine** (`protocol/autosave.rs`), so
  the Worker and the Tauri app share them and one protocol: the engine
  hands the host the state (`Output::Save`) and boots with a kept state
  (`Engine::boot_restoring`); the host only stores (IndexedDB
  `auto:<model>` from the Worker; `states/<model>.auto.state` in the
  app's data folder). `bootModel` gained `fresh`, `boot`'s result
  `restored`/`restoreError` (sent only when set, so a cold boot's reply
  is unchanged), and the event `autoSaved`.
- In the browser a computation stops while the page is hidden, so a
  change made just before is saved when the page is shown again and the
  computation ends; the last settled state is what a killed page keeps.
  A model switch saves the machine it replaces if it settled.
- Forget ROMs deletes the auto-saved 49G state with the user's (both hold
  the flash) and the Worker writes none until the 49G boots again; the
  desktop app's states stay, like its state files.

## 2026-10-09 (iteration 29: create a directory)

- **`createDir {dir, name}` is a host command, `'name' CRDIR`**, run in
  `dir` by the hidden Kermit transaction like `purge`: the calculator's
  own command, so its memory manager makes the directory; the same
  refusals, cleanup and change back as the other writes. One of the
  `WRITE_COMMANDS`, so every host serves it without code of its own;
  `saturnus ctl mkdir`.
- **The button says "New directory…", not "folder"**: the owner asked for
  a "create folder" button, but the page and the calculator say
  directory everywhere ("Make current", "Purge the directory").
- **A taken name is refused twice**: the page checks the directory shown
  (its tree is at hand) and says so under the field without sending
  anything; the engine checks again against RAM, for the other hosts and
  for a tree the page read before a change. The plain-name rule stays the
  engine's alone (`check_name`, as for `rename`); a command's name
  (`SIN`, `SUB`) passes it and the calculator answers "Invalid Syntax".

## 2026-10-09 (fullscreen crops only on phones)

- Owner: on a computer, fullscreen cropped the case like on a phone
  ("looks weird"); the whole calculator is wanted there. The crop of
  2026-10-08 ("fullscreen fills the screen") was applied to every
  fullscreen; it now applies only where the primary pointer is coarse,
  `(pointer: coarse)`, the query the page already uses for its touch
  sizing (`--tap`, the palette's focus on touch). Elsewhere fullscreen
  shows the whole skin, case and logo included, scaled to fit the
  screen with its aspect kept, centred, on the desk colour with a 12 px
  margin; `edgeLayout` lays it out with the whole skin as both face and
  core, so the corner buttons keep their band or side room and cover
  nothing.
- Not the 760 px breakpoint of the phone layout: a phone on its side is
  wider than that and must keep the crop, and a narrow desktop window
  (the stage laid over the page where there is no Fullscreen API) gets
  the whole calculator, smaller, with the buttons' band above it, which
  reads better than a cropped case under a mouse. A touch laptop or a
  tablet with a mouse follows its primary pointer; a change of pointer
  re-lays the fullscreen out at once.

## 2026-10-09 (the editor closes after a save; a dropped non-ROM says so)

- **Owner: a save that went through closes the editor**, the Save (and
  Send back) button and Cmd/Ctrl+S alike. Was: the editor stayed open
  with "Saved P in HOME in 0.24 s." under the text. Now that message is
  the status line's; the command line's Send back closed already. A save
  that did not go through (the calculator's compile error, an open
  string, the calculator busy, a changed object) keeps the editor open
  with the error and the text as it was. The decision is one function,
  `afterSave` in `web/editor.js`, and Cmd/Ctrl+S is the button's action
  through `editorKey`, both tested in Node.
- **The focus after the close** goes back to the memory view's Edit when
  Edit was pressed by keyboard (drawn again if the object changed),
  otherwise nowhere, so the keys go to the calculator, as for every
  button a mouse presses on this page.
- A failed read-back after a save (`reread`) no longer asks for a reopen
  in the editor: the editor is gone, and the next Edit reads the object
  again.
- **Owner: Cmd/Ctrl+E opens the editor** (the app shortcut "Edit",
  `Mod+KeyE`, rebindable in the shortcuts dialog): the object selected in
  the memory view (the variable shown, or the stack tab's level), else
  stack level 1 (read at the press when the memory view is closed),
  else nothing, quietly, as a disabled Edit button. Only while a 48SX,
  48GX or 49G runs and no write or typing runs, and not while a dialog
  (the palette) is open. The page takes the key (`preventDefault`), so
  the browser's Cmd/Ctrl+E (search with the selection, the search bar)
  does not run. The choice is `editTarget` in `web/editor.js`; from
  inside the memory view the focus comes back there on close.
- **Owner: a dropped file that is not a ROM says so.** `test.txt`
  dropped on the calculator went to the ROM chooser, whose notice ("is
  not a ROM image of a model saturnus runs") sat in the controls, out of
  sight with the panel hidden or on a phone. Now when no dropped file
  was assigned, offered or failed to boot (a ROM always is one of those)
  the notice beside the ROMs says "test.txt is not a ROM. To put a file
  on the calculator, drop it on the memory view." (the second sentence
  only while a 48SX, 48GX or 49G runs, the models whose memory view
  stores files) and the controls come into view (`sat-show-controls`:
  the sheet on a phone, the panel when hidden). No toast: the page has
  none, and the ROMs' notice is where ROM messages live.
- The desktop app takes ROMs only through Choose… and ignored a file
  dropped outside the memory view; it now says the file was not used,
  with the same pointer to the memory view.
- The whole memory view takes dropped files, any tab: over the
  Variables pane into the directory under the pointer, elsewhere into
  the directory shown. When they cannot be stored (no calculator, a
  model without a Kermit server, the memory not read yet) the view says
  why (`dropRefusal`); they never reach the ROM chooser.
- After a save that closes, nothing is read back; the read stays for a
  save the editor stays open after (text typed while it saved). The
  focus goes back to Edit lazily: to an enabled Edit at each redraw for
  1.5 s, then the selected row (`editFocusStep`). A late `close` event
  of an earlier open is ignored.

## 2026-10-09 (v0.1.0 release: re-run on release-workflows v0.2.3)

- The first v0.1.0 release run (37872237428) uploaded the seven archives
  and published nothing else: crates-io, homebrew and scoop were
  skipped. The shared workflow's `release` job needs `linux-packages`,
  which is skipped while deb/rpm are off here, and GitHub skips every
  job downstream of a skipped one whose `if` has no status function.
  hyalo enables Linux packages, so it never met this.
- Fixed in ractive/release-workflows v0.2.3 (its PR 2): each
  post-release job guards with `!cancelled() && needs.release.result ==
  'success'`; `!cancelled()` rather than `always()`, so cancelling a run
  still stops an irreversible publish.
- Nothing had reached crates.io, so the v0.1.0 GitHub release and tag
  were deleted and created again on `main` with v0.2.3, instead of
  publishing the crates by hand through `publish-crates.yml` and
  leaving Homebrew and Scoop to be written by hand.

## 2026-10-09 (iteration 32: the memory view's actions)

- **One primary button and a "⋯" menu in the preview head**, the head
  never wrapping, instead of a wrapping row of every action: the owner
  approved the mockup. The primary is the next step (Edit, Open, Make
  current), the menu the rest with Purge last in the error colour; a menu
  of one is drawn as a button beside the primary.
- **The directory shown gets a head and the same actions** (the tree's
  choice and HOME included), so a directory has the same set wherever it
  was chosen; "Change to this one" leaves the status line, as Make
  current is now where the directory's actions are.
- **The context menu repeats the menu** (right-click, Shift+F10, the Menu
  key) and adds nothing; no long-press on touch, where the "⋯" is always
  shown. The bar's two writes become one "New" menu.
- **The divider between the tree and the list is resizable** like the
  page's edges (one helper, `web/resize.js`), kept as
  `saturnus.treeWidth`; not on a phone.

## 2026-10-09 (iteration 30: shift-click and the shift glow)

- **Ctrl is the left shift, Option/Alt the right**, fixed and not
  bindings: a click has no physical key to rebind, and the pair matches
  the order of the shifts on the 48 and 49G. Exactly one of the two must
  be held: Ctrl+Alt is AltGr on Windows, and Shift or Cmd/Win with it is
  left to the browser (Cmd+click, Shift+click select). The 38G, 39G,
  40G and 42S have one shift: either modifier presses it.
- **Decided by the host's key queue when the press plays**: the click
  sends one `keyDown` with `shift`; when its turn comes, after every key
  queued before it has played and the ROM has settled (the rule letters
  follow), the queue taps the shift unless its annunciator is on, then
  presses the key, held as any click. A first version decided in the
  page from the last frame's annunciator and was wrong twice (review of
  PR 64): two quick Ctrl+clicks saw the first one's queued shift lit and
  sent the second unshifted, and a click on ↰ then a quick Ctrl+click
  queued a second shift that cancelled the first. The frame lags the
  queue; only the queue knows. On a one-shift model either shift
  annunciator counts (the host does not assume which one its ROM
  lights). A click on a shift key itself is a plain press. Mouse only:
  touch and pen send no modifiers.
- **The glow is a state machine of its own** (`web/shiftclick.js`),
  lit 150 ms after the modifier goes down alone, so Ctrl+K and Alt+L
  never flash; any other key, the modifier's release, a window blur, a
  hidden page, or any key or pointer event whose flags show the
  modifier up put it out, so a keyup lost to another window never
  leaves it stuck. It listens in the capture phase, before the page's
  shortcuts may prevent a chord. Only while the calculator has the keys
  (no text field, open dialog or memory view on the event's path).
- **A lone Alt's release is prevented** while the calculator has the
  keys: on Windows and Linux it toggles the menu bar (Firefox, the
  desktop app's WebView) and would take the next keys. After an
  Alt+click it is prevented too: harmless, and right whichever way a
  browser treats a click between the press and the release.
- **Look**: an SVG filter per shift colour. A light ink (on the dark
  cases of the 48, 38G, 39G/40G, 42S) turns nearly white in a halo of
  its own colour, a neon; the 49G's dark inks on its light face keep
  their colour on a pale plate. The other labels fade to 40 % so the
  lit set reads at a glance; the fade is a 160 ms transition, none under
  reduced motion. The skin keeps its own colours in both page themes,
  so the effect is the same in light and dark.

## 2026-10-09 (iteration 31: the screen as an image)

- **Drawn in the page from the frame, not in the host**: the page has
  the frame and draws the LCD already; a PNG from a canvas needs no
  encoder in the core and no new engine command. Only saving in the
  desktop app needs the host: `saveFile`, answered by the app itself
  (dialog, write), as the page never names a path.
- **Annunciators as pixel marks**: the page's LCD draws them as font
  glyphs, which would come out blurred at 4× and differ between systems;
  the image draws each as a small pixel mark in the same slot, so the
  whole image has hard edges and is the same everywhere.
- **Copy falls back to Save**: where a browser cannot put an image on
  the clipboard (no `ClipboardItem`, no `image/png`, a refused write),
  the image is saved and the status line says so, rather than a button
  that does nothing.
- **Keys**: Copy screen Alt+Shift+C and Save screen Alt+Shift+S, beside
  Alt+S (speed) and Alt+Shift+M. The owner first asked for Mod+Shift+C,
  but that is the browsers' inspect-element key (Ctrl+Shift+C, Cmd+Shift+C
  in Chrome and Firefox), which a page generally cannot take; the
  shortcuts dialog now warns when it is bound. Mod+S is the browser's
  save page and Ctrl+Shift+S Firefox's screenshot. Both rebindable.
- **The menu opens on a long press too**: iOS sends no `contextmenu` for
  a long press, so a finger resting 500 ms on the display opens it; a
  browser that does send one finds the menu open and does nothing more.

## 2026-10-09 (About names the release; the no-ROM message fits)

- **The version comes from the host's `hello`** (`env!("CARGO_PKG_VERSION")`
  of saturnus-host, the workspace's version): one place, the same for
  the Worker's wasm and the desktop app's native core, with nothing
  generated into the page that could fall behind. The site's build is
  the service worker's `BUILD` (web/site.sh's hash of the files), which
  the page already asks for; the desktop app says "desktop app".
- **The no-ROM message steps down until it fits** (`fitNoRom`, measured
  in the page): smaller print, then the first sentence without the
  download hint, then "No ROM" beside the button. Text outside the
  display was the other option; it would cover the face or the keys.
  What is cut (the hint's words too) stays in the tooltip, and as a
  phone has none, the text cut short is a button that opens the whole
  message in a menu, with the hpcalc.org page and Choose ROM. The
  status line was the other place: on a phone it is inside the closed
  sheet. overflow.test.mjs checks every model at seven sizes.
- **About says "build unknown"** until the service worker answers (a
  first visit, a reload past it, a local copy), and follows the answer
  while open: "no build" would read as a failed deployment.

## 2026-10-09 (49G writes in algebraic mode)

- **The 49G's writes leave algebraic mode for the server and return to
  it**, instead of being refused. A server entered from algebraic mode
  packs the stack in a list, so -95 cannot be cleared from inside the
  server as -35 is: `CF(-95)` is typed before `SERVER` (its echo dropped
  with the backspace key) and `-95 SF` after the server's end, in RPN
  mode, where it leaves no echo. The second is a cleanup step: it runs
  after the calculator's refusal, after an E packet, after a dead server
  ended with ON, and a stop types it at once. Inside the server nothing
  differs: the host commands are RPN, the replies the same. Checked on
  the 49G ROM 2.10 (`writes_on_the_49g_in_algebraic_mode`): createDir,
  storeFile, fetchFile, rename, purge, changeDir and a refusal leave -95
  set, the stack and the screen as they were; the keys add about 4 s of
  emulated time. `setFlag` alone keeps its key path.
- **A stop ends what it interrupted**, found while testing the above: a
  line typed in part (`SERV`) was left open, a stop right after
  `SERVER`'s ENTER (before the first NAK) left the server running, and
  the 49G's server takes ON during a transaction as the transaction's
  end and goes on serving. The stop now cancels an open line with ON,
  counts the server as running from `SERVER`'s ENTER, and presses ON
  again (at most four times) while the calculator still transmits within
  the idle server's NAK period (7 s).
- **A stop's cleanup runs in turns, not in the stop** (PR 69's review):
  ending a server can take tens of seconds of emulated time (ON, the
  return to the stack, 7 s of listening for a NAK, up to four times), and
  the 49G's mode is set again by keys after it. Run inside `stop`, that
  blocked the page's thread; now `stop` lets go of the keys and turns
  the rest into cleanup steps that `step` runs within each turn's budget.
  The write's reply waits for them, with what they could not do: a
  failed return to algebraic mode is never dropped.

## 2026-10-09 (Key waits per model)

- **A queued press waits each ROM's own time awake after a key.** Two
  quick Ctrl+clicks on √x on the 48SX gave 3 instead of 81 about one run
  in three under CPU load. Every press reached the machine in order; the
  ROM dropped the second shift. After a key goes up the 48SX ROM stays
  awake about 250 ms or, by where its timer stands, about 525 ms (after
  a shift; up to 750 ms after √x), and a key pressed in that time may be
  lost. The queue pressed after 300 ms (400 ms for a shifted press or a
  letter), inside that window in about one phase in five; load only moved
  the phase. Measured on every ROM over 40 timer phases, after a shift, a
  digit, ENTER and a function key (most ms awake after release): 48SX
  583/317/605/753, 48GX 521/197/516/508, 49G 5/74/462/89, 38G
  519/523/968/523, 39G and 40G 5/243/244/243, 42S 17/50/62/78. The busy
  gap is now per model, the longest of these plus a margin: 48SX 850 ms,
  48GX 600, 49G 550, 38G 1100, 39G, 40G and 42S 300 as before
  (`host::busy_gap_ms`). A key for a running program waits that long:
  about 1.1 keys a second on the 48SX instead of 2.8, unchanged on the
  39G, 40G and 42S. One cap of 600 ms for all was the first fix; it slowed
  the models that never needed it and was still short on the 48SX and 38G.
- **The letter settle stays a floor**: 400 ms, or the model's busy gap
  where that is longer, so it still counts on the 39G and 40G.
- **Tests**: a unit test with a ROM that stays awake 520 ms after each key
  (no ROM needed) and one pinning the key rate while the ROM is busy;
  `crates/saturnus-host/tests/keys.rs` replays the two Ctrl+clicks at 60
  phases on the 48SX and 48GX ROMs (8 of 60 wrong on the 48SX before) and
  checks that quick keys on the 48GX, 49G and 38G never go down while the
  ROM is still awake; each fails with the old 300/400 ms.

## 2026-10-09 (the CLI as .deb and .rpm)

- **The release pipeline builds a .deb and an .rpm of the CLI** (owner's
  request, as satx does): `enable-linux-packages: true` and
  `linux-package-crate: saturnus-cli` in `release.yml`, the metadata in
  `crates/saturnus-cli/Cargo.toml`. x86_64 only, the binary to
  `/usr/bin/saturnus`, the root README and LICENSE as its docs. Release
  assets `saturnus-vV-x86_64-linux.deb` and `.rpm`.
- **The package is `saturnus-cli`, not `saturnus`**: the desktop app's
  .deb and .rpm (Tauri, from its `productName`) are already the package
  `saturnus`, version V; a CLI package of the same name would make apt and
  dnf replace one with the other. Renaming the app's package instead
  would change a package already published with 0.1.0. The binaries do
  not collide (`saturnus` and `saturnus-app`), so both can be installed.
  On a release page the two read alike: `saturnus-vV-x86_64-linux.deb`
  is the CLI, `saturnus_V_amd64.deb` the app; the README says which.
- **No shell completions** in the packages: the CLI does not generate
  any yet. A `completions` subcommand (clap_complete, as satx) and the
  `pre-package-command` that writes them are a follow-up.
- **Cloudsmith stays off** until the owner creates the repository
  `ractive/saturnus` on cloudsmith.io (`CLOUDSMITH_API_KEY` exists).
- **release-workflows stays at v0.2.3.** With deb/rpm on, `linux-packages`
  runs and is no longer skipped, so the post-release jobs would also run on
  v0.2.2; v0.2.3's guard still matters whenever deb/rpm are turned off.

## 2026-10-09 (the CLI packages carry the musl binary)

- **The .deb and .rpm package the static musl binary**, not the glibc
  one: the first dry run (37911820913) measured the glibc build's floor
  at 2.39 (`Depends: libc6 (>= 2.39)`, `libc.so.6(GLIBC_2.39)`), because
  release-workflows builds it on `ubuntu-latest` (Ubuntu 24.04). That
  shut out Ubuntu 22.04, Debian 12 and RHEL 9. The musl archive's binary
  is already static-pie and the one Homebrew installs on Linux.
- **Through `pre-package-command`, without changing release-workflows**:
  in the `linux-packages` job (`BIN_PATH` is `target/release/saturnus`
  there) it builds `x86_64-unknown-linux-musl` and copies the binary over
  the glibc one before packaging. It relies on v0.2.3's `BIN_PATH`
  convention, which the exact-tag pin keeps stable; a `linux-package-target`
  input in release-workflows would be the clean form (proposed, not done).
- Rejected: an older runner for the job (not an input of v0.2.3, and
  ubuntu-22.04 would only lower the floor to 2.35); documenting the 2.39
  floor and leaving it.

## 2026-10-09 (the first minutes: the boot's question, messages, Forget ROMs)

- **The host answers "Try To Recover Memory?"**, not the page: the Worker
  and the desktop app run the same engine, so both get it, and only the
  engine knows when the ROM is idle at the question (no fixed delay). It
  is recognised by the screen's layout, measured on ROMs J, R and 2.10:
  a label under the first and the sixth menu key only, blank rows
  between, the question on top; the 49G's "Memory Clear" box after it
  by five solid labels and OK (`protocol/recover.rs`). Only on a cold
  boot with no kept state (and Start fresh), only on the hosts that
  keep the calculator: the CLI's control API and key scripts see the
  ROM as it is. A key the user presses first leaves the question to
  them; a question that does not come in 30 emulated seconds is not
  waited for. The answer is not a change to keep (no auto-save): a
  reload before the user does anything boots cold and is answered again,
  and Start fresh leaves nothing kept.
- **Messages**: an outcome goes after 6 s, a step in progress ("…")
  stays, an error stays until the next click or key (not the one that
  caused it). The palette maps the typing engine's internal failures to
  one sentence and logs the detail; the host's own reasons (a character
  the model cannot type, text too long) are shown as they are. The
  Fullscreen API's exception text is never shown.
- **Forget ROMs… asks** in the page's card (the Start fresh one, now
  `confirmCard`). **The keep-the-ROM offer** moved into the panel under
  the ROM row and waits until no frame has come for 2 s and nothing is
  being typed, so it never lands on the keys or on the boot.

## 2026-10-09 (a theme switch: System, Light, Dark)

- **`data-theme` on `<html>`, nothing for System**: the dark tokens apply
  under `:root[data-theme="dark"]` and, with a dark device, under
  `:root:not([data-theme="light"])`. CSS cannot put one rule under a
  media query or an attribute, so the set is written twice and
  web/test/theme.test.mjs checks the two are the same. `light-dark()`
  would need it once, but needs a newer engine than the desktop app's
  WebKitGTK on some Linux systems.
- **Applied before the first paint** by `theme-boot.js`, a classic script
  in the head: the site's CSP allows only the site's own scripts, so not
  inline. The module (`theme.js`) changes it later and moves the
  `color-scheme` meta, the installed page's theme-color and the desktop
  window's theme (`set_theme`, Tauri's own `WebviewWindow::set_theme`,
  no new dependency) with it.
- The Screen images choice was stored under the key "undefined" (it was
  missing from the page's preference keys); it is `saturnus.screenLook`
  now.
- **Disabled controls by colour, not opacity**: `opacity: 0.45` left
  their text at about 2.7:1 (light) and 3.6:1 (dark). The tokens
  `--ink-disabled` and `--border-disabled`, in the light set and both
  dark ones, give about 4:1 on the control's own background (3.5:1 or
  more wherever it sits) while enabled text stays above 10:1; the
  background stays the control's, and a primary button gives up its
  accent. The page test measures both themes.
- **The shift glow halfway back** (owner): PR 76 had made it faint; it is
  now between that and the first one of iteration 30. Light inks: dilate
  0.6, blur 2.6, one halo layer at 0.7, the text tinted 45 % towards
  white; dark inks: dilate 2, blur 1.8, the pale halo at 0.9. The label
  is still drawn sharp over its halo, so a light one does not blur into
  a blob.

## 2026-10-09 (iteration 33: copy and move)

- **A copy is the calculator's `RCL` and `STO`** inside the server, not a
  fetch and a store through the computer: one transaction, nothing
  leaves the calculator, and a directory comes with everything in it
  (as `rename` already does). One host command per packet (a Kermit
  command holds about 77 bytes): the paths, the `RCL`, the `STO`.
- **A move purges only after the calculator compared the copy**: `BYTES`
  (size and checksum) of both, compared with `==` and the purge or
  `DOERR "Copy differs"` chosen by `IFTE`, all on the calculator, so no
  memory read has to happen while the server runs. A failure leaves the
  original; a copy may be left in the target. `3 ROLLD` rather than
  `UNROT`, which the 48s do not have.
- **A taken name is asked about, and a directory is never replaced** (nor
  replaced by a directory): replacing one would need a `PGDIR` before
  the copy, and a copy that then fails would lose both.
- **The picker is in the preview, not a dialog**, like Rename and Purge:
  the same place, the same Escape, no focus trap. Rows that cannot be
  chosen stay focusable and say why.

## 2026-10-09 (the memory view's status row)

- Owner: a flag write made the view jump twice (the old message went,
  then the new one came, each a line inserted or removed above the
  flags). The view now has one status row under its tabs, always one
  line high: a write under way, then its outcome, else that the memory
  shown is old (the "calculator is busy" note, which also came and
  went), else a hint for the tab. The outcome gives way to the hint
  after 6 s and an error stays until the next click or key, by the
  status line's rules (web/status.js, now shared). One line with an
  ellipsis and the full text in its tooltip; the flags' own "Click a
  lamp" line moved into the hint. The busy overlay is absolute and the
  edit rows open only on a user's action, so nothing else moves on a
  write.

## 2026-10-09 (the ROM table: a primary and a ⋯ per row, Remove)

- Owner: empty rows stacked Download… over Choose…, doubling their
  height, with a tooltip over the button below. A first take gave each
  row one button with a menu (Add ▾, Change ▾); the owner chose the
  memory view's pattern instead (iteration 32's preview head): a primary
  button that acts at once and a "⋯" for the rest (web/romrows.js).
  Filled: Change… (the file chooser), ⋯ with Download again… (the app)
  and Remove…. Empty in the app with a known download: Download…, ⋯ with
  Choose a file…. Empty in the browser (its Download link to hpcalc.org
  stays in the row) and the 42S: Choose… alone. The ≤1-item rule of the
  memory view has one exception: Remove… alone stays behind the ⋯ (the
  browser's filled rows), as a destructive action is never a bare
  button. Every primary has one width and every ⋯ one size (a gap of
  its size where there is none), so the buttons line up and the rows
  keep one height.
- The file name sits under the model, smaller and muted (or "none", or
  the browser's Download link), not in a column of its own: beside a
  primary and a ⋯, the app's narrow panel cut it to "sxr…". Each row is
  two lines, the buttons centred against them; a name longer than the
  cell is cut, the whole one (and its revision) in the tooltip.
- **Remove…** asks first (the shared modal, below), and
  uses the hosts' `forgetRom` with a model (it existed on both). The 39G
  and the 40G share one file, so both go together, as the question says.
  A model running from the removed ROM stops through a new engine
  command, `unload`: no machine, as before the first boot, so the
  display shows its empty state. It stops before the ROM is forgotten:
  a change not yet auto-saved is saved first (saved states stay; the
  browser then deletes a 49G's with its ROM), and while a send types the
  engine refuses `unload`, so nothing is removed and the message says
  to remove it when the send is done (review of PR 92).
- "Forget ROMs…" is "Remove ROMs…", the same word as the rows'.

## 2026-10-09 (one modal question before what cannot be undone)

- Owner: the inline questions (Purge in the memory view's preview,
  Replace in Copy to and Move to, the ROM row's Remove) and the cards at
  the bottom (Start fresh, Forget ROMs) looked odd. Every confirmation of
  something that cannot be undone is now one modal (`components/
  confirm.js`, `confirmAction({title, body, action})`): a native
  `<dialog>` with `showModal()`, a short title naming the question, one
  sentence of what happens, Cancel with the focus and the action on a
  solid button in the error colour (`--error-ink` on it, 5.5:1 light,
  7:1 dark). Escape and a click on the backdrop cancel, the focus goes
  back to the opener, and as a modal dialog it keeps the keys from the
  calculator. At 560 px and less it is a sheet at the bottom, its
  buttons sharing the width.
- Fields stay where they are: Rename, the new directory's name and the
  directory picker. A Cancel in Replace keeps the picker open.
- The inline question code (the purge row, the Replace row, the ROM
  row's question) and the bottom card (`confirmCard`) are gone.

## 2026-10-10 (the logo in each model's colours)

- Owner (2026-10-09): the logo's light highlight on the planet's edge
  looked off; it is gone everywhere, so the plain logo is flat
  (`web/logo.svg`, `web/favicon.svg`, the installed page's icons, the
  desktop app's icons; regenerated with `web/pwa/icons.sh` and
  `cargo tauri icon` from `logo.svg` at 1024 px). The owner then asked
  for a colour pair per model.
- The headers' logo (side panel, phone bar) and the logo on the case
  take the model's colours: the running model's, else the one chosen in
  Model, else the plain logo's. One table in `web/style.css`
  (`[data-logo=<model>]`, `--logo-planet` and `--logo-ring`, each with
  its source in the skin). A model with two shift keys: left shift the
  planet, right shift the ring (48SX, 48GX, 49G). One shift key: the
  shift the planet, the case or a display accent the ring (38G: shift,
  ALPHA ink; 39G/40G: shift, case blue; 42S: shift, display plate). The
  40G's skin is the 39G's, so is its pair.
- One contrast rule for every pair (review of PR 102): in the headers
  each colour shows at least 2.4:1 against the panel it is drawn on, the
  plain logo's planet on the light panel (2.43:1), the least of the logo
  the owner has seen and kept. A skin colour short of it on one panel
  gets a tint for that panel only (`--logo-planet-light`,
  `--logo-ring-dark`, ...): its own hue, darker on the light panel or
  lighter on the dark one in steps of 0.5% HSL lightness until it
  reaches 2.4:1. Tinted: the 48SX's ring (light, 1.7:1 raw) and planet
  (light, 2.3:1), the 49G's planet (dark), the 38G's shift and ALPHA ink
  (light), the 39G/40G's case blue (dark), the 42S's planet (light). The
  case shows the skin's colours as they are. `web/test/logo.test.mjs`
  checks the rule from style.css.
- The headers' logo is inline SVG coloured by those properties through
  `data-logo` on `<html>`, set before the first paint by `theme-boot.js`
  and by `web/logo.js` later; the case draws the same mark (`drawLogo`)
  with its own model's `data-logo`. The boot script picks the model the
  page will show: the remembered one, else the first in the page's order
  (`web/models.js`, a global the boot script and `norom.js`'s
  `MODEL_ORDER` share; app.js picks the same), so the colour never
  changes as the page starts (review of PR 102: with nothing remembered
  it did). About, the favicon, the README and the icons keep the plain
  logo.

## 2026-10-10 (consolidation)

Owner decisions; the plan is iterations 36a (import), 36b (renames),
36d (site) and 36c (release), done in that order.

- **One repository.** The transfer tool moves from its own repository into
  saturnus with its git history, imported with `git filter-repo`.
  [[iterations/iteration-36a-import]] has the details.
  - Its tool is renamed **satx**: the crate and binary `satx`, and the library
    `satx-core`.
  - `kermit-proto` and `xmodem-proto` keep their names.
  - The old repository is deleted after the import and an export of its PRs.
    ractive/hp-literature stays archived.
- **Names.**
  - The core crate `saturnus` becomes **`saturnus-core`**.
  - The CLI crate takes the name **`saturnus`**, so `cargo install saturnus`
    installs the CLI.
  - Real renames, not `package =` aliases.
  - Linux packages: the CLI's .deb and .rpm are `saturnus`. The desktop app's
    are `saturnus-app` (a Linux-only `productName`); macOS and Windows keep
    "saturnus". satx's are `satx`.
- **One version for the whole workspace: 0.2.0 next.** Two binaries,
  `saturnus` and `satx`, come from one tag.
  - Each is published to crates.io, Homebrew and Scoop. The Scoop bucket was
    made public on 2026-10-10.
  - There is no winget and no new Cloudsmith repository.
  - This takes release-workflows v0.3.0, which lets two callers share one run
    and waits on crates.io's rate limit.
- **No backwards compatibility.** The owner is the only user. That means no
  redirects, no stub or alias crates, no Homebrew renames, no "moved to"
  notices, no fallbacks for old environment variables, and no migrations.
  Retired names simply disappear.
- **The old tool name appears nowhere afterwards, under variant A.**
  - The current files of every repository the owner maintains, and the
    imported history (contents, messages, paths), are free of it.
  - This repository's own older history is not rewritten, and there is no
    force-push.
  - No other repository's history is rewritten, and there is no GitHub
    Support purge.
  - The accepted exceptions are listed in [[iterations/iteration-36c-release]].
- **Commit addresses.** A mailmap gives every imported commit the author and
  committer `James Bergamin <james@ractive.ch>`. Every new commit's author
  email is james@ractive.ch:
  - a CI job checks it on every pull request;
  - the GitHub account's primary email is set to it, so merge commits carry
    it too.
- **crates.io: all seven published crates are deleted and published again**
  (saturnus-cli, saturnus-drive, saturnus-host, saturnus-objects, saturnus,
  xmodem-proto, kermit-proto, in that order).
  - Why: the 0.1.x proto crates point at the old repository, and the 0.1.0
    sources of saturnus-objects and saturnus-cli name it.
  - `saturnus-core` 0.2.0 goes out first. 24 hours after the deletions,
    everything else is published, with `saturnus` as soon as its dependencies
    are up.
  - The owner accepts the risk that someone takes a name, `saturnus`
    included, in that window.
- **The site on ractive.ch** ([[iterations/iteration-36d-site]]).
  - `/saturnus/` becomes a static landing page, and the emulator moves to
    `/saturnus/emulator/`. saturnus tx will be at `/saturnus/transfer/`; PR 103
    said `/saturnus-tx/`.
  - No redirects.
  - A kill-switch service worker at `/saturnus/sw.js` removes the old
    worker's caches and registration on the devices that had it. It does
    nothing else, and it is removed on 2027-01-15.
  - The desktop app does not change.
- **Iteration numbers.** saturnus tx (PR 103) moves to iterations 37 to 40.

## 2026-10-09 (macOS signing and the desktop app's cask)

Owner decisions on Developer ID signing (PR 93; release-workflows PR 3,
`docs/macos-signing.md`):

1. **No Developer ID Installer certificate and no `.pkg`.** The CLI ships
   as archives, the app as a `.dmg`; only the Application certificate is
   needed.
2. **Dry runs sign only with `macos-sign-dry-run: true`.** A dry run builds
   an unreviewed ref and would otherwise get the signing key and spend a
   notarisation; the input is set once by hand to test the secrets.
3. **No Intel build of the desktop app.** It stays Apple silicon only
   (`aarch64`), so the cask says `depends_on arch: :arm64`.
4. **release-workflows keeps no Apple secrets**, so its selftest goes on
   checking the unsigned path.
5. **A Homebrew cask `saturnus-app`** in `ractive/homebrew-tap`, kept by
   `desktop.yml`'s `cask` job from `packaging/homebrew/saturnus-app.rb`,
   and only for a `.dmg` that passes Gatekeeper (Developer ID, notarised,
   stapled) on a published, non-pre-release release. The token is not
   `saturnus`, the CLI formula's name in the same tap.

## 2026-10-10 (release-workflows v0.2.4: the CLI's macOS binary signed)

- `release.yml` and `publish-crates.yml` pin
  `ractive/release-workflows@v0.2.4`. With the `APPLE_*` repository
  secrets (set), it signs the `aarch64-apple-darwin` binary with a
  Developer ID certificate (hardened runtime, secure timestamp) and
  notarises it before the archive, so the checksums, SBOM, attestation
  and Homebrew formula describe the signed binary. `secrets: inherit`
  already passes them; no other caller change.
- Dry runs stay unsigned (`macos-sign-dry-run` left off): a dispatch
  builds an unreviewed ref, which should not get the signing key.
- Re-read for the musl swap (kb/docs/releasing.md): the job ids (`build`,
  `linux-packages`) and the `BIN_PATH` each exports are unchanged, and
  `pre-package-command` runs before signing, so it never touches a
  signed binary. `publish-crates.yml` did not change since v0.2.2.
- This is the CLI only; the desktop app's `.dmg` is signed by
  `desktop.yml` (2026-10-09, macOS signing and the desktop app's cask).

## 2026-10-10 (an off button changes its form, not only its colour)

- **Owner**: off buttons in light mode were not recognisable as off; an
  off Edit read as a normal button with lighter text. Text colour was
  the only difference (fill, border and shadow were the enabled ones).
- An off button now sinks into its surface: no fill (transparent), a
  solid, very light border (`--border-disabled`, just apart from the
  surface and fainter than the enabled border in both themes), no
  shadow, no hover or press, text and icon in `--ink-disabled`
  (3.6:1 on the panel and 3.9:1 on paper light, 4.5:1 and 5:1 dark;
  no off button sits on the desk, where light would give 2.9:1).
- **Owner**, after the first contact sheet: no dashed border, it is not
  a well recognised pattern. The border's style is its own token
  (`--border-disabled-style: solid`), so the choice is one line.
- One rule block in `web/style.css` for every kind: plain, primary
  (gives up its accent), icon, toolbar tool, segment and menu item,
  `:disabled` and `aria-disabled="true"` alike. Off segments keep only
  their divider, in the light border; the group's own border stays. A
  flag's lamp and bit cells are left out: off while a write runs, they
  still show whether the flag is set. Focus rings stay on the focusable
  aria-disabled buttons. This supersedes "the background stays the
  control's" of the 2026-10-09 entry on disabled controls.

## 2026-10-09 (iteration 35: a restartable tour)

- Owner: "It should not explode and overwhelm the user, but a
  (restartable) tour through the app would be amazing." Built as coach
  marks: one element ringed at a time, a bubble of one or two sentences
  with Back, Next, Skip tour and "3 of 7"; Esc ends it. No backdrop and
  no blocked page: the ring takes no pointer and the bubble is a
  non-modal `<dialog>` (`show()`), so the calculator leaves the keys
  typed in it alone and a modal opened over it (Search) keeps its own
  Esc. On a phone the bubble is a sheet at the bottom, and the panel
  gets room at its end so its last element can scroll out from under it.
- Never forced: one line on a first visit ("New here? Take a 2-minute
  tour."), on the stage's top-left over the calculator's case, where it
  covers no key; × or taking the tour is kept (`saturnus.tour`).
  "Show me around" is an action in Search and a button in About; each
  chapter is a Search action of its own ("Tour: The calculator"), found
  by a search only, so the list before typing grows by one row.
- The steps are data (`web/tour.js`): an anchor selector, words with
  `{key:id}` for a binding's label, a setup naming only what to open
  (`panel`, `roms`, `calculator`, `layer:<tab>`, carried out by app.js),
  and conditions (host, pointer, phone, Mac). Variants for touch and the
  phone are steps of their own rather than branches in the words. The
  tour shows and never changes the calculator: the page test fails if
  anything but a read reaches the host during a whole tour.
- A step whose anchor is missing, hidden or covered (the hit test at its
  middle) is skipped and leaves the count rather than being shown
  floating. Against UI drift a page test opens every applicable step of
  every chapter in light and dark, at 1280 and 390 px, in the browser
  and a stubbed app (the ROM steps differ), and fails on an anchor not
  seen. Anchors are the page's stable ids; the Download link got
  `data-tour="rom-download"`, the one element with neither.
- The memory view's chapter is offered only while a model with one
  runs; the calculator's chapter runs without a ROM too (the skin is
  drawn). The offer appears on the first visit, not after the first
  boot: a first visit has no ROM, and the first chapter is the one that
  helps there.

## 2026-10-09 (the screen to a variable or the stack, as a GROB)

- Owner approved: Screen to a variable… and Screen to stack beside Copy
  screen and Save screen (the panel, the palette, the display's menu).
  The screen is the display's 131 × 64 pixels without annunciators, the
  GROB the ROM's LCD→ makes of the same screen (ROM-gated test on the
  48SX, 48GX and 49G: LCD→ on a menu key against the stored GROB; equal
  on all three, and the 49G's LCD→ is 131 × 64 too). The ROM's LCD→ is
  not run: it would leave the command line typed and the screen changed
  by the typing; the palette's LCD→ stays the ROM's.
- **The GROB is built on the host and sent through the existing hidden
  Kermit write** (`storeScreen`, `saturnus-host`'s transfer): the
  encoder is `Graphic::to_grob` in saturnus-objects, the inverse of the
  decoder, with round-trip tests. It travels in binary under a free
  temporary name (`SATSCRN`), recalled and purged first in one host
  command (`'T' RCL 'T' PURGE`, then `'name' STO` for a variable), so a
  failure after the send leaves nothing in the user memory and the
  baseline drops what the command left on the stack.
- **To the stack is clean**: the recall leaves the GROB on level 1, the
  only write that changes the stack. The 49G in algebraic mode drops the
  echoes of its typed mode switches down to the depth it started with;
  a counted push (`Step::Push`) raises that depth, so the GROB stays.
- **The screen is the one shown when asked for**: the page sends the
  frame it showed (`pixels`, the frame's packing), so typing the name
  takes no other screen; without `pixels` (the CLI, `ctl screen
  --to-var` and `--to-stack`) the host takes the display at once.
- **The name** is asked in the memory view's own field (New
  directory's, opened in the calculator's current directory); a taken
  name asks "Replace X?" in the shared modal, a directory's is refused.
  The host refuses a taken name without `replace` too.
- **No default keys**: none is obviously free beside Alt+Shift+C and S;
  the shortcuts dialog can give them one (`keys: []`).

## 2026-10-09 (iteration 34: saved states, the same on both hosts)

- Owner: Save state and Load state behave the same in the browser and
  the desktop app. Save state writes the model's slot at once (IndexedDB
  as before; `states/<model>.state` in the app's data folder, beside the
  auto slot) and says which save it replaced; Load state asks in the
  shared modal, naming the save's date. The app's file dialogs move to
  "Save state to a file…" and "Load state from a file…" in a "⋯" beside
  the two buttons, which the browser now has too (a download and a file
  input).
- One file format: the core's state bytes, which the CLI's `--load` and
  `--save` already used, now as `.sat` (`<model>-YYYY-MM-DD-HHMM.sat`);
  the app's open dialog still takes its older `.state` files. A
  ROM-gated page test saves a file in the page, loads it in the CLI and
  loads the CLI's back.
- The page reads a state's header (magic, model) before it asks: a file
  that is not a state and a state of another model are refused in plain
  words. Offering to switch to that model is not done: the switch only
  helps when that model's ROM is the one the state was saved with, which
  only the load can tell. Another ROM of the same model stays the core's
  refusal, said plainly.
- The page names no path in the app: a slot is `slot: true` with the
  model (the host picks the file, and the runner refuses a save for a
  model that is not running), a file is `name` (offered in the dialog)
  or `openState` (the bytes come to the page, which asks before sending
  them back). Bare `saveState` and `loadState` now carry bytes on every
  host.
- Nothing to migrate: the browser's slot keeps its key and record; the
  app had no slot.

## 2026-10-10 (saturnus-tx: a transfer app for real calculators)

Owner decisions; the design is [[docs/saturnus-tx]], the plan iterations
37 to 40.

- **At <https://ractive.ch/saturnus/transfer/>**, beside the emulator
  (consolidation entry above).
- **A new app, saturnus-tx**, in this repository: a modern successor of
  the owner's HPComm (1999), the calculator's tree beside the computer's
  files, drag and drop. No "HP" in the product name.
- **Web first** with Web Serial (Chrome, Edge), then a **Tauri desktop
  app** for macOS, Windows and Linux with native serial, built from the
  same web code, with the same CI/CD as saturnus: deploy to ractive.ch,
  releases, signing, a Homebrew cask.
- **`satx-core` is the protocol core**, the library in this repository
  behind `satx`, compiled to WebAssembly, with its native-only parts
  (serial ports, the in-process emulator) behind features the page turns
  off.
- **No web framework**: saturnus's vanilla web components, menus,
  confirmations, toasts, notes, theme, tokens and GROB rendering
  (through `saturnus-objects`) are reused.
- **Every saturnus model with a serial port**: 48SX, 48GX, 49G (Kermit,
  XModem), 38G, 39G, 40G (Kermit and XModem aplets). The
  42S has no serial port and is not offered.
- **saturnus as a device** in the same page: for moving files between an
  emulated and a real calculator, and as the test double (no hardware
  in CI).
- **No ROMs in CI**: fetching them there is over-engineered. CI replays
  transfers recorded from the emulator; the run against the emulator
  itself is local, where the ROMs are.
- **HPGComm's source is a documented exception**, as Conn4x is in
  calculator-knowledgebase's rules (it is mainly the owner's own code):
  facts only, no code, no routine names. It may be read for the 38G/39G
  directory file.
- **No XSERV for now**: Kermit works on every supported model, and XSERV
  belongs to the 49g+/50g. It can come back if the owner ever has one.
- The owner has the 38G's 10-pin cable, so the 38G is tested on
  hardware.

## 2026-10-11 (0.2.0: the old crates stay on crates.io)

The owner dropped iteration 36c's deletion of the seven old crates. Five
of them (`saturnus`, `saturnus-cli`, `saturnus-drive`, `saturnus-host`,
`saturnus-objects`, all 0.1.0) already named this repository; only
`kermit-proto` and `xmodem-proto` 0.1.x link to the old transfer
repository, and those two links stop resolving once it is deleted.
Deleting them would have taken the five others with them (crates.io
refuses a crate others depend on), a day without the names and the risk
of someone taking `saturnus`.

- All eight crates went out at 0.2.0 in one `publish-crates.yml` run on
  2026-10-11, without a rate-limit wait (only `satx-core` and `satx` were
  new names).
- `saturnus` changed from the core library (0.1.0) to the command line
  (0.2.0); `saturnus-cli` gets no new versions. The owner yanked and then
  deleted it on 2026-10-11 (one owner, 10 downloads, no dependents).
- The 0.1.x versions of `kermit-proto` and `xmodem-proto` are an accepted
  exception to "no old name anywhere", next to the others in
  [[iterations/iteration-36c-release]].

## 2026-10-11 (the transfer app's names, and Edit on the interactive stack)

Owner decisions:

- **Two names, two things.** `satx` stays the command line and its
  library (crates `satx` and `satx-core`, the packages, `SATX_PORT`): short
  to type, as `saturnus` is, and already published. The web and desktop
  transfer app is **saturnus tx**: that name in menus and window titles,
  bundle identifier `ch.ractive.saturnus-tx`, Homebrew cask and Linux
  packages `saturnus-tx`. The apps share the `saturnus-*` family
  (`saturnus-app`, `saturnus-tx`); the short command line `satx` is kept
  for typing. Renaming the command line to `saturnus-tx`
  was rejected: a longer command and another round of crates and packages
  for no gain. This answers open question 2 of [[docs/saturnus-tx]].
- **No Edit while only the calculator's interactive stack is open** (▲,
  no command line). The page cannot tell reliably which level is
  selected, and the calculator's own VIEW edits it. The emulator page's
  Edit button (`#bar-edit` in `web/app.js`) and Cmd/Ctrl+E stay as they
  are; nothing to build.

## 2026-10-11 (satx-core goes sans-I/O)

Owner decision, answering open question 1 of [[docs/saturnus-tx]]: the
transfer web app needs satx-core in the browser, where Web Serial only
reads and writes asynchronously and WebAssembly cannot block the page.
satx-core's session layer (sessions, list, get, put, backup, server and
mode handling) becomes a **sans-I/O** state machine: it is fed the bytes
that arrived or a timeout, and answers with bytes to send, a wait or done.
`satx` drives it with blocking reads through a thin wrapper and behaves
as before; the page drives it with `await`. No runtime or async crate
comes in (only `web-time` on wasm32, already in the tree through
kermit-proto); serialport and TCP go behind the default feature `native`,
the in-process emulator behind the opt-in feature `saturnus`, which
`satx` turns on; the page builds without both. The draft still needs a
rebase onto main (it was branched before the import).

It is the first task of [[iterations/iteration-37-tx-skeleton]], starting
from draft PR 108. Rejected: `async fn` without a runtime (the style
spreads through every caller, a timer per platform), the blocking core in
a Web Worker over `Atomics.wait` (cross-origin isolation headers and a
bridge), the session logic again in JavaScript (a second implementation
the CLI's e2e suite does not test), and the desktop app alone (it would
drop the install-free web app).

## 2026-10-11 (iteration 37: the transfer page)

Choices the design left open, made while building
[[iterations/iteration-37-tx-skeleton]]'s page ([[docs/saturnus-tx]]):

- **The icon sprite is one file fetched once** (`web/shared/icons.svg`,
  `loadIcons` in `shared/components/icons.js`), not inlined at build
  time: the pages served from `web/` (development, tests, `cargo tauri
  dev`) stay the pages that ship. The emulator's `index.html` lost its
  inline sprite.
- **The page holds the serial port; the protocol runs in a dedicated
  Worker** and the page relays bytes by messages: the speed search
  reopens the port, which neither a posted port nor transferred streams
  allow. A hidden tab was measured not to slow a transfer
  ([[docs/saturnus-tx]], "Hidden-tab measurement").
- **The emulated calculator keeps nothing**: its own Worker
  (`web/tx/emulator-worker.js`) over the emulator's wasm package with
  auto-save off (`Host.setAutoSave`, new in saturnus-web), booted cold
  from a kept ROM (the emulator's IndexedDB, read only, opened without a
  version) or a ROM file, SERVER typed. It never writes the emulator's
  storage and never restores the emulator's calculator.
- **Previews fetch by kind**: programs, algebraics and units as the
  calculator's text (one text transfer; no ROM in the page), other
  objects as the binary file decoded by saturnus-objects; libraries,
  backups and code not at all; above 2 KB only on a click.
- **The page test replays a recorded page session**
  (`web/test/fixtures/tx-page.trace`, `just tx-page-trace DIR`) with a
  fixed device seed; a FakeLink checks every byte the page writes.
- **The transfer page reuses the saturnus mark** for its icons and
  manifest (no HP logo); a mark of its own can come with the desktop app.

## 2026-10-11 (iteration 38: saturnus-tx's operations)

Choices made while building [[iterations/iteration-38-tx-two-panes]]'s
device operations in `crates/saturnus-tx` ([[docs/saturnus-tx]]):

- **What replaces or removes is its own request.** A put or tree put to
  a name in use fails (`NameClash`, `NamesInUse` with every path) until
  sent again with `replace`, which keeps PR 123's pattern (an unused
  `SATXPUTn` first, swapped in once stored). A directory is never
  replaced or merged into by a new directory; a tree put into an existing
  directory (`name: None`) enters the directories it shares. A purge of a
  directory that holds variables fails with `NotEmpty` and their count,
  every level counted; only a purge naming that count sends `PGDIR`.
- **Restore leaves the device "restarted"**: no connection, every request
  but connect refused (`Restarted`, "the calculator restarted; start
  SERVER on it and reconnect"); the next connect purges the `:0:SATXRS`
  that RESTORE leaves in port 0, and until that purge succeeds it is
  tried again before each request. RESTORE keeps the calculator's own
  flags (a backup made with -35 clear, restored onto a calculator with it
  set, left it set), so the restore puts flag -35 back to the user's
  setting just before RESTORE (satx-core's `Op::RestoreLeaving`), as a
  disconnect would.
- **The backup's memory check is twice HOME plus 1024 bytes free.**
  Measured on the emulated 48SX, 48GX and 49G: ARCHIVE takes HOME's size
  plus about 80 bytes, storing the recalled copy as much again, a peak of
  twice HOME and about 175 bytes. HOME's size is the sum of its `G D`
  sizes; free memory the `G D` header (48GX, 49G) or `MEM` (48SX).
- **Across families a copy goes as text.** A binary file with the other
  family's header is stored as a string (wiki `protocols/hp-object-format`;
  the emulated 48GX to 49G copy shows it), and a program's binary form
  holds its own ROM's addresses; so the copy plan warns and gets each
  variable's text form, which the target compiles. A folder's file of the
  other family gets the warning that it will arrive as a string. A 48G/GX
  to a 48S/SX also goes as text, with its own warning (the S series lacks
  the G series' commands, which a binary program calls by address). Kinds
  without a text form a calculator compiles (libraries, backups, code,
  XLIB names, ...; and the 49G's integers and symbolic matrices going to
  a 48) are left out: listed as `untranslatable` in the plan, `skipped`
  inside a directory got for the copy. The plan tells variables in use
  (replace asks) from names where a directory is on either side
  (`blocked`: purge first).
- **A tree put that fails part of the way says what it left**
  (`Partial`: directories made, variables stored, whether its new
  directory exists); a replace whose swap fails removes its `SATXPUTn`.
- **A tree put keeps the calculator's order**: a new variable goes first
  in its directory, so each directory's children are put in reverse; a
  directory got, purged and put back is byte for byte the same, order
  included.
- **The 49G's ops trace stops before its backup**: every 49G backup ends
  in a list of ROM objects that is a 20-byte run of the ROM, which the
  recorder's check refuses. `just tx-traces` still runs the 49G's backup
  and restore live (the restored HOME compared); the 48SX's and 48GX's
  traces hold theirs.

## 2026-10-11 (iteration 38: the two panes)

Choices made while building [[iterations/iteration-38-tx-two-panes]]'s
page ([[docs/saturnus-tx]], "Layout"):

- **One queue for the page, not one per calculator.** Every operation
  (a copy, a rename, a backup) is a job on the places it uses
  (`web/tx/queue.js`); a copy holds both of its places, so a folder's
  write never runs ahead of the get it needs, and each place does its
  jobs in the order asked. The list under the panes shows them with
  their progress in bytes; the waiting ones are cancellable. Previews and
  listings stay outside it, on the calculator's own line, so a long copy
  does not freeze browsing.
- **A copy asks once, before anything is sent.** The plan
  (`web/tx/transfers.js`, saturnus-tx's `copy_plan` between two
  calculators) collects everything to warn about (strings, the other
  family, no text form, a directory in the way) into one question, then
  one "Replace X?" for what is in use. A Cancel stops the whole copy, not
  only the replace: a half-done copy the user did not see coming is
  worse than none. A folder merges into a folder of the same name, its
  files in use asked about; a directory is never replaced.
- **A purge's count comes from the listings before the question**, not
  from a purge sent to find it out: a purge without the count removes an
  empty directory at once, which would be a removal without a question.
- **The folder's permission is asked up front**, when it is picked, and
  on a later visit by "Allow" before anything is listed, not at the
  first write.
- **The preview sits under each pane's list**, and a narrow pane shows a
  path line instead of the tree (a container query): two panes side by
  side leave no room for a third column.
- **The page test's fake calculator replaces the protocol Worker**
  (`web/test/tx-fake-calc.js`), not the link: the protocol is tested
  against the recorded traces already (tx-driver, tx-bindings), and the
  page's own flows (drags, questions, the queue, restore) need a
  calculator whose state can be read back.

## 2026-10-11 (iteration 39: aplets for the 38G, 39G and 40G)

- **The page makes the directory files; the calculator never has to
  initialise.** `HP38DIR.CUR` and the directory file it names are
  answered from the folder (the calculator's last copy, its entries
  whose files remain, then the folder's other files of the family with
  a calculator header and an 8.3 name). HPGComm instead refused a
  missing `HP38DIR.CUR` and let the calculator write both after "Initialize
  directory?"; that works too (wiki `protocols/aplet-transfer`), but then
  a folder of aplets from anywhere is invisible until the calculator has
  written a list naming them. The copies the calculator writes are still
  kept in the folder, so a folder stays usable by HPGComm.
- **Received files replace their namesake without a question here**:
  the calculator already asked "Replace existing aplet?" (it reads the
  directory file first), and a transfer it drives cannot wait for a
  dialog on the computer (its timeout is 10 s).
- **The Kermit server is its own module of `kermit-proto`** (`Server`,
  beside `Client`), sharing the codec, prefixing and negotiation; the
  aplet logic (directory files, the folder, XModem) is `saturnus-tx`'s
  `aplets`, which is no published crate, so `satx` gains only the models
  in `Model` (`Hp38G`, `Hp39G`; the 39G and 40G run one ROM and are not
  told apart), not a serve command.
- **Traces carry list L1, not an aplet.** A new aplet is a copy of one in
  ROM (the 38G's Function aplet is in its ROM byte for byte), and
  committed traces hold no ROM bytes. The recorder checks against the
  ROM file and its packed image, and ignores runs of one or two byte
  values (the XModem padding). The e2e test uses the built-in aplets,
  whose bytes stay in memory.
- **The wire send offers only binary aplets** (`HP39BinB`) and pads with
  NUL, as the 39G's own sender does: anything else ends in "Transfer
  Failed" on the calculator after every block was taken, which the page
  cannot see. It waits 2 minutes for the calculator's RECV.
- **The serve screen covers the panes** rather than being a third kind
  of place in a pane: the calculator drives, there is nothing to browse
  or drag, and HPGComm's one small window is the shape that fits. An
  emulated 38G or 39G shows its display and only the keys a transfer
  needs.

## 2026-10-11 (iteration 39: the fast transfer)

Choices made while building the XModem part of
[[iterations/iteration-39-tx-aplets]] ([[docs/saturnus-tx]], "Fast
transfer"):

- **One request per fast transfer** (`Request::XmodemPut`,
  `Request::XmodemGet` in `saturnus-tx`'s `xmodem` module): the checks,
  the RPN switch and FINISH, an `Event::Xmodem` with what to type, the
  transfer, all one job of the page's queue. satx-core's `PrepareXmodem`
  is not used (review of PR 139): it lists the directory the checks just
  listed and asks VERSION again, and if FINISH's answer is lost it says
  nothing of the RPN switch it made. saturnus-tx switches with one host
  command that says whether it did, records the server as ended before
  FINISH is sent (a lost answer may still have ended it), and refuses a
  directory for a fast get. The device then
  stays out of server mode (`Device::left_server`, every request but a
  connect is `LeftServer`) until the user starts `SERVER` and the page
  reconnects, as after a restore. The XModem session code is satx-core's
  `Op::XmodemSend`/`Op::XmodemReceive`; `saturnus_tx::xmodem::transfer`
  wraps it for any XModem peer, so the 39G/40G aplet send can reuse it.
- **A name in use is refused, not replaced**: the 48GX's XRECV stops with
  "Name Conflict", the 49G's stores `NAME.1` (wiki
  `protocols/xmodem-hp`), and a swap through a temporary name would need
  the reconnect the user may never make. The page says to purge it or
  use the normal copy, which replaces.
- **Binary files of the model's family only**: XModem on the calculators
  is binary; a text file or a file of the other family goes by the
  normal copy, which converts it.
- **The reconnect keeps the user's transfer mode** (flag -35 as read at
  the first connect, not the binary mode the page set) and puts a 49G
  that was switched to RPN for the command back in algebraic mode
  (`-95 SF` after the reconnect, as satx does; checked on the emulated
  49G).
- **The page waits three minutes** for the command (satx-core clamps to
  ten): typing `'NAME' XRECV` on a calculator's keyboard takes a while.
  Cancel stops the wait; the calculator side is stopped with ON.
- **An emulated calculator gets "Type it for me"** (the emulator's `run`),
  which is also how `just tx-e2e` drives XRECV and XSEND.
- **The traces of the fast transfer are the page's** (`just
  tx-xmodem-traces DIR`, a RecordingLink with each request as a note),
  replayed in `crates/saturnus-tx-web/tests/xmodem.rs` with the page's
  own request parser. The 49G's XSEND pads its last block with memory
  that held a run of the ROM: the recorder zeroes the padding and
  recomputes the block's CRC before writing the trace. The page's trace
  writer now escapes a space that ends a line (`\x20`), as kermit-proto's
  does: the Rust reader trims line ends.

## 2026-10-11 (the service worker checks every file it caches)

- **The installed pages' service worker verifies each file's SHA-256
  before it caches a build** (`web/pwa/sw.js`; `web/site.sh` writes
  FILES as `[path, hash]` pairs). Cause: the transfer page showed `No
  request "pict"`, new modules against the previous wasm. The deploy
  (SamKirkland/FTP-Deploy-Action, through ftp-deploy's `HashDiff`) sorts
  its upload and replace lists by path and is not atomic, so
  `transfer/sw.js` goes up before `transfer/tx/...`; a browser that
  checked for an update in that window installed the new BUILD with
  `cache.addAll` over the files the server still had, a mixed build
  kept under the new name until the next deploy. Now a file whose body
  does not match (or does not come) fails the install, nothing is
  cached, the browser keeps the worker it has and tries again on a
  later visit. This holds for any upload order, a truncated file
  mid-upload included.
- **No second deploy pass with `sw.js` last.** Two FTP-Deploy-Action
  steps, the first with `exclude: **/sw.js`, share the sync-state file:
  the first writes a state without any `sw.js`, so the second sees them
  as new and uploads them every time, and can never delete one (the
  kill switch's removal, kb/backlog/remove-kill-switch.md, relies on
  that delete). Fixing that needs a second `state-name` and negated
  exclude patterns, more machinery than the order is worth once the
  worker refuses a mixed build. A page loaded from the network during a
  deploy can still mix files once; the transfer page then says "This
  page's files are from different versions. Reload the page." for a
  request its wasm does not know.
- **The test browsers download into their throwaway profiles**
  (`Browser.setDownloadBehavior` in `web/test/chrome.mjs` and
  `firstminutes.test.mjs`, `user.js` prefs for Firefox): the transfer
  page's tests had left dozens of `PRG (n).hp` in the owner's Downloads.

## 2026-10-11 (folders only in the right pane)

Owner decision for [[docs/saturnus-tx]]'s two panes:

- **A folder on the computer opens only in the right pane.** The left
  pane offers calculators only: one on the cable (Web Serial) or an
  emulated one. The right pane offers a folder (or, where the browser
  cannot pick one, the page's own files) and a calculator too, so a copy
  between two calculators (an emulated 49G to the real one) still works.
  The "Change" menu, the start cards and the kept folder of an earlier
  visit follow it: no path puts a folder on the left (`web/tx/pane.js`,
  `folders`). One side for folders keeps the page's direction fixed,
  calculator left and computer right, as the defaults already were.

## 2026-10-12 (iteration 40: saturnus tx's desktop app)

For [[iterations/iteration-40-tx-desktop]], on the owner's decisions of
2026-10-11 (names; the app's right pane is the real file system):

- **The right pane is the whole computer, opening at the home folder.**
  The folder is one `Folder` (kind "native", `web/tx/tauri.js`) whose top
  is the root (`/`; on Windows a list of the drives), shown first at the
  home folder, so the tree and the crumbs go up and down anywhere; "Open
  a folder…" (Tauri's dialog) jumps there in the same place. The aplet
  server's "Open a folder…" gets a folder of its own (kind "picked").
  The native side is reached through handles of the File System Access
  API's shape, so `folder.js`'s checks (portable names, case collisions,
  never replacing without asking) are the same as in the browser;
  renames are done in place where the handle can (`renameEntry`).
- **Scope: no confinement to one folder**, which the owner's "anywhere
  the user can" rules out. Tight where it can be: no Tauri plugin but
  the dialog, opened from Rust only; the capability grants nothing (no
  core events, no dialog, fs, shell or opener permissions); serial bytes
  come on a `Channel` the page passes to `serial_open` (ordered, scoped
  to the open, no event permission) rather than the plan's event; every
  path is a list of plain names checked natively (no `..`, separators,
  NUL, or `:` but the drive on Windows); only plain files and folders
  are listed or read (no devices, pipes, broken links); hidden files are
  not listed; a read is capped at 256 MB; writes go to a temporary file
  renamed over the target; the page is the embedded files only, under a
  strict CSP. A script injected into the page could still reach the
  user's files: the CSP and the embedded-only page are the guard.
- **The protocol and its wasm in the app**: both wasm packages are
  embedded, built by `beforeBuildCommand`; `build.rs` fails a production
  build without them (tauri's `DEP_TAURI_DEV`), warns otherwise (CI's
  clippy builds no wasm).
- **Downloads** go through a native save dialog (`save_file`): a
  webview's blob downloads do not reliably reach the disk.
- **Ports**: `serialport` without libudev (sysfs gives the USB names on
  Linux; no libudev-dev to build); macOS's `/dev/tty.*` twin of each
  `/dev/cu.*` port is left out (its open waits for the carrier). The
  port chooser is the app's own dialog, as the browser's chooser in the
  same place of the flow; the last port is kept under
  `saturnus.transfer.port`. Ports close when the page (re)loads.
- **Assets**: the installers' "saturnus tx" becomes `saturnus-tx` in
  their file names (`desktop.yml` renames them when collected); the
  casks are rendered one after the other.
- **The pseudo-terminal test** opens the slave end by name on Linux; on
  macOS, whose ptys refuse the speed's ioctl, the open end is handed to
  the same port code (`Ports::adopt`).

## 2026-10-12 (iteration 41: shared calculators)

- **Only a kept ROM is shared.** saturnus tx's emulated calculator from
  a ROM the emulator keeps is the emulator's calculator of that model; a
  ROM file chosen in the dialog boots cold and keeps nothing, as before.
  Sharing it would overwrite the emulator's state with a calculator of
  another ROM.
- **Fencing, not just the lock.** Besides the Web Lock, the Worker that
  gets a calculator writes a holder record (`holder:<model>`, a random
  token) and every auto-save checks it in the write's own transaction.
  A frozen tab whose lock was stolen may wake with a write already
  queued; the record refuses it however the browser orders the wake-up.
- **Who asks.** saturnus tx asks for the calculator when the user picks
  it; the emulator when the user chooses the model or presses Take it
  back. The emulator's own boot of the last model when it opens does not
  take it from saturnus tx (`bootModel` with `ask: false`): opening a tab
  must not end a transfer in another one.
- **Timeouts**: 3 s for any answer (the holder answers at once, even
  while it wraps up), then a minute for the release (saturnus tx lets a
  running copy end and leaves SERVER first); after either, `steal`.
- **The format version** is the core's own (the u16 after the state's
  magic), exported to the pages as `state_format()`; only a newer one is
  refused ("Reload this page to use it."). An older or broken state boots
  cold, as since iteration 27.
- **Leaving SERVER happens on a handover and on Turn it off.** A
  saturnus tx tab that is closed or killed cannot talk to the
  calculator any more: its last auto-save may be in SERVER. Not saving
  while in SERVER would lose the copies made instead; the emulator ends
  that server when it boots the state (2026-10-13 below).
- **No Web Locks** (an old browser): every calculator is the page's own,
  as before; the holder record still guards the writes of a page that
  boots after another.
- **Remove ROMs takes the ROM-holding calculators first** (the 49G, whose
  state holds its flash): the emulator asks saturnus tx for it and
  replaces the holder record, so saturnus tx cannot write the ROM back
  after the page deleted it.

## 2026-10-12 (one Kermit client and one character set)

For [[backlog/satx-fold-duplicates]]:

- **saturnus-kermit runs satx-core's operations**, driving its sans-I/O
  `Machine` with the link's emulated idle clock rather than a new
  emulated-time `Transport`: the `Machine` already takes the caller's
  clock, and a blocking transport would have needed satx-core's
  wall-clock `Session` loop changed. The link (turnaround, busy time not
  counted, the read cap) stays in saturnus-kermit; so does `semantic`,
  which needs `saturnus-objects`' typed objects.
- **An emulated link is a mode of the `Machine`**
  (`set_emulated_link`), not a set of `Options` fields: on an emulator on
  its idle clock a host command's `C` may be resent and no stale input is
  read before it (the host drops it without running the calculator).
  The read alone shifted emulated time by a millisecond per command,
  changed `TICKS` in the generated reference and left the 49G silent
  after `→DATE`; with the mode the reference regenerates byte for byte.
  A method keeps `Options` (a plain struct with public fields) unbroken.
- **The HP character set lives in `saturnus-objects`**, which satx-core
  now depends on (published before it, wasm-clean, only `saturnus-core`
  below it); a protocol crate was the other candidate, but neither
  Kermit nor XModem needs the table. satx-core's `charset` keeps its API
  as wrappers with its own error type, and its `StackReply`/`parse_stack`
  and name check are `saturnus-objects`' as well.

## 2026-10-13 (iteration 42: HP 27S, 17BII and 17B)

For [[iterations/iteration-42-pioneer-27s-17b]]:

- **Three models on the 42S's profile.** `Model::Hp27s`, `Hp17bii` and
  `Hp17b` share `HP42S`'s fixed Lewis map, the 131x16 LCD, the clock and
  cycle table; `Model::is_pioneer` replaces the `== Hp42s` checks in the
  hosts. State model codes 7, 8 and 9. Two new keyboard layouts on the
  42S's matrix (`Layout::Hp27`, `Layout::Hp17`, from KML 2.0); the six
  menu keys are `A`-`F` as on the 48.
- **`exit` is a key of its own.** The 27S and 17Bs have an EXIT matrix
  key besides the ON line's CLR, so `Key::Exit` ("exit") replaces the old
  name alias; on the 42S, whose ON key is labelled EXIT, `Key::Exit`
  presses ON and is left out of its key list, so 42S scripts keep
  working.
- **The 17B takes 96 KB too.** The plan assumed only the 17BII had an
  international ROM; the owner's 17B image is 96 KB, as the research
  showed (both have one). The 96 KB image maps from #00000 to #2FFFF;
  the main ROM finds the language ROM by its mark at #20000.
- **Both byte orders are read.** The owner's images store the ROM in 256
  units last first ("dump order"); his 42S image of 1999 is the same ROM
  in address order. The core reorders an image whose first unit holds
  "COPYRIGHT HP 198" (`pioneer_address_order`), for every Pioneer, so
  every host and `disasm` take either file; no conversion tool, no
  second loader.
- **Signatures are the self-test texts.** A 64 KB image fits four models
  and a 96 KB one two; the text each main ROM keeps for its self-test
  pass ("OK-42S-", "OK-27S-", "17B II-", "OK-17B-E") names it, at the
  ROM's end, documented in the manuals. An image with none (or two) is
  offered, as ambiguous images were.
- **The self-test acceptance is not met.** Every image's ROM step fails
  with the 48's CRC rule, the same way as the 42S's; four independent
  dumps make bad dumps unlikely, and no CRC-16 variant passes all four
  (wiki: questions/hp42s-rom-crc). The goldens record the step's printed
  value as found; the Lewis CRC rule stays open.
- **Annunciators by position.** The core reports the seven Lewis
  positions as on the 42S; the page draws six of them for these models
  (print as PRT, the alarm at the 42S's busy position, no G), an
  inference recorded in wiki: questions/pioneer-annunciator-mapping.

## 2026-10-13 (iteration 43a: the HP 28S machine)

For [[iterations/iteration-43a-hp28s-machine]]:

- **The two-Lewis map is fixed; no window logic is modelled.** A trace of
  the owner's 28S ROM (revision 2BB) on a logging bus through cold boot,
  arithmetic, menus, typing, printing (ON + L), System Halt, Memory Reset
  and 30 s of the self-test saw RESET at reset (once more after each
  halt or memory reset), no CONFIG, UNCNFG or C=ID, and no access outside ROM
  (#00000-#3FFFF), RAM (#C0000-#DFFFF) and the two chips' blocks; the
  ROM writes the master's registers at #FFF00 and the slave's at #FFB00
  in its first instructions after RESET. So the 28S runs on a second
  fixed map, as the 42S does on its own: `LewisBoard::Clamshell` beside
  `LewisBoard::Pioneer` in `HardwareProfile::lewis` (was a bool). Where
  the chips take their addresses from (straps or reset defaults) stays
  open (wiki: questions/lewis-memory-map).
- **The slave chip has its own timers.** The ROM arms the slave's TIMER2
  with WAKE and INT and rearms it every #E67 ticks (about 0.45 s): the
  command line's cursor blinks on it. The slave's timers run on the
  machine's clock, share the interrupt line and wake the CPU; its CRC
  and other registers are storage, its LPD reads 0. Inferred from the
  ROM's handler, which polls both chips' timer controls.
- **The display: slave left, master right.** The slave shows columns
  0-67 from #FF840, the master 68-136 from #FFC00, eight nibbles per
  32-pixel column (Gariepy's scroll routine; the ROM's text reads right
  in both halves). Each half follows its chip's DON. The seven
  annunciators are 5-nibble words in the slave's first #40 nibbles
  (busy, alpha, battery, shift, RAD, suspended, printer), each seen lit
  by the ROM; `Annunciators::suspended` is new, the printer is reported
  as `transmitting`.
- **`Lcd` rows are as wide as the display.** `Lcd::pixels` is
  `Vec<Vec<bool>>` with `Lcd::width()` (131, 137 on the 28S) instead of
  `Vec<[bool; 131]>`: a breaking change of the core's public API, which
  the iteration 15 entry anticipated (consumers read sizes from values).
  Every consumer takes the width from the `Lcd` (PNG and text dumps, the
  frame event, `storeScreen`, the run's `model` info via
  `Model::lcd_size`); the 131-column goldens are byte-identical.
- **A wider keyboard.** The 28S's matrix uses IN bits 0-11, so
  `KeyPos` masks and keyboard rows are u16; the state writes a row as a
  u16 only on the 28S, so other models' states are unchanged. The six
  menu keys are `A`-`F` as on every model (the cursor-menu INS, DEL and
  arrows printed above them are their default functions); the left
  half's letters are new keys `LetterA`-`LetterZ`.
- **Hosts wait for the skin.** `skins::skin` returns an `Option`; the
  pages' model lists (`hello`, the Worker's ROM slots, the desktop's
  slots) offer only models with a drawn skin (`skins::hosted_models`),
  so the 28S appears there with iteration 43b; `romid::plan` likewise
  treats an image of an unskinned model as unknown, so neither page
  stores or boots a 28S image it cannot draw (review finding). The CLI
  runs it now.
- **A 128 KB image must say "OK-28S".** The plan wanted an unsigned
  image offered; with the 28C, 19B and 19BII ROMs 128 KB as well and none
  of them runnable, an unsigned 128 KB image fits no model instead, until
  the 19BII (iteration 44) gives a second candidate. The owner's image
  is in dump order (512-byte units).
- **The Lewis CRC is its own.** The wiki found it from the ROM images
  (wiki: hardware/lewis "CRC", questions/hp42s-rom-crc): the remainder
  modulo x^16 + x^14 + x^13 + x^11 + 1, each nibble most significant bit
  first, from 0, fed by the same data reads as the 48's. The Lewis models
  use it (`IoRegisters::lewis_crc_update`); the 48 family keeps the
  Clarke's. Every Lewis self-test's ROM step now passes: "ROM OK" on the
  42S, 27S, 17B and 17BII (goldens updated), "OK-28S" on the 28S (with
  the 48's rule it got #185A and "8 FAIL", ST bit 11). The iteration 42
  entry's open self-test acceptance is closed by this.
- **The CRC tells the image orders apart.** A Lewis ROM's first 64 KB
  give #FFFF only in address order, so `pioneer_address_order` reorders
  an image when its CRC fails and the reordered one's passes; the
  "COPYRIGHT HP 198" text decides only when neither passes (a damaged
  dump). The 28S's image has no such text.

## 2026-10-13 (saturnus tx: the emulated calculator, faster and never left in SERVER)

- **The owner's error.** A 48GX saved in SERVER (saturnus tx's tab
  closed while it served, the iteration-41 limitation) was booted again
  by saturnus tx, which typed SERVER into a running server: its
  keyboard does not answer the typing engine's alpha key, so it failed
  with "could not bring the keyboard to AlphaLocked", and the failed
  start saved it back, still in SERVER, for the emulator.
- **Asked, in emulated time, by the engine.** `probeServer` and
  `endServer` (`Op::Server` in `saturnus-host`'s transfer) send one I
  packet over the emulated line and wait in emulated time, as the
  writes' probe after ON does; `endServer` then sends `G F` and waits
  for the stack. In the engine rather than through saturnus tx's Kermit
  client, so the emulator page has the same check and a wall-clock
  deadline never meets a busy page. A port that is off (IOC SON clear)
  or a command line open is no server: answered at once, nothing sent,
  so a calculator that never served costs nothing at boot (the port
  stays on after a server until the calculator is turned off).
- **Ended on both sides.** saturnus tx's Worker ends a running server on
  every handover, whatever the page did before; the emulator's Worker
  ends one in every kept state it boots, which covers the tab closed
  or frozen, which nothing in saturnus tx can.
- **Max speed for the emulated calculator in saturnus tx.** Measured on
  the 48GX: connect and list 10.8 s at 1x, 3.3 s at Max (with the
  shorter wait for a restored calculator to settle); a 16 KB string
  put 115 s at 1x, 4-7 s at Max. A server waiting for a packet sleeps,
  so Max costs nothing while idle. The fast transfer (XModem) runs at
  1x: at Max XSEND gave up before the page's first NAK.
- **The Kermit turnaround stays 200 ms** for an emulated calculator too.
  100 ms took connect and list from 3.3 s to 2.3 s (50 ms: 1.8 s), but
  the 48GX's folder copy of HOME then failed on the graphic every time
  (tx-e2e); at 200 ms it passes.
- **The line stays at the calculator's own speed.** The emulated UART
  run at 2x 9600 baud moved nothing (at Max the line already runs in
  emulated time while the CPU is awake); at 4x the 48GX never answered
  the connect. Dropped.

## 2026-10-13 (iteration 43b: the 28S's clamshell skin)

For [[iterations/iteration-43b-hp28s-skin-hosts]]:

- **The open clamshell, both halves, as on the desk.** The 28S is drawn
  open: the letter half on the left, the display half on the right, the
  hinge's barrel between them, one drawing. On a stage taller than wide
  (a phone, a portrait window) the page stacks the same data: the
  display half on top, the barrel across, the letter half below. Each
  half is a group with a transform in every layer of the SVG, so the
  key handlers, the press animation and the labels stay as they are for
  every other skin. Rejected: the keyboard half alone beside a separate
  display (not what the calculator looks like) and a closed case
  (nothing to press). The owner can overrule in the PR.
- **Stacked shows less of each half.** At 390 px the whole half (836
  units wide) made the 28S's keys 13% smaller than the 42S's; the
  stacked view cuts each half to its face (`stacked_display`: without
  the dark spine beside the keys; `stacked_letters`: the keyboard plate
  without the reference card above it), which gives keys as large as
  the 42S's and less to scroll. Side by side shows everything. The
  choice between the two is the stage's shape alone (taller than wide),
  not a breakpoint, so a narrow desktop window stacks too; stacked the
  page fills the width and scrolls, but never so large that the display
  half overflows the stage's height.
- **`Skin::fold`**, an `Option` like `glass`: the halves' rectangles,
  the barrel, and the stacked views. The case (the first panel) stays
  the whole drawing for the rules every skin keeps; the page draws it
  per half. The skin tests run per half where a rule is about a case
  (the margin under the keys) and check that every key, panel, mark and
  line lies in one half.
- **Letters straight to the letter keys.** `Typing::direct`: on the 28S
  a typed letter is its own key, without an alpha press; a lowercase
  letter is LC, the key, LC (LC toggles lowercase; there is no
  annunciator to read, so the queue assumes it off, as after a reset).
- **Measured on photographs, not the manual's figures.** The Owner's
  Manual's keyboard figures (p. 26-27) are line drawings on two pages,
  not to scale; they give the grid and the labels. The geometry comes
  from a square-on Commons photograph of the open calculator, each half
  from its own corner (the letter half was 2% nearer the lens), checked
  against a second. The cursor menu over the menu keys and ATTN are
  white on the light plate on the calculator; drawn dark to keep 4.5:1.
- **The reference card's examples are our own.** The card lists the
  object types and their delimiters (the 28S's syntax); the examples
  printed on the calculator are replaced by others.
- **No MCP host.** The plan's MCP items do not apply: saturnus-mcp was
  retired in iteration 18.

## 2026-10-13 (iteration 46: one live calculator per model in a SharedWorker)

- **SharedWorker instead of the handover** (owner: "Let's do the
  SharedWorker, yes!"). Iteration 41 shared a calculator by handing it
  between two Workers (Web Locks, a BroadcastChannel, holder records);
  the emulator and saturnus tx now talk to one running machine at once.
  No fallback to the handover: Chrome (desktop, and Android since 148),
  Firefox and Safari 16 have SharedWorker; Samsung Internet does not,
  and there the emulator runs alone in a plain Worker (the same hub, one
  page) and saturnus tx says it cannot share.
- **One URL for both pages.** A SharedWorker is one per script URL, so
  both pages open the emulator folder's `shared-worker.js` (the tx page
  finds it with `siteHref("emulator")`, as its links do). Its module
  graph and wasm are the emulator's, served by the emulator's service
  worker, whose scope its URL is in (worker scripts are matched by
  scope, not by the page's worker).
- **Who owns what.** The hub owns one engine per kept model (made on the
  first boot or attach, running while a page shows it) and one "home"
  engine per page (no ROM, or a ROM given once). It is the only writer
  of the auto-saved states, so the holder records and the write fencing
  went. With one page on an engine the page's commands pass through
  unchanged: the cross-host script test runs through the hub as before.
- **A page that is gone** is learnt from a Web Lock it holds for its
  life (the hub asks for the same lock), since a SharedWorker hears no
  close of a port; saturnus tx also says `leave`. A leaving holder of
  the serial line gets `serialClose` and `endServer`; the last page of
  a machine has it saved (`unload`) and stopped.
- **Arbitration only on the serial line.** Keys from both pages go in as
  they come (two hands). One holder of the line (saturnus tx); another
  page's `serialOpen` and the emulator's own memory writes (which use
  the line inside the engine) are refused with a sentence naming
  saturnus tx. The emulator user sees "Awaiting Server Cmd." while tx is
  connected; a key there may end the server, as on a real calculator,
  and tx's next request then fails as with a cable.
- **Speed: the emulator page's, tx's only while it transfers.** tx's
  Max (1x for the fast transfer) applies while it holds the line and
  bytes moved or it worked the calculator in the last 3 s, and always
  when no emulator page shows the machine. A waiting server sleeps, so
  between copies the user's setting is all that shows. The first
  version applied tx's speed only when "the line became busy" and missed
  a new holder while the line was still busy from the last one: the
  second connect then ran at 1x (9.4 s instead of 3 s). Re-applied on
  every byte now.
- **Visibility per machine**: hidden only when every page that shows it
  is; tx never reports hidden, so a transfer in a background tab runs.
- **Mixed builds.** `web/site.sh` writes one build id into both folders'
  `shared/build.js`: a hash of the SharedWorker and what it loads (the
  wasm package, `hub.js`, `romstore.js`, the shared modules), not of
  everything, so a change to one page's stylesheet does not make the
  other page's service worker fetch its build again. The hub refuses a
  page of another build with the existing "This page's files are from
  different versions. Reload the page." The state format is unchanged.
- **Measured** (48GX, headless Chrome, the emulator running it):
  saturnus tx connected and HOME listed in about 3.0-3.3 s, of which
  0.45 s is attaching, the probe and typing SERVER; the rest is the
  Kermit connect and the first listing.
- **Removed**: `web/shared/calc-lock.js`, the holder records, `away`,
  `ask` on `bootModel`, "In use by…", Take it back, tx's "…is elsewhere"
  card and `?take=`; "Open in the emulator" opens the emulator on the
  model (`?model=`) and asks nothing, since nothing stops.
- **Review fixes.** A running machine is shown to a page only for the
  same ROM bytes: another ROM chosen in the emulator saves the machine
  and boots it again on the new ROM (as the Worker did with `saveNow`).
  A fresh start that the engine would refuse (a send types) is checked
  before saturnus tx is dropped or the state deleted. tx's 1x for the
  fast transfer holds for as long as it has the line, not only while
  bytes move. A page leaving its own engine for a kept one unloads it.
  A page refused at join is forgotten at once. Sharing needs Web Locks
  as well as SharedWorker (a page over plain HTTP has none, and the hub
  could not tell it closed). The emulator page refused for another
  build runs in a plain Worker of its own instead of not at all.

## 2026-10-13 (iteration 44: the HP 19BII)

For [[iterations/iteration-44-hp19bii]]:

- **The 28S's map, RAM at #C4000.** A trace of the owner's 19BII image
  on a logging bus (cold boot, the language menu, arithmetic, TEXT,
  MODES, the clock and appointments, the self-test) touches the ROM
  at #00000-#3FFFF, the two chips' blocks at #FF800 and #FFC00 and RAM only
  at #C4000-#C7FFF; an object-walking routine (#02EA9) reads scattered
  words everywhere, data, not a probe. So `Model::Hp19bii` runs on
  `LewisBoard::Clamshell` unchanged, with 8 KB of RAM repeating through
  the 28S's window (offset #4000 is the RAM's start), which matches the
  manual's "approximately 6,600" free bytes. Model code 11.
- **137 columns, not 132.** The language menu's text runs across both
  chips' halves and the six menu labels fill columns 0-136 exactly, as on
  the 28S; the "132 x 32" on the wiki was wrong and is corrected there.
- **The KML 2.0 table, checked key by key.** Every OUT/IN pair was
  pressed on the ROM's MAIN menu; all match except INPUT, which the
  table prints as `6 128` (STO's code) and the ROM answers at `5 128`.
  New keys `ins`, `question`, `dollar`, `colon`; DISP and PRNT reuse the
  17B's `dsp` and `prt` (same functions), the letters the 28S's
  `lettera`-`letterz`.
- **The 28S's annunciator words.** Word 0 (the 28S's busy) is the
  19BII's appointment alarm, reported as `busy` as on the Pioneers;
  alpha, shift and RAD as on the 28S; battery and printer taken as the
  28S's (not seen lit). No new field.
- **Identification by "OK-19B II".** The two ROM halves end "OK-19B-1DK"
  and "OK-19B-2A", but a 19B's ROM might start its tags the same way, so
  the signature is the self-test's whole pass message, "OK-19B II", which
  the ROM keeps in one piece (review finding). A 128 KB image is the
  19BII's with that text and the 28S's with "OK-28S"; with both or
  neither it fits nothing.
- **The clock through TIME SET, as on the 17BII.** Same menus and key
  plan; the format nibble is #C41E1, the language prompt's nibble is
  at #C402B (0 only on the prompt), both found as on the Pioneers by
  watching RAM.
- **No download, said plainly.** The 19BII cannot read its own ROM out
  (no memory scanner), so `rom fetch 19bii` and the pages do not say
  "read it out of your own calculator": saturnus has no way to get this
  ROM; it takes a file the user has.
- **Held keys** work as the manual says (MEM and SHOW "until you
  release" them): the "keys act on release" seen here was the keyboard
  interrupt bug fixed on 2026-10-13 ("fix: held keys on the Lewis
  models" below), not the ROM.

## 2026-10-13 (fix: held keys on the Lewis models)

- **Root cause of "keys act only on release".** The 1 ms keyboard poll
  that raises the keyboard interrupt read IN through the OUT lines the
  ROM last drove. The Lewis ROMs (42S, 27S, 17B, 17BII, 28S, 19BII)
  debounce inside the keyboard handler by scanning row by row every 2 ms,
  paced by TIMER2 (#403F9), with interrupts enabled; a poll that fell
  between two row writes saw the held key gone and the next one saw it
  back, a new rising edge. The edge arrived while the handler ran, so it
  was latched as pending and the handler ran again at RTI: with a key
  held, the CPU never left the handler (traced on the 42S rev. C ROM: a
  keyboard interrupt every ~15 ms, the main loop at #009A3 never ran).
  The ROM's main loop, which shows MEM, SHOW and the 42S's preview and
  NULL, got the key only once it was released. Traced on the 42S, tested
  on the 42S and 27S; the 17B, 17BII, 28S and 19BII run the same chip
  model and are expected to behave alike (not traced). The "4096 stable scans"
  noted in iteration 44 was this starvation, not a ROM timeout.
- **Fix**: the poll is the chip's own scan of the whole keyboard,
  whatever OUT holds (wiki: hardware/keyboard "Scanning model": Ervin 2,
  "1 ms hardware scan of the whole keyboard, no software involved";
  Duchesne: holding a key causes no interrupt). KDN set by the poll
  follows the same scan; A=IN/C=IN still read through OUT (and set KDN
  from what they read, so a row scan can clear it until the next poll).
  SHUTDN wake is unchanged (Voyage p. 130: a key whose OUT row is
  active), and so is RSI (SASM: "any input line ... presently high",
  i.e. IN as read). All models. Emu48's change log describes the key
  interrupt as the rising edge of OR(IN[8:0]) and as raised after an OUT
  with an IN bit set (wiki: emulators/emu48 SP4, SP16), which reads as
  OUT-dependent; Ervin's hardware description (the scan is the
  hardware's own, of the whole keyboard) is followed over an emulator's
  change log, and the Lewis ROMs only work with it. Filed in the wiki
  (hardware/keyboard "Scanning model").
- **Effect on the other models.** The 48, 49 and aplet ROMs scan rows in
  their handler too, so they also took extra handler passes while a key
  was held (48SX, 200 ms holds: 5-10 handler entries before, 3-7 after;
  the ROM is idle 5-250 ms after the release instead of 95-290 ms). No
  screen golden of theirs changes; what changes is how much emulated time
  typing takes (38G: setting the clock by keys 10.4 s instead of 12.7 s),
  and so the outputs that read the clock: the 38G/39G/40G clock goldens
  (the TIME shown when it is read) and the TICKS and TIME command
  examples were re-recorded. Not a regression: those values are the
  clock at the moment of reading.
- **Measured after the fix**: 42S hold of SIN shows SIN at once and NULL
  after 950 ms of emulated time ("about a second", 42S Owner's Manual
  p. 75); 27S shift MEM and shift SHOW show until release. The web
  page's pointer handling was not at fault (a held mouse button keeps
  the key down; `web/test/holdkey-page.test.mjs`).

## 2026-10-13 (fix: the clock's lead measured per set)

- **Root cause of "the 27S's clock is 9 s ahead".** The models set by
  keys keyed the time ahead by a fixed lead per model (200 ms per press
  on the Pioneers, 230-240 ms on the aplet models, times the presses
  before the time is taken). Two faults: the leads were measured before
  the held-keys fix made the keys faster (38G: 4.3 s keyed vs 3.1 s
  taken; 39G and 40G 4.1 vs 2.6; 27S 4.4 vs 4.1; 17B, 17BII 4.6 vs 4.4;
  19BII 4.6 vs 5.4), and they assumed 1x. At any other speed the keys
  run in emulated time faster than the wall clock, and so does the
  calculator's clock, until the CPU sleeps again (asleep, emulated time
  follows the wall clock at 1x). At Max a 27S keyed 4.4 s ahead, then
  ran the 4.9 s of its set in a fraction of a second: about 9 s ahead,
  as the owner saw. The page also sent whole seconds, losing up to 1 s.
- **Fix**: each set is rehearsed first on a clone of the machine, as fast
  as the host can: the whole plan queued at once, run in slices cut as
  the host's passes cut them (`Pace::pass_ms`: the pass period times the
  speed, 1000 ms at Max), with a count of presses started marking each
  commit key. A commit counts halfway between its key going down and
  the CPU asleep after it (or the next key down). With `c` the time's
  commit and `end` the set's end in emulated ms, and `r` the emulated ms
  per wall ms the set will run at, the time keyed is `now + planning +
  c / r - (end - c) * (1 - 1 / r)`, rounded to the second: the clock then
  reads the wall clock when the set is done and follows it from there.
  `r` is the speed (1, 2, 4; at Max what a pass may run per period, 60
  on the Worker), capped by the host's own speed as the rehearsal
  measured it times the pass's budget share; a 48 or 49 write runs as
  fast as the host can. `planning` is the wall time the rehearsals took.
  The date keyed is the date of the clock when the date is taken. The
  keys are planned again with the time the rehearsal asks for and
  rehearsed again until a plan asks for itself (usually two rehearsals;
  after three, the last one rehearsed). The page sends `millisecond`.
  A set whose rehearsal never settles is refused in words; a time out
  of 1991-2090 after the lead is refused; a wake that halts the CPU or
  leaves time owed refuses the set.
- **The 38G's set is chaotic in slice phase.** How long its queue waits
  for the ROM to go idle depends on where slices end: by pass length
  alone (16.66 to 16.67 ms, 15.3, 17, 1, 4) its time's commit fell
  anywhere from 6.9 to 8.4 s with the date stored first, and the set's
  end from 8.3 to 10.6 s with the time first (commit 2.6 to 2.8 s, two
  outliers at 3.6 and 3.8). The 39G varied by ±0.2 s, the 27S by ±0.1 s.
  A rehearsal on a host whose passes jitter cannot pin that down, so the
  aplet models store the time first at 1x (only the commit counts) and
  last above it (the tail after the commit counts, `keyed`). Not looked
  into: why the 38G's ROM is so rarely idle when the queue looks.
- **One digit for another changes the 38G's timing** by over a second
  (the same chaos), so convergence compares the whole plan, not just its
  length: a plan is only keyed after its own rehearsal.
- **The 39G and 40G dropped a repeated key**: 1133 typed as 13, twelve
  1s as six; before and after the held-keys fix. The ROM takes a key
  pressed again within about 95 ms of its release as the same press
  (measured by the gap needed: 90 ms lost, 100 ms kept). A queued press
  of the key just released now waits 150 ms on those two
  (`host::repeat_gap_ms`). The 38G, 27S, 17B, 17BII and 19BII kept 1133
  whole with the 30 ms gap. This had made the 39G's clock 3 s off
  whenever the time had a doubled digit.
- **Measured** (`tests/clock.rs`, the engine on a hand-moved clock, the
  request at .200, .450 and .700 s, at 1x, 2x, 4x and Max; the test
  asserts within 1 s of the wall clock): every model set by keys and the
  48GX came within 0.45 s of the span of wall time during the readback
  (the 38G at 4x the worst). On a real host the 38G may land up to about
  a second off at 1x by the chaos above. The 48s and 49G write uses the
  same formula; its lead comes out near zero, as the write's commit
  falls at its end. Rehearsals natively (release): 37-170 ms per set.

## 2026-10-14 (two apps; plans 47 to 49)

Owner decisions:

- **The emulator and saturnus tx stay two separate apps.** tx is mainly
  for real calculators over a cable, USB or serial. A later merge must
  stay possible, so nothing new is built on the tab messages of
  `web/shared/site-links.js` or on the hub's serial-line ownership.
  tx reaches the emulator's own calculator through the hub's memory
  commands (a `HubDevice`), not by typing SERVER into it
  ([[iterations/iteration-47-hub-device]]); its private calculator keeps
  the SERVER and Kermit path as the honest test of the cable path.
- **Navigation names**: "Calculator" and "Files"; the landing headline
  leads with saturnus ("saturnus: classic HP calculators in your
  browser"), HP only in running text.
- Planned from the 2026-10-14 reviews: one emulated serial wire and a
  decode error type ([[iterations/iteration-48-one-wire]]), Rust-side
  roots for the desktop tx app's files
  ([[iterations/iteration-49-tx-desktop-roots]]).

Owner answers on the plans (PR 175):

- **47**: Restore and the graphics screen (PICT) are never offered on the
  emulator's shared calculator (its Load state and the "+" above the
  display cover them); they stay for cable and private calculators. The
  emulator's chip is "Connected to saturnus tx · Show" only, no
  Disconnect and no `dismiss` hub command. Serial commands on a shared
  machine are refused with a clear message; no serial console in the
  emulator is planned (reopen if one is).
- **48**: the engine's and satx-core's Kermit clients are not merged now
  ([[backlog/one-kermit-client]]). `MIN_WAIT` is 10 ms, the products'
  value; the refgen reference is regenerated once for it. `satx get X`
  writes `X.hp`, no compatibility flag.
- **49**: picked folders are remembered across runs and the folder pane
  gets "Recent folders" (desktop: Rust-kept, picked ones are roots; web:
  File System Access handles in IndexedDB, one click to grant again).
  Browsing is not confined. Mounted volumes are not automatic roots:
  picked once, remembered.

## 2026-10-14 (iteration 47: HubDevice, the command policy, the apps' names)

- **saturnus tx reaches the emulator's calculator as a `HubDevice`**
  (`web/shared/hub-device.js`), through the engine's memory commands,
  never its serial port: nothing typed, no line held. Gone with it: the
  hub's serial-line ownership (`holder`, `LINE_TAKEN`, `keysElsewhere`,
  the speed juggling of `SERIAL_IDLE_MS`), the shared branch of
  `EmulatedCalculator` and KermitDevice's recovery of a server ended
  behind the page's back. The private calculator keeps SERVER and Kermit
  over `EmulatorLink`, the honest test of the cable path.
- **The memory commands' gaps**, decided:
  1. Free memory: not added (the memory tree has none; a `MEM`
     transaction per listing is not worth it). The emulator's
     calculator shows no free figure.
  2. Get as text: `fetchFile` takes `text: true` (a Kermit get in ASCII
     mode, flag -35 cleared and put back).
  3. Put with replace: `storeFile` takes `replace`, three ways: absent
     is as before (flag -36 decides, the emulator's own drops keep their
     `.1`), `false` refuses a name in use, `true` stores under a free
     temporary name, recalls it over the old variable (STO replaces) and
     purges the temporary one only then; a directory replaces only after
     `MEM` showed room for it, the old variable purged first; an error
     names the temporary variable while it holds the only new copy.
  4. Backup: a new `backup` command, one hidden transaction (`MEM`
     checked against twice HOME and 1024 bytes, saturnus-tx's
     `BACKUP_MARGIN`; `ARCHIVE` to `:0:`, recalled into a variable, the
     port's copy purged, got, purged), the same `.hp` a cable gives.
  5. Restore, 6. the graphics screen: never on the emulator's
     calculator (owner); its Load state and the "+" above its display
     cover them. The pane's "⋯" has a line for the Load state instead.
  7. The fast transfer: not offered (a hidden transaction takes 0.15 to
     0.25 s of wall time).
  8. Flags: no gap (tx has no flags view).
- **The serial port of a shared calculator** is refused to every page
  (owner); a page alone on its engine opens it, and a port it opened is
  closed when a second page comes.
- **The emulator's chip** "Connected to saturnus tx · Show" follows the
  hub's new `peers` event; Show brings tx's tab forward through
  `openSiteTab` without a request (an anchor's `target` alone would load
  tx's tab again and drop its connection). No Disconnect (owner).
- **A file dropped on the drawn calculator**: one with an HP transfer
  header, or a `.txt` that is text, is stored in the current directory;
  any other file goes to the ROMs as before (handled first, so a ROM
  dropped with them boots before they are stored).
- **The command policy**: `CommandKind` and one exhaustive `policy()`
  match in `saturnus-host` replace the four command tables of the
  protocol, the runner's native list and the CLI's memory endpoint list;
  `backup` is on `POST /v1/memory` like every write.
- **Names**: the emulator's link to tx reads "Files", tx's link to the
  emulator "Calculator"; the landing page's title and headline
  "saturnus: classic HP calculators in your browser".

## 2026-10-14 (iteration 49: roots for the desktop tx app)

- **Roots in Rust** ([[iterations/iteration-49-tx-desktop-roots]]): the
  desktop tx app refuses reads of a file's bytes and every change outside
  the home folder and the folders picked in its dialog; browsing
  (`folder_list`, `folder_stat`) stays open (owner). Picked folders are
  remembered in `folders.json` (Rust-only writes, 0600); a mounted volume
  is picked once like any other folder (owner). A remove or rename is
  checked on the folder holding the entry, so a link in home pointing out
  of it can still be removed while nothing can be written through it.
- **Picked folders stay roots after the recent list moves on**: the file
  keeps the 8 newest entries plus up to 32 picked ones; "Clear recent
  folders" forgets the picked ones too (the roots are home alone again),
  as clearing the browser's list drops its handles. A picked folder that
  is not there at start (unplugged) is no root until it is back; the app
  looks again before it refuses.
- **What the desktop pane reports**: every folder it shows
  (`folder_used`), the home folder left out, and a walk down or up from
  the newest unpicked entry replaces it, so opening `a/b/c` leaves one
  entry. `folder_used` never makes a root.
- **The `x-serve` header** marks the aplet server's writes so Rust
  refuses them directly in home. It guards against a page bug, not a
  hostile page (which would drop the header), and says so in the scope
  note.
- **Web**: the single kept handle (`FOLDER_KEY`) is gone, not migrated
  (owner's standing rule): the folder kept before is picked once more.
- **Debug builds** read both apps' files (tx's `folders.json`, the
  emulator app's `settings.json`) from `SATURNUS_SETTINGS_DIR` when set,
  so tests never touch the user's.

## 2026-10-14 (iteration 48: one emulated wire, a decode error type)

- **One emulated wire** ([[iterations/iteration-48-one-wire]]):
  `saturnus_host::wire` holds the serial port's rules on emulated time
  (turnaround, quiet line, busy time, read cap, the timeouts) once, with
  a resumable `Wire::step` the engine's hidden writes call in turns and
  a blocking `read`/`write_packet` that saturnus-kermit's `Link` uses.
  satx-core's `saturnus://` transport takes its step, quiet line and
  server wait from it too. The real-line loops of satx-core and satx
  (`session.rs`, `xmodem.rs`, `link.rs`, `xserv.rs`) keep their own
  wall-clock constants: they are not the emulated wire.
- **`MIN_WAIT` is 10 ms everywhere** (owner), the products' value;
  saturnus-kermit had 1 ms. The command reference, regenerated in full
  on the shared wire at 10 ms (`saturnus-refgen` catalog, menus and
  examples for the 48SX, 48GX and 49G), came out byte for byte the same
  as the committed one, so the planned regeneration commit has nothing
  to commit; `tests/regen.rs --ignored` passes at 10 ms.
- **`saturnus-objects` errors**: `DecodeError { offset, reason, address }`
  from the decoder, `saturnus_objects::Error` from the rest; no anyhow
  in its API. Breaking for the next release.
- **satx file names**: tx's mapping (`names`) moved to satx-core and is
  satx's too: `satx get X` writes `X.hp` (owner: no compatibility flag).
  `satx backup` keeps its timestamped name (the model is not known
  before connecting).
- **Kermit watchdog**: satx-core's transaction loop (Session and
  Machine) ends a transaction without progress (a packet taken as new,
  counted by `kermit-proto`'s `Client::accepted`) for
  (timeout + pause) x (retries + 2) with `Error::Stalled`.

## 2026-10-17 (one app)

Owner decisions, reversing "two separate apps" of 2026-10-14:

- **One app.** The emulator and saturnus tx become one app, "saturnus":
  one HTML document with a hash router at `/saturnus/app/`, a home page
  that lists the calculators, and the views "Calculator" and "Files".
  Chosen for maintenance: one start code, one service worker, no tab
  messages, no SharedWorker ([[iterations/iteration-50d-one-document]]).
  No iframes.
- **Platforms**: the web app is the product, installable as a PWA on
  phones and tablets; no native mobile apps for now (no USB serial on
  iOS). macOS, Windows and Linux get one Tauri app around the same
  document ([[iterations/iteration-51-one-desktop-app]]).
- **The desktop engine stays native**: it keeps running while the
  window is hidden and survives a reload; WebAssembly there would save
  only the bridge (about 500 lines).
- **No SharedWorker**: `hub.js` runs in a plain Worker; one live
  calculator per tab. A second tab gets a calculator that does not
  save.
- **No migration**: old URLs, subsites, crates and casks are removed
  without redirects; the site may break between iterations.
- **Defaults taken, open to the owner**: one WebAssembly package
  (`saturnus-tx-web` into `saturnus-web`); tx's private calculator kept
  for the cable path's test and the aplet server, not listed on home.
- [[backlog/desktop-shared-calculator]] is superseded.
- **Plan reviewed** (fresh review, 2026-10-17): the web step is split into
  50a (one WebAssembly package), 50b (Files mountable), 50c (calculator
  mountable), 50d (one document, router, home); views are hidden, not
  unmounted, so the calculator never stops on a view switch; the old
  subsites get kill switches (cleanup, not migration); the storage names
  keep their values; 51's Files-on-the-running-calculator needs
  bytes-returning native commands and an in-page multiplexer, and the
  desktop app gets wasm and a CSP that allows it.

## 2026-10-18 (auto power-off stays)

- **The ROMs' auto power-off stays as it is**: each ROM switches off
  after about ten idle minutes, and ON brings the calculator back with
  its state intact. Removing it would mean patching the idle counter per
  ROM revision (fragile, and ROM digging against the clean-room rule),
  hiding time from it (breaks the clock), or pressing ON for the user
  when the ROM switches off (a workaround). No setting for it.

## 2026-10-19 (fix: a held key runs at 1x)

- **Root cause of "1 ENTER 2 ENTER + shows NULL on the 42S" (owner, at a
  speed above 1x).** While keys were down, the passes ran at the chosen
  speed: at Max up to 1000 emulated ms per pass, so a 250 ms click lasted
  seconds of emulated time. The 42S cancels a key held about a second
  (NULL); the 48 and 49 auto-repeat a held key; every model was affected.
- **Fix**: while a key the user pressed is down (`KeyQueue::held`), a pass
  runs at 1x whatever the speed. Queued and typed presses keep the speed:
  their holds are fixed in emulated time. Tests: `protocol/tests.rs`
  `a_held_key_runs_at_1x_at_every_speed`, `web/test/speedhold-page.test.mjs`
  (42S ROM, 250 ms taps at Max; fails without the fix).

## 2026-10-19 (28S: 0.00042 is stored unnormalised, ROM behaviour)

- **"0.00042 ENTER MANT gives .0042 on the 28S" is the ROM, not the
  CPU.** Traced with registers: the 28S parser's leading-zero loop
  (#10938) leaves at the point of `0.`, and the point handler's GOTO
  (#109C1, `68DF`, to #1099A) enters the digit loop, so the zeros after
  the point become mantissa digits (B = 0004200000000, exponent 999 =
  -1) and nothing normalises them. `.00042` goes through the zero loop
  with the point seen and gives 4.2E-4. Every branch on the path matches
  the instruction semantics; the 28C ROM has the same nibbles. The core
  stays unchanged; the fact is in the wiki (`wiki: hardware/hp28s`
  "Number entry"), unconfirmed on a real 28S (the owner has none).
- **Not the cause of `0.5959 0.0001 HMS+` = .6 on the 48SX**: the 48SX
  and 48GX parse 0.0001 normalised (MANT 1, XPON -4). The .6 comes from
  the 48SX's own HMS+ code (the 48GX gives 1); not yet checked against
  the saturnng oracle, so the example stays as generated.

## 2026-10-19 (48SX HMS+ is the ROM's)

- `0.5959 ENTER 0.0001 HMS+` gives .6 on the 48SX (ROM J in saturnus) and 1
  on the 48GX (rev R). The owner's real 48SX also gives .6, so it is the SX
  ROM's HMS+, not the emulation; the 48SX can do the sum right by hand (HMS→,
  +, →HMS gives 1). The generated 48SX example keeps .6, the real result.
  Wiki: hardware/hp48sx "Differences from the G/GX".

## 2026-10-19 (38G ACOT is the ROM's)

- On the 38G (ROM A167) ACOT(1) in DEG mode gives .785398163397, the radian
  answer; the owner's real 38G in DEG gives the same, so it is the ROM's ACOT,
  not the emulation (ATAN, ACSC and ASEC follow the mode). The 38G's ACOT
  description says so and it has no example. The 39G and 40G examples run
  in RAD, their default, and are right. Wiki: hardware/hp38g "ROM quirks".

## 2026-10-19 (49G clock examples: the ROM's server context, not the emulator)

- The 49G's `1.012030 →DATE DATE` gave 61.877009 in the generated examples
  (48SX and 48GX right). Traced with registers in saturnus (ROM 2.15 and
  2.10): the 49G ROM sets the clock by storing a next-event time and loading
  TIMER2 with it less the new time; it takes the next event from the
  five-minute deadline the Kermit server has armed (#80069), so moves of
  more than 2^31 ticks (about 3 days) wrap in TIMER2's 32 bits. Every
  instruction on the path does what it should, and from the keyboard the
  same `→DATE` is exact, so the core stays unchanged. Not checked on a real
  49G or saturnng (no container runtime set up on this Mac).
- saturnus-refgen runs `707788800. CLKADJ` and `-707788800. CLKADJ` as two
  commands before every 49G example that sets the clock (`CLOCK_49G`): the
  day forward passes the deadline, which clears it, and the day back is
  exact. The 49G examples now show what a keyboard user gets, and the 49G
  shares the 48s' TIME example. Wiki: hardware/clocks "49G: setting the
  clock from the Kermit server".
- STOF `{ #0h #5h }` RCLF gave `{ # 0d # 1d }`: the example's fault. Clearing
  every system flag clears flags -5 to -10 too, the word size, which becomes
  1 bit, so RCLF's words show one bit (48G AUR, RCLF and STOF; 48G UG
  appendix D). The examples now store `#FF0h`, which keeps the 64-bit word
  size and HEX base.
