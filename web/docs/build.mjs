#!/usr/bin/env node
// The docs of https://ractive.ch/saturnus/docs/ (iteration 57), built
// from the Markdown pages in web/docs/pages/ into a site directory
// (web/site.sh runs it after the app is in place):
//
//   node web/docs/build.mjs OUT [--csp POLICY] [--base URL]
//
// writes, under OUT:
//
//   docs/index.html, docs/index.md   the docs index (pages/index.md)
//   docs/<p>/index.html, docs/<p>.md every other page (pages/<p>.md):
//                                    the HTML at /saturnus/docs/<p>/,
//                                    its Markdown beside it, as written
//   docs/commands/index.html         the command reference page
//                                    (pages/commands.md, the page's
//                                    script commands/page.js and
//                                    commands/slug.js); it runs the app's
//                                    reference code from app/
//   docs/commands/<model>.md         every command of one model with its
//                                    examples, for LLMs (12 files)
//   docs/docs.css                    the pages' own styles
//   llms.txt                         the llmstxt.org index of the .md files
//   sitemap.xml                      the landing page, the app and the docs
//                                    pages, for search engines, each with
//                                    its lastmod from git (sitemapXml)
//
// A page's frontmatter: `title`, `description`, `section` (index,
// models, commands, saturn, transfer), `model` on a model page (the
// page takes its colours), `order` (a number) on a section's sub-page
// (`pages/saturn/cpu.md`: its place in llms.txt after the section's
// overview, `pages/saturn.md`), `status: placeholder` for a page not
// written yet (llms.txt leaves it out). A page links others by their .md path
// (`[48GX](48gx.md)`), which works in the Markdown as published; the
// HTML gets the page's address. The pages look like the app: its
// tokens, its stylesheet (the logo's colours per model, the reference
// entry), its theme script, all from app/.
//
// A model page may show a picture of its calculator from the landing
// page's images (`../img/<model>-640.webp`, saturnus's own drawing),
// as a figure with its caption (markdown.mjs).
//
// Checks, failing the build: every relative link and source of the HTML
// names a file of the site, no page links into the wiki, no file is
// empty. Pure functions exported for web/test/docs.test.mjs.

import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, posix } from "node:path";
import { fileURLToPath } from "node:url";

import { escapeHtml, parseFrontmatter, renderMarkdown } from "./markdown.mjs";
import { LinkBudget } from "../shared/ext-links.js";
import { slugMap } from "./commands/slug.js";
import { buildIndex, exampleBefore, exampleResult, exampleRun, keysText, legendText, manualLinks, placement } from "../reference.js";
import { STACK_NONE, stackEmpty } from "../components/entry-view.js";
import { MODEL_ORDER, MODEL_TITLES } from "../norom.js";

const HERE = dirname(fileURLToPath(import.meta.url));
const WEB = join(HERE, "..");
export const PAGES = join(HERE, "pages");
export const BASE = "https://ractive.ch/saturnus/";

/** The sections, in the order the index and the header list them. */
export const SECTIONS = [
  { id: "models", title: "Models", href: "index.md#models" },
  { id: "commands", title: "Command reference", href: "commands.md" },
  { id: "saturn", title: "How the Saturn works", href: "saturn.md" },
  { id: "transfer", title: "Moving files to a real calculator", short: "Moving files", href: "transfer.md" },
];

/** Links that must never appear: the hardware wiki (owner, 2026-10-21). */
const WIKI = /calculator-knowledgebase|github\.com\/ractive\/calculator/i;

/** Every page under `dir`: `{rel, meta, body, source}`, `rel` its path without `.md` ("index", "48gx", "saturn/cpu"). */
export function loadPages(dir = PAGES) {
  const out = [];
  const walk = (d, prefix) => {
    for (const name of readdirSync(d).sort()) {
      const p = join(d, name);
      if (statSync(p).isDirectory()) walk(p, `${prefix}${name}/`);
      else if (name.endsWith(".md")) {
        const source = readFileSync(p, "utf8");
        const { meta, body } = parseFrontmatter(source);
        const rel = `${prefix}${name.slice(0, -3)}`;
        for (const k of ["title", "description", "section"]) if (!meta[k]) throw new Error(`docs: ${rel}.md has no ${k}`);
        if (meta.model && !MODEL_ORDER.includes(meta.model)) throw new Error(`docs: ${rel}.md names the unknown model ${meta.model}`);
        out.push({ rel, meta, body, source });
      }
    }
  };
  walk(dir, "");
  return out;
}

/** The site path of a page's HTML folder ("docs/", "docs/48gx/") and of its Markdown ("docs/index.md", "docs/48gx.md"). */
export function pagePaths(rel) {
  return {
    html: rel === "index" ? "docs/" : `docs/${rel}/`,
    md: `docs/${rel}.md`,
    mdDir: `docs/${posix.dirname(rel) === "." ? "" : `${posix.dirname(rel)}/`}`,
  };
}

/** `to` (a site path; a folder ends in "/") relative to the folder `from`. */
export function relative(from, to) {
  const dir = to === "" || to.endsWith("/");
  let r = posix.relative(`/${from}`, `/${to}`);
  if (dir) r = r ? `${r}/` : "./";
  return r || "./";
}

/**
 * A link of page `rel`'s Markdown as its HTML needs it: relative links
 * resolved against the Markdown's folder, a page's `.md` turned into its
 * address, then made relative to the HTML's folder. Other sites,
 * absolute paths and anchors stay as they are.
 */
export function htmlLink(rel, href, pageRels) {
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("/") || href.startsWith("#")) return href;
  const { html, mdDir } = pagePaths(rel);
  const at = href.search(/[?#]/);
  const path = at < 0 ? href : href.slice(0, at);
  const rest = at < 0 ? "" : href.slice(at);
  let target = posix.normalize(posix.join(mdDir, path));
  if (path.endsWith("/") && !target.endsWith("/")) target += "/";
  const m = target.match(/^docs\/(.+)\.md$/);
  if (m && pageRels.has(m[1])) target = pagePaths(m[1]).html;
  return relative(html, target) + rest;
}

/** The mark of logo.svg (web/logo.js's LOGO_PARTS), coloured by style.css through `data-logo`. */
function logoSvg(size, model = null) {
  return `<svg class="logo" viewBox="0 0 32 32" width="${size}" height="${size}" aria-hidden="true"${model ? ` data-logo="${model}"` : ""}><path class="ring" d="M 1.6 16 A 14.4 4.6 0 0 1 30.4 16" transform="rotate(-24 16 16)"/><circle class="planet" cx="16" cy="16" r="8.6"/><path class="ring" d="M 30.4 16 A 14.4 4.6 0 0 1 1.6 16" transform="rotate(-24 16 16)"/></svg>`;
}

/** The icon of a button into the app: the mark of the model it opens, in its colours. */
function appIcon(href) {
  const model = href.match(/#\/(?:calculator|files)\/([a-z0-9]+)/)?.[1];
  return model ? logoSvg(18, model) : "";
}

/**
 * The drawings' sizes, from the landing page's <img> tags (img/48gx-320.webp
 * … width="1248" height="2721"): a model page's picture takes its own, so
 * the text does not move when it loads.
 */
let drawingSizes = null;
export function sizeImages(html) {
  if (!drawingSizes) {
    drawingSizes = new Map();
    const landing = readFileSync(join(WEB, "landing", "index.html"), "utf8");
    for (const m of landing.matchAll(/<img [^>]*src="img\/([a-z0-9]+)-320\.webp"[^>]*width="(\d+)" height="(\d+)"/g)) drawingSizes.set(m[1], [m[2], m[3]]);
  }
  return html.replace(/<img src="([^"]*\bimg\/([a-z0-9]+)-\d+\.webp)"/g, (all, src, model) => {
    const size = drawingSizes.get(model);
    return size ? `<img src="${src}" width="${size[0]}" height="${size[1]}" decoding="async"` : all;
  });
}

/** A heading's id: its words in lower case, joined by hyphens. */
function headingId(text) {
  return text.toLowerCase().replace(/[`*_[\]()]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

/** The page's HTML document: the header, the rendered body (and `extra` after it), the footer. */
export function pageHtml(page, bodyHtml, { csp = null, extra = "", after = "" } = {}) {
  const { html } = pagePaths(page.rel);
  const root = relative(html, "");
  const app = relative(html, "app/");
  const docs = relative(html, "docs/");
  const md = relative(html, pagePaths(page.rel).md);
  const model = page.meta.model ?? null;
  const nav = SECTIONS.map((s) => {
    const [file, anchor] = s.href.split("#");
    const href = relative(html, pagePaths(file.replace(/\.md$/, "")).html) + (anchor ? `#${anchor}` : "");
    const current = page.meta.section === s.id && page.rel !== "index";
    return `<a href="${escapeHtml(href)}"${current ? ' aria-current="page"' : ""}>${escapeHtml(s.short ?? s.title)}</a>`;
  }).join("\n    ");
  const title = page.rel === "index" ? page.meta.title : `${page.meta.title} · saturnus docs`;
  return `<!doctype html>
<html lang="en"${model ? ` data-logo="${model}"` : ""}>
<head>
<meta charset="utf-8">
${csp ? `<meta http-equiv="Content-Security-Policy" content="${escapeHtml(csp)}">\n` : ""}<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="color-scheme" content="light dark">
<title>${escapeHtml(title)}</title>
<meta name="description" content="${escapeHtml(page.meta.description)}">
<link rel="icon" href="${root}favicon.svg" type="image/svg+xml">
<link rel="alternate" type="text/markdown" href="${md}">
<link rel="stylesheet" href="${app}shared/tokens.css">
<link rel="stylesheet" href="${app}style.css">
<link rel="stylesheet" href="${docs}docs.css">
<script src="${app}shared/theme-boot.js"></script>
</head>
<body class="docs${model ? " docs-model" : ""} docs-${escapeHtml(page.meta.section)}">
<header class="docs-bar">
  <div class="docs-brand">
    <a class="brand" href="${root}" title="saturnus">${logoSvg(24)}<span>saturnus</span></a>
    <a class="docs-brand-sub" href="${docs}">docs</a>
  </div>
  <nav class="docs-nav" aria-label="Sections">
    ${nav}
  </nav>
  <a class="docs-app" href="${app}" title="Open the app"><svg class="ic docs-app-ic" viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="2.5" width="14" height="19" rx="2"/><path d="M8 6h8v4H8zM8.5 14h.01M12 14h.01M15.5 14h.01M8.5 17.5h.01M12 17.5h.01M15.5 17.5h.01"/></svg><span>Open the app</span></a>
</header>
<main class="docs-main">
<article class="docs-page">
<h1>${model ? logoSvg(36) : ""}<span>${escapeHtml(page.meta.title)}</span></h1>
${bodyHtml}
</article>
${extra}${after ? `\n<div class="docs-page docs-after">\n${after}\n</div>` : ""}
</main>
<footer class="docs-foot">
  <a href="${md}">This page as Markdown</a>
  <a href="${root}llms.txt">llms.txt</a>
</footer>
</body>
</html>
`;
}

/**
 * The command reference page's tool, after its lead (page.js fills it):
 * the bar, stuck to the top while the page scrolls, with the search
 * field (the model in it, its suggestions under it), on a phone the
 * Browse button, and the count line; the two panes as cards, the
 * navigation by menu (a skeleton until the data is there) and the
 * entry; the sheet the navigation moves into on a phone.
 */
function commandsExtra() {
  const bars = "<span></span>".repeat(6);
  return `<section class="docs-ref-app" aria-label="Commands">
<div id="ref-top" class="ref-top"></div>
<div id="ref-bar" class="ref-bar">
  <div class="ref-bar-row">
    <div class="ref-search" role="search">
      <svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20.5 20.5 16 16"/></svg>
      <input id="ref-q" type="search" role="combobox" aria-label="Search the commands" aria-expanded="false" aria-controls="ref-suggest" aria-autocomplete="list" placeholder="A name, ->LIST, or what it does" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" enterkeyhint="go">
      <span class="ref-model"><select id="ref-model" aria-label="Model"></select><svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><path d="m7 10 5 5 5-5"/></svg></span>
      <ul id="ref-suggest" class="ref-suggest" role="listbox" aria-label="Commands found" hidden></ul>
    </div>
    <button id="ref-browse" type="button" class="ref-browse" title="Browse the commands by menu" aria-haspopup="dialog" aria-controls="ref-sheet"><svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h10"/></svg><span>Browse</span></button>
  </div>
  <p id="ref-count" class="ref-count muted" role="status">Loading the reference…</p>
</div>
<div class="ref-panes">
  <aside id="ref-side" class="ref-side">
    <nav id="ref-nav" class="ref-nav" aria-label="Commands by menu"><div class="ref-skeleton" aria-hidden="true">${bars}</div></nav>
  </aside>
  <div id="ref-entry" class="ref-entry docs-ref"></div>
</div>
<dialog id="ref-sheet" class="ref-sheet" aria-labelledby="ref-sheet-title">
  <div class="ref-sheet-head"><h2 id="ref-sheet-title" class="ref-sheet-title">Browse</h2><button type="button" class="icon ref-sheet-close" aria-label="Close"><svg class="ic" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18"/></svg></button></div>
  <div class="ref-sheet-body"></div>
</dialog>
<noscript><p>The page needs JavaScript. Each model's commands are also a Markdown file; <a href="../../llms.txt">llms.txt</a> lists them.</p></noscript>
</section>
<script type="module" src="page.js"></script>`;
}

/** The manuals of `model` in `data` as Markdown links, for a Sources list. */
function manualSources(data, models) {
  return Object.values(data.manuals ?? {})
    .filter((m) => m.models.some((x) => models.includes(x)))
    .map((m) => `- [${m.title}](${m.url})`);
}

/** The models with a reference in `data`, in the app's order. */
export function referenceModels(data) {
  const has = new Set([...data.models, ...Object.keys(data.keyModels ?? {})]);
  return MODEL_ORDER.filter((m) => has.has(m));
}

/** A stack (`levels`) or a display as lines in a code block; "(empty stack)" for none, a display without its blank lines above and below. */
function block(lines, levels) {
  if (!levels) {
    const shown = [...lines];
    while (shown.length && !shown[0].trim()) shown.shift();
    while (shown.length && !shown.at(-1).trim()) shown.pop();
    return shown.length ? shown.join("\n") : "(blank)";
  }
  if (!lines.length) return "(empty stack)";
  return lines.map((l, j) => (levels ? `${lines.length - j}: ${l}` : l)).join("\n");
}

/** The stack effect as Markdown lines: a code block, or the text for an arrow alone (entry-view.js). */
function stackLines(stack) {
  return stackEmpty(stack) ? [`Stack: ${STACK_NONE}`, ""] : ["Stack:", "", "```", stack || "—", "```", ""];
}

/**
 * Every command of `model` as Markdown: its name and long name, its
 * stack effect or keys, the description and note, where it is, its
 * examples as the emulator showed them, the manuals' pages and its
 * address on the page. The same data and helpers as the page
 * (reference.js), in text.
 */
export function commandsMarkdown(data, model) {
  const index = buildIndex(data, model);
  const title = MODEL_TITLES[model] ?? model;
  const slugs = slugMap(index.commands.map((c) => c.name));
  const out = [
    `# ${title} commands`,
    "",
    `Every command of the ${title} in saturnus's reference (${index.commands.length} ${index.keyed ? "functions" : "commands"}): ${index.keyed ? "the keys that run it" : "its stack effect"}, what it does, where it is on the calculator, and examples with their results from saturnus's emulated ${title}. The same reference as a page: [the command reference](./${`#${model}`}).`,
    "",
    index.keyed
      ? `The ${title} has no command line: a function is pressed. An example shows the display before, the keys pressed, and the display after.`
      : "A stack effect reads as the manuals write it: the arguments, an arrow, the results. An example shows the stack before the command and after it, level 1 last, as the calculator displayed it.",
    "",
  ];
  for (const c of index.commands) {
    out.push(`## ${c.name}${c.longName ? ` · ${c.longName}` : ""}`, "");
    if (index.keyed) {
      const e = c.entry;
      if (c.byName) out.push(`Keys: XEQ, then \`"${e.xeq}"\` through the ALPHA menu, then ENTER`, "");
      else if (c.steps) out.push(`Keys: ${c.keysText}`, "");
      if (e.inserts) out.push(`Puts \`${e.inserts.trimEnd()}\` on the edit line`, "");
      if (e.param) out.push(`Asks for: ${e.param}`, "");
      if (c.stack) out.push(...stackLines(c.stack));
    } else {
      out.push(...stackLines(c.stack));
    }
    out.push(c.description, "");
    if (c.note) out.push(`Note: ${c.note}`, "");
    if (index.keyed) {
      if (c.entry.menu) out.push(`Menu: ${c.entry.menu}`, "");
    } else {
      const where = placement(index, c);
      if (where.menus.length) out.push(`Menu: ${where.menus.join(" · ")}`, "");
      if (where.key) out.push(`Key: ${where.key.category}`, "");
    }
    const examples = c.per.examples ?? [];
    examples.forEach((x, i) => {
      out.push(`Example${examples.length > 1 ? ` ${i + 1}` : ""}:`, "", "```");
      if (index.keyed) {
        const input = x.labels?.length ? x.labels.join(" ") : legendText(x.input);
        const then = x.then?.length ? ` ${legendText(x.then)}` : "";
        const run = x.in_menu ? keysText(exampleRun(c, x)) : c.keysText;
        if (input) out.push(`First:   ${input}`);
        out.push("Display:", block(x.before, false), `Keys:    ${run}${then}`, "Display:", block(x.after, false));
      } else {
        const r = exampleResult(x);
        if (x.setup) out.push(`After:   ${x.setup}`);
        out.push("Before:", block(exampleBefore(x), true), `Run:     ${x.run}`);
        out.push(r.error ? `Error:   ${r.error}` : `After:\n${block(r.lines, true)}`);
      }
      out.push("```", "");
      if (x.comment) out.push(x.comment, "");
    });
    if (!examples.length && c.per.skip) out.push(`No example: ${c.per.skip}.`, "");
    const links = index.keyed
      ? (data.manuals?.[index.km.manual] && c.entry.page ? [{ title: data.manuals[index.km.manual].title, page: c.entry.page, url: `${data.manuals[index.km.manual].url}#page=${c.entry.page}` }] : [])
      : manualLinks(index, c, [model]);
    if (links.length) out.push(`In the manuals: ${links.map((l) => `[${l.title}, p. ${l.page}](${l.url})`).join("; ")}`, "");
    out.push(`On the page: [${c.name}](./#${model}/${slugs.get(c.name)})`, "");
  }
  out.push("## Sources", "", ...manualSources(data, [model]), "");
  return out.join("\n");
}

/**
 * The written pages (no placeholders) of the section `id` in reading
 * order: the section's overview (the page named like the section,
 * "saturn"), then its sub-pages by `order`, then by path.
 */
export function sectionPages(pages, id) {
  const rank = (p) => (p.rel === id ? -Infinity : Number.isFinite(Number(p.meta.order)) && p.meta.order !== "" ? Number(p.meta.order) : Infinity);
  return pages
    .filter((p) => p.meta.section === id && p.meta.status !== "placeholder")
    .sort((a, b) => rank(a) - rank(b) || (a.rel < b.rel ? -1 : a.rel > b.rel ? 1 : 0));
}

/**
 * The repository files a sitemap address is made from, as git pathspecs
 * relative to the repository's root, for its lastmod:
 *
 * - the landing page "": web/landing/;
 * - the app "app/": web/ but the docs, the landing page and the tests
 *   (what web/site.sh puts under app/, with web/pwa/ and the .htaccess,
 *   plus a few READMEs and build scripts, which change rarely), and the
 *   crates the wasm package is built from (crates/saturnus-web and its
 *   dependencies in the workspace): an emulator change is a change of
 *   the app;
 * - a docs page: its own web/docs/pages/<rel>.md. Not the shared docs
 *   code (build.mjs, markdown.mjs, docs.css): a change there touches
 *   every page's markup, not what a page says, and would mark all of
 *   them changed at once, which is what a crawler should not be told;
 * - the command reference page also web/commands.json (its data) and
 *   web/docs/commands/ (its script).
 */
export const APP_CRATES = ["saturnus-web", "saturnus-core", "saturnus-host", "saturnus-objects", "saturnus-tx", "satx-core", "kermit-proto", "xmodem-proto"];
export function sitemapSources(path) {
  if (path === "") return ["web/landing"];
  if (path === "app/") return ["web", ":(exclude)web/docs", ":(exclude)web/landing", ":(exclude)web/test", ...APP_CRATES.map((c) => `crates/${c}`)];
  const rel = path === "docs/" ? "index" : path.replace(/^docs\//, "").replace(/\/$/, "");
  const page = [`web/docs/pages/${rel}.md`];
  return rel === "commands" ? [...page, "web/commands.json", "web/docs/commands"] : page;
}

let shallow;
/**
 * The date (YYYY-MM-DD) of the last commit touching `paths` (git
 * pathspecs relative to the repository's root), or null: no git, no
 * repository, no commit touches them, or a shallow clone (its one
 * commit's date would stand for every file; pages.yml checks out the
 * whole history).
 */
export function gitLastmod(paths) {
  const git = (...args) => execFileSync("git", ["-C", join(WEB, ".."), ...args], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  try {
    shallow ??= git("rev-parse", "--is-shallow-repository") === "true";
    if (shallow) return null;
    const date = git("log", "-1", "--first-parent", "--format=%cs", "--", ...paths);
    return /^\d{4}-\d{2}-\d{2}$/.test(date) ? date : null;
  } catch {
    return null;
  }
}

/**
 * /saturnus/sitemap.xml (https://www.sitemaps.org): the landing page, the
 * app and every written docs page (no placeholders), by their HTML
 * addresses, each with its lastmod: `lastmod(sitemapSources(path))`, by
 * default the date of the last commit touching them (gitLastmod); a
 * null leaves the address without one. ractive.ch's robots.txt names it
 * beside the site's own (ractive/ractive.ch#17); after a deploy,
 * scripts/indexnow.mjs tells IndexNow the addresses that are new or got
 * a new lastmod.
 */
export function sitemapXml(pages, base = BASE, lastmod = gitLastmod) {
  const written = pages.filter((p) => p.meta.status !== "placeholder");
  const docs = [...written.filter((p) => p.rel === "index"), ...written.filter((p) => p.rel !== "index")];
  const paths = ["", "app/", ...docs.map((p) => pagePaths(p.rel).html)];
  const urls = paths.map((p) => {
    const date = lastmod(sitemapSources(p));
    return `  <url>\n    <loc>${escapeHtml(`${base}${p}`)}</loc>\n${date ? `    <lastmod>${date}</lastmod>\n` : ""}  </url>`;
  });
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.join("\n")}\n</urlset>\n`;
}

/**
 * /saturnus/llms.txt (https://llmstxt.org): the title, a summary, then a
 * list of links per section to the Markdown files: the written pages
 * (no placeholders) and one command file per model.
 */
export function llmsTxt(pages, data, base = BASE) {
  const models = referenceModels(data);
  const titles = MODEL_ORDER.map((m) => MODEL_TITLES[m].replace(/^HP /, ""));
  const list = `${titles.slice(0, -1).join(", ")} and ${titles.at(-1)}`;
  const url = (p) => `${base}${p}`;
  const out = [
    "# saturnus",
    "",
    `> saturnus runs classic HP calculators in the browser: the HP ${list}. These docs cover the models, their commands, how the Saturn processor works and how to move files to a real calculator.`,
    "",
    "Every docs page is also Markdown: its address with `.md` (`/saturnus/docs/48gx/` is `/saturnus/docs/48gx.md`). The command reference has one Markdown file per model with every command, its stack effect, a description and examples with their results from the emulated calculator.",
    "",
  ];
  const section = (heading, rows) => {
    if (rows.length) out.push(`## ${heading}`, "", ...rows, "");
  };
  const links = (id) => sectionPages(pages, id).map((p) => `- [${p.meta.title}](${url(pagePaths(p.rel).md)}): ${p.meta.description}`);
  section("Docs", links("index"));
  section("Models", links("models"));
  section("Command reference", [
    ...links("commands"),
    ...models.map((m) => {
      const index = buildIndex(data, m);
      return `- [${MODEL_TITLES[m]} commands](${url(`docs/commands/${m}.md`)}): ${index.commands.length} ${index.keyed ? "functions with their keys" : "commands with their stack effects"}, descriptions and examples`;
    }),
  ]);
  section("How the Saturn works", links("saturn"));
  section("Moving files to a real calculator", links("transfer"));
  section("Optional", [`- [The app](${url("app/")}): the calculators in the browser, with the same reference in Search`]);
  return `${out.join("\n").trimEnd()}\n`;
}

/** The index page's model links with each model's logo in its colours. */
function modelLogos(html) {
  return html.replace(/<li><a href="([a-z0-9]+)\/">/g, (all, m) => (MODEL_ORDER.includes(m) ? `<li class="docs-model-link"><a href="${m}/">${logoSvg(20, m)}` : all));
}

/** Every relative href and src of `html` (in the site folder `dir`) that names no file; `exists(path)` checks a site path. */
export function brokenLinks(html, dir, exists) {
  const out = [];
  for (const [, ref] of html.matchAll(/(?:href|src)="([^"]*)"/g)) {
    if (/^[a-z][a-z0-9+.-]*:/i.test(ref) || ref.startsWith("/") || ref.startsWith("#")) continue;
    const path = ref.replace(/[?#].*$/, "");
    let target = posix.normalize(posix.join(dir, path));
    if (path.endsWith("/") || path === "" || path === "." || path === "./") target = `${target.replace(/\/$/, "")}/index.html`.replace(/^\.\//, "");
    if (target.startsWith("..")) { out.push(ref); continue; }
    if (!exists(target)) out.push(ref);
  }
  return out;
}

/** Build the docs into the site folder `out`. */
export function build(out, { csp = null, base = BASE, pagesDir = PAGES, commandsJson = join(WEB, "commands.json") } = {}) {
  const data = JSON.parse(readFileSync(commandsJson, "utf8"));
  const pages = loadPages(pagesDir);
  const pageRels = new Set(pages.map((p) => p.rel));
  for (const s of SECTIONS) {
    const target = s.href.replace(/#.*$/, "").replace(/\.md$/, "");
    if (!pageRels.has(target)) throw new Error(`docs: no page for the section ${s.title} (${s.href})`);
  }
  const written = [];
  const write = (path, text) => {
    const file = join(out, path);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, text);
    written.push(path);
  };
  const html = [];
  for (const page of pages) {
    const paths = pagePaths(page.rel);
    let source = page.source;
    let body = page.body;
    let after = "";
    // One link budget per page (markdown.mjs): its body, then the notes and sources after it.
    const budget = new LinkBudget();
    const render = (md) => sizeImages(renderMarkdown(md, { link: (h) => htmlLink(page.rel, h, pageRels), headingId, appIcon, budget }));
    let notes = "";
    let sources = "";
    if (page.rel === "commands") {
      // The lead above the reference; its sections (the notes) and the
      // sources (every manual it links to) after it, each in a column
      // of its own (docs.css).
      const at = body.search(/^## /m);
      const rest = at < 0 ? "" : body.slice(at);
      if (at >= 0) body = body.slice(0, at);
      notes = rest.trimEnd();
      sources = `## Sources\n\n${manualSources(data, referenceModels(data)).join("\n")}\n`;
      source = `${source.trimEnd()}\n\n${sources}`;
    }
    let bodyHtml = render(body);
    if (page.rel === "commands") after = `<div class="docs-after-notes">\n${render(notes)}\n</div>\n<div class="docs-after-sources">\n${render(sources)}\n</div>`;
    if (page.rel === "index") bodyHtml = modelLogos(bodyHtml);
    const extra = page.rel === "commands" ? commandsExtra() : "";
    const doc = pageHtml(page, bodyHtml, { csp, extra, after });
    write(`${paths.html}index.html`, doc);
    write(paths.md, source);
    html.push({ dir: paths.html, doc });
  }
  for (const m of referenceModels(data)) write(`docs/commands/${m}.md`, commandsMarkdown(data, m));
  write("llms.txt", llmsTxt(pages, data, base));
  write("sitemap.xml", sitemapXml(pages, base));
  cpSync(join(HERE, "docs.css"), join(out, "docs", "docs.css"));
  for (const f of ["page.js", "slug.js"]) cpSync(join(HERE, "commands", f), join(out, "docs", "commands", f));
  written.push("docs/docs.css", "docs/commands/page.js", "docs/commands/slug.js");

  // A site path exists in the site, or, when the app and the landing
  // page are not built into `out` (the tests), in web/: the app's files,
  // the landing page, and its images (img/, from web/landing/img/), which a
  // model page may show (`![HP 48GX](../img/48gx-640.webp)`).
  const isFile = (f) => existsSync(f) && statSync(f).isFile();
  const exists = (p) => {
    if (isFile(join(out, p))) return true;
    if (p.startsWith("app/")) return isFile(join(WEB, p.slice(4)));
    if (p.startsWith("img/")) return isFile(join(WEB, "landing", p));
    if (p === "index.html") return isFile(join(WEB, "landing", "index.html"));
    return p === "favicon.svg" && existsSync(join(WEB, "favicon.svg"));
  };
  const problems = [];
  for (const { dir, doc } of html) {
    for (const ref of brokenLinks(doc, dir, exists)) problems.push(`${dir}index.html refers to ${ref}, which is not in the site`);
  }
  for (const path of written) {
    const text = readFileSync(join(out, path), "utf8");
    if (!text.length) problems.push(`${path} is empty`);
    if (WIKI.test(text)) problems.push(`${path} links into the wiki`);
  }
  if (problems.length) throw new Error(`docs:\n${problems.join("\n")}`);
  return { pages, written: written.sort() };
}

// Run as a script (the path compared resolved: /var is /private/var on macOS).
if (process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url))) {
  const args = process.argv.slice(2);
  const opt = (name) => {
    const i = args.indexOf(name);
    if (i < 0) return null;
    const v = args[i + 1];
    args.splice(i, 2);
    return v;
  };
  const csp = opt("--csp");
  const base = opt("--base") ?? BASE;
  const out = args[0];
  if (!out) {
    console.error("usage: node web/docs/build.mjs OUT [--csp POLICY] [--base URL]");
    process.exit(2);
  }
  try {
    const { pages, written } = build(out, { csp, base });
    console.log(`web/docs/build.mjs: docs/ ${pages.length} pages, ${written.length} files`);
  } catch (err) {
    console.error(err.message);
    process.exit(1);
  }
}
