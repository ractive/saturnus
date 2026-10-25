// Ctrl+click and Option/Alt+click on a drawn key: its left- or
// right-shifted function. The page names the shift with the key press
// (`keyDown` with `shift`, web/protocol.md); the host's key queue taps it
// first unless it is on when the press plays.
// While one of the two modifiers is held, the skin lights the labels it
// reaches (`ModifierGlow`). Pure, without the DOM: web/test/shiftclick.test.mjs.

import { shiftedText } from "./hiddenkeys.js";

/** How long a modifier is held before the labels light, in ms: a chord (Ctrl+K) is quicker. */
export const GLOW_DELAY = 150;

/**
 * The modifier of a click or keyboard event: "ctrl" or "alt" when exactly
 * that one is held, else null (Ctrl+Alt is AltGr on Windows; Shift, Cmd
 * or Win with it is no shift-click).
 */
export function modifierOf(e) {
  if (e.shiftKey || e.metaKey || e.ctrlKey === e.altKey) return null;
  return e.ctrlKey ? "ctrl" : "alt";
}

/**
 * Whether a key's `pointerdown` comes with a `contextmenu` of its own
 * later: a secondary button (every browser), or Ctrl held (Chrome on
 * macOS sends a Ctrl+click as a secondary click). A plain or Option
 * left press never does.
 */
export function awaitsContextMenu(e) {
  return e.pointerType === "mouse" && (e.button === 2 || e.ctrlKey);
}

/**
 * Which `contextmenu` on a drawn key is a press of its own. Firefox on
 * macOS turns a Ctrl+click into a secondary click with `mousedown`
 * (button 2) and `contextmenu` but no `pointerdown`, so the key's
 * handler never sees the press; Chrome sends `pointerdown` first (which
 * presses), and on Windows the `contextmenu` of a right-click comes after
 * `mouseup`, however long it was held. A `pointerdown` that will have a
 * `contextmenu` leaves a token for its key; that `contextmenu` takes it,
 * and the next `pointerdown` on the key drops a token left over. A
 * `contextmenu` without a token, with Ctrl or Option alone, is the
 * Firefox press.
 */
export class ContextClicks {
  constructor() {
    /** Keys whose `pointerdown` still awaits its `contextmenu`. */
    this.awaiting = new Set();
  }

  /** A `pointerdown` on `key`. */
  pointerdown(key, e) {
    this.awaiting.delete(key);
    if (awaitsContextMenu(e)) this.awaiting.add(key);
  }

  /** A `contextmenu` on `key`: the modifier to press it with as a tap, or null. */
  contextmenu(key, e) {
    if (this.awaiting.delete(key)) return null;
    return modifierOf(e);
  }
}

/**
 * The shift key `mod` stands for on a model with keys `keyNames`: Ctrl the
 * left shift, Alt the right one; a model with one shift (38G, 39G, 40G,
 * 42S, 28S, 27S, 19BII, 17BII, 17B: `shift`) uses it for both. Null without a modifier or a shift.
 */
export function shiftKeyFor(mod, keyNames) {
  if (!mod) return null;
  if (keyNames.has("shift")) return "shift";
  const name = mod === "ctrl" ? "leftshift" : "rightshift";
  return keyNames.has(name) ? name : null;
}

/** The side whose labels `mod` reaches, "left" or "right" (one shift prints on the left), or null. */
export function glowSide(mod, keyNames) {
  const shift = shiftKeyFor(mod, keyNames);
  if (!shift) return null;
  return shift === "rightshift" ? "right" : "left";
}

/**
 * The shift a click with modifier `mod` on key `key` asks for, or null:
 * none without a modifier, and none for a click on a shift key itself.
 * Whether it is tapped is the host's, when the press plays: the last
 * frame shown may not have seen a shift still queued, or one a queued
 * key will use up.
 */
export function shiftFor(mod, key, keyNames) {
  if (key === "shift" || key === "leftshift" || key === "rightshift") return null;
  return shiftKeyFor(mod, keyNames);
}

/**
 * How a click with `mod` is written: in text `Control-click` on a Mac (as
 * Apple's prose writes it), `Alt+click` elsewhere; `short` (a tooltip) on
 * a Mac with the signs of its menus, `⌃-click`, `⌥-click`.
 */
export function clickLabel(mod, isMac, short = false) {
  if (isMac && short) return mod === "ctrl" ? "⌃-click" : "⌥-click";
  if (isMac) return mod === "ctrl" ? "Control-click" : "Option-click";
  return mod === "ctrl" ? "Ctrl+click" : "Alt+click";
}

/**
 * A key's tooltip lines for its shifted labels (`left`, `right`, as the
 * skin has them): `⌃-click: PREV` on a Mac, `Ctrl+click: PREV` elsewhere.
 * A one-shift model's label (its `left`) takes both modifiers. `extra` (the key's
 * web/hiddenkeys.js entries) adds what the print does not say:
 * `⌥-click: Kermit server (not printed)`, `⌥-click: ARG (LASTARG)`,
 * `⌃-click: SOLVE menu`.
 */
export function clickHints(k, keyNames, isMac, extra = {}) {
  if (k.name === "shift" || k.name === "leftshift" || k.name === "rightshift") return [];
  const click = (mod) => clickLabel(mod, isMac, true);
  if (keyNames.has("shift")) {
    const text = shiftedText(k, "left", extra, caps);
    return text ? [`${isMac ? "⌃- or ⌥-click" : `${click("ctrl")} or ${click("alt")}`}: ${text}`] : [];
  }
  const out = [];
  const left = shiftedText(k, "left", extra, caps);
  const right = shiftedText(k, "right", extra, caps);
  if (left) out.push(`${click("ctrl")}: ${left}`);
  if (right) out.push(`${click("alt")}: ${right}`);
  return out;
}

/** Whether `text` is a word of letters (a command's name: NXT, sin, ln). */
const isWord = (text) => /^[a-z]+$/i.test(text);

/**
 * A command in a tooltip, in capitals as the calculator writes it: `nxt`
 * is NXT, the 39G's printed `ln` LN. Only a word of letters: math such as
 * x², eˣ, √x or 1/x keeps its case, which is its meaning.
 */
export const caps = (text) => (isWord(text) ? text.toUpperCase() : text);

/**
 * A key's tooltip head: its printed label in capitals and its ALPHA
 * letter, `NXT (alpha L)`. A label that is no word (a glyph or math: ▶,
 * 1/x) adds the key's name, which list_keys and press_keys use:
 * `▶ (RIGHT, alpha R)`.
 */
export function keyTitle(k) {
  const label = k.label || "";
  const named = isWord(label) || !label ? null : k.name.toUpperCase();
  const head = label ? caps(label) : k.name.toUpperCase();
  const extra = [named, k.alpha ? `alpha ${k.alpha}` : null].filter(Boolean);
  return extra.length ? `${head} (${extra.join(", ")})` : head;
}

const MOD_KEYS = { Control: "ctrl", Alt: "alt" };

/**
 * Whether a modifier is lit: `lit` is "ctrl", "alt" or null. A modifier
 * pressed alone lights after `delay` ms, unless another key comes first
 * (a chord); it goes out on its release, another key, `clear` (the window
 * lost the focus, the page was hidden) and any event whose flags show it
 * up (`sync`). `onChange(lit)` hears every change. Timers are injected.
 */
export class ModifierGlow {
  constructor({ delay = GLOW_DELAY, setTimer = (f, ms) => setTimeout(f, ms), clearTimer = (t) => clearTimeout(t), onChange = () => {} } = {}) {
    this.delay = delay;
    this.setTimer = setTimer;
    this.clearTimer = clearTimer;
    this.onChange = onChange;
    /** The modifier held alone, waiting or lit. */
    this.held = null;
    this.lit = null;
    this.timer = null;
    /** Alt went down and no other key since (an Alt+click keeps it): its release would open the menu bar. */
    this.loneAlt = false;
  }

  /**
   * A keydown; `owned` when the calculator has the keys. A modifier alone
   * starts the wait; anything else is a chord or typing, and puts it out.
   */
  keydown(e, owned) {
    const mod = MOD_KEYS[e.key];
    this.loneAlt = mod === "alt" && modifierOf(e) === "alt";
    if (mod && owned && modifierOf(e) === mod) {
      if (e.repeat || this.held === mod) return;
      this.set(null);
      this.held = mod;
      this.timer = this.setTimer(() => {
        this.timer = null;
        if (this.held === mod) this.set(mod);
      }, this.delay);
      return;
    }
    this.clear();
  }

  /** A keyup: true when it is a lone Alt's release, which the page should prevent. */
  keyup(e) {
    const lone = e.key === "Alt" && this.loneAlt;
    this.loneAlt = false;
    const mod = MOD_KEYS[e.key];
    if (mod && mod === this.held) this.clear();
    else this.sync(e);
    return lone;
  }

  /** Any key or pointer event: out when its flags show the modifier up (or another one with it). */
  sync(e) {
    if (this.held && modifierOf(e) !== this.held) this.clear();
  }

  /** Out at once: the window lost the focus, the page was hidden, the keys left the calculator. */
  clear() {
    if (this.timer !== null) this.clearTimer(this.timer);
    this.timer = null;
    this.held = null;
    this.set(null);
  }

  set(lit) {
    if (lit === this.lit) return;
    this.lit = lit;
    this.onChange(lit);
  }
}
