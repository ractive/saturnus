// What search engines and link previews read (iteration 76b), on the
// landing page (web/landing/index.html), the docs pages (web/docs/
// build.mjs) and the app's page (web/index.html, its head only): one
// <h1>, a title, a description and a canonical address per page, Open
// Graph tags, headings in order, images with an alt text, and JSON-LD
// that parses and says what the page shows: a SoftwareApplication on
// the landing page, a BreadcrumbList on every docs page, a FAQPage
// where the page has its questions (the twelve model pages and the
// landing page), word for word. No ROM, no browser.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { BASE, FAQ_HEADING, breadcrumbs, build, documentTitle, faqOf, jsonLd, llmsTxt, loadPages, pagePaths, plainText, sitemapXml } from "../docs/build.mjs";
import { MODEL_ORDER } from "../norom.js";

const WEB = new URL("..", import.meta.url);
const landing = readFileSync(new URL("landing/index.html", WEB), "utf8");
const appIndex = readFileSync(new URL("index.html", WEB), "utf8");
const commands = JSON.parse(readFileSync(new URL("commands.json", WEB), "utf8"));

/** What an external link has after its text (web/shared/ext-links.js). */
const MARKS = '<span class="ext" aria-hidden="true">↗</span><span class="visually-hidden"> (opens in a new tab)</span>';

/** The JSON-LD blocks of `html`, parsed (a block that does not parse fails the test). */
function ldBlocks(html) {
  return [...html.matchAll(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g)].map(([, json]) => JSON.parse(json));
}

/** HTML as a reader sees it: no tags, no marks of links to other sites, entities decoded, spaces collapsed. */
function shown(html) {
  return html
    .replaceAll(MARKS, "")
    .replace(/<\/?(?:p|li|ul|ol|pre|div|table|tr|td|th|h[1-6])\b[^>]*>/g, " ")
    .replace(/<[^>]+>/g, "")
    .replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&amp;/g, "&")
    .replace(/\s+/g, " ")
    .trim();
}

/** The content of a `<meta name|property="key">`, the `<title>` and the canonical address of `html`. */
function head(html) {
  const meta = (key) => html.match(new RegExp(`<meta (?:name|property)="${key}" content="([^"]*)">`))?.[1]?.replace(/&amp;/g, "&").replace(/&quot;/g, '"') ?? null;
  return {
    title: html.match(/<title>([^<]*)<\/title>/)?.[1]?.replace(/&amp;/g, "&") ?? null,
    titles: (html.match(/<title>/g) ?? []).length,
    description: meta("description"),
    canonical: html.match(/<link rel="canonical" href="([^"]*)">/)?.[1] ?? null,
    canonicals: (html.match(/<link rel="canonical"/g) ?? []).length,
    meta,
  };
}

/** The heading levels of `html` in order, the hidden parts (`<dialog>`, `<noscript>`) left out. */
function headingLevels(html) {
  const body = html.replace(/<dialog[\s\S]*?<\/dialog>/g, "").replace(/<noscript>[\s\S]*?<\/noscript>/g, "");
  return [...body.matchAll(/<h([1-6])[\s>]/g)].map((m) => Number(m[1]));
}

/** No heading more than one level below the one before it. */
function assertHeadingsInOrder(levels, what) {
  assert.equal(levels[0], 1, `${what}: the first heading is the h1`);
  assert.equal(levels.filter((l) => l === 1).length, 1, `${what}: one h1`);
  for (let i = 1; i < levels.length; i++) assert.ok(levels[i] <= levels[i - 1] + 1, `${what}: an h${levels[i]} follows an h${levels[i - 1]}`);
}

/** The docs built once into a temporary folder: `{out, pages, html(rel)}`. */
function builtDocs(t) {
  const out = mkdtempSync(join(tmpdir(), "saturnus-seo-"));
  t.after(() => rmSync(out, { recursive: true, force: true }));
  const { pages } = build(out);
  return { out, pages, html: (rel) => readFileSync(join(out, pagePaths(rel).html, "index.html"), "utf8") };
}

test("a page's questions: each ### of \"Questions and answers\" with the text up to the next one", () => {
  const body = `Lead.\n\n## In Saturnus\n\n### Not a question\n\nText.\n\n## ${FAQ_HEADING}\n\n### Where is the ROM?\n\nAt [hpcalc.org](https://www.hpcalc.org/details/4368), with \`rom fetch\`.\n\nA second paragraph.\n\n### Does it run?\n\nYes.\n\n## Sources\n\n### Nor this\n\n- x\n`;
  assert.deepEqual(faqOf(body), [
    { question: "Where is the ROM?", markdown: "At [hpcalc.org](https://www.hpcalc.org/details/4368), with `rom fetch`.\n\nA second paragraph." },
    { question: "Does it run?", markdown: "Yes." },
  ]);
  assert.deepEqual(faqOf("Lead.\n\n## Sources\n\n- x\n"), [], "no such section: no questions");
  // A heading inside a code block is text.
  assert.deepEqual(faqOf(`## ${FAQ_HEADING}\n\n### Q?\n\n\`\`\`\n## not a heading\n\`\`\`\n`), [{ question: "Q?", markdown: "```\n## not a heading\n```" }]);
  assert.throws(() => faqOf(`## ${FAQ_HEADING}\n\nText only.\n`, "x.md"), /x\.md: text before the first question/);
  assert.throws(() => faqOf(`## ${FAQ_HEADING}\n\n## Sources\n`, "x.md"), /x\.md: "Questions and answers" has no question/);
  assert.throws(() => faqOf(`## ${FAQ_HEADING}\n\n### Q?\n\n### R?\n\nA.\n`, "x.md"), /x\.md: the question "Q\?" has no answer/);
});

test("an answer as plain text; JSON-LD as a data block that no text can end", () => {
  assert.equal(plainText("Yes. See [the page](https://www.hpcalc.org/details/1) and `saturnus rom fetch` (**once**).\n\n- one\n- two"), "Yes. See the page and saturnus rom fetch (once). one two");
  assert.equal(plainText("2 < 3 & \"x\""), '2 < 3 & "x"');
  const block = jsonLd({ a: "</script><b>" });
  assert.equal(block, '<script type="application/ld+json">{"a":"\\u003c/script>\\u003cb>"}</script>');
  assert.deepEqual(ldBlocks(block), [{ a: "</script><b>" }]);
});

test("a page's title names the site once; its trail leads from the landing page", () => {
  const page = (rel, meta) => ({ rel, meta: { title: rel, description: "d", ...meta } });
  assert.equal(documentTitle(page("48gx", { title: "HP 48GX", section: "models" })), "HP 48GX · Saturnus");
  assert.equal(documentTitle(page("48gx", { title: "HP 48GX", page_title: "HP 48GX emulator for the browser", section: "models" })), "HP 48GX emulator for the browser · Saturnus");
  assert.equal(documentTitle(page("index", { title: "Saturnus docs", section: "index" })), "Saturnus docs");
  const names = (p) => breadcrumbs(p).map((c) => `${c.name}=${c.path}`);
  assert.deepEqual(names(page("index", { title: "Saturnus docs", section: "index" })), ["Saturnus=", "Docs=docs/"]);
  assert.deepEqual(names(page("48gx", { title: "HP 48GX", section: "models" })), ["Saturnus=", "Docs=docs/", "HP 48GX=docs/48gx/"]);
  assert.deepEqual(names(page("saturn", { title: "How the Saturn works", section: "saturn" })), ["Saturnus=", "Docs=docs/", "How the Saturn works=docs/saturn/"]);
  assert.deepEqual(names(page("saturn/cpu", { title: "The CPU", section: "saturn" })), ["Saturnus=", "Docs=docs/", "How the Saturn works=docs/saturn/", "The CPU=docs/saturn/cpu/"]);
  assert.deepEqual(names(page("transfer/satx", { title: "satx", section: "transfer" })), ["Saturnus=", "Docs=docs/", "Moving files to a real calculator=docs/transfer/", "satx=docs/transfer/satx/"]);
});

test("every docs page: one h1, a title, a description, its canonical address, Open Graph, its trail as JSON-LD", (t) => {
  const { pages, html } = builtDocs(t);
  const titles = new Map();
  const descriptions = new Map();
  for (const p of pages) {
    const doc = html(p.rel);
    const h = head(doc);
    const url = `${BASE}${pagePaths(p.rel).html}`;
    assert.equal(h.titles, 1, `${p.rel}: one title`);
    assert.equal(h.title, documentTitle(p), `${p.rel}: the title`);
    assert.ok(h.title.includes("Saturnus") && h.title.length <= 70, `${p.rel}: the title names Saturnus in at most 70 characters (${h.title.length})`);
    assert.equal(h.description, p.meta.description, `${p.rel}: the description`);
    assert.ok(h.description.length >= 70 && h.description.length <= 190, `${p.rel}: a description of 70 to 190 characters (${h.description.length})`);
    assert.equal(h.canonicals, 1, `${p.rel}: one canonical link`);
    assert.equal(h.canonical, url, `${p.rel}: its canonical address`);
    assert.ok(!titles.has(h.title), `${p.rel} and ${titles.get(h.title)} have the same title`);
    assert.ok(!descriptions.has(h.description), `${p.rel} and ${descriptions.get(h.description)} have the same description`);
    titles.set(h.title, p.rel);
    descriptions.set(h.description, p.rel);
    // Open Graph and the Twitter card.
    assert.equal(h.meta("og:title"), h.title, `${p.rel}: og:title`);
    assert.equal(h.meta("og:description"), h.description, `${p.rel}: og:description`);
    assert.equal(h.meta("og:url"), url, `${p.rel}: og:url`);
    assert.equal(h.meta("og:site_name"), "Saturnus");
    assert.equal(h.meta("og:type"), p.rel === "index" ? "website" : "article");
    assert.equal(h.meta("og:image"), `${BASE}img/og.png`);
    assert.ok(h.meta("og:image:alt"), `${p.rel}: the image's alt`);
    assert.equal(h.meta("twitter:card"), "summary_large_image");
    // The headings: the title as the one h1, then no level skipped.
    assertHeadingsInOrder(headingLevels(doc), p.rel);
    assert.equal(shown(doc.match(/<h1>([\s\S]*?)<\/h1>/)[1]), p.meta.title, `${p.rel}: the h1 is the page's title`);
    // Every image says what it shows.
    for (const [img] of doc.matchAll(/<img [^>]*>/g)) assert.match(img, / alt="[^"]{12,}"/, `${p.rel}: ${img}`);
    // The trail: the landing page, the docs, the section's overview, the page.
    const crumbs = ldBlocks(doc).filter((b) => b["@type"] === "BreadcrumbList");
    assert.equal(crumbs.length, 1, `${p.rel}: one BreadcrumbList`);
    assert.equal(crumbs[0]["@context"], "https://schema.org");
    const items = crumbs[0].itemListElement;
    assert.deepEqual(items.map((i) => [i["@type"], i.position, i.name, i.item]), breadcrumbs(p).map((c, i) => ["ListItem", i + 1, c.name, `${BASE}${c.path}`]), `${p.rel}: the trail`);
    assert.equal(items.at(-1).item, url, `${p.rel}: the trail ends on the page`);
    // Only data blocks beside the scripts with a src (the CSP allows no inline script).
    for (const [tag] of doc.matchAll(/<script\b[^>]*>/g)) assert.ok(/ src="/.test(tag) || tag === '<script type="application/ld+json">', `${p.rel}: ${tag}`);
  }
});

test("each model page answers its questions, and its FAQPage says what the page shows", (t) => {
  const { pages, html } = builtDocs(t);
  for (const p of pages) {
    const doc = html(p.rel);
    const faq = ldBlocks(doc).filter((b) => b["@type"] === "FAQPage");
    const section = doc.match(/<h2 id="questions-and-answers">Questions and answers<\/h2>([\s\S]*?)(?=<h2[ >]|<\/article>)/)?.[1] ?? null;
    if (!MODEL_ORDER.includes(p.rel)) {
      assert.equal(section, null, `${p.rel}: no questions`);
      assert.equal(faq.length, 0, `${p.rel}: no FAQPage without questions on the page`);
      continue;
    }
    assert.ok(section, `${p.rel}: a "Questions and answers" section`);
    assert.equal(faq.length, 1, `${p.rel}: one FAQPage`);
    // The page's questions: each h3 with the text up to the next.
    const visible = section.split(/(?=<h3[ >])/).filter((part) => part.startsWith("<h3")).map((part) => {
      const m = part.match(/^<h3[^>]*>([\s\S]*?)<\/h3>([\s\S]*)$/);
      return [shown(m[1]), shown(m[2])];
    });
    assert.ok(visible.length >= 5, `${p.rel}: ${visible.length} questions`);
    assert.deepEqual(faq[0].mainEntity.map((q) => [q.name, q.acceptedAnswer.text]), visible, `${p.rel}: the FAQPage is the page's questions and answers`);
    for (const q of faq[0].mainEntity) {
      assert.equal(q["@type"], "Question");
      assert.equal(q.acceptedAnswer["@type"], "Answer");
      assert.match(q.name, /\?$/, `${p.rel}: "${q.name}" is a question`);
      assert.ok(q.acceptedAnswer.text.length >= 40 && q.acceptedAnswer.text.length <= 700, `${p.rel}: "${q.name}" has a short answer (${q.acceptedAnswer.text.length})`);
    }
    // The questions every model answers, in its own words.
    const names = faq[0].mainEntity.map((q) => q.name).join("\n");
    for (const about of [/ROM\?/, /browser\?/, /Mac, Windows and Linux\?/, /a real HP /, /keep my work\?/, /How close/]) assert.match(names, about, `${p.rel}: a question about ${about}`);
    // The Markdown beside the page has them too, and the page links the pages that say more.
    const md = readFileSync(new URL(`docs/pages/${p.rel}.md`, WEB), "utf8");
    assert.equal(faqOf(md).length, visible.length, `${p.rel}.md: the same questions`);
    for (const href of ["../transfer/", "../how-it-is-built/", `../commands/#${p.rel}`, `../../app/#/calculator/${p.rel}`]) assert.ok(doc.includes(`href="${href}`), `${p.rel}: links ${href}`);
    // The title and the description say what the page is for someone searching.
    const h = head(doc);
    assert.match(h.title, new RegExp(`^${p.meta.title} emulator `), `${p.rel}: the title names the emulator`);
    assert.ok(h.description.includes(p.meta.title) && h.description.includes("emulator"), `${p.rel}: the description`);
  }
});

test("the page on how Saturnus is built: in the docs index, the landing page, the sitemap and llms.txt", (t) => {
  const { pages, html } = builtDocs(t);
  const page = pages.find((p) => p.rel === "how-it-is-built");
  assert.ok(page, "the page");
  const doc = html("how-it-is-built");
  for (const word of ["Rust", "WebAssembly", "Tauri", "clean-room", "Web Serial"]) assert.ok(shown(doc).includes(word), word);
  for (const href of ["https://github.com/ractive/saturnus", "https://docs.rs/saturnus-core"]) assert.ok(doc.includes(`href="${href}"`), href);
  assert.ok(html("index").includes('href="how-it-is-built/"'), "the docs index links it");
  assert.ok(landing.includes('href="docs/how-it-is-built/"'), "the landing page links it");
  assert.ok(sitemapXml(pages, BASE, () => null).includes(`<loc>${BASE}docs/how-it-is-built/</loc>`), "the sitemap");
  assert.ok(llmsTxt(pages, commands).includes(`(${BASE}docs/how-it-is-built.md)`), "llms.txt");
});

test("the landing page: what Saturnus is in its title and description, one h1, the application as JSON-LD", () => {
  const h = head(landing);
  assert.equal(h.titles, 1);
  assert.match(h.title, /^Saturnus: /, "the title leads with Saturnus");
  assert.match(h.title, /HP calculator emulator/, "and says what it is");
  assert.ok(h.title.length <= 80, `${h.title.length} characters`);
  assert.match(h.description, /emulator of the HP 48GX/);
  assert.ok(h.description.length >= 70 && h.description.length <= 190, `${h.description.length} characters`);
  assert.equal(h.canonicals, 1);
  assert.equal(h.canonical, BASE);
  assert.equal(h.meta("og:title"), h.title);
  assert.equal(h.meta("og:url"), BASE);
  assert.equal(h.meta("og:site_name"), "Saturnus");
  assert.match(landing, /<h1>Saturnus: an emulator of classic HP calculators[^<]*<\/h1>/);
  assertHeadingsInOrder(headingLevels(landing), "the landing page");
  // Every picture outside the dialogs says what it shows (a dialog repeats its card's picture, with an empty alt).
  for (const [img] of landing.replace(/<dialog[\s\S]*?<\/dialog>/g, "").matchAll(/<img [^>]*>/g)) assert.match(img, / alt="[^"]{12,}"/, img);

  const blocks = ldBlocks(landing);
  assert.deepEqual(blocks.map((b) => b["@type"]), ["SoftwareApplication", "FAQPage"]);
  const app = blocks[0];
  assert.equal(app["@context"], "https://schema.org");
  assert.equal(app.name, "Saturnus");
  assert.equal(app.url, BASE);
  assert.equal(app.applicationCategory, "UtilitiesApplication");
  assert.deepEqual(app.operatingSystem, ["Web browser", "macOS", "Windows", "Linux"]);
  assert.deepEqual(app.offers, { "@type": "Offer", price: "0", priceCurrency: "USD" });
  assert.equal(app.license, "https://github.com/ractive/saturnus/blob/main/LICENSE");
  assert.equal(app.installUrl, `${BASE}app/`);
  // The pictures are the site's own files, the downloads the page's own download links.
  for (const key of ["image", "screenshot"]) {
    assert.ok(app[key].startsWith(`${BASE}img/`), key);
    assert.doesNotThrow(() => readFileSync(new URL(`landing/${app[key].slice(BASE.length)}`, WEB)), `${app[key]} is a file of the landing page`);
  }
  const downloads = [...landing.matchAll(/<a [^>]*href="([^"]*)"[^>]* download>/g)].map((m) => m[1]);
  assert.deepEqual(app.downloadUrl, [...new Set(downloads), "https://github.com/ractive/saturnus/releases/latest"]);
  assert.ok(landing.includes('href="https://github.com/ractive/saturnus/releases/latest"'), "all downloads, linked on the page");
  // Each model the description names is a card on the page.
  for (const m of MODEL_ORDER) assert.ok(landing.includes(`id="dlg-${m}"`), m);
});

test("the landing page's FAQPage is its Questions section, word for word", () => {
  const section = landing.match(/<section id="questions"[^>]*>([\s\S]*?)<\/section>/)?.[1];
  assert.ok(section, "the Questions section");
  const visible = [...section.matchAll(/<li>\s*<h3>([\s\S]*?)<\/h3>\s*<p>([\s\S]*?)<\/p>\s*<\/li>/g)].map((m) => [shown(m[1]), shown(m[2])]);
  assert.ok(visible.length >= 5, `${visible.length} questions`);
  const faq = ldBlocks(landing).find((b) => b["@type"] === "FAQPage");
  assert.deepEqual(faq.mainEntity.map((q) => [q.name, q.acceptedAnswer.text]), visible);
  for (const q of faq.mainEntity) assert.match(q.name, /\?$/);
});

test("the app's page: a title and a description that say what it is, its canonical address", () => {
  const h = head(appIndex);
  assert.equal(h.titles, 1);
  assert.equal(h.title, "Saturnus: HP calculator emulator");
  assert.match(h.description, /^Saturnus emulates the HP 48SX/);
  assert.ok(h.description.length >= 70 && h.description.length <= 190, `${h.description.length} characters`);
  assert.equal(h.canonicals, 1);
  assert.equal(h.canonical, `${BASE}app/`);
  assert.equal(h.meta("og:site_name"), "Saturnus");
  assert.match(h.meta("og:title"), /^Saturnus app: /);
  assert.equal(h.meta("og:description"), h.description);
});
