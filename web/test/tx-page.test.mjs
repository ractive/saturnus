// The transfer page (web/tx.html) in headless Chrome, without a ROM: a
// FakeLink plays the emulated 48GX of the recorded page session
// (web/test/fixtures/tx-page.trace, `just tx-page-trace`), and the page
// connects, lists HOME, puts a text file, a plain file as a string after
// asking, the text file again after "Replace PRG?", previews NOTE, gets
// PRG as a download and disconnects, writing exactly the recorded bytes.
// Then no horizontal overflow at 360, 390, 430, 768 and 1280 px in both
// themes, on the start card, the connected pane with the preview, a
// row's menu and each dialog; and a browser without Web Serial gets the
// plain message and still the emulated calculator. Skipped without
// Chrome or the wasm packages (`just web tx-web`; `SATURNUS_AUDIT=1`
// makes the skip a failure).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { session, sleep } from "./chrome.mjs";
import { PAGE_FILES, PAGE_SEED, pageSession, txBuilt, txPage } from "./tx-chrome.mjs";

const TRACE = readFileSync(new URL("./fixtures/tx-page.trace", import.meta.url), "utf8");
const WIDTHS = [360, 390, 430, 768, 1280];
const HEIGHTS = { 360: 780, 390: 844, 430: 932, 768: 1024, 1280: 900 };
/* Containers that may scroll sideways by design: the list (wide names), the tree, a matrix, a picture. */
const MAY_SCROLL = [".dlist-wrap", ".dtree", "device-tree", ".grid-wrap", ".graphic-wrap"];

const MEASURE = `(() => {
  const de = document.documentElement;
  const sel = (e) => e.tagName.toLowerCase() + (e.id ? "#" + e.id : "") + (typeof e.className === "string" && e.className ? "." + e.className.trim().split(/\\s+/).join(".") : "");
  const visible = (e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== "hidden"; };
  const scrollers = [];
  const outside = [];
  for (const e of document.querySelectorAll("body *")) {
    if (!visible(e) || e.closest("svg")) continue;
    if (/auto|scroll/.test(getComputedStyle(e).overflowX) && e.scrollWidth > e.clientWidth + 1 && !e.matches(${JSON.stringify(MAY_SCROLL.join(","))})) scrollers.push(sel(e) + " +" + (e.scrollWidth - e.clientWidth));
  }
  for (const e of document.querySelectorAll("dialog[open], .menu")) {
    const r = e.getBoundingClientRect();
    if (r.left < -0.5 || r.right > innerWidth + 0.5) outside.push(sel(e));
  }
  return { page: de.scrollWidth - de.clientWidth, inner: innerWidth, scrollers, outside };
})()`;

/** A Chrome session for the page, or null (skipped). */
async function open(t) {
  if (!txBuilt()) {
    if (process.env.SATURNUS_AUDIT) assert.fail("web/pkg or web/tx/pkg not built (just web tx-web)");
    t.skip("web/pkg or web/tx/pkg not built (just web tx-web)");
    return null;
  }
  return session(t, { audit: true });
}

test("the page with a recorded calculator: connect, list, put, replace, preview, get, disconnect", { timeout: 180_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  const p = txPage(c);
  await p.open(c.port);
  assert.equal(await c.ev(`document.getElementById("pane-title").textContent`), "No calculator");
  await c.ev(`window.__link = new tx.links.FakeLink(${JSON.stringify(TRACE)}); tx.state.deviceOptions = { seed: ${PAGE_SEED} }; tx.connectLink(__link, { model: "48gx" }); true`);
  await p.until("!!tx.state.device?.connection", 60_000, "connected");
  assert.equal(await c.ev(`document.getElementById("pane-title").textContent`), "HP 48G/GX on test link");
  // The live region hears steps, not packets: progress goes beside it.
  await c.ev(`(() => {
    window.__live = 0; window.__progress = 0;
    new MutationObserver(() => __live++).observe(document.getElementById("pane-status"), { childList: true, characterData: true, subtree: true });
    new MutationObserver(() => __progress++).observe(document.getElementById("pane-progress"), { childList: true, characterData: true, subtree: true });
    return true;
  })()`);
  const seen = await pageSession(c);
  const live = await c.ev(`({ live: __live, progress: __progress, outside: document.getElementById("pane-progress").closest("[role=status]") === null })`);
  assert.ok(live.outside, "the progress is not in the live region");
  assert.ok(live.progress > 0, "the progress was shown");
  assert.ok(live.live < 40, `the live region changed ${live.live} times in the session`);
  assert.deepEqual(seen.home, ["IOPAR"]);
  assert.deepEqual([...seen.after].sort(), ["IOPAR", "NOTE", "PRG"]);
  assert.equal(seen.preview, PAGE_FILES.note);
  assert.equal(seen.download.name, "PRG.hp");
  assert.equal(String.fromCharCode(...seen.download.bytes.slice(0, 7)), "HPHP48-");
  assert.equal(seen.note, "The calculator has left server mode.");
  assert.equal(await c.ev("__link.failure"), null);
  assert.equal(await c.ev("__link.next === __link.steps.length"), true, "every recorded step was played");
  assert.deepEqual(c.errors, []);
});

test("no horizontal overflow at five widths in both themes; dialogs and menus inside the window", { timeout: 300_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  const { send, ev, settled } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  const metrics = (width, height) => send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 2, mobile: width < 1000 });
  await metrics(1280, 900);
  const p = txPage(c);
  await p.open(c.port);
  // A connected pane with a long name, a directory and the preview, shown
  // from a fake device (the Device interface, web/shared/device.js).
  await ev(`(() => {
    const entries = [
      { name: "AVERYLONGVARIABLENAMEINDEED", kind: "Program", size: 1204.5, checksum: 0xbeef, directory: false },
      { name: "NOTES", kind: "String", size: 30.5, checksum: 0x1234, directory: false },
      { name: "GAMES", kind: "Directory", size: 300, checksum: 0x4321, directory: true },
      { name: "PIC", kind: "Graphic", size: 1103.5, checksum: 0x0f0f, directory: false },
    ];
    window.__fake = Object.assign(new EventTarget(), {
      connection: { model: "48gx", settings: { baud: 9600, parity: "none", dataBits: 8 } },
      running: null, queued: 0,
      list: async (path) => ({ path, entries: path.length > 1 ? [] : entries }),
      read: async (path, name, { text } = {}) => ({ name, fileName: name + (text ? ".txt" : ".hp"), format: text ? "text" : "binary", data: new TextEncoder().encode('%%HP: T(3)A(R)F(.);\\r\\n"a string long enough to wrap in the preview of a narrow phone, with-a-very-long-unbroken-token-in-it-that-goes-on-and-on"') }),
      close: async () => {},
      cancelQueued: () => 0,
      refresh: () => {},
    });
    tx.state.device = __fake;
    tx.state.place = { model: "48gx", emulated: false, linkLabel: "USB serial port with a long name" };
    tx.ui.tree.device = __fake;
    tx.ui.list.device = __fake;
    tx.navigate(["HOME"]);
    return true;
  })()`);
  const connected = async () => {
    await ev(`(() => { tx.state.device = __fake; document.getElementById("pane-empty").hidden = true; document.getElementById("pane-body").hidden = false; document.getElementById("put-file").hidden = false; document.getElementById("refresh").hidden = false; document.getElementById("pane-more").hidden = false; document.getElementById("pane-title").textContent = "HP 48G/GX on USB serial port with a long name"; document.getElementById("pane-status").textContent = "Getting AVERYLONGVARIABLENAMEINDEED… 1.2 KB of 3.4 KB (2 more waiting)"; return true; })()`);
    await p.until(`!!document.querySelector("device-list tr[data-name]")`, 10_000, "the list");
  };
  const reset = () => ev(`(() => {
    for (const d of document.querySelectorAll("dialog[open]")) d.close();
    document.querySelector(".menu")?.remove();
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    return true;
  })()`);
  const VIEWS = {
    start: () => ev(`(() => { tx.state.device = null; tx.render(); document.getElementById("pane-empty").hidden = false; document.getElementById("pane-body").hidden = true; document.getElementById("last-note").hidden = false; document.getElementById("last-note").textContent = "The calculator has left server mode. If its server was started in algebraic mode, the 49G has now put what was on its stack into one list."; return true; })()`),
    list: connected,
    preview: async () => {
      await connected();
      await ev(`tx.select("AVERYLONGVARIABLENAMEINDEED")`);
      await p.until(`!!document.querySelector("#preview pre.obj")`, 10_000, "the preview");
    },
    menu: async () => {
      await connected();
      await ev(`document.querySelector('device-list tr[data-name="NOTES"] button.more').click()`);
      await p.until(`!!document.querySelector(".menu")`, 5_000, "the row's menu");
    },
    connect: () => ev(`(() => { tx.state.device = null; tx.render(); document.getElementById("add-serial").click(); return true; })()`).then(() => p.until(`document.getElementById("connect-dialog").open`, 5_000, "the connect dialog")),
    emulated: () => ev(`(() => { tx.state.device = null; tx.render(); document.getElementById("add-emulated").click(); return true; })()`).then(() => p.until(`document.getElementById("emulated-dialog").open`, 5_000, "the emulated dialog")),
    settings: () => ev(`(() => { document.getElementById("open-settings").click(); return true; })()`).then(() => p.until(`document.getElementById("settings-dialog").open`, 5_000, "the settings")),
    question: () => ev(`(() => { import("./shared/components/confirm.js").then((m) => m.confirmAction({ title: "Replace AVERYLONGVARIABLENAMEINDEED?", body: "The calculator already has a variable named AVERYLONGVARIABLENAMEINDEED here.", action: "Replace" })); return true; })()`).then(() => p.until(`!!document.querySelector("dialog.confirm[open]")`, 5_000, "the question")),
  };
  const failures = [];
  for (const w of WIDTHS) {
    for (const scheme of ["light", "dark"]) {
      await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
      for (const [view, show] of Object.entries(VIEWS)) {
        await metrics(w, HEIGHTS[w]);
        await reset();
        await show();
        await sleep(150);
        const m = await settled(MEASURE);
        if (m.page > 0) failures.push(`${view} at ${w}px ${scheme}: the page is ${m.page}px too wide (innerWidth ${m.inner})`);
        for (const s of m.scrollers) failures.push(`${view} at ${w}px ${scheme}: ${s} scrolls sideways`);
        for (const s of m.outside) failures.push(`${view} at ${w}px ${scheme}: ${s} is outside the window`);
      }
    }
  }
  assert.deepEqual(failures, []);
  // The themes are the tokens': dark when asked, light otherwise.
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "dark" }] });
  assert.equal(await ev(`getComputedStyle(document.body).backgroundColor`), "rgb(28, 28, 27)");
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
  assert.equal(await ev(`getComputedStyle(document.body).backgroundColor`), "rgb(218, 215, 206)");
  assert.deepEqual(c.errors, []);
});

test("a browser without Web Serial says so plainly and still offers the emulated calculator", { timeout: 60_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Page.addScriptToEvaluateOnNewDocument", { source: "Object.defineProperty(Navigator.prototype, 'serial', { get: () => undefined, configurable: true });" });
  const p = txPage(c);
  await p.open(c.port);
  const s = await c.ev(`({
    serial: !!navigator.serial,
    note: document.getElementById("no-serial").checkVisibility() ? document.getElementById("no-serial").textContent : null,
    add: document.getElementById("add-serial").checkVisibility(),
    emulated: document.getElementById("add-emulated").checkVisibility(),
    primary: document.getElementById("add-emulated").classList.contains("primary"),
  })`);
  assert.equal(s.serial, false);
  assert.match(s.note, /cannot reach a serial port.*Chrome or Edge.*emulated calculator works here/);
  assert.equal(s.add, false);
  assert.equal(s.emulated, true);
  assert.equal(s.primary, true);
  // The emulated calculator's dialog opens (no ROM kept here: it says so).
  await c.ev(`document.getElementById("add-emulated").click(); true`);
  await p.until(`document.getElementById("emulated-dialog").open`, 5_000, "the emulated dialog");
  assert.equal(await c.ev(`document.getElementById("emulated-none").hidden`), false);
  // Reading the kept ROMs created no database.
  assert.deepEqual(await c.ev(`indexedDB.databases().then((d) => d.map((x) => x.name))`), []);
  assert.deepEqual(c.errors, []);
});

// ---------------------------------------------------------------- the review's cases (PR 130)

/**
 * A stand-in for the protocol Worker in the page (`tx.state.deviceOptions.worker`):
 * `window.__answer(request, worker)` gives the messages a request ends
 * with, after `window.__delay(request)` ms; every worker made is in
 * `__workers`. And `__stubLink(writeMs)`, a link that takes everything.
 */
const FAKES = `(() => {
  window.__workers = [];
  window.__requests = [];
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const conn = { model: "48gx", settings: { baud: 9600, parity: "none", dataBits: 8 }, version: null, userMode: "binary" };
  const entries = ["A", "B", "C", "D"].map((name) => ({ name, kind: "Program", size: 20, checksum: name.charCodeAt(0), directory: false }));
  window.__delay = (req) => (req.type === "get" ? 300 : 20);
  window.__answer = async (req, w) => {
    __requests.push(req);
    await sleep(__delay(req));
    const done = (outcome) => [{ type: "connection", connection: conn }, { type: "result", ok: true, outcome }];
    if (req.type === "connect") return [{ type: "write", bytes: new Uint8Array([1]) }, ...done({ type: "connected", connection: conn })];
    if (req.type === "list") return done({ type: "listing", path: req.dir, entries });
    if (req.type === "get") return [{ type: "connection", connection: conn }, { type: "result", ok: true, outcome: { type: "file", name: req.name, fileName: req.name + ".txt", format: req.format }, data: new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n\\\\<< " + req.name + " \\\\>>") }];
    if (req.type === "put") return done({ type: "stored", name: req.name });
    return done({ type: "disconnected" });
  };
  window.__fakeWorker = () => {
    const w = { onmessage: null, terminated: false, postMessage(m) {
      setTimeout(async () => {
        if (w.terminated) return;
        if (m.cmd === "init") { w.onmessage({ data: { type: "ready" } }); return; }
        if (m.cmd !== "start") return;
        const out = await __answer(m.request, w);
        if (!out) return;
        for (const o of out) if (!w.terminated) w.onmessage({ data: { ...o, id: m.id } });
      }, 0);
    }, terminate() { w.terminated = true; } };
    __workers.push(w);
    return w;
  };
  window.__stubLink = (writeMs = 0) => Object.assign(new EventTarget(), {
    label: "stub link", closed: false,
    open: async () => {}, write: async () => { await sleep(writeMs); }, close: async function () { this.closed = true; },
  });
  tx.state.deviceOptions = { worker: __fakeWorker };
  return true;
})()`;

async function fakePage(t, width = 1280) {
  const c = await open(t);
  if (!c) return null;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width, height: width < 1000 ? 844 : 900, deviceScaleFactor: 1, mobile: width < 1000 });
  const p = txPage(c);
  await p.open(c.port);
  await c.ev(FAKES);
  return { c, p };
}

test("one calculator at a time: the add buttons are off while one connects, and a newer connect wins over a stale one", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  // The first connect's answer arrives, but its link's write holds it back 1.5 s.
  await c.ev(`window.__first = __stubLink(1500); window.__a = tx.connectLink(__first, { model: "48gx" }); true`);
  await p.until(`document.getElementById("pane-title").textContent !== "No calculator"`, 5_000, "connecting");
  const off = await c.ev(`["add-serial", "add-emulated", "connect-go", "emulated-file"].map((id) => document.getElementById(id).disabled)`);
  assert.deepEqual(off, [true, true, true, true], "the ways to add another are off");
  await c.ev(`window.__firstDevice = tx.state.device; true`);
  // A second connect meanwhile (as a script could): it closes the first and wins.
  await c.ev(`window.__second = __stubLink(0); window.__b = tx.connectLink(__second, { model: "49g" }); true`);
  await p.until("tx.idle()", 10_000, "the second connected and listed");
  await c.ev(`__a.then((d) => { window.__aResult = d; }); true`);
  await p.until("window.__aResult !== undefined", 5_000, "the first connect settled");
  const s = await c.ev(`({
    a: __aResult, firstClosed: __first.closed, secondClosed: __second.closed,
    device: tx.state.device !== __firstDevice, list: tx.ui.list.device === tx.state.device, tree: tx.ui.tree.device === tx.state.device,
    path: tx.state.path, title: document.getElementById("pane-title").textContent, terminated: __workers[0].terminated,
  })`);
  assert.equal(s.a, null, "the stale connect gives nothing");
  assert.equal(s.firstClosed, true, "its link was let go");
  assert.equal(s.terminated, true, "and its Worker");
  assert.equal(s.secondClosed, false);
  assert.ok(s.device && s.list && s.tree, "the tree and the list show the newer device");
  assert.deepEqual(s.path, ["HOME"]);
  assert.equal(s.title, "HP 48G/GX on stub link");
  // Connected: the add buttons stay off until it is let go.
  assert.equal(await c.ev(`document.getElementById("add-emulated").disabled`), true);
  await c.ev("tx.disconnect()");
  await p.until("!tx.state.device", 5_000, "disconnected");
  assert.equal(await c.ev(`document.getElementById("add-emulated").disabled`), false);
  assert.deepEqual(c.errors, []);
});

test("the name dialog: Enter puts, a refused name asks again, Escape cancels", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t, 390);
  if (!f) return;
  const { c, p } = f;
  const key = async (k, code) => {
    await c.send("Input.dispatchKeyEvent", { type: "keyDown", key: k, code, windowsVirtualKeyCode: k === "Enter" ? 13 : 27, ...(k === "Enter" ? { text: "\r" } : {}) });
    await c.send("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: k === "Enter" ? 13 : 27 });
  };
  const type = (text) => c.ev(`(() => { const i = document.getElementById("name-input"); i.focus(); i.value = ${JSON.stringify(text)}; return true; })()`);
  await c.ev(`tx.connectLink(__stubLink(0), { model: "48gx" }); true`);
  await p.idle(10_000);
  const putBad = `window.__put = false; tx.putFiles([new File(["%%HP: T(3)A(R)F(.);\\r\\n1\\r\\n"], "1BAD.txt")]).then(() => { __put = true; }); true`;
  await c.ev(putBad);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name dialog");
  await type("2X");
  await key("Enter", "Enter");
  await p.until(`document.getElementById("name-problem").textContent !== ""`, 5_000, "the refusal");
  assert.equal(await c.ev(`document.getElementById("name-dialog").open`), true, "a refused name asks again");
  await key("Escape", "Escape");
  await p.until("__put", 5_000, "Escape ends the put");
  assert.equal(await c.ev(`document.getElementById("name-dialog").open`), false);
  assert.equal(await c.ev(`__requests.filter((r) => r.type === "put").length`), 0, "nothing put after Escape");
  // Cancel the same.
  await c.ev(putBad);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name dialog again");
  await c.ev(`document.getElementById("name-cancel").click(); true`);
  await p.until("__put", 5_000, "Cancel ends the put");
  assert.equal(await c.ev(`__requests.filter((r) => r.type === "put").length`), 0, "nothing put after Cancel");
  // Enter with a good name puts it.
  await c.ev(putBad);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name dialog a third time");
  await type("GOOD");
  await key("Enter", "Enter");
  await p.until("__put", 10_000, "the put");
  assert.deepEqual(await c.ev(`__requests.filter((r) => r.type === "put").map((r) => r.name)`), ["GOOD"]);
  // Its toast at the window's bottom, clear of the pane's head.
  await p.until(`!!document.querySelector(".toast")`, 5_000, "the toast");
  const r = await c.ev(`(() => { const t = document.querySelector(".toast").getBoundingClientRect(); const h = document.querySelector(".pane-head").getBoundingClientRect(); return { top: t.top, bottom: t.bottom, head: h.bottom, inner: innerHeight, text: document.querySelector(".toast").textContent }; })()`);
  assert.match(r.text, /is now GOOD/);
  assert.ok(r.top > r.head, `below the pane's head (${r.top} > ${r.head})`);
  assert.ok(r.bottom <= r.inner && r.bottom > r.inner - 80, `at the bottom of the window (${r.bottom} of ${r.inner})`);
  assert.deepEqual(c.errors, []);
});

test("previews: one waits at a time, a selection that moved on drops it, and a dropped one is not kept as an error", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`tx.connectLink(__stubLink(0), { model: "48gx" }); true`);
  await p.idle(10_000);
  await c.ev(`for (const n of ["A", "B", "C", "D"]) tx.select(n); true`);
  await p.until(`/«\\s*D\\s*»/.test(document.querySelector("#preview pre")?.textContent ?? "")`, 10_000, "D previewed");
  await p.idle(10_000);
  const got = await c.ev(`__requests.filter((r) => r.type === "get").map((r) => r.name)`);
  assert.deepEqual(got, ["A", "D"], "A was already running; B and C were dropped");
  assert.equal(await c.ev(`[...tx.state.previews.values()].some((v) => v.status === "error")`), false, "no error kept for B or C");
  await c.ev(`tx.select("B")`);
  await p.until(`/«\\s*B\\s*»/.test(document.querySelector("#preview pre")?.textContent ?? "")`, 10_000, "B previewed when selected again");
  assert.deepEqual(c.errors, []);
});

test("an unplug during a put says the calculator is gone, once", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`window.__link = __stubLink(0); tx.connectLink(__link, { model: "48gx" }); true`);
  await p.idle(10_000);
  // The put is never answered; the cable goes meanwhile.
  await c.ev(`window.__answerWas = __answer; window.__answer = async (req, w) => (req.type === "put" ? (__requests.push(req), null) : __answerWas(req, w)); window.__done = false; tx.putFiles([new File(["%%HP: T(3)A(R)F(.);\\r\\n1\\r\\n"], "X.txt")]).then(() => { __done = true; }); true`);
  await p.until(`__requests.some((r) => r.type === "put")`, 5_000, "the put asked");
  await c.ev(`__link.dispatchEvent(new CustomEvent("lost", { detail: "The cable or adapter was unplugged." })); true`);
  await p.until("__done", 5_000, "the put ended");
  await sleep(300);
  await p.until(`!!document.querySelector(".toast")?.textContent`, 5_000, "the toast");
  const toasts = await c.ev(`[...document.querySelectorAll(".toast")].map((e) => e.textContent)`);
  assert.deepEqual(toasts, ["The calculator is gone: The cable or adapter was unplugged."]);
  assert.equal(await c.ev(`document.getElementById("pane-title").textContent`), "No calculator");
  assert.deepEqual(c.errors, []);
});

test("a Worker that fails drops the device with a plain reason", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`tx.connectLink(__stubLink(0), { model: "48gx" }); true`);
  await p.idle(10_000);
  await c.ev(`__workers.at(-1).onmessage({ data: { type: "failed", message: "unreachable executed" } }); true`);
  await p.until(`!tx.state.device && !!document.querySelector(".toast")?.textContent`, 5_000, "dropped");
  assert.match(await c.ev(`document.querySelector(".toast")?.textContent ?? ""`), /The calculator is gone: the transfer part of the page stopped \(unreachable executed\)/);
  assert.deepEqual(c.errors, []);
});
