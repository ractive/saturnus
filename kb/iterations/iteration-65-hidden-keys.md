---
type: iteration
title: "Iteration 65: unprinted key functions on the skin, and the ON combinations"
date: 2026-10-25
status: in-progress
tags:
  - iteration
  - web
  - skins
branch: iter-65/hidden-keys
---

# Iteration 65: the 48GX's unprinted key functions on the skin, and the ON combinations

The owner found that → ▶ on the 48GX starts the Kermit server, which no
label on the keyboard says. A sweep of the 48GX ROM on the emulator and the
manuals gave the full list: wiki `hardware/hp48gx-keyboard` (calculator
knowledgebase PR 21, 2026-10-25). Owner 2026-10-25: show these functions on
the drawn calculator, and give the ON combinations a menu. 48GX only; the
48SX and 49G follow after a sweep of each.

## Design (owner 2026-10-25)

1. **Tooltips.** A key's tooltip lists what each shifted click does,
   printed or not: an unprinted function fills the empty line, marked
   "(not printed)" (▶: `⌃-click: SWAP`, `⌥-click: Kermit server (not
   printed)`). A printed label that misleads gets a short note
   (→ EEX: `ARG (LASTARG)`).
2. **Ghost labels while a modifier is held.** Holding ⌃ or ⌥ (Ctrl or
   Alt) already lights the labels that click reaches and dims the others.
   An unprinted function whose label slot above the key is empty appears
   there while its modifier is held, in that shift's colour but drawn so
   it does not pass for print (outlined or dashed), and is gone when the
   modifier is released: ⌥ shows SERVER above ▶. Owner 2026-10-25: the
   ghost takes the key's whole label area at full size, and the printed
   label it would share it with (SWAP above ▶, VIEW above ▼, which that
   modifier does not reach) is hidden while the modifier is held and back
   on its release; nothing is squeezed beside it. A ghost too wide at full
   size gets the skin's normal fitting for a single label. Where the
   ghost's own slot holds print, the function stays in the tooltip only.
3. **Left-shifted application menus.** ← on an application key opens its
   command menu (← 7 the SOLVE menu) while only the right-shift label
   (SOLVE) is printed. Holding ⌃ lights that label in the left shift's
   violet, as ⌥ lights it in its own green: ⌥ 7 a green SOLVE (the
   application), ⌃ 7 a violet SOLVE (its menu). No ghost label, no layout
   change. The tooltip writes the menu in words (owner 2026-10-25, not
   with a ≡): `⌃-click: SOLVE menu`, `⌥-click: SOLVE`.
4. **The ON combinations.** A long press or a right-click on the ON key
   opens a menu: Restart (ON+C, clears the stack, keeps the variables),
   Self-test (ON+D), Print display (ON+1), Reset memory (ON+A+F, asked
   first). The same four in the command palette. The side panel's Reset
   stays as it is.

Strictly data-driven (owner 2026-10-25): per model a table of
unprinted and misleading shifted functions (key, shift, label text,
kind: unprinted, note or menu; a one-shift model's one shift) and the
ON menu's chords (they differ per model); the UI knows nothing of a
model beyond its table. The other eleven models follow from a sweep of
each, written into the wiki first, then added to the tables (in this
PR or a follow-up).

## Tasks

- [x] The 48GX's unprinted and misleading shifted functions as data, each citing the wiki page
- [x] Tooltips show them, "(not printed)" or the note
- [x] Ghost labels in empty slots while ⌃ or ⌥ is held
- [x] ⌃ lights an application key's label in violet where ← opens its menu
- [x] The ON key's menu (long press, right-click) and the four palette entries
- [x] Page tests; screenshots at 1280 and 390 wide with ⌃ and ⌥ held
- [ ] Owner: hold ⌃ and ⌥ on the 48GX, read the tooltips, try the ON menu

## The other eleven models (branch iter-65b/hidden-keys-all-models)

The sweeps of the other models went into the wiki on 2026-10-25
(calculator knowledgebase PR 22): `hardware/hp48sx-keyboard`,
`hp49g-keyboard`, `hp28s-keyboard`, `hp38g-39g-keyboard`,
`hp42s-keyboard`, `hp27s-17b-17bii-keyboard`, `hp19bii-keyboard`. The
design above applies unchanged; what the 48GX did not need went into
the data model, not into the UI:

- A **menu** entry may sit on either shift and lights the printed
  application label in its shift's colour: on the 48SX the application
  names are printed under the left shift and → opens the other menu
  (→ 7 the SOLVR menu), so ⌥ lights SOLVE green; its tooltip text
  names that menu ("SOLVR menu of the current equation").
- A **held** entry is a function of a shift held down while the key is
  pressed (the 49G). A modifier click taps the shift and the skin has
  no gesture for a held one, so it is a tooltip line only ("With the
  left shift held down: MODES menu (not printed)"), with no ghost and
  no glow.
- No ghost where the key has no label slot of usable size (review of
  PR 240): at least twice the skin's label size free above the key, up
  to the nearest key above it, decided from the skin's geometry
  (`ghostFits`). The 49G's cursor pad has none (XSERV and REVIEW were
  drawn tiny between the arrows), so its functions stay in the
  tooltips.
- An unprinted function that works only in one context (a command
  line, a list, the edit line) has a tooltip line and no ghost; one
  named elsewhere on the face (the 38G's SETUP bracket) says so in
  place of "not printed".
- An ON combination has a `chord` as the menu writes it (ON+F3,
  EXIT+√x, ON+menu keys 1 and 6) and an optional `hold` (the 48SX's
  ON+A+F, held a second of emulated time); its keys always go down in the same instant,
  which the Pioneers' memory clear needs (tried on the 42S, 17BII and
  19BII ROMs). The menu is named after the key's print (EXIT on the
  42S, CLR on the 27S, 17B and 17BII). Every memory clear asks first,
  and so does the 49G's tests menu, whose test 9 (FROM Format) formats
  the flash. Whether the 48SX's ROM then shows "Try To Recover
  Memory?" is not checked: `uiState` reads that screen as the stack.

Left out, as the wiki gives no way out or no visible result: the
self-tests of the 38G, 28S and the Pioneers other than the 42S, ON+F5
and ON+F6 on the 49G, ON+1 on the 38G, 39G and 40G (serial port only);
the 40G's CAS menu key is no shifted function.

- [x] Every model's table and ON combinations, each citing its wiki page
- [x] Held-shift and context-only entries as tooltip lines
- [x] Unit tests per model against the real skins, page tests (48SX, 49G, 38G, 19BII, 42S) and ROM checks per family
- [x] Screenshots with a modifier held: 48SX, 49G, 38G, 19BII at 1280
- [ ] Owner: hold ⌃ and ⌥ on the 48SX, 49G and a one-shift model, try the ON menus
