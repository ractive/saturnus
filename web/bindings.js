// The keyboard shortcuts that can be changed: the calculator keys a
// computer has no key for (ON, α, the shifts) and the app's actions. A
// binding is the physical key (`KeyboardEvent.code`) plus its modifiers,
// `Alt+KeyO`, so it is the same key on every layout; it is shown with the
// label the layout gives that key. Defaults, the user's changes (only
// those are stored), matching a keydown, labels and the warnings for a
// binding (another action's key, a browser's or the system's shortcut, a
// key that types, a dead key). Pure, without the DOM: web/test/.
//
// Typed characters stay as they are (a letter types itself): they are
// sat-calculator.js's, not bindings.

/** The modifiers in the order a binding is written. */
const MODS = ["Ctrl", "Alt", "Shift", "Meta"];

/**
 * The bindable actions in the order the dialog lists them (and the order
 * that decides when two share a key). `Mod` is Cmd on a Mac, Ctrl
 * elsewhere. Every action has a default reachable without a dead key on
 * the US, UK, German, Swiss German and French layouts, and no default
 * without a modifier is a key that types a character on them (or on
 * Dvorak): `[` and `]` are `+` on German and `/` and `=` on Dvorak, so
 * the shifts are Alt chords (web/test/bindings.test.mjs).
 */
export const ACTIONS = [
  { id: "on", group: "calculator", title: "ON", description: "The ON key (also CANCEL and EXIT).", keys: ["Escape", "Alt+KeyO"] },
  { id: "alpha", group: "calculator", title: "α", description: "The alpha key; twice for alpha lock on the 48 and 49G.", keys: ["Tab"] },
  { id: "leftshift", group: "calculator", title: "Left shift", description: "The left shift key (the only shift on the 38G, 39G and 40G).", keys: ["Alt+KeyL"] },
  { id: "rightshift", group: "calculator", title: "Right shift", description: "The right shift key.", keys: ["Alt+KeyR"] },
  { id: "palette", group: "app", title: "Command palette", description: "Open or close the command palette.", keys: ["Mod+KeyK"] },
  { id: "edit", group: "app", title: "Edit", description: "Edit in the editor the object selected in the memory view, else stack level 1 (48SX, 48GX, 49G).", keys: ["Mod+KeyE"] },
  { id: "shortcuts", group: "app", title: "Keyboard shortcuts", description: "This dialog.", keys: ["Alt+KeyK"] },
  { id: "layerFocus", group: "app", title: "Keys to the memory view", description: "Move the keys between the calculator and the memory view (opening it).", keys: ["Alt+KeyM"] },
  { id: "layer", group: "app", title: "Memory view", description: "Show or hide the memory view beside the calculator.", keys: ["Alt+Shift+KeyM"] },
  { id: "fullscreen", group: "app", title: "Fullscreen", description: "Enter or leave fullscreen.", keys: ["Alt+Enter"] },
  { id: "speed", group: "app", title: "Next speed", description: "1×, 2×, 4×, max, and round again.", keys: ["Alt+KeyS"] },
  { id: "copyScreen", group: "app", title: "Copy screen", description: "The display as a PNG image, to the clipboard (in the colours chosen in the panel).", keys: ["Alt+Shift+KeyC"] },
  { id: "saveScreen", group: "app", title: "Save screen", description: "The display as a PNG file.", keys: ["Alt+Shift+KeyS"] },
  { id: "darker", group: "app", title: "Darker display", description: "The contrast one step up (ON and +).", keys: ["Alt+ArrowUp"] },
  { id: "lighter", group: "app", title: "Lighter display", description: "The contrast one step down (ON and −).", keys: ["Alt+ArrowDown"] },
];

const BY_ID = new Map(ACTIONS.map((a) => [a.id, a]));

/** The modifiers the palette's row numbers (1-9) can use. */
export const NUMBER_MODIFIERS = ["meta", "ctrl", "alt"];

/**
 * The row-number modifier's default: Cmd in the desktop app on a Mac,
 * where nothing else owns Cmd+digit; Ctrl in Mac browsers (they keep
 * Cmd+digit for tabs); Alt in browsers on Windows and Linux, which keep
 * Ctrl+digit (Alt+digit can be prevented there).
 */
export function defaultNumberModifier(host, isMac) {
  if (host === "tauri" && isMac) return "meta";
  return isMac ? "ctrl" : "alt";
}

/** How a row number with `mod` is shown: `⌘3`, `⌃3`, `Alt+3`. */
export function numberLabel(mod, isMac, n) {
  if (isMac) return `${{ meta: "⌘", ctrl: "⌃", alt: "⌥" }[mod]}${n}`;
  return `${{ meta: "Win", ctrl: "Ctrl", alt: "Alt" }[mod]}+${n}`;
}

/** Whether `e` is a row-number key with `mod` alone held; the digit or null. */
export function numberDigit(e, mod) {
  const d = /^Digit([1-9])$/.exec(e.code);
  if (!d || e.shiftKey) return null;
  const held = { meta: e.metaKey && !e.ctrlKey && !e.altKey, ctrl: e.ctrlKey && !e.metaKey && !e.altKey, alt: e.altKey && !e.metaKey && !e.ctrlKey }[mod];
  return held ? Number(d[1]) : null;
}

/** `Alt+KeyO` as `{code, ctrl, alt, shift, meta}`, or null when it names no key. */
export function parseCombo(text, isMac = false) {
  const parts = String(text).split("+");
  const code = parts.pop();
  if (!/^[A-Z][A-Za-z0-9]+$/.test(code ?? "") || MODIFIER_CODES.has(code)) return null;
  const c = { code, ctrl: false, alt: false, shift: false, meta: false };
  for (const p of parts) {
    if (p === "Mod") c[isMac ? "meta" : "ctrl"] = true;
    else if (MODS.includes(p)) c[p.toLowerCase()] = true;
    else return null;
  }
  return c;
}

/** The canonical text of a combination: modifiers in a fixed order, then the code. */
export function comboText(c) {
  return [...MODS.filter((m) => c[m.toLowerCase()]), c.code].join("+");
}

/** A binding's canonical text from what is written (`Mod` resolved), or null. */
export function normalize(text, isMac = false) {
  const c = parseCombo(text, isMac);
  return c ? comboText(c) : null;
}

const MODIFIER_CODES = new Set([
  "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight",
  "MetaLeft", "MetaRight", "OSLeft", "OSRight", "CapsLock", "Fn", "FnLock", "NumLock", "ContextMenu",
]);

/** The combination of a keydown, or null for a modifier pressed alone (or no code). */
export function comboOf(e) {
  if (!e.code || MODIFIER_CODES.has(e.code) || e.key === "AltGraph") return null;
  return comboText({ code: e.code, ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey });
}

/** Labels of keys whose name is not their character (or that a layout map does not cover). */
const KEY_LABELS = {
  Escape: "Esc", Tab: "Tab", Enter: "Enter", NumpadEnter: "Enter", Backspace: "⌫", Delete: "Del",
  Space: "Space", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→",
  Home: "Home", End: "End", PageUp: "PgUp", PageDown: "PgDn", Insert: "Ins",
  Backquote: "`", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backslash: "\\",
  Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/", IntlBackslash: "<",
};

/** Codes whose label comes from the layout (a character key). */
const CHARACTER = /^(Key[A-Z]|Digit\d|Backquote|Minus|Equal|BracketLeft|BracketRight|Backslash|Semicolon|Quote|Comma|Period|Slash|IntlBackslash|IntlRo|IntlYen)$/;

/**
 * The label of `code`: the layout's character for it when `layout` (a
 * `Map` of code to character, `navigator.keyboard.getLayoutMap()`) knows
 * it, else the US keyboard's.
 */
export function keyLabel(code, layout = null) {
  if (layout && CHARACTER.test(code)) {
    const ch = layout.get(code);
    if (ch && ch.trim()) return ch.toUpperCase();
  }
  if (KEY_LABELS[code]) return KEY_LABELS[code];
  let m = /^Key([A-Z])$/.exec(code) ?? /^Digit(\d)$/.exec(code) ?? /^(F\d{1,2})$/.exec(code);
  if (m) return m[1];
  m = /^Numpad(.*)$/.exec(code);
  if (m) return `Num ${m[1]}`;
  return code;
}

/** A combination as shown: `⌥O` on a Mac, `Alt+O` elsewhere. */
export function comboLabel(text, { isMac = false, layout = null } = {}) {
  const c = parseCombo(text, isMac);
  if (!c) return String(text);
  const key = keyLabel(c.code, layout);
  if (isMac) return `${c.ctrl ? "⌃" : ""}${c.alt ? "⌥" : ""}${c.shift ? "⇧" : ""}${c.meta ? "⌘" : ""}${key}`;
  return [c.ctrl && "Ctrl", c.alt && "Alt", c.shift && "Shift", c.meta && "Win", key].filter(Boolean).join("+");
}

/** Keys that type a character when pressed without Ctrl, Alt or Cmd: every character key, the space and the keypad's. */
const TYPES = /^(Key[A-Z]|Digit\d|Numpad(\d|Add|Subtract|Multiply|Divide|Decimal)|Space|Backquote|Minus|Equal|BracketLeft|BracketRight|Backslash|Semicolon|Quote|Comma|Period|Slash|IntlBackslash|IntlRo|IntlYen)$/;

/** Whether `ch` is a character the calculator types from the keyboard (sat-calculator.js: letters, digits, operators). */
export function calculatorTypes(ch) {
  return typeof ch === "string" && /^[a-z0-9+\-*/.,'^ ]$/i.test(ch);
}

/** The calculator keys that are not bindings (sat-calculator.js's KEYMAP). */
const FIXED = {
  Enter: "ENTER", NumpadEnter: "ENTER", Backspace: "⬅ (backspace)", Delete: "DEL",
  ArrowUp: "▲", ArrowDown: "▼", ArrowLeft: "◀", ArrowRight: "▶",
  F1: "first menu", F2: "second menu", F3: "third menu", F4: "fourth menu", F5: "fifth menu", F6: "sixth menu",
};

/** Physical keys that are dead keys, unshifted or shifted, on the layouts the defaults are checked against. */
export const DEAD_KEYS = {
  Backquote: ["German"],
  Equal: ["German", "Swiss German"],
  BracketLeft: ["French"],
  BracketRight: ["Swiss German"],
};

/**
 * What a browser or the system does with a combination before (or
 * instead of) the page, or null. `host` is "tauri" in the desktop app,
 * where the browser's tab shortcuts do not exist.
 */
export function reserved(text, { isMac = false, host = "worker" } = {}) {
  const c = parseCombo(text, isMac);
  if (!c) return null;
  const only = (want) => ["ctrl", "alt", "shift", "meta"].every((m) => c[m] === Boolean(want[m]));
  const browser = host !== "tauri";
  const digit = /^Digit[1-9]$/.test(c.code);
  const mod = isMac ? { meta: true } : { ctrl: true };
  if (digit && browser && only(mod)) return `${isMac ? "Cmd" : "Ctrl"}+digit switches the browser's tabs.`;
  if (digit && browser && !isMac && only({ alt: true })) return "Alt+digit switches tabs in browsers on Linux.";
  if (c.code === "Escape" && only({ shift: true })) return "Shift+Esc opens Firefox's process manager.";
  if (browser && c.code === "KeyC" && only({ ...mod, shift: true })) return "The browser's inspect element (its developer tools); the page may never get it.";
  if (c.code === "Escape" && only({})) return "In fullscreen Esc leaves fullscreen first (Chrome gives it to the page unless it is held).";
  if (isMac) {
    if (only({ meta: true }) && ["KeyQ", "KeyW", "KeyH", "KeyM", "Tab", "Space"].includes(c.code)) return "The system's: quit, close, hide, minimize or switch.";
    if (browser && only({ meta: true }) && ["KeyT", "KeyN", "KeyR", "KeyL", "BracketLeft", "BracketRight"].includes(c.code)) return "The browser's: new tab or window, reload, address bar, back or forward.";
    if (only({ ctrl: true }) && c.code === "Space") return "The system's input source switch.";
  } else {
    if (only({ alt: true }) && ["F4", "Tab"].includes(c.code)) return "The system's: close the window or switch.";
    if (only({ ctrl: true }) && ["KeyW", "KeyQ"].includes(c.code)) return "Closes the window or quits.";
    if (browser && only({ ctrl: true }) && ["KeyT", "KeyN", "KeyR", "KeyL", "Tab"].includes(c.code)) return "The browser's: new tab or window, reload, address bar, next tab.";
    if (browser && only({ alt: true }) && ["ArrowLeft", "ArrowRight", "Home"].includes(c.code)) return "The browser's back, forward or home page.";
  }
  if (browser && only({}) && ["F5", "F11", "F12"].includes(c.code)) return "The browser's: reload, fullscreen or developer tools.";
  return null;
}

/**
 * The bindings in force: the defaults with the user's changes (`saved`,
 * as `toJSON` gives it). `isMac` resolves `Mod` and picks the labels;
 * `host` the row-number default and which shortcuts are the browser's.
 */
export class Bindings {
  constructor({ isMac = false, host = "worker", saved = null } = {}) {
    this.isMac = isMac;
    this.host = host;
    /** The layout's labels (`Map` code -> character), once known. */
    this.layout = null;
    this.listeners = new Set();
    this.load(saved);
  }

  load(saved) {
    /** Per action: the user's keys, where they differ from the defaults. */
    this.changed = new Map();
    this.numberChanged = null;
    const keys = saved?.keys ?? {};
    for (const [id, list] of Object.entries(keys)) {
      if (!BY_ID.has(id) || !Array.isArray(list)) continue;
      const clean = [...new Set(list.map((k) => normalize(k, this.isMac)).filter(Boolean))];
      this.changed.set(id, clean);
    }
    if (NUMBER_MODIFIERS.includes(saved?.numberModifier)) this.numberChanged = saved.numberModifier;
    this.index();
  }

  /** What is stored: only the changes. */
  toJSON() {
    const keys = {};
    for (const [id, list] of this.changed) keys[id] = list;
    return { version: 1, keys, ...(this.numberChanged ? { numberModifier: this.numberChanged } : {}) };
  }

  defaults(id) {
    return (BY_ID.get(id)?.keys ?? []).map((k) => normalize(k, this.isMac));
  }

  /** The keys of action `id`, canonical. */
  keys(id) {
    return this.changed.get(id) ?? this.defaults(id);
  }

  isDefault(id) {
    const a = this.keys(id);
    const b = this.defaults(id);
    return a.length === b.length && a.every((k, i) => k === b[i]);
  }

  get numberModifier() {
    return this.numberChanged ?? defaultNumberModifier(this.host, this.isMac);
  }

  setNumberModifier(mod) {
    if (!NUMBER_MODIFIERS.includes(mod)) return;
    this.numberChanged = mod === defaultNumberModifier(this.host, this.isMac) ? null : mod;
    this.changed_();
  }

  /** Replace action `id`'s keys. */
  set(id, list) {
    if (!BY_ID.has(id)) return;
    const clean = [...new Set(list.map((k) => normalize(k, this.isMac)).filter(Boolean))];
    const d = this.defaults(id);
    if (clean.length === d.length && clean.every((k, i) => k === d[i])) this.changed.delete(id);
    else this.changed.set(id, clean);
    this.index();
    this.changed_();
  }

  add(id, combo) {
    const k = normalize(combo, this.isMac);
    if (k && !this.keys(id).includes(k)) this.set(id, [...this.keys(id), k]);
  }

  remove(id, combo) {
    this.set(id, this.keys(id).filter((k) => k !== combo));
  }

  /** Back to the defaults: action `id`, or everything. */
  reset(id = null) {
    if (id) this.changed.delete(id);
    else {
      this.changed.clear();
      this.numberChanged = null;
    }
    this.index();
    this.changed_();
  }

  index() {
    /** Combination -> the actions bound to it, in the list's order. */
    this.byCombo = new Map();
    for (const a of ACTIONS) {
      for (const k of this.keys(a.id)) {
        if (!this.byCombo.has(k)) this.byCombo.set(k, []);
        this.byCombo.get(k).push(a.id);
      }
    }
  }

  /** The action a keydown is bound to (of `group` when given), or null; the first in the list wins a conflict. */
  match(e, group = null) {
    const combo = comboOf(e);
    if (!combo) return null;
    const ids = this.byCombo.get(combo) ?? [];
    return ids.find((id) => !group || BY_ID.get(id).group === group) ?? null;
  }

  /** Whether keydown `e` is action `id`'s. */
  is(id, e) {
    const combo = comboOf(e);
    return Boolean(combo) && this.keys(id).includes(combo);
  }

  /** The layout's labels (a `Map` of code to character); the labels change, the bindings do not. */
  setLayout(map) {
    this.layout = map;
    this.changed_();
  }

  /** `combo` as shown on this platform and layout. */
  label(combo) {
    return comboLabel(combo, { isMac: this.isMac, layout: this.layout });
  }

  /** Action `id`'s first key as shown, or "" when it has none. */
  labelOf(id) {
    const k = this.keys(id)[0];
    return k ? this.label(k) : "";
  }

  numberLabel(n) {
    return numberLabel(this.numberModifier, this.isMac, n);
  }

  /**
   * What is wrong with `combo` as one of action `id`'s keys:
   * `[{kind, text}]`, kind `conflict` (another action has it), `reserved`
   * (the browser's or the system's), `fixed` (a calculator key that is not
   * a binding), `typing` (a key that types) or `dead` (a dead key on a
   * common layout).
   */
  warnings(id, combo) {
    const out = [];
    const c = parseCombo(combo, this.isMac);
    if (!c) return out;
    for (const other of this.byCombo.get(combo) ?? []) {
      if (other !== id) out.push({ kind: "conflict", text: `Also the key of “${BY_ID.get(other).title}”; ${ACTIONS.findIndex((a) => a.id === other) < ACTIONS.findIndex((a) => a.id === id) ? "that one wins" : "this one wins"}.` });
    }
    const r = reserved(combo, { isMac: this.isMac, host: this.host });
    if (r) out.push({ kind: "reserved", text: r });
    const plain = !c.ctrl && !c.alt && !c.meta;
    if (plain && !c.shift && FIXED[c.code]) out.push({ kind: "fixed", text: `Also the calculator's ${FIXED[c.code]} key; this binding takes it.` });
    if (plain && TYPES.test(c.code)) {
      // With the layout's map the key's own character decides; without it every character key may type one.
      const ch = this.layout?.get(c.code);
      if (ch === undefined || !CHARACTER.test(c.code)) out.push({ kind: "typing", text: "This key types a character; bound, it no longer does." });
      else if (calculatorTypes(ch) || (c.shift && ch.trim())) out.push({ kind: "typing", text: `This key types “${ch}” on your layout; bound, it no longer does.` });
    }
    if (plain && DEAD_KEYS[c.code]) out.push({ kind: "dead", text: `A dead key on the ${DEAD_KEYS[c.code].join(" and ")} layout${DEAD_KEYS[c.code].length > 1 ? "s" : ""}; there, use another key.` });
    return out;
  }

  /** The row-number modifier's warning, or null. */
  numberWarning(mod = this.numberModifier) {
    const browser = this.host !== "tauri";
    if (browser && ((this.isMac && mod === "meta") || (!this.isMac && mod === "ctrl"))) return "The browser switches tabs with this; the palette may not get the keys.";
    return null;
  }

  onChange(fn) {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  changed_() {
    for (const fn of this.listeners) fn(this);
  }
}

/** The action `id`'s description (title, group, text, default keys). */
export function action(id) {
  return BY_ID.get(id) ?? null;
}
