// The logo's colours per model (web/logo.js, web/theme-boot.js, style.css,
// logo.svg): which models have their own, the attribute following the
// running model, the boot script picking the model the page will show,
// style.css's table covering every model with each colour shown at least
// 2.4:1 on both panels, and the static logo plain and flat.
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

test("the boot script sets what the page will show, before the first paint", () => {
  const order = readFileSync(new URL("models.js", WEB), "utf8");
  const src = readFileSync(new URL("theme-boot.js", WEB), "utf8");
  const boot = (getItem) => {
    const doc = fakeDoc();
    const ctx = { document: doc, localStorage: { getItem } };
    runInNewContext(order, ctx);
    runInNewContext(src, ctx);
    return doc.attrs.get("data-logo") ?? null;
  };
  for (const m of LOGO_MODELS) assert.equal(boot((k) => (k === "saturnus.model" ? m : null)), m, `stored ${m}`);
  // None remembered, an unknown one, or storage blocked: the first model, as app.js picks.
  for (const stored of [null, "junk", "48g"]) assert.equal(boot((k) => (k === "saturnus.model" ? stored : null)), MODEL_ORDER[0], `stored ${stored}`);
  assert.equal(boot(() => { throw new Error("blocked"); }), MODEL_ORDER[0], "storage blocked");
  // The page loads the order before the boot script, both in the head.
  const html = readFileSync(new URL("index.html", WEB), "utf8");
  assert.ok(html.indexOf('<script src="models.js"></script>') < html.indexOf('<script src="theme-boot.js"></script>'));
  assert.ok(html.indexOf('<script src="theme-boot.js"></script>') < html.indexOf("<body>"));
});

/** WCAG relative luminance contrast of two "#rrggbb". */
function contrast(a, b) {
  const lum = (h) => {
    const [r, g, bl] = [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}
/** The HSL hue of "#rrggbb", in degrees. */
function hue(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  const h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return (h * 60 + 360) % 360;
}

/** The least contrast of a header logo colour against its panel (style.css). */
const MIN_CONTRAST = 2.4;

test("style.css: a pair per model, each colour at least 2.4:1 on both panels, tinted only where short", () => {
  const css = readFileSync(new URL("style.css", WEB), "utf8");
  const props = (selector) => {
    const i = css.indexOf(`${selector} { --logo-planet`);
    assert.ok(i >= 0, selector);
    return Object.fromEntries([...css.slice(i, css.indexOf("}", i)).matchAll(/--logo-([a-z-]+): (#[0-9a-f]{6})/g)].map((m) => [m[1], m[2]]));
  };
  // The panels the headers' logo is drawn on: the light and the dark `--panel`.
  const panels = [...css.matchAll(/--panel: (#[0-9a-f]{6});/g)].map((m) => m[1]);
  assert.deepEqual([...new Set(panels)], ["#f1efe9", "#272724"]);
  const PANEL = { light: "#f1efe9", dark: "#272724" };
  for (const selector of [":root", ...LOGO_MODELS.map((m) => `[data-logo="${m}"]`)]) {
    const p = props(selector);
    for (const part of ["planet", "ring"]) {
      const skin = p[part];
      assert.ok(skin, `${selector} ${part}`);
      for (const [theme, panel] of Object.entries(PANEL)) {
        const tint = p[`${part}-${theme}`];
        const what = `${selector} ${part} on the ${theme} panel`;
        if (contrast(skin, panel) >= MIN_CONTRAST) {
          assert.equal(tint, undefined, `${what}: no tint where the skin's colour shows`);
          continue;
        }
        assert.ok(tint, `${what}: ${contrast(skin, panel).toFixed(2)}:1 needs a tint`);
        assert.ok(contrast(tint, panel) >= MIN_CONTRAST, `${what}: the tint ${contrast(tint, panel).toFixed(2)}:1`);
        assert.ok(contrast(tint, panel) < MIN_CONTRAST + 0.15, `${what}: the tint goes no further than it needs`);
        const dh = Math.abs(hue(tint) - hue(skin));
        assert.ok(Math.min(dh, 360 - dh) <= 3, `${what}: the tint keeps the hue`);
      }
    }
  }
  const svgColors = (file) => {
    const svg = readFileSync(new URL(file, WEB), "utf8");
    const fills = [...svg.matchAll(/fill="(#[0-9a-f]{6})"/g)].map((m) => m[1]);
    const strokes = new Set([...svg.matchAll(/stroke="(#[0-9a-f]{6})"/g)].map((m) => m[1]));
    assert.equal(fills.length, 1, `${file}: the planet only, no highlight`);
    assert.equal(strokes.size, 1, `${file}: one ring colour, no highlight`);
    return { planet: fills[0], ring: [...strokes][0] };
  };
  assert.deepEqual(props(":root"), svgColors("logo.svg"));
  assert.equal(readFileSync(new URL("favicon.svg", WEB), "utf8"), readFileSync(new URL("logo.svg", WEB), "utf8"));
});
