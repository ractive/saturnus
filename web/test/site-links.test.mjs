// The links between the site's pages (web/shared/site-links.js): the
// site's sibling layout (/saturnus/, emulator/, transfer/), web/'s layout
// served directly (index.html, tx.html, landing/), and none in the
// desktop app. The pages themselves: web/test/nav-page.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { applySiteLinks, inDesktopApp, siteHref } from "../shared/site-links.js";

test("on the site the pages are siblings under /saturnus/", () => {
  for (const path of ["/saturnus/emulator/", "/saturnus/emulator/index.html", "/emulator/"]) {
    assert.equal(siteHref("transfer", path), "../transfer/", path);
    assert.equal(siteHref("landing", path), "../", path);
  }
  for (const path of ["/saturnus/transfer/", "/saturnus/transfer/index.html", "/transfer/"]) {
    assert.equal(siteHref("emulator", path), "../emulator/", path);
    assert.equal(siteHref("landing", path), "../", path);
  }
});

test("served from web/ the emulator is index.html, saturnus tx tx.html, the landing page landing/", () => {
  for (const path of ["/", "/index.html"]) {
    assert.equal(siteHref("transfer", path), "tx.html");
    assert.equal(siteHref("landing", path), "landing/");
  }
  assert.equal(siteHref("emulator", "/tx.html"), "./");
  assert.equal(siteHref("landing", "/tx.html"), "landing/");
  // A folder named like a subsite higher up is not the site.
  assert.equal(siteHref("transfer", "/emulator/web/index.html"), "tx.html");
  assert.equal(siteHref("nowhere", "/saturnus/emulator/"), null);
});

/** A document of `a[data-site]` links, enough for applySiteLinks. */
function fakeDoc(links) {
  const as = links.map(({ site, keep = false, href }) => {
    const attrs = new Map([["href", href]]);
    if (keep) attrs.set("data-site-keep", "");
    return {
      dataset: { site },
      hidden: !keep,
      hasAttribute: (n) => attrs.has(n),
      getAttribute: (n) => attrs.get(n) ?? null,
      setAttribute: (n, v) => attrs.set(n, v),
    };
  });
  return { as, querySelectorAll: () => as };
}

test("in a browser the links show with the layout's hrefs", () => {
  const doc = fakeDoc([{ site: "landing", keep: true, href: "./" }, { site: "emulator", href: "../emulator/" }]);
  applySiteLinks(doc, { location: { pathname: "/saturnus/transfer/" } });
  assert.deepEqual(doc.as.map((a) => [a.getAttribute("href"), a.hidden]), [["../", false], ["../emulator/", false]]);
  const dev = fakeDoc([{ site: "landing", keep: true, href: "./" }, { site: "emulator", href: "../emulator/" }]);
  applySiteLinks(dev, { location: { pathname: "/tx.html" } });
  assert.deepEqual(dev.as.map((a) => [a.getAttribute("href"), a.hidden]), [["landing/", false], ["./", false]]);
});

test("in the desktop app the links stay hidden; the brand stays and keeps its own page", () => {
  const win = { __TAURI__: {}, location: { pathname: "/index.html" } };
  assert.equal(inDesktopApp(win), true);
  assert.equal(inDesktopApp({ location: win.location }), false);
  const doc = fakeDoc([{ site: "landing", keep: true, href: "./" }, { site: "transfer", href: "../transfer/" }]);
  applySiteLinks(doc, win);
  assert.deepEqual(doc.as.map((a) => [a.getAttribute("href"), a.hidden]), [["./", false], ["../transfer/", true]]);
});
