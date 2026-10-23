// A small Markdown renderer for the docs pages (web/docs/pages/), and
// their frontmatter. No dependency: the project has no JavaScript
// package manager, and the pages use a small subset that a short,
// tested renderer covers (web/test/docs.test.mjs):
//
//   # headings (1 to 4), paragraphs, `-` / `*` and `1.` lists (one
//   level; an indented line continues the item), > blockquotes,
//   ``` fenced code, | tables | with a --- row |, --- rules;
//   inline: `code`, **bold**, *italic* or _italic_, [links](url),
//   ![images](src), <https://autolinks>.
//
// Two forms for a model page's typography: a paragraph that starts with
// an image alone on its line is a <figure>, the paragraph's other lines
// its caption; a table whose header cells are all empty (`| | |`) has
// no header row (a list of facts, a label and a value per row).
// A paragraph that is one link into the app (`../app/...`, `../../app/...` from a sub-page) alone is
// its button, as the app's own, `appIcon(href)` before its text (the
// model's mark, from the build): "[Open the HP 48GX in saturnus](../app/#/calculator/48gx)".
//
// Everything else is text: HTML in a page is escaped, never passed
// through. `link(href)` lets the caller rewrite each link and image
// source (a page's `.md` links become its HTML page's).
//
// Links to other sites (web/shared/ext-links.js) open in a new tab, with
// a ↗ after their text and " (opens in a new tab)" for screen readers. A
// site named in the text ("hpcalc.org") is a link to it, at most twice
// per address on a page, its explicit links counted too (`budget`, one
// LinkBudget per page, shared by the page's render calls); a link beyond
// that is its text. Under a "Sources" heading (level 1 or 2, to the next
// one) links do not count and are always links, and names stay text.
// Never in code, in a link's text or in a heading. The Markdown itself
// is not changed: the .md beside the HTML stays as written.

import { EXT_ATTRS, EXT_MARKS_HTML, LinkBudget, isExternal, siteNameAt, siteUrl } from "../shared/ext-links.js";

/** `s` with &, <, > and " escaped for HTML text and attributes. */
export function escapeHtml(s) {
  return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/**
 * The frontmatter and body of `text`: `{meta, body}`. The frontmatter
 * is `key: value` lines between two `---` lines at the top; a value may
 * be quoted ("..." or '...'). No frontmatter: `meta` is empty.
 */
export function parseFrontmatter(text) {
  const src = text.replace(/\r\n?/g, "\n");
  const m = src.match(/^---\n([\s\S]*?)\n---\n?/);
  if (!m) return { meta: {}, body: src };
  const meta = {};
  for (const line of m[1].split("\n")) {
    if (!line.trim() || line.trimStart().startsWith("#")) continue;
    const kv = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
    if (!kv) throw new Error(`frontmatter: cannot read the line "${line}"`);
    let v = kv[2].trim();
    if ((v.startsWith('"') && v.endsWith('"') && v.length > 1) || (v.startsWith("'") && v.endsWith("'") && v.length > 1)) v = v.slice(1, -1);
    meta[kv[1]] = v;
  }
  return { meta, body: src.slice(m[0].length) };
}

/**
 * A link or image target the pages may use: relative, a fragment, http:,
 * https: or mailto:. Anything else (javascript:, data:, …) fails the build
 * rather than reaching the HTML.
 */
export function safeHref(href) {
  const scheme = href.trim().match(/^([a-z][a-z0-9+.-]*):/i)?.[1]?.toLowerCase();
  if (scheme && !["http", "https", "mailto"].includes(scheme)) throw new Error(`link to a ${scheme}: URL: ${href}`);
  return href;
}

/**
 * Inline Markdown of `text` as HTML; `link(href)` rewrites each link's
 * target. `opts`: `budget` (the page's LinkBudget), `autolink` (site
 * names become links; false in a heading and a link's text), `sources`
 * (in a list of sources: links do not count, names stay text).
 */
export function renderInline(text, rewrite = (h) => h, opts = {}) {
  const link = (h) => rewrite(safeHref(h));
  const { budget = new LinkBudget(), autolink = true, sources = false } = opts;
  const inner = (t, o = {}) => renderInline(t, link, { budget, autolink, sources, ...o });
  // An <a> for `href` with `html` as its text; another site's in a new tab,
  // or its text alone when the page has linked the address enough.
  const anchor = (href, html) => {
    if (!isExternal(href)) return `<a href="${escapeHtml(link(href))}">${html}</a>`;
    if (!sources && !budget.take(href)) return html;
    return `<a href="${escapeHtml(link(href))}" ${EXT_ATTRS}>${html}${EXT_MARKS_HTML}</a>`;
  };
  let out = "";
  let i = 0;
  const s = text;
  while (i < s.length) {
    const c = s[i];
    // A backslash escapes the next punctuation character.
    if (c === "\\" && i + 1 < s.length && /[\\`*_[\]()<>#!|-]/.test(s[i + 1])) {
      out += escapeHtml(s[i + 1]);
      i += 2;
      continue;
    }
    if (c === "`") {
      const run = s.slice(i).match(/^`+/)[0];
      const end = s.indexOf(run, i + run.length);
      if (end > 0) {
        out += `<code>${escapeHtml(s.slice(i + run.length, end).replace(/^ (.*) $/, "$1"))}</code>`;
        i = end + run.length;
        continue;
      }
    }
    if (c === "<") {
      const auto = s.slice(i).match(/^<(https?:\/\/[^\s<>]+)>/);
      if (auto) {
        out += anchor(auto[1], escapeHtml(auto[1]));
        i += auto[0].length;
        continue;
      }
    }
    if (c === "!" && s[i + 1] === "[") {
      const img = linkAt(s, i + 1);
      if (img) {
        out += `<img src="${escapeHtml(link(img.href))}" alt="${escapeHtml(img.label)}">`;
        i = img.end;
        continue;
      }
    }
    if (c === "[") {
      const a = linkAt(s, i);
      if (a) {
        out += anchor(a.href, inner(a.label, { autolink: false }));
        i = a.end;
        continue;
      }
    }
    if ((c === "*" || c === "_") && s[i + 1] === c) {
      const end = s.indexOf(c + c, i + 2);
      if (end > i + 2) {
        out += `<strong>${inner(s.slice(i + 2, end))}</strong>`;
        i = end + 2;
        continue;
      }
    }
    if (c === "*" || c === "_") {
      // Not inside a word for `_` (snake_case stays as it is).
      const opens = s[i + 1] && s[i + 1] !== " " && (c === "*" || !/\w/.test(s[i - 1] ?? ""));
      const end = opens ? findClose(s, c, i + 1) : -1;
      if (end > i + 1) {
        out += `<em>${inner(s.slice(i + 1, end))}</em>`;
        i = end + 1;
        continue;
      }
    }
    const name = /[a-z0-9]/i.test(c) ? siteNameAt(s, i) : null;
    if (name) {
      // A site's name: a link while the page allows, else its text.
      const url = autolink && !sources ? siteUrl(name) : null;
      out += url && budget.take(url) ? `<a href="${escapeHtml(url)}" ${EXT_ATTRS}>${escapeHtml(name)}${EXT_MARKS_HTML}</a>` : escapeHtml(name);
      i += name.length;
      continue;
    }
    out += escapeHtml(c);
    i++;
  }
  return out;
}

/** The closing `c` of an emphasis opened before `from`, or -1. */
function findClose(s, c, from) {
  for (let j = from; j < s.length; j++) {
    if (s[j] === "\\") { j++; continue; }
    if (s[j] === c && s[j - 1] !== " " && (c === "*" || !/\w/.test(s[j + 1] ?? ""))) return j;
  }
  return -1;
}

/**
 * A `[label](href)` at `i`: `{label, href, end}`, or null. The href may
 * hold balanced parentheses (a Wikimedia file name: `File:HP_48_(1).jpg`).
 */
function linkAt(s, i) {
  let depth = 0;
  let j = i;
  for (; j < s.length; j++) {
    if (s[j] === "\\") { j++; continue; }
    if (s[j] === "[") depth++;
    else if (s[j] === "]" && --depth === 0) break;
  }
  if (j >= s.length || s[j + 1] !== "(") return null;
  let close = -1;
  for (let k = j + 2, open = 1; k < s.length; k++) {
    if (s[k] === "(") open++;
    else if (s[k] === ")" && --open === 0) { close = k; break; }
  }
  if (close < 0) return null;
  const href = s.slice(j + 2, close).trim();
  if (!href || /\s/.test(href)) return null;
  return { label: s.slice(i + 1, j), href, end: close + 1 };
}

/** The cells of a table row `| a | b |`. */
function cells(line) {
  return line.trim().replace(/^\|/, "").replace(/\|$/, "").split(/(?<!\\)\|/).map((c) => c.trim());
}

const HEADING = /^(#{1,4})\s+(.*?)(?:\s+#+)?\s*$/;
const BULLET = /^[-*]\s+(.*)$/;
const NUMBER = /^\d+[.)]\s+(.*)$/;
const FENCE = /^```\s*([\w-]*)\s*$/;
const RULE = /^(-{3,}|\*{3,})\s*$/;
const FIGURE = /^!\[[^\]]*\]\([^\s]+\)$/;
const TABLE_SEP = /^\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;

/**
 * `markdown` as HTML (the subset above). `link(href)` rewrites link
 * targets and image sources; `headingId(text)` gives a heading its id
 * (default: none).
 */
export function renderMarkdown(markdown, { link = (h) => h, headingId = null, appIcon = () => "", budget = new LinkBudget(), state = { sources: false } } = {}) {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n");
  const out = [];
  const inline = (t) => renderInline(t, link, { budget, sources: state.sources });
  let i = 0;
  const isBlockStart = (l) => HEADING.test(l) || FENCE.test(l) || BULLET.test(l) || NUMBER.test(l) || l.startsWith(">") || RULE.test(l) || l.trimStart().startsWith("|");
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) { i++; continue; }
    const fence = line.match(FENCE);
    if (fence) {
      const body = [];
      i++;
      while (i < lines.length && !/^```\s*$/.test(lines[i])) body.push(lines[i++]);
      if (i >= lines.length) throw new Error("markdown: a code block is not closed");
      i++;
      const cls = fence[1] ? ` class="language-${escapeHtml(fence[1])}"` : "";
      out.push(`<pre><code${cls}>${escapeHtml(body.join("\n"))}</code></pre>`);
      continue;
    }
    const h = line.match(HEADING);
    if (h) {
      const level = h[1].length;
      const id = headingId ? headingId(h[2]) : null;
      if (level <= 2) state.sources = /^sources$/i.test(h[2].trim());
      out.push(`<h${level}${id ? ` id="${escapeHtml(id)}"` : ""}>${renderInline(h[2], link, { budget, autolink: false, sources: state.sources })}</h${level}>`);
      i++;
      continue;
    }
    if (RULE.test(line)) {
      out.push("<hr>");
      i++;
      continue;
    }
    if (line.trimStart().startsWith("|") && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1].trim())) {
      const head = cells(line);
      const align = cells(lines[i + 1]).map((c) => (c.startsWith(":") && c.endsWith(":") ? "center" : c.endsWith(":") ? "right" : null));
      i += 2;
      const rows = [];
      while (i < lines.length && lines[i].trimStart().startsWith("|")) rows.push(cells(lines[i++]));
      const td = (tag, c, k) => `<${tag}${align[k] ? ` style="text-align: ${align[k]}"` : ""}>${inline(c)}</${tag}>`;
      const thead = head.some((c) => c) ? `<thead><tr>${head.map((c, k) => td("th", c, k)).join("")}</tr></thead>\n` : "";
      out.push(`<div class="table-wrap"><table>\n${thead}<tbody>\n${rows.map((r) => `<tr>${head.map((_, k) => td("td", r[k] ?? "", k)).join("")}</tr>`).join("\n")}\n</tbody>\n</table></div>`);
      continue;
    }
    if (line.startsWith(">")) {
      const body = [];
      while (i < lines.length && lines[i].startsWith(">")) body.push(lines[i++].replace(/^> ?/, ""));
      out.push(`<blockquote>\n${renderMarkdown(body.join("\n"), { link, headingId, budget, state })}\n</blockquote>`);
      continue;
    }
    const ordered = NUMBER.test(line);
    if (ordered || BULLET.test(line)) {
      const re = ordered ? NUMBER : BULLET;
      const items = [];
      while (i < lines.length) {
        const m = lines[i].match(re);
        if (m) {
          items.push([m[1]]);
          i++;
        } else if (items.length && /^\s{2,}\S/.test(lines[i])) {
          items.at(-1).push(lines[i].trim());
          i++;
        } else break;
      }
      const tag = ordered ? "ol" : "ul";
      out.push(`<${tag}>\n${items.map((it) => `<li>${inline(it.join(" "))}</li>`).join("\n")}\n</${tag}>`);
      continue;
    }
    if (FIGURE.test(line.trim())) {
      const caption = [];
      i++;
      while (i < lines.length && lines[i].trim() && !isBlockStart(lines[i])) caption.push(lines[i++].trim());
      out.push(`<figure>${inline(line.trim())}${caption.length ? `<figcaption>${inline(caption.join(" "))}</figcaption>` : ""}</figure>`);
      continue;
    }
    const para = [];
    while (i < lines.length && lines[i].trim() && !(para.length && isBlockStart(lines[i]))) para.push(lines[i++].trim());
    const only = para.length === 1 && para[0].startsWith("[") ? linkAt(para[0], 0) : null;
    if (only && only.end === para[0].length && /^(\.\.\/)+app\//.test(only.href)) {
      const a = inline(para[0]).replace(/^<a ([^>]*)>(.*)<\/a>$/, `<a class="button primary" $1>${appIcon(only.href)}<span>$2</span></a>`);
      out.push(`<p class="docs-action">${a}</p>`);
      continue;
    }
    out.push(`<p>${inline(para.join(" "))}</p>`);
  }
  return out.join("\n");
}
