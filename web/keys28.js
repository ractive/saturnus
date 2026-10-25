// The 28S in Search: a command chosen is typed on its keys, as a person
// would, then ENTER ("run"), or typed only ("insert"). The palette does not
// read the 28S's command line, so nothing is cleared: a name typed into an open
// line is added to it after a space, and ENTER runs the whole line. The
// commands that are keys (+, DROP, SWAP, ...) are pressed as keys. The
// keys follow the generator's typing (crates/saturnus-refgen/src/hp28.rs,
// `keys`), where each was checked on the 28S ROM. Pure functions; the
// palette (palette.js) sends the keys and reads the state they depend on:
// the shift and α annunciators, the menu labels on the display, and the
// two modes without an annunciator (the protocol's `entryModes`).

/** The commands that are keys: pressed, not typed, when run. */
export const KEY_COMMANDS = {
  "+": ["plus"], "-": ["minus"], "*": ["multiply"], "/": ["divide"],
  "^": ["shift", "multiply"], SQ: ["shift", "plus"], "√": ["shift", "minus"], INV: ["shift", "divide"],
  "%": ["shift", "2"], "%CH": ["shift", "3"], "∫": ["shift", "5"], "∂": ["shift", "6"],
  DROP: ["drop"], SWAP: ["shift", "backspace"], ROLL: ["shift", "drop"], CLEAR: ["shift", "0"],
  EVAL: ["eval"], "→NUM": ["shift", "eval"], STO: ["sto"], RCL: ["shift", "sto"], PURGE: ["shift", "4"],
  LAST: ["shift", "9"], CONVERT: ["shift", "comma"],
};

const LETTER = (c) => `letter${c.toLowerCase()}`;
const PLAIN = {
  " ": ["space"], ".": ["point"], ",": ["comma"], "#": ["hash"], "{": ["lbrace"], "[": ["lbracket"], "(": ["lparen"],
  "«": ["program"], "=": ["equals"], "'": ["quote"],
  '"': ["shift", "hash"], "}": ["shift", "lbrace"], "]": ["shift", "lbracket"], ")": ["shift", "lparen"],
  "»": ["shift", "program"], "?": ["shift", "lc"], "°": ["shift", "letterw"], "µ": ["shift", "letterx"],
  "Σ": ["shift", "letterv"], "→": ["shift", "letteru"], "≤": ["shift", "letters"], "≥": ["shift", "lettert"],
  "<": ["shift", "lettery"], ">": ["shift", "letterz"], "≠": ["shift", "equals"], "π": ["shift", "point"],
  "^": ["shift", "multiply"], "%": ["shift", "2"], "√": ["shift", "minus"], "∫": ["shift", "5"], "∂": ["shift", "6"],
  "+": ["plus"], "-": ["minus"], "*": ["multiply"], "/": ["divide"],
};
/** Keys that run their function outside alpha entry mode, so a name holding one is typed in alpha mode. */
const RUNS = new Set(["+", "-", "*", "/", "^", "%", "√", "∫", "∂"]);
/** Keys that alpha entry mode types with a space before (not after a space) and after. */
const PADDED = new Set([...RUNS, "→", "≤", "≥", "<", ">", "≠", "π"]);

/**
 * The 48's translation codes (crates/saturnus-objects/src/charset.rs,
 * the table of ASCII transfers) for the characters the 28S has a key for
 * (`PLAIN`); its other characters have no key, nor has the backslash.
 */
const CODES28 = {
  "\\<<": "«", "\\>>": "»", "\\->": "→", "\\GS": "Σ", "\\pi": "π", "\\v/": "√", "\\.S": "∫",
  "\\.d": "∂", "\\<=": "≤", "\\>=": "≥", "\\=/": "≠", "\\^o": "°", "\\Gm": "µ",
};

/**
 * `text` with the 48's translation codes the 28S can type (`CODES28`)
 * turned into their characters. Any other backslash is an error that
 * says so: the 28S has no backslash key, and the rest of the 48's codes
 * stand for characters it has no key for.
 */
export function fromCodes28(text) {
  let out = "";
  for (let i = 0; i < text.length; ) {
    if (text[i] !== "\\") {
      out += text[i++];
      continue;
    }
    const code = text.slice(i, i + 3);
    if (!CODES28[code]) {
      throw new Error(`The 28S cannot type ${text.slice(i, i + 3)}: it has no backslash key, and of the 48's character codes it takes only ${Object.keys(CODES28).join(" ")} (for ${Object.values(CODES28).join(" ")}).`);
    }
    out += CODES28[code];
    i += 3;
  }
  return out;
}

/**
 * The menu labels shown on the bottom line of `frame` (the protocol's
 * `frame` event: base64 rows, most significant bit leftmost): six
 * inverse boxes 22 pixels wide, 23 apart (wiki: hardware/hp28s
 * "Display"), pixel rows 25-31: their top and bottom rows are dark and
 * the gaps between them light (28S ROM).
 */
export function labelsShown(frame) {
  if (!frame?.pixels || frame.width !== 137 || frame.height !== 32) return false;
  const bin = atob(frame.pixels);
  const row = Math.ceil(frame.width / 8);
  const dark = (x, y) => (bin.charCodeAt(y * row + (x >> 3)) >> (7 - (x & 7))) & 1;
  for (let i = 0; i < 6; i++) {
    for (let x = i * 23; x < i * 23 + 22; x++) if (!dark(x, 25) || !dark(x, 31)) return false;
    if (i < 5 && (dark(i * 23 + 22, 25) || dark(i * 23 + 22, 31))) return false;
  }
  return true;
}

/**
 * The keys that run (`verb` "run": the name typed, then ENTER) or insert
 * (`verb` "insert": typed only) command `name` on the 28S, given its
 * state: `shift` and `alpha` (annunciators), `lowercase` and `menuLock`
 * (`entryModes`), `labels` (`labelsShown`). Every mode it changes for the
 * name it gives back, except what ENTER ends itself (α, lowercase).
 */
export function keys28(name, verb, { shift = false, alpha = false, lowercase = false, menuLock = false, labels = false } = {}) {
  const out = [];
  // A pending shift would shift the first key.
  if (shift) out.push("shift");
  if (verb === "run" && KEY_COMMANDS[name]) return [...out, ...KEY_COMMANDS[name]];
  // Menu Lock swaps the letter keys A-R with their menus: off for the name.
  if (menuLock) out.push("shift", "alpha");
  const chars = [...name];
  for (const c of chars) {
    if (!PLAIN[c] && !/^[A-Za-z0-9]$/.test(c)) throw new Error(`The 28S has no key for ${c} in ${name}.`);
  }
  const inAlpha = alpha || chars.some((c) => RUNS.has(c));
  const padded = inAlpha && chars.some((c) => PADDED.has(c));
  // Taking a padding space out needs the cursor menu's ◀ and ▶ on the
  // menu keys, so a menu shown makes way for it, and comes back after.
  const cursor = padded && labels;
  if (cursor) out.push("cursor");
  if (inAlpha && !alpha) out.push("alpha");
  // A space first: the name is a word of its own in an open line.
  out.push("space");
  let lower = lowercase;
  let prev = " ";
  for (const c of chars) {
    if (/^[A-Za-z]$/.test(c)) {
      const want = c !== c.toUpperCase();
      if (want !== lower) {
        out.push("lc");
        lower = want;
      }
      out.push(LETTER(c));
    } else if (/^[0-9]$/.test(c)) {
      out.push(c);
    } else {
      out.push(...PLAIN[c]);
      if (inAlpha && PADDED.has(c)) {
        out.push("backspace");
        if (prev !== " ") out.push("e", "backspace", "f");
      }
    }
    prev = c;
  }
  if (cursor) out.push("cursor");
  if (verb === "run") {
    out.push("enter");
  } else {
    if (lower !== lowercase) out.push("lc");
    if (inAlpha && !alpha) out.push("alpha");
  }
  if (menuLock) out.push("shift", "alpha");
  return out;
}

/**
 * Wait until the calculator has played the queued keys and sleeps (two
 * looks in a row), at most `pace.limit` ms; then one more look's time
 * for the last frame.
 */
export async function settle(backend, store, pace = { first: 40, poll: 40, limit: 10_000 }) {
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const until = Date.now() + pace.limit;
  let calm = 0;
  await wait(pace.first);
  while (Date.now() < until) {
    const s = await backend.stats();
    if (s.loop === "sleep" && !store.state.keysDown?.length) {
      if (++calm >= 2) break;
    } else {
      calm = 0;
    }
    await wait(pace.poll);
  }
  await wait(pace.poll);
}

/**
 * Type `text` on the running 28S (`verb` "run" or "insert", `keys28`),
 * after reading the state the keys depend on.
 */
export async function send28(backend, store, verb, text, pace) {
  const frame = store.state.frame;
  const ann = frame?.annunciators ?? {};
  const modes = await backend.entryModes();
  const keys = keys28(text, verb, {
    shift: Boolean(ann.leftshift),
    alpha: Boolean(ann.alpha),
    lowercase: Boolean(modes.lowercase),
    menuLock: Boolean(modes.menuLock),
    labels: labelsShown(frame),
  });
  backend.typeKeys(keys);
  await settle(backend, store, pace);
  return { keys: keys.length };
}
