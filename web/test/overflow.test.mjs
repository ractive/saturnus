// The page on phones: no horizontal overflow on any view at any width,
// and the command palette usable as a sheet with the on-screen keyboard
// open. Runs the real page in headless Chrome with touch emulation over
// the DevTools protocol (no dependencies), served by a static server in
// this process. Skipped when no Chrome is found (`SATURNUS_CHROME` names
// one; `SATURNUS_AUDIT=1` makes the skip a failure, `just web-audit`) or
// the wasm package is not built (`just web`). No ROM is needed: every
// view has a state without one. Fullscreen: on every model, upright and
// on its side, the keys take the width and the buttons cover nothing.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

// A filesystem path ending in a separator (no %20, no "/C:/" on Windows).
const WEB = fileURLToPath(new URL("../", import.meta.url));
const WIDTHS = [360, 390, 430, 768, 1280];
const HEIGHTS = { 360: 780, 390: 844, 430: 932, 768: 1024, 1280: 900 };
const KEYBOARD = 336; // an on-screen keyboard's height on a phone, roughly
/* Containers that may scroll sideways by design: trees (deep nesting),
   tables of a calculator object, the lists of variables and commands,
   the editor's lines (a program is not wrapped). */
const MAY_SCROLL = [".tree", ".cmds-menus", ".grid-wrap", ".list-wrap", ".nibbles pre", ".rpl-text"];
/** A program for the palette's editor, with a line wider than a phone. */
const PROGRAM = '« → N\n  « IF N 0 > THEN "a long string that runs well past the edge of a phone" END »\n»';
const MIME = { ".html": "text/html", ".css": "text/css", ".js": "text/javascript", ".mjs": "text/javascript", ".json": "application/json", ".svg": "image/svg+xml", ".wasm": "application/wasm" };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findChrome() {
  if (process.env.SATURNUS_CHROME) return process.env.SATURNUS_CHROME;
  const names = ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "chrome"];
  for (const dir of (process.env.PATH ?? "").split(delimiter)) {
    for (const n of names) if (dir && existsSync(join(dir, n))) return join(dir, n);
  }
  const mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
  return existsSync(mac) ? mac : null;
}

/** A static server for web/ on a free port; resolves to the port. */
function serve() {
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
    const file = join(WEB, path === "/" ? "index.html" : path);
    if (!file.startsWith(WEB) || !existsSync(file) || !statSync(file).isFile()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream" });
    res.end(readFileSync(file));
  });
  // Never keeps the process alive: a failed test must still end the run.
  server.unref();
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve({ server, port: server.address().port })));
}

/** Headless Chrome with one page over the DevTools protocol. */
async function chrome(binary) {
  const dir = mkdtempSync(join(tmpdir(), "saturnus-chrome-"));
  const proc = spawn(binary, [
    "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${dir}`, "--no-first-run", "--hide-scrollbars",
    "--window-size=1280,900", ...(process.env.CI ? ["--no-sandbox"] : []), "about:blank",
  ], { stdio: "ignore" });
  const portFile = join(dir, "DevToolsActivePort");
  for (let i = 0; i < 150 && !existsSync(portFile); i++) await sleep(100);
  if (!existsSync(portFile)) throw new Error("Chrome did not start");
  const dport = readFileSync(portFile, "utf8").split("\n")[0];
  const targets = await (await fetch(`http://127.0.0.1:${dport}/json`)).json();
  const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const pending = new Map();
  const errors = [];
  ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) {
      const [r, j] = pending.get(m.id);
      pending.delete(m.id);
      m.error ? j(new Error(JSON.stringify(m.error))) : r(m.result);
    } else if (m.method === "Runtime.exceptionThrown") errors.push(JSON.stringify(m.params.exceptionDetails).slice(0, 300));
  };
  const send = (method, params = {}) => new Promise((r, j) => { pending.set(++id, [r, j]); ws.send(JSON.stringify({ id, method, params })); });
  const ev = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 500));
    return r.result.value;
  };
  const close = () => {
    try { ws.close(); } catch { /* closing */ }
    proc.kill();
    try { rmSync(dir, { recursive: true, force: true, maxRetries: 3 }); } catch { /* Chrome still letting go of it */ }
  };
  return { send, ev, errors, close };
}

const MEASURE = `(() => {
  const de = document.documentElement;
  const sel = (e) => e.tagName.toLowerCase() + (e.id ? "#" + e.id : "") + (typeof e.className === "string" && e.className ? "." + e.className.trim().split(/\\s+/).join(".") : "");
  const visible = (e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== "hidden"; };
  const scrollers = [];
  for (const e of document.querySelectorAll("body *")) {
    if (e.closest(".skin") || !visible(e)) continue;
    if (/auto|scroll/.test(getComputedStyle(e).overflowX) && e.scrollWidth > e.clientWidth + 1 && !e.matches(${JSON.stringify(MAY_SCROLL.join(","))})) scrollers.push(sel(e) + " +" + (e.scrollWidth - e.clientWidth));
  }
  return { page: de.scrollWidth - de.clientWidth, inner: innerWidth, scrollers };
})()`;

test("no horizontal overflow on any view at any width; the palette as a phone sheet", { timeout: 180_000 }, async (t) => {
  const binary = findChrome();
  const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));
  if (!binary || !built) {
    const why = !binary ? "no Chrome found (SATURNUS_CHROME=/path/to/chrome)" : "web/pkg not built (just web)";
    if (process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return;
  }
  const { server, port } = await serve();
  const c = await chrome(binary);
  try {
    const { send, ev } = c;
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
    const metrics = (width, height) => send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 2, mobile: width < 1000 });
    await metrics(390, 844);
    // persist() recorded (and refused), to see that the page never calls it by itself.
    await send("Page.addScriptToEvaluateOnNewDocument", { source: `
      window.__persist = [];
      navigator.storage.persist = () => { window.__persist.push(new Error().stack); return Promise.resolve(false); };
      navigator.storage.persisted = () => Promise.resolve(false);` });
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
    assert.ok(await ev("!!window.saturnus"), "the page started");
    await ev("window.saturnus.started");
    await sleep(300);
    assert.equal(await ev("window.__persist.length"), 0, "no persist() on load");

    const reset = () => ev(`(() => {
      for (const d of document.querySelectorAll("dialog[open]")) d.close();
      document.body.classList.remove("sheet-open");
      document.getElementById("roms")?.removeAttribute("open");
      window.saturnus.store.set({ storageOffer: null });
      if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
      return window.saturnus.setLayer(false);
    })()`);
    /** Poll `expression` (at most `ms`) until it is true. */
    const until = async (expression, ms = 8000) => {
      for (let i = 0; i < ms / 100; i++) {
        if (await ev(expression).catch(() => false)) return true;
        await sleep(100);
      }
      return false;
    };
    const layer = async (tab) => {
      await ev(`window.saturnus.setLayer(true).then(() => window.saturnus.explorer.setTab(${JSON.stringify(tab)}))`);
      if (tab === "commands") assert.ok(await until(`!!window.saturnus.explorer.cmdIndex`), "the Commands tab read its index");
    };
    const palette = async (query) => {
      await ev(`window.saturnus.palette.open(${JSON.stringify(query)})`);
      assert.ok(await until(`document.querySelectorAll("dialog.palette .prow").length > 3`), "the palette listed its rows");
    };
    // A ROM shown as kept (no ROM is needed for the ROMs panel's look).
    const KEPT = `(() => { const s = window.saturnus.store; s.set({ storage: "best-effort", roms: { ...s.state.roms, slots: s.state.roms.slots.map((x, i) => i ? x : { ...x, fileName: "a-rom-file-with-a-long-name.bin", state: "ready" }) } }); })()`;
    const VIEWS = {
      calculator: () => ev(`window.saturnus.store.set({ message: "A status line long enough to wrap in the panel of a narrow phone, with-a-very-long-unbroken-token-in-it" })`),
      sheet: () => ev(`document.body.classList.add("sheet-open")`),
      roms: () => ev(`document.body.classList.add("sheet-open"); document.getElementById("roms").open = true; ${KEPT}`),
      storage: () => ev(`window.saturnus.store.set({ storageOffer: "ask" })`),
      vars: () => layer("vars"),
      stack: () => layer("stack"),
      flags: () => layer("flags"),
      commands: () => layer("commands"),
      palette: () => palette("sto"),
      editor: async () => {
        await palette("sto");
        await ev(`window.saturnus.palette.enterEditor(${JSON.stringify(PROGRAM)})`);
      },
      shortcuts: () => ev(`window.saturnus.shortcuts.open()`),
      about: () => ev(`document.querySelector("sat-about").open()`),
    };
    const failures = [];
    for (const w of WIDTHS) {
      for (const scheme of ["light", "dark"]) {
        await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
        for (const [view, show] of Object.entries(VIEWS)) {
          await metrics(w, HEIGHTS[w]);
          await reset();
          await show();
          await sleep(view === "about" ? 700 : 300);
          const m = await ev(MEASURE);
          if (m.page > 0) failures.push(`${view} at ${w}px ${scheme}: the page is ${m.page}px too wide (innerWidth ${m.inner})`);
          for (const s of m.scrollers) failures.push(`${view} at ${w}px ${scheme}: ${s} scrolls sideways`);
        }
      }
    }
    assert.deepEqual(failures, []);

    // Persistent storage asked for in context (kb iteration 28b): the
    // notice after a ROM is kept, Keep it, Not now, the ROMs panel's button.
    await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
    await metrics(390, 844);
    await reset();
    const kept = () => ev(`document.querySelector("sat-controls").kept({ slots: [{ fileName: "rom" }] })`);
    const notice = () => ev(`(() => { const n = document.querySelector(".storage-notice"); if (!n) return null; const r = n.getBoundingClientRect(); return { text: n.querySelector("p").textContent, buttons: [...n.querySelectorAll("button")].map((b) => b.textContent), inside: r.left >= 0 && r.right <= innerWidth && r.bottom <= innerHeight }; })()`);
    const click = (sel) => ev(`document.querySelector(${JSON.stringify(sel)}).click()`);
    await kept();
    assert.ok(await until(`!!document.querySelector(".storage-notice")`), "the notice after a ROM is kept");
    const ask = await notice();
    assert.match(ask.text, /^Keep this ROM on this device\?/);
    assert.deepEqual(ask.buttons, ["Not now", "Keep it"]);
    assert.ok(ask.inside, "the notice is inside the viewport at 390 px");
    await click(".storage-notice button.primary");
    assert.ok(await until(`window.__persist.length === 1`), "Keep it calls persist()");
    assert.ok(await until(`/said no/.test(document.querySelector(".storage-notice p")?.textContent)`), "the refusal in one line");
    await click(".storage-notice button");
    assert.equal(await notice(), null, "OK closes it");
    await kept();
    assert.ok(await until(`!!document.querySelector(".storage-notice")`));
    await click(".storage-notice button:not(.primary)");
    assert.equal(await notice(), null, "Not now closes it");
    assert.equal(await ev(`localStorage.getItem("saturnus.storageAsk")`), "not-now", "Not now is remembered");
    await kept();
    await sleep(300);
    assert.equal(await notice(), null, "and the notice is not offered again");
    await ev(`document.body.classList.add("sheet-open"); document.getElementById("roms").open = true; ${KEPT}`);
    assert.equal(await ev(`document.querySelector(".rom-storage .storage-state").textContent`), "May be cleared when space runs low.");
    await click("#rom-keep");
    assert.ok(await until(`window.__persist.length === 2`), "the ROMs panel's Keep permanently calls persist()");
    await ev(`localStorage.removeItem("saturnus.storageAsk")`);
    await reset();

    // The palette on a phone with the keyboard up: the sheet follows the
    // visual viewport (simulated by a shorter viewport), the input and the
    // list stay inside it, a tapped row opens its entry with the way
    // back and the buttons inside the viewport.
    await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
    await metrics(390, 844 - KEYBOARD);
    await reset();
    await palette("sto");
    await sleep(300);
    const sheet = await ev(`(() => {
      const d = document.querySelector("dialog.palette");
      const list = d.querySelector(".palette-list");
      const input = d.querySelector(".palette-input input");
      const rows = [...list.querySelectorAll(".prow")].map((r) => r.getBoundingClientRect().height);
      return { dialog: d.getBoundingClientRect().toJSON(), list: list.getBoundingClientRect().toJSON(), input: input.getBoundingClientRect().toJSON(), inner: innerHeight, vvh: d.style.getPropertyValue("--vvh"), rows: rows.length, minRow: Math.min(...rows) };
    })()`);
    assert.equal(sheet.vvh, `${844 - KEYBOARD}px`, "the sheet's height follows the visual viewport");
    assert.ok(sheet.dialog.bottom <= sheet.inner + 1, `the sheet ends inside the viewport (${sheet.dialog.bottom} of ${sheet.inner})`);
    assert.ok(sheet.input.bottom <= sheet.inner, "the input is visible");
    assert.ok(sheet.list.bottom <= sheet.inner + 1 && sheet.list.height > 100, `the list is inside the viewport (${JSON.stringify(sheet.list)})`);
    assert.ok(sheet.rows > 3 && sheet.minRow >= 44, `rows are tappable (${sheet.rows} rows, the smallest ${sheet.minRow}px)`);
    const [x, y] = await ev(`(() => { const r = document.querySelector(".prow").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
    await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    await sleep(300);
    const detail = await ev(`(() => {
      const box = document.querySelector(".palette-box");
      const back = box.querySelector(".palette-back button");
      const actions = box.querySelector(".palette-actions");
      return { open: box.classList.contains("detail-open"), back: back?.getBoundingClientRect().toJSON(), actions: actions?.getBoundingClientRect().toJSON(), inner: innerHeight, name: box.querySelector(".palette-back-name")?.textContent };
    })()`);
    assert.ok(detail.open, "a tapped row opens its entry as a second step");
    assert.equal(detail.name, "STO");
    assert.ok(detail.back && detail.back.height >= 44 && detail.back.top >= 0, "the way back is a finger's size at the top");
    assert.ok(detail.actions && detail.actions.bottom <= detail.inner + 1, `the buttons are inside the viewport (${JSON.stringify(detail.actions)})`);

    // The editor mode with the keyboard up: the head, the text and the
    // buttons inside the viewport, the buttons a finger's size.
    await ev(`window.saturnus.palette.enterEditor(${JSON.stringify(PROGRAM)})`);
    await sleep(300);
    const editor = await ev(`(() => {
      const d = document.querySelector("dialog.palette");
      const r = (sel) => d.querySelector(sel).getBoundingClientRect().toJSON();
      return { head: r(".editor-head"), text: r(".rpl-text"), bar: r(".editor-bar"), primary: r('[data-ed="primary"]'), inner: innerHeight };
    })()`);
    assert.ok(editor.head.top >= 0 && editor.bar.bottom <= editor.inner + 1, `the editor is inside the viewport (${JSON.stringify(editor)})`);
    assert.ok(editor.text.height > 120, `the text has room (${editor.text.height}px)`);
    assert.ok(editor.primary.height >= 44, "the buttons are a finger's size");
    assert.deepEqual(c.errors, [], "no exception in the page");
  } finally {
    c.close();
    server.closeAllConnections();
    server.close();
  }
});

/** Fullscreen on phones: the models and the sizes the face is checked at. */
const FS_MODELS = ["48sx", "48gx", "49g", "38g", "39g", "40g", "42s"];
const FS_SIZES = [[360, 780], [390, 844], [430, 932], [844, 390], [932, 430]];
const FS_MEASURE = `(() => {
  const box = (e) => { const r = e.getBoundingClientRect(); return { l: r.left, t: r.top, r: r.right, b: r.bottom }; };
  const keys = [...document.querySelectorAll("sat-calculator .skin g.skey .cap")].map(box);
  const span = keys.reduce((a, k) => ({ l: Math.min(a.l, k.l), r: Math.max(a.r, k.r) }), { l: Infinity, r: -Infinity });
  const buttons = [...document.querySelectorAll("button.fs-tool")].map(box);
  // The print and the logo where they show: inside the drawn part of the face.
  const view = box(document.querySelector("sat-calculator .skin svg"));
  const print = [...document.querySelectorAll("sat-calculator .skin g.print > *, sat-calculator .skin image.logo")]
    .filter((e) => getComputedStyle(e).display !== "none").map(box)
    .map((b) => ({ l: Math.max(b.l, view.l), t: Math.max(b.t, view.t), r: Math.min(b.r, view.r), b: Math.min(b.b, view.b) }))
    .filter((b) => b.r > b.l && b.b > b.t);
  return { w: innerWidth, h: innerHeight, edge: document.querySelector("sat-calculator").classList.contains("edge"), keys, span, print, lcd: box(document.querySelector("sat-calculator canvas")), buttons };
})()`;

test("fullscreen: the keys take a phone's width and the buttons cover nothing", { timeout: 180_000 }, async (t) => {
  const binary = findChrome();
  const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));
  if (!binary || !built) {
    const why = !binary ? "no Chrome found (SATURNUS_CHROME=/path/to/chrome)" : "web/pkg not built (just web)";
    if (process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return;
  }
  const { server, port } = await serve();
  const c = await chrome(binary);
  try {
    const { send, ev } = c;
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
    const metrics = (width, height) => send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 2.625, mobile: true });
    await metrics(390, 844);
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
    await ev("window.saturnus.started");
    // Fullscreen wants a user gesture.
    await send("Runtime.evaluate", { expression: `document.getElementById("bar-fullscreen").click()`, userGesture: true });
    await sleep(500);
    const inside = (a, b) => a.l >= b.l - 0.5 && a.t >= b.t - 0.5 && a.r <= b.r + 0.5 && a.b <= b.b + 0.5;
    const overlap = (a, b) => a.l < b.r && b.l < a.r && a.t < b.b && b.t < a.b;
    const failures = [];
    for (const model of FS_MODELS) {
      await ev(`(() => { const s = document.getElementById("model"); s.value = ${JSON.stringify(model)}; s.dispatchEvent(new Event("change", { bubbles: true })); })()`);
      await sleep(400);
      for (const [w, h] of FS_SIZES) {
        await metrics(w, h);
        await sleep(300);
        const m = await ev(FS_MEASURE);
        const at = `${model} at ${w}x${h}`;
        const screen = { l: 0, t: 0, r: m.w, b: m.h };
        if (!m.edge) failures.push(`${at}: not edge to edge`);
        if (m.keys.length < 30 || m.keys.some((k) => !inside(k, screen))) failures.push(`${at}: a key is off the screen`);
        if (!inside(m.lcd, screen)) failures.push(`${at}: the display is off the screen`);
        // Upright, the keys take the screen's width (the bezel and the rim
        // cropped), or as much of it as the screen's height leaves.
        if (w < h && m.span.r - m.span.l < 0.84 * w) failures.push(`${at}: the keys span ${Math.round(m.span.r - m.span.l)}px of ${w}`);
        for (const b of m.buttons) {
          if (Math.min(b.r - b.l, b.b - b.t) < 44) failures.push(`${at}: a button is under 44px`);
          if (overlap(b, m.lcd) || m.keys.some((k) => overlap(b, k))) failures.push(`${at}: a button covers the display or a key`);
          if (m.print.some((p) => overlap(b, p))) failures.push(`${at}: a button covers the print or the logo`);
        }
      }
    }
    assert.deepEqual(failures, []);
    assert.deepEqual(c.errors, [], "no exception in the page");
  } finally {
    c.close();
    server.closeAllConnections();
    server.close();
  }
});
