// Links to other sites on the landing page (web/landing/index.html, its
// model dialogs included) and in the app's texts (web/shared/ext-links.js):
// each opens in a new tab with rel="noopener noreferrer" and its ↗, and
// one address is a link at most twice on the page, the header's and the
// footer's navigation and the dialogs' source lists aside. The docs
// pages' links: web/test/docs.test.mjs.
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

test("the landing page: every link to another site opens in a new tab, with its arrow", () => {
  const ext = anchors(landing).filter((a) => isExternal(a.href));
  assert.ok(ext.length > 40, `${ext.length} links to other sites`);
  for (const a of ext) {
    assert.match(a.attrs, /target="_blank" rel="noopener noreferrer"/, a.href);
    assert.ok(a.inner.endsWith(MARKS), `${a.href}: the arrow and the words for screen readers`);
  }
  for (const a of anchors(landing).filter((x) => !isExternal(x.href))) assert.doesNotMatch(a.attrs, /target=/, `${a.href} stays in the tab`);
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
