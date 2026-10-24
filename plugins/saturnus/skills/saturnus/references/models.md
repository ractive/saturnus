# The key-driven models

On these models nothing reads memory: you press keys (`press_keys`), or a
function's keys by its name (`run_function`, from the reference's key path),
and read the display image after every step. `lookup_command` gives each
function's keys, its menu and examples with the display before and after.

Key names come from `list_keys`. On all of them the six keys under the display
are menu keys, named `"a"` to `"f"` from left to right; their meaning is
whatever the display's bottom line shows above them.

## HP 42S

RPN with a four-level stack (X, Y, Z, T): `2 ENTER 3 +` gives 5.

- Most of its 350-odd functions live in menus. A menu's labels sit above the
  top row of keys, which then act as menu keys; a menu can have several rows,
  moved through with the up and down arrows. EXIT (key `"on"`) goes back one
  level; press it until the stack shows alone.
- `run_function` presses a function's keys; for your own program it runs
  `XEQ "NAME"`.
- Programs are keyed in by hand in program mode (⇧ PRGM): one line per
  function, `LBL "NAME"` first. Read the display after each line, since a
  menu key in program mode records the function it shows. Leave program mode
  with EXIT before running.
- The solver and the integrator work on programs that declare their
  variables with MVAR.
- Docs: <https://ractive.ch/saturnus/docs/42s.md>.

## HP 17BII, 17B, 19BII, 27S

The business models (and the 27S scientific) work through menus: the bottom
line shows a menu, usually MAIN with FIN, BUS, SUM, TIME and SOLVE on the
17BII.

- Menus that calculate work the same way everywhere: key in a number and
  press a variable's menu key to store it; press the key with no number typed
  to calculate it. `run_function` with the variable's name (PMT, N, I%YR)
  presses its whole menu path, so a number typed just before is stored.
- ⇧ MAIN (shift, then EXIT) goes back to the main menu; EXIT goes back one
  level. ⇧ CLEAR DATA clears the variables of the menu on display (TVM's
  five values, a SUM list).
- Money paid out is negative, money received positive. A loan you receive
  is a positive PV with negative payments.
- Entry: the 17BII and 19BII start in ALG mode, which works in chain order:
  `2 + 3 × 4 =` gives 20, not 14. MODES switches them to RPN, where = is
  ENTER. The 17B has chain-order ALG only; the 27S is algebraic with operator
  precedence. Check the mode before typing a calculation, or put parentheses
  in.
- On a first start the 17BII, 17B and 19BII ask for a language. English is
  menu key `"b"` (ENGL) on the 17BII and 17B, `"f"` (ENG) on the 19BII. If a
  calculator already shows another language, MODES changes it.
- Docs: <https://ractive.ch/saturnus/docs/17bii.md>,
  <https://ractive.ch/saturnus/docs/17b.md>,
  <https://ractive.ch/saturnus/docs/19bii.md>,
  <https://ractive.ch/saturnus/docs/27s.md>.

## HP 38G, 39G, 40G

Algebraic graphing calculators built around aplets.

- HOME (key `"home"`) is the calculation view with a history. Its edit line
  is algebraic: `SIN(30)`, `2+3*4`. ENTER evaluates; the result goes into
  `Ans`. Starting an entry with an operator puts Ans in front.
- `run_function` puts a function on HOME's edit line (`PREDY(`); type the
  arguments, the closing parenthesis and ENTER with `press_keys`.
- Store with the STO▶ menu key (`"a"` in HOME): `5 STO▶ A` stores 5 in A.
  Variables have fixed types: A to Z reals, Z0 to Z9 complex, L0 to L9 lists,
  M0 to M9 matrices, C0 to C9 the Statistics aplet's data columns.
- Letters: ALPHA (key `"alpha"`), then the key with the letter (`list_keys`
  gives each key's letter). ALPHA lasts for one letter.
- An aplet has three main views: PLOT, SYMB and NUM, each with a setup screen
  (⇧ PLOT, ⇧ SYMB, ⇧ NUM). LIB opens the aplet library: choose an aplet and
  press START.
- Views and setup screens are input forms and choose boxes: read the display
  to see which field is highlighted. CANCEL is key `"on"`; HOME leaves any
  view.
- The 38G's ACOT ignores the angle mode (a ROM bug, in `lookup_command`'s
  note): use ATAN(1/x) in DEG mode.
- Docs: <https://ractive.ch/saturnus/docs/38g.md>,
  <https://ractive.ch/saturnus/docs/39g.md>,
  <https://ractive.ch/saturnus/docs/40g.md>.

## HP 28S

RPL, typed through its keys. See [rpl.md](rpl.md#the-28s). It draws the
calculator open, with the letter keys on the left half; `list_keys` names
them. Docs: <https://ractive.ch/saturnus/docs/28s.md>.
