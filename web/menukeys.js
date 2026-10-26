// The menu keys' tooltips name what the screen shows above them: the host
// reads the six keys from RAM and sends them in its `menu` event when they
// change (web/protocol.md). On the 48SX, 48GX and 49G it knows each key's
// command (crates/saturnus-objects/src/softkeys.rs), on the 28S too, from
// its table of menus (crates/saturnus-host/src/menulabels.rs); the full
// name comes from the command reference (web/commands.json, `buildIndex`'s
// `longName`). On the 42S, 27S, 17BII, 17B and 19BII the host sends the
// labels alone, and the key reference (`keyModels` in commands.json) says
// what each names: `labelIndex`, `keyedLine`. Pure functions;
// <sat-calculator> puts the lines in the keys' SVG <title>.

const AF = ["a", "b", "c", "d", "e", "f"];

/** The models whose menu keys are read, and the keys: A-F (the 49G's F1-F6), the 42S's top row. */
export const MENU_KEYS = {
  "48sx": AF,
  "48gx": AF,
  "49g": AF,
  "28s": AF,
  "42s": ["sigmaplus", "inv", "sqrt", "log", "ln", "xeq"],
  "27s": AF,
  "17bii": AF,
  "17b": AF,
  "19bii": AF,
};

/** The models whose labels are named from the key reference (`keyModels`). */
export const KEYED = new Set(["42s", "27s", "17bii", "17b", "19bii"]);

/** The position (0-5) of key `name` among `model`'s menu keys, or -1. */
export function menuKeyIndex(model, name) {
  return MENU_KEYS[model]?.indexOf(name) ?? -1;
}

/** Whether the reference is wanted for `model`'s `keys`: a key runs a command, or a key-driven model shows labels. */
export function wantsReference(keys, model = null) {
  return Boolean(keys?.some((k) => k?.command || (KEYED.has(model) && k?.label)));
}

/** A label as the key reference spells it: no mode mark, `…` for `...`, the minus sign as the ROM's hyphen. */
function plainLabel(label) {
  return label.replace(/■$/, "").replace(/\.\.\./g, "…").replace(/−/g, "-");
}

/**
 * Whether the label a key shows (`shown`, from the buffer) is the
 * reference's `label`: the same, the same without the buffer's spaces
 * (`C X,Y` for `CX,Y`), or cut short in the box and so in the reference
 * (`TRANS` shows as `TRAN`).
 */
export function sameLabel(shown, label) {
  const a = plainLabel(shown);
  const b = plainLabel(label);
  return a === b || a.replace(/ /g, "") === b || ([...b].length >= 3 && a.length > b.length && a.startsWith(b));
}

const labelIndexes = new WeakMap();

/**
 * The menu labels of a key-driven model's reference (`index`, from
 * `buildIndex`: its `km` and `commands`): every step of a function's keys
 * that presses a menu key under a menu, as `{label, row, name, long,
 * description, submenu}`, and each row's labels (`rows`: the keys
 * before it → its labels). A menu key is under a menu after any key but a
 * shift on the 42S and the 27S, always on the models whose normal state
 * shows MAIN (17BII, 17B, 19BII); a shifted menu key is its printed
 * function.
 */
export function labelIndex(index) {
  if (!index?.km) return null;
  if (labelIndexes.has(index)) return labelIndexes.get(index);
  const km = index.km;
  const menuKeys = km.menuKeys ?? [];
  const main = (km.booted ?? []).some((l) => l.startsWith("["));
  const entries = [];
  const rows = new Map();
  const seen = new Map();
  for (const c of index.commands) {
    const steps = c.entry?.keys;
    if (!steps?.length) continue;
    let opened = main;
    for (let i = 0; i < steps.length; i++) {
      const s = steps[i];
      const shifted = i > 0 && steps[i - 1].key === "shift";
      if (opened && !shifted && s.label && menuKeys.includes(s.key)) {
        const row = JSON.stringify(steps.slice(0, i).map((x) => [x.key, x.label ?? null]));
        if (!rows.has(row)) rows.set(row, new Set());
        rows.get(row).add(plainLabel(s.label));
        const last = i === steps.length - 1;
        const e = {
          label: s.label,
          row,
          name: last ? c.name : null,
          long: last ? c.longName : null,
          description: last ? c.description : null,
          submenu: !last,
        };
        // One entry per row and label, a function's before a submenu's.
        const id = `${row}\u0000${s.label}`;
        const old = seen.get(id);
        if (old === undefined) {
          seen.set(id, entries.length);
          entries.push(e);
        } else if (entries[old].submenu && !e.submenu) entries[old] = e;
      }
      if (s.key !== "shift") opened = true;
    }
  }
  const out = { entries, rows };
  labelIndexes.set(index, out);
  return out;
}

/** What an entry means: its function, or the menu it opens. */
function meaning(e) {
  return e.submenu ? `menu ${plainLabel(e.label)}` : `fn ${e.name}`;
}

/** The first sentence of a description. */
function firstSentence(text) {
  return (text ?? "").split(/(?<=\.)\s/)[0];
}

/**
 * The tooltip line of key `i` of a key-driven model's `keys` (the `menu`
 * event's labels), from `labels` (`labelIndex`), or null when the label is
 * not in the reference (a name the user gave, another language): the key
 * then keeps its own tooltip, as it does when the reference's menus of
 * that label share none of the other labels shown (the Solver's variables
 * A and B are not the forecast's B). A label that means different things
 * in different menus is told apart by the other labels in the row.
 *
 * - a function: `TRANS · Transpose`, its description's first sentence
 *   where the reference has no full name
 * - a submenu: `TVM menu`
 */
export function keyedLine(keys, i, labels) {
  return keyedMatch(keys, i, labels)?.line ?? null;
}

/** `keyedLine`'s line and the function it names (`command`, null for a submenu or a label of several meanings). */
export function keyedMatch(keys, i, labels) {
  const shown = keys?.[i]?.label;
  if (!shown || !labels) return null;
  const candidates = labels.entries.filter((e) => sameLabel(shown, e.label));
  if (!candidates.length) return null;
  // The other labels shown: a menu of the reference shares some of them
  // (not a menu of names the user gave that happens to hold one).
  const others = new Set((keys ?? []).filter((k, j) => k?.label && j !== i).map((k) => plainLabel(k.label)));
  const score = (e) => [...(labels.rows.get(e.row) ?? [])].filter((l) => others.has(l)).length;
  const top = Math.max(...candidates.map(score));
  if (others.size && top === 0) return null;
  const best = candidates.filter((e) => score(e) === top);
  // Still more than one meaning: the label alone.
  if (new Set(best.map(meaning)).size > 1) return { line: shown, command: null };
  const e = best.find((x) => !x.submenu) ?? best[0];
  if (e.submenu) return { line: `${shown} menu`, command: null };
  const text = e.long ?? firstSentence(e.description);
  return { line: text ? `${shown} · ${text}` : shown, command: e.name };
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
 * The command menu key `name` of `model` runs now, as the command
 * reference names it, or null (a submenu, a variable, a blank or unknown
 * key, a label the reference does not know).
 */
export function menuKeyCommand(model, name, keys, index = null) {
  const i = menuKeyIndex(model, name);
  if (i < 0 || !keys) return null;
  if (KEYED.has(model)) return keyedMatch(keys, i, labelIndex(index))?.command ?? null;
  const k = keys[i];
  return k?.command && !k.type && !k.submenu ? k.command : null;
}

/** The tooltip line for looking a command up in Search with `shortcut` (`⌘K`), or null without one. */
export function lookupLine(command, shortcut) {
  return command && shortcut ? `${shortcut}: look up ${command}` : null;
}

/**
 * The tooltip of `model`'s key `name`: `base` (the key's own lines, its
 * name and click hints) under the line of what the screen shows above it
 * now, when `keys` (the `menu` event's, null when unknown) say, and for a
 * key that runs a command the line that looks it up with `shortcut` (the
 * palette's key, `⌘K`; none when unbound). `names` is `longNames(index)`;
 * on a key-driven model the line comes from the key reference (`index`),
 * and a label it does not know leaves `base`.
 */
export function menuKeyTitle(model, name, base, keys, names, index = null, shortcut = "") {
  const i = menuKeyIndex(model, name);
  let line = null;
  if (i >= 0 && keys) line = KEYED.has(model) ? keyedLine(keys, i, labelIndex(index)) : menuKeyLine(keys[i], names);
  if (!line) return base;
  const lookup = lookupLine(menuKeyCommand(model, name, keys, index), shortcut);
  return [line, lookup, base].filter(Boolean).join("\n");
}

/** `longName` from a reference index (`buildIndex`), or one that knows none. */
export function longNames(index) {
  return (command) => index?.byName?.get(command)?.longName ?? null;
}
