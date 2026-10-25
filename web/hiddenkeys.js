// What a model's keys do beyond their print, per model (iteration 65):
// shifted functions with no label (→ ▶ starts the Kermit server on the
// 48GX), printed labels that do something else (→ EEX "ARG" is LASTARG),
// application keys whose left shift opens the application's command
// menu while only the right-shift label is printed (← 7 the SOLVE
// menu), and the ON-key combinations. The skin shows them in the
// tooltips, as ghost labels while ⌃ or ⌥ is held (sat-calculator.js
// `drawGhosts`) and in the ON key's menu (web/onkeys.js). Only the 48GX
// so far: the 48SX and 49G follow after a sweep of each. Pure, without
// the DOM: web/test/hiddenkeys.test.mjs.

/**
 * Per model, by key: `{left?, right?}`, each shift's entry one of
 * - `{unprinted: hint, ghost?: label}`: a function no label names; the
 *   tooltip writes `hint (not printed)`, and `ghost` is drawn in the empty
 *   slot while that shift's modifier is held;
 * - `{note: text}`: the printed label does something its name does not
 *   say, `ARG (LASTARG)`;
 * - `{menu: true}`: (left shift) the command menu of the application
 *   whose right-shift label is printed, `SOLVE menu`.
 */
export const HIDDEN_KEYS = {
  // wiki: hardware/hp48gx-keyboard
  "48gx": {
    // "→ ▶ with no command line starts the Kermit server" (FAQ 4.19, AUR SERVER).
    right: { right: { unprinted: "Kermit server", ghost: "SERVER" } },
    // "→ ▼ shows the current menu's labels in full" (User's Guide G-51
    // "→VIEW"); the 48SX prints this function as REVIEW.
    down: { right: { unprinted: "REVIEW, the menu's labels in full", ghost: "REVIEW" } },
    // "→ EEX "ARG" runs LASTARG" (User's Guide p. 3-5, G-23).
    eex: { right: { note: "LASTARG" } },
    // "Left shift on the application keys": the command menus (User's
    // Guide p. 1-7; observed on ROM R).
    ...Object.fromEntries(["prg", "cst", "var", "up", "7", "8", "9", "4", "5", "6", "1", "2", "3"].map((k) => [k, { left: { menu: true } }])),
  },
};

/** The entries of `model`'s key `name`: `{left?, right?}` (empty when none). */
export function hiddenOf(model, name) {
  return HIDDEN_KEYS[model]?.[name] ?? {};
}

/**
 * What a shifted click on key `k` (the skin's `{label, left, right}`) does,
 * for its tooltip line, `side` "left" or "right"; null when nothing.
 * `extra` is the key's `hiddenOf` entries.
 */
export function shiftedText(k, side, extra = {}, caps = (t) => t) {
  const printed = k[side] ? caps(k[side]) : "";
  const e = extra[side];
  if (e?.menu && k.right) return `${caps(k.right)} menu`;
  if (e?.unprinted && !printed) return `${e.unprinted} (not printed)`;
  if (e?.note && printed) return `${printed} (${e.note})`;
  return printed || null;
}

/** The ghost label of key `k` on `side`, or null: an unprinted function with a ghost, and that slot empty. */
export function ghostSlot(k, side, extra = {}) {
  const e = extra[side];
  if (!e?.ghost || k[side]) return null;
  return e.ghost;
}

/** Keys whose right-shift label ⌃ lights too, in the left shift's colour (their ← is that application's menu). */
export function menuKeys(model) {
  return new Set(Object.entries(HIDDEN_KEYS[model] ?? {}).filter(([, e]) => e.left?.menu).map(([k]) => k));
}

/** How far a ghost pair may reach past the key on each side, and the gap between its two labels, in skin units. */
export const GHOST_PAD = 10;
export const GHOST_GAP = 6;
/** The most a ghost (and its printed neighbour) is squeezed before it stays in the tooltip only. */
export const GHOST_MIN_SCALE = 0.5;

/**
 * Where the ghost of key `{x, w}` goes on `side` while its modifier is
 * held, from the measured widths of the ghost and of the printed label
 * of the other shift (`mateWidth`, null when there is none), or null when
 * it does not fit (squeezed below `GHOST_MIN_SCALE`) or could not be
 * measured. Alone, it is centred, as the skin draws a single label (room
 * `w + 34`); beside a printed label, the two take their sides as a
 * printed pair does, reaching `GHOST_PAD` past the key, both squeezed
 * alike: `{ghost, mate}`, each `{x, anchor, max}` (`max` null: its own width).
 */
export function ghostPlacement({ x, w, side, ghostWidth, mateWidth = null }) {
  if (!(ghostWidth > 0) || (mateWidth !== null && !(mateWidth > 0))) return null;
  if (mateWidth === null) {
    const room = w + 34;
    const scale = Math.min(1, room / ghostWidth);
    if (scale < GHOST_MIN_SCALE) return null;
    return { ghost: { x: x + w / 2, anchor: "middle", max: scale < 1 ? room : null }, mate: null };
  }
  const room = w + 2 * GHOST_PAD - GHOST_GAP;
  const scale = Math.min(1, room / (ghostWidth + mateWidth));
  if (scale < GHOST_MIN_SCALE) return null;
  const max = (width) => (scale < 1 ? width * scale : null);
  const leftEdge = { x: x - GHOST_PAD, anchor: "start" };
  const rightEdge = { x: x + w + GHOST_PAD, anchor: "end" };
  return side === "right"
    ? { ghost: { ...rightEdge, max: max(ghostWidth) }, mate: { ...leftEdge, max: max(mateWidth) } }
    : { ghost: { ...leftEdge, max: max(ghostWidth) }, mate: { ...rightEdge, max: max(mateWidth) } };
}
