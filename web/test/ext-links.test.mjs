// Links to other sites on the landing page (web/landing/index.html, its
// model dialogs included) and in the app's texts (web/shared/ext-links.js):
// each opens in a new tab with rel="noopener noreferrer" and its ↗ (a
// `download` link, a file, stays in the tab without them), and one address is a link at most twice on the page, the header's and the
// footer's navigation and the dialogs' source lists aside. The docs
// pages' links: web/test/docs.test.mjs. Also the addresses
// scripts/check-links.mjs reads from a docs page.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { LinkBudget, isExternal, urlKey } from "../shared/ext-links.js";

const landing = readFileSync(new URL("../landing/index.html", import.meta.url), "utf8");
const MARKS = '<span class="ext" aria-hidden="true">↗</span><span class="visually-hidden"> (opens in a new tab)</span>';

/** Every <a> of `html`: its opening tag, href and text (the markup inside). */
function anchors(html) {
  return [...html.matchAll(/<a ([^>]*)>([\s\S]*?)<\/a>/g)].map(([, attrs, inner]) => ({ attrs, inner, href: attrs.match(/href="([^"]*)"/)?.[1] ?? "" }));
}

/** A link to a file to save (the desktop app's installers): it opens no page. */
const isDownload = (a) => /(^|\s)download(\s|=|$)/.test(a.attrs);

test("the landing page: every link to another site opens in a new tab, with its arrow", () => {
  const ext = anchors(landing).filter((a) => isExternal(a.href) && !isDownload(a));
  assert.ok(ext.length > 40, `${ext.length} links to other sites`);
  for (const a of ext) {
    assert.match(a.attrs, /target="_blank" rel="noopener noreferrer"/, a.href);
    assert.ok(a.inner.endsWith(MARKS), `${a.href}: the arrow and the words for screen readers`);
  }
  for (const a of anchors(landing).filter((x) => !isExternal(x.href) || isDownload(x))) {
    assert.doesNotMatch(a.attrs, /target=/, `${a.href} stays in the tab`);
    assert.doesNotMatch(a.inner, /class="ext"/, `${a.href}: no arrow`);
  }
});

test("the landing page: the installers' links are the names desktop.yml gives the newest release's copies", () => {
  const latest = "https://github.com/ractive/saturnus/releases/latest/download/";
  const files = anchors(landing).filter(isDownload).map((a) => a.href);
  assert.deepEqual(files, ["saturnus-mac.dmg", "saturnus-windows-x64-setup.exe", "saturnus-windows-arm64-setup.exe"].map((f) => latest + f));
  const workflow = readFileSync(new URL("../../.github/workflows/desktop.yml", import.meta.url), "utf8");
  for (const f of files) assert.ok(workflow.includes(` ${f.slice(latest.length)}\n`), `desktop.yml copies an installer to ${f.slice(latest.length)}`);
});

test("the landing page: an address is a link at most twice, navigation and sources aside; hpcalc.org named is a link", () => {
  const content = landing
    .replace(/<header class="top">[\s\S]*?<\/header>/, "")
    .replace(/<footer>[\s\S]*?<\/footer>/, "")
    .replace(/<div class="dlg-part dlg-sources">[\s\S]*?<\/div>/g, "");
  const counts = new Map();
  for (const a of anchors(content).filter((x) => isExternal(x.href))) counts.set(urlKey(a.href), (counts.get(urlKey(a.href)) ?? 0) + 1);
  for (const [url, n] of counts) assert.ok(n <= 2, `${url}: ${n} links`);
  assert.equal(counts.get(urlKey("https://www.hpcalc.org/")), 2, "the two mentions of hpcalc.org in Get it");
});

test("a link budget counts an address however it is written", () => {
  const b = new LinkBudget();
  assert.equal(b.take("https://www.hpcalc.org/"), true);
  assert.equal(b.take("https://hpcalc.org"), true);
  assert.equal(b.take("http://HPCALC.org/"), false);
  assert.equal(b.take("https://www.hpcalc.org/details/1"), true, "another page of the site is another address");
});

test("scripts/check-links.mjs reads a page's addresses: parentheses in one, autolinks, site names outside code", async () => {
  const { isAppText, markdownUrls } = await import("../../scripts/check-links.mjs");
  // The app's scripts: web/'s own and its folders', not the other pages', the tests' or the vendored and generated code.
  for (const rel of ["home.js", "components/sat-about.js", "shared/ext-links.js", "tx/app.js"]) assert.equal(isAppText(rel), true, rel);
  for (const rel of ["pkg/saturnus_web.js", "vendor/tab-mcp-client/x.js", "test/chrome.mjs", "docs/commands/page.js", "landing/dialogs.js"]) assert.equal(isAppText(rel), false, rel);
  const md = [
    "A [photo](https://commons.wikimedia.org/wiki/File:HP_28S_(open).jpg) and <https://e.org/a>.",
    "From hpcalc.org, not from hpcomm.org; [inside](48gx.md).",
    "`crates.io` and",
    "```",
    "scoop bucket add r https://github.com/r/b",
    "```",
  ].join("\n");
  assert.deepEqual(markdownUrls(md), ["https://commons.wikimedia.org/wiki/File:HP_28S_(open).jpg", "https://e.org/a", "https://www.hpcalc.org/"]);
});
