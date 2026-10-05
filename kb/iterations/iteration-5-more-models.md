---
title: "Iteration 5: More models"
type: iteration
date: 2026-10-04
status: completed
branch: iter-5/more-models
tags:
  - iteration
  - saturnus
---

# Iteration 5: More models

Read first: wiki `hardware/hp48gx`, `hardware/hp49g`, `hardware/memory-controller`
(bank switching, DA19, Giesselink controller model), `hardware/keyboard`
(49G matrix), `questions/hp49g-bank-latch-bits`, `hp49g-flash-write`,
`hp49g-ram-controllers`, `da19-polarity`, `emulators/emu48` (facts only).

## Context from iterations 1-4 (2026-10-05)

- Today `Machine`/`Hardware` are 48SX-specific: `Model` has one variant,
  `Hardware::new(rom, ram)` wires NCE2 RAM, CE1/CE2 card slots and the HDW
  window; `io/registers.rs` is the HDW window incl. UART, timers, cards;
  `state.rs` saves it all (format version 1, unshipped, may change freely).
  The CLI (`saturnus run --model`, `rom fetch`) and `scripts/diff-vs-saturnng.sh`
  (scenario `config` files; the oracle container takes `MODEL=48sx|48gx|49g`)
  know only the SX.
- ROMs: 48GX ROM R is `https://www.hpcalc.org/hp48/pc/emulators/gxrom-r.zip`
  (file `gxrom-r`, 524288 bytes packed); 49G ROM 2.15 is
  `https://www.hpcalc.org/hp49/pc/rom/hp4950emurom.zip` (member `rom.49g`,
  2097152 bytes, the whole 2 MB flash image packed two nibbles per byte).
  Same curl default user agent rule; `roms/` is gitignored; record SHA-256s
  in `rom fetch`. The hptx container has all three models for the oracle.
- Yorke facts the GX needs (wiki: hardware/hp48gx, memory-controller):
  4 MHz; 512 KB ROM with DA19 (#129 bit 3) switching upper ROM (DA19=1) vs
  port 2 (DA19=0, lower ROM mirrored at #80000); 128 KB RAM at #80000; CE1
  is the bank latch (74HC174) usually at #7F000, a byte read at
  `base+#40+2n` latches bank n (A1-A5) and BEN (A6); CE2 = port 1, NCE3 =
  port 2 banks; `#11F` reads 8 on G/GX; empty slots configured as 2 KB
  at #7E000; a GX without cards shows ROM at #C0000-#FFFFF. Keyboard, display,
  timers and UART are the SX layout.
- 49G facts (wiki: hardware/hp49g): Yorke at 4 MHz; NCE1 = 2 MB flash in 16
  banks of 128 KB, two views through the CE1 latch (Giesselink: A1-A2 pick
  the #00000-#3FFFF bank 0-3, A3-A6 the #40000-#7FFFF bank 0-15); NCE2 =
  256 KB RAM at #80000 (HOME/port 0); CE2 and NCE3 = 128 KB each (port 1),
  unconfigured by default; flash writes go through NCE3 at #40000 with bit
  3 of #11C set, using the Intel 28F160S5 command set (public datasheet,
  not in raw/); different keyboard matrix (wiki: hardware/keyboard "HP49G
  matrix"); the ROM asks "Try To Recover Memory?" then shows "Memory Clear"
  with an OK softkey.
- 38G/39G/40G: the wiki pages are stubs. Research first (HP Journal 38G
  articles in `raw/saturn-hardware/hp-journal/hpj-38g/`, Emu48 KML model
  letters `A`/`6`/`E` in `wiki/emulators/emu48`), then decide whether a
  boot is feasible in this iteration; ROM availability on hpcalc.org is
  unchecked.
- Work split: one agent generalises the machine for a second model and
  lands the 48GX end to end; one builds the 49G's standalone parts (flash
  chip model, 49G keyboard table) and wires the 49G once the generalised
  machine exists; one does the 38G/39G/40G research in the wiki.

## Tasks

- [x] 48GX: 128 KB RAM, bank-switched port 2 with the byte-read latch quirk,
  DA19 polarity (wiki settled it against Mastracci and Voyage).
- [x] 49G: 512 KB RAM, 2 MB flash with banking and the write-enable path
  (`questions/hp49g-bank-latch-bits`, `hp49g-flash-write` to resolve by
  experiment), different keyboard.
- [x] 38G, 39G/40G: research first (HP Journal 38G articles in raw/, Emu48
  documentation facts, ROM availability); fill the wiki pages; boot only if
  a ROM and the memory map are in hand, otherwise plan a follow-up.

## Acceptance criteria

- [x] 48GX: boots to the stack, differential screens vs saturnng match,
  hptx e2e passes over TCP.
- [x] 49G: boots to the stack, differential screens vs saturnng match,
  hptx e2e passes over TCP.
- [x] 38G/39G/40G: wiki pages filled from sources; the 38G boots to HOME
  (no oracle); 39G/40G are planned in iteration 5b.

## Outcome

### 48GX and multi-model core

- **Model abstraction** (`crates/saturnus/src/machine/model.rs`). `Model`
  is `Hp48sx`, `Hp48gx` or `Hp49g`. It provides `name`, `rom_bytes`,
  `ram_nibbles` (NCE2), `clock_hz` (2 MHz SX, 4 MHz GX and 49G),
  `contrast_range` (3-19 SX, 9-24 GX and 49G), `card_max_bytes(port)` and
  `hardware()`. `hardware()` returns a `HardwareProfile`, which gives the
  `ChipRole` of CE1, CE2 and NCE3, whether NCE3 shares its pin with ROM
  A19 (the GX's DA19), and the card size limits per port. The roles are
  `Empty`, `Ram(nibbles)`, `Card(port)`, `BankedCard(port)` and
  `BankLatch`. `Hardware::new(model, nce1)` builds RAM behind every
  `Ram` role and decodes reads and writes through the roles. Power-on is
  the same on every model: everything unconfigured, latch 0.
- **NCE1 extension point** (`crates/saturnus/src/modules/nce1.rs`). The
  enum `Nce1` has one variant, `Rom(Rom)`. `Nce1::read(addr, latch)` gets
  the CE1 latch so a banked device can pick its bank, and
  `Nce1::nibbles()` feeds the state checksum. The 49G adds a
  `Flash` variant. Flash writes through NCE3 need a write hook in
  `Hardware::write_nibble` (NCE3 role plus #11C bit 3), and the flash
  contents then belong in the saved state. `Machine::new(Model::Hp49g, ..)`
  returns `Error::Unsupported` until then; `Machine::with_hardware`
  (crate-private) takes prebuilt hardware. The provisional 49G profile is
  CE1 latch, CE2 and NCE3 128 KB RAM each, no card ports.
- **48GX hardware.** CE1 is the 74HC174 latch. Every nibble read in its
  window latches nibble address bits A1-A6: bits 0-4 are the port 2 bank,
  bit 5 is BEN. Writes do not latch. SHUTDN and CPU reset clear the latch.
  CE2 is port 1, with cards up to 128 KB. NCE3 is port 2, showing a
  128 KB bank (card offset `bank * #40000 + offset`) of a card up to
  4 MB; NCE3 takes part in decoding only while DA19 = 0 and BEN = 1. With
  DA19 = 0 the ROM loses A19, so the lower 256 KB repeat at #80000. #10F
  bits 1 and 3 belong to the CE2 card and bits 0 and 2 to the other slot,
  on both models.
- **Byte-read latch quirk.** Giesselink's three-nibble skew is not
  modelled. ROM R latches with byte reads at #7F040+2n, each preceded by
  a BEN = 0 read of #7F000. With the exact latch, `33 PVARS` with a 4 MB
  card already gives "Invalid Card Data", and `1`/`2`/`33 PVARS` match
  saturnng.
- **State format** is now version 2. It adds the model codes (SX 0,
  GX 1, 49G 2), the RAM blocks behind CE1, CE2 and NCE3, the latch byte,
  and per-port card size limits.
- **CLI.** `--model 48gx`, and `rom fetch --model 48gx` downloads
  `gxrom-r` (SHA-256 `de3a5a07...5b33`, 524288 bytes). The 49G entry has
  no checksum yet and refuses to verify. `--autostart` types the same keys
  on the GX as on the SX. New cards default to 128 KB (`NEW_CARD_BYTES`),
  and `CARD_MAX_BYTES` is the 4 MB limit over all ports.
- **Results** (2026-10-05). ROM R reaches "Try To Recover Memory?" after
  about 0.6 s of emulated time, then NO gives "Memory Clear". After boot
  the contrast is 14 and #11F holds 8. GX goldens and e2e tests cover boot,
  state round trip and Kermit; `SATURNUS_ROM_DIR=$PWD/roms cargo test
  --release -p saturnus --test e2e` passes 7/7 in about 2 s. Diff
  scenarios `gx-boot`, `gx-arith`, `gx-menu`, `gx-alpha`, `gx-offon`,
  `gx-card`, `gx-card-p2` and `gx-card-p33` all match saturnng pixel for
  pixel at the first oracle run; 3 scenarios take about 65-90 s. The SX
  scenarios still match. hptx e2e against
  `saturnus run --model 48gx --rom roms/gxrom-r --serial tcp:4862
  --autostart` passes 6/6 in 61 s; port 4852 was taken by another
  container.
- **38G (stretch).** `Model::Hp38g` is a configuration: it uses the 48G
  profile without card ports (CE1 latch, DA19, CE2 and NCE3 empty; the
  wiring is inferred), 32 KB on NCE2, 4 MHz, and the 48 key matrix with
  48 key names. `rom fetch --model 38g` downloads `38G_A167.ROM` (SHA-256
  `3c9f747f...16be7`, packed, I/O window already zero). The ROM places
  the RAM at #F0000 itself. A cold boot shows a "Memory Clear" box, OK
  goes to HOME, and `6 * 7 ENTER` gives 42. The e2e test
  `hp38g_boot_to_home_and_compute` covers this with three goldens and a
  state reload, and passes. There is no oracle. Findings are on the wiki
  pages `hardware/hp38g` and `questions/hp38g-memory-controllers`.
  Remaining: 38G key names (the follow-up task below), Kermit or the
  38G's own transfer protocol, and the 39G/40G.
- **49G hooks** for the 49G agent: `Nce1::{set_write_enabled, nce3_read,
  nce3_write, state_blob, load_state_blob}`, and the profile flags
  `latch_writes` and `nce3_flash_path`. Every #11C write is forwarded to
  NCE1, and the state carries an NCE1 byte block.
- **Open.** G-series cycle counts differ from the S series (wiki:
  emulators/emu48 SP1) and are not modelled, so the GX uses the SX's
  counts. Display, timers and UART use the SX layout without per-model
  checks beyond the scenarios. The 48G (32 KB) is not a separate model.
  Port 2 write protect (wired to CE1 on the GX) is not modelled.

### 38G/39G/40G research

Done in the calculator wiki (`~/devel/hp-literature`, log entry
2026-10-05 "HP 38G, 39G and 40G hardware"). Wiki pages to read:
`hardware/hp38g`, `hardware/hp39g-40g`, then `hardware/hp48gx`,
`hardware/hp49g`, `hardware/memory-controller`, `hardware/keyboard`, and
the open questions `questions/hp38g-memory-controllers`,
`hp39g-40g-memory-map`, `hp39g-40g-model-detection`,
`hp38g-39g-transfer-protocol`.

Feasibility verdict:

- **38G: needs the generalised Yorke machine (48GX work), then boots with
  little new code.** It is a 48G with 512 KB OTP ROM and 32 KB RAM at the
  top of the address space, #F0000-#FFFFF (Mueller, via Finseth), the 48
  keyboard matrix with two
  positions empty, IR and serial as on the 48G, no card slots. ROM:
  hpcalc.org `https://www.hpcalc.org/hp38/pc/38grom.zip` (323,277 bytes),
  member `38G_A167.ROM`, 524,288 bytes packed, revision A1.67. Unknown:
  which controller carries the RAM (NCE2 assumed), whether CE1/CE2/NCE3
  are wired, DA19 use; the ROM's own CONFIG sequence will show them.
- **39G/40G: needs the 49G model (CE1 latch, NCE1 banking, 49G keyboard),
  then boots as a configuration of it.** A 49G cut to 1 MB mask ROM and
  256 KB RAM; one ROM image for both models; 49G keyboard matrix; 40G has
  no IR. ROM: hpcalc.org details page 6739, `rom3940.zip` (563,090 bytes),
  member `rom.39g`, 2,097,152 bytes (1 MB unpacked or 2 MB packed and
  mirrored: check on download). Unknown: how the shared ROM tells a 40G
  from a 39G (Emu48 needs `Class 39`/`40`); booting whichever model the
  ROM picks comes first.
- Neither model can be checked against saturnng (the oracle container has
  48sx, 48gx and 49g only), so acceptance has to be a boot to HOME, key
  input, and the reset chords from the user's guides.
- ROM licence: the hpcalc 38G and 39/40 pages lack the "HP graciously began
  allowing this to be downloaded" sentence the 48 pages carry; the Emu48
  manual calls them freely available since fall 2000 without a
  distribution licence. Same policy as the 48/49: `rom fetch`, never
  shipped.

Follow-up: the 38G model landed in this iteration (see "38G" above). The
remaining 38G key names, the 39G/40G models, the model strap, the 38G/39G
wire protocol and the oracle-less acceptance are planned in
[[iterations/iteration-5b-39g-40g-and-38g-keys]].

### 49G

Phase one (standalone parts, not yet wired into the machine):

- `modules/flash.rs`: `Flash`, the Intel 28F160S5 (2 MB, x8, 32 erase
  blocks of 64 KB) from the public datasheet, order 290609-004 (wiki:
  `sources/intel-28f160s5`). Read array, identifier, CFI query, status and
  extended status modes; program (bits only clear), block and full chip
  erase, write to buffer, clear status, suspend/resume as no-ops, lock-bits
  with a WP# input, STS configuration. Operations complete at once, so
  status always reads ready. Nibble writes become byte cycles: the even
  nibble is held, the odd nibble of the same byte completes the byte (low
  nibble first; an assumption, wiki `questions/hp49g-flash-write`).
  Writes are dropped unless the machine opens `set_write_enabled` (#11C
  bit 3). Banking helpers: latch = nibble address bits A1-A6, A19
  ignored; NCE3 writes go to the bank of the #40000 view. (Phase one
  followed Giesselink's bit order; phase two found the ROMs need Sousa's,
  see below.) Packed and unpacked image loaders and a packed export.
- `io/keyboard49.rs`: the 8x8 matrix plus ON at IN bit 15. (Phase one had
  its own `Key49` type; phase two folded it into the shared `Key`, see
  below.)
- **ROM images.** Primary: `hp4950emurom.zip` member `rom.49g` (2.15,
  2 MB packed, SHA-256
  `b01c13e24a692f35e6087106d58ec205b4696d5b5e35d57f8f94015f8bb1f1ca`), the
  image the saturnng 49g oracle runs and boots. Its readme labels it for
  the 48gII/49g+/50g, it lacks the original 49G boot sector ("Boot
  Version" at #00214), and its first 64 KB differ from the images below.
  Fallback: `hp4950v210.zip` member `rom.49g` (2.10, 2 MB packed, SHA-256
  `58c3de6b7fc75a0ba65fca7437c4d49d8f26ca9e334a57e4d3bc4f8fb2dc8c11`).
  Also seen: `beta1196.zip` member `rom.49g` (1.19-6, 4 MB unpacked,
  SHA-256 `b5371ca6d25424a8138705aea919b7590b11f00b5eeb32f4aac33664d7d1c476`).
  Both of those have "Boot Version 1.A" at #00214. 2.15 boots on saturnus
  and matches the oracle, so it stays primary.

Phase two (wired into the multi-model core; decisions in the decision
log, iteration 5):

- `Nce1::Flash` behind the GX agent's hooks: NCE1 reads go through the
  latch views; while #11C bit 3 is set, NCE3 reads and writes the flash at
  the #40000 view's bank. `Machine::new(Hp49g)` takes the 2 MB packed or
  4 MB unpacked image. Reset returns the chip to read array with its gate
  closed. The flash, lock-bits, status, read mode and WP# go into the
  existing NCE1 state block (format stays version 2).
- **Latch bit order is Sousa's**: A1-A4 pick the #40000 bank, A5-A6
  the #00000 bank. With Giesselink's order ROM 2.15 runs into data within
  1 M cycles and ROMs 2.10 and 1.19-6 stop at "No System"; with Sousa's
  all three boot (wiki `questions/hp49g-bank-latch-bits`, answered).
- **SHUTDN keeps the 49G's latch** (profile `shutdn_clears_latch`): the OS
  sleeps while running from a switched bank (#017E7 in 2.15's boot).
- **Keys**: one `Key` set for all models, placed by a per-model `Layout`;
  49G-only `apps mode tool hist cat eqw symb x`, softkeys `a`-`f` with
  `f1`-`f6` aliases. Alpha letters G-Z sit on APPS to the divide key,
  measured on saturnus and checked against saturnng (`49g-alpha`).
- CLI: `rom fetch --model 49g` (2.15, SHA-256 recorded; 2.10 documented as
  fallback in the README); `--autostart` answers NO and OK and types
  SERVER with the 49G's letters; `wait-idle` now needs lit pixels or a
  switched-off display, because the 49G boot sleeps with a blank screen.
- Flash writes by the ROM: `42 STO :2:A` programs 24 bytes of bank 8 with
  write to buffer (#50, #E8, count, data, #D0, #70, #FF); `RCL(:2:A)`
  gives 42.
- Acceptance: e2e `hp49g_boot_to_stack` (goldens
  `49g-try-to-recover-memory`, `49g-memory-clear`, `49g-stack`; the last
  equals saturnng's screen), `hp49g_state_round_trip`,
  `hp49g_kermit_server_receives_a_file`,
  `hp49g_store_to_port2_programs_flash`; diff scenarios `49g-boot`,
  `49g-arith`, `49g-menu`, `49g-alpha`, `49g-port2` match saturnng pixel
  for pixel; hptx's e2e suite passes 6/6 against
  `saturnus run --model 49g --rom roms/rom.49g --serial tcp:4863
  --autostart`.
- Open: how the real 49G glue logic turns nibble writes into byte cycles
  and protects the boot sector (the ROM's writes fit the model); the
  oracle's OFF state is not compared.
