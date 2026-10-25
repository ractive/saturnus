// What a model's keys do beyond their print, per model (iteration 65):
// shifted functions with no label (→ ▶ starts the Kermit server on the
// 48GX), printed labels that do something else (→ EEX "ARG" is LASTARG),
// application keys whose other shift opens the application's command
// menu while only one label is printed (← 7 the SOLVE menu on the 48GX,
// → 7 the SOLVR menu on the 48SX), functions of a shift held down while
// the key is pressed (the 49G), and the ON-key combinations. The skin
// shows them in the tooltips, as ghost labels while ⌃ or ⌥ is held
// (sat-calculator.js `drawGhosts`) and in the ON key's menu
// (web/onkeys.js). Data only, per model, from each model's keyboard page
// in the wiki. Pure, without the DOM: web/test/hiddenkeys.test.mjs.

/**
 * Per model, a list of entries `{key, shift, kind, label, ghost?, held?,
 * aside?, text?}`: `key` a key's script name, `shift` "left" or "right"
 * (a one-shift model's one shift is "left", where its skin prints), and
 * `kind`
 * - "unprinted": a function no label names; the tooltip writes
 *   `label (not printed)` (`aside` in place of "not printed" where the
 *   function is named elsewhere on the face, as the 38G's SETUP bracket),
 *   and `ghost` (short, as print would be) is drawn in the empty slot
 *   while that shift's modifier is held, where the key has a label slot
 *   of usable size (`ghostFits`). A function that works only in one
 *   context (a command line, a list) has no ghost;
 * - "note": the printed label does something its name does not say; the
 *   tooltip writes `ARG (label)`;
 * - "menu": the shift opens a command menu of application `label`,
 *   printed on the key under the other shift; the tooltip writes `text`
 *   (default `label menu`), and the shift's modifier lights that printed
 *   label in its own colour.
 * `held`: the function needs the shift held down while the key is
 * pressed (the 49G); a modifier click taps the shift, so such an entry is
 * a tooltip line only (`heldText`), "unprinted", with no ghost and no
 * glow. The UI knows nothing else of a model: a model without a list gets
 * nothing.
 */
export const HIDDEN_KEYS = {
  // wiki: hardware/hp48gx-keyboard
  "48gx": [
    // "→ ▶ with no command line starts the Kermit server" (FAQ 4.19, AUR SERVER).
    { key: "right", shift: "right", kind: "unprinted", label: "Kermit server", ghost: "SERVER" },
    // "→ ▼ shows the current menu's labels in full" (User's Guide G-51
    // "→VIEW"); the 48SX prints this function as REVIEW.
    { key: "down", shift: "right", kind: "unprinted", label: "REVIEW, the menu's labels in full", ghost: "REVIEW" },
    // "→ EEX "ARG" runs LASTARG" (User's Guide p. 3-5, G-23).
    { key: "eex", shift: "right", kind: "note", label: "LASTARG" },
    // "Left shift on the application keys": the command menus (User's
    // Guide p. 1-7; observed on ROM R).
    ...[["prg", "CHARS"], ["cst", "MODES"], ["var", "MEMORY"], ["up", "STACK"], ["7", "SOLVE"], ["8", "PLOT"], ["9", "SYMBOLIC"],
      ["4", "TIME"], ["5", "STAT"], ["6", "UNITS"], ["1", "I/O"], ["2", "LIBRARY"], ["3", "EQ LIB"]]
      .map(([key, label]) => ({ key, shift: "left", kind: "menu", label })),
  ],
  // wiki: hardware/hp48sx-keyboard (Owner's Manual pages; observed on ROM J)
  "48sx": [
    // "Right shift on the application keys": the slot is empty, the
    // application's name printed under the left shift.
    // → PRG: "Kermit server: Awaiting Server Cmd." (p. 33-9).
    { key: "prg", shift: "right", kind: "unprinted", label: "Kermit server", ghost: "SERVER" },
    // → CST, → VAR, → 7, → 8, → 5, → 6: the application's other menu
    // (p. 15-5, 6-10, 17-2, 1-33, 13-1; → 5 observed only).
    { key: "cst", shift: "right", kind: "menu", label: "MODES", text: "MODES customization menu" },
    { key: "var", shift: "right", kind: "menu", label: "MEMORY", text: "MEMORY arithmetic menu" },
    { key: "7", shift: "right", kind: "menu", label: "SOLVE", text: "SOLVR menu of the current equation" },
    { key: "8", shift: "right", kind: "menu", label: "PLOT", text: "PLOTR menu" },
    { key: "5", shift: "right", kind: "menu", label: "STAT", text: "STAT commands menu" },
    { key: "6", shift: "right", kind: "menu", label: "UNITS", text: "UNITS command menu" },
    // → 9 "Equation Catalog" (p. 17-2), → 4 "Alarm Catalog" (observed only).
    { key: "9", shift: "right", kind: "unprinted", label: "Equation Catalog", ghost: "EQ CAT" },
    { key: "4", shift: "right", kind: "unprinted", label: "Alarm Catalog", ghost: "ALRM CAT" },
    // "Other unprinted functions": → DEL types CLVAR (p. 6-9), → NXT the
    // menu's first page (p. 2-12), → ▼ acts as → +/- VISIT (observed).
    { key: "del", shift: "right", kind: "unprinted", label: "types CLVAR, which ENTER runs: clears all variables", ghost: "CLVAR" },
    { key: "nxt", shift: "right", kind: "unprinted", label: "the menu's first page", ghost: "FIRST" },
    { key: "down", shift: "right", kind: "unprinted", label: "VISIT, as → +/-", ghost: "VISIT" },
    // "In a command line, → ◀ and → ▶ move the cursor to the start and end" (observed).
    { key: "left", shift: "right", kind: "unprinted", label: "in a command line, the cursor to its start" },
    { key: "right", shift: "right", kind: "unprinted", label: "in a command line, the cursor to its end" },
    // "The LAST keys": STACK, CMD, ARG and MENU are LAST STACK, LAST CMD,
    // LAST ARG and LAST MENU (p. 1-3, 3-6).
    { key: "2", shift: "left", kind: "note", label: "LAST STACK, the stack before the last command line" },
    { key: "2", shift: "right", kind: "note", label: "LAST ARG, the last command's arguments" },
    { key: "3", shift: "left", kind: "note", label: "LAST CMD, the last command line back in the editor" },
    { key: "3", shift: "right", kind: "note", label: "LAST MENU" },
    // "→ ⬅ CLR clears the whole stack at once, with no confirmation" (observed).
    { key: "backspace", shift: "right", kind: "note", label: "clears the whole stack, with no confirmation" },
  ],
  // wiki: hardware/hp49g-keyboard (User's Manual, FAQ 4.10 and 4.14;
  // observed on ROM 2.10)
  "49g": [
    // "Held shift: a second function per key" (FAQ 4.14, observed).
    { key: "mode", shift: "left", held: true, kind: "unprinted", label: "MODES menu" },
    { key: "cat", shift: "right", held: true, kind: "unprinted", label: "CHARS menu" },
    { key: "7", shift: "right", held: true, kind: "unprinted", label: "SOLVE menu" },
    { key: "9", shift: "right", held: true, kind: "unprinted", label: "TIME menu" },
    { key: "var", shift: "left", held: true, kind: "unprinted", label: "HOME" },
    { key: "nxt", shift: "left", held: true, kind: "unprinted", label: "the last menu" },
    { key: "tool", shift: "left", held: true, kind: "unprinted", label: "complex mode on or off" },
    { key: "enter", shift: "right", held: true, kind: "unprinted", label: "exact or approximate mode" },
    { key: "eqw", shift: "right", held: true, kind: "unprinted", label: "types back quotes" },
    { key: "space", shift: "right", held: true, kind: "unprinted", label: "types ;" },
    { key: "right", shift: "right", held: true, kind: "unprinted", label: "Kermit server" },
    // → ▶ tapped: "XModem server: Xmodem Server Waiting for command";
    // in a command line the cursor to its end; in algebraic mode it
    // types XSERV() (observed).
    { key: "right", shift: "right", kind: "unprinted", label: "XModem server; in a command line, the cursor to its end; in algebraic mode it types XSERV()", ghost: "XSERV" },
    // "Other unprinted functions": the cursor keys have no shift labels.
    { key: "down", shift: "right", kind: "unprinted", label: "the menu's labels in full", ghost: "REVIEW" },
    { key: "up", shift: "right", kind: "unprinted", label: "the interactive stack, as ▲" },
    { key: "down", shift: "left", kind: "unprinted", label: "edits level 1, as ▼" },
    { key: "right", shift: "left", kind: "unprinted", label: "SWAP, as ▶; in a command line, the cursor to its end" },
    { key: "left", shift: "left", kind: "unprinted", label: "in a command line, the cursor to its start" },
    { key: "left", shift: "right", kind: "unprinted", label: "in a command line, the cursor to its start" },
    // The plot keys: in RPN mode a tapped shift gives the soft menu's
    // left-shift action, the shift must be held down (FAQ 4.10).
    ...["a", "b", "c", "d", "e", "f"].map((key) => ({ key, shift: "left", kind: "note", label: "algebraic mode; in RPN mode hold the left shift down" })),
    // "→ ÷ ARG on a real number asks Complex mode on? first" (observed).
    { key: "divide", shift: "right", kind: "note", label: "on a real number, asks \"Complex mode on?\" first" },
  ],
  // wiki: hardware/hp28s-keyboard (Owner's Manual; observed on ROM 2BB)
  "28s": [
    // "Shifted menu keys in a command line": the cursor menu INS DEL ▲ ▼
    // ◀ ▶ (p. 166-168).
    { key: "a", shift: "left", kind: "unprinted", label: "in a command line, deletes everything left of the cursor" },
    { key: "b", shift: "left", kind: "unprinted", label: "in a command line, deletes from the cursor to the right" },
    { key: "c", shift: "left", kind: "unprinted", label: "in a command line, the top row" },
    { key: "d", shift: "left", kind: "unprinted", label: "in a command line, the bottom row" },
    { key: "e", shift: "left", kind: "unprinted", label: "in a command line, the left end" },
    { key: "f", shift: "left", kind: "unprinted", label: "in a command line, the right end" },
    // "Shift α, printed MENUS, is Menu Lock" (p. 169, 192, 284).
    { key: "alpha", shift: "left", kind: "note", label: "Menu Lock: A to R select their menus unshifted until pressed again" },
  ],
  // wiki: hardware/hp38g-39g-keyboard (38G User's Guide; observed on A1.67)
  "38g": [
    // "SHIFT PLOT, SHIFT SYMB, SHIFT NUM open the setup views" (p. 1-13).
    { key: "plot", shift: "left", kind: "unprinted", label: "SETUP-PLOT, the plot setup view", aside: "the SETUP bracket" },
    { key: "symb", shift: "left", kind: "unprinted", label: "SETUP-SYMB, the symbolic setup view", aside: "the SETUP bracket" },
    { key: "num", shift: "left", kind: "unprinted", label: "SETUP-NUM, the numeric setup view", aside: "the SETUP bracket" },
    // "SHIFT ▲ and SHIFT ▼ in a list" (p. 1-11), "SHIFT ◀ and SHIFT ▶ on the edit line" (observed).
    { key: "up", shift: "left", kind: "unprinted", label: "the beginning of a list; in HOME, the oldest entry" },
    { key: "down", shift: "left", kind: "unprinted", label: "the end of a list; in HOME, back to the edit line" },
    { key: "left", shift: "left", kind: "unprinted", label: "the start of the edit line" },
    { key: "right", shift: "left", kind: "unprinted", label: "the end of the edit line" },
  ],
};

// wiki: hardware/hp38g-39g-keyboard (39G/40G User's Guide; observed on
// rom.39g): as the 38G, but SHIFT SYMB opens a symbolic setup only where
// the aplet has one (p. 8-3: the Statistics aplet's fit); the 40G gave
// the same as the 39G for every combination.
HIDDEN_KEYS["39g"] = HIDDEN_KEYS["38g"].map((e) => (e.key === "symb" ? { ...e, label: "SETUP-SYMB, the symbolic setup view where the aplet has one, as the Statistics aplet's fit" } : e));
HIDDEN_KEYS["40g"] = HIDDEN_KEYS["39g"];
// wiki: hardware/hp19bii-keyboard: "ON turns the calculator off as well
// as on" (p. 20); "shift ON turned the display off" (observed), and the
// key has no shifted OFF label.
HIDDEN_KEYS["19bii"] = [{ key: "on", shift: "left", kind: "unprinted", label: "OFF", ghost: "OFF" }];
// The 42S prints a shifted function above every key (wiki:
// hardware/hp42s-keyboard); the 27S, 17B and 17BII's shifts on keys with
// no label act as the key alone or do nothing
// (wiki: hardware/hp27s-17b-17bii-keyboard): no entries.

/** The key of an entry in `hiddenOf`'s result: its shift, `left held` for a held one. */
const slot = (e) => (e.held ? `${e.shift} held` : e.shift);

/**
 * The entries of `model`'s key `name` by shift: `{left?, right?, "left
 * held"?, "right held"?}` (empty when none).
 */
export function hiddenOf(model, name) {
  const out = {};
  for (const e of HIDDEN_KEYS[model] ?? []) if (e.key === name) out[slot(e)] = e;
  return out;
}

/**
 * What a shifted click on key `k` (the skin's `{label, left, right}`) does,
 * for its tooltip line, `side` "left" or "right"; null when nothing.
 * `extra` is the key's `hiddenOf` entries.
 */
export function shiftedText(k, side, extra = {}, caps = (t) => t) {
  const printed = k[side] ? caps(k[side]) : "";
  const e = extra[side];
  if (e?.kind === "menu") return e.text ?? `${caps(e.label)} menu`;
  if (e?.kind === "unprinted" && !printed) return `${e.label} (${e.aside ?? "not printed"})`;
  if (e?.kind === "note" && printed) return `${printed} (${e.label})`;
  return printed || null;
}

/**
 * The tooltip line of key `k`'s function with shift `side` held down
 * while the key is pressed (a `held` entry), or null:
 * `With the left shift held down: MODES menu (not printed)`. A modifier
 * click taps the shift; holding a shift is a hand on the calculator's
 * shift key.
 */
export function heldText(side, extra = {}) {
  const e = extra[`${side} held`];
  return e ? `With the ${side} shift held down: ${e.label} (not printed)` : null;
}

/** The ghost label of key `k` on `side`, or null: an unprinted function with a ghost, and that slot empty. */
export function ghostSlot(k, side, extra = {}) {
  const e = extra[side];
  if (e?.kind !== "unprinted" || !e.ghost || k[side]) return null;
  return e.ghost;
}

/**
 * The free height above key `k` of skin `s`, in skin units: from the
 * key's top up to the nearest key above that overlaps it sideways
 * (unbounded when none). Its label slot.
 */
export function labelRoom(s, k) {
  const [x, y, w] = k.rect;
  let room = Infinity;
  for (const o of s.keys) {
    const [ox, oy, ow, oh] = o.rect;
    if (o !== k && ox < x + w && ox + ow > x && oy + oh <= y) room = Math.min(room, y - (oy + oh));
  }
  return room;
}

/**
 * Whether key `k` has a label slot a ghost can use: at least twice the
 * skin's label size (`s.small`) free above it, the label and the air the
 * skin leaves around it. The 49G's cursor pad has none (its keys sit
 * close under each other, with no print): its unprinted functions stay
 * in the tooltips.
 */
export function ghostFits(s, k) {
  return labelRoom(s, k) >= 2 * s.small;
}

/**
 * The printed labels a modifier lights beyond its own (an application
 * whose menu it opens): `Map` of key name to `{shift, label}`, the
 * shift whose modifier lights it and the printed text.
 */
export function menuLabels(model) {
  return new Map((HIDDEN_KEYS[model] ?? []).filter((e) => e.kind === "menu" && !e.held).map((e) => [e.key, { shift: e.shift, label: e.label }]));
}
