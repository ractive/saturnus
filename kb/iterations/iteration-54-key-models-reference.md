---
type: iteration
title: "Iteration 54: command reference and running for the key-driven models"
date: 2026-10-19
status: completed
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

- **Data** (as built on the 42S; the full reference is the doc comment
  of `crates/saturnus-refgen/src/keyed.rs`): one file per key-driven
  model, `crates/saturnus-cli/data/commands/keys-<model>.json`:
  `{ model, manual, menu_keys, marks?, ready?, leave?, learn?, alpha?,
  xeq?, commands: [{ name, long?, description, stack?, param?, keys |
  xeq, menu?, page, examples? }] }`. A step is `{key, label?}`: the
  key's script name (the skin's) and what it shows, its legend (the
  shifted legend after a shift) or the label of the menu box above a
  menu key; every step but a shift has a label, and the keys read as
  the labels: `⇧ CONVERT →POL`. `keys` run the function from the
  normal state (as booted and `ready` leaves it: no menu, no message,
  no entry). A function reached only by name has `xeq: "NAME"` instead:
  `xeq.start`, `alpha.start`, the name's characters from `alpha.chars`
  (each from the ALPHA menu, which it returns to), `xeq.end`.
  `menu_keys` are the keys under the six boxes; `leave` the keys that
  leave one menu level and the dots that show a menu is open
  (`menu_box`), for the app; `param` what the function asks for after
  its keys; `page` the manual's PDF page; `examples` the keys pressed
  before the function's.
- **Examples** are generated on the emulator by `saturnus-refgen keys`
  into `examples-<model>.json` (the input keys, the display before and
  after as text lines). It also checks the whole file: every label
  against the skin's legend or the menu box the display shows, every
  name XEQ runs against `xeq.unknown`, `leave.menu_box` against the
  display. The display is read with fonts learned at run time (the
  `learn` screens and the `alpha.chars` typed one by one); no font is
  stored. `saturnus-refgen screen --press "KEYS"` prints the display for
  writing a file. A model without its ROM keeps the committed examples
  (the ROM-gated regen test skips it).
- **Running**: Search's verb for a key-driven model presses the keys
  through the key queue (`typeKeys`, not `insert`/`run` text). Where
  the display shows a menu (the top row is then that menu's keys) it
  first presses `leave.keys` until none shows; otherwise nothing extra.
  "Try it" presses `leave.keys` once more (ending an entry), then the
  example's and the function's keys. A function that needs arguments
  says so (stack, "Asks for") and runs on what the calculator holds, as
  a key press would.
- **Search texts**: the "no command reference" messages go for these
  models; the reference rows show the keys ("⇧ CONVERT → POL").
- `saturnus ref --model 42s NAME` works the same way.
- **38G, 39G, 40G** (HOME's edit line). The typing engine
  (`saturnus-host/src/typing.rs`) knows the command line's RAM layout
  of the 48SX, 48GX and 49G only; for the others `commandLine()` refuses
  ("no RPL command line"), so Search cannot type text. Of the two ways
  left, (a) pressing the function's keys from HOME and (b) typing its
  name letter by letter through ALPHA, (a) is chosen: it is how the
  guides tell a person to do it, it reads well (`MATH P P ▶ ENTER`), and
  it needs no name parsing (∂, Σ, →HMS, ⁻¹ have no letters to type). A
  function's keys are its key (SIN, ⇧ x⁻¹), or MATH (CMDS for a
  programming command: ⇧ MATH on the 39G, MATH then the CMDS menu key on
  the 38G), the category and the entry, then ENTER. MATH always opens on
  Real and CMDS on Aplet (checked on the emulator), and a letter key
  moves to the next entry starting with it (wrapping; leading signs are
  skipped: %TOTAL by T), so the path is the same from any state; where
  a letter cannot reach an entry (∂, ΔLIST, the 39G's ⁻¹ for INVERSE)
  the path uses ▼. The keys put the function on the edit line
  (`inserts`: `COMB(`, `Ans!` from an empty line); the person adds the
  arguments and presses ENTER, as on the calculator. The format gains
  `home` (keys before every function: HOME), `inserts`, and an
  example's `then` (keys after the function's: the arguments and
  ENTER). Search closes a choose box first (`leave`: CANCEL while the
  box's top border shows on row 4), presses HOME, then the keys; "Try
  it" also clears the edit line (CANCEL). Not in the references: the
  aplets, the program constants (CONS), the 40G's CAS (its own manual),
  the 39G's DISPXY (the guide does not describe it), and the menu
  entries listed twice (one entry per name: END, EDITMAT, FREEZE,
  REPLACE, SUB, SIZE, `=`). `saturnus-refgen keys` reads HOME
  (`apletext.rs`): the four history lines and the edit line, fonts
  learned from the `learn` screens, the `inserts` and the edit line
  echoed into the history; the 40G's file is the 39G's.

## Tasks

- [x] Data format, generator, `commands-json.py`, `reference.js`,
  `reference.rs` and Search's running for key-driven models, built on
  the 42S
- [x] 42S reference: every function of the manual's operation index
- [x] 27S, 17BII, 17B, 19BII references
- [x] 28S reference (RPL pipeline, Reference Manual)
- [x] 38G, 39G, 40G references and running
- [x] Search texts and tour/README mention the new models
- [x] Page test: a 42S function chosen in Search runs (ROM-gated)
- [x] Gates, review, merge, deploy

## Done means

Search on the 42S, 27S, 17BII, 17B, 19BII, 28S, 38G, 39G and 40G lists their functions
with description, keys and manual page; choosing one on a running 42S,
27S, 17BII, 17B, 19BII, 38G, 39G or 40G runs it and the display shows the
result; `saturnus ref` answers for these models; `commands-json.py
--check`, the web suite and the cargo gates pass.
