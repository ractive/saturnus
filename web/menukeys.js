// The menu keys' tooltips name what the screen shows above them (48SX,
// 48GX, 49G): the host reads the current menu and its page from RAM and
// sends the six keys in its `menu` event when they change (web/protocol.md;
// crates/saturnus-objects/src/softkeys.rs); the full name of a command
// comes from the command reference (web/commands.json, `buildIndex`'s
// `longName`). Pure functions; <sat-calculator> puts the lines in the keys'
// SVG <title>.

/** The models whose menu keys are read, and the keys: A-F (the 49G's F1-F6). */
export const MENU_KEYS = {
  "48sx": ["a", "b", "c", "d", "e", "f"],
  "48gx": ["a", "b", "c", "d", "e", "f"],
  "49g": ["a", "b", "c", "d", "e", "f"],
};

/** The position (0-5) of key `name` among `model`'s menu keys, or -1. */
export function menuKeyIndex(model, name) {
  return MENU_KEYS[model]?.indexOf(name) ?? -1;
}

/** Whether any of the keys runs a command (the reference is then wanted for its name). */
export function wantsReference(keys) {
  return Boolean(keys?.some((k) => k?.command));
}

/**
 * The tooltip line of one menu key (`{label, command?, submenu?, type?}`,
 * the event's), or null for a blank or unknown key. `longName(command)`
 * gives the reference's full name or null.
 *
 * - a command: `EXPAN · Expand products`; a label other than its
 *   command's name says which it runs: `T: TAN · Tangent`
 * - a submenu: `VECTR menu`
 * - a variable: `A7 · variable (real number)`, a directory `SUBD · directory`
 * - anything else: its label
 */
export function menuKeyLine(key, longName = () => null) {
  if (!key?.label) return null;
  if (key.type) {
    const t = key.type.toLowerCase();
    return t === "directory" ? `${key.label} · directory` : `${key.label} · variable (${t})`;
  }
  if (key.submenu) return `${key.label} menu`;
  if (key.command) {
    const long = longName(key.command);
    const head = key.label === key.command ? key.command : `${key.label}: ${key.command}`;
    return long ? `${head} · ${long}` : head;
  }
  return key.label;
}

/**
 * The tooltip of `model`'s key `name`: `base` (the key's own lines, its
 * name and click hints) under the line of what the screen shows above it
 * now, when `keys` (the `menu` event's, null when unknown) say.
 */
export function menuKeyTitle(model, name, base, keys, longName) {
  const i = menuKeyIndex(model, name);
  const line = i >= 0 && keys ? menuKeyLine(keys[i], longName) : null;
  return line ? `${line}\n${base}` : base;
}

/** `longName` from a reference index (`buildIndex`), or one that knows none. */
export function longNames(index) {
  return (command) => index?.byName?.get(command)?.longName ?? null;
}
