# RPL on the 48SX, 48GX, 49G and 28S

RPL works on a stack of objects. Level 1 is the bottom; commands take their
arguments from the stack and leave results there. `2 3 +` leaves 5.

## Objects you will type

| Object | Typed as |
|---|---|
| Real number | `3.14`, `-2`, `1.5E-3` |
| Complex number | `(1,2)` |
| String | `"HELLO"` |
| Name | `'X'` (quoted: put on the stack, not evaluated) |
| Algebraic | `'X^2+1'` |
| List | `{ 1 2 3 }` |
| Vector, matrix | `[ 1 2 ]`, `[[ 1 2 ][ 3 4 ]]` |
| Program | `« 1 + »` (or `\<< 1 + \>>`) |
| Unit object (48, 49) | `5_m` |
| Binary integer | `#FFh` |

ASCII spellings: `run_rpl` and `store_variable` take the 48's character
codes (as in its ASCII transfers) in place of the characters, e.g. `\<<`
`\>>` for « », `\->` for →, `\GS` for Σ, `\v/` for √, `\pi` for π, `\<=`
`\>=` `\=/` for ≤ ≥ ≠, `\Ga` for α, `\nnn` for character nnn (128-255) and
`\\` for a backslash; the 28S takes only those it has keys for.
`lookup_command` takes the codes too, and `->LIST` for →LIST.

## Programs

A program is a list of commands between « and ». Running it runs them in
order, on the stack. Store it with `store_variable` (name and the program as
text), then run it by typing its name in `run_rpl`, after its arguments:

```
store_variable  name: SQSUM  text: « SQ SWAP SQ + »
run_rpl         text: 3 4 SQSUM          → stack level 1: 25
```

### Local variables

`→ a b « … »` takes the top two levels into local names (the last name gets
level 1). Locals keep programs readable:

```
« → a b « a SQ b SQ + √ » »        hypotenuse of a and b
```

### Structures

- `IF test THEN … ELSE … END`, and `CASE test THEN … END … END`
- `start end FOR i … NEXT` (or `step STEP`); `start end START … NEXT` without
  a counter
- `DO … UNTIL test END`, `WHILE test REPEAT … END`
- `IFERR … THEN … END` to catch an error

Tests leave 1 (true) or 0 (false): `==`, `≠`, `<`, `≤`, `AND`, `OR`, `NOT`,
`SAME`.

## Checking your work

- `run_rpl` gives back `error` (the calculator's message, or null) and the
  stack. An error leaves the stack as the command found it, on the 48 and 49
  with the arguments put back (LASTARG).
- `read_variable` shows a stored program as the calculator decompiled it:
  this is what it really holds. Compare it with what you meant.
- `store_variable` compiles on the calculator. A syntax error comes back as
  its message and nothing is stored: fix the text, do not fall back to typing
  keys.
- Test a program on known values before giving the user a result. Edge cases
  (zero, negative, an empty list) are cheap to try.
- A program that does not return: `run_rpl` reports `running: true`. Ask the
  user before pressing CANCEL (key `"on"`), which stops it.

## Display modes

Results show in the current number format: `STD` shows all digits, `2 FIX`
two decimals, `3 SCI` and `3 ENG` scientific. `DEG`, `RAD` and `GRAD` set the
angle mode; the 48 and 49 show RAD or GRAD in the header. Set them in a program
only when the user wants it, and say so: they change the user's calculator.

## The 49G

- Flag -95 set means algebraic entry, and a fresh 49G starts that way:
  `read_stack`, `read_display` and `run_rpl` report `"mode": "algebraic"`
  (`list_calculators` for the shown 49G). Type algebraic syntax there, or run
  `CF(-95)` to switch to RPN (say that you did, and drop what the switch left
  on the stack). RPN text in algebraic mode gives "Invalid Syntax" and stays
  on an open command line: cancel it with key `"on"`.
- Exact mode (the default, flag -105 clear) keeps integers exact: `2 √` gives
  `'√2'` and `1 3 /` gives `'1/3'`. Use real numbers (`2.`) or `→NUM` for a
  decimal.
- The computer algebra commands (FACTOR, EXPAND, SOLVEVX, INTVX, …) are 49G
  only. Several work on the current variable VX, normally X.
- Many menus are choose boxes, not menu keys: typing the command name in
  `run_rpl` avoids them.

## The 28S

- `run_rpl` types the text through the keys and presses ENTER. Nothing reads
  the 28S's memory: check every result with `read_display`.
- Before `run_rpl`, look at the display for an open command line (text with
  a cursor below the stack). Finish it with ENTER or clear it first.
- Number entry ends only at a space or a command; when you use `press_keys`,
  separate numbers with a space or ENTER.
- It has symbolic algebra (COLCT, EXPAN, ISOL, derivatives and integrals) but
  no list processing, and many 48 commands do not exist. Check names with
  `lookup_command` and `"model": "28s"`.

## Moving a program between RPL models

See [workflows.md](workflows.md#move-a-program-to-another-model).
