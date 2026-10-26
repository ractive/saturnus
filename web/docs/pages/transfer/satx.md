---
title: satx
description: satx, saturnus's command-line tool for moving files between a computer and an HP 48S/SX, 48G/GX or 49G over a serial cable: installing it, starting the calculator's server, and its commands with examples.
section: transfer
order: 3
---

satx is the project's command-line tool for a real calculator on a serial cable. It talks to the calculator's [Kermit](kermit-xmodem.md) server, or to XModem on the 48G series and the 49G, and lists, copies, renames and deletes variables, runs [RPL](https://en.wikipedia.org/wiki/RPL_(programming_language)) commands, saves the graphics screen and backs up HOME. It runs on macOS, Windows and Linux, and [Files](files.md) in the app uses the same code.

satx works with the HP 48S and 48SX, the 48G, 48G+ and 48GX, and the 49G. The 38G, 39G and 40G have no Kermit server, so satx cannot drive them; use Files to serve them a folder. The 42S and the other Pioneer models have no serial port.

## Installing

Any one of:

```sh
cargo install satx                        # from crates.io, with Rust installed

brew trust --formula ractive/tap/satx     # once
brew install ractive/tap/satx             # macOS (Apple silicon) and Linux
```

```powershell
scoop bucket add ractive https://github.com/ractive/scoop-bucket
scoop install satx                        # Windows
```

There are also archives and `.deb` and `.rpm` packages on the [releases page](https://github.com/ractive/saturnus/releases).

## First connection

1. Connect the calculator with a [cable](cables.md) and a USB serial adapter.
2. Start the calculator's Kermit server: on a 48S or 48SX type `SERVER` and press ENTER; on the 48G series press right-shift, then →; on the 49G press right-shift and → quickly, without a pause. The display says "Awaiting Server Cmd.".
3. Find the port:

```sh
satx ports
```

With exactly one USB serial port, satx uses it and `--port` can be left out. Otherwise give it each time, or once in the environment:

```sh
satx --port /dev/cu.usbserial-1410 ls     # macOS
satx --port /dev/ttyUSB0 ls               # Linux
satx --port COM3 ls                       # Windows
export SATX_PORT=/dev/ttyUSB0             # then plain `satx ls`
```

satx always talks at 9600 baud, 8 data bits, no parity, which is the calculator's default. If yours is set to another speed, set it back to 9600 on the calculator (in its I/O settings, or by purging the variable IOPAR) before you start the server.

`satx info` shows the model, the ROM version, the free memory, the current directory and the calculator's I/O settings; `satx settings` shows and changes them, from the calculator's next `SERVER` on:

```sh
satx info
satx settings                             # show IOPAR and the transfer mode
satx settings --checksum 3 --mode ascii   # takes effect at the next SERVER
```

## Moving files

```sh
satx ls                                   # the current directory
satx ls HOME/GAMES                        # another one; it becomes the current one
satx get PRG                              # writes ./PRG.hp, the exact object
satx get NOTES --ascii                    # writes ./NOTES.txt, as text
satx get PRG -o backups/prg.hp
satx put PRG.hp                           # stores it as PRG
satx put prg.hp --as PRG2                 # under another name
satx put PRG.hp --overwrite               # replace an existing PRG
satx --dir HOME/GAMES put TETRIS.hp       # into another directory
```

- `get` writes `NAME.hp` (binary, with the `HPHP48-` or `HPHP49-` header) or, with `--ascii`, `NAME.txt` (the calculator's text, with the `%%HP:` header). It never replaces a file unless you add `--force`.
- `put` sends a binary file as it is and a `%%HP:` text file in text mode, where the calculator compiles it. A file with neither header arrives as a string. A name in use is refused unless you add `--overwrite`; satx then sends the file under a temporary name first, so a failed transfer leaves the old variable alone.
- Calculator names with characters a file system refuses are written `%XX`: `satx get 'A?B'` writes `A%3FB.hp`, and `satx put A%3FB.hp` stores `A?B` again.
- `--dry-run` on `get`, `put`, `rm` and `restore` shows what would happen and changes nothing.

## Changing things on the calculator

```sh
satx mkdir GAMES
satx mv PRG OLDPRG                        # the new name must not exist
satx rm OLD TMP --dry-run
satx rm OLD TMP                           # a directory goes with its contents
```

## Running RPL

`satx run` sends RPL to the calculator and prints the stack as the calculator shows it:

```sh
satx run '6 7 *'
satx run -35 SF                           # negative numbers are RPL, not options
satx run '\<< 1 2 + \>> EVAL'             # ASCII trigraphs for « and »
```

HP characters can be typed as Unicode or as trigraphs (`\->` for →, `\<<` for «, `\v/` for √); `satx chars` lists them all. A command is sent once and never resent: if its reply does not come in time, satx says so, and you check on the calculator before you run it again.

`satx repl` keeps the link open and runs each line you type. Lines that start with a colon are satx's own commands: `:ls`, `:cd`, `:get`, `:put`, `:rm`, `:pict`, `:info`, `:help`, `:quit`.

```text
> 42 'X' STO
> X
1: 42
> :rm X
Deleted X (Real Number, 16 bytes)
```

## The graphics screen, backups

```sh
satx pict -o plot.png                     # PICT, the graphics screen: plots and drawings
satx backup -o home.hp                    # all of HOME in one file
satx restore home.hp                      # replaces everything in HOME; asks first
satx finish                               # end the server on the calculator
```

`backup` uses the calculator's ARCHIVE, so the calculator needs free memory for a copy of HOME. Turn off the clock display first: a ticking clock can damage the archive. `restore` puts the backup back with RESTORE; the calculator warm-starts and leaves server mode, so run `SERVER` on it again afterwards.

## XModem

On the 48G series and the 49G, `get` and `put` can use XModem instead of Kermit:

```sh
satx put prg.hp --protocol xmodem
satx get PRG --protocol xmodem
```

The calculator's Kermit server cannot start XModem, so satx ends server mode and tells you what to type on the calculator (`'PRG' XRECV` or `'PRG' XSEND`, then ENTER). It waits 60 seconds for it, or as long as `--start-timeout` says. Afterwards, type `SERVER` on the calculator again. On the 49G, XModem moves 1 KB blocks. Kermit stays the default because it needs no typing on the calculator.

## Files on the computer

Some commands need no calculator:

```sh
satx object inspect prg.hp                # header, type, size of a transfer file
satx object convert x.hp --to ascii       # writes ./x.txt
satx grob to-png PIC.hp                   # a GROB file as a PNG
```

## For scripts

On a terminal satx prints text; piped, it prints JSON in one envelope, `{"results": ..., "total": N, "hints": [...]}`, with errors on stderr and a non-zero exit code. `--jq` filters it without [jq](https://jqlang.org/) installed:

```sh
satx ls --json | jq -r '.results[] | select(.type == "Program") | .name'
satx ls --jq '.total'
printf "6 7 *\n:ls\n" | satx repl --json  # one JSON object per line
```

For tests without hardware, `--port` also takes `tcp://host:port` (an emulator with its serial port on TCP) and `saturnus://48gx@/path/to/rom`, which boots saturnus's emulated 48SX, 48GX or 49G inside satx for one command.

`satx --help` lists every command, and `satx <command> --help` has the details and more examples.

## Sources

- satx's own help (`satx --help`) and its [README](https://github.com/ractive/saturnus/tree/main/crates/satx-cli): the commands, their options and examples.
- [HP-48 Kermit Hints and Tips, the Kermit Project](https://www.columbia.edu/kermit/hp48.html): the server's defaults, commands run by the server.
- [HP 48G Series User's Guide](https://literature.hpcalc.org/community/hp48g-ug-en.pdf): ARCHIVE to the computer and the ticking clock (27-12).
- [HP 48 FAQ 4.62](https://www.hpcalc.org/hp48/docs/faq/48faq.html), edited by André Schoorl: starting the server on the G series (4.19), XModem on the 48G and the 49G (2.14, 10.1).
- [Conn4x help, by Bill Graves](https://www.hpcalc.org/details/5457): the 49G's keys for the Kermit server.
