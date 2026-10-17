// The ways through the app in headless Chrome (iteration 50d: one
// document, routes in the hash, web/main.js and web/router.js): the home
// page with the kept calculators, Calculator's panel link and palette
// command to Files, Files' header link to Calculator, both brands home,
// the home page's link to the landing page; switching views never loads
// the page again and never stops the calculator; back and forward follow
// the routes. In the desktop app (`window.__TAURI__`, simulated) Files
// links to Calculator too (one app, iteration 51), there is no home page
// and the window opens the calculator. And leaving mid-transfer asks first (beforeunload).
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

/** A fake desktop app's globals: enough for the page's modules to load (the emulator's backend never answers). */
const TAURI = `window.__TAURI__ = { core: { invoke: () => new Promise(() => {}) }, event: { listen: async () => () => {} } };`;

test("Calculator links to Files, its palette opens Files, both brands go home; no home page in the desktop app", { timeout: 60_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started");
  const link = await ev(`(() => { const a = document.getElementById("to-transfer"); const r = a.getBoundingClientRect(); return { href: a.getAttribute("href"), shown: !a.hidden && r.width > 0, target: a.target, text: a.textContent.trim() }; })()`);
  // No calculator running: Files as it is, in this document.
  assert.deepEqual(link, { href: "#/files", shown: true, target: "", text: "Files" });
  assert.deepEqual(await ev(`[...document.querySelectorAll("#calculator-view .brand")].map((b) => b.getAttribute("href"))`), ["#/", "#/"]);
  const action = await ev(`(() => { const a = window.saturnus.palette.actions().find((x) => x.id === "open-tx"); return a && { title: a.title, off: a.off ?? null }; })()`);
  assert.deepEqual(action, { title: "Open Files", off: null });
  // On a phone the panel is a sheet under the bar; the link is in it.
  await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
  await ev(`document.getElementById("bar-menu").click(); true`);
  await until(ev, `document.getElementById("calculator-view").classList.contains("sheet-open") && document.getElementById("to-transfer").getBoundingClientRect().height > 0`, 5_000, "the link in the sheet");
  // The palette's command shows Files, in the same document.
  await ev(`window.__same = true; window.saturnus.palette.actions().find((x) => x.id === "open-tx").run(); true`);
  await until(ev, `saturnusApp.view === "files" && !!window.tx`, 15_000, "Files shown");
  assert.equal(await ev("location.hash"), "#/files");
  assert.equal(await ev("window.__same"), true, "not loaded again");
  assert.equal(await ev(`document.getElementById("calculator-view").hidden`), true, "Calculator hidden, not removed");
  assert.deepEqual(c.errors, []);

  // The desktop app: no home page, the brand stays on the calculator.
  await send("Page.addScriptToEvaluateOnNewDocument", { source: TAURI });
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  await until(ev, `!!window.__TAURI__ && document.readyState === "complete" && window.saturnusApp?.view === "calculator"`, 15_000, "the page loaded as the app, on the calculator");
  assert.equal(await ev("location.hash"), "#/calculator");
  assert.equal(await ev(`document.querySelector(".home-landing").hidden`), true);
  await ev(`location.hash = "#/"; true`);
  await until(ev, `location.hash === "#/calculator"`, 5_000, "the brand's home is the calculator");
  assert.equal(await ev("saturnusApp.view"), "calculator");
});

test("Files links to Calculator, its brand goes home; #/files/<model> says why it cannot connect", { timeout: 60_000 }, async (t) => {
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
    const link = await ev(`(() => { const a = document.getElementById("to-emulator"); const r = a.getBoundingClientRect(); return { href: a.getAttribute("href"), shown: !a.hidden && r.width > 0, text: a.textContent.trim() }; })()`);
    assert.deepEqual(link, { href: "#/calculator", shown: true, text: "Calculator" }, `${width} px`);
    assert.equal(await ev(`document.querySelector(".tx-bar .brand").getAttribute("href")`), "#/");
    // The header keeps to one line: the link and Settings inside it.
    const fits = await ev(`(() => { const bar = document.querySelector(".tx-bar").getBoundingClientRect(); return ["#to-emulator", "#open-settings", ".tx-bar .brand"].every((s) => { const r = document.querySelector(s).getBoundingClientRect(); return r.top >= bar.top && r.bottom <= bar.bottom + 0.5 && r.right <= bar.right + 0.5; }); })()`);
    assert.equal(fits, true, `${width} px: the header's parts inside it`);
  }
  assert.deepEqual(c.errors, []);

  // #/files/48gx with no ROM kept for that model: said, the left pane as it was.
  await p.open(port, "/48gx");
  await p.until(`[...document.querySelectorAll(".toast")].some((t) => /Files reaches only a calculator whose ROM this browser keeps/.test(t.textContent))`, 10_000, "the missing ROM said");
  assert.equal(await ev("tx.panes[0].place?.kind ?? null"), null, "the left pane as it was");
  // A model Files cannot connect: said why.
  await p.open(port, "/38g");
  await p.until(`[...document.querySelectorAll(".toast")].some((t) => t.textContent.includes("The HP 38G is not connected here: it sends and receives files itself"))`, 10_000, "the reason said");
  assert.equal(await ev("tx.panes[0].place?.kind ?? null"), null, "the left pane as it was");

  // The desktop app links its views too (one app, iteration 51).
  await send("Page.addScriptToEvaluateOnNewDocument", { source: TAURI });
  await p.open(port);
  assert.equal(await ev(`document.getElementById("to-emulator").hidden`), false);
});

test("the home page: the kept calculators with Calculator and Files, the running one marked; views switch without a reload", { timeout: 120_000 }, async (t) => {
  if (!txBuilt()) return t.skip("web/pkg not built (just web)");
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  await until(ev, `window.saturnusApp?.view === "home" && !!document.getElementById("home-view").dataset.rendered`, 15_000, "the home page");
  assert.equal(await ev("location.hash"), "#/", "no route: the home page");
  assert.equal(await ev(`document.getElementById("home-empty").hidden`), false, "nothing kept yet");
  assert.equal(await ev("!!window.saturnus || !!window.tx"), false, "no view mounted before it is visited");
  assert.equal(await ev(`document.querySelector(".home-landing").getAttribute("href")`), "landing/");
  // Two kept ROMs, as the ROM store keeps them (web/romstore.js's slots).
  await ev(`new Promise((resolve, reject) => {
    const req = indexedDB.open("saturnus-roms", 1);
    req.onupgradeneeded = () => { req.result.createObjectStore("slots"); req.result.createObjectStore("images"); };
    req.onsuccess = () => {
      const tx = req.result.transaction("slots", "readwrite");
      tx.objectStore("slots").put({ name: "gxrom-r", sha256: "aa" }, "48gx");
      tx.objectStore("slots").put({ name: "42s.rom", sha256: "bb" }, "42s");
      tx.oncomplete = () => { req.result.close(); resolve(true); };
      tx.onerror = () => reject(tx.error);
    };
    req.onerror = () => reject(req.error);
  })`);
  await ev(`saturnusApp.home.render().then(() => true)`);
  const rows = await ev(`[...document.querySelectorAll("#home-list .home-calc")].map((li) => ({ model: li.dataset.model, title: li.querySelector(".home-title").textContent, rom: li.querySelector(".home-rom").textContent, go: [...li.querySelectorAll("a")].map((a) => [a.textContent.trim(), a.getAttribute("href")]) }))`);
  assert.deepEqual(rows, [
    { model: "48gx", title: "HP 48GX", rom: "gxrom-r", go: [["Files", "#/files/48gx"], ["Calculator", "#/calculator/48gx"]] },
    { model: "42s", title: "HP 42S", rom: "42s.rom", go: [["Calculator", "#/calculator/42s"]] },
  ], "in the page's model order; Files only where it can reach the calculator");
  // The running marker comes from the hub's `engines`.
  await ev(`window.__same = true; true`);
  await ev(`location.hash = "#/calculator"; true`);
  await until(ev, "!!window.saturnus", 15_000, "Calculator mounted");
  await ev("window.saturnus.started");
  await ev(`history.back(); true`);
  await until(ev, `saturnusApp.view === "home"`, 5_000, "back: home");
  assert.equal(await ev(`document.getElementById("calculator-view").hidden`), true);
  assert.equal(await ev(`saturnusApp.home.engines().then((e) => JSON.stringify(e))`), "[]", "the hub runs, no calculator yet");
  await ev(`history.forward(); true`);
  await until(ev, `saturnusApp.view === "calculator"`, 5_000, "forward: Calculator");
  assert.equal(await ev("window.__same"), true, "never loaded again");
  // New calculator: Calculator with the ROMs of every model open.
  await ev(`location.hash = "#/"; true`);
  await until(ev, `saturnusApp.view === "home"`, 5_000, "home");
  await ev(`document.getElementById("home-new").click(); true`);
  await until(ev, `saturnusApp.view === "calculator" && document.getElementById("roms").open`, 5_000, "the ROMs open");
  assert.equal(await ev("location.hash"), "#/calculator");
  assert.deepEqual(c.errors, []);
});

test("Files asks before the page is left mid-transfer or with a calculator on a cable, not when idle", { timeout: 60_000 }, async (t) => {
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
