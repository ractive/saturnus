// What a model's keys do beyond their print, per model (iteration 65):
// shifted functions with no label (→ ▶ starts the Kermit server on the
// 48GX), printed labels that do something else (→ EEX "ARG" is LASTARG),
// application keys whose left shift opens the application's command
// menu while only the right-shift label is printed (← 7 the SOLVE
// menu), and the ON-key combinations. The skin shows them in the
// tooltips, as ghost labels while ⌃ or ⌥ is held (sat-calculator.js
// `drawGhosts`) and in the ON key's menu (web/onkeys.js). Data only, per
// model; only the 48GX so far, the other models follow from a sweep of
// each. Pure, without the DOM: web/test/hiddenkeys.test.mjs.

/**
 * Per model, a list of entries `{key, shift, kind, label, ghost?}`:
 * `key` a key's script name, `shift` "left" or "right" (a one-shift
 * model's one shift is "left", where its skin prints), and `kind`
 * - "unprinted": a function no label names; the tooltip writes
 *   `label (not printed)`, and `ghost` (short, as print would be) is drawn
 *   in the empty slot while that shift's modifier is held;
 * - "note": the printed label does something its name does not say; the
 *   tooltip writes `ARG (label)`;
 * - "menu": the shift opens the command menu of application `label`,
 *   printed on the key under another shift; the tooltip writes
 *   `label menu`, and the shift's modifier lights that printed label.
 * The UI knows nothing else of a model: a model without a list gets
 * nothing. Other models follow from a sweep of each, into the wiki first.
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
};

/** The entries of `model`'s key `name` by shift: `{left?, right?}` (empty when none). */
export function hiddenOf(model, name) {
  const out = {};
  for (const e of HIDDEN_KEYS[model] ?? []) if (e.key === name) out[e.shift] = e;
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
  if (e?.kind === "menu") return `${caps(e.label)} menu`;
  if (e?.kind === "unprinted" && !printed) return `${e.label} (not printed)`;
  if (e?.kind === "note" && printed) return `${printed} (${e.label})`;
  return printed || null;
}

/** The ghost label of key `k` on `side`, or null: an unprinted function with a ghost, and that slot empty. */
export function ghostSlot(k, side, extra = {}) {
  const e = extra[side];
  if (e?.kind !== "unprinted" || !e.ghost || k[side]) return null;
  return e.ghost;
}

/**
 * The printed labels a modifier lights beyond its own (an application
 * whose menu it opens): `Map` of key name to `{shift, label}`, the
 * shift whose modifier lights it and the printed text.
 */
export function menuLabels(model) {
  return new Map((HIDDEN_KEYS[model] ?? []).filter((e) => e.kind === "menu").map((e) => [e.key, { shift: e.shift, label: e.label }]));
}
