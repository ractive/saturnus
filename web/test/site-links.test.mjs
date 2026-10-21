// The links between the site's two pages (web/shared/site-links.js): the
// landing page and the app, in the site's layout (/saturnus/, app/) and
// web/'s served directly (index.html, landing/), none in the desktop app;
// which calculator Files connects from Calculator's link, and why not.
// The app's routes (web/router.js). The pages themselves:
// web/test/nav-page.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { applySiteLinks, connectModel, connectRefusal, inDesktopApp, refusalText, siteHref } from "../shared/site-links.js";
import { parseRoute, routeHash, startRoute } from "../router.js";

test("on the site the app sits in app/ under the landing page", () => {
  for (const path of ["/saturnus/app/", "/saturnus/app/index.html", "/app/"]) {
    assert.equal(siteHref("landing", path), "../", path);
    assert.equal(siteHref("app", path), "./", path);
    // The docs beside the app (iteration 57).
    assert.equal(siteHref("docs", path), "../docs/", path);
    assert.equal(siteHref("commands", path), "../docs/commands/", path);
  }
});

test("served from web/ the app is index.html, the landing page landing/", () => {
  for (const path of ["/", "/index.html"]) {
    assert.equal(siteHref("landing", path), "landing/");
    assert.equal(siteHref("app", path), "./");
    // web/ has no docs: the site's.
    assert.equal(siteHref("commands", path), "https://ractive.ch/saturnus/docs/commands/");
  }
  // A folder named like the app higher up is not the site.
  assert.equal(siteHref("landing", "/app/web/index.html"), "landing/");
  assert.equal(siteHref("nowhere", "/saturnus/app/"), null);
});

/** A document of `a[data-site]` links, enough for applySiteLinks. */
function fakeDoc(links) {
  const as = links.map(({ site, href }) => {
    const attrs = new Map([["href", href]]);
    return {
      dataset: { site },
      hidden: true,
      hasAttribute: (n) => attrs.has(n),
      getAttribute: (n) => attrs.get(n) ?? null,
      setAttribute: (n, v) => attrs.set(n, v),
    };
  });
  return { as, querySelectorAll: () => as };
}

test("in a browser the links show with the layout's hrefs", () => {
  const doc = fakeDoc([{ site: "landing", href: "../" }]);
  applySiteLinks(doc, { location: { pathname: "/saturnus/app/" } });
  assert.deepEqual(doc.as.map((a) => [a.getAttribute("href"), a.hidden]), [["../", false]]);
  const dev = fakeDoc([{ site: "landing", href: "../" }]);
  applySiteLinks(dev, { location: { pathname: "/index.html" } });
  assert.deepEqual(dev.as.map((a) => [a.getAttribute("href"), a.hidden]), [["landing/", false]]);
});

test("in the desktop app the links stay hidden", () => {
  const win = { __TAURI__: {}, location: { pathname: "/index.html" } };
  assert.equal(inDesktopApp(win), true);
  assert.equal(inDesktopApp({ location: win.location }), false);
  const doc = fakeDoc([{ site: "landing", href: "../" }]);
  applySiteLinks(doc, win);
  assert.deepEqual(doc.as.map((a) => [a.getAttribute("href"), a.hidden]), [["../", true]]);
});

test("Calculator's link to Files connects the running calculator when Files can reach it", () => {
  const on = (booted, over = {}) => connectModel({ booted, kept: true, ...over });
  for (const m of ["48sx", "48gx", "49g"]) assert.equal(on(m), m, m);
  // No serial port (the Pioneers, the 28S, the 19BII) or aplets served to them (38G, 39G, 40G).
  for (const m of ["42s", "27s", "17bii", "17b", "28s", "19bii", "38g", "39g", "40g"]) assert.equal(on(m), null, m);
  assert.equal(on(null), null, "nothing running");
  assert.equal(on("48gx", { kept: false }), null, "a ROM not kept: Files would start another calculator");
});

test("connectRefusal: why Files does not connect a calculator, in words", () => {
  const why = (booted, over = {}) => connectRefusal({ booted, kept: true, ...over });
  assert.equal(why(null), null, "nothing running: nothing said");
  assert.equal(why("48gx"), null, "it connects");
  assert.equal(why("38g"), "aplet");
  assert.equal(why("42s"), "noserial");
  assert.equal(why("49g", { kept: false }), "unkept");
  assert.equal(refusalText("38g", "aplet"), "The HP 38G is not connected here: it sends and receives files itself, from a folder Files serves to it.");
  assert.equal(refusalText("42s", "noserial"), "The HP 42S has no serial port: Files cannot reach it.");
  assert.match(refusalText("49g", "unkept"), /^The HP 49G is not connected here: Files reaches only a calculator whose ROM this browser keeps/);
  assert.equal(refusalText("48gx", "nonsense"), null, "an unknown code: nothing said");
});

test("the routes: home, Calculator and Files, each on a model or none", () => {
  assert.equal(parseRoute(""), null, "no route: the shell picks the start");
  assert.equal(parseRoute("#"), null);
  assert.deepEqual(parseRoute("#/"), { view: "home", model: null });
  assert.deepEqual(parseRoute("#/calculator"), { view: "calculator", model: null });
  assert.deepEqual(parseRoute("#/calculator/"), { view: "calculator", model: null });
  assert.deepEqual(parseRoute("#/calculator/48gx"), { view: "calculator", model: "48gx" });
  assert.deepEqual(parseRoute("#/files"), { view: "files", model: null });
  assert.deepEqual(parseRoute("#/files/17bii"), { view: "files", model: "17bii" });
  assert.deepEqual(parseRoute("#/files/%3Cscript%3E"), { view: "files", model: null }, "only a model's name");
  assert.deepEqual(parseRoute("#/nowhere"), { view: "home", model: null });
  assert.deepEqual(parseRoute("#/files/48gx/more"), { view: "home", model: null });
  assert.deepEqual(parseRoute("#files"), { view: "home", model: null });
  for (const r of [{ view: "home" }, { view: "calculator", model: null }, { view: "calculator", model: "49g" }, { view: "files", model: null }, { view: "files", model: "48sx" }]) {
    assert.deepEqual(parseRoute(routeHash(r)), { model: null, ...r }, routeHash(r));
  }
  assert.equal(routeHash({ view: "files", model: "48gx" }), "#/files/48gx");
  assert.equal(routeHash({ view: "home" }), "#/");
});

test("the start: the last calculator when the setting is on and its ROM kept, else home", () => {
  assert.deepEqual(startRoute({}), { view: "home", model: null });
  assert.deepEqual(startRoute({ lastModel: "48gx", bootLast: true }), { view: "calculator", model: "48gx" });
  assert.deepEqual(startRoute({ lastModel: "48gx", bootLast: false }), { view: "home", model: null }, "the setting off");
  assert.deepEqual(startRoute({ lastModel: null, bootLast: true }), { view: "home", model: null }, "no last calculator");
});
