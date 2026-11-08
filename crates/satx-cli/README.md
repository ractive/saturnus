# satx

Transfer files between an HP 48S/SX, 48G/GX or 49G and your computer over a
serial cable, on macOS, Windows and Linux; and between an HP 100LX or 200LX
palmtop and your computer (`satx lx`). satx uses the calculator's Kermit
server or XModem, runs RPL commands on the calculator, and backs up and
restores HOME. Its output is text for people and JSON for scripts and AI
agents. Files, the view for moving files in the
[saturnus](https://ractive.ch/saturnus/) app, uses the same code.

The Kermit client is `crates/kermit-proto`, the XModem one
`crates/xmodem-proto`, the HP 100LX/200LX Filer one `crates/filer-proto`,
and the calculator layer (serial and TCP links,
listing, get and put in binary or ASCII, commands, the graphics screen
PICT, backup and restore) is `crates/satx-core`. They are tested against
recorded replies and the emulated 48SX, 48GX and 49G. The end-to-end tests
talk to the real calculator ROMs running in the saturnng emulator, in a
Docker container with the serial port on TCP (`tools/saturnng-oracle/`).
The design notes and plans are in saturnus's `kb/satx/`.

`satx-core` is also a library for other hosts. Besides the blocking API the
CLI uses, `satx_core::Machine` runs the same operations sans-I/O: fed bytes
and time, it hands out bytes, events and results. Without its default
feature `native` (and without the opt-in `saturnus`) it builds for
`wasm32-unknown-unknown`, for example for a browser with Web Serial. See
satx-core's README (`crates/satx-core/README.md`).

## Usage

Install, one of:

```sh
cargo install satx                  # from crates.io, any platform with Rust

brew trust --formula ractive/tap/satx   # once: Homebrew 5.1.15+ refuses untrusted taps
brew install ractive/tap/satx           # macOS (Apple silicon), Linux
```

```powershell
scoop bucket add ractive https://github.com/ractive/scoop-bucket
scoop install satx                  # Windows
```

or an archive, `.deb` or `.rpm` from the
[releases page](https://github.com/ractive/saturnus/releases).

On the calculator, run `SERVER` (the display shows "Awaiting Server Cmd.").
satx talks at 9600 baud; `satx info` shows the calculator's IOPAR. Then:

```sh
satx ports                              # find the cable
satx --port /dev/cu.usbserial-1410 ls   # or set SATX_PORT once
satx --port tcp://localhost:4848 ls     # the emulator
satx get PRG                            # download a variable: ./PRG.hp (binary)
satx get NOTES --ascii                  # as text: ./NOTES.txt
satx put PRG.hp                         # upload a file as PRG
satx put prg.hp --as PRG2               # under another name
satx run '6 7 *'                        # run RPL, print the stack
satx pict -o plot.png                   # the graphics screen PICT (plots)
satx backup -o home.hp
satx object inspect prg.hp              # offline: type and size of a file
satx object convert dl/x.hp --to ascii  # offline: writes ./x.txt
```

Files are written only where you say (`-o FILE`) or under a name satx picks
in the current directory: `NAME.hp` (binary) or `NAME.txt` (`--ascii`) for
`get`, a timestamped name for `backup` and `pict`, and the input's file name
with a new extension for `object convert` and `grob to-png` (not beside the
input). An existing file is never replaced without `--force`.

`get` and `put` name files as Files in the saturnus app does. Calculator
names such as `A→B` stay as they are; the characters a file system refuses
(`/ \ : * ? " < > |`), `%` itself, control characters, a trailing dot or
space and the first letter of a Windows device name (`AUX`) are written
`%XX`, one per UTF-8 byte: `satx get 'A?B'` writes `A%3FB.hp`. `put`
without `--as` reads them back and drops the extension: `satx put
A%3FB.hp` stores `A?B`. The calculator tells `A` from `a`; a
case-insensitive file system (Windows, macOS by default) does not.

A command that acts on the calculator (`run`, a REPL line, `rm`, `mv`, ...)
is sent once and never resent: if its reply does not come in time, satx
says so, and you check on the calculator before running it again.

With exactly one USB serial port, `--port` can be left out. Output is text
on a terminal and JSON when piped, in one envelope
`{"results": ..., "total": N, "hints": [...]}`; errors go to stderr as
`{"error", "hint"}` with a non-zero exit code. Hints are ready-to-run
commands, which makes satx usable from scripts and AI agents:

```sh
satx ls --json | jq -r '.results[] | select(.type == "Program") | .name'
satx ls --jq '.total'                   # jq built in, no jq needed
satx completions zsh > ~/.local/share/zsh/site-functions/_satx
```

`satx --help` lists every command, and `satx <command> --help` has the
details and examples.

`satx repl` keeps one link open and sends each line you type as RPL, then
prints the stack as the calculator displays it. Lines starting with a colon
are satx commands (`:ls`, `:cd`, `:get`, `:put`, `:rm`, `:pict`, `:info`,
`:chars`, `:help`, `:quit`), and `::` sends RPL that starts with a colon. `:put`
replaces an existing variable only with `--overwrite`. HP characters can be
typed as ASCII trigraphs in REPL lines and `satx run` (`\->` for →, `\<<` for
«, `\v/` for √); `satx chars` (`:chars` in the REPL) lists them all. Line editing and
history work on a terminal. Piped lines run without a prompt, for scripts
and agents:

```text
> 42 'X' STO
> X
1: 42
> :rm X
Deleted X (Real Number, 16 bytes)
```

For scripts and agents on real hardware, use `satx repl --json` with piped
input: one process and one link for many commands, no reconnect per call,
and no late reply from an aborted command landing on the next one.
Each input line gives exactly one JSON object on stdout, in order:
`{"stack": [...]}` (level 1 first), `{"error", "hint", "stack"}` for a
calculator error (on stdout too, unlike the other commands, so results and
errors stay in order), `{"results": ...}` for a colon command, `{}` for a
blank line and `{"quit": true}` for `:quit`:

```sh
$ printf "6 7 *\n1 0 /\nCLEAR\n:ls\n" | satx repl --json
{"stack":["42"]}
{"error":"calculator error: Infinite Result","hint":"...","stack":["0","1","42"]}
{"stack":[]}
{"results":[{"checksum":8861,"directory":false,"name":"IOPAR","size":29.5,"type":"List"}]}
```

The 49G sends each stack level only as wide as its display (20 or 22
characters), so a longer string, list, program or algebraic arrives cut.
Levels that end inside an object are listed in `"cut"` (level numbers)
with a `"hint"`, here and in `satx run`'s results (a 48 string that holds
a `"` looks the same); store the object (`'NAME' STO`) and get it as text
to see all of it: `:get NAME --ascii` in the REPL, `satx get NAME --ascii
-o -` outside it.

XModem is the alternative to Kermit on the 48G/GX and 49G (the 48S/SX has
none): `satx put prg.hp --protocol xmodem` or `satx get PRG --protocol
xmodem`. The Kermit server cannot start XRECV/XSEND, so satx puts the
program `« 'PRG' XRECV »` (or `XSEND`) on the calculator's stack, ends
server mode, prints the key that runs it (EVAL on the 48G/GX; right-shift,
then SYMB on the 49G) and waits `--start-timeout` seconds (default 60). If
you do not go on, the backspace key drops the program again. After the
transfer, start the server again: right-shift, then → on the 48G/GX; on
the 49G type `SERVER` (right-shift → can start its XModem server instead).
Kermit stays the default because it needs no key press on the calculator; XModem is for when the
Kermit server is not wanted, and on the 49G it moves 1k blocks. `--dry-run`
shows the plan without ending the server.

`satx xserv ls|get|put|eval|mem|quit` talks to the XSERV command server
of the 49G, 49g+ and 50g: type `XSERV` and press ENTER on the
calculator instead of `SERVER`. It was tested on a real 49G (ROM 1.18) on
2026-11-08, where 4 KB took 8.4 s against Kermit's 16.7 s, and on the
emulated 49G (ROM 2.15); it is not yet tried on a 49g+ or 50g. `xserv
ls` gives the sizes `satx ls` gives (the variable with its name, not
only the object). `satx info` says which server answered, and the
Kermit commands stop at once with a hint when the calculator runs XSERV
(satx sends XSERV's `M` while it waits for the line to go quiet; a
Kermit server ignores it).

`satx lx` reaches the files of an HP 100LX or 200LX palmtop through its
built-in Filer in server mode, over HP's 10-pin serial cable (or TCP). On
the palmtop: Filer, MENU, Communications, Remote Settings: Baud 19200 (or
what you pass to `--baud`), Interface Com1, Server Mode enabled; leave
Filer open and in front. Paths are DOS paths, with `/` or `\`; the drive
defaults to C:.

```sh
satx lx drives                       # A: C: D: with label and free space
satx lx ls C:/_DAT                   # a directory; also a pattern, C:/*.TXT
satx lx get C:/_DAT/PHONE.PDB        # ./PHONE.PDB, with the palmtop's date
satx lx put notes.txt C:/_DAT        # C:\_DAT\NOTES.TXT, with the file's date
satx lx put notes.txt --overwrite    # replace C:\NOTES.TXT
satx lx rm C:/OLD.TXT --dry-run
satx lx mkdir C:/WORK && satx lx mv C:/A.TXT C:/WORK
satx lx rmdir C:/WORK
```

The protocol is `crates/filer-proto` (sans-IO, wasm32). It is tested
against BSD Filer v0.3, a Unix emulation of the palmtop's Filer server, in
Docker (`tools/filer-oracle/`), and against its own test server; not yet
on a real palmtop.

Developing satx itself:

```sh
just test               # the workspace's fast suite, satx's included
just lint               # rustfmt check, clippy and cargo-deny
just satx-oracle-up     # build and start the emulated HP 48SX on tcp://localhost:4848
just satx-e2e           # end-to-end tests against it
just satx-e2e-saturnus  # the same suite against saturnus in-process (ROMs in roms/)
just satx-record-trace host "6 7 *"   # record a Kermit trace from the oracle
just lx-oracle-up       # BSD Filer (HP 100LX/200LX) on tcp://localhost:4853
just lx-e2e             # satx lx against it and the in-process server
```

Not affiliated with HP. HP, HP48 and HP49 are trademarks of HP Inc.
HP's calculator ROMs are not included; the emulator container downloads them
from hpcalc.org at build time for local use only.

License: MIT, see `LICENSE` and saturnus's `AI_NOTICE`.
