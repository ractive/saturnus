// The logo in the model's colours on the real page in headless Chrome over
// the DevTools protocol: the side panel's and the phone bar's logo take the
// running model's colours, the selected model's with nothing running, the
// plain logo's with no model, and change when the model does; the case's
// logo takes its own model's; the remembered model's colours are there
// before the page's modules run (the first model's with none remembered,
// as the app shows it); the headers' colours on the light and the dark
// panel; and the colours come from the skins (the two shift keys where a
// model has two, else the shift key for the planet). No ROM: the running
// model is set in the store. Skipped without Chrome (`SATURNUS_CHROME`
// names one) or the wasm package (`just web`).
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

/**
 * style.css's table: the skin's colours (the case), and the headers' on
 * the light and the dark panel (a tint where the skin's is too faint
 * there); logo.svg's for no model.
 */
const PLAIN = { planet: "#d9874a", ring: "#3f9a8c" };
const PAIRS = {
  "48sx": { skin: { planet: "#e2893c", ring: "#8ebfd2" }, light: { planet: "#e18535", ring: "#5fa4bf" } },
  "48gx": { skin: { planet: "#8c86cc", ring: "#2ba6a2" } },
  "49g": { skin: { planet: "#4d59a3", ring: "#d5493c" }, dark: { planet: "#4f5ba6", ring: "#d5493c" } },
  "38g": { skin: { planet: "#a8d8ce", ring: "#cf8e78" }, light: { planet: "#4caa97", ring: "#ce8b74" } },
  "39g": { skin: { planet: "#c64d0a", ring: "#27467f" }, dark: { planet: "#c64d0a", ring: "#355fac" } },
  "40g": { skin: { planet: "#c64d0a", ring: "#27467f" }, dark: { planet: "#c64d0a", ring: "#355fac" } },
  "42s": { skin: { planet: "#ef8b1d", ring: "#89807a" }, light: { planet: "#e78110", ring: "#89807a" } },
};
/** The headers' colours of `model` in `theme`. */
const heads = (model, theme) => PAIRS[model][theme] ?? PAIRS[model].skin;

/** "rgb(r, g, b)" of "#rrggbb". */
const rgb = (hex) => `rgb(${[1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)).join(", ")})`;
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
  const plain = await until(ev, `(${LOGOS}).length === 3 && (${LOGOS})`, 5_000, "the logos drawn");
  assert.deepEqual(plain.slice(0, 2), [look(PLAIN), look(PLAIN)], "no model: logo.svg's colours");
  for (const theme of ["light", "dark"]) {
    await ev(`document.documentElement.setAttribute("data-theme", "${theme}")`);
    // Nothing running: the Model select's choice.
    await ev(`window.saturnus.store.set({ model: "48gx", booted: null })`);
    await expect(heads("48gx", theme), PAIRS["48gx"].skin, `${theme}: nothing running, 48GX selected`);
    // Running: the booted model, and each switch updates the logos; the case in the skin's colours.
    for (const m of ["48sx", "49g", "38g", "39g", "40g", "42s", "48gx"]) {
      await ev(`window.saturnus.store.set({ model: "${m}", booted: "${m}" })`);
      await expect(heads(m, theme), PAIRS[m].skin, `${theme}: ${m} running`);
    }
  }
  await ev(`document.documentElement.removeAttribute("data-theme")`);

  // The skins' colours: both shift keys, or the one shift key the planet.
  for (const [model, { skin }] of Object.entries(PAIRS)) {
    const keys = await ev(`window.saturnus.backend.skin("${model}").then((s) => Object.fromEntries(s.keys.filter((k) => /shift$/.test(k.name)).map((k) => [k.name, k.fill])))`);
    if (keys.leftshift && keys.rightshift) assert.deepEqual(keys, { leftshift: skin.planet, rightshift: skin.ring }, model);
    else assert.deepEqual(Object.values(keys), [skin.planet], `${model}: the planet is the shift key`);
  }

  // A reload: the colours of the model the page will show as the parser
  // inserts the panel's logo, before any module runs; the remembered 49G,
  // then with nothing remembered the first model (the 48SX).
  // A reload with the 49G remembered: its colours as the parser inserts the
  // panel's logo, before any module runs.
  await send("Page.addScriptToEvaluateOnNewDocument", {
    source: `new MutationObserver((_, o) => {
      const p = document.querySelector(".panel-head svg.logo .planet");
      if (!p) return;
      window.__firstPlanet = getComputedStyle(p).fill;
      window.__early = !window.saturnus;
      o.disconnect();
    }).observe(document, { childList: true, subtree: true });`,
  });
  for (const [stored, model] of [["49g", "49g"], [null, "48sx"]]) {
    await ev(stored ? `localStorage.setItem("saturnus.model", "${stored}")` : `localStorage.clear()`);
    await send("Page.reload");
    await until(ev, "window.__firstPlanet", 15_000, "the reloaded page parsed");
    assert.equal(await ev("window.__early"), true, "measured before the app started");
    assert.equal(await ev("window.__firstPlanet"), rgb(heads(model, "light").planet), `stored ${stored}`);
    // The app shows that model, so the colour stays.
    await until(ev, "window.saturnus?.store.state.model", 15_000, "the app started");
    assert.equal(await ev("window.saturnus.store.state.model"), model);
    assert.equal(await ev(`getComputedStyle(document.querySelector(".panel-head svg.logo .planet")).fill`), rgb(heads(model, "light").planet));
  }
});
