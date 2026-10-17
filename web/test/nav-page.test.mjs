// The ways between the pages in headless Chrome (web/shared/site-links.js;
// the hrefs of each layout: web/test/site-links.test.mjs): the emulator's
// panel links to saturnus tx, its palette opens it, saturnus tx's header
// links to the emulator, and both brands to the landing page; served from
// web/ as here, the hrefs are web/'s and each one is a page that loads.
// In the desktop app (`window.__TAURI__`, simulated) the links stay
// hidden and the brands keep their own page. On the site, web/site.sh
// checks the hrefs the HTML carries (../transfer/, ../emulator/, ../).
// And leaving saturnus tx mid-transfer asks first (beforeunload).
// Skipped without Chrome or the wasm package.
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
  assert.equal(await ev(`document.getElementById("to-transfer").textContent.trim()`), "Files", "the navigation names: Calculator and Files");
  // No calculator running: the plain link, to saturnus tx's own tab.
  assert.equal(await ev(`document.getElementById("to-transfer").target`), "saturnus-tx");
  assert.equal(await ev("window.name"), "saturnus-emulator", "this tab named, for saturnus tx's links");
  assert.deepEqual(brand, { href: "landing/", shown: true, status: 200 });
  assert.equal(await ev(`document.querySelector("header.bar .brand").getAttribute("href")`), "landing/");
  const action = await ev(`(() => { const a = window.saturnus.palette.actions().find((x) => x.id === "open-tx"); return a && { title: a.title, off: a.off ?? null }; })()`);
  assert.deepEqual(action, { title: "Open saturnus tx", off: null });
  // On a phone the panel is a sheet under the bar; the link is in it.
  await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
  await ev(`document.getElementById("bar-menu").click(); true`);
  await until(ev, `document.getElementById("calculator-view").classList.contains("sheet-open") && document.getElementById("to-transfer").getBoundingClientRect().height > 0`, 5_000, "the link in the sheet");

  // The desktop app: no link, no palette action, the brands their own page.
  await send("Page.addScriptToEvaluateOnNewDocument", { source: TAURI });
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  await until(ev, `!!window.__TAURI__ && document.readyState === "complete"`, 15_000, "the page loaded as the app");
  await until(ev, `document.getElementById("to-transfer").hidden && [...document.querySelectorAll(".brand")].every((b) => b.getAttribute("href") === "./")`, 5_000, "the app's links");
});

test("saturnus tx links to the emulator and the landing page; none in the desktop app", { timeout: 60_000 }, async (t) => {
  if (!txBuilt()) return t.skip("web/pkg not built (just web)");
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
    assert.equal(await ev(`document.getElementById("to-emulator").textContent.trim()`), "Calculator");
    assert.deepEqual(brand, { href: "landing/", shown: true, status: 200 }, `${width} px`);
    // The header keeps to one line: the link and Settings inside it.
    const fits = await ev(`(() => { const bar = document.querySelector(".tx-bar").getBoundingClientRect(); return ["#to-emulator", "#open-settings", ".tx-bar .brand"].every((s) => { const r = document.querySelector(s).getBoundingClientRect(); return r.top >= bar.top && r.bottom <= bar.bottom + 0.5 && r.right <= bar.right + 0.5; }); })()`);
    assert.equal(fits, true, `${width} px: the header's parts inside it`);
  }
  assert.equal(await ev(`document.getElementById("to-emulator").target`), "saturnus-emulator");
  assert.equal(await ev("window.name"), "saturnus-tx", "this tab named, for the emulator's links");
  assert.deepEqual(c.errors, []);

  // ?connect= with no ROM kept for that model: said once, taken off the address, the left pane as it was.
  await p.open(port, "?connect=48gx");
  await p.until(`[...document.querySelectorAll(".toast")].some((t) => /keeps no HP 48GX ROM/.test(t.textContent))`, 10_000, "the missing ROM said");
  assert.equal(await ev("location.search"), "", "?connect= taken off the address");
  assert.equal(await ev("tx.panes[0].place?.kind ?? null"), null, "the left pane as it was");

  // The emulator's calculator that cannot be connected (?refused=&why=, shared/site-links.js connectRefusal): said here, taken off the address.
  await p.open(port, "?refused=38g&why=aplet");
  await p.until(`[...document.querySelectorAll(".toast")].some((t) => t.textContent.includes("The emulator's HP 38G is not connected here: it sends and receives files itself"))`, 10_000, "the reason said");
  assert.equal(await ev("location.search"), "", "?refused= and ?why= taken off the address");
  assert.equal(await ev("tx.panes[0].place?.kind ?? null"), null, "the left pane as it was");

  await send("Page.addScriptToEvaluateOnNewDocument", { source: TAURI });
  await p.open(port);
  assert.equal(await ev(`document.getElementById("to-emulator").hidden`), true);
  assert.equal(await ev(`document.querySelector(".tx-bar .brand").getAttribute("href")`), "./");
});

test("saturnus tx asks before the page is left mid-transfer or with a calculator on a cable, not when idle", { timeout: 60_000 }, async (t) => {
  if (!txBuilt()) return t.skip("web/pkg not built (just web)");
  const c = await session(t);
  if (!c) return;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  const p = txPage(c);
  await p.open(c.port);
  // Whether a beforeunload was cancelled (the browser's "Leave site?").
  const asks = `(() => { const e = new Event("beforeunload", { cancelable: true }); window.dispatchEvent(e); return e.defaultPrevented; })()`;
  assert.equal(await c.ev(asks), false, "idle: no question");
  await c.ev(`tx.queue.add({ label: "Copy X", places: ["x"], run: () => new Promise((done) => { window.__done = done; }) }).catch(() => {}); tx.queue.add({ label: "Copy Y", places: ["x"], run: () => new Promise(() => {}) }).catch(() => {}); true`);
  await p.until(`tx.queue.running.length === 1 && tx.queue.waiting.length === 1`, 5_000, "one running, one waiting");
  assert.equal(await c.ev(asks), true, "a job running: the question");
  await c.ev(`tx.queue.cancelWaiting(); window.__done(); true`);
  await p.until(`tx.queue.running.length + tx.queue.waiting.length === 0`, 5_000, "the queue empty");
  assert.equal(await c.ev(asks), false, "idle again: no question");
  // A calculator on a cable (its port and server mode are not got back by a reload) asks; an emulated one does not, unless it works.
  const place = (p) => c.ev(`(() => { tx.panes[0].place = ${p}; return true; })()`);
  await place(`{ kind: "device", emulated: false, device: { connection: { model: "48gx" }, running: null } }`);
  assert.equal(await c.ev(asks), true, "a calculator on a cable: the question");
  await place(`{ kind: "device", emulated: true, hub: true, device: { connection: { model: "48gx" }, running: null } }`);
  assert.equal(await c.ev(asks), false, "the emulator's calculator, idle: no question");
  await place(`{ kind: "device", emulated: true, hub: true, device: { connection: { model: "48gx" }, running: { label: "Reading HOME" } } }`);
  assert.equal(await c.ev(asks), true, "a calculator working: the question");
  await place("null");
});
