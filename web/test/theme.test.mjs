// The colour theme (web/theme.js, web/theme-boot.js, style.css): which
// attribute each choice sets, what follows it (color-scheme, the
// theme-color bar, the desktop window), the boot script agreeing with the
// module, and the two dark token sets in style.css staying the same.
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { THEMES, THEME_COLORS, applyTheme, themeAttribute, themeOf } from "../theme.js";

const WEB = new URL("../", import.meta.url);

/** A document with <html>, the color-scheme meta and two theme-color metas. */
function fakeDoc() {
  const attrs = new Map();
  const meta = (name, content, media = null) => {
    const a = new Map([["name", name], ["content", content]]);
    if (media) a.set("media", media);
    return {
      dataset: {},
      getAttribute: (k) => a.get(k) ?? null,
      setAttribute: (k, v) => a.set(k, String(v)),
    };
  };
  const scheme = meta("color-scheme", "light dark");
  const colors = [meta("theme-color", "#f1efe9", "(prefers-color-scheme: light)"), meta("theme-color", "#272724", "(prefers-color-scheme: dark)")];
  return {
    attrs,
    scheme,
    colors,
    documentElement: {
      setAttribute: (k, v) => attrs.set(k, v),
      removeAttribute: (k) => attrs.delete(k),
      getAttribute: (k) => attrs.get(k) ?? null,
    },
    querySelector: (sel) => (sel.includes("color-scheme") ? scheme : null),
    querySelectorAll: (sel) => (sel.includes("theme-color") ? colors : []),
  };
}

test("a stored choice and its attribute", () => {
  assert.deepEqual(THEMES, ["system", "light", "dark"]);
  assert.equal(themeOf("dark"), "dark");
  assert.equal(themeOf(null), "system");
  assert.equal(themeOf("sepia"), "system");
  assert.equal(themeAttribute("system"), null, "System sets nothing");
  assert.equal(themeAttribute("light"), "light");
  assert.equal(themeAttribute("dark"), "dark");
});

test("applying a theme: the attribute, color-scheme, the bar colour and the window", () => {
  const doc = fakeDoc();
  const calls = [];
  const tauri = { core: { invoke: (cmd, args) => { calls.push([cmd, args]); return Promise.resolve(); } } };
  applyTheme("dark", doc, tauri);
  assert.equal(doc.attrs.get("data-theme"), "dark");
  assert.equal(doc.scheme.getAttribute("content"), "dark");
  assert.deepEqual(doc.colors.map((m) => m.getAttribute("content")), [THEME_COLORS.dark, THEME_COLORS.dark]);
  applyTheme("light", doc, tauri);
  assert.equal(doc.attrs.get("data-theme"), "light");
  assert.deepEqual(doc.colors.map((m) => m.getAttribute("content")), [THEME_COLORS.light, THEME_COLORS.light]);
  applyTheme("system", doc, tauri);
  assert.equal(doc.attrs.has("data-theme"), false);
  assert.equal(doc.scheme.getAttribute("content"), "light dark");
  assert.deepEqual(doc.colors.map((m) => m.getAttribute("content")), ["#f1efe9", "#272724"], "each back to its own scheme's colour");
  assert.deepEqual(calls, [["set_theme", { theme: "dark" }], ["set_theme", { theme: "light" }], ["set_theme", { theme: null }]]);
  // In a browser there is no window to tell.
  applyTheme("dark", fakeDoc(), undefined);
});

test("the boot script sets what the module would, before the first paint", () => {
  const src = readFileSync(new URL("theme-boot.js", WEB), "utf8");
  for (const stored of [null, "system", "light", "dark", "sepia"]) {
    const doc = fakeDoc();
    runInNewContext(src, { document: doc, localStorage: { getItem: (k) => (k === "saturnus.theme" ? stored : null) } });
    const attr = themeAttribute(themeOf(stored));
    assert.equal(doc.attrs.get("data-theme") ?? null, attr, `stored ${stored}`);
    // The installed page's bar: the theme's colour, as applyTheme sets it, or each meta's own.
    const bar = doc.colors.map((m) => m.getAttribute("content"));
    assert.deepEqual(bar, attr ? [THEME_COLORS[attr], THEME_COLORS[attr]] : ["#f1efe9", "#272724"], `bar for ${stored}`);
    if (attr) {
      applyTheme("system", doc, undefined);
      assert.deepEqual(doc.colors.map((m) => m.getAttribute("content")), ["#f1efe9", "#272724"], "System later restores each meta's own colour");
    }
  }
  // Storage blocked: the device's scheme.
  const doc = fakeDoc();
  runInNewContext(src, { document: doc, localStorage: { getItem: () => { throw new Error("blocked"); } } });
  assert.equal(doc.attrs.has("data-theme"), false);
  // It is loaded in the head, before the body.
  const html = readFileSync(new URL("index.html", WEB), "utf8");
  assert.ok(html.indexOf('<script src="theme-boot.js"></script>') < html.indexOf("<body>"));
});

test("style.css: the dark tokens are the same for Dark and for a dark device", () => {
  const css = readFileSync(new URL("style.css", WEB), "utf8");
  const block = (start) => {
    const i = css.indexOf(start);
    assert.ok(i >= 0, start);
    const body = css.slice(i + start.length, css.indexOf("}", i));
    return body.split(";").map((d) => d.trim()).filter(Boolean);
  };
  const forced = block(':root[data-theme="dark"] {');
  const device = block(':root:not([data-theme="light"]) {');
  assert.ok(forced.length > 20, "the whole set");
  assert.deepEqual(forced, device);
  assert.ok(forced.includes("color-scheme: dark"));
  assert.match(css, /:root\[data-theme="light"\] \{\s*color-scheme: light;/);
});
