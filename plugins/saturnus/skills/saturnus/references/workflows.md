# Workflows

Each starts with `list_calculators` and `read_display`, and gets out of any
open form or menu first (SKILL.md, "Screens that catch key presses").

## Write, store and test a program (48SX, 48GX, 49G)

1. Check every command you plan to use with `lookup_command` for the model:
   name, stack effect, and whether it exists there at all.
2. Try the core of the program on the stack with `run_rpl` and sample values
   before you wrap it in « ».
3. Store it: `store_variable` with a short upper-case name and the program
   text. The user allows it in the app; a compile error comes back as the
   calculator's message.
4. `read_variable` to see what was stored.
5. Run it on cases you can check by hand: `run_rpl` with `"3 4 SQSUM"`. Look
   at `error` and the stack. Then the edge cases.
6. Tell the user the name, what it takes and leaves on the stack, and the
   results of your tests. Leave the stack as you found it if you can (`DROP`
   your test results, or `CLEAR` only if the stack was empty before).

Example: a program that sums the squares of a list's numbers, on the 48GX:

```
« SQ ΣLIST »          { 1 2 3 } gives 14
```

It is this short because the G series processes lists itself: SQ squares
each element. It fails on the 48SX, which has neither list processing nor
ΣLIST: there, loop over the list (`OBJ→`, then `1 SWAP START … NEXT`).

## Move a program to another model

1. `read_variable` on the source calculator (or take the user's text).
2. For each command, `lookup_command` with the target `model`. A missing name
   means the target lacks it: rewrite that part with commands it has.
3. Watch the differences that compile but behave differently: list
   processing (48GX and 49G, not 48SX or 28S), exact integers on the 49G, and
   the 49G's algebraic mode.
4. `store_variable` on the target (`calculator` names it), then run the same
   tests as on the source and compare the results.

From an RPL model to the 42S, 38G or a business model there is no common
language: write the program anew in that model's own way (42S key-by-key
program, 38G program in the Program catalog), or say it cannot be done.

## A loan on the 17BII (time value of money)

Example: 100,000 borrowed over 30 years, paid monthly at 6 % a year. What is
the payment?

1. Get to the main menu: ⇧ MAIN (`press_keys ["shift", "exit"]`).
2. Type 12 (`press_keys ["1", "2"]`) and `run_function` P/YR, then
   `run_function` END: monthly payments at the end of each period. Both stay
   set until changed.
3. `run_function` TVM to show the TVM menu, then clear its old values with
   ⇧ CLEAR DATA (`press_keys ["shift", "input"]`).
4. For each known value, type it and `run_function` its name: 360 then N,
   6 then I%YR, 100000 then PV, 0 then FV. Use the +/− key (`"neg"`) for
   negative amounts. Each answers with its value on the display (`N=360.00`).
5. With nothing typed, `run_function` PMT calculates it: `PMT=-599.55`, a
   payment of 599.55 going out.
6. Read the display to confirm, and give the answer with its sign explained.

The display shows two decimals by default (DSP sets more). The value behind
it has full precision.

## A linear fit on the 38G (Statistics aplet)

Example: fit y = m·x + b to five points and predict y at x = 10.

1. In HOME (key `"home"`), store the data as lists in the data columns:
   `{1,2,3,4,5}` STO▶ C1 and the y values STO▶ C2. Keys: ⇧ `8` is `{`, ⇧ `9`
   is `}`, `"comma"` separates, STO▶ is menu key `"a"`, then ALPHA C
   (`"alpha"`, `"cos"`) and `"1"`, then ENTER.
2. LIB, highlight Statistics, START (`"f"`). The NUM view shows C1 and C2.
3. The menu key under 1VAR/2VAR switches to two-variable statistics; make it
   show 2VAR. STATS shows the means, sums and the correlation (scroll down).
   OK goes back.
4. The fit model is in the SYMB view's setup (⇧ SYMB, S1FIT); Linear is the
   default. The SYMB view pairs S1 with C1 (independent) and C2 (dependent).
5. HOME, then `run_function` PREDY, `press_keys ["1", "0", "rparen",
   "enter"]` gives the predicted y at 10. PREDY(0) is the intercept b;
   PREDY(1) minus PREDY(0) is the slope m.
6. Read the HOME history to confirm, and say which fit model was used.

The 39G and 40G work the same way; `lookup_command` gives their keys.
