# saturnus

A headless emulator of the HP Saturn-based calculators, written in Rust from
published documentation and the behaviour of the calculators' own ROMs.
Library first, with a CLI, an MCP server and UIs on top.

Targets, in order: HP 48SX, HP 48GX, HP 49G, then HP 38G, 39G and 40G.

Status: iteration 1 done (CPU core and disassembler). Plan and docs live in `kb/`.

Saturnus is an independent, clean-room project. It shares no code with
Emu48, x48, x48ng, saturnng or HP EMU. It does not include HP's ROM images;
you download them yourself from hpcalc.org, where HP has allowed them to be
downloaded since 2000. Not affiliated with HP. HP, HP48 and HP49 are
trademarks of HP Inc.

License: MIT, see `LICENSE` and `AI_NOTICE`.
