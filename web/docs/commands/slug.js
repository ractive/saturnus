// The command reference page's addresses (/saturnus/docs/commands/):
// `#48gx/hms-` is the HP 48GX's HMS-. A command's slug is its name in
// lower case with the calculator's characters as words (`→LIST` is
// `to-list`, `%CH` `pct-ch`, `∫` `integral`, `Σ+` `sigma+`); where two
// names of one model give the same slug (the 49G's QR and qr), each
// keeps its own name (percent-encoded where a hash needs it). Pure, shared by the page
// (page.js) and the build (web/docs/build.mjs, the Markdown files'
// links); web/test/docs.test.mjs checks every model's slugs are unique
// and lead back to their command.

/** The words for the characters of command names that are no letter or digit. */
const WORDS = {
  "→": "to", "←": "from", "↑": "up", "↓": "down", "Σ": "sigma", "π": "pi",
  "∫": "integral", "∂": "der", "√": "sqrt", "Δ": "delta", "Π": "prod",
  "λ": "lambda", "∞": "inf", "≤": "le", "≥": "ge", "≠": "ne", "%": "pct",
  "°": "deg", "²": "2", "¹": "1", "⁻": "minus", "ˣ": "x", "×": "times",
  "÷": "div", "−": "minus", "«": "open", "»": "close", "∡": "angle",
  "≷": "lg", "▲": "up", "▼": "down", "◀": "left", "▶": "right",
  "⇤": "start", "⇥": "end", "⬅": "back", "<": "lt", ">": "gt", "^": "pow",
  "|": "bar", "#": "hash", "&": "and", "~": "tilde",
};

/** Punctuation an address's hash carries as it is (RFC 3986's fragment characters). */
const PLAIN = /[!$&'*+,;=:@/?._~-]/;

/**
 * `text` for a hash: letters, digits and `PLAIN` as they are, anything
 * else percent-encoded (parentheses too, which would end a Markdown link).
 */
export function hashEncode(text) {
  let out = "";
  for (const c of text) {
    if (/[A-Za-z0-9]/.test(c) || PLAIN.test(c)) out += c;
    else out += encodeURIComponent(c).replace(/[()]/g, (p) => `%${p.charCodeAt(0).toString(16).toUpperCase()}`);
  }
  return out;
}

/** Characters that end a word but are no part of the slug. */
const SEPARATORS = new Set([" ", "(", ")"]);

/** The slug of `name` before collisions are settled. */
export function slugBase(name) {
  // Tokens: runs of letters and digits, the characters' words, other
  // punctuation as it is; a hyphen between two word tokens only.
  const tokens = [];
  let run = "";
  const flush = () => {
    if (run) tokens.push({ text: run.toLowerCase(), word: true });
    run = "";
  };
  for (const c of name) {
    if (/[A-Za-z0-9]/.test(c)) { run += c; continue; }
    flush();
    if (SEPARATORS.has(c)) { tokens.push({ sep: true }); continue; }
    if (WORDS[c]) tokens.push({ text: WORDS[c], word: true });
    else tokens.push({ text: hashEncode(c), word: false });
  }
  flush();
  let out = "";
  let prev = null;
  for (const t of tokens) {
    if (t.sep) continue;
    if (prev?.word && t.word) out += "-";
    out += t.text;
    prev = t;
  }
  return out || hashEncode(name);
}

/** Each of `names` (one model's commands) with its slug: a Map name → slug, unique. */
export function slugMap(names) {
  const groups = new Map();
  for (const n of names) {
    const s = slugBase(n);
    groups.set(s, [...(groups.get(s) ?? []), n]);
  }
  const out = new Map();
  for (const [s, ns] of groups) {
    for (const n of ns) out.set(n, ns.length === 1 ? s : hashEncode(n));
  }
  return out;
}

/**
 * The command of `slug` among `slugs` (a `slugMap`): the one with that
 * slug, else the one so named (the slug decoded), else one so named in
 * any case; null when none.
 */
export function commandOfSlug(slugs, slug) {
  for (const [n, s] of slugs) if (s === slug) return n;
  let decoded = slug;
  try { decoded = decodeURIComponent(slug); } catch { /* not encoded */ }
  if (slugs.has(decoded)) return decoded;
  const upper = decoded.toUpperCase();
  for (const n of slugs.keys()) if (n.toUpperCase() === upper) return n;
  return null;
}

/** `#48gx/hms-` for model `model` and slug `slug` (the model alone without one). */
export function refHash(model, slug = null) {
  return slug ? `#${model}/${slug}` : `#${model}`;
}

/** The model and slug of an address's hash: `{model, slug}` (either may be null). */
export function parseRefHash(hash) {
  const h = (hash ?? "").replace(/^#/, "");
  if (!h) return { model: null, slug: null };
  const at = h.indexOf("/");
  if (at < 0) return { model: h.toLowerCase(), slug: null };
  return { model: h.slice(0, at).toLowerCase(), slug: h.slice(at + 1) || null };
}
