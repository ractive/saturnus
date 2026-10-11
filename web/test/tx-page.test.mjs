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
  const seen = await pageSession(c);
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
    start: () => ev(`(() => { document.getElementById("pane-empty").hidden = false; document.getElementById("pane-body").hidden = true; document.getElementById("last-note").hidden = false; document.getElementById("last-note").textContent = "The calculator has left server mode. If its server was started in algebraic mode, the 49G has now put what was on its stack into one list."; return true; })()`),
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
    connect: () => ev(`(() => { document.getElementById("add-serial").click(); return true; })()`).then(() => p.until(`document.getElementById("connect-dialog").open`, 5_000, "the connect dialog")),
    emulated: () => ev(`(() => { document.getElementById("add-emulated").click(); return true; })()`).then(() => p.until(`document.getElementById("emulated-dialog").open`, 5_000, "the emulated dialog")),
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
