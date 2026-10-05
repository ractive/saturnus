// Calculator objects as the memory view shows them: the text form of a
// typed object (web/protocol.md, `objectAt` and `stack`), a program's
// source laid out in indented lines, and the model a preview is drawn
// from. Pure functions, no DOM: tested in web/test/ under Node.

/** Most list elements, program lines and string characters a preview shows. */
export const MAX_ITEMS = 200;
export const MAX_LINES = 300;
export const MAX_CHARS = 4000;
/** Most matrix rows and columns a preview shows. */
export const MAX_ROWS = 24;
export const MAX_COLS = 8;

/** The names the page uses for the object types. */
const TYPE_TITLES = {
  real: "Real number",
  integer: "Integer",
  complex: "Complex number",
  string: "String",
  name: "Global name",
  local_name: "Local name",
  character: "Character",
  binary: "Binary integer",
  list: "List",
  tagged: "Tagged object",
  unit: "Unit object",
  array: "Array",
  program: "Program",
  algebraic: "Algebraic",
  command: "Command",
  unknown: "Object",
};

/** A short type name for `obj`, e.g. "Program", "2 × 3 matrix". */
export function typeTitle(obj) {
  if (!obj || typeof obj !== "object") return "Object";
  if (obj.type === "array") {
    const d = obj.dims ?? [];
    return d.length === 1 ? `Vector of ${d[0]}` : `${d.join(" × ")} matrix`;
  }
  if (obj.type === "unknown") return obj.kind ?? `Object (prolog ${obj.prolog})`;
  return TYPE_TITLES[obj.type] ?? "Object";
}

/**
 * A real as the calculator writes it in STD format: up to 12 digits, no
 * leading zero before the point, `E` for the exponent; `dot` keeps the
 * point on whole numbers, as the 49G does to tell reals from integers.
 * A value beyond a double's range arrives as text and stays as it is.
 */
export function formatReal(value, dot = false) {
  if (typeof value === "string") return value;
  if (typeof value !== "number" || !Number.isFinite(value)) return String(value);
  if (value === 0) return dot ? "0." : "0";
  let [m, e] = value.toPrecision(12).split("e");
  if (m.includes(".")) m = m.replace(/0+$/, "");
  if (e !== undefined) {
    if (!m.includes(".")) m += ".";
    return `${m}E${Number(e)}`;
  }
  if (m.endsWith(".")) m = m.slice(0, -1);
  m = m.replace(/^(-?)0\./, "$1.");
  return dot && !m.includes(".") ? `${m}.` : m;
}

function integerText(value) {
  return typeof value === "string" ? value : String(Math.trunc(value));
}

function arrayText(items, o) {
  const parts = [];
  for (const it of items) {
    const t = Array.isArray(it) ? arrayText(it, o) : textOf(it, o, false);
    if (t === null) return null;
    parts.push(t);
  }
  return `[ ${parts.join(" ")} ]`;
}

/**
 * The text form of `obj` as the calculator writes it, or `null` if a part
 * of it has no text form (an object the host could not turn into text:
 * a program without `source`, an unnamed command). `opts.dot`: reals keep their point (49G). `top`: a name at
 * the top level is quoted, inside a list it is not.
 */
export function textOf(obj, opts = {}, top = true) {
  if (!obj || typeof obj !== "object") return null;
  switch (obj.type) {
    case "real":
      return formatReal(obj.value, opts.dot);
    case "integer":
      return integerText(obj.value);
    case "complex":
      return `(${formatReal(obj.re, opts.dot)},${formatReal(obj.im, opts.dot)})`;
    case "string":
      return `"${obj.value}"`;
    case "name":
    case "local_name":
      return top ? `'${obj.value}'` : String(obj.value);
    case "binary":
      return obj.text ?? `# ${obj.value}d`;
    case "list": {
      const parts = [];
      for (const it of obj.items ?? []) {
        const t = textOf(it, opts, false);
        if (t === null) return null;
        parts.push(t);
      }
      return parts.length ? `{ ${parts.join(" ")} }` : "{ }";
    }
    case "tagged": {
      const t = textOf(obj.object, opts, true);
      return t === null ? null : `:${obj.tag}: ${t}`;
    }
    case "unit":
      return obj.unit ? `${formatReal(obj.value, opts.dot)}_${obj.unit}` : null;
    case "array":
      return arrayText(obj.items ?? [], opts);
    case "program":
    case "algebraic":
      return obj.source ?? null;
    case "command":
      // An XLIB name the ROM's tables do not know is shown by its numbers.
      return obj.name ?? obj.source ?? (obj.library != null && obj.command != null ? `XLIB ${obj.library} ${obj.command}` : null);
    case "unknown":
      return obj.source ?? null;
    default:
      return null;
  }
}

/** What stands for an object without a text form inside a list or a line. */
export function placeholder(obj) {
  switch (obj?.type) {
    case "program": return "« … »";
    case "algebraic": return "'…'";
    case "command": return "‹command›";
    case "unit": return `${formatReal(obj.value)}_‹unit›`;
    default: return `‹${typeTitle(obj).toLowerCase()}›`;
  }
}

/**
 * One line for `obj` (a stack level, a list row): its text form, with
 * placeholders for the parts that have none, cut to `max` characters.
 * Returns `{text, complete}`.
 */
export function summary(obj, opts = {}, max = 120) {
  let complete = true;
  const walk = (o, top) => {
    const t = textOf(o, opts, top);
    if (t !== null) return t;
    if (o?.type === "list") return `{ ${(o.items ?? []).map((i) => walk(i, false)).join(" ")} }`;
    if (o?.type === "tagged") return `:${o.tag}: ${walk(o.object, true)}`;
    complete = false;
    return placeholder(o);
  };
  let text = walk(obj, true).replace(/\s+/g, " ");
  if (text.length > max) text = `${text.slice(0, max - 1)}…`;
  return { text, complete };
}

// ------------------------------------------------------------ programs

const OPEN = new Set(["IF", "IFERR", "CASE", "START", "FOR", "DO", "WHILE"]);
const MID = new Set(["THEN", "ELSE", "REPEAT", "UNTIL"]);
const CLOSE = new Set(["END", "NEXT", "STEP"]);
const LINE_WIDTH = 60;

/** RPL source as tokens: strings and quoted algebraics stay whole. */
export function tokens(source) {
  const out = [];
  const s = String(source);
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c)) {
      i++;
    } else if (c === "«" || c === "»") {
      out.push(c);
      i++;
    } else if (c === '"' || c === "'") {
      let j = s.indexOf(c, i + 1);
      if (j < 0) j = s.length - 1;
      out.push(s.slice(i, j + 1));
      i = j + 1;
    } else {
      let j = i;
      while (j < s.length && !/\s/.test(s[j]) && s[j] !== "«" && s[j] !== "»") j++;
      out.push(s.slice(i, j));
      i = j;
    }
  }
  return out;
}

/**
 * A program's source as indented lines `[{depth, text}]`: program
 * delimiters on their own lines, each structure word (IF, THEN, FOR,
 * NEXT, ...) starting a line, bodies one level in.
 */
export function indentRpl(source) {
  const lines = [];
  let depth = 0;
  let cur = null;
  const flush = () => {
    if (cur) lines.push({ depth: cur.depth, text: cur.words.join(" ") });
    cur = null;
  };
  const start = (d, word) => {
    flush();
    cur = { depth: Math.max(d, 0), words: [word], len: word.length };
  };
  for (const t of tokens(source)) {
    if (t === "«") {
      start(depth, t);
      flush();
      depth++;
    } else if (t === "»") {
      depth = Math.max(depth - 1, 0);
      start(depth, t);
      flush();
    } else if (OPEN.has(t)) {
      start(depth, t);
      depth++;
    } else if (MID.has(t)) {
      start(depth - 1, t);
    } else if (CLOSE.has(t)) {
      depth = Math.max(depth - 1, 0);
      start(depth, t);
      flush();
    } else if (t === "→") {
      start(depth, t);
    } else if (!cur) {
      start(depth, t);
    } else if (cur.len + 1 + t.length > LINE_WIDTH) {
      // A long run of words wraps, one level in from where it began.
      const d = cur.cont ?? cur.depth + (OPEN.has(cur.words[0]) || MID.has(cur.words[0]) ? 1 : 0);
      start(d, t);
      cur.cont = d;
    } else {
      cur.words.push(t);
      cur.len += 1 + t.length;
    }
  }
  flush();
  return lines;
}

// ------------------------------------------------------------ previews

function cut(list, max) {
  return list.length > max ? { shown: list.slice(0, max), more: list.length - max } : { shown: list, more: 0 };
}

/**
 * What a preview of `obj` shows, for `<sat-explorer>` to draw:
 * `{kind, title, copy, ...}`. `copy` is the whole text form, or `null`
 * while a part of the object has none.
 *
 * - `text`: `{text}` (numbers, names, units, tagged objects)
 * - `string`: `{text, length, more}`
 * - `program`: `{lines: [{depth, text}], more}` (also an algebraic: one line)
 * - `list`: `{items: [{text, complete, type}], count, more}`
 * - `matrix`: `{rows: [[text]], dims, moreRows, moreCols}` (a vector is one row)
 * - `unavailable`: `{reason, hex?, nibbles?, truncated?}`
 */
export function previewOf(obj, opts = {}) {
  const title = typeTitle(obj);
  const copy = textOf(obj, opts);
  const base = { title, copy };
  if (!obj || typeof obj !== "object") {
    return { ...base, kind: "unavailable", reason: "The calculator gave no object." };
  }
  switch (obj.type) {
    case "string": {
      const value = String(obj.value ?? "");
      const more = Math.max(value.length - MAX_CHARS, 0);
      return { ...base, kind: "string", text: value.slice(0, MAX_CHARS), length: value.length, more };
    }
    case "program":
    case "algebraic": {
      if (obj.source == null) break;
      const all = obj.type === "program" ? indentRpl(obj.source) : [{ depth: 0, text: String(obj.source) }];
      const { shown, more } = cut(all, MAX_LINES);
      return { ...base, kind: "program", lines: shown, more };
    }
    case "list": {
      const items = obj.items ?? [];
      const { shown, more } = cut(items, MAX_ITEMS);
      return {
        ...base,
        kind: "list",
        count: items.length,
        items: shown.map((it) => ({ ...summary(it, opts, 200), type: typeTitle(it) })),
        more,
      };
    }
    case "array": {
      const dims = obj.dims ?? [];
      const cell = (it) => (Array.isArray(it) ? arrayText(it, opts) : textOf(it, opts, false)) ?? "?";
      const rows = dims.length === 1 ? [obj.items ?? []] : (obj.items ?? []);
      const r = cut(rows, MAX_ROWS);
      let moreCols = 0;
      const shown = r.shown.map((row) => {
        const c = cut(Array.isArray(row) ? row : [row], MAX_COLS);
        moreCols = Math.max(moreCols, c.more);
        return c.shown.map(cell);
      });
      return { ...base, kind: "matrix", dims, rows: shown, moreRows: r.more, moreCols };
    }
    case "unknown":
      if (obj.source != null) return { ...base, kind: "text", text: obj.source };
      return {
        ...base,
        kind: "unavailable",
        reason: `${title}: this kind of object has no text form here.`,
        hex: obj.hex,
        nibbles: obj.nibbles,
        truncated: Boolean(obj.truncated),
      };
    default:
      if (copy !== null) return { ...base, kind: "text", text: copy };
  }
  const what = {
    program: "This program could not be turned back into text: it holds something the ROM's command tables do not name.",
    algebraic: "This expression could not be turned back into text: it holds a function the ROM's command tables do not describe.",
    unit: "This unit could not be turned back into text; its number is " + formatReal(obj.value, opts.dot) + ".",
    command: "A ROM object that the ROM's command tables do not name.",
    tagged: "The tagged object has no text form here.",
  }[obj.type] ?? "This object has no text form here.";
  return { ...base, kind: "unavailable", reason: what };
}

// ------------------------------------------------------------ variables

/** `12.5 bytes`, as BYTES reports a variable. */
export function sizeText(bytes) {
  const n = Number(bytes);
  return `${Number.isInteger(n) ? n : n.toFixed(1)} ${n === 1 ? "byte" : "bytes"}`;
}

/** A BYTES checksum as the calculator shows it in HEX: `# 3A5Fh`. */
export function checksumText(sum) {
  return `# ${Number(sum).toString(16).toUpperCase()}h`;
}

/** The variables of the directory at `path` (`["HOME", "A"]`) in `tree`, or null. */
export function directoryAt(tree, path) {
  let vars = tree;
  for (const name of path.slice(1)) {
    const dir = vars?.find((v) => v.name === name && Array.isArray(v.variables));
    if (!dir) return null;
    vars = dir.variables;
  }
  return vars ?? null;
}

/** Every variable under `vars` whose name contains `needle`, with its path. */
export function findVariables(vars, needle, path = ["HOME"], out = []) {
  const n = needle.toLowerCase();
  for (const v of vars ?? []) {
    if (v.name.toLowerCase().includes(n)) out.push({ variable: v, path });
    if (Array.isArray(v.variables)) findVariables(v.variables, needle, [...path, v.name], out);
  }
  return out;
}

// ------------------------------------------------------------ flags

/** The flag numbers that are set, from a `flags` reply. */
export function setFlags(flags) {
  return new Set(flags?.set ?? []);
}

/**
 * The rows of the flags panel for one model's entry of flags.json and a
 * `flags` reply: `{topics: [{topic, rows}], undocumented: [{flag, set, status}]}`.
 * A row is `{first, last, label, name, status, bits: [bool], set, now, other, field}`:
 * `now` is the meaning of the current state, `other` that of the opposite.
 */
export function flagRows(entry, topics, flags) {
  const on = setFlags(flags);
  const byTopic = new Map(topics.map((t) => [t, []]));
  const undocumented = [];
  for (const e of entry?.system ?? []) {
    const numbers = [];
    for (let n = e.first; n >= e.last; n--) numbers.push(n);
    const bits = numbers.map((n) => on.has(n));
    const documented = e.status !== "unknown" && e.status !== "unused" ? true : Boolean(e.clear || e.set || e.field);
    if (!documented) {
      for (const n of numbers) undocumented.push({ flag: n, set: on.has(n), status: e.status });
      continue;
    }
    const single = e.first === e.last;
    const set = single ? bits[0] : null;
    const row = {
      first: e.first,
      last: e.last,
      label: single ? String(e.first) : `${e.first} … ${e.last}`,
      name: e.name,
      status: e.status,
      bits,
      set,
      field: e.field ?? null,
      now: single ? ((set ? e.set : e.clear) ?? null) : null,
      other: single ? ((set ? e.clear : e.set) ?? null) : null,
    };
    if (!byTopic.has(e.topic)) byTopic.set(e.topic, []);
    byTopic.get(e.topic).push(row);
  }
  return {
    topics: [...byTopic].filter(([, rows]) => rows.length).map(([topic, rows]) => ({ topic, rows })),
    undocumented,
  };
}
