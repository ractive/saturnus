---
type: iteration
title: "Iteration 54: command reference and running for the key-driven models"
date: 2026-10-19
status: in-progress
tags:
  - iteration
  - web
  - reference
branch: iter-54/key-models-reference
---

# Iteration 54: command reference and running for the key-driven models

Owner, 2026-10-19, on Search's "The HP 42S has no command reference and no
command line to type into": "do the 42S reference and try the 'running'.
Also do it for the 27S and 17BII (and others?)."

Today Search and `saturnus ref` know the 48SX, 48GX and 49G only
(`crates/saturnus-cli/data/commands/`, `scripts/commands-json.py`,
`web/commands.json`, `web/reference.js`, `crates/saturnus-cli/src/reference.rs`).
Search runs a command by typing its text into the RPL command line
(`web/palette.js`, the protocol's `insert`/`run`). The 42S, 27S, 17BII,
17B and 19BII have no such command line: a function is a key, a shifted
key, a menu key under a menu, or (42S) XEQ with a name.

## Scope

- **42S, 27S, 17BII, 17B, 19BII**: a reference per model and "running":
  choosing a function in Search presses its keys on the running
  calculator.
- **28S**: RPL with a command line that Search can already type into; it
  gets a reference in the existing RPL pipeline (its Reference Manual).
- **38G, 39G, 40G** (owner: "and others"): a reference from the user's
  guides; running goes through their HOME edit line (algebraic, as
  `SIN(45)`) or the keys, whichever Search can drive. Text extracted
  2026-10-19: `hp39g40g-ug-en.txt` (the PDF's text layer) and
  `hp38g-ug-en.txt` (tesseract OCR of the scan at 300 dpi; key glyphs
  are mangled, so key names come from the skins, and the 39G text is the
  better read where the two models share a function).

## Sources

Owner's manuals as text, block N = PDF page N, in
`~/devel/calculator-knowledgebase/raw/manuals/text/`: `hp42s-om-en.txt`,
`hp27s-om-en.txt`, `hp17bii-om-en.txt`, `hp17b-om-en.txt`,
`hp19bii-om-en.txt`, `hp28s-om-en.txt`, `hp28s-rm-en.txt`,
`hp38g-ug-en.txt`, `hp39g40g-ug-en.txt`; their wiki
pages `wiki/sources/<id>.md` (page offsets, public URLs). Descriptions
are written in our own words (kb/docs/writing-style.md); never copied.
Key names come from the skins (`crates/saturnus-host/src/skins/`).
Clean room: never open `~/devel/hp-emulator-refs/`.

## Design

- **Data**: one file per key-driven model,
  `crates/saturnus-cli/data/commands/keys-<model>.json`:
  `{ model, manual, commands: [{ name, long?, description, stack?, keys,
  menu?, page, examples? }] }`. `keys` is the key names pressed from the
  calculator's normal state (menus closed, no entry in progress) to run
  the function; `menu` the menu path as the manual names it; `page` the
  PDF page. A function reached only by XEQ and a name has `keys: null`
  and `xeq: "NAME"`.
- **Examples** are generated on the emulator, as for the 48 (a key
  script, the display before and after), by a generator next to the
  existing one; a model without its ROM in `SATURNUS_ROM_DIR` keeps the
  committed examples.
- **Running**: Search's verb for a key-driven model presses the keys
  through the key queue (the protocol's key presses, not `insert`/`run`
  text). First it brings the calculator to the normal state (EXIT/CLEAR
  as the model needs) only where the function needs it. A function that
  needs arguments says so in its row (stack or inputs) and runs on what
  the calculator holds, as a key press would.
- **Search texts**: the "no command reference" messages go for these
  models; the reference rows show the keys ("⇧ CONVERT → POL").
- `saturnus ref --model 42s NAME` works the same way.

## Tasks

- [ ] Data format, generator, `commands-json.py`, `reference.js`,
  `reference.rs` and Search's running for key-driven models, built on
  the 42S
- [ ] 42S reference: every function of the manual's operation index
- [ ] 27S, 17BII, 17B, 19BII references
- [ ] 28S reference (RPL pipeline, Reference Manual)
- [ ] 38G, 39G, 40G references and running
- [ ] Search texts and tour/README mention the new models
- [ ] Page test: a 42S function chosen in Search runs (ROM-gated)
- [ ] Gates, review, merge, deploy

## Done means

Search on the 42S, 27S, 17BII, 17B, 19BII, 28S, 38G, 39G and 40G lists their functions
with description, keys and manual page; choosing one on a running 42S,
27S, 17BII, 17B, 19BII, 38G, 39G or 40G runs it and the display shows the
result; `saturnus ref` answers for these models; `commands-json.py
--check`, the web suite and the cargo gates pass.
