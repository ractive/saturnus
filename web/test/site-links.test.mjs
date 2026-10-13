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

test("modelParam: the model saturnus tx opened the emulator on, read once and taken off the address", async () => {
  const { modelParam } = await import("../shared/site-links.js");
  const page = (href) => {
    const win = { location: { href }, history: { state: { s: 1 }, replaceState(state, _t, url) { win.replaced = { state, url }; } } };
    return win;
  };
  const a = page("https://ractive.ch/saturnus/emulator/?model=48gx#x");
  assert.equal(modelParam(a), "48gx");
  assert.deepEqual(a.replaced, { state: { s: 1 }, url: "https://ractive.ch/saturnus/emulator/#x" });
  const none = page("https://ractive.ch/saturnus/emulator/");
  assert.equal(modelParam(none), null);
  assert.equal(none.replaced, undefined, "an address without it is left alone");
  const bad = page("http://127.0.0.1:4860/?model=%3Cscript%3E&x=1");
  assert.equal(modelParam(bad), null, "only a model's name");
  assert.equal(bad.replaced.url, "http://127.0.0.1:4860/?x=1");
});

test("the emulator's link to saturnus tx connects the running calculator when tx can use it", async () => {
  const { connectModel, transferHref } = await import("../shared/site-links.js");
  const on = (booted, over = {}) => connectModel({ booted, shared: true, kept: true, ...over });
  for (const m of ["48sx", "48gx", "49g"]) assert.equal(on(m), m, m);
  // No serial port (the Pioneers, the 28S, the 19BII) or aplets served to them (38G, 39G, 40G): the plain link.
  for (const m of ["42s", "27s", "17bii", "17b", "28s", "19bii", "38g", "39g", "40g"]) assert.equal(on(m), null, m);
  assert.equal(on(null), null, "nothing running");
  assert.equal(on("48gx", { shared: false }), null, "the page's own Worker: tx cannot reach it");
  assert.equal(on("48gx", { kept: false }), null, "a ROM not kept: tx would start another calculator");
  assert.equal(transferHref("/index.html", "48gx"), "tx.html?connect=48gx");
  assert.equal(transferHref("/saturnus/emulator/", "49g"), "../transfer/?connect=49g");
  assert.equal(transferHref("/saturnus/emulator/", null), "../transfer/");
});

test("takeParam reads ?connect= once and takes it off the address", async () => {
  const { takeParam } = await import("../shared/site-links.js");
  const win = { location: { href: "https://ractive.ch/saturnus/transfer/?connect=48gx" }, history: { state: null, replaceState(_s, _t, url) { win.url = url; } } };
  assert.equal(takeParam("connect", win), "48gx");
  assert.equal(win.url, "https://ractive.ch/saturnus/transfer/");
});

test("the links between the pages name the other page's tab", async () => {
  const { TAB_NAMES } = await import("../shared/site-links.js");
  const doc = fakeDoc([{ site: "landing", keep: true, href: "./" }, { site: "emulator", href: "../emulator/" }, { site: "transfer", href: "../transfer/" }]);
  applySiteLinks(doc, { location: { pathname: "/saturnus/transfer/" } });
  assert.deepEqual(doc.as.map((a) => a.target ?? null), [null, "saturnus-emulator", "saturnus-tx"]);
  assert.deepEqual(TAB_NAMES, { emulator: "saturnus-emulator", transfer: "saturnus-tx" });
});

/** A window that opens `tab` for any name (null: a blocker), on `href`; records what it did. */
function opener(tab, href = "https://ractive.ch/saturnus/emulator/") {
  const win = {
    did: [],
    location: { href, origin: new URL(href).origin, assign: (u) => win.did.push(["assign", u]) },
    open: (url, name) => {
      win.did.push(["open", url, name]);
      return tab;
    },
  };
  return win;
}

/** A tab on `href` (a function: one that throws, another origin's page). */
function fakeTab(href) {
  const tab = { did: [], focus: () => tab.did.push(["focus"]), postMessage: (m, origin) => tab.did.push(["post", m, origin]) };
  let at = href;
  tab.location = {
    get href() {
      if (typeof at === "function") return at();
      return at;
    },
    set href(u) {
      tab.did.push(["load", u]);
      at = u;
    },
  };
  return tab;
}

test("openSiteTab: the open tab is brought forward and told; no tab, or another page in it, is loaded", async () => {
  const { openSiteTab } = await import("../shared/site-links.js");
  const url = "https://ractive.ch/saturnus/transfer/?connect=48gx";
  // The tab is there on saturnus tx: told, not loaded again.
  const tx = fakeTab("https://ractive.ch/saturnus/transfer/index.html");
  const w = opener(tx);
  assert.equal(openSiteTab("transfer", url, { connect: "48gx" }, {}, w), "told");
  assert.deepEqual(w.did, [["open", "", "saturnus-tx"]], "looked up by name, nothing opened");
  assert.deepEqual(tx.did, [["post", { type: "saturnus-site", connect: "48gx" }, "https://ractive.ch"], ["focus"]]);
  // A tab the lookup has just opened (about:blank), another page of ours, another origin's: loaded with the URL.
  for (const at of ["about:blank", "https://ractive.ch/saturnus/", () => { throw new Error("cross-origin"); }]) {
    const tab = fakeTab(at);
    assert.equal(openSiteTab("transfer", url, { connect: "48gx" }, {}, opener(tab)), "opened");
    assert.deepEqual(tab.did, [["load", url]]);
  }
  // A relative URL, from web/ (the emulator at /, tx at tx.html).
  const local = fakeTab("about:blank");
  openSiteTab("transfer", "tx.html", null, {}, opener(local, "http://127.0.0.1:4860/"));
  assert.deepEqual(local.did, [["load", "http://127.0.0.1:4860/tx.html"]]);
  // No tab at all (a blocker): this one goes.
  const blocked = opener(null);
  assert.equal(openSiteTab("transfer", url, null, {}, blocked), "here");
  assert.deepEqual(blocked.did.at(-1), ["assign", url]);
  // ... unless told to stay (saturnus tx's "Open in the emulator").
  const stay = opener(null);
  assert.equal(openSiteTab("emulator", url, null, { here: false }, stay), "blocked");
  assert.equal(stay.did.some(([what]) => what === "assign"), false);
  // Nothing to ask: brought forward only.
  const emu = fakeTab("https://ractive.ch/saturnus/emulator/");
  assert.equal(openSiteTab("emulator", "https://ractive.ch/saturnus/emulator/", null, {}, opener(emu, "https://ractive.ch/saturnus/transfer/")), "told");
  assert.deepEqual(emu.did, [["focus"]]);
});

test("onSiteMessage takes saturnus requests of this origin from the tab that opened this one or one it opened", async () => {
  const { onSiteMessage } = await import("../shared/site-links.js");
  let listener = null;
  const opened = {};
  const win = { location: { origin: "https://ractive.ch" }, addEventListener: (type, fn) => type === "message" && (listener = fn) };
  const openerTab = {};
  win.opener = openerTab;
  opened.opener = win;
  const got = [];
  onSiteMessage((m) => got.push(m), win);
  const msg = (connect) => ({ type: "saturnus-site", connect });
  listener({ origin: "https://ractive.ch", source: openerTab, data: msg("48gx") });
  listener({ origin: "https://ractive.ch", source: opened, data: msg("49g") });
  listener({ origin: "https://evil.example", source: openerTab, data: msg("48sx") });
  listener({ origin: "https://ractive.ch", source: { opener: null }, data: msg("48sx") });
  listener({ origin: "https://ractive.ch", source: win, data: msg("48sx") });
  listener({ origin: "https://ractive.ch", source: openerTab, data: { connect: "48sx" } });
  listener({ origin: "https://ractive.ch", source: openerTab, data: null });
  assert.deepEqual(got, [msg("48gx"), msg("49g")]);
});
