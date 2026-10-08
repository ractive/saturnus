<img src="web/logo.svg" alt="saturnus logo: a planet with a ring" width="72" height="72">

# saturnus

A headless emulator of the HP Saturn-based calculators, written in Rust from
published documentation and the behaviour of the calculators' own ROMs.
Library first, with a CLI that serves the serial port and a control API,
and UIs on top.

Targets, in order: HP 48SX, HP 48GX, HP 49G, then HP 38G, 39G and 40G,
and the HP 42S.

Status: iterations 1-4 built the CPU core, disassembler, memory
controller, I/O registers, display, keyboard, card ports, save/load state,
the UART and a serial bridge. Iteration 5 added a per-model
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
HP's Lewis chip with a 131x16 display, from the owner's own ROM dump.
Iteration 17 added the control API: `saturnus run` serves the serial port
and an HTTP + JSON API on 127.0.0.1, `saturnus ctl` drives it. Plan
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

## Install

No ROM comes with saturnus; see "Getting the ROM" below.

**Web page**, nothing to install: <https://ractive.ch/saturnus/>.
Pick a ROM file from your disk; it stays in your browser and nothing is
uploaded.

**Command line** (`saturnus`), one of:

```sh
cargo install saturnus-cli          # from crates.io, any platform with Rust

brew trust --formula ractive/tap/saturnus   # Homebrew 6+: trust the tap once
brew install ractive/tap/saturnus           # macOS (Apple silicon), Linux
```

```powershell
scoop bucket add ractive https://github.com/ractive/scoop-bucket
scoop install saturnus              # Windows
```

or a prebuilt archive from the
[releases page](https://github.com/ractive/saturnus/releases):
`saturnus-v<version>-<target>.tar.gz` (Linux x86_64 and aarch64, gnu and
musl; macOS aarch64) or `.zip` (Windows x86_64 and aarch64), checksums in `SHA256SUMS`; the
natively built ones also have an SBOM and a build provenance attestation
(`gh attestation verify <archive> --repo ractive/saturnus`).

**Desktop app**: installers on the
[releases page](https://github.com/ractive/saturnus/releases), `.dmg`
(macOS, Apple silicon), `.msi` or setup `.exe` (Windows x86_64), `.deb`,
`.rpm` or `.AppImage` (Linux x86_64). They are **not signed** yet:

- macOS: open the `.dmg` and move saturnus to Applications. The first
  start is refused ("is damaged" or "cannot be opened"); run
  `xattr -dr com.apple.quarantine /Applications/saturnus.app` once in
  Terminal (or, where the message offers it, System Settings, Privacy &
  Security, "Open Anyway").
- Windows: SmartScreen says "Windows protected your PC"; click "More
  info", then "Run anyway".

**Library**: the core is the crate
[`saturnus`](https://crates.io/crates/saturnus) (no dependencies, builds
for `wasm32`), with
[`saturnus-objects`](https://crates.io/crates/saturnus-objects) (RPL
objects, user memory) and
[`saturnus-drive`](https://crates.io/crates/saturnus-drive) (key scripts,
screen dumps) on top.

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

# Serve: the serial port on tcp:4841, the control API on 4840, until Ctrl-C.
$S run --model 48sx --rom roms/sxrom-j --serve

# Disassemble ROM code.
$S disasm --rom roms/sxrom-j --at 0 --count 20
```

What makes `run` finish or serve:

- **It serves** when it has `--serve`, `--serial` or `--control`. It runs
  `--load`, cards, `--cycles`, the key script and `--autostart`, then
  serves in the foreground until Ctrl-C (SIGINT/SIGTERM) or, with
  `--exit-on-disconnect`, until the serial client leaves; then it writes
  `--screen`, `--annunciators`, `--save` and the card files. What it
  serves: `--serial SPEC` bridges the serial port (with `--serve`, by
  default `tcp:4841` on models that have one; `--no-serial` turns that
  off), and the control API runs on `--control ADDR` (with `--serve`, by
  default on port 4840; `--no-control` turns it off). So `--serve` gives
  both, `--serial` alone the bridge alone (as before the control API), and
  a 42S serves the API with `--serve` and still writes its outputs at the
  end. The machine runs paced to wall-clock time while it serves.
- **Anything else finishes** on its own, exactly as before the control
  API: it runs `--load`, cards, `--cycles` and the key script, writes what
  was asked for, and stops. `--no-serial`, `--no-control` and
  `--token-file` without `--serve` are errors.

There is no daemon: one process, one calculator, its endpoints printed at
start.

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
| `--serve` | after the key script, serve the serial port and the control API until Ctrl-C (above) |
| `--serial SPEC` | bridge the serial port (and serve; see below); with `--serve` the default is `tcp:4841` on models with a serial port |
| `--serial-remote` | allow `--serial tcp:HOST:PORT` on a non-loopback address (prints a warning) |
| `--no-serial` | with `--serve`: no serial bridge |
| `--autostart` | before serving: answer the boot prompt with NO and start the Kermit server |
| `--exit-on-disconnect` | stop when the first serial client leaves |
| `--serial-log FILE` | append the serial traffic with emulated timestamps |
| `--control ADDR` | serve the control API on `PORT` or `127.0.0.1:PORT` (and serve); with `--serve` the default is 4840, or `SATURNUS_CONTROL`; 127.0.0.1 only |
| `--no-control` | with `--serve`: no control API |
| `--token-file FILE` | the control API's token file (default below, or `SATURNUS_TOKEN_FILE`) |
| `-v` | report when each `wait-idle` became idle, and serial connections |

A card file that does not exist is created as a zeroed 128 KB card, with a
note on stderr. Cards are 1 KB to 128 KB, a power of two, two nibbles per
byte like the ROM. They are written back only with `--card-writeback`; a
saved state already contains the card contents.

The `.txt` screen is 64 lines of 131 characters, `#` for a dark pixel and
`.` for a light one, every line ended by a newline. The `.png` is 131x64,
1 bit per pixel, dark pixels black.

## Serial port and Kermit

A serving `run` bridges the calculator's wired serial port (`--serial`,
or `tcp:4841` with `--serve`), so Kermit clients
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
| `tcp:HOST:PORT` | listen on HOST; an address other than loopback (e.g. `0.0.0.0`) needs `--serial-remote` |
| `stdio` | bytes on stdin go to the calculator, its output goes to stdout |

The order is: `--load`, cards, `--cycles`, the key script, then
`--autostart`, then serving. `--autostart` answers "Try To Recover
Memory?" with NO (skipped with `--load`) and types ALPHA ALPHA S E R V E R
ENTER; the screen then shows "Awaiting Server Cmd.". When the port is
listening, saturnus prints `serial bridged on tcp:HOST:PORT` on stdout
(stderr for `stdio`), so scripts can wait for that line.

While serving, the machine runs paced to wall-clock time on its own thread
(the Tauri app's, `crates/saturnus-drive/src/runner.rs`, driving the
protocol's state machine in `crates/saturnus-host/src/protocol/` as the
browser does), which serves the bridge between its passes: 2 MHz of
emulated cycles per real second, in passes every millisecond; a CPU
asleep in SHUTDN costs nothing until a timer event or a byte from the
client wakes it. A pass runs the wall time since the last one; what it
cannot fit into its 4 ms budget, and anything beyond 100 ms (a stopped
process, an overloaded host), is dropped instead of run in a burst. Kermit
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
`--save` and the card files as usual. A CPU halt (an undefined opcode)
no longer ends the run: it is reported on stderr and by `ctl info`, and
`--trace` covers the run before serving.

The serial port has no token: it is a raw wire, as on the calculator, so
that hptx and other Kermit or XMODEM clients work unchanged. It listens on
127.0.0.1 unless `--serial-remote` says otherwise. A browser page can
still send a request to it (a no-cors `fetch`), so the bridge looks at the
first bytes of every new connection and closes it, passing nothing to the
calculator, if they start an HTTP request line (`GET `, `POST `, `PUT `,
`HEAD `, `OPTIONS `, `DELETE `, `PATCH `, `TRACE `); the one-client slot is
free again at once. Kermit (SOH) and XMODEM (NAK, `C`, SOH) are told apart
by their first byte and pass without delay. Other users on the same
machine can still connect to the port; on a shared machine, do not serve
the serial port while it matters (`--no-serial`).

```sh
# hptx's end-to-end suite against saturnus
$S run --rom roms/sxrom-j --serial tcp:4850 --autostart &
cd ~/devel/hptx && HPTX_E2E_ADDR=tcp://localhost:4850 \
    cargo test -p hptx-core --test e2e -- --nocapture
```

hptx can also run saturnus in-process, without a socket: build it with the
`saturnus` feature and open `saturnus:///path/to/sxrom-j`.

## Control API and `saturnus ctl`

`run --serve` (or `run --control PORT`) also serves a control API: HTTP/1.1
with JSON bodies on 127.0.0.1, port 4840 by default. It prints both
endpoints at start:

```text
serial bridged on tcp:127.0.0.1:4841
control API on http://127.0.0.1:4840 (token file: /home/me/.config/saturnus/control-token)
```

`saturnus ctl` is its client; it finds the API and the token by itself:

```sh
$S run --model 48sx --rom roms/sxrom-j --serve &   # or in its own terminal
$S ctl keys "wait-idle 60000" f      # boot: answer "Try To Recover Memory?" with NO
$S ctl keys "2 ENTER 3 +"            # a key script; returns when the calculator is idle
$S ctl screen                        # the screen as 131x64 text (# dark, . light)
$S ctl screen --png s.png --scale 3  # or a PNG
$S ctl stack                         # [{"type": "real", "value": 5.0}]
$S ctl type "« 1 2 + » EVAL" --run   # typed by key presses, then ENTER (48SX, 48GX, 49G)
$S ctl type "'X^2'"                  # insert at the cursor, or start a command line
$S ctl type --replace "123"          # clear the line being edited (EDIT too), type anew
$S ctl cmdline                       # the command line from RAM, cursor as │; no key pressed
$S ctl keys --down on                # hold a key (--up on releases it)
$S ctl mem read 80000 16             # nibbles through the current mapping
$S ctl mem write 80000 0F            # written as the CPU would
$S ctl snapshot get a.state          # the whole state into a file ...
$S ctl snapshot put a.state          # ... and back
$S ctl info                          # model, ROM revision and SHA-256, speed, endpoints
$S ctl cycles                        # cycles and timing counters
$S ctl model                         # clock, display size, serial port, key names
$S ctl tree                          # HOME's variables (48SX, 48GX, 49G)
$S ctl object 7A2F5                  # one variable's value, at its address from tree
```

`--json` prints the API's result as JSON for scripts. `ctl` exits
non-zero with the API's error message (and the HTTP status) when a request
fails. A second instance needs other ports: `run --serve --control 4842
--serial tcp:4843`, and `ctl --control 4842` (or `SATURNUS_CONTROL=4842` for
both).

A 504 means the command did not run and will not (it waited 90 s for the
machine), so a retry is safe; a key script that had already started is
stopped and the 504 says so. A client that disconnects withdraws its
command the same way. A 503 means nothing was queued (8 requests in
progress, or 8 commands waiting).
A busy port is refused at start with the process that holds it, where the
platform tells us (lsof or ss on macOS and Linux, netstat on Windows).

The endpoints (`web/protocol.md`, "HTTP", has the complete mapping and the
status codes): `GET /v1/screen` (JSON rows, or `image/png` by `Accept`),
`POST /v1/keys`, `POST /v1/type` (`insert`, `run`, `replace`), `GET
/v1/cmdline`, `GET`/`POST /v1/mem`, `GET`/`PUT /v1/snapshot`, `GET
/v1/info`, `/v1/cycles`, `/v1/model`, `/v1/stack`, `/v1/tree`,
`/v1/flags`. Bodies are the commands of the front end's
protocol, the one the browser's Web Worker and the desktop app speak:

```sh
T=$(cat ~/.config/saturnus/control-token)
curl -s -H "Authorization: Bearer $T" -H "Content-Type: application/json" \
    -d '{"cmd": "keyScript", "script": "2 ENTER 3 +"}' http://127.0.0.1:4840/v1/keys
# {"ok":true,"result":{"emulatedMs":1203.4,"warnings":[]},"type":"reply"}
```

Key scripts and typed text run at once in emulated time (much faster than
real time while the calculator waits for keys) and return when it is idle;
the clock then follows the wall clock again. Meanwhile the serial bridge
waits, so do not send keys during a Kermit transfer.

**How an agent uses it.** Start `saturnus run --serve` once (in the foreground, in
its own terminal or as a background job of the agent's shell), then call
`saturnus ctl` for keys, screens and state; for calculator operations
(list, get, put, run programs) use hptx over the serial port, exactly as
against a real calculator.

### Security

Listening on 127.0.0.1 alone does not keep others out: any program of any
user on the machine can connect, and a web page in your browser can send
requests to 127.0.0.1 (and with DNS rebinding make its own name point
there). So:

- **A token.** On first start `run` creates a token file with 256 random
  bits from the operating system. Every request must carry it
  (`Authorization: Bearer ...`); without it, or with a wrong one, the answer
  is 401 and nothing more. The token is compared in constant time and is
  never printed, logged or put into an error message; `run` prints only
  the file's path. `ctl` first asks the server, without the token, to
  prove that it holds it (`GET /v1/hello`), so another user who took the
  port never receives it; a script with `curl` does not check that.
  Where it lives:
  - Linux and macOS: `$XDG_CONFIG_HOME/saturnus/control-token`, else
    `~/.config/saturnus/control-token`, created with mode 0600 in a
    directory of mode 0700; a token file other users can read is refused.
  - Windows: `%LOCALAPPDATA%\saturnus\control-token`, inside your user
    profile, which only you (and SYSTEM and the administrators) can read
    by its default ACL; saturnus sets no ACL of its own.
  - `--token-file` or `SATURNUS_TOKEN_FILE` choose another file. Delete
    the file to get a new token on the next start.
- **Only its own name.** The `Host` header must be `127.0.0.1:PORT` or
  `localhost:PORT` (else 421), which defeats DNS rebinding; a request with
  any other `Origin` header (every cross-site browser request has one) is
  refused (403); `OPTIONS` is refused and no CORS header is ever sent, so a
  browser never lets a page read an answer or send the token.
- **Reads do not write.** `GET` never changes anything; keys, typing,
  memory writes and loading a state need `POST` or `PUT`.
- **Bounded.** Bodies are capped (256 KiB of JSON, 4 MiB of state), memory
  reads and writes stay inside the 20-bit address space and move at most
  65536 nibbles, a request head must arrive within 2 s and a body within
  20 s; connections still sending their head have their own budget of 16
  (the oldest is dropped for a new one), at most 8 authenticated requests
  run at once and at most 8 commands wait for the machine, each
  connection on its own thread: a slow or stalled client never holds up
  the calculator or the serial port, and idle connections without the
  token cannot lock out the token holder.
- **No files.** The API never takes a file path: states travel as bytes,
  `ctl` reads and writes the files on its side, and the ROM is the one
  given to `run`.
- **127.0.0.1 only.** `run` refuses to put the API on any other address.

The rules and their tests: `kb/docs/control-api-security.md`.

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
EXIT key (`exit` is accepted as a name for `on` on every model, as `f1`-`f6`
are for `a`-`f`); it has no `a`-`f` (its top row keeps its labels). `on` with `ln`
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

## Kermit test host

`crates/saturnus-kermit` (not published) owns one emulated calculator and
talks to its ROM's Kermit server in process, for the ROM-gated tests and
for `saturnus-refgen`: it drives the machine's serial port directly
(`Machine::serial_push`/`serial_drain`), speaks Kermit through
[`kermit-proto`](https://crates.io/crates/kermit-proto) and takes the
objects on the wire from `saturnus_objects::transfer`. Agents use the
control API and hptx over the serial port instead (above).

The link runs on emulated time only. The Kermit client's clock is the
emulated time the calculator spent idle while a reply was awaited: the
calculator answers a host command only when it is done, so time it
spends computing (not in SHUTDN) does not count toward the 6 s reply
timeout, for up to 10 minutes per read; a packet that opens a
transaction waits 200 ms of emulated time first (a command right after
the final ACK is lost). The same exchange always leaves the same machine
state, which is what lets `saturnus-refgen` regenerate its data byte for
byte.

On top of host commands, `G D`, GET, SEND and `G F` it has typed
operations (`saturnus_kermit::semantic`): `eval`
(levels 1..n typed, or the calculator's error with the stack's display
text), the typed stack, push, pop, drop, variables and directories. They
enter the server when needed (ON, then `SERVER`) and leave it unless
asked to keep it. Values are decoded from the binary object a GET
returns (wiki `protocols/hp-object-format`), so reals keep all 12 digits;
programs, algebraics and units take their text from an ASCII GET of the
same object. Levels are read without changing the stack through a
temporary list variable (`SATRNTMP`, or `SATRNTM1` to `SATRNTM3` if
taken). Source the calculator rejects as `Invalid Syntax` is tried again
as an algebraic (`SIN(0.5)` as `'SIN(0.5)' EVAL`); source longer than one
packet (77 encoded bytes) travels as a string that `STR→` compiles. An
`eval` limit counts from the calculator's receipt of the command to the
start of its reply; on the limit the calculator is interrupted with ON,
which ends its server, and the text the 48SX ROM puts back on level 1 is
dropped. The objects as JSON (`saturnus_objects::Object`):

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

## Command reference

`crates/saturnus-cli/data/commands/` holds a reference of every built-in
command of the 48SX, 48GX and 49G, generated from the ROMs on the emulator by
`saturnus-refgen` (`crates/saturnus-refgen`). In the web page and the
desktop app it is the **command palette** (Cmd/Ctrl+K: suggestions while
typing, the entry with its examples beside them, Enter sends the command
to the calculator; `web/README.md`, "Command palette") and the Commands
tab of the side layer; the page reads `web/commands.json`, folded from
these files by `scripts/commands-json.py`. `saturnus ref` looks a
command up (embedded in the binary):

```sh
saturnus ref STO --model 48sx     # description, stack effect, menu, examples, manual pages
saturnus ref '->LIST' --json      # the same as JSON, every model (menus per model)
saturnus ref '\.S'                # ∫ by the calculator's ASCII code (\-> \GS \v/ \pi ...)
```

An exact name wins. Otherwise the query may differ in case, use the
calculator's ASCII translation codes (`\->`, `\GS`, `\.S`, ...), or a
friendly spelling (`->LIST`, `SIGMA+`, `UPMATCH`) where that spelling is
no other command's name (`INT` is the 49G's `INT`, so `∫` is `\.S`). A
query that names several commands (`Qr`: `QR` and `qr`) lists them and
exits with status 2 (`--json`: `{"candidates": [...]}`). `--model`
selects that model's menus, examples and manual pages. Where a command
is: `menu:` the ROM's own menus that offer it (named by the key that
opens the root menu and the labels of the keys down to it, `MTH BASE`;
`MENU n` for a menu no key opens directly), `key:` the key or menu a
manual names, with its page, or, without one, the key whose legend is the
command's name; and only where none of these exists, `group:` our own
grouping for browsing (not a menu location). `--json` gives them per
model (`categories`), the first as `category` with its `category_source`
(`rom`, `manual`, `keyboard` or `ours`).

A stack effect marked "from the manuals, not run here" has no example
that ran the command (interactive, plotting and I/O commands).

| File | What | Made by |
|---|---|---|
| `48sx.json`, `48gx.json`, `49g.json` | The ROM's command names, with library and command numbers; the menus that offer each (`menus`) and which key opens which root menu (`menu_keys`) | `saturnus-refgen catalog`, then `saturnus-refgen menus` |
| `reference.json` | Our description, stack effect and example inputs per command, and our group where neither the ROM's menus nor a manual places it | written by hand |
| `examples-48sx.json`, ... | Each example input run on that model: the typed input stack and result, the display text, or the calculator's error | `saturnus-refgen examples` |
| `manuals.json` | The public URLs of HP's manuals and the PDF page of each command in them | `scripts/manual-pages.py` |
| `categories.json` | Per command and model, the key or menu a manual names for it (`MTH`, `PLOT`, `Keyboard`, the 49G CAS's `Arithmetic`), with the manual and page, and the ROM's menus (`menus`, as in the catalogs) | `scripts/manual-categories.py` and `saturnus-refgen menus` |

The names come from the ROM: every library number (0-7FF) is probed with
lists of XLIB names sent over Kermit and fetched back as text, so the
ROM's own decompiler prints each command's name from its library's name
table. The 48SX has its commands in libraries 2 and 700; the 48GX adds
library AB; the 49G adds the CAS libraries, the development library (256)
and the assembler (257).

The menus come from the ROM too, without pressing through them:
`saturnus-refgen menus` reads the built-in menu definitions from the ROM
image (`saturnus_objects::menus`: `MENU`'s own code leads to them; wiki
protocols/rpl-libraries, "Built-in menus") and names each menu by the key
that opens it, observed by pressing every key and shifted key once and
reading the current menu from RAM:

```sh
saturnus-refgen menus --model 48sx --rom sxrom-j --catalog crates/saturnus-cli/data/commands/48sx.json \
  --categories crates/saturnus-cli/data/commands/categories.json --out crates/saturnus-cli/data/commands/48sx.json
```

The categories come from the manuals' own statements of where a command
is found: the 48SX owner's manual's operation index (48SX), the 48G
Advanced User's Reference's "Keyboard Access" lines (48GX, and the 48SX
where its own manual is silent), and the 49G Advanced User's Guide's
"Access" lines (the 49G's computer algebra commands). The menu labels in
those scans do not read reliably, so a category is the menu key the
manual names (`MTH`, not `MTH PARTS`). A command no manual places has our
own category, marked as ours, or none. "Ours" is an editorial grouping
for browsing, not a statement of where the command's key is: `saturnus
ref` prints it as `group: X (ours; not a menu location)`, a manual's
statement as `menu: X (manual, p. N)`. The groups will be replaced by the
ROM's own menu definitions, decoded statically (shown on the 48SX in
iteration 12c: 334 of 397 commands placed), in a follow-up.

```sh
R=/path/to/roms
saturnus-refgen catalog --model 48sx --rom $R/sxrom-j --out crates/saturnus-cli/data/commands/48sx.json
saturnus-refgen examples --model 48sx --rom $R/sxrom-j --catalog crates/saturnus-cli/data/commands/48sx.json \
    --reference crates/saturnus-cli/data/commands/reference.json --out crates/saturnus-cli/data/commands/examples-48sx.json
scripts/manual-pages.py          # texts from ~/devel/hp-literature/raw/manuals/text ($HP_LITERATURE_TEXT)
scripts/manual-categories.py     # the same texts
scripts/check-similarity.py      # the same texts; all three skip with a message without them (CI)
```

A catalog takes about a minute and an examples file a few minutes in a
release build; both are deterministic, and
`cargo test --release -p saturnus-refgen -- --ignored` (with
`SATURNUS_ROM_DIR`) regenerates them all and compares byte for byte. The
ROM-gated tests that run by default compare the names and a sample of
examples in seconds.

The descriptions are ours. Command names, the manuals' categories and
stack effects are facts; HP's manual text is not copied or paraphrased.
`scripts/check-similarity.py` flags any description that shares six or
more consecutive words with the manuals' text layers (the 48G AUR, the
48G user's guide and the 48SX owner's manual from literature.hpcalc.org,
OCR text of the 49G Advanced User's Guide, which has no text layer, and
the 49G user's manual); it reports none. The manuals themselves are only
linked: `manuals.json` stores page numbers of these public copies, so a
link is `<url>#page=<n>`:

| Manual | URL |
|---|---|
| HP 48SX Owner's Manual | https://literature.hpcalc.org/community/hp48sx-om-en.pdf |
| HP 48G Series User's Guide | https://literature.hpcalc.org/community/hp48g-ug-en.pdf |
| HP 48G Series Advanced User's Reference Manual | https://literature.hpcalc.org/community/hp48g-aur-en.pdf |
| HP 49G Advanced User's Guide | https://literature.hpcalc.org/official/hp49g-aug-en.pdf |

## Web UI

`web/` is a static page that runs the core compiled to WebAssembly
(`crates/saturnus-web`, bindings over `crates/saturnus-host`). No
framework, no bundler, no server code.

```sh
cargo install wasm-pack            # once
web/build.sh                       # writes web/pkg/ (gitignored)
cd web && python3 -m http.server 4860
# open http://127.0.0.1:4860/
```

Pick a model and a ROM file (the same files as for the CLI; the model is
switched to match the ROM). Each ROM is kept in this browser, per model,
so you pick it once: selecting the model boots it, and the last model
boots when the page opens. Several files can be chosen at once, or
dropped on the page; each is recognised by its content (the images
`saturnus rom fetch` knows by SHA-256, others by size) and goes to its
model. The core runs in a Web Worker in real
time (or 2x, 4x, Max), sleeps while the calculator's CPU does, and pushes
the display to the page when it changes. The speed applies only while the
calculator computes: while it waits for a key its clock runs in real time
at any speed, so the auto-off (about ten minutes on the 48) and the
cursor blink keep their pace. The page shows the LCD with
its annunciators, the contrast as pixel darkness (each model's power-on
contrast properly dark; "Darker display" and "Lighter display" in the
panel and the palette send ON + and ON -), and the calculator drawn as a vector skin per
model (48SX, 48GX, 38G, 49G, 39G, 42S; the 40G uses the 39G drawing with
its own name): the case, the display window around the LCD, every key with its
cap colour, the shifted labels above it in the model's shift colours and
the alpha letters where the model prints them. The skins are our own SVG
drawings: the 48SX, 38G, 49G and 42S measured from the owner's photographs
of their calculators (the 48GX shares the 48SX's mould), the 39G/40G and
every model's labels from the keyboard figures in HP's manuals; no HP logo or wordmark appears, the
saturnus logo sits in its place. Choosing a model draws it at once; with
no ROM for it the display says so and offers "Choose ROM…", and a key
press makes that message pulse. Click or tap the keys, or use the
computer keyboard: letters,
digits, `+ - * /`, `.`, Space, Enter, Backspace, Delete (DEL), arrows,
`'`, `^` and F1-F6 for the menu keys; ON (Escape), α (Tab), the shifts
and the app's actions are shortcuts on physical keys that the "Keyboard
shortcuts" dialog lists and lets you change, with warnings for keys the
browser or another action takes. Reset and Save/Load state are
buttons, pausing is in the command palette; the status line shows the model and ROM,
paused or halted, a send in progress and the last message. Pasting text
(or the command palette, Cmd/Ctrl+K) types it into the command line by
key presses (48SX, 48GX, 49G).

**Memory view.** The Memory button opens a layer beside the calculator
(over it on a narrow window) that shows the calculator's variables, stack
and flags live, read straight from its RAM: the directory tree of HOME
with the current directory marked, the variables of a directory with
type, size and checksum, a typed preview of the selected object with
copy-as-text, the stack, and every system flag with what its state
means. It is read-only and sends no keys; browsing directories there
does not change the calculator's directory. 48SX, 48GX and 49G; programs
are shown as indented text, read back from memory with the ROM's own
command names. Typing still
goes to the calculator; Alt+M moves the keyboard into the layer and
Escape back. The side panel and the layer are resized from their inner
edges, and the widths are remembered. The desktop app has the same layer. See `web/README.md`,
"Memory view".

What stays in the browser: the chosen model, view, speed, panel and
memory view with their widths, changed keyboard shortcuts (localStorage `saturnus.*`), the
ROM of each model (IndexedDB `saturnus-roms`; "Forget ROMs" removes them,
and the saved 49G state, which holds the 49G's flash and so its ROM) and
one saved state per model (IndexedDB `saturnus`). Nothing is
uploaded. Where the browser refuses to store (storage blocked or full)
the page says so and works as before: pick the ROM again after a reload.
A state only loads with the ROM it was saved from. See `web/README.md`.

The page is published to <https://ractive.ch/saturnus/> by
`.github/workflows/pages.yml` (manual; see `kb/docs/releasing.md`).

## Desktop app

`crates/saturnus-tauri` is a Tauri 2 app with the same page as its front
end and the core linked natively: the machine runs on its own thread,
paced to the wall clock (1x, 2x, 4x or Max), and sends the display to the
window when it changes. The ROM and the saved states are files chosen in
native dialogs. The app remembers each model's ROM file by its path, in
`settings.json` in its config directory (macOS `~/Library/Application
Support/ch.ractive.saturnus/`, Linux `~/.config/ch.ractive.saturnus/`,
Windows `%APPDATA%\ch.ractive.saturnus\`), so selecting a model boots it
and the last model boots at start (a setting). When you choose a ROM, the
other ROMs in the same folder are recognised by content and given to
their models at once; a file that only fits by size is offered, an
unknown one is left alone. A remembered file that was moved or changed is
reported and asked for again. It speaks the page's protocol (`web/protocol.md`), so the
page does not know which host it runs on.

```sh
cargo install tauri-cli --version 2.12.1 --locked   # once
cd crates/saturnus-tauri
cargo tauri dev        # the app in development, from web/
cargo tauri build      # an installer for this OS, in target/release/bundle/
```

On Linux the app needs the system webview's development packages
(Debian/Ubuntu: `libwebkit2gtk-4.1-dev libxdo-dev libssl-dev`, and
`librsvg2-dev patchelf` for `cargo tauri build`). Plain `cargo build` and
`cargo test` leave the app out (it is not a default workspace member);
`just tauri` runs its clippy and tests, `just app` is `cargo tauri dev`.
Installers for macOS, Windows and Linux are built by
`.github/workflows/desktop.yml` (manual, unsigned for now; see
`kb/docs/releasing.md`). The app icon is drawn from `web/logo.svg`; no
HP mark appears.

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
cargo test --workspace --exclude saturnus-tauri -q   # just test
node --test web/test/*.test.mjs    # the page's object and flag functions (just web-test)
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus --test e2e   # needs the ROM
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus-kermit --test e2e   # Kermit: 48SX, 48GX, 49G, 38G, 39G ROMs
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus-cli --test e2e    # control API: 48SX, 42S
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus-refgen --test regen   # command data: names, menus, sample examples
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus-host --test typing   # typing into the command line
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus-tauri --test runner  # the desktop app's machine thread
SATURNUS_ROM_DIR=$PWD/roms cargo test --release -p saturnus-refgen -- --ignored   # all of it, byte for byte
just rom-tests $PWD/roms   # every one of them (the release checklist)
```

The control API's tests run `saturnus run` and `saturnus ctl` as
processes on ephemeral ports with a token file in a temporary directory:
`crates/saturnus-cli/tests/control.rs` needs no ROM (a 48SX on a ROM of
zeros), `tests/e2e.rs` boots a 48SX and a 42S. The refusals (no token,
wrong token, foreign Host, foreign Origin, OPTIONS, wrong methods,
oversized bodies and heads, out-of-range memory, stalled clients) are unit
tests in `crates/saturnus-cli/src/control/server.rs`.

The Kermit e2e suite (`crates/saturnus-kermit/tests/e2e.rs`) includes the
summation benchmark of "Speed" and `ram_reads_match_kermit`: on the 48SX,
48GX and 49G it builds a directory tree, a stack and flags over Kermit, and
checks that the RAM reads equal `G D` in every directory, the path, the
typed stack and `RCLF`, and that the change counter moves with a `STO`.

Without `SATURNUS_ROM_DIR` the e2e test is skipped. The bring-up example
`cargo run --release -p saturnus --example boot -- roms/sxrom-j --screen`
also still works; `--model 39g` picks the model when the ROM size is
ambiguous, and `--io-trace N` prints the first N CONFIG, UNCNFG, C=ID,
OUT and IN events and I/O register accesses with their values.

## Legal

Saturnus is an independent, clean-room project. It shares no code with
Emu48, Emu42, jsEmu48, x48, x48ng, x50ng, ui4x, saturnng or HP EMU. It does not include HP's ROM images;
you download them yourself from hpcalc.org, where HP has allowed them to be
downloaded since 2000. Not affiliated with HP. HP, HP48 and HP49 are
trademarks of HP Inc.

Saturnus was written from documentation and the ROMs' observed behaviour.
The other emulators' source code is off limits to anyone implementing
saturnus; facts about them come from their documentation and change logs
and from black-box runs (saturnng, GPL, served as an oracle: same ROM,
same keys, the screens compared), written down with citations in the
project's hardware wiki, and the code was written from the wiki. The complete list of sources (manuals, HP Journal articles,
datasheets, forum threads, emulator notes, with authors, years and what
each was used for), the skin references and the tools is in the web
page's and the app's About panel, generated from the wiki into
`web/about.json` by `scripts/about-json.py`.

License: MIT, see `LICENSE` and `AI_NOTICE`.
