---
title: "Comments on reference examples"
type: doc
status: active
date: 2026-10-19
tags:
  - docs
  - reference
---

# Comments on reference examples

Owner, 2026-10-19: examples in Search and `saturnus ref` may carry a short
comment that says what the example shows, "but don't add obvious
explanations". [[docs/writing-style]] applies to every comment.

## When an example gets a comment

Only when someone who knows the calculator could not read the meaning off
the input and the result at a glance. Typical cases:

- A notation hides the meaning: HH.MMSS times, MM.DDYYYY dates, binary
  integers (`#FFh`), units, flags by number, list and matrix indices.
- The arguments have roles the input does not show: which value is old and
  which new (%CH), which level is the count (ROLL, PICK), start and end
  positions (SUB), what N, I%YR, PV and PMT are in a TVM example.
- Stack shuffles: what moved where.
- The example was picked for an edge: a non-integer factorial (the gamma
  function), negative decimal places in RND, a date step across a month or
  leap day, an empty list.

No comment when the example is plain: arithmetic, a function of one number
with an expected result, a conversion whose result names its unit. When in
doubt, leave it out. Most examples have no comment.

## How to write one

- One line, at most 90 characters, no full stop at the end.
- Plain words, the reader's terms: "1h30m plus 2h45m", "the 2nd to 4th
  character". British spelling.
- Say what the inputs mean and what happens, not what the command is (the
  description does that). Do not repeat the description.
- A number in the comment should come from the example's input or its
  result on every model that shows the example, or follow from them plainly
  (1.3 is 1h30m). The check script lists every number it cannot find there,
  and each listed one is read by a person. Shared examples can give
  different results on different models (the 49G's exact mode); then
  describe without the result.
- Never: "This example", "Here", "we", "simply", "just", "note that",
  exclamation marks, emoji, jokes.

## Worked examples

| Command | Example (input → result) | Comment |
|---|---|---|
| HMS+ | `1.3 2.45` → `4.15` | 1h30m plus 2h45m gives 4h15m |
| DATE+ | `7.151991 120.` → `11.121991` | 120 days after 15 July 1991 is 12 November 1991 |
| DATE+ | `3.012024 -1.` → `2.292024` | One day before 1 March 2024 is 29 February, a leap year |
| DDAYS | `12.252026 1.012026` → `-358` | The second date is earlier, so the count is negative |
| ROLL | `1. 2. 3. 4. 4.` → `1 4 3 2` (level 1 first) | Rolls the 4 levels: the 1 from level 4 comes down to level 1 |
| PICK | `10. 20. 30. 3.` → `10 30 20 10` | Copies level 3, the 10, to level 1 |
| B→R | `#FFh` → `255` | Hexadecimal FF is 255 |
| SUB | `"HELLO" 2. 4.` → `"ELL"` | The 2nd to 4th character |
| GET | `[[ 1. 2. ][ 3. 4. ]] { 2. 1. }` → `3` | Row 2, column 1 of the matrix |
| %CH | `50. 60.` → `20` | From 50 to 60 is a rise of 20 percent |
| RND | `12345.678 -3.` → `12300` | A negative count rounds to 3 significant digits |
| ! | `0.5` → `.886226925453` | A non-integer gives the gamma function's value Γ(1.5) |
| →LIST | `0.` → `{ }` | A count of 0 gives the empty list |
| SF | `3. SF 3. FS?` → `1` | Sets flag 3, then tests it: 1 means set |
| %CHG (17BII) | `BUS %CHG 100 OLD 125 NEW %CH` | From 100 to 125 is a rise of 25 percent |
| ! | `5.` → `120` | (none: plain) |
| SIN | `30.` → `.5` | (none: plain) |
| + | `7 ENTER 5` → `12` | (none: plain) |
| CONVERT | `1._ft 1._m` → `.3048_m` | (none: the result names its unit) |

## Bad comments

| Comment | Why not |
|---|---|
| Computes the factorial of 5 | Repeats the command; the example is plain |
| This example shows how HMS+ adds times | "This example", says nothing the input does not |
| 1h30m plus 2h45m gives 4h15m. Very handy! | Full stop, exclamation, opinion |
| Rolls the stack so that the element that was at the fourth level of the stack moves down | Too long, vague |
| Returns 1 | Restates the result |

## Where comments live

Each example object gets an optional `comment` field: in
`crates/saturnus-cli/data/commands/reference.json` (shared by the 48SX,
48GX, 49G and 28S) and in `keys-<model>.json` (the key-driven models; the
40G's file must stay equal to the 39G's). `scripts/check-comments.py`
fails on length and banned words, and lists the numbers it cannot find in
the generated `examples-<model>.json`, for review.
