// Ctrl+click and Option/Alt+click on a drawn key: its left- or
// right-shifted function. The page names the shift with the key press
// (`keyDown` with `shift`, web/protocol.md); the host's key queue taps it
// first unless it is on when the press plays.
// While one of the two modifiers is held, the skin lights the labels it
// reaches (`ModifierGlow`). Pure, without the DOM: web/test/shiftclick.test.mjs.

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
 * The shift key `mod` stands for on a model with keys `keyNames`: Ctrl the
 * left shift, Alt the right one; a model with one shift (38G, 39G, 40G,
 * 42S: `shift`) uses it for both. Null without a modifier or a shift.
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

/** How a click with `mod` is written: `Control-click` on a Mac, `Alt+click` elsewhere. */
export function clickLabel(mod, isMac) {
  if (isMac) return mod === "ctrl" ? "Control-click" : "Option-click";
  return mod === "ctrl" ? "Ctrl+click" : "Alt+click";
}

/**
 * A key's tooltip lines for its shifted labels (`left`, `right`, as the
 * skin has them): `Control-click: PREV`. A one-shift model's label takes
 * both modifiers.
 */
export function clickHints(k, keyNames, isMac) {
  if (k.name === "shift" || k.name === "leftshift" || k.name === "rightshift") return [];
  if (keyNames.has("shift")) {
    return k.left ? [`${clickLabel("ctrl", isMac)} or ${clickLabel("alt", isMac).toLowerCase()}: ${k.left}`] : [];
  }
  const out = [];
  if (k.left) out.push(`${clickLabel("ctrl", isMac)}: ${k.left}`);
  if (k.right) out.push(`${clickLabel("alt", isMac)}: ${k.right}`);
  return out;
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
