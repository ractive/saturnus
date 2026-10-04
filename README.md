# saturnus

A headless emulator of the HP Saturn-based calculators, written in Rust from
published documentation and the behaviour of the calculators' own ROMs.
Library first, with a CLI, an MCP server and UIs on top.

Targets, in order: HP 48SX, HP 48GX, HP 49G, then HP 38G, 39G and 40G.

Status: iterations 1-2 done: CPU core, disassembler, memory controller and
I/O registers; the HP 48SX ROM boots to the memory prompt. Plan and docs
live in `kb/`.

## Running a ROM

Download the HP 48SX ROM J (`sxrom-j`, 262144 bytes) from
https://www.hpcalc.org/hp48/pc/emulators/sxrom-j.zip into `roms/` (ignored by
git; never commit ROMs). Then:

```sh
SATURNUS_ROM_DIR=$PWD/roms cargo test -p saturnus --test e2e   # boot test
cargo run --release -p saturnus --example boot -- roms/sxrom-j \
    --cycles 60000000 --keys "f@40000000" --screen               # press NO
```

The boot example also takes `--trace N` (last N instructions) and
`--watch-pc HEX`. Without `SATURNUS_ROM_DIR` the e2e test is skipped.

Saturnus is an independent, clean-room project. It shares no code with
Emu48, x48, x48ng, saturnng or HP EMU. It does not include HP's ROM images;
you download them yourself from hpcalc.org, where HP has allowed them to be
downloaded since 2000. Not affiliated with HP. HP, HP48 and HP49 are
trademarks of HP Inc.

License: MIT, see `LICENSE` and `AI_NOTICE`.
