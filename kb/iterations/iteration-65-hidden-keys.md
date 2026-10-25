---
type: iteration
title: "Iteration 65: the 48GX's unprinted key functions on the skin, and the ON combinations"
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
   modifier is released: ⌥ shows SERVER above ▶. The skin's label fitting
   keeps it inside the slot; where the slot is taken or the text does not
   fit, the function stays in the tooltip only.
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
