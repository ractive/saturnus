// The command palette's editor mode, without the DOM: RPL text split into
// tokens and highlighted (delimiters, numbers, strings, names, the
// model's commands from the reference), bracket matching, indentation of
// nested `« »` and structure words, completion, the ASCII digraphs turned
// into the calculator's characters, the history of sent text, and the
// one function that sends an edit back to the calculator (`saveEdit`).
// `<sat-palette>` and `components/rpl-editor.js` draw it; web/test/ drives
// it in Node.

import { CODES, fromCodes } from "./reference.js";

/** Characters that end a word and are tokens of their own. */
const DELIMS = new Set(["«", "»", "{", "}", "[", "]", "(", ")", "'", ","]);
/** The structure words: open a construct, continue it, close it. */
const OPENERS = new Set(["IF", "IFERR", "CASE", "FOR", "START", "DO", "WHILE"]);
const MIDS = new Set(["THEN", "ELSE", "REPEAT", "UNTIL"]);
const CLOSERS = new Set(["END", "NEXT", "STEP"]);
/** Words shown as structure words (the local variables' arrow too). */
export const KEYWORDS = new Set([...OPENERS, ...MIDS, ...CLOSERS, "→"]);
const PAIRS = { "«": "»", "{": "}", "[": "]", "(": ")" };
const CLOSING = new Map(Object.entries(PAIRS).map(([o, c]) => [c, o]));
/** Algebraic operators, which split names inside `' '`. */
const OPERATORS = /([+\-*/^=<>≤≥≠|!√∂∫Σ]+)/;
const NUMBER = /^[-+]?(\d+[.,]?\d*|[.,]\d+)(E[-+]?\d+)?(_.+)?$/i;
const BINARY_DIGITS = /^[0-9A-F]+[hdobHDOB]?$/;

/**
 * `text` as tokens `{type, text, start, end}` covering it whole: `space`,
 * `comment` (`@` to the next `@` or the line's end), `string` (`"` to `"`,
 * `open: true` when unclosed), `tag` (`:name:`), `delim`, and `word` (any
 * other run of characters).
 */
export function tokenize(text) {
  const out = [];
  let i = 0;
  const push = (type, start, end, extra = {}) => out.push({ type, text: text.slice(start, end), start, end, ...extra });
  while (i < text.length) {
    const c = text[i];
    const start = i;
    if (/\s/.test(c)) {
      while (i < text.length && /\s/.test(text[i])) i++;
      push("space", start, i);
    } else if (c === '"') {
      const close = text.indexOf('"', i + 1);
      i = close < 0 ? text.length : close + 1;
      push("string", start, i, { open: close < 0 });
    } else if (c === "@") {
      let j = i + 1;
      while (j < text.length && text[j] !== "@" && text[j] !== "\n") j++;
      i = j < text.length && text[j] === "@" ? j + 1 : j;
      push("comment", start, i);
    } else if (c === ":" && /^:[^\s:"]+:/.test(text.slice(i, i + 130))) {
      i = text.indexOf(":", i + 1) + 1;
      push("tag", start, i);
    } else if (DELIMS.has(c)) {
      i++;
      push("delim", start, i);
    } else {
      while (i < text.length && !/\s/.test(text[i]) && !DELIMS.has(text[i]) && text[i] !== '"' && text[i] !== "@") i++;
      push("word", start, i);
    }
  }
  return out;
}

/**
 * The tokens with a class each for the highlighting (`cls`): `delim`,
 * `num`, `str`, `com`, `tag`, `kw` (structure words), `cmd` (the model's
 * commands, `commands` a Set of names), `var` (`variables`, a Set), `op`,
 * `name`. Inside `' '` names and operators are split.
 */
export function classify(tokens, { commands = new Set(), variables = new Set() } = {}) {
  const out = [];
  let quoted = false;
  let binary = false;
  const word = (t) => {
    const w = t.text;
    const upper = w.toUpperCase();
    if (binary && BINARY_DIGITS.test(w)) return "num";
    if (NUMBER.test(w)) return "num";
    if (w === "#") return "num";
    if (KEYWORDS.has(upper) || w === "->") return "kw";
    if (commands.has(w)) return "cmd";
    if (variables.has(w)) return "var";
    return "name";
  };
  for (const t of tokens) {
    if (t.type === "word") {
      if (quoted && !commands.has(t.text) && OPERATORS.test(t.text)) {
        // `X^2+SIN`: names, numbers and operators each with their class.
        let at = t.start;
        for (const part of t.text.split(OPERATORS)) {
          if (!part) continue;
          const sub = { type: "word", text: part, start: at, end: at + part.length };
          sub.cls = OPERATORS.test(part) && /^[+\-*/^=<>≤≥≠|!√∂∫Σ]+$/.test(part) ? "op" : word(sub);
          out.push(sub);
          at += part.length;
        }
        binary = false;
        continue;
      }
      out.push({ ...t, cls: word(t) });
      binary = t.text === "#";
      continue;
    }
    if (t.type === "delim" && t.text === "'") quoted = !quoted;
    if (t.type !== "space") binary = false;
    const cls = { delim: "delim", string: "str", comment: "com", tag: "tag", space: null }[t.type];
    out.push({ ...t, cls });
  }
  return out;
}

const escapeHtml = (s) => s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]);

/**
 * `text` as HTML for the editor's underlay: each classed token in a
 * `<span class="t-…">`, the characters at `marks` (offsets, a matched
 * bracket pair) in `<mark class="bm">`.
 */
export function highlight(text, ctx = {}, marks = []) {
  const toks = classify(tokenize(text), ctx);
  const at = new Set(marks);
  let html = "";
  for (const t of toks) {
    let inner = "";
    if (marks.some((m) => m >= t.start && m < t.end)) {
      for (let i = t.start; i < t.end; i++) {
        const ch = escapeHtml(text[i]);
        inner += at.has(i) ? `<mark class="bm">${ch}</mark>` : ch;
      }
    } else {
      inner = escapeHtml(t.text);
    }
    html += t.cls ? `<span class="t-${t.cls}">${inner}</span>` : inner;
  }
  // A last newline needs a line under it, or the underlay is one short.
  return text.endsWith("\n") ? `${html} ` : html;
}

/** The structure word `t` is, uppercase, or null. */
const structure = (t) => {
  if (t.type !== "word") return null;
  const u = t.text.toUpperCase();
  return OPENERS.has(u) || MIDS.has(u) || CLOSERS.has(u) ? u : null;
};

/**
 * The pair of the bracket at or just before `pos` (`« » { } [ ] ( )`,
 * outside strings and comments): `[open, close]` offsets, or null.
 */
export function matchBracket(text, pos) {
  const toks = tokenize(text).filter((t) => t.type === "delim" && (PAIRS[t.text] || CLOSING.has(t.text)));
  const here = toks.find((t) => t.start === pos) ?? toks.find((t) => t.end === pos);
  if (!here) return null;
  const i = toks.indexOf(here);
  if (PAIRS[here.text]) {
    let depth = 0;
    for (let j = i; j < toks.length; j++) {
      if (toks[j].text === here.text) depth++;
      else if (toks[j].text === PAIRS[here.text] && --depth === 0) return [here.start, toks[j].start];
    }
    return null;
  }
  const open = CLOSING.get(here.text);
  let depth = 0;
  for (let j = i; j >= 0; j--) {
    if (toks[j].text === here.text) depth++;
    else if (toks[j].text === open && --depth === 0) return [toks[j].start, here.start];
  }
  return null;
}

/**
 * What is still open at the end of `text`: the brackets and structure
 * words not closed, outermost first, and whether a string is open.
 */
export function openAt(text) {
  const stack = [];
  let string = false;
  for (const t of tokenize(text)) {
    if (t.type === "string") string = Boolean(t.open);
    if (t.type === "delim") {
      if (PAIRS[t.text]) stack.push(t.text);
      else if (CLOSING.has(t.text) && stack.at(-1) === CLOSING.get(t.text)) stack.pop();
      continue;
    }
    step(stack, structure(t));
  }
  return { stack, string };
}

/** One structure word on the stack of open constructs. */
function step(stack, word) {
  if (!word) return;
  if (OPENERS.has(word)) stack.push(word);
  // In CASE, `cond THEN … END` is a clause of its own.
  else if (word === "THEN" && stack.at(-1) === "CASE") stack.push("THEN");
  else if (CLOSERS.has(word) && stack.length && !PAIRS[stack.at(-1)]) stack.pop();
}

/** Whether `text` leaves a delimiter or a string open (the palette grows to the editor). */
export function unclosed(text) {
  const o = openAt(text);
  return o.string || o.stack.some((s) => PAIRS[s] !== undefined) || (tokenize(text).filter((t) => t.type === "delim" && t.text === "'").length % 2 === 1);
}

/** The indentation, in spaces, of one level. */
export const INDENT = "  ";

/** Whether the line `rest` (after its indentation) starts by closing or continuing a construct. */
function dedents(rest) {
  const t = tokenize(rest).find((x) => x.type !== "space");
  if (!t) return false;
  if (t.type === "delim") return CLOSING.has(t.text);
  const w = structure(t);
  return Boolean(w) && (CLOSERS.has(w) || MIDS.has(w));
}

/**
 * `text` with every line indented by its depth: one level for each open
 * `« { [ (` and structure word (IF, CASE and its clauses, FOR, START,
 * DO, WHILE, IFERR); THEN, ELSE, REPEAT and UNTIL sit at their
 * construct's level. Lines inside a string are left as they are, and the
 * text's tokens do not change.
 */
export function reindent(text) {
  const lines = text.split("\n");
  const out = [];
  const stack = [];
  let inString = false;
  for (const line of lines) {
    if (inString) {
      out.push(line);
    } else {
      const rest = line.replace(/^[ \t]+/, "");
      // THEN opens a clause in CASE, so it dedents only under IF/IFERR.
      const first = tokenize(rest).find((x) => x.type !== "space");
      const w = first ? structure(first) : null;
      let depth = stack.length;
      if (dedents(rest) && !(w === "THEN" && stack.at(-1) === "CASE")) depth = Math.max(0, depth - 1);
      out.push(rest ? INDENT.repeat(depth) + rest : "");
    }
    // Carry the state over the line (a string may continue).
    for (const t of tokenize((inString ? '"' : "") + line)) {
      if (t.type === "string") inString = Boolean(t.open);
      else if (t.type === "delim") {
        if (PAIRS[t.text]) stack.push(t.text);
        else if (CLOSING.has(t.text) && stack.at(-1) === CLOSING.get(t.text)) stack.pop();
      } else step(stack, structure(t));
    }
  }
  return out.join("\n");
}

/** Structure words a formatted program starts a line with, and those it ends one with. */
const BREAK_BEFORE = new Set(["IF", "IFERR", "CASE", "DO", "WHILE", "THEN", "ELSE", "END", "NEXT", "STEP", "REPEAT", "UNTIL"]);
const BREAK_AFTER = new Set(["CASE", "DO", "THEN", "ELSE", "END", "NEXT", "STEP", "REPEAT", "START"]);

/**
 * `text` laid out as a program is read: a program's `«` and `»` on lines
 * of their own (not inside a list or an array), each structure word
 * starting a line with what it takes (`IF cond`, `UNTIL cond`, the bounds
 * then `FOR` and its counter, in CASE a clause's condition with its
 * THEN), runs of spaces as one, the user's own line breaks kept, then
 * indented (`reindent`). Only whitespace outside strings and comments
 * changes, so the text compiles to the same object.
 */
export function format(text) {
  const toks = tokenize(text);
  const stack = [];
  let out = "";
  /** A line break before what comes next (and no space). */
  let pending = false;
  const newline = () => {
    out = out.replace(/[ \t]+$/, "");
    if (out && !out.endsWith("\n")) out += "\n";
  };
  const quoted = () => (out.match(/'/g)?.length ?? 0) % 2 === 1;
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i];
    if (t.type === "space") {
      if (t.text.includes("\n")) newline();
      else if (out && !out.endsWith("\n") && !pending) out += " ";
      continue;
    }
    const w = structure(t);
    const inCase = stack.at(-1) === "CASE";
    const inData = stack.at(-1) === "{" || stack.at(-1) === "[";
    const program = t.type === "delim" && (t.text === "«" || t.text === "»") && !quoted() && !(t.text === "«" ? inData : stack.at(-2) === "{" || stack.at(-2) === "[");
    if ((w && BREAK_BEFORE.has(w) && !(w === "THEN" && inCase)) || program || pending) {
      newline();
      pending = false;
    }
    out += t.text;
    if (t.type === "comment" && !t.text.endsWith("@")) newline();
    // The state for the next token.
    if (t.type === "delim") {
      if (PAIRS[t.text]) stack.push(t.text);
      else if (CLOSING.has(t.text) && stack.at(-1) === CLOSING.get(t.text)) stack.pop();
    } else step(stack, w);
    if (program && t.text === "«") pending = true;
    else if (w === "FOR") {
      // The counter stays with FOR.
      const next = toks.slice(i + 1).find((x) => x.type !== "space");
      if (next && next.type === "word") {
        out += ` ${next.text}`;
        i = toks.indexOf(next);
      }
      pending = true;
    } else if (w && BREAK_AFTER.has(w)) pending = true;
  }
  return reindent(out.replace(/[ \t]+$/gm, ""));
}

/**
 * The indentation the line ending at `end` of `text` should have, from
 * the lines before it and its own first token (`reindent`'s rule).
 */
export function lineIndent(text, end) {
  return reindent(text.slice(0, end)).split("\n").at(-1).match(/^[ \t]*/)[0];
}

/** The indentation for a new line after `before` (the text up to the cursor). */
export function indentAfter(before) {
  const o = openAt(before);
  return o.string ? "" : INDENT.repeat(o.stack.length);
}

/** The ASCII digraphs the editor turns into the calculator's characters as they are typed. */
export const DIGRAPHS = [["<<", "«"], [">>", "»"], ["->", "→"], ...CODES.map(([h, code]) => [code, h])];

/**
 * A digraph or translation code ending at `cursor` in `text`, outside
 * strings and comments, replaced by its character: `{text, cursor}`, or
 * null. `\->` wins over `->`.
 */
export function digraph(text, cursor) {
  const before = text.slice(0, cursor);
  const t = tokenize(before).at(-1);
  if (t && (t.type === "string" || t.type === "comment")) return null;
  const hit = DIGRAPHS.filter(([a]) => before.endsWith(a)).sort((a, b) => b[0].length - a[0].length)[0];
  if (!hit) return null;
  const [ascii, ch] = hit;
  return { text: text.slice(0, cursor - ascii.length) + ch + text.slice(cursor), cursor: cursor - ascii.length + ch.length };
}

/** `text` with every digraph and translation code outside strings turned into its character (pasted text). */
export function fromDigraphs(text) {
  return tokenize(text).map((t) => {
    if (t.type !== "word" && t.type !== "space") return t.text;
    let s = fromCodes(t.text) ?? t.text;
    s = s.replaceAll("<<", "«").replaceAll(">>", "»").replaceAll("->", "→");
    return s;
  }).join("");
}

/** The word the cursor ends: `{start, end, word}` (empty at a space). */
export function wordAt(text, cursor) {
  let start = cursor;
  while (start > 0 && !/[\s«»{}[\]()',"@]/.test(text[start - 1])) start--;
  // A word in an algebraic starts after its operator.
  const m = text.slice(start, cursor).match(/^.*[+\-*/^=<>]/);
  if (m) start += m[0].length;
  return { start, end: cursor, word: text.slice(start, cursor) };
}

/**
 * Up to `limit` completions of `word`: the model's commands (from the
 * palette's index, by name, translation codes or friendly spelling) and
 * the user's `variables` (names), shortest first; none for a number or a
 * word of less than two characters, nor for an exact name alone.
 */
export function completions(index, variables, word, limit = 8) {
  if (word.length < 2 || NUMBER.test(word) || word.startsWith("#")) return [];
  const upper = word.toUpperCase();
  const decoded = fromCodes(word)?.toUpperCase() ?? null;
  const out = [];
  for (const v of variables ?? []) {
    if (v.name.toUpperCase().startsWith(upper)) out.push({ name: v.name, kind: "variable", detail: "variable" });
  }
  for (const c of index?.commands ?? []) {
    if (c.upper.startsWith(upper) || c.codes.startsWith(upper) || (c.friendly && c.friendly.startsWith(upper)) || (decoded && c.upper.startsWith(decoded))) {
      out.push({ name: c.name, kind: "command", detail: c.stack });
    }
  }
  out.sort((a, b) => a.name.length - b.name.length || a.name.localeCompare(b.name));
  const seen = new Set();
  const list = out.filter((c) => !seen.has(c.name) && seen.add(c.name)).slice(0, limit);
  return list.length === 1 && list[0].name === word ? [] : list;
}

/** The text sent before, newest first, kept per browser (at most `max`). */
export class History {
  constructor(storage, { key = "saturnus.sent", max = 50 } = {}) {
    this.storage = storage;
    this.key = key;
    this.max = max;
    /** Where Alt+↑/↓ is: -1 is the text being edited. */
    this.at = -1;
    this.draft = "";
  }

  list() {
    try {
      const v = JSON.parse(this.storage?.getItem(this.key) ?? "[]");
      return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
    } catch {
      return [];
    }
  }

  /** `text` sent: first in the list, once. */
  push(text) {
    const t = text.trim();
    this.at = -1;
    if (!t) return;
    const list = [t, ...this.list().filter((x) => x !== t)].slice(0, this.max);
    try { this.storage?.setItem(this.key, JSON.stringify(list)); } catch { /* storage blocked */ }
  }

  /** One step back (`-1`, older) or forward (`1`) from `current`; the text to show, or null at an end. */
  move(dir, current) {
    const list = this.list();
    const to = this.at - dir;
    if (to < -1 || to >= list.length) return null;
    if (this.at === -1) this.draft = current;
    this.at = to;
    return to === -1 ? this.draft : list[to];
  }

  reset() {
    this.at = -1;
  }
}

/** What an editor edits: the free text of the palette, the calculator's command line, a variable or a stack level. */
export function targetTitle(target) {
  switch (target?.kind) {
    case "cmdline": return "the command line";
    case "variable": return `${target.name} in ${target.dir.join(" › ")}`;
    case "level": return `stack level ${target.level}`;
    default: return "text to send";
  }
}

/** `text` with its first letter in capitals, to start a sentence. */
export function sentence(text) {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

const message = (err) => String(err?.message ?? err);

/**
 * Send `text` back to where `target` came from, the editor's one
 * transport: a live command line through `replace` (the calculator stays
 * in its edit); a variable or a stack level through `storeText`, which
 * the calculator compiles (a hidden Kermit transaction; `was`, the
 * object's identity `editText` gave when the editor opened it, must still
 * be what is there). Text that leaves a string open is refused here, as
 * the host refuses it. Resolves to `{ok}`, or
 * `{ok: false, error, calculator}` with `calculator` true when the error
 * is the calculator's own (its compile), and nothing changed.
 */
export async function saveEdit(backend, target, text, was = null) {
  if (target.kind !== "cmdline" && openAt(text).string) {
    return { ok: false, error: "a string is not closed (a \" is missing)", calculator: false };
  }
  try {
    if (target.kind === "cmdline") {
      const r = await backend.replace(text);
      return { ok: true, commandLine: r?.commandLine ?? null };
    }
    const where = target.kind === "level" ? { level: target.level } : { dir: target.dir, name: target.name };
    const r = await backend.storeText({ ...where, text, was });
    if (r?.error) return { ok: false, error: r.error, calculator: true };
    return { ok: true };
  } catch (err) {
    return { ok: false, error: message(err), calculator: false };
  }
}

/**
 * The text of `target` to edit and the object's identity (`was`, its size
 * and checksum, the same in any display mode): `editText` for a variable
 * or a level.
 */
export async function pullEdit(backend, target) {
  const where = target.kind === "level" ? { level: target.level } : { dir: target.dir, name: target.name };
  const r = await backend.editText(where);
  return { text: r.text, was: r.was ?? null };
}

/**
 * Save `session`'s text (`saveEdit`) and bring the session up to date:
 * clean, and for an object `was` read again, since the next save checks
 * against it. If that read fails, the next save cannot be checked, so
 * the session is `broken` with why (it must be opened again). Resolves
 * to `saveEdit`'s result, with `reread` (the read's error) when it
 * failed. `keep()`, asked once the save went through, says whether the
 * session goes on (the editor stays open); when it does not, nothing is
 * read again.
 */
export async function saveSession(backend, session, text, { keep = () => true } = {}) {
  const r = await saveEdit(backend, session.target, text, session.was);
  if (!r.ok) return r;
  session.text = text;
  if (session.target.kind === "cmdline" || !keep()) {
    session.savedAs();
    return r;
  }
  try {
    session.savedAs((await pullEdit(backend, session.target)).was);
    return r;
  } catch (err) {
    session.savedAs();
    session.broken = `Saved, but ${targetTitle(session.target)} could not be read back (${message(err)}). Open it again before saving again.`;
    return { ...r, reread: message(err) };
  }
}

/**
 * What the edit shortcut (Cmd/Ctrl+E) opens, or null for nothing. It
 * follows the keys: with the keys in the memory view (`inView`), the
 * object selected there (`picked`, `{target, object}`, null when nothing
 * is); with the keys on the calculator, the calculator's open command
 * line (`cmdline`), else stack level 1 of `stack` (the levels, level 1
 * first, or null), whatever the view has selected. Nothing when the
 * calculator takes no edit now (`writable`: a 48SX, 48GX or 49G runs and
 * no write or typing runs), when the selected object has not arrived or
 * has no text form (as the Edit button), or when the stack is empty.
 */
export function editTarget({ writable, inView = false, picked = null, cmdline = false, stack = null }) {
  if (!writable) return null;
  if (inView && picked) return typeof picked.object?.text === "string" ? picked.target : null;
  if (cmdline) return { kind: "cmdline" };
  return typeof stack?.[0]?.text === "string" ? { kind: "level", level: 1 } : null;
}

/**
 * The Edit button's state (the top bar's and the one over the
 * calculator), which does what Cmd/Ctrl+E does (`editTarget`):
 * `{off, title}`, the tooltip naming the target and `key` (the edit
 * shortcut's label, "" for none) or why nothing can be edited. `booted`
 * the running model or null; `supported` it has the editor (48SX, 48GX,
 * 49G); `busy` a write or typing runs; `level1` stack level 1's object
 * (or null for an empty or unread stack); the rest as for `editTarget`.
 */
export function editButtonState({ booted, supported, busy, inView = false, picked = null, cmdline = false, level1 = null, key = "" }) {
  const hint = key ? ` (${key})` : "";
  if (!booted) return { off: true, title: "Start the calculator first" };
  if (!supported) return { off: true, title: `The HP ${booted.toUpperCase()} has no editor here (48SX, 48GX and 49G only)` };
  if (busy) return { off: true, title: "Wait: the calculator is busy" };
  const target = editTarget({ writable: true, inView, picked, cmdline, stack: level1 ? [level1] : null });
  if (target) return { off: false, title: `Edit ${targetTitle(target)}${hint}` };
  if (inView && picked) {
    return { off: true, title: picked.object ? `${sentence(targetTitle(picked.target))} has no text form to edit` : "Still reading the selection from the calculator" };
  }
  return { off: true, title: level1 ? "Stack level 1 has no text form to edit" : "Nothing to edit: the stack is empty and no command line is open" };
}

/**
 * One step of giving the focus back to the memory view's Edit after an
 * editor opened from it by keyboard closed. The view redraws as the
 * calculator's memory changes, so the Edit there now may be replaced,
 * missing until the object is read again, or disabled while a write
 * runs: the focus goes to an enabled Edit at every redraw for a moment
 * (`expired` ends it), then to the selected row if there is none.
 * `focusFree`: the focus is on nothing or already in the preview (not
 * moved elsewhere by the user, which ends it). Returns "edit", "row",
 * "wait" or "drop".
 */
export function editFocusStep({ editEnabled, focusFree, expired }) {
  if (!focusFree) return "drop";
  if (editEnabled) return "edit";
  return expired ? "row" : "wait";
}

/**
 * What the editor does after saving `target` (`saveSession`'s result
 * `r`, after `ms`): once it went through it closes, `{close: true,
 * message}` with the status line's message (null for the command line,
 * which the calculator shows in its edit); else it stays open with the
 * text as it was, `{close: false, notice}` the error to show. Text typed
 * while it saved (`changed`) keeps it open too, unsaved, with the save's
 * message, or why the next save cannot go (`broken`, the session's: the
 * object could not be read back).
 */
export function afterSave(target, r, ms, { changed = false, broken = null } = {}) {
  if (!r.ok) {
    const text = r.calculator ? `The calculator says: ${r.error}. Nothing was changed.` : `Not saved: ${r.error}${/[.!?]$/.test(r.error) ? "" : "."}`;
    return { close: false, notice: { text, error: true, calculator: r.calculator } };
  }
  const name = targetTitle(target);
  const message = target.kind === "cmdline" ? null : `Saved ${name} in ${(ms / 1000).toFixed(2)} s.`;
  if (changed) return { close: false, notice: broken ? { text: broken, error: true } : { text: message ?? "Sent back.", error: false } };
  return { close: true, message };
}

/**
 * The editor's own keys, before the text area's (`e`, a keydown): the
 * Save or Send back button, or Run with free text (`"primary"`: Cmd/Ctrl+S
 * when something is edited, `target`, or Cmd/Ctrl+Enter), Insert
 * (`"secondary"`, Cmd/Ctrl+Shift+Enter), the history (`"older"`,
 * `"newer"`: Alt+↑/↓), `"format"` (Shift+Alt+F), `"none"` for Cmd/Ctrl+S
 * on free text (taken, so the browser does not save the page), else null.
 */
/** The editor's Format key, as a binding combo (for its label). */
export const FORMAT_KEY = "Alt+Shift+KeyF";

export function editorKey(e, target) {
  const mod = e.metaKey || e.ctrlKey;
  if (mod && !e.altKey && e.key.toLowerCase() === "s") return target ? "primary" : "none";
  if (mod && e.key === "Enter") return e.shiftKey ? "secondary" : "primary";
  if (e.altKey && !mod && (e.key === "ArrowUp" || e.key === "ArrowDown")) return e.key === "ArrowUp" ? "older" : "newer";
  if (e.altKey && e.shiftKey && !mod && e.code === "KeyF") return "format";
  return null;
}

/**
 * One editing session: what is edited, the text it opened with (`was`),
 * and the text now. `dirty` when they differ.
 */
export class EditSession {
  /**
   * `shown` is what the editor shows (a program laid out), `was` the
   * object's identity from `editText` (null for free text and the command
   * line).
   */
  constructor(target = null, shown = "", was = null) {
    this.target = target;
    /** The object's identity when it was opened or last saved. */
    this.was = target && target.kind !== "cmdline" ? was : null;
    this.saved = shown;
    this.text = shown;
    /** Why it cannot be saved (it could not be read), or null. */
    this.broken = null;
  }

  get dirty() {
    return this.target !== null && this.text !== this.saved;
  }

  /** A save went through: the editor's text is clean; the calculator now holds the object `was` (when it was read again). */
  savedAs(was) {
    this.saved = this.text;
    if (this.was !== null && was !== undefined) this.was = was;
  }
}
