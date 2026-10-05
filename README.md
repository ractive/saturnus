<img src="web/logo.svg" alt="saturnus logo: a planet with a ring" width="72" height="72">

# saturnus

A headless emulator of the HP Saturn-based calculators, written in Rust from
published documentation and the behaviour of the calculators' own ROMs.
Library first, with a CLI, an MCP server and UIs on top.

Targets, in order: HP 48SX, HP 48GX, HP 49G, then HP 38G, 39G and 40G,
and the HP 42S.

Status: iterations 1-4 built the CPU core, disassembler, memory
controller, I/O registers, display, keyboard, card ports, save/load state,
the UART and a serial bridge. Iteration 5 (in progress) added a per-model
hardware description, the HP 48GX, the HP 49G with its 2 MB flash, and,
as a configuration, the HP 38G. The HP 48SX, HP 48GX and HP 49G ROMs boot,
and their screens match the saturnng emulator pixel for pixel in the
scenarios below (the 48s with and without RAM cards). hptx's Kermit
end-to-end suite passes against saturnus over TCP on all three. The HP 38G
boots to HOME and takes key input; there is no oracle for it. Iteration 5b
added the HP 39G and HP 40G (one ROM; the 40G is told apart by a board
strap the ROM reads) and the 38G's and 39G's own key names. Iteration 7
calibrated instruction timing against real-hardware benchmark times (see
"Speed" below). Iteration 15 added the HP 42S, a Pioneer-series machine on
HP's Lewis chip with a 131x16 display, from the owner's own ROM dump. Plan
and docs live in `kb/`.

| Model | ROM | CPU clock | RAM | Ports | Status |
|-------|-----|-----------|-----|-------|--------|
| HP 48SX | J, 256 KB | 2 MHz | 32 KB | port 1 (CE1) and port 2 (CE2), up to 128 KB each | boots, screens match, Kermit |
| HP 48GX | R, 512 KB | 4 MHz | 128 KB | port 1 (CE2) up to 128 KB, port 2 (NCE3) up to 4 MB in 128 KB banks | boots, screens match, Kermit |
| HP 38G | A1.67, 512 KB | 4 MHz | 32 KB at #F0000 | none | boots to HOME, takes keys (no oracle) |
| HP 49G | 2.15, 2 MB flash (banked, programmable) | 4 MHz | 512 KB (256 KB NCE2, 128 KB each on CE2 and NCE3) | none | boots, screens match, Kermit |
| HP 39G | `rom.39g`, 1 MB mask ROM (banked) | 4 MHz | 256 KB (NCE2) | none | boots to HOME, takes keys, reset chords (no oracle) |
| HP 40G | the 39G's ROM | 4 MHz | 256 KB (NCE2) | none | as the 39G; HOME shows the CAS key (no oracle) |
| HP 42S | your own dump, 64 KB (rev. C tested) | 1 MHz (uncalibrated) | 8 KB at #50000 | none (IR printer not modelled) | boots to "Memory Clear", takes keys, self-test runs (no oracle) |

## Speed

Emulated programs take as long as on the real calculators, within a few
percent, on the HP Museum summation benchmark
(`'Σ(X=1,n,XROOT(3,EXP(SIN(ATAN(X)))))'`):

| Model | Run | Real | saturnus |
| --- | --- | --- | --- |
| HP 48SX | n = 1000 | 95.5 s | 95.5 s |
| HP 48GX | n = 100 | 5.9 s | 5.9 s |
| HP 49G (ROM 2.10) | FOR/NEXT, n = 100 | 5.5 s | 5.4 s |

Instructions are timed with the SASM manual's cycle counts on the 48SX and
the Meta Kernel counts (from the Saturn tutorial) on the Yorke models,
plus a 13% display-refresh stall, times a per-model calibration factor.
The factor (1.20-1.34) is fitted to these benchmarks; its cause is not
known. See `kb/decision-log.md`, iteration 7. The 42S runs the SASM counts
at a flat 1 MHz with no factor and no stall: there is no benchmark of a
real 42S yet.

## Getting the ROM

saturnus does not include HP's ROM images. The CLI downloads the HP 48SX
ROM J, the HP 48GX ROM R, the HP 38G ROM A1.67, the HP 49G ROM 2.15 or the
HP 39G/40G ROM from hpcalc.org after asking for confirmation, then checks
its size and checksum:

```sh
cargo run --release -p saturnus-cli -- rom fetch --model 48sx --dir roms
cargo run --release -p saturnus-cli -- rom fetch --model 48gx --dir roms
cargo run --release -p saturnus-cli -- rom fetch --model 38g --dir roms
cargo run --release -p saturnus-cli -- rom fetch --model 49g --dir roms
cargo run --release -p saturnus-cli -- rom fetch --model 39g --dir roms   # also the 40G's
```

| File      | Size         | SHA-256 |
|-----------|--------------|---------|
| `sxrom-j` | 262144 bytes | `e5eb3af020e4910f35a7580a705cf0a46f3ba9d7ba5516582d98010c93af7c74` |
| `gxrom-r` | 524288 bytes | `de3a5a07b0f00640f4ba3599ea4092e9473113aad75c04bd03d3e37c059b5b33` |
| `38G_A167.ROM` | 524288 bytes | `3c9f747f637757d3adc414ed14d7f3636033f34f0a72e6e453ee197987f16be7` |
| `rom.49g` (2.15) | 2097152 bytes | `b01c13e24a692f35e6087106d58ec205b4696d5b5e35d57f8f94015f8bb1f1ca` |
| `rom.39g` (39G/40G) | 2097152 bytes | `69220f42d5e90dd8825e7d1596d9eaca490ee6a7a52a3b8b96469a5f3d3f627f` |

The 49G's `rom.49g` comes from `hp4950emurom.zip`, the image the saturnng
container runs. Its readme labels it for the 48gII/49g+/50g and its boot
sector differs from the original 49G one, but it boots as a 49G. A
fallback with the original 49G boot sector, fetched by hand: ROM 2.10,
`https://www.hpcalc.org/hp49/pc/rom/hp4950v210.zip`, member `rom.49g`
(2097152 bytes, SHA-256
`58c3de6b7fc75a0ba65fca7437c4d49d8f26ca9e334a57e4d3bc4f8fb2dc8c11`).
`--model 49g` also loads unpacked images (4 MB, one nibble per byte), such
as the 1.19-6 beta's `rom.49g` from `beta1196.zip`.

The 39G/40G `rom.39g` (from `rom3940.zip`) holds the 1 MB ROM unpacked,
one nibble per byte, and carries the I/O registers of the calculator it
was read from at #00100-#0013F; saturnus zeroes them when it loads the
image. `--model 39g` and `--model 40g` also take the 1 MB packed form.

The download uses the system `curl` with its own user agent, then `unzip`
(or `tar`). `--yes` skips the prompt. An existing file that verifies is kept.
`roms/` is ignored by git; never commit ROMs or state files.

**HP 42S.** HP never released the 42S ROM and no site may offer it, so
`rom fetch --model 42s` refuses. Dump your own calculator: the 42S sends its
ROM over the infrared printer port to an HP 48 running a binary-safe INPRT
(Christoph Gießelink's `PIONEER.TXT` in the Emu42 ROM upload package walks
through it; `LEWISCRC` from the Emu42 package checks the image), then move
it to the PC with Kermit. Pass the 64 KB packed image with
`--model 42s --rom FILE`. The tested image is revision C (SHA-256
`f4c5f9f0e1d89074b7ca49add99b3ea72ed7fae9370b421de20a0cd8384c08f3`); its
self-test (EXIT + LN) reports a ROM CRC of #1BE8 instead of the expected
#FFFF, so that dump may have bad bits (kb: iteration 15).

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
| `--model M` | calculator model: `48sx` (default), `48gx`, `38g`, `49g`, `39g`, `40g` or `42s` |
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
and types SERVER with the 49G's letter keys. The 38G, 39G and 40G have no
Kermit server command, so `--autostart` refuses them.

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

Key names are case-insensitive. All models share the names of keys they
have in common, wherever the key sits on the model's matrix. A script that
uses a key the model lacks is refused before it runs, e.g.
`key "prg" is not on the 49g keyboard (line 2)`.

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
| 38G | menu keys `a`-`f`, `plot` `symb` `num` `up` `lib` `var` `math` `left` `down` `right` `home` `sin` `cos` `tan` `xt` `sqrt` `enter` `lparen` `rparen` `neg` `power` `alpha` `shift` `del` `comma`, digits, `point`, operators, `on` |
| 39G and 40G | menu keys `a`-`f`, `symb` `plot` `num` `up` `home` `aplet` `views` `left` `down` `right` `vars` `math` `ddx` `xt` `del` `sin` `cos` `tan` `ln` `log` `square` `power` `lparen` `rparen` `comma` `alpha` `shift` `neg`, digits, `point`, operators, `enter`, `on` |
| 42S | `sigmaplus` `inv` `sqrt` `log` `ln` `xeq` (the top row, also the menu keys), `sto` `rcl` `rdn` `sin` `cos` `tan`, `enter` `swap` `neg` `eex` `backspace`, `up` `down` `shift`, digits, `point`, operators, `rs`, `on` (also `exit`) |

On the 49G, `var` `up` `nxt` `sto` `left` `down` `right` `sin` `cos` `tan`
`sqrt` `power` `inv` `neg` `eex` `backspace` and the digits and operators
name its keys of the same label; `mth` `prg` `cst` `quote` `eval` `del`
are 48 only.

The 38G, 39G and 40G keys carry their own labels: `xt` is X,T,θ, `neg` the
(-) key, `power` x^y, `alpha` the A...Z key and `ddx` d/dx. They sit where
the 48 (38G) or 49G (39G, 40G) key in the same place on the case sits, so
`sin` on the 38G is the 48's COS position (wiki: hardware/hp38g,
hardware/hp39g-40g). The reset chords of the user's guides are `on` with
`c` (reset) and `on` with `a` and `f` (memory clear), held together with
`down` and `up` lines.

On the 42S `eex` is the E key, `swap` x≷y, `rdn` R↓, `rs` R/S and `on` the
EXIT key; it has no `a`-`f` (its top row keeps its labels). `on` with `ln`
runs the ROM's self-test, `on` with `sqrt` resets it, `on` with `inv` clears
memory. Screens are 131x16 (16 text lines).

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

## MCP server

`saturnus-mcp` is a Model Context Protocol server on stdin/stdout. It owns
one emulated calculator and gives an agent its keyboard, its screen and,
through the Kermit server and [hptx](https://github.com/ractive/hptx)'s
`hptx-core`, its stack and variables as typed objects. Build it with
`cargo build --release -p saturnus-mcp`.

Claude Code (`.mcp.json` in a project, or `claude mcp add`) and Claude
Desktop (`claude_desktop_config.json`) take the same entry:

```json
{
  "mcpServers": {
    "saturnus": {
      "command": "/path/to/saturnus/target/release/saturnus-mcp",
      "args": ["--model", "48sx", "--rom", "/path/to/roms/sxrom-j"]
    }
  }
}
```

With `--rom` the server boots that ROM at startup (`--model` defaults to
`48sx`; `--autostart` also starts the Kermit server). Without arguments the
agent calls `boot`. Tools:

| Tool | Arguments | What it does |
|------|-----------|--------------|
| `boot` | `model` (`48sx`, `48gx`, `49g`, `38g`, `39g`, `40g`, `42s`), `rom_path`, `autostart` | Build and boot, answer the first prompt (NO; then OK on the 49G; OK on the 38G, 39G and 40G; nothing on the 42S), optionally start the Kermit server (not on the 38G, 39G, 40G or 42S) |
| `press_keys` | `script` | Run a key script (below); returns emulated ms, annunciators and the screen as text. Leaves Kermit server mode first |
| `type_text` | `text` | Type letters (alpha mode, lowercase too), digits, `. + - * /`, space and newline (ENTER). Leaves Kermit server mode first |
| `screen` | `format` (`png` default, `text`), `scale` (1-8, PNG) | The 131x64 LCD (131x16 on the 42S) as an image or `#`/`.` text, plus the annunciators |
| `start_server` / `stop_server` | | Type `SERVER` with the stack showing / end it with Kermit FINISH |
| `read_stack` | `levels` | The stack as display text, highest level first |
| `run_command` | `command` | Execute an RPL command line, return the stack |
| `send_object` | `name`, `text` or `bytes_base64`, `mode` (`ascii`, `binary`) | Store a variable (Kermit PUT) |
| `receive_object` | `name`, `mode` | Fetch a variable (Kermit GET): text, or base64 for binary |
| `save_state` | `path`, `overwrite` | Machine state to a file (atomic write; never the session's ROM; a foreign existing file needs `overwrite`) |
| `load_state` | `path` | Machine state from a file; a refused state changes nothing |
| `reset` | | Hardware reset (RAM kept), run until idle |
| `status` | | Model, ROM, cycles, emulated time, server running, `mode` (`server` or `keyboard`), keys pressed |

### Semantic tools: eval and the typed stack

On the 48SX, 48GX and 49G these tools work on values instead of keys.
Each takes `keep_server` (default `false`), see "Server mode" below.

| Tool | Arguments | What it does |
|------|-----------|--------------|
| `eval` | `source`, `levels` (default 1), `keep_server`, `timeout_ms` (1000-600000, default 60000) | Run RPL source; return levels 1..`levels` typed, with display text and the depth |
| `stack` | `levels` (default all), `keep_server` | The stack as typed objects, level 1 first; the stack is not changed |
| `push` | `object`, `keep_server` | Put a typed object on level 1 |
| `pop` | `keep_server` | Remove level 1 and return it typed |
| `drop` | `count` (default 1), `keep_server` | Drop levels |
| `clear_stack` | `keep_server` | `CLEAR` |
| `get_var` | `name`, `keep_server` | A variable of the current directory, typed, not evaluated |
| `set_var` | `name`, `object`, `keep_server` | Store a typed object (`STO`, replacing the variable) |
| `list_vars` | `keep_server` | The current path and its variables: name, type, size, checksum |
| `cd` | `path`, `keep_server` | `HOME`, `HOME/A/B`, `A/B` (relative) or `..`; returns the new path |

`eval` takes a command line (`2 3 +`, `'X^2' 3 'X' STO EVAL`), an
algebraic or a program; Unicode or ASCII trigraphs (`\->`, `\<<`). Source
the calculator rejects as `Invalid Syntax` is run again as an algebraic,
so `SIN(0.5)` works as `'SIN(0.5)' EVAL`. Source longer than one Kermit
packet goes over as a string that `STR→` compiles. Results stay on the
calculator's stack.

```json
{"name": "eval", "arguments": {"source": "SIN(0.5)"}}
{"levels":[{"type":"real","value":0.479425538604}],"display":[".479425538604"],"depth":1,"server":"stopped"}
```

That is in radians; a fresh 48SX is in degrees (`eval "RAD"` first), the
49G in radians. A calculator error is a tool error with the same JSON
shape, and the arguments stay on the stack as the calculator leaves them:

```json
{"name": "eval", "arguments": {"source": "1 0 /"}}
{"error":"Infinite Result","depth":2,"display":["0","1"],"server":"stopped"}
```

A syntax error leaves the stack as it was. `0 0 /` is `Undefined Result`.

Objects, as `eval`, `stack`, `pop` and `get_var` return them and `push`
and `set_var` take them:

| Type | JSON |
|------|------|
| real | `{"type":"real","value":0.479425538604}`; exact to the calculator's 12 digits; text such as `"1.5E-400"` beyond an f64's exponent range |
| integer (49G) | `{"type":"integer","value":5}`; text beyond 15 digits, e.g. `"1267650600228229401496703205376"` |
| complex | `{"type":"complex","re":2.0,"im":-4.0}` |
| string | `{"type":"string","value":"Hello"}` |
| name, local name | `{"type":"name","value":"X"}`, `{"type":"local_name","value":"x"}` |
| binary integer | `{"type":"binary","value":255,"base":"dec","text":"# 255d"}`; the base is the calculator's display mode |
| list | `{"type":"list","items":[...]}` |
| tagged | `{"type":"tagged","tag":"T","object":{...}}` |
| unit | `{"type":"unit","value":9.81,"unit":"m/s^2"}` |
| array | `{"type":"array","dims":[2,2],"items":[[{...},{...}],[{...},{...}]]}` (reals or complex numbers) |
| program | `{"type":"program","source":"« 1 2 +\n»"}` |
| algebraic | `{"type":"algebraic","source":"'X^2+1'"}` |
| character | `{"type":"character","value":"A"}` |
| command | `{"type":"command","source":"+"}`: a built-in command inside a list or program |
| unknown | `{"type":"unknown","prolog":"02B1E","kind":"Graphic","nibbles":2196,"hex":"E1B20..."}` (hex cut at 4096 nibbles) |

Values are decoded from the binary object a Kermit GET returns (the
format is in the wiki's `protocols/hp-object-format`), so reals keep all
12 digits and strings are not truncated. Programs, algebraics and unit
expressions take their text from an ASCII GET of the same object. To read
levels without changing the stack, the tools copy them into a temporary
list variable (`SATRNTMP`, or `SATRNTM1` to `SATRNTM3` if taken), fetch it
and purge it. `push` sends an object as RPL text in a host command when it
has text that fits one packet, otherwise as a binary object (exact; a
string holding `"` goes this way), or as a string compiled with `STR→`.
The decoder lives in its own crate, `crates/saturnus-objects` (no I/O,
builds for `wasm32`); `saturnus-mcp` adds the Kermit side (binary files,
ASCII sources, the encoder).

### Memory read from RAM: memory_tree and flags

These two read the calculator's memory straight from RAM, the way the ROM
keeps it, without the Kermit server and without running the calculator
(48SX, 48GX, 49G; wiki `hardware/hp48-system-ram` has the locations).
The 38G, 39G and 40G (aplets) and the 42S (no RPL user memory) answer
with an error:

| Tool | Arguments | What it does |
|------|-----------|--------------|
| `memory_tree` | none | The current path and HOME's whole tree: every variable with name, type, size, checksum and address, sub-directories nested, newest first |
| `flags` | none | System and user flags as 64-flag words (16 hex digits) and the list of set flags |

After `42 'X' STO 'DA' CRDIR DA 5 'Z' STO HEX 7 SF` on a fresh 48SX
(`IOPAR` is the Kermit server's):

```json
{"name": "memory_tree", "arguments": {}}
{"path":["HOME","DA"],"variables":[{"name":"DA","type":"Directory","size":29.0,"checksum":44093,"address":524238,"variables":[{"name":"Z","type":"Real Number","size":16.0,"checksum":23381,"address":524262}]},{"name":"X","type":"Real Number","size":16.0,"checksum":59472,"address":524204},{"name":"IOPAR","type":"List","size":29.5,"checksum":8861,"address":524153}],"changes":"5E51808715E428BB"}
{"name": "flags", "arguments": {}}
{"system":["0000000000000FF0"],"user":["0000000000000040"],"set":[-5,-6,-7,-8,-9,-10,-11,-12,7]}
```

Type, size and checksum are what the calculator's own directory listing
(`G D`, `list_vars`) and `BYTES` report: the size counts the name, the
checksum is the CRC of the object. The 49G has two flag words of each
kind (`RCLF` order: system 1, user 1, system 2, user 2). `changes` moves
whenever a variable, the current directory, the stack or a flag changes.
The same API (`saturnus_objects::ram`: `memory_tree`, `current_path`,
`stack_objects`, `flags`, `change_counter`) also reads the stack; that
read is only meaningful with the server stopped, because the ROM's saved
stack is the server's own while it runs, so it is not an MCP tool yet.
On the 49G in algebraic mode (its default) the stack holds the algebraic
history next to the results; RPN mode (`-95 CF`) shows the plain stack.

### Server mode

The ROM's Kermit server owns the keyboard while it runs. The semantic
tools start it when needed: they press ON (which clears a half-typed
command line) and type `SERVER`. They stop it again afterwards (Kermit
FINISH), so the screen shows the stack, unless `keep_server` is `true`.
`press_keys` and `type_text` stop a running server themselves and say so.
The raw Kermit tools (`read_stack`, `run_command`, `send_object`,
`receive_object`) still need `start_server` or `boot` with `autostart`.

Each Kermit transaction costs about 1.4 s of emulated time at the ROM's
pace. Wall time is small; measured on an Apple M1 Pro,
`eval "2. 3. +"`:

| Call | Emulated time | Wall time, release | Wall time, debug |
|------|---------------|--------------------|------------------|
| entering, eval, leaving | 18-21 s | 0.2 s | 4 s |
| eval with the server kept | 6-9 s | 0.07 s | 1.3 s |
| leaving after a kept eval | +4-5 s | 0.02 s | 0.5 s |

Use `keep_server: true` for a batch of semantic calls and leave it off on
the last one. Emulated time matters for the calculator's clock (`TICKS`)
and nothing else.

`timeout_ms` bounds an evaluation's emulated time, 1000 to 600000 ms. It
counts from the calculator's receipt of the command to the start of its
reply, so it includes the server's own handling: 0.3-0.45 s for a trivial
command, more with a deep stack, whose display the reply carries. On
the limit the calculator is interrupted with ON, which also ends its
Kermit server (ROM behaviour). The tool then enters server mode again to
look: when ON stops the 48SX while it is still compiling the command,
the ROM puts the text back on level 1 as a string, and the tool drops
it. The tool error says what happened; whatever the evaluation itself
had pushed stays on the stack. The 49G computes integer
literals exactly or symbolically, which can take minutes: write reals
with a dot (`2.`), or raise `timeout_ms`. The other semantic tools'
commands have a 60 s limit. The 38G, 39G and 40G have no Kermit server:
the semantic tools return "no Kermit server on this model".

### Keys and limits

`press_keys` takes the key script format of the CLI ("Key scripts" below).
A line may also hold several key names separated by spaces, and `+ - * /
.` name the plus, minus, multiply, divide and point keys, so
`6 enter 7 * enter` is a valid one-line script.

Limits:

- Time only passes while a tool runs; the calculator is frozen between
  calls, so its clock lags wall time.
- `press_keys` and `type_text` leave Kermit server mode first (about 5 s
  of emulated time). `start_server` needs the stack showing with an empty
  command line. The 38G, 39G and 40G have no Kermit server, so the stack,
  transfer and semantic tools do not work on them. On the 38G each letter is
  A...Z then its key (SHIFT first for lowercase), and space is SHIFT
  then 2. On the 39G and 40G each letter is ALPHA then its key (SHIFT
  first for lowercase), and space is ALPHA then plus.
- `type_text` refuses characters without their own key (quotes, brackets,
  `=`, `<<`...); use `press_keys` with the shift keys, or `eval`.
  Operators act like their keys: in RPN they execute at once.
- `run_command` takes one Kermit packet, about 77 encoded bytes. The
  calculator answers when the command is done; while it computes, the
  6 s reply timeout does not run, for up to 10 minutes of emulated time.
  On the 49G, integer literals are exact: `0 1 100 FOR ...` computes
  symbolically and can take minutes, so write reals (`0. 1. 100.`).
  `send_object` text in `ascii` mode is compiled by the calculator; start
  it with a `%%HP: T(3)A(D)F(.);` header so ASCII trigraphs such as `\<<`
  are translated.
- Every tool is serialised behind one session lock; errors come back as
  tool errors with the message.

## Web UI

`web/` is a static page that runs the core compiled to WebAssembly
(`crates/saturnus-web`). No framework, no bundler, no server code.

```sh
cargo install wasm-pack            # once
web/build.sh                       # writes web/pkg/ (gitignored)
cd web && python3 -m http.server 4860
# open http://127.0.0.1:4860/
```

Pick a model and a ROM file (the same files as for the CLI; the model is
switched to match the ROM size). The page runs in real time from
`requestAnimationFrame` and shows the LCD with its six annunciators, the
contrast as pixel darkness, and the calculator drawn as a vector skin per
model (48SX, 48GX, 38G, 49G, 39G; the 40G uses the 39G drawing with its own
name): the case, the display window around the LCD, every key with its
cap colour, the shifted labels above it in the model's shift colours and
the alpha letters where the model prints them. The skins are our own SVG
drawings measured from the keyboard figures in HP's user's guides, with
colours read off photographs; no HP logo or wordmark appears, the
saturnus logo sits in its place. Untick "Drawn calculator" for the plain
button grid (on the 39G and 40G each button also shows the letter it types
after ALPHA). Click or tap the keys, or use the computer keyboard: digits,
`+ - * /`, `.`, Space, Enter, Backspace, Delete (DEL), arrows, `'`, `^`,
Escape for ON and F1-F6 for the menu keys. Run/Pause, Reset, and
Save/Load state are buttons; the status line shows the model, emulated
time and speed.

What stays in the browser: the chosen model and view (localStorage) and one saved
state per model (IndexedDB). The ROM is read locally and never uploaded or
stored, so after a reload pick the ROM again, then Load state. A state only
loads with the ROM it was saved from. See `web/README.md`.

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
(default `48sx`; the script uses the model's TUI letter map; the 38G, 39G
and 40G have no oracle and are tested by golden screens instead),
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
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus-mcp --test e2e   # MCP: 48SX, 48GX, 49G, 39G ROMs
```

The MCP e2e suite includes `ram_reads_match_kermit`: on the 48SX, 48GX
and 49G it builds a directory tree, a stack and flags over Kermit, and
checks that the RAM reads equal `G D` in every directory, the path, the
typed stack and `RCLF`, and that the change counter moves with a `STO`.

Without `SATURNUS_ROM_DIR` the e2e test is skipped. The bring-up example
`cargo run --release -p saturnus --example boot -- roms/sxrom-j --screen`
also still works; `--model 39g` picks the model when the ROM size is
ambiguous, and `--io-trace N` prints the first N CONFIG, UNCNFG, C=ID,
OUT and IN events and I/O register accesses with their values.

## Legal

Saturnus is an independent, clean-room project. It shares no code with
Emu48, x48, x48ng, saturnng or HP EMU. It does not include HP's ROM images;
you download them yourself from hpcalc.org, where HP has allowed them to be
downloaded since 2000. Not affiliated with HP. HP, HP48 and HP49 are
trademarks of HP Inc.

License: MIT, see `LICENSE` and `AI_NOTICE`.
