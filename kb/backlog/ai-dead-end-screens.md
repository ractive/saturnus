---
title: "AI tools: name the other screens they read as unknown or as program input"
type: backlog
date: 2026-11-15
status: planned
priority: medium
tags:
  - backlog
  - saturnus
  - mcp
---

# AI tools: name the other screens they read as unknown or as program input

The sweep of [[iterations/iteration-74-eqw-screens]] (48SX J, 48GX R, 49G
2.10, observed in saturnus; raw dumps were scratch) found more screens
where `uiState` says `unknown`, or `input` for something that is not a
program's INPUT. Iteration 74 only corrected the texts: `unknown`'s `next`
now says to press CANCEL yourself and what CANCEL loses (a matrix being
built, a program in WAIT); `input`'s covers an object being edited and
says it is off the stack meanwhile. Naming the screens would let
`to_stack` close the safe ones and give each its own `next`.

Most rules below rest on the shapes of unnamed temporaries (the types of
the bound objects), seen on one ROM revision each; the ones with named
temporaries are sturdier. Check each on the 49G 1.19-6 ROM before relying
on it.

## Ranked

Items 1 to 3 were done in [[iterations/iteration-75-more-screens]]
(`interactiveStack`, `objectEdit`, `matrixWriter`, checked on 48SX J,
48GX R, 49G 2.10 and 1.19-6). Two of the sweep's rules did not hold as
written: the 49G edits a string with system binary #0 (as INPUT), so the
edit is told by the environment under the four, which binds the edited
object alone; and the 48's MatrixWriter's second binding turns into a
binary integer once a cell is filled. Items 4 to 11 remain.

1. **Interactive stack** (▲ at the stack; on the 49G also HIST). 48:
   innermost 4 unnamed (command, three system binaries) under `SavedUI`;
   49G: newest 8 unnamed, binding 1 a system binary, 3 a command, 8 a
   program. ENTER or CANCEL leaves in one press; operations already done
   stay (48SX OM 3994-3996, 48G UG 1686-1689, 49G UM 9231-9234).
   Closable.
2. **An object edited on the command line** inside a loop (▼ on anything
   but an algebraic or a matrix, ◀EDIT, ▶VISIT, ◀VIEW, the interactive
   stack's VIEW/EDIT, TOOL EDIT, the File Manager's EDIT), read as
   `input`. Innermost 4 unnamed (command, system binary #2, command,
   string); INPUT binds #0 (#1 with the V option) there. ENTER puts the
   object back, CANCEL restores the original (48SX OM 4113-4115, 48G UG
   1338-1343). The object is off the stack while it is edited. Not
   closable.
3. **MatrixWriter** (◀MATRIX, ▼ on a matrix, the interactive stack's VIEW,
   STAT EDIT; a cell being typed reads `input`). 48: 22 unnamed starting
   with two system binaries and a command; 49G: names `_IDX`, `_TAB2`.
   ENTER with nothing typed puts the matrix on the stack (48G UG
   4108-4120); CANCEL drops a new matrix and restores an edited one
   (observed). Not closable.
4. **49G viewer** (TOOL VIEW, the interactive stack's VIEW, the File
   Manager's VIEW, a form's HELP): newest 8 unnamed (command, command, a
   #02A2C object, command, command, ...). Closable.
5. **A program in WAIT** (0 or -1): innermost 1 unnamed (a command), under
   `SavedUI` on the 48. The next key is the program's answer (48G AUR
   WAIT); CANCEL stops the program. Not closable.
6. **Full-screen browsers**: the 48GX's Variable Browser, alarm and flag
   browsers, the 49G's alarm browser (48GX: the browser's 22 unnamed, no
   framed box; the environment under `SavedUI` tells which; 49G: 22
   unnamed and `ChooseSmall?`). ENTER/OK and CANCEL close. Whether the
   flag browser's CANCEL drops toggled flags is not tested.
7. **49G File Manager**: names `FPData`, `FPPath` (the tree: an 8-unnamed
   environment holding the string "File Manager"). CANCEL leaves in one
   press (49G UM 5569-5590, 5688-5697). Closable.
8. **48SX catalogs** (equation, statistics, alarm; one shared shape, 8
   unnamed): ENTER copies the entry to the stack and stays, ATTN leaves
   (48SX OM 11410-11412, 16173, 19260, 19339). Closable.
9. **Kermit SERVER** (all three): names `PKNO`, `KP`, `EXCHP`. CANCEL ends
   server mode. Closable.
10. **48GX Equation and Constants Libraries**: names `EL1`… (`CLFlag`)
    with `BR*`, `EN*`, `PC*`. ENTER goes a level deeper; CANCEL leaves
    from any level. Closable.
11. **49G CHARS** (6 unnamed) and **the 49G's statistics results** (names
    `Q`, `T?`, `H`, `G`, `T`; 5 working objects on the stack, as in the
    hypothesis choose box before it).

## Minor

- The 48GX's CHARS reads `inputForm` (binds `IF`); CANCEL works, to_stack
  closes it.
- FREEZE and the 48SX's REVIEW leave a stale display that reads `stack`;
  run_rpl works there.

## Not tried

The 49G equation writer's views, the 48GX PORT browser, HIST in the
49G's algebraic mode, RECV/XRECV/KGET waits, INPUT with a cursor
position, ON combinations other than ON+D.
