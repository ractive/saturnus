// The logo in the model's colours on the real page in headless Chrome over
// the DevTools protocol: the side panel's and the phone bar's logo take the
// running model's colours, the selected model's with nothing running, the
// plain logo's with no model, and change when the model does; the case's
// logo takes its own model's; the remembered model's colours are there
// before the page's modules run; and the colours come from the skins (the
// two shift keys where a model has two, else the shift key's hue for the
// planet). No ROM: the running model is set in the store. Skipped without
// Chrome (`SATURNUS_CHROME` names one) or the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

/** The pairs of style.css's table, and logo.svg's for no model. */
const PLAIN = { planet: "#d9874a", ring: "#3f9a8c" };
const PAIRS = {
  "48sx": { planet: "#e2893c", ring: "#8ebfd2" },
  "48gx": { planet: "#8c86cc", ring: "#2ba6a2" },
  "49g": { planet: "#4d59a3", ring: "#d5493c" },
  "38g": { planet: "#4fb09c", ring: "#cf8e78" },
  "39g": { planet: "#c64d0a", ring: "#3c6cc3" },
  "40g": { planet: "#c64d0a", ring: "#3c6cc3" },
  "42s": { planet: "#ef8b1d", ring: "#89807a" },
};

/** "rgb(r, g, b)" of "#rrggbb". */
const rgb = (hex) => `rgb(${[1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)).join(", ")})`;
/** The HSL hue of "#rrggbb", in whole degrees. */
function hue(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  const h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return Math.round((h * 60 + 360) % 360);
}

// Each logo's computed planet and ring colours: the two headers', then the case's.
const LOGOS = `[...document.querySelectorAll("header.bar svg.logo, .panel-head svg.logo, sat-calculator .skin g.logo")].map((l) => ({
  planet: getComputedStyle(l.querySelector(".planet")).fill,
  ring: [...l.querySelectorAll(".ring")].map((r) => getComputedStyle(r).stroke),
}))`;

test("the header logos and the case logo follow the model", { timeout: 120_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started");

  const look = (p) => ({ planet: rgb(p.planet), ring: [rgb(p.ring), rgb(p.ring)] });
  const expect = async (heads, skin, what) => {
    const want = [look(heads), look(heads), look(skin)];
    let got;
    const end = Date.now() + 5_000;
    for (;;) {
      got = await ev(LOGOS);
      if (JSON.stringify(got) === JSON.stringify(want) || Date.now() > end) break;
      await sleep(100);
    }
    assert.deepEqual(got, want, what);
  };

  // No model: the plain logo in the headers.
  await ev(`window.saturnus.store.set({ booted: null, model: null })`);
  await until(ev, `!document.documentElement.hasAttribute("data-logo")`, 5_000, "no model, no attribute");
  // Nothing running: the Model select's choice.
  await ev(`window.saturnus.store.set({ model: "48gx" })`);
  await expect(PAIRS["48gx"], PAIRS["48gx"], "nothing running, 48GX selected");
  // Running: the booted model, and each switch updates the logos.
  for (const m of ["48sx", "49g", "38g", "39g", "40g", "42s", "48gx"]) {
    await ev(`window.saturnus.store.set({ model: "${m}", booted: "${m}" })`);
    await expect(PAIRS[m], PAIRS[m], `${m} running`);
  }
  await ev(`window.saturnus.store.set({ model: null, booted: null })`);
  const heads = await until(ev, `(${LOGOS}).length === 3 && !document.documentElement.hasAttribute("data-logo") && (${LOGOS})`, 5_000, "no model");
  assert.deepEqual(heads.slice(0, 2), [look(PLAIN), look(PLAIN)], "no model: logo.svg's colours");

  // The colours from the skins: both shift keys, or the planet the one shift key's hue.
  for (const [model, pair] of Object.entries(PAIRS)) {
    const keys = await ev(`window.saturnus.backend.skin("${model}").then((s) => Object.fromEntries(s.keys.filter((k) => /shift$/.test(k.name)).map((k) => [k.name, k.fill])))`);
    if (keys.leftshift && keys.rightshift) assert.deepEqual(keys, { leftshift: pair.planet, rightshift: pair.ring }, model);
    else assert.ok(Math.abs(hue(Object.values(keys)[0]) - hue(pair.planet)) <= 2, `${model}: the planet in the shift key's hue`);
  }

  // A reload with the 49G remembered: its colours as the parser inserts the
  // panel's logo, before any module runs.
  await ev(`localStorage.setItem("saturnus.model", "49g")`);
  await send("Page.addScriptToEvaluateOnNewDocument", {
    source: `new MutationObserver((_, o) => {
      const p = document.querySelector(".panel-head svg.logo .planet");
      if (!p) return;
      window.__firstPlanet = getComputedStyle(p).fill;
      window.__early = !window.saturnus;
      o.disconnect();
    }).observe(document, { childList: true, subtree: true });`,
  });
  await send("Page.reload");
  await until(ev, "window.__firstPlanet", 15_000, "the reloaded page parsed");
  assert.equal(await ev("window.__early"), true, "measured before the app started");
  assert.equal(await ev("window.__firstPlanet"), rgb(PAIRS["49g"].planet));
});
