// The page on phones: no horizontal overflow on any view at any width,
// and the command palette usable as a sheet with the on-screen keyboard
// open. Runs the real page in headless Chrome with touch emulation over
// the DevTools protocol (no dependencies), served by a static server in
// this process. Skipped when no Chrome is found (`SATURNUS_CHROME` names
// one; `SATURNUS_AUDIT=1` makes the skip a failure, `just web-audit`) or
// the wasm package is not built (`just web`). No ROM is needed: every
// view has a state without one.
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
   tables of a calculator object, the lists of variables and commands. */
const MAY_SCROLL = [".tree", ".cmds-menus", ".grid-wrap", ".list-wrap", ".nibbles pre"];
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
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
    assert.ok(await ev("!!window.saturnus"), "the page started");
    await ev("window.saturnus.started");

    const reset = () => ev(`(() => {
      for (const d of document.querySelectorAll("dialog[open]")) d.close();
      document.body.classList.remove("sheet-open");
      document.getElementById("roms")?.removeAttribute("open");
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
    const VIEWS = {
      calculator: () => ev(`window.saturnus.store.set({ message: "A status line long enough to wrap in the panel of a narrow phone, with-a-very-long-unbroken-token-in-it" })`),
      sheet: () => ev(`document.body.classList.add("sheet-open")`),
      roms: () => ev(`document.body.classList.add("sheet-open"); document.getElementById("roms").open = true`),
      vars: () => layer("vars"),
      stack: () => layer("stack"),
      flags: () => layer("flags"),
      commands: () => layer("commands"),
      palette: () => palette("sto"),
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
    assert.deepEqual(c.errors, [], "no exception in the page");
  } finally {
    c.close();
    server.closeAllConnections();
    server.close();
  }
});
