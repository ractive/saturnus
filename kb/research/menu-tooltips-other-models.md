---
title: "Menu-key tooltips on the key-driven models and the 28S"
type: research
status: completed
date: 2026-10-25
tags:
  - ux
  - pioneer
  - reference
---

# Menu-key tooltips on the key-driven models and the 28S (research, 2026-10-25)

Question: what would it take to give the menu keys of the 42S, 27S, 17BII,
17B, 19BII, 38G, 39G, 40G and 28S a tooltip that names the command shown
above each key, with its full name, as
[[iterations/iteration-66-menu-key-tooltips]] does for the 48SX, 48GX and
49G ("EXPA" → "EXPAN · Expand products").

Method: the app's code, `web/commands.json`, the wiki's model pages, and
black-box runs on the emulator (keys pressed, RAM read back, the LCD saved
as PNG). No ROM code was read or disassembled. The probe was a throwaway
test in the scratchpad (`menuprobe.rs` next to the 2026-10-25 key sweeps),
not committed. ROMs: `hp42s.rom`, `hp27s.rom`, `hp17bii.rom`, `hp17b.rom`,
`hp19bii.rom`, `hp28s.rom`, `38G_A167.ROM`, `rom.39g`.

## Answer in short

| Models | Which menu shows | Labels | Full names | Effort | Recommendation |
|---|---|---|---|---|---|
| 42S, 27S, 17BII, 17B, 19BII | RAM: a buffer of the six label texts | in the buffer, as text | `keyModels` in commands.json: 65-83 % of the labels have a full name, all have a description | small | do it, next after iteration 66 |
| 28S | RAM: menu number and first label shown | a table menu → labels, generated once on the emulator or taken from the 28S reference manual | commands.json: 211 of 260 commands have a full name | medium | do it after the Pioneers |
| 39G, 40G | RAM: the 49G's menu pointer (#807ED) | a list of label strings in some views, a program in others | none: commands.json has no view labels | medium to large | defer |
| 38G | not found in a quick probe | not in RAM as text | none | large | defer |

Reading labels off the LCD and tracking key presses in the page are both
worse than reading RAM on every model where RAM works (see below).

## 1. How the menus show

- **42S** (131 × 16 dots, two lines): a menu replaces the upper line with
  six boxes over the top row of keys (Σ+, 1/x, √x, LOG, LN, XEQ), which act
  as menu keys while a menu shows and as their printed functions when none
  does. Menus have rows of six; ▲▼ (the annunciator) step through the
  rows. Submenus nest (BASE → LOGIC). Toggle labels get a ■ after the
  label, or in place of its last letter (`FIX■`, `ALL■` for ALLΣ).
  CATALOG, the ALPHA menu and the XEQ catalog use the same row.
- **17BII, 17B, 19BII**: the menu keys are blank (printed ^); a menu
  always shows, MAIN at the top (FIN BUS SUM TIME SOLVE, and TEXT on the
  19BII). Menus are trees per application (FIN → TVM → OTHER → AMRT);
  MORE pages through longer menus. The 17BII and 17B ROMs ask for a
  language on a cold start (the reference presses ENGL); the 19BII too.
  The 19BII shows three text lines over the label line (137 × 32 dots).
- **27S**: like the 17BII, but the top row has printed functions (√x, x²,
  eˣ, LN, yˣ, 1/x) that work when no menu shows; menus come from the
  shifted digit keys (TVM, TIME, MODES, ...).
- **28S** (137 × 32 dots, four lines): RPL. Menus are chosen with the
  shifted letter keys (ARRAY to PLOT), TRIG/LOGS, SOLV/PLOT, USER (the
  variables) and the cursor key; NEXT and PREV page through them; six
  inverse boxes on the bottom line. ATTN leaves the menu shown.
- **38G, 39G, 40G** (131 × 64): each view of each aplet (HOME, the aplet
  library, SYMB, PLOT, NUM, the MATH box, editors) has its own menu row:
  EDIT ✓CHK X SHOW EVAL, SAVE RESET SORT SEND RECV START, ZOOM TRACE ...
  These are actions of the view, not functions; the functions live in the
  MATH box, which the command reference covers (70 of the 38G's commands
  go through it).

## 2. Knowing which menu shows without pressing keys

### (a) RAM

None of these addresses was in the wiki. All were found by pressing keys
and diffing RAM (2026-10-25); each holds across two different key paths
to the same menu.

**Pioneers: a label buffer.** The five Pioneer ROMs keep the six label
texts in RAM, one entry of 11 nibbles per key: a length nibble, then up to
5 characters, low nibble first. Start of the buffer:

| Model | Buffer | Checked menus |
|---|---|---|
| 42S | #5033C | CONVERT (both rows), STAT (both rows), DISP, PROB, BASE, BASE LOGIC, MODES, CATALOG, CATALOG FCN (two rows), ALPHA (two levels), XEQ catalog |
| 17BII | #503D2 | MAIN, FIN, FIN TVM, TVM OTHER, BUS, TIME |
| 17B | #503D1 | the same menus |
| 19BII | #C4376 | the same menus (BUS has CURRX and UNITS) |
| 27S | #503DF | TVM, TVM OTHER, TIME, MODES |

The buffer holds the label as the reference writes it (`ALLΣ` where the
screen shows `ALL■`, `ICNV`, `AGRA` for AGRAPH), so a toggle's mark does
not get in the way. Characters are in each ROM's character set: 0x0F is →
and 0x05 is Σ on the 42S; a small map of the non-ASCII codes is needed
(the owner's manuals list the character sets).

The buffer is **not cleared** when the menu goes away: after EXIT the 42S
keeps the last labels, and the 27S's buffer holds ALPHA labels while no
menu shows. So the tooltip must first check that a menu shows. Two ways:
the LCD (the six boxes' border rows are dark; `has_menu` in
`crates/saturnus-refgen/src/lcdtext.rs` and `labelsShown` in
`web/keys28.js` already test this) or, on the 42S, #50201, which reads 0
with no menu and the submenu depth otherwise (1 for BASE, 2 for BASE
LOGIC). The LCD test works on all five and also catches a blank or
undrawn box (an empty buffer entry).

The 42S also keeps a pointer to the menu's definition at #50202 (5
nibbles, one per menu: CONVERT, STAT, DISP, PROB and BASE all differ), the
parent's after it for a submenu (#50209), and #50207 changes from 1 to 7
when ▼ shows the second row. Not needed for tooltips, since the labels
themselves are in RAM.

Not checked yet: the 17BII/19BII SOLVE menu of an equation's variables,
the CFLO and SUM list menus, the 19BII's TEXT menus, the 42S's variable
menus (VARMENU) and program menus. These are the first host tests to
write: if one of them draws labels without the buffer, the tooltip must
fall back for it.

**28S: menu number and page.** Two nibbles at #C0151 (low nibble first)
hold the menu's number and two at #C0154 the index of the first label
shown (0, 6, 12, wrapping at the menu's end). Seen: ARRAY 1, REAL 6,
STACK 7, LOGS 17, TRIG 21, USER 23; 0 at boot and in the cursor menu.
The 28S builds its labels as RPL strings in temporary memory on every
redraw (as `find_message` in `crates/saturnus-host/src/typing.rs` already
knows), but at moving addresses with stale copies, so they cannot be read
reliably. The menu's labels by number therefore need a table (section 3).

**39G (and 40G, same ROM): the 49G's menu pointer.** The 39G runs on the
49G board, and #807ED, the 49G's pointer to the menu shown
(wiki: hardware/hp48-system-ram, "Menu"), points at the 39G's menu too. In
the aplet library it points to a list in RAM of `{ "SAVE" action }`
entries whose label strings are readable as they are; in the Function
aplet's symbolic view it points to a program in RAM that builds the keys,
whose labels are not plain strings. Iteration 66's reader for the 49G
(the menu definition, `crates/saturnus-objects/src/menus.rs`) would cover
the first kind and not the second. The 40G was not probed.

**38G: nothing yet.** Its labels are not in RAM as text in any view tried
(HOME, library, SYMB, PLOT, NUM). Three 5-nibble values near #F0758 change
with every view, but what they point to holds no label strings. Finding
the 38G's menu would take more probing, and its labels would then still
come from the ROM.

### (b) Reading the labels off the LCD

The app gets every LCD frame, and the label boxes sit at fixed places
(rows 9-15 on the 42S/27S/17BII/17B, the last 7 rows on the 38G family,
rows 25-31 on the 28S and 19BII). Cutting the six boxes out is easy and
already written; turning the dots into text is not:

- The app has no font. `read_display` on these models returns the PNG and
  the annunciators only, no text.
- The refgen readers (`lcdtext.rs` for the Pioneers and the 38G family's
  menu row, `apletext.rs` for the 38G family's HOME, `hp28.rs` for the
  28S) learn each font at run time from text the calculator is made to
  show, on purpose: "no glyph table is copied or committed". Shipping the
  learned glyphs in the app would commit ROM-drawn font data, which this
  project has avoided so far; that is an owner decision under
  [[docs/clean-room-rule]], not an implementation detail.
- A cleaner variant: refgen, while it runs the reference, records a hash
  of each label box it reads together with the label's text, and the app
  looks up the hash. Hashes are not glyphs. But it only knows labels the
  reference visits, toggles (■) change the dots, and each ROM version
  needs its own table.

Worth it only where RAM fails: the 38G, and as a fallback on the 39G's
program-built views.

### (c) Tracking key presses in the page

Rejected. The menu state lives in the calculator: it survives EXIT and
re-entry (the 42S comes back to the row it left), programs change menus,
a saved state loads with any menu, and keys also arrive from the AI
tools, the palette and the keyboard, through the worker. The page would
have to re-implement each ROM's menu logic and would still drift.

## 3. Where the full names come from

- **Pioneers.** `web/commands.json` → `keyModels.<model>.commands` has,
  per command, the keys with the label each menu key shows, the menu path
  (`menu`), `long` (the full name) and our `description`. Labels on menu
  keys that are the last key of a command: 42S 164 (109 with a full
  name), 27S 129 (98), 17BII 151 (113), 17B 148 (110), 19BII 254 (212).
  Where `long` is missing the tooltip can use the description's first
  sentence or just the label. A label used in several menus usually means
  one thing; the exceptions (DATE, MIN, TIME, EXP, LOG, PRICE, NEXT: 3 to
  6 per model, none on the 42S) are told apart by the other labels in the
  row, which commands.json also has (the sibling keys of the same menu
  path). Labels that open a submenu (FIN, TVM, OTHER) have entries where
  the reference lists them; the few without one fall back to the label.
- **28S.** commands.json → `commands.<name>` with `models.28s`: 260
  commands, 211 with a full name (`name`), 226 with their menu
  (`models.28s.menus`), but not their position in it. The tooltip needs
  menu number → labels in order. Two sources: transcribe the menu maps of
  the 28S reference manual (facts, rephrased; a few hundred labels in
  about 25 menus), or let refgen show each menu page by page and read the
  labels with its learned 28S font (`hp28.rs` already reads inverse video)
  and write `menus-28s.json`, as it writes the examples. The second checks
  itself on every regeneration; the owner removed a much larger crawler
  for the 48 in PR 22 as over-engineered, but this one is a key per menu
  plus NEXT, with no MENU numbers to probe. The USER menu shows variable
  names, which cannot be named without reading the 28S's variables (not
  read today): tooltip "variable".
- **38G, 39G, 40G.** No source in the repo: commands.json covers the MATH
  functions, not the views' menu labels. The user's guides describe each
  view's menu keys; a hand-written table (label → plain description,
  roughly 60-100 labels shared across views) would be needed. Most labels
  are whole words already (EDIT, SHOW, ZOOM, TRACE, SORT, SEND), so the
  gain is small.

## 4. Effort and risk

**Pioneers (small, low risk).**
- Host: one function per model that reads the 66 nibbles at the address
  above and decodes them (a table of five addresses and the 42S/business
  character map), exposed to the page like `entryModes`, read when the
  frame changes, not every frame. A host test per ROM: press into three
  menus, compare with the labels the reference expects.
- Page: gate on the LCD boxes, look each label up in an index built from
  `keyModels` (label plus row → command), write the tooltip as iteration
  66 does.
- Risks: a menu that draws labels without the buffer (check the list in
  section 2); the int'l 17BII/17B and the 19BII in another language
  (labels do not match the English reference: fall back to the key's own
  tooltip); the buffer address could differ on other ROM versions (one
  version per model is supported today; the host test pins it).

**28S (medium).** RAM read is as small as for the Pioneers. The work is
the menu table (generator or transcription, about 25 menus), checked on
the emulator, plus label → command, and the USER menu.

**39G/40G (medium to large).** Builds on iteration 66's 49G code once it
lands: follow #807ED, read a list's label strings. Program-built menus
need either evaluating the program's structure or the LCD fallback. Names
need a new hand-written table.

**38G (large).** First find the menu in RAM (more probing) or use the LCD
hash fallback; then the same names table.

## 5. Recommendation

1. **42S, 27S, 17BII, 17B, 19BII**: one iteration right after 66, RAM
   label buffer + LCD box check + `keyModels` names. First task: host
   tests of the menus listed as not checked.
2. **28S**: the next iteration, with `menus-28s.json` generated by refgen
   (preferred over transcription because it checks itself) and verified
   against the reference manual.
3. **38G, 39G, 40G**: defer. Their menu rows are view actions with
   self-explaining names, there is no source for full names, and the 38G
   needs more black-box work first. Revisit if the owner wants them; then
   start with the 39G/40G on top of iteration 66's 49G reader.

Before any of it, the addresses above go into the wiki
(`hardware/hp42s`, `hp17bii`, `hp27s`, `hp19bii`, `hp28s`, `hp39g-40g`),
cited as observed in saturnus, per the clean-room rule.
