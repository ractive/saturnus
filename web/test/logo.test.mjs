// The logo's colours per model (web/logo.js, web/theme-boot.js, style.css,
// logo.svg): which models have their own, the attribute following the
// running model, the boot script agreeing with the module, style.css's
// table covering every model, and the static logo plain and flat.
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { LOGO_MODELS, LOGO_PARTS, drawLogo, logoAttribute, watchLogo } from "../logo.js";
import { MODEL_ORDER } from "../norom.js";
import { Store } from "../store.js";

const WEB = new URL("../", import.meta.url);

/** A document with <html> only. */
function fakeDoc() {
  const attrs = new Map();
  return {
    attrs,
    documentElement: {
      setAttribute: (k, v) => attrs.set(k, v),
      removeAttribute: (k) => attrs.delete(k),
      getAttribute: (k) => attrs.get(k) ?? null,
    },
    querySelector: () => null,
    querySelectorAll: () => [],
  };
}

test("every model has its own colours, anything else the plain logo", () => {
  assert.deepEqual([...LOGO_MODELS].sort(), [...MODEL_ORDER].sort());
  for (const m of LOGO_MODELS) assert.equal(logoAttribute(m), m);
  for (const m of [null, undefined, "", "48g", "toString"]) assert.equal(logoAttribute(m), null, String(m));
});

test("the attribute follows the running model, else the selected one", () => {
  const store = new Store();
  const doc = fakeDoc();
  watchLogo(store, doc);
  assert.equal(doc.attrs.has("data-logo"), false, "no model: the plain logo");
  store.set({ model: "48gx" });
  assert.equal(doc.attrs.get("data-logo"), "48gx", "nothing running: the selected model");
  store.set({ booted: "48sx" });
  assert.equal(doc.attrs.get("data-logo"), "48sx", "the running model wins");
  store.set({ booted: "49g", model: "49g" });
  assert.equal(doc.attrs.get("data-logo"), "49g");
});

test("the case's mark: a g.logo with the model's attribute, scaled into the skin's box", () => {
  const made = [];
  const make = (tag, attrs, parent) => {
    const e = { tag, attrs, parent };
    made.push(e);
    return e;
  };
  const g = drawLogo(make, "face", [66, 42, 48, 48], "42s");
  assert.deepEqual(g.attrs, { class: "logo", transform: "translate(66 42) scale(1.5 1.5)", "data-logo": "42s" });
  assert.deepEqual(made.slice(1).map((e) => [e.tag, e.attrs.class, e.parent === g]), LOGO_PARTS.map(([t, a]) => [t, a.class, true]));
  assert.equal(drawLogo(make, "face", [0, 0, 32, 32], "nope").attrs["data-logo"], undefined);
});

test("the boot script sets what the module would, before the first paint", () => {
  const src = readFileSync(new URL("theme-boot.js", WEB), "utf8");
  for (const stored of [null, ...LOGO_MODELS, "junk"]) {
    const doc = fakeDoc();
    runInNewContext(src, { document: doc, localStorage: { getItem: (k) => (k === "saturnus.model" ? stored : null) } });
    assert.equal(doc.attrs.get("data-logo") ?? null, logoAttribute(stored), `stored ${stored}`);
  }
  const doc = fakeDoc();
  runInNewContext(src, { document: doc, localStorage: { getItem: () => { throw new Error("blocked"); } } });
  assert.equal(doc.attrs.has("data-logo"), false);
});

test("style.css has a pair per model and logo.svg's as the default; the static logo is flat", () => {
  const css = readFileSync(new URL("style.css", WEB), "utf8");
  const pair = (selector) => {
    const m = new RegExp(`${selector.replace(/[[\]"]/g, "\\$&")} \\{ --logo-planet: (#[0-9a-f]{6}); --logo-ring: (#[0-9a-f]{6}); \\}`).exec(css);
    assert.ok(m, selector);
    return { planet: m[1], ring: m[2] };
  };
  for (const m of LOGO_MODELS) pair(`[data-logo="${m}"]`);
  const svgColors = (file) => {
    const svg = readFileSync(new URL(file, WEB), "utf8");
    const fills = [...svg.matchAll(/fill="(#[0-9a-f]{6})"/g)].map((m) => m[1]);
    const strokes = new Set([...svg.matchAll(/stroke="(#[0-9a-f]{6})"/g)].map((m) => m[1]));
    assert.equal(fills.length, 1, `${file}: the planet only, no highlight`);
    assert.equal(strokes.size, 1, `${file}: one ring colour, no highlight`);
    return { planet: fills[0], ring: [...strokes][0] };
  };
  assert.deepEqual(pair(":root"), svgColors("logo.svg"));
  assert.equal(readFileSync(new URL("favicon.svg", WEB), "utf8"), readFileSync(new URL("logo.svg", WEB), "utf8"));
});
