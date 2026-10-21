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
// Everything else is text: HTML in a page is escaped, never passed
// through. `link(href)` lets the caller rewrite each link and image
// source (a page's `.md` links become its HTML page's).

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

/** Inline Markdown of `text` as HTML; `link(href)` rewrites each link's target. */
export function renderInline(text, rewrite = (h) => h) {
  const link = (h) => rewrite(safeHref(h));
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
        out += `<a href="${escapeHtml(link(auto[1]))}">${escapeHtml(auto[1])}</a>`;
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
        out += `<a href="${escapeHtml(link(a.href))}">${renderInline(a.label, link)}</a>`;
        i = a.end;
        continue;
      }
    }
    if ((c === "*" || c === "_") && s[i + 1] === c) {
      const end = s.indexOf(c + c, i + 2);
      if (end > i + 2) {
        out += `<strong>${renderInline(s.slice(i + 2, end), link)}</strong>`;
        i = end + 2;
        continue;
      }
    }
    if (c === "*" || c === "_") {
      // Not inside a word for `_` (snake_case stays as it is).
      const opens = s[i + 1] && s[i + 1] !== " " && (c === "*" || !/\w/.test(s[i - 1] ?? ""));
      const end = opens ? findClose(s, c, i + 1) : -1;
      if (end > i + 1) {
        out += `<em>${renderInline(s.slice(i + 1, end), link)}</em>`;
        i = end + 1;
        continue;
      }
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

/** A `[label](href)` at `i`: `{label, href, end}`, or null. */
function linkAt(s, i) {
  let depth = 0;
  let j = i;
  for (; j < s.length; j++) {
    if (s[j] === "\\") { j++; continue; }
    if (s[j] === "[") depth++;
    else if (s[j] === "]" && --depth === 0) break;
  }
  if (j >= s.length || s[j + 1] !== "(") return null;
  const close = s.indexOf(")", j + 2);
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
const TABLE_SEP = /^\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;

/**
 * `markdown` as HTML (the subset above). `link(href)` rewrites link
 * targets and image sources; `headingId(text)` gives a heading its id
 * (default: none).
 */
export function renderMarkdown(markdown, { link = (h) => h, headingId = null } = {}) {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n");
  const out = [];
  const inline = (t) => renderInline(t, link);
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
      out.push(`<h${level}${id ? ` id="${escapeHtml(id)}"` : ""}>${inline(h[2])}</h${level}>`);
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
      out.push(`<div class="table-wrap"><table>\n<thead><tr>${head.map((c, k) => td("th", c, k)).join("")}</tr></thead>\n<tbody>\n${rows.map((r) => `<tr>${head.map((_, k) => td("td", r[k] ?? "", k)).join("")}</tr>`).join("\n")}\n</tbody>\n</table></div>`);
      continue;
    }
    if (line.startsWith(">")) {
      const body = [];
      while (i < lines.length && lines[i].startsWith(">")) body.push(lines[i++].replace(/^> ?/, ""));
      out.push(`<blockquote>\n${renderMarkdown(body.join("\n"), { link, headingId })}\n</blockquote>`);
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
    const para = [];
    while (i < lines.length && lines[i].trim() && !(para.length && isBlockStart(lines[i]))) para.push(lines[i++].trim());
    out.push(`<p>${inline(para.join(" "))}</p>`);
  }
  return out.join("\n");
}
