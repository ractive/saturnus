// The ways between the pages in headless Chrome (web/shared/site-links.js;
// the hrefs of each layout: web/test/site-links.test.mjs): the emulator's
// panel links to saturnus tx, its palette opens it, saturnus tx's header
// links to the emulator, and both brands to the landing page; served from
// web/ as here, the hrefs are web/'s and each one is a page that loads.
// In the desktop app (`window.__TAURI__`, simulated) the links stay
// hidden and the brands keep their own page. On the site, web/site.sh
// checks the hrefs the HTML carries (../transfer/, ../emulator/, ../).
// Skipped without Chrome or the wasm packages.
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

/** Each selector's link: its href attribute, whether it shows, and the status of fetching it. */
const LINKS = (sels) => `Promise.all(${JSON.stringify(sels)}.map(async (s) => {
  const a = document.querySelector(s);
  const r = a.getBoundingClientRect();
  return { href: a.getAttribute("href"), shown: !a.hidden && r.width > 0 && r.height > 0, status: (await fetch(a.href)).status };
}))`;

/** A fake desktop app's globals: enough for the page's modules to load (the emulator's backend never answers). */
const TAURI = `window.__TAURI__ = { core: { invoke: () => new Promise(() => {}) }, event: { listen: async () => () => {} } };`;

test("the emulator links to saturnus tx and the landing page; its palette opens saturnus tx; none in the desktop app", { timeout: 60_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started");
  const [transfer, brand] = await ev(LINKS(["#to-transfer", ".panel-head .brand"]));
  assert.deepEqual(transfer, { href: "tx.html", shown: true, status: 200 });
  assert.deepEqual(brand, { href: "landing/", shown: true, status: 200 });
  assert.equal(await ev(`document.querySelector("header.bar .brand").getAttribute("href")`), "landing/");
  const action = await ev(`(() => { const a = window.saturnus.palette.actions().find((x) => x.id === "open-tx"); return a && { title: a.title, off: a.off ?? null }; })()`);
  assert.deepEqual(action, { title: "Open saturnus tx", off: null });
  // On a phone the panel is a sheet under the bar; the link is in it.
  await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
  await ev(`document.getElementById("bar-menu").click(); true`);
  await until(ev, `document.body.classList.contains("sheet-open") && document.getElementById("to-transfer").getBoundingClientRect().height > 0`, 5_000, "the link in the sheet");

  // The desktop app: no link, no palette action, the brands their own page.
  await send("Page.addScriptToEvaluateOnNewDocument", { source: TAURI });
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  await until(ev, `!!window.__TAURI__ && document.readyState === "complete"`, 15_000, "the page loaded as the app");
  await until(ev, `document.getElementById("to-transfer").hidden && [...document.querySelectorAll(".brand")].every((b) => b.getAttribute("href") === "./")`, 5_000, "the app's links");
});

test("saturnus tx links to the emulator and the landing page; none in the desktop app", { timeout: 60_000 }, async (t) => {
  if (!txBuilt()) return t.skip("web/pkg or web/tx/pkg not built (just web tx-web)");
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  const p = txPage(c);
  for (const width of [1280, 390]) {
    await send("Emulation.setDeviceMetricsOverride", { width, height: width > 1000 ? 900 : 844, deviceScaleFactor: 1, mobile: width < 1000 });
    await p.open(port);
    const [emulator, brand] = await ev(LINKS(["#to-emulator", ".tx-bar .brand"]));
    assert.deepEqual(emulator, { href: "./", shown: true, status: 200 }, `${width} px`);
    assert.deepEqual(brand, { href: "landing/", shown: true, status: 200 }, `${width} px`);
    // The header keeps to one line: the link and Settings inside it.
    const fits = await ev(`(() => { const bar = document.querySelector(".tx-bar").getBoundingClientRect(); return ["#to-emulator", "#open-settings", ".tx-bar .brand"].every((s) => { const r = document.querySelector(s).getBoundingClientRect(); return r.top >= bar.top && r.bottom <= bar.bottom + 0.5 && r.right <= bar.right + 0.5; }); })()`);
    assert.equal(fits, true, `${width} px: the header's parts inside it`);
  }
  assert.deepEqual(c.errors, []);

  await send("Page.addScriptToEvaluateOnNewDocument", { source: TAURI });
  await p.open(port);
  assert.equal(await ev(`document.getElementById("to-emulator").hidden`), true);
  assert.equal(await ev(`document.querySelector(".tx-bar .brand").getAttribute("href")`), "./");
});
