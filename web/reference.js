// The command reference in the page: the lookup rules of `saturnus ref`
// (crates/saturnus-cli/src/reference.rs: an exact name wins, the
// calculator's ASCII translation codes and friendly spellings count as
// the name, collisions list every candidate), the ranking of the command
// palette's suggestions and its Enter rule. Pure functions over the data
// of web/commands.json (scripts/commands-json.py); tested by web/test/.

/**
 * The calculator's translation codes for the HP characters in command
 * names: what an ASCII transfer writes for them (wiki:
 * protocols/hp-object-format, "trigraphs"). No command name holds a
 * backslash, so a code is never part of a name.
 */
export const CODES = [
  ["\u221a", "\\v/"], ["\u222b", "\\.S"], ["\u03a3", "\\GS"], ["\u25b6", "\\|>"],
  ["\u03c0", "\\pi"], ["\u2202", "\\.d"], ["\u2264", "\\<="], ["\u2265", "\\>="],
  ["\u2260", "\\=/"], ["\u2192", "\\->"], ["\u2193", "\\|v"], ["\u2191", "\\|^"],
  ["\u03bb", "\\Gl"], ["\u0394", "\\GD"], ["\u03a0", "\\PI"], ["\u221e", "\\oo"],
  ["\u00ab", "\\<<"], ["\u00bb", "\\>>"],
];

/** A friendly ASCII spelling per HP character: `→LIST` as `->LIST`, `Σ+` as `SIGMA+`. */
const FRIENDLY = {
  "\u2192": "->", "\u03a3": "SIGMA", "\u03c0": "PI", "\u2202": "DER", "\u222b": "INT",
  "\u221a": "SQRT", "\u2264": "<=", "\u2265": ">=", "\u2260": "<>", "\u2191": "UP",
  "\u2193": "DOWN", "\u0394": "DELTA", "\u03a0": "PROD", "\u03bb": "LAMBDA",
  "\u25b6": ">", "\u221e": "INF", "\u00ab": "<<", "\u00bb": ">>",
};

/** `name` with its HP characters as translation codes. */
export function toCodes(name) {
  let out = "";
  for (const c of name) out += CODES.find(([h]) => h === c)?.[1] ?? c;
  return out;
}

/** `query` with translation codes turned into HP characters, or null when it holds none. */
export function fromCodes(query) {
  if (!query.includes("\\")) return null;
  let out = query;
  for (const [h, code] of CODES) out = out.replaceAll(code, h);
  return out;
}

/** The friendly ASCII spelling of `name`, uppercase. */
export function friendly(name) {
  let out = "";
  for (const c of name) out += FRIENDLY[c] ?? c.toUpperCase();
  return out;
}

// eslint-disable-next-line no-control-regex
const ASCII = /^[\x00-\x7f]*$/;

/**
 * The index of one model's commands, built once from the data: each
 * command with the forms a query is matched against, and the ROM's
 * menus as a tree. `model` may have no catalog (the 38G, the 42S): the
 * index is then empty and says so.
 */
export function buildIndex(data, model) {
  const commands = [];
  const names = Object.keys(data.commands);
  const upperOf = new Map(names.map((n) => [n, n.toUpperCase()]));
  for (const name of names) {
    const entry = data.commands[name];
    const per = entry.models[model];
    if (!per) continue;
    const f = friendly(name);
    // A friendly spelling is the command's own only when no other command
    // is spelt so (`INT` is the 49G's INT, so `∫` keeps only `\.S`); the
    // others still count as an alias, ranked below an exact name.
    const unique = !ASCII.test(name) && !names.some((o) => o !== name && (upperOf.get(o) === f || (!ASCII.test(o) && friendly(o) === f)));
    const examples = per.examples ?? [];
    commands.push({
      name,
      upper: upperOf.get(name),
      codes: toCodes(name).toUpperCase(),
      friendly: ASCII.test(name) ? null : f,
      friendlyUnique: unique,
      description: entry.description,
      descUpper: entry.description.toUpperCase(),
      descWords: entry.description.toUpperCase().split(/[^A-Z0-9→]+/).filter(Boolean),
      exampleText: examples.map((x) => [x.setup, x.input, x.run].filter(Boolean).join(" ")).join("\n").toUpperCase(),
      stack: entry.stack,
      entry,
      per,
    });
  }
  commands.sort((a, b) => a.upper.localeCompare(b.upper) || a.name.localeCompare(b.name));
  const byName = new Map(commands.map((c) => [c.name, c]));
  return {
    model,
    data,
    commands,
    byName,
    menus: menuTree(commands, data.menuKeys?.[model] ?? []),
    supported: commands.length > 0,
  };
}

/** The headings of the menu tree that are not menus of the ROM. */
export const OTHER_MENUS = "Other menus";
export const NOT_IN_MENU = "Not in a ROM menu";

/** Manual categories that say where a key is, not which menu. */
const NOT_MENUS = new Set(["Keyboard", "Catalog", "Other", "Internal"]);

/**
 * What the manuals call a menu no key opens (`MENU 21`): the category
 * most of its commands are placed in, by the manual of this model or else
 * of another, when at least half of them agree; else null. On the 48SX
 * `MENU 21` is MODES (its second page), `MENU 23` MEMORY.
 */
export function menuLabel(commands) {
  const counts = new Map();
  for (const c of commands) {
    let cat = c.per.key?.category;
    if (!cat) {
      for (const per of Object.values(c.entry.models ?? {})) {
        if (per.key?.category) { cat = per.key.category; break; }
      }
    }
    const top = cat?.split(" ")[0];
    if (top && !NOT_MENUS.has(top)) counts.set(top, (counts.get(top) ?? 0) + 1);
  }
  let best = null;
  for (const [cat, n] of counts) if (!best || n > best[1] || (n === best[1] && cat < best[0])) best = [cat, n];
  return best && best[1] * 2 >= commands.length ? best[0] : null;
}

/**
 * The ROM's menus as one tree: `{name, title, path, kind, commands,
 * children, fromRom}` from the menu paths of the commands (`MTH BASE BIT`;
 * `MENU 74 TVM` for a menu no key opens), roots in the order the keys are
 * on the model. A `MENU n` the manuals name (`menuLabel`) is titled so,
 * under the key menu of that name when the model has one (the 48SX's
 * `MENU 21` under MODES), else among the roots; the others go under the
 * heading "Other menus". Commands no menu offers come last, under the
 * heading "Not in a ROM menu", grouped by what places them: the manual's
 * key or our own group. `kind` is `menu`, `heading` or `placement`.
 */
export function menuTree(commands, menuKeys) {
  const roots = new Map();
  const node = (name, path) => ({ name, path, commands: [], children: new Map() });
  const place = (pathText, c) => {
    const parts = pathText.split(" ");
    const steps = [];
    // `MENU n` is one name of two words.
    for (let i = 0; i < parts.length; i++) {
      if (parts[i] === "MENU" && /^\d+$/.test(parts[i + 1] ?? "")) steps.push(`MENU ${parts[++i]}`);
      else steps.push(parts[i]);
    }
    let level = roots;
    let n = null;
    for (let i = 0; i < steps.length; i++) {
      const path = steps.slice(0, i + 1).join(" ");
      n = level.get(steps[i]) ?? node(steps[i], path);
      level.set(steps[i], n);
      level = n.children;
    }
    if (n && !n.commands.includes(c)) n.commands.push(c);
  };
  const elsewhere = new Map();
  for (const c of commands) {
    const menus = c.per.menus ?? [];
    for (const m of menus) place(m, c);
    if (!menus.length) {
      const label = c.per.key?.category ?? c.entry.group ?? "Other";
      if (!elsewhere.has(label)) elsewhere.set(label, node(label, label));
      elsewhere.get(label).commands.push(c);
    }
  }
  const all = (n) => [...n.commands, ...[...n.children.values()].flatMap(all)];
  const menuNumber = (name) => (name.startsWith("MENU ") ? Number(name.slice(5)) : null);
  const order = (map, keys) => {
    const list = [...map.values()];
    const rank = (n) => {
      const i = keys.indexOf(n.name);
      return i < 0 ? (menuNumber(n.name) !== null ? 2000 + menuNumber(n.name) : 1000) : i;
    };
    list.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
    return list.map((n) => ({ ...n, title: n.title ?? n.name, kind: "menu", children: order(n.children, []), fromRom: true }));
  };
  // The numbered menus: named where the manuals say, under their key menu
  // when there is one; the rest under one heading.
  const unnamed = new Map();
  for (const [name, n] of [...roots]) {
    if (menuNumber(name) === null) continue;
    const label = menuLabel(all(n));
    if (!label) {
      roots.delete(name);
      unnamed.set(name, n);
      continue;
    }
    n.title = `${label} (${name})`;
    n.label = label;
    const parent = roots.get(label);
    if (parent && menuNumber(label) === null) {
      roots.delete(name);
      parent.children.set(name, n);
    }
  }
  const rom = order(roots, menuKeys);
  const heading = (name, children, fromRom) => ({
    name, title: name, path: name, kind: "heading", commands: [], children, fromRom,
  });
  if (unnamed.size) rom.push(heading(OTHER_MENUS, order(unnamed, []), true));
  const rest = [...elsewhere.values()].sort((a, b) => (a.name === "Keyboard" ? -1 : b.name === "Keyboard" ? 1 : a.name.localeCompare(b.name)));
  if (rest.length) {
    rom.push(heading(NOT_IN_MENU, rest.map((n) => ({
      ...n, name: n.name, title: n.name, path: `${NOT_IN_MENU} ${n.name}`, kind: "placement", children: [], fromRom: false,
    })), false));
  }
  return rom;
}

/** The menu of `tree` at `path` and the paths of the menus above it, or null. */
export function findMenu(tree, path, above = []) {
  for (const n of tree) {
    if (n.path === path) return { menu: n, above };
    const found = findMenu(n.children, path, [...above, n.path]);
    if (found) return found;
  }
  return null;
}

/**
 * The commands of `index` matching `query` in the Commands tab, by the
 * palette's ranking (`search`): names before descriptions.
 */
export function findCommands(index, query) {
  return search(index, query, { typing: false, limit: Infinity })
    .filter((r) => r.kind === "command")
    .map((r) => r.command);
}

/** Every menu of the tree, depth first, with its depth. */
export function flattenMenus(tree, depth = 0, out = []) {
  for (const n of tree) {
    out.push({ ...n, depth });
    flattenMenus(n.children, depth + 1, out);
  }
  return out;
}

/** The commands of a menu and of every menu under it, once each. */
export function menuCommands(menu) {
  const seen = new Set();
  const out = [];
  const walk = (n) => {
    for (const c of n.commands) if (!seen.has(c.name)) { seen.add(c.name); out.push(c); }
    for (const k of n.children) walk(k);
  };
  walk(menu);
  return out;
}

/**
 * Whether `text` could be one name (a command's, a variable's): one
 * token without a space or a delimiter that starts an object. `13 4 ^`,
 * `« 1 2 + »`, `'X'` and `"a"` are text to send as typed.
 */
export function isSingleToken(text) {
  const t = text.trim();
  return t.length > 0 && !/\s/.test(t) && !/^[0-9#'"«{[(:]/.test(t) && !/^[-+.]?[0-9]/.test(t);
}

/**
 * Score tiers; a higher tier goes first. Within a tier a shorter name
 * goes first, then the user's own variable before a command of the same
 * length, then the alphabet.
 */
const T = {
  exact: 1000, exactOther: 990, alias: 950, variableExact: 900,
  prefix: 800, variablePrefix: 800, menu: 770,
  within: 600, variableWithin: 600, action: 590, actionWithin: 560, // stem sits above within: `STO` for "store"
  stem: 650, word: 400, description: 350, example: 200,
};
const KIND_ORDER = { variable: 0, command: 1, menu: 2, action: 3, send: 4 };

/**
 * The suggestions for `query`: the model's commands (by the lookup rules
 * above, then prefix, substring, description and example text), the
 * user's `variables` (`[{name, path}]`), the `actions` (`[{id, title,
 * keywords?}]`), the ROM's menus, and "send as typed" for text that is
 * not a single name. `typing` is false where no text can be sent (no
 * ROM, a model without a command line): then no send row and no
 * variables. At most `limit` rows, each `{kind, name, score, ...}`.
 */
export function search(index, query, { variables = [], actions = [], typing = true, limit = 60 } = {}) {
  const q = query.trim();
  const rows = [];
  if (!q) {
    for (const a of actions) rows.push(actionRow(a, T.action));
    return rows.slice(0, limit);
  }
  const upper = q.toUpperCase();
  const decoded = fromCodes(q);
  const decodedUpper = decoded?.toUpperCase() ?? null;
  const words = upper.split(/\s+/).filter(Boolean);
  const single = isSingleToken(q);

  for (const c of index.commands) {
    let score = 0;
    if (c.name === q) score = T.exact;
    else if (c.upper === upper || (decoded !== null && c.name === decoded) || c.upper === decodedUpper) score = T.exactOther;
    else if (c.friendly !== null && (c.friendly === upper || c.codes === upper)) score = c.friendlyUnique ? T.exactOther : T.alias;
    else if (c.upper.startsWith(upper) || (c.friendly !== null && c.friendly.startsWith(upper)) || c.codes.startsWith(upper) || (decodedUpper !== null && c.upper.startsWith(decodedUpper))) score = T.prefix;
    else if (c.upper.includes(upper) || (c.friendly !== null && c.friendly.includes(upper)) || (decodedUpper !== null && c.upper.includes(decodedUpper))) score = T.within;
    else if (single && c.upper.length >= 2 && upper.startsWith(c.upper)) score = T.stem;
    else if (single && c.descWords.some((w) => w.startsWith(upper))) score = T.word;
    else if (words.every((w) => c.descUpper.includes(w))) score = T.description;
    else if (c.exampleText.includes(upper)) score = T.example;
    if (score) rows.push({ kind: "command", name: c.name, command: c, stack: c.stack, description: c.description, score });
  }
  if (typing) {
    for (const v of variables) {
      const u = v.name.toUpperCase();
      let score = 0;
      if (v.name === q || u === upper) score = T.variableExact;
      else if (u.startsWith(upper)) score = T.variablePrefix;
      else if (u.includes(upper)) score = T.variableWithin;
      if (score) rows.push({ kind: "variable", name: v.name, path: v.path, type: v.type, score });
    }
  }
  for (const m of flattenMenus(index.menus)) {
    if (m.kind !== "menu") continue;
    const last = m.name.toUpperCase();
    if (last.startsWith(upper) && !last.startsWith("MENU ")) {
      rows.push({ kind: "menu", name: m.path, menu: m, score: T.menu, count: menuCommands(m).length });
    }
  }
  for (const a of actions) {
    const title = a.title.toUpperCase();
    const keys = (a.keywords ?? "").toUpperCase();
    let score = 0;
    if (title.split(/\s+/).some((w) => w.startsWith(upper)) || keys.split(/\s+/).some((w) => w.startsWith(upper))) score = T.action;
    else if (words.every((w) => title.includes(w) || keys.includes(w))) score = T.actionWithin;
    if (score) rows.push(actionRow(a, score));
  }
  // Within a name tier a shorter name goes first (so is a name the query
  // begins with, `STO` for "store"); a description or example match is
  // ordered by name alone (a short operator's name says nothing about how
  // well its description matched).
  const byLength = (a, b) => (a.score >= T.stem ? a.name.length - b.name.length : 0);
  // Operators (`-`, `!`) after the words.
  const letters = (r) => (/^[A-Za-z]/.test(r.name) ? 0 : 1);
  rows.sort((a, b) => b.score - a.score || byLength(a, b) || KIND_ORDER[a.kind] - KIND_ORDER[b.kind] || letters(a) - letters(b) || a.name.localeCompare(b.name));
  const out = rows.slice(0, limit);
  if (typing) {
    // Text that is not a single name is sent as typed, first; a bare
    // token that names nothing exactly can still be sent, last.
    const send = { kind: "send", name: q, score: 0 };
    const exact = out.some((r) => r.score >= T.exactOther || r.score === T.variableExact);
    if (!single) out.unshift(send);
    else if (!exact) out.push(send);
  }
  return out;
}

function actionRow(a, score) {
  return { kind: "action", name: a.title, action: a, description: a.description ?? "", score };
}

/** The verb Enter uses on `row` given the command line's state; `opposite` is Cmd/Ctrl+Enter. */
export function enterVerb(row, commandLine, opposite = false) {
  const active = Boolean(commandLine?.active);
  let verb;
  // Variables insert their name (as the plan says), commands and typed
  // text mimic the keys: executed with no line open, inserted into one.
  if (row.kind === "variable") verb = "insert";
  else verb = active ? "insert" : "run";
  if (opposite) verb = verb === "insert" ? "run" : "insert";
  return verb;
}

/**
 * What Enter sends for `row`: `{verb, text}` for a command, a variable
 * or typed text, `{action}` for an app action, `{menu}` for a menu, or
 * null. Inserted into an open line, a name gets the space the calculator
 * would put between it and its neighbours.
 */
export function enterPlan(row, commandLine, opposite = false) {
  if (!row) return null;
  if (row.kind === "action") return { action: row.action };
  if (row.kind === "menu") return { menu: row.menu };
  const verb = enterVerb(row, commandLine, opposite);
  let text = row.name;
  if (verb === "insert" && commandLine?.active) text = spaced(text, commandLine);
  else if (verb === "run" && commandLine?.active && row.kind !== "send") text = spaced(text, commandLine);
  return { verb, text };
}

/** `text` with a space before it after a non-space, and one after it before a non-space. */
export function spaced(text, { text: line, cursor }) {
  const chars = [...(line ?? "")];
  const before = chars[(cursor ?? 0) - 1];
  const after = chars[cursor ?? 0];
  const lead = before !== undefined && !/\s/.test(before) ? " " : "";
  const tail = after !== undefined && !/\s/.test(after) ? " " : "";
  return `${lead}${text}${tail}`;
}

/** The text "try it" sends for an example: its setup, input and command. */
export function exampleText(example) {
  return [example.setup, example.input, example.run].filter(Boolean).join(" ");
}

/** The example's result as the calculator showed it: display lines, level 1 last, or its error. */
export function exampleResult(example) {
  if (example.error) return { error: example.error };
  const lines = [...(example.display ?? [])].reverse();
  return { lines };
}

/**
 * Where `command` is on `model`: `{menus, key, keyboard, group}` with the
 * ROM's menus, the manual's statement (`{category, manual, title, page,
 * url, other}`), the keyboard legend (`SIN key`, from `legends`, the
 * skin's keys) and our own group only where nothing else places it.
 */
export function placement(index, command, legends = null) {
  const per = command.per;
  const manual = per.key ? index.data.manuals[per.key.manual] : null;
  const key = per.key
    ? {
      category: per.key.category,
      manual: per.key.manual,
      title: manual?.title ?? per.key.manual,
      page: per.key.page,
      url: manual && per.key.page ? `${manual.url}#page=${per.key.page}` : null,
      other: Boolean(per.key.other),
    }
    : null;
  const menus = per.menus ?? [];
  const keyboard = !menus.length && !key && legends ? keyLegend(legends, command.name) : null;
  const group = !menus.length && !key && !keyboard ? command.entry.group ?? null : null;
  return { menus, key, keyboard, group };
}

/** The key of a skin whose legend is `name`: `SIN key`, `left shift, ASIN key`. */
export function keyLegend(keys, name) {
  const cap = (k) => (k.label ? k.label : String(k.name).toUpperCase());
  for (const k of keys) {
    if (k.label === name) return `${cap(k)} key`;
    if (k.left === name) return `left shift, ${cap(k)} key`;
    if (k.right === name) return `right shift, ${cap(k)} key`;
  }
  return null;
}

/** Deep links into the manuals for `command`, those of `models` (default: all). */
export function manualLinks(index, command, models = null) {
  const pages = command.entry.pages ?? {};
  const out = [];
  for (const [id, m] of Object.entries(index.data.manuals)) {
    if (models && !m.models.some((x) => models.includes(x))) continue;
    const page = pages[id];
    if (page) out.push({ id, title: m.title, page, url: `${m.url}#page=${page}`, models: m.models });
  }
  return out;
}

/** Whether an example of `command` ran it on `model` without an error: the stack effect was seen. */
export function stackVerified(command) {
  return (command.per.examples ?? []).some((x) => !x.error && x.run.split(/\s+/).includes(command.name));
}
