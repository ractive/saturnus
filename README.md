# saturnus

A headless emulator of the HP Saturn-based calculators, written in Rust from
published documentation and the behaviour of the calculators' own ROMs.
Library first, with a CLI, an MCP server and UIs on top.

Targets, in order: HP 48SX, HP 48GX, HP 49G, then HP 38G, 39G and 40G.

Status: iterations 1-4 built the CPU core, disassembler, memory
controller, I/O registers, display, keyboard, card ports, save/load state,
the UART and a serial bridge. Iteration 5 (in progress) added a per-model
hardware description, the HP 48GX, the HP 49G with its 2 MB flash, and,
as a configuration, the HP 38G. The HP 48SX, HP 48GX and HP 49G ROMs boot,
and their screens match the saturnng emulator pixel for pixel in the
scenarios below (the 48s with and without RAM cards). hptx's Kermit
end-to-end suite passes against saturnus over TCP on all three. The HP 38G
boots to HOME and takes key input; there is no oracle for it. Plan and
docs live in `kb/`.

| Model | ROM | CPU clock | RAM | Ports | Status |
|-------|-----|-----------|-----|-------|--------|
| HP 48SX | J, 256 KB | 2 MHz | 32 KB | port 1 (CE1) and port 2 (CE2), up to 128 KB each | boots, screens match, Kermit |
| HP 48GX | R, 512 KB | 4 MHz | 128 KB | port 1 (CE2) up to 128 KB, port 2 (NCE3) up to 4 MB in 128 KB banks | boots, screens match, Kermit |
| HP 38G | A1.67, 512 KB | 4 MHz | 32 KB at #F0000 | none | boots to HOME, takes keys (no oracle) |
| HP 49G | 2.15, 2 MB flash (banked, programmable) | 4 MHz | 512 KB (256 KB NCE2, 128 KB each on CE2 and NCE3) | none | boots, screens match, Kermit |

## Getting the ROM

saturnus does not include HP's ROM images. The CLI downloads the HP 48SX
ROM J, the HP 48GX ROM R, the HP 38G ROM A1.67 or the HP 49G ROM 2.15 from
hpcalc.org after asking for confirmation, then checks its size and
checksum:

```sh
cargo run --release -p saturnus-cli -- rom fetch --model 48sx --dir roms
cargo run --release -p saturnus-cli -- rom fetch --model 48gx --dir roms
cargo run --release -p saturnus-cli -- rom fetch --model 38g --dir roms
cargo run --release -p saturnus-cli -- rom fetch --model 49g --dir roms
```

| File      | Size         | SHA-256 |
|-----------|--------------|---------|
| `sxrom-j` | 262144 bytes | `e5eb3af020e4910f35a7580a705cf0a46f3ba9d7ba5516582d98010c93af7c74` |
| `gxrom-r` | 524288 bytes | `de3a5a07b0f00640f4ba3599ea4092e9473113aad75c04bd03d3e37c059b5b33` |
| `38G_A167.ROM` | 524288 bytes | `3c9f747f637757d3adc414ed14d7f3636033f34f0a72e6e453ee197987f16be7` |
| `rom.49g` (2.15) | 2097152 bytes | `b01c13e24a692f35e6087106d58ec205b4696d5b5e35d57f8f94015f8bb1f1ca` |

The 49G's `rom.49g` comes from `hp4950emurom.zip`, the image the saturnng
container runs. Its readme labels it for the 48gII/49g+/50g and its boot
sector differs from the original 49G one, but it boots as a 49G. A
fallback with the original 49G boot sector, fetched by hand: ROM 2.10,
`https://www.hpcalc.org/hp49/pc/rom/hp4950v210.zip`, member `rom.49g`
(2097152 bytes, SHA-256
`58c3de6b7fc75a0ba65fca7437c4d49d8f26ca9e334a57e4d3bc4f8fb2dc8c11`).
`--model 49g` also loads unpacked images (4 MB, one nibble per byte), such
as the 1.19-6 beta's `rom.49g` from `beta1196.zip`.

The download uses the system `curl` with its own user agent, then `unzip`
(or `tar`). `--yes` skips the prompt. An existing file that verifies is kept.
`roms/` is ignored by git; never commit ROMs or state files.

## Running

The binary is called `saturnus` (crate `saturnus-cli`):

```sh
cargo build --release -p saturnus-cli
S=target/release/saturnus

# Boot, answer "Try To Recover Memory?" with NO, dump the screen.
$S run --model 48sx --rom roms/sxrom-j \
    --keys scripts/scenarios/boot/keys.txt --screen boot.txt

# Same, as a 131x64 PNG, and save the machine state.
$S run --rom roms/sxrom-j --keys scripts/scenarios/boot/keys.txt \
    --screen boot.png --save boot.state

# Continue from the saved state with more keys.
$S run --rom roms/sxrom-j --load boot.state --keys more.txt --screen out.txt

# Disassemble ROM code.
$S disasm --rom roms/sxrom-j --at 0 --count 20
```

`run` options:

| Option | Meaning |
|--------|---------|
| `--model M` | calculator model: `48sx` (default), `48gx`, `38g` or `49g` |
| `--rom FILE` | packed ROM image; the size is checked |
| `--load FILE` | restore a saved state first (it must come from the same ROM) |
| `--cycles N` | run N CPU cycles before the key script |
| `--keys FILE` | replay a key script |
| `--screen FILE` | write the final screen: `.txt` or `.png` |
| `--annunciators FILE` | write the lit annunciators, e.g. `alpha` or `-` |
| `--save FILE` | save the machine state at the end |
| `--card1 FILE` | packed RAM-card image for port 1 (48SX CE1, 48GX CE2), inserted after `--load`; a missing file becomes a zeroed 128 KB card |
| `--card2 FILE` | the same for port 2 (48SX CE2; 48GX NCE3, up to 4 MB) |
| `--card-writeback` | write the card images back to their files at the end |
| `--trace N` | print the last N instructions at the end or on a CPU halt |
| `--serial SPEC` | after the key script, bridge the serial port (see below) |
| `--autostart` | with `--serial`: answer the boot prompt with NO and start the Kermit server |
| `--exit-on-disconnect` | with `--serial`: stop when the first client leaves |
| `--serial-log FILE` | with `--serial`: append the wire traffic with emulated timestamps |
| `-v` | report when each `wait-idle` became idle, and serial connections |

A card file that does not exist is created as a zeroed 128 KB card, with a
note on stderr. Cards are 1 KB to 128 KB, a power of two, two nibbles per
byte like the ROM. They are written back only with `--card-writeback`; a
saved state already contains the card contents.

The `.txt` screen is 64 lines of 131 characters, `#` for a dark pixel and
`.` for a light one, every line ended by a newline. The `.png` is 131x64,
1 bit per pixel, dark pixels black.

## Serial port and Kermit

`--serial` bridges the calculator's wired serial port, so Kermit clients
such as [hptx](https://github.com/ractive/hptx) talk to saturnus the way they
talk to the saturnng container or a real HP 48SX:

```sh
$S run --model 48sx --rom roms/sxrom-j --serial tcp:4850 --autostart
# serial bridged on tcp:127.0.0.1:4850
$S run --model 48gx --rom roms/gxrom-r --serial tcp:4862 --autostart
$S run --model 49g --rom roms/rom.49g --serial tcp:4863 --autostart
```

On the 49G `--autostart` answers NO and then OK on the "Memory Clear" box,
and types SERVER with the 49G's letter keys.

| `--serial` | Meaning |
|------------|---------|
| `tcp:PORT` | listen on 127.0.0.1:PORT, one client at a time; reconnects are fine |
| `tcp:HOST:PORT` | listen on HOST, e.g. `0.0.0.0` |
| `stdio` | bytes on stdin go to the calculator, its output goes to stdout |

The order is: `--load`, cards, `--cycles`, the key script, then
`--autostart`, then the bridge. `--autostart` answers "Try To Recover
Memory?" with NO (skipped with `--load`) and types ALPHA ALPHA S E R V E R
ENTER; the screen then shows "Awaiting Server Cmd.". When the port is
listening, saturnus prints `serial bridged on tcp:HOST:PORT` on stdout
(stderr for `stdio`), so scripts can wait for that line.

While bridged, the machine runs paced to wall-clock time: 2 MHz of emulated
cycles per real second, in slices of at most 1 ms. It sleeps when ahead and
catches up when behind; after more than 200 ms behind (a stopped process,
an overloaded host) it re-anchors instead of running a burst. Kermit
timeouts on both ends are wall-clock, so running flat out would break them.
Incoming bytes reach the emulated UART at line rate (11.375 bit times per
byte at the IOPAR baud rate); outgoing bytes are written to the socket as
soon as the UART sends them, with Nagle disabled. At most 2 KiB wait for
the UART; above that the bridge stops reading, so TCP flow control (or the
bounded stdin pipe) holds a faster sender back. When a client leaves, what
it already sent still goes out to the calculator, typically its final ACK;
the next client is accepted once that has drained, so sessions never mix. Bytes the calculator
sends while no client is connected are dropped; the container's pty keeps
them instead, which is the stale NAK hptx drains on connect.

The bridge runs until SIGINT or SIGTERM (or, with `--exit-on-disconnect`,
until the client leaves), then writes `--screen`, `--annunciators`,
`--save` and the card files as usual.

```sh
# hptx's end-to-end suite against saturnus
$S run --rom roms/sxrom-j --serial tcp:4850 --autostart &
cd ~/devel/hptx && HPTX_E2E_ADDR=tcp://localhost:4850 \
    cargo test -p hptx-core --test e2e -- --nocapture
```

hptx can also run saturnus in-process, without a socket: build it with the
`saturnus` feature and open `saturnus:///path/to/sxrom-j`.

## Key scripts

A key script is a text file with one action per line. Times are emulated
milliseconds, so a script gives the same result on any host. `#` starts a
comment.

| Action | Meaning |
|--------|---------|
| `press KEY [MS]` | hold KEY for MS (default 60), release it, then `wait-idle` |
| `KEY` | same as `press KEY` |
| `down KEY` | press KEY and keep it down |
| `up KEY` | release KEY |
| `wait MS` | run for MS |
| `wait-idle [CAP]` | run until idle, at most CAP ms (default 10000) |

Idle means the LCD has not changed for 300 ms while the CPU sits in SHUTDN,
the ROM's wait for a key, and the screen shows something (or the display is
switched off): the 49G spends seconds of its boot in SHUTDN with a blank
screen. The ROM takes a variable time to react, so scripts
use `wait-idle` rather than fixed waits. Reaching the cap prints a warning
and the script continues; a blinking cursor, for example, never goes idle.
A press holds 60 ms by default because the ROM only accepts a key after
about 10 ms of debouncing.

Key names are case-insensitive. The 48 models and the 49G share the names
of keys they have in common; a key the model lacks is ignored.

| Group | Names |
|-------|-------|
| Softkeys | `a` `b` `c` `d` `e` `f` (also `f1` to `f6`) |
| Row 2 and 3 | `mth` `prg` `cst` `var` `up` `nxt` `quote` `sto` `eval` `left` `down` `right` |
| Row 4 | `sin` `cos` `tan` `sqrt` `power` `inv` |
| Row 5 | `enter` `neg` `eex` `del` `backspace` |
| Digits | `0` to `9`, `point` |
| Operators | `plus` `minus` `multiply` `divide` `space` |
| Modifiers | `alpha` `leftshift` `rightshift` `on` |
| 49G only | `apps` `mode` `tool` `hist` `cat` `eqw` `symb` `x` |

On the 49G, `var` `up` `nxt` `sto` `left` `down` `right` `sin` `cos` `tan`
`sqrt` `power` `inv` `neg` `eex` `backspace` and the digits and operators
name its keys of the same label; `mth` `prg` `cst` `quote` `eval` `del`
are 48 only.

Example, `6 ENTER 7 * ENTER` after a cold boot:

```text
# Boot, answer NO, then 6 ENTER 7 * ENTER: 42 on levels 1 and 2.
wait-idle 60000
press f
6
enter
7
multiply
enter
```

## Differential tests against saturnng

`scripts/diff-vs-saturnng.sh` replays each scenario in
`scripts/scenarios/<name>/keys.txt` on saturnus and on the saturnng emulator
in Docker. It then compares the final screens pixel for pixel and the lit
annunciators. saturnng is used as a black box only.

```sh
scripts/diff-vs-saturnng.sh            # all scenarios
scripts/diff-vs-saturnng.sh boot arith # some
```

It needs Docker (Rancher Desktop's `~/.rd/bin` is added to `PATH`),
`python3`, the ROM in `roms/`, and the oracle image `hp49g-emu`. The image
is built from `~/devel/hptx/emulator` if it is missing; set `EMU_DIR` or
`IMAGE` to change that. Each scenario starts a fresh container with
`AUTOSTART=0 CARDS=0` and the scenario's model, so both sides have empty
card slots. A scenario can change that in an optional `config` file next
to `keys.txt`: `MODEL=48gx` or `MODEL=49g` runs both sides as that model
(default `48sx`; the script uses the model's TUI letter map),
`ORACLE_CARDS=1` keeps the oracle's default cards (128 KB in port 1, plus
4 MB in port 2 on the 48GX), and `SATURNUS_CARD1=1` or `SATURNUS_CARD2=1`
gives saturnus a fresh zeroed card in that port, 128 KB unless
`SATURNUS_CARD1_KB` or `SATURNUS_CARD2_KB` says otherwise.
The script translates the key script into saturnng TUI keys and waits
between keys until the oracle's LCD stops changing. `down` and `up` cannot
be replayed there.

Both screens and annunciator lines land in `target/diff-vs-saturnng/<name>/`.
On a difference the script prints the first differing row and column and a
diff. The oracle's TUI occasionally delivers a key twice, so a differing
scenario is replayed once more on a fresh container (`ORACLE_RETRIES`) and
every differing run is kept. The exit status is 0 when everything matches,
1 on a difference and 2 on a setup error.

| Scenario | Keys | Result |
|----------|------|--------|
| `boot` | NO at "Try To Recover Memory?" | match |
| `arith` | `6 ENTER 7 * ENTER` | match |
| `menu` | MTH, NXT, softkey A | match |
| `alpha` | ALPHA | match |
| `offon` | `6 ENTER`, OFF, 2 s, ON | match |
| `card` | 128 KB RAM card, `2 PVARS` | match |
| `gx-boot` | 48GX: NO at "Try To Recover Memory?" | match |
| `gx-arith` | 48GX: `6 ENTER 7 * ENTER` | match |
| `gx-menu` | 48GX: MTH, NXT, softkey A | match |
| `gx-alpha` | 48GX: ALPHA | match |
| `gx-offon` | 48GX: `6 ENTER`, OFF, 2 s, ON | match |
| `gx-card` | 48GX, 128 KB and 4 MB cards: `1 PVARS` | match |
| `gx-card-p2` | 48GX, same cards: `2 PVARS` | match |
| `gx-card-p33` | 48GX, same cards: `33 PVARS` gives "Invalid Card Data" | match |
| `49g-boot` | 49G: NO, then OK on "Memory Clear" | match |
| `49g-arith` | 49G: `6 * 7 ENTER` (algebraic mode) | match |
| `49g-menu` | 49G: MODE input form | match |
| `49g-alpha` | 49G: ALPHA ALPHA, then every lettered key A to Z | match |

The `card` scenario found a real bug: the #10F card-status bits pair with
the chip selects (bits 0 and 2 for the CE1 card, 1 and 3 for CE2), not
with Mastracci's port numbers. The 48SX ROM tests the card flagged by bits
1 and 3 in the CE2 window at #C0000 and calls it port 2, which is why the
oracle's card file named `port1` shows up as port 2.

The calculator's OFF state is not compared. saturnng keeps drawing a
display bitmap while the ROM has switched the display off.

## Tests

```sh
cargo test --workspace -q
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus --test e2e   # needs the ROM
```

Without `SATURNUS_ROM_DIR` the e2e test is skipped. The bring-up example
`cargo run --release -p saturnus --example boot -- roms/sxrom-j --screen`
also still works.

## Legal

Saturnus is an independent, clean-room project. It shares no code with
Emu48, x48, x48ng, saturnng or HP EMU. It does not include HP's ROM images;
you download them yourself from hpcalc.org, where HP has allowed them to be
downloaded since 2000. Not affiliated with HP. HP, HP48 and HP49 are
trademarks of HP Inc.

License: MIT, see `LICENSE` and `AI_NOTICE`.
