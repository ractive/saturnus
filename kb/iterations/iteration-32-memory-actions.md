---
type: iteration
title: "Iteration 32: The memory view's actions"
date: 2026-10-09
status: in-progress
tags:
  - iteration
  - saturnus
  - explorer
branch: iter-32/memory-actions
---

# Iteration 32: The memory view's actions

Owner (2026-10-09), on the actions of the Variables view: too many
buttons in the preview, and the directory chosen in the tree has none
(HOME never gets "Make current" where its actions are). The design was
drawn as a mockup and approved by the owner ("The memory view looks
great!"); this iteration builds it. Plus an owner request of the same
day: the divider between the tree and the list can be dragged.

## What was wrong

- **The preview head wraps.** An object had five buttons (Edit, Copy
  text, Save as file, Rename, Purge) in a box that wraps: two rows at the
  layer's desktop width, three on a phone, so the number of buttons set
  the head's height and the title got what was left.
- **The tree and the list disagree.** A directory clicked in the list had
  its head and four buttons; the same directory clicked in the tree was
  shown with a sentence and no actions. HOME is only in the tree, so it
  never had a head. "Make current" for it hid in the status line's
  "Change to this one", where the owner did not find it.

## The design

Of three options (a primary button and a "⋯" menu; a toolbar over the
list with a context menu; both plus a menu for the bar's writes) the
third was chosen: once the menu exists the context menu and the bar's
menu cost little, and a toolbar cannot say which of the view's two
selections (the directory shown, the variable selected) it acts on.

**One subject, one head.** The preview always shows a subject with a
head (title, one line of facts, one primary button, a "⋯" button): the
variable selected in the list, else the directory shown (the tree's
selection, HOME included). The head never wraps its buttons; the title
gives way. The facts lose the word "checksum" (`Program · 23 bytes · # E145h`);
a directory adds its count of variables.

**Primary and menu**, the same set wherever the subject was chosen:

| Action | Object | Directory in the list | Directory shown | HOME shown | Stack level |
|---|---|---|---|---|---|
| Edit | primary | – | – | – | primary |
| Open | – | primary | – | – | – |
| Make current | – | menu | primary | primary | – |
| Copy text | menu | – | – | – | beside Edit |
| Save as file… | menu | – | – | – | – |
| Store file here… | – | menu | menu | menu | – |
| New directory here… | – | menu | menu | menu | – |
| Rename… | menu | menu | menu | – | – |
| Purge… | menu, last | menu, last | menu, last | – | – |

- Writes are left out without writes and off (in place, so the menu
  keeps its shape) while one runs or the calculator types. Make current
  is left out for the calculator's current directory, whose title says
  "current" instead.
- A menu of at most one item is no menu: the item stands beside the
  primary (the stack level's Edit and Copy text; an object without
  writes). HOME without writes has a head and no buttons.
- Purge is last, after a rule, in the error colour, and opens the same
  question as before: a purge still takes two steps.
- The menu closes when an item runs, so "Copied" is a short status
  before the buttons, not the button's text.
- Keys on the right of the items: F2, Del, Ctrl+C (⌘C), and the edit
  shortcut when one is bound; hidden on touch screens.
- An object read after its menu opened gets its Edit and Copy text in
  the open menu.
- The directory shown has no Open, not even an inactive one: it is open
  already (a review finding; the mockup had it greyed out).

**The same menu from a row or a node.** A right-click on a list row or a
tree node selects it (a node: shows its directory) and opens its menu at
the pointer, the primary first. Shift+F10 and the Menu key do the same
under the focused row or node. F2 renames, Delete asks to purge, Ctrl+C
(⌘C) copies the text when no text is selected. No long-press on touch:
the "⋯" is always on screen.

**The bar's "New" menu.** "Store file…" and "New directory…" become one
"New" button: "Store file in DATA…", "New directory in DATA…" and a line
"Or drop files on a directory." A directory's own menu offers the same
two "here", so a directory in the list takes a file without being opened
first; its "New directory here…" field names where it creates. The
status line keeps "Show it" and loses "Change to this one".

**The menu.** `role="menu"` with a label, items `role="menuitem"` with a
roving tabindex, rules `role="separator"`, off items `aria-disabled`
and still focusable. Opening (a click, Enter, Space, ArrowDown on the
button) focuses the first item, ArrowUp the last; arrows wrap, Home and
End jump, a letter jumps to the next item starting with it. Escape
closes and gives the focus back to the "⋯" button (or the row or node)
and stops there, so the view's own Escape does not also give the keys
to the calculator. Tab, a click outside, a scroll or a resize close it.
It is `position: fixed` inside `<sat-explorer>` (no scrolling box clips
it, and the keys stay with the view), under its button, flipped above
it when there is no room below, never wider than the window.

**The divider.** The line between the tree and the list is a handle as
the layer's own edge: drag it, or move it with the arrow keys while it
has the focus, double-click for the default; the width is kept in the
page's preferences (`saturnus.treeWidth`). Neither side gets narrower
than a name (the tree 80 px, the list 160 px). Not on a phone, where the
tree keeps its default width.

## Tasks

- [x] `web/actions.js`: the action set per subject as a pure function, `web/test/actions.test.mjs`
- [x] `web/components/menu.js`: the menu (placement, keys, closing), its placement tested in node
- [x] Preview heads: one primary and "⋯" for objects, directories in the list, the directory shown and HOME; the stack level's Edit and Copy text
- [x] Context menu on rows and tree nodes (right-click, Shift+F10, Menu key); F2, Delete, Ctrl+C (⌘C)
- [x] The bar's "New" menu; "Store file here…" and "New directory here…" for any directory; "Change to this one" removed
- [x] Writes off while one runs or the calculator types; an open menu closes when a write starts
- [x] Icons in the sprite: more, copy, folder, folder-open, pin, rename, trash
- [x] The divider between the tree and the list (`web/resize.js` shared with the page's edges)
- [x] Headless-Chrome tests in `web/test/explorer.test.mjs`; the menu open in `web/test/overflow.test.mjs`
- [x] `web/README.md`
- [ ] Owner: the menu, context menu and divider in the desktop app and a desktop browser, and the menu on the phone
