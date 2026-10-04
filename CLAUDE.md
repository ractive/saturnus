# saturnus

A headless emulator of the HP Saturn calculators (HP48 SX/GX, HP49G, later
HP38G/39G/40G), written in Rust from documentation and from the ROMs'
observed behaviour. Read `PLAN.md` before doing anything.

## The clean-room rule (non-negotiable)

Existing emulators (Emu48, x48ng, saturnng, HP EMU) are GPL community
projects and their authors would not welcome an AI-written clone. Their
sources are at `~/devel/hp-emulator-refs/` for one purpose: learning hardware
facts. Procedure: read to learn a fact, write the fact with a citation into
the wiki (`~/devel/hp-literature/`, page `emulators/<name>`), close the file,
implement from the wiki. Never have their source open while writing ours,
never mirror their structure, never copy a table. Facts about a chip are not
copyrightable; their expression is.

## Where knowledge lives

- `~/devel/hp-literature/` is the LLM wiki. Use `hyalo` from that directory.
  Hardware pages: `hardware/saturn-cpu`, `memory-controller`, `io-ram`,
  `interrupts`, `timers`, `uart`, `display`, `keyboard`, `crc`, `card-ports`,
  and the model pages `hp48sx`, `hp48gx`, `hp49g`. `questions/` lists open
  contradictions between sources; resolve them by experiment and answer them
  in the wiki. The opcode table is in `raw/saturn-hardware/` (HP's SASM.OPC
  and the Fernandes/Rechlin tutorial). Cite wiki pages in code comments as
  `wiki: hardware/timers`.
- `~/devel/hptx/emulator/` runs the saturnng emulator in Docker with the
  calculator's serial port on TCP 4848 and `calc-keys` / `calc-screen` to
  press keys and dump the LCD. It is the differential oracle: same ROM, same
  keys, same screen expected.
- `~/devel/hptx` is the Kermit/XModem client that will talk to this emulator
  in-process and over TCP once the UART works.

## Decisions (do not re-litigate)

- License MIT with `AI_NOTICE`; public repo; GitHub user `ractive`.
- Rust, edition 2024. Crates: `saturnus` (core: CPU, bus, modules,
  peripherals, machine configs; no I/O, no UI, WASM-able), `saturnus-cli`
  (headless runner), later `saturnus-mcp` and UIs.
- First target HP48SX (Clarke chip, no bank switching, 32 KB RAM), then
  HP48GX, then HP49G, then HP38G/39G/40G.
- ROMs are never committed or shipped. `saturnus rom fetch` downloads from
  hpcalc.org after a confirmation prompt (identify as curl/Wget, never as a
  browser: the site serves a gzip bomb to fake browser user agents) and
  verifies size and a checksum. No HP logos or wordmarks in any UI chrome;
  "emulates the HP 48SX" in text is fine.
- Accuracy target: what the ROM needs, not cycle-exactness. The ROM checks
  TIMER2 running, a RAM magic word, module configuration (48S), battery and
  card switches on every interrupt; it halts with "Clock corrupted" if
  TIMER2 stops (wiki `hardware/interrupts`, `hardware/timers`).

## Test policy (important)

- Instruction tests are unit tests in the core crate: register state in,
  state out. Fast.
- One integration test binary (`tests/e2e.rs`) that boots a ROM only when
  `SATURNUS_ROM_DIR` is set, with a handful of scenarios: boot to the memory
  prompt, a key sequence producing a known screen, a Kermit exchange.
- Differential tests against the saturnng container are a script, run
  manually or in a separate CI job, limited to a few scripted scenarios.
- No fuzzing or property tests until a bug justifies one. Keep `cargo test`
  under a few seconds.

## Working style

- Milestones are done by Opus agents with a brief naming the PLAN.md
  milestone, the wiki pages to read and the acceptance criteria. The main
  session reviews. Branch per milestone, PR, the user merges.
- Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`
  (or the model that wrote it). Never commit ROMs, state files or secrets.
