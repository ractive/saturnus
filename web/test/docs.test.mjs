// The docs build (web/docs/build.mjs, iteration 57): the Markdown
// renderer and frontmatter (web/docs/markdown.mjs), the links of a
// page's HTML, the command reference's addresses (web/docs/commands/
// slug.js: unique per model, each leading back to its command), the
// built files (each page's HTML with its Markdown beside it as written,
// one command file per model with every command, the CSP, the model
// colours), llms.txt in the llmstxt.org format, and no link into the
// wiki. The page in Chrome: web/test/docs-page.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync as writeFile } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { escapeHtml, parseFrontmatter, renderInline, renderMarkdown } from "../docs/markdown.mjs";
import { commandOfSlug, hashEncode, parseRefHash, refHash, slugBase, slugMap } from "../docs/commands/slug.js";
import { brokenLinks, build, commandsMarkdown, htmlLink, llmsTxt, loadPages, pagePaths, referenceModels, relative } from "../docs/build.mjs";
import { buildIndex } from "../reference.js";
import { MODEL_ORDER } from "../norom.js";

const data = JSON.parse(readFileSync(new URL("../commands.json", import.meta.url), "utf8"));

test("the renderer covers the pages' Markdown and escapes HTML", () => {
  assert.equal(renderMarkdown("# Title\n\nOne *two* **three** `x<y`."), "<h1>Title</h1>\n<p>One <em>two</em> <strong>three</strong> <code>x&lt;y</code>.</p>");
  assert.equal(renderMarkdown("a\nb\n\nc"), "<p>a b</p>\n<p>c</p>");
  // A link into the app alone in its paragraph is the app's button; elsewhere a link.
  assert.equal(renderMarkdown("[Open it](../app/#/calculator/48gx)", { appIcon: (h) => `<i>${h.split("/").at(-1)}</i>` }), '<p class="docs-action"><a class="button primary" href="../app/#/calculator/48gx"><i>48gx</i><span>Open it</span></a></p>');
  assert.equal(renderMarkdown("See [it](../app/) here."), '<p>See <a href="../app/">it</a> here.</p>');
  assert.equal(renderMarkdown("[Other](commands.md)"), '<p><a href="commands.md">Other</a></p>');
  assert.equal(renderMarkdown("- one\n- two\n  more\n\n1. first\n2. second"), "<ul>\n<li>one</li>\n<li>two more</li>\n</ul>\n<ol>\n<li>first</li>\n<li>second</li>\n</ol>");
  assert.equal(renderMarkdown("```text\n<b>\n  kept\n```"), '<pre><code class="language-text">&lt;b&gt;\n  kept</code></pre>');
  assert.equal(renderMarkdown("> quoted\n> on"), "<blockquote>\n<p>quoted on</p>\n</blockquote>");
  assert.equal(renderMarkdown("| A | B |\n|---|--:|\n| 1 | 2 |"), '<div class="table-wrap"><table>\n<thead><tr><th>A</th><th style="text-align: right">B</th></tr></thead>\n<tbody>\n<tr><td>1</td><td style="text-align: right">2</td></tr>\n</tbody>\n</table></div>');
  assert.equal(renderMarkdown("---"), "<hr>");
  // A model page's forms: an image starting a paragraph is a figure, the other lines its caption; a header of empty cells is no header.
  assert.equal(renderMarkdown("![HP 48GX](a.webp)\nThe *48GX*\nin saturnus.\n\nText"), '<figure><img src="a.webp" alt="HP 48GX"><figcaption>The <em>48GX</em> in saturnus.</figcaption></figure>\n<p>Text</p>');
  assert.equal(renderMarkdown("![p](a.png)"), '<figure><img src="a.png" alt="p"></figure>');
  assert.equal(renderMarkdown("Text ![p](a.png) inline"), '<p>Text <img src="a.png" alt="p"> inline</p>', "an image within text stays inline");
  assert.equal(renderMarkdown("| | |\n|---|---|\n| RAM | 128 KB |"), '<div class="table-wrap"><table>\n<tbody>\n<tr><td>RAM</td><td>128 KB</td></tr>\n</tbody>\n</table></div>');
  assert.equal(renderMarkdown("<script>alert(1)</script>"), "<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>");
  assert.equal(renderInline("[a *b*](x.md#y) <https://e.org/> ![p](i.png)"), '<a href="x.md#y">a <em>b</em></a> <a href="https://e.org/">https://e.org/</a> <img src="i.png" alt="p">');
  assert.equal(renderInline("snake_case_name and 2*3*4"), "snake_case_name and 2<em>3</em>4");
  assert.equal(renderInline("\\*not\\* [x](a\"b)"), '*not* <a href="a&quot;b">x</a>');
  // Review of PR 206: only relative, fragment, http(s) and mailto targets; a closing # needs a space.
  for (const bad of ["[x](javascript:alert(1))", "[x]( JavaScript:alert(1))", "![p](data:image/png;base64,AA)", "[x](vbscript:x)"]) {
    assert.throws(() => renderInline(bad), /link to a /, bad);
  }
  assert.equal(renderInline("[m](mailto:a@b.c) [h](#top)"), '<a href="mailto:a@b.c">m</a> <a href="#top">h</a>');
  assert.match(renderMarkdown("## C#"), />C#<\/h2>/);
  assert.match(renderMarkdown("## Title ##"), />Title<\/h2>/);
  assert.equal(renderInline("[x](u)", (h) => `../${h}`), '<a href="../u">x</a>');
  assert.equal(renderMarkdown("## Two words", { headingId: (t) => t.toLowerCase().replace(/ /g, "-") }), '<h2 id="two-words">Two words</h2>');
  assert.throws(() => renderMarkdown("```\nopen"), /not closed/);
  assert.equal(escapeHtml(`<a href="x">&</a>`), "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;");
});

test("frontmatter: key: value lines, quotes taken off; none is an empty one", () => {
  assert.deepEqual(parseFrontmatter("---\ntitle: HP 48GX\ndescription: \"A: b\"\n---\nBody"), { meta: { title: "HP 48GX", description: "A: b" }, body: "Body" });
  assert.deepEqual(parseFrontmatter("No frontmatter"), { meta: {}, body: "No frontmatter" });
  assert.throws(() => parseFrontmatter("---\nnot a pair\n---\n"), /cannot read/);
});

test("a page's .md links become its HTML's addresses, relative to where the HTML is", () => {
  const rels = new Set(["index", "48gx", "commands", "saturn", "saturn/cpu"]);
  assert.deepEqual(pagePaths("index"), { html: "docs/", md: "docs/index.md", mdDir: "docs/" });
  assert.deepEqual(pagePaths("saturn/cpu"), { html: "docs/saturn/cpu/", md: "docs/saturn/cpu.md", mdDir: "docs/saturn/" });
  assert.equal(htmlLink("index", "48gx.md", rels), "48gx/");
  assert.equal(htmlLink("48gx", "commands.md#48gx", rels), "../commands/#48gx");
  assert.equal(htmlLink("48gx", "index.md#models", rels), "../#models");
  assert.equal(htmlLink("commands", "commands/48gx.md", rels), "48gx.md", "a file that is no page stays a file");
  assert.equal(htmlLink("saturn/cpu", "../48gx.md", rels), "../../48gx/");
  assert.equal(htmlLink("saturn/cpu", "cpu.md", rels), "./");
  assert.equal(htmlLink("index", "../llms.txt", rels), "../llms.txt");
  for (const href of ["https://example.org/x.md", "#top", "/saturnus/"]) assert.equal(htmlLink("48gx", href, rels), href);
  assert.equal(relative("docs/48gx/", ""), "../../");
  assert.equal(relative("docs/", "docs/"), "./");
  assert.deepEqual(brokenLinks('<a href="../x/">a</a><img src="y.png"><a href="https://e.org">e</a>', "docs/48gx/", (p) => p === "docs/x/index.html"), ["y.png"]);
});

test("every command of every model has a readable address of its own that leads back to it", () => {
  assert.equal(slugBase("HMS-"), "hms-");
  assert.equal(slugBase("→LIST"), "to-list");
  assert.equal(slugBase("%CH"), "pct-ch");
  assert.equal(slugBase("∫"), "integral");
  assert.equal(slugBase("Σ+"), "sigma+");
  assert.equal(slugBase("R→B"), "r-to-b");
  assert.equal(slugBase("INSRT (CFLO)"), "insrt-cflo");
  assert.equal(hashEncode("a b"), "a%20b");
  assert.equal(hashEncode("("), "%28", "no parenthesis to end a Markdown link");
  const g49 = slugMap(buildIndex(data, "49g").commands.map((c) => c.name));
  assert.equal(g49.get("QR"), "QR", "two names, one slug: each keeps its name");
  assert.equal(g49.get("qr"), "qr");
  assert.equal(refHash("48gx", "hms-"), "#48gx/hms-");
  assert.deepEqual(parseRefHash("#48GX/hms-"), { model: "48gx", slug: "hms-" });
  assert.deepEqual(parseRefHash("#42s//"), { model: "42s", slug: "/" });
  assert.deepEqual(parseRefHash("#28s"), { model: "28s", slug: null });
  assert.deepEqual(parseRefHash(""), { model: null, slug: null });
  const models = referenceModels(data);
  assert.deepEqual(models, MODEL_ORDER, "all 12 models, in the app's order");
  for (const m of models) {
    const names = buildIndex(data, m).commands.map((c) => c.name);
    const slugs = slugMap(names);
    assert.equal(new Set(slugs.values()).size, names.length, `${m}: one slug per command`);
    for (const [n, s] of slugs) {
      assert.equal(commandOfSlug(slugs, s), n, `${m}: ${s} is ${n}`);
      assert.match(s, /^[A-Za-z0-9!$&'*+,;=:@/?._~%-]+$/, `${m}: ${n}'s slug ${s} needs no encoding`);
    }
  }
  // An address with the name itself, in any case, still finds it.
  const sx = slugMap(buildIndex(data, "48sx").commands.map((c) => c.name));
  assert.equal(commandOfSlug(sx, encodeURIComponent("→LIST")), "→LIST");
  assert.equal(commandOfSlug(sx, "dup"), "DUP");
  assert.equal(commandOfSlug(sx, "no-such-command"), null);
});

test("one command file per model: every command, its stack or keys, examples, manuals and address", () => {
  const gx = commandsMarkdown(data, "48gx");
  const index = buildIndex(data, "48gx");
  assert.match(gx, /^# HP 48GX commands\n/);
  for (const c of index.commands) assert.ok(gx.includes(`\n## ${c.name}${c.longName ? ` · ${c.longName}` : ""}\n`), `48GX: ${c.name}`);
  assert.match(gx, /## HMS- · [^\n]+\n\nStack:\n\n```\nhms1 hms2 → hms\n```/);
  assert.match(gx, /Before:\n2: 4\.15\n1: 1\.3\nRun: {5}HMS-\nAfter:\n1: 2\.45/);
  assert.match(gx, /On the page: \[HMS-\]\(\.\/#48gx\/hms-\)/);
  assert.match(gx, /\n## Sources\n\n- \[[^\]]+\]\(https:\/\/literature\.hpcalc\.org\/[^)]+\)/);
  const s42 = commandsMarkdown(data, "42s");
  assert.match(s42, /^# HP 42S commands\n/);
  assert.match(s42, /\n## % · Percent\n\nKeys: ⇧ %\n/);
  assert.match(s42, /Keys: {4}⇧ %\nDisplay:\ny:200\.0000\nx:30\.0000/);
});

test("llms.txt: the title, a summary, link lists per section to the Markdown files; no placeholders", () => {
  const pages = loadPages();
  const txt = llmsTxt(pages, data);
  const lines = txt.split("\n");
  assert.equal(lines[0], "# saturnus", "an H1 first");
  assert.match(lines[2], /^> saturnus runs classic HP calculators in the browser/, "a summary as a blockquote");
  assert.ok(!/^#{3,} /m.test(txt), "sections are H2");
  const sections = lines.filter((l) => l.startsWith("## "));
  assert.deepEqual(sections, ["## Docs", "## Models", "## Command reference", "## Optional"]);
  // Every list item is a link with a description.
  for (const l of lines.filter((x) => x.startsWith("- "))) assert.match(l, /^- \[[^\]]+\]\(https:\/\/ractive\.ch\/saturnus\/[^)]+\)(: .+)?$/, l);
  for (const m of MODEL_ORDER) assert.ok(txt.includes(`(https://ractive.ch/saturnus/docs/commands/${m}.md)`), `${m}'s command file`);
  assert.ok(txt.includes("- [HP 48GX](https://ractive.ch/saturnus/docs/48gx.md): "), "a written model page is listed");
  assert.ok(!txt.includes("(https://ractive.ch/saturnus/docs/48sx.md)"), "a placeholder is left out");
  assert.ok(txt.includes("(https://ractive.ch/saturnus/docs/index.md)"));
  assert.ok(txt.endsWith("\n") && !txt.endsWith("\n\n"));
});

test("the build: each page's HTML and its Markdown as written, the command files, llms.txt, the CSP and the colours", () => {
  const out = mkdtempSync(join(tmpdir(), "saturnus-docs-"));
  try {
    const { pages, written } = build(out, { csp: "default-src 'self'" });
    const rels = pages.map((p) => p.rel).sort();
    assert.deepEqual(rels, [...MODEL_ORDER, "commands", "index", "saturn", "transfer"].sort());
    for (const p of pages) {
      const { html, md } = pagePaths(p.rel);
      assert.ok(written.includes(`${html}index.html`), `${p.rel}'s HTML`);
      assert.equal(readFileSync(join(out, md), "utf8").startsWith(p.source.trimEnd()), true, `${p.rel}'s Markdown as written`);
      const doc = readFileSync(join(out, html, "index.html"), "utf8");
      assert.match(doc, /^<!doctype html>\n<html lang="en"/);
      assert.ok(doc.includes(`<meta http-equiv="Content-Security-Policy" content="default-src 'self'">`), `${p.rel}: the CSP`);
      assert.match(doc, new RegExp(`<link rel="alternate" type="text/markdown" href="[./]*${md.replace(/^docs\//, "").replace(/.*\//, "")}">`));
      assert.match(doc, /app\/shared\/tokens\.css"/);
      assert.match(doc, /app\/shared\/theme-boot\.js"><\/script>/);
      assert.ok(!/<script>/.test(doc), "no inline script (the CSP)");
      // A model page takes its colours; the section pages keep the plain logo's.
      if (p.meta.model) assert.match(doc, new RegExp(`<html lang="en" data-logo="${p.meta.model}">`));
      else assert.match(doc, /<html lang="en">/);
    }
    for (const m of MODEL_ORDER) assert.ok(existsSync(join(out, "docs", "commands", `${m}.md`)), `${m}.md`);
    assert.ok(existsSync(join(out, "llms.txt")));
    const ref = readFileSync(join(out, "docs", "commands", "index.html"), "utf8");
    assert.match(ref, /<script type="module" src="page\.js"><\/script>/);
    assert.match(ref, /<input id="ref-q" type="search" role="combobox"/);
    // The bar: the field with the model in it (a pill with the page's chevron), the count line under it.
    assert.match(ref, /<span class="ref-model"><select id="ref-model" aria-label="Model"><\/select><svg class="ic"[^>]*aria-hidden="true">/);
    assert.match(ref, /<div id="ref-bar" class="ref-bar">\s*<div class="ref-bar-row">[\s\S]*<\/div>\s*<p id="ref-count" class="ref-count muted" role="status">[^<]*<\/p>\s*<\/div>\s*<div class="ref-panes">/, "the count line in the bar");
    // The two panes: the navigation (a skeleton until the data is there) and the entry; the sheet for a phone.
    assert.match(ref, /<nav id="ref-nav" class="ref-nav" aria-label="Commands by menu"><div class="ref-skeleton" aria-hidden="true">(<span><\/span>){6}<\/div><\/nav>/);
    assert.match(ref, /<aside id="ref-side"[^>]*>[\s\S]*<\/aside>\s*<div id="ref-entry" class="ref-entry docs-ref"><\/div>/);
    assert.match(ref, /<button id="ref-browse" type="button" class="ref-browse"[^>]* aria-haspopup="dialog" aria-controls="ref-sheet">/);
    assert.match(ref, /<dialog id="ref-sheet" class="ref-sheet" aria-labelledby="ref-sheet-title">/);
    // No list of the models' Markdown files on the page (llms.txt has them): the noscript line points there.
    assert.doesNotMatch(ref, /each-model-as-markdown|<a href="48gx\.md">/);
    assert.match(ref, /<noscript><p>[^<]*<a href="\.\.\/\.\.\/llms\.txt">llms\.txt<\/a>[^<]*<\/p><\/noscript>/);
    // The lead before the tool, the page's sections (the notes) and the sources after it, each in a column.
    assert.ok(ref.indexOf("<p>Every command of the 12 calculators") < ref.indexOf('<section class="docs-ref-app"'), "the lead first");
    assert.ok(ref.indexOf('<h2 id="reading-an-example">') > ref.indexOf('<script type="module" src="page.js">'), "the sections after the tool");
    assert.match(ref, /<div class="docs-page docs-after">\n<div class="docs-after-notes">\n<h2 id="reading-an-example">[\s\S]*<\/div>\n<div class="docs-after-sources">\n<h2 id="sources">Sources<\/h2>\n<ul>\n<li><a href="https:/, "the notes and the sources in columns of their own");
    assert.match(readFileSync(join(out, "docs", "commands.md"), "utf8"), /\n## Sources\n\n- \[/, "the reference lists its manuals");
    // The header: "saturnus" leads to the landing page, "docs" to the docs' index, the app's link beside them.
    assert.match(ref, /<a class="brand" href="\.\.\/\.\.\/" title="saturnus"><svg class="logo"[^>]*>[\s\S]*?<\/svg><span>saturnus<\/span><\/a>\s*<a class="docs-brand-sub" href="\.\.\/">docs<\/a>/);
    assert.match(ref, /<a class="docs-app" href="\.\.\/\.\.\/app\/" title="Open the app">/);
    const home = readFileSync(join(out, "docs", "index.html"), "utf8");
    assert.match(home, /<a class="brand" href="\.\.\/" title="saturnus">/);
    assert.match(home, /<a class="docs-brand-sub" href="\.\/">docs<\/a>/);
    assert.match(home, /<h2 id="models">Models<\/h2>/);
    assert.match(home, /<li class="docs-model-link"><a href="48gx\/"><svg class="logo"[^>]* data-logo="48gx">/);
    for (const s of ["commands/", "saturn/", "transfer/"]) assert.ok(home.includes(`href="${s}"`), s);
    // The 48GX page: its picture from the landing page's images (saturnus's drawing) with a caption, its facts table, its links into the app and the reference, its sources last.
    const gx = readFileSync(join(out, "docs", "48gx", "index.html"), "utf8");
    assert.match(gx, /<figure><img src="\.\.\/\.\.\/img\/48gx-640\.webp" alt="HP 48GX"><figcaption>[^<]+<\/figcaption><\/figure>/);
    assert.match(gx, /<h2 id="at-a-glance">At a glance<\/h2>\n<div class="table-wrap"><table>\n<tbody>/, "the facts without a header row");
    assert.ok(gx.includes('href="../../app/#/calculator/48gx"') && gx.includes('href="../commands/#48gx"'));
    assert.match(gx, /<h2 id="sources">Sources<\/h2>\n<ul>\n(<li><a href="https:\/\/[^"]+">[\s\S]*?<\/li>\n)+<\/ul>\n<\/article>/, "the sources last, each linked");
    assert.doesNotMatch(readFileSync(join(out, "docs", "48gx.md"), "utf8"), /status: placeholder|coming soon/);
    // The page's own modules import the app's from app/ beside docs/.
    const page = readFileSync(join(out, "docs", "commands", "page.js"), "utf8");
    for (const m of ["palette.js", "reference.js", "components/entry-view.js"]) assert.ok(page.includes(`"../../app/${m}"`), m);
    for (const f of written) {
      const text = readFileSync(join(out, f), "utf8");
      assert.ok(text.length > 0, `${f} is not empty`);
      assert.doesNotMatch(text, /calculator-knowledgebase/i, `${f} has no link into the wiki`);
    }
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
});

test("a broken link or a link into the wiki fails the build", () => {
  const pages = mkdtempSync(join(tmpdir(), "saturnus-pages-"));
  const out = mkdtempSync(join(tmpdir(), "saturnus-docs-"));
  try {
    const page = (name, meta, body) => {
      const lines = Object.entries(meta).map(([k, v]) => `${k}: ${v}`).join("\n");
      return [name, `---\n${lines}\n---\n\n${body}\n`];
    };
    const base = [
      page("index.md", { title: "Docs", description: "d", section: "index" }, "[Gone](gone.md)"),
      page("commands.md", { title: "C", description: "d", section: "commands" }, "See [the wiki](https://github.com/ractive/calculator-knowledgebase)."),
      page("saturn.md", { title: "S", description: "d", section: "saturn" }, "x"),
      page("transfer.md", { title: "T", description: "d", section: "transfer" }, "x"),
    ];
    for (const [n, text] of base) writeFile(join(pages, n), text);
    assert.throws(() => build(out, { pagesDir: pages }), (err) => /docs\/index\.html refers to gone\.md/.test(err.message) && /commands\.md links into the wiki/.test(err.message));
  } finally {
    rmSync(pages, { recursive: true, force: true });
    rmSync(out, { recursive: true, force: true });
  }
});
