# saturnus

A headless emulator of the HP Saturn-based calculators, written in Rust from
published documentation and the behaviour of the calculators' own ROMs.
Library first, with a CLI, an MCP server and UIs on top.

Targets, in order: HP 48SX, HP 48GX, HP 49G, then HP 38G, 39G and 40G.

Status: iterations 1-4: CPU core, disassembler, memory controller, I/O
registers, display, keyboard, card ports, save/load state, the UART and a
serial bridge. The HP 48SX ROM boots and its screens match the saturnng
emulator pixel for pixel in the scenarios below, with and without a RAM
card. hptx's Kermit end-to-end suite passes against saturnus over TCP and
in-process. Plan and docs live in `kb/`.

## Getting the ROM

saturnus does not include HP's ROM images. The CLI downloads the HP 48SX
ROM J from hpcalc.org after asking for confirmation, then checks its size and
checksum:

```sh
cargo run --release -p saturnus-cli -- rom fetch --model 48sx --dir roms
```

| File      | Size         | SHA-256 |
|-----------|--------------|---------|
| `sxrom-j` | 262144 bytes | `e5eb3af020e4910f35a7580a705cf0a46f3ba9d7ba5516582d98010c93af7c74` |

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
| `--model 48sx` | calculator model (the only one so far) |
| `--rom FILE` | packed ROM image; the size is checked |
| `--load FILE` | restore a saved state first (it must come from the same ROM) |
| `--cycles N` | run N CPU cycles before the key script |
| `--keys FILE` | replay a key script |
| `--screen FILE` | write the final screen: `.txt` or `.png` |
| `--annunciators FILE` | write the lit annunciators, e.g. `alpha` or `-` |
| `--save FILE` | save the machine state at the end |
| `--card1 FILE` | packed RAM-card image for port 1 (CE1), inserted after `--load` |
| `--card2 FILE` | the same for port 2 (CE2) |
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
```

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
soon as the UART sends them, with Nagle disabled. Bytes the calculator
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
the ROM's wait for a key. The ROM takes a variable time to react, so scripts
use `wait-idle` rather than fixed waits. Reaching the cap prints a warning
and the script continues; a blinking cursor, for example, never goes idle.
A press holds 60 ms by default because the ROM only accepts a key after
about 10 ms of debouncing.

Key names are case-insensitive:

| Group | Names |
|-------|-------|
| Softkeys | `a` `b` `c` `d` `e` `f` |
| Row 2 and 3 | `mth` `prg` `cst` `var` `up` `nxt` `quote` `sto` `eval` `left` `down` `right` |
| Row 4 | `sin` `cos` `tan` `sqrt` `power` `inv` |
| Row 5 | `enter` `neg` `eex` `del` `backspace` |
| Digits | `0` to `9`, `point` |
| Operators | `plus` `minus` `multiply` `divide` `space` |
| Modifiers | `alpha` `leftshift` `rightshift` `on` |

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
`MODEL=48sx AUTOSTART=0 CARDS=0`, so both sides have empty card slots. A
scenario can change that in an optional `config` file next to `keys.txt`:
`ORACLE_CARDS=1` keeps the oracle's default 128 KB card, and
`SATURNUS_CARD1=1` or `SATURNUS_CARD2=1` gives saturnus a fresh zeroed
card in that port.
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
