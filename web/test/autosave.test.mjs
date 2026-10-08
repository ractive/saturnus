// The calculator keeps its state across a reload (iteration 27), in the
// real page in headless Chrome over the DevTools protocol (no
// dependencies): a 48SX booted from `SATURNUS_ROM_DIR/sxrom-j`, `1 ENTER
// 2` typed through `window.saturnus.backend`, the state auto-saved after
// the delay and not again while idle, the page reloaded: the same stack
// and command line, no "Try To Recover Memory?". Start fresh asks in the
// page, cold-boots and forgets the kept state. Skipped without the ROM,
// without Chrome (`SATURNUS_CHROME` names one) or without the wasm
// package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const WEB = fileURLToPath(new URL("../", import.meta.url));
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

/** web/ on a free port, and the ROM at /__rom (this test's only). */
function serve(rom) {
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
    if (path === "/__rom") {
      res.writeHead(200, { "content-type": "application/octet-stream" });
      res.end(readFileSync(rom));
      return;
    }
    const file = join(WEB, path === "/" ? "index.html" : path);
    if (!file.startsWith(WEB) || !existsSync(file) || !statSync(file).isFile()) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream" });
    res.end(readFileSync(file));
  });
  server.unref();
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve({ server, port: server.address().port })));
}

/** Headless Chrome with one page over the DevTools protocol. */
async function chrome(binary) {
  const dir = mkdtempSync(join(tmpdir(), "saturnus-chrome-"));
  const proc = spawn(binary, [
    "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${dir}`, "--no-first-run",
    "--window-size=1280,900", ...(process.env.CI ? ["--no-sandbox"] : []), "about:blank",
  ], { stdio: "ignore" });
  proc.unref();
  const portFile = join(dir, "DevToolsActivePort");
  for (let i = 0; i < 150 && !existsSync(portFile); i++) await sleep(100);
  if (!existsSync(portFile)) {
    proc.kill();
    throw new Error("Chrome did not start");
  }
  const dport = readFileSync(portFile, "utf8").split("\n")[0];
  const targets = await (await fetch(`http://127.0.0.1:${dport}/json`)).json();
  const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const pending = new Map();
  ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) {
      const [r, j] = pending.get(m.id);
      pending.delete(m.id);
      m.error ? j(new Error(JSON.stringify(m.error))) : r(m.result);
    }
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
  return { send, ev, close };
}

/** Poll `expression` in the page until it is truthy, at most `ms`. */
async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

// In the page: count the `autoSaved` events; wait for the CPU to sleep
// with no key down (the ROM waits for a key); press a key as the page does.
const HELPERS = `
  window.__saved = 0;
  window.saturnus.backend.addEventListener("autoSaved", () => window.__saved++);
  window.__idle = async (ms = 1500) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (;;) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  };
  window.__press = async (key) => {
    const b = window.saturnus.backend;
    b.keyDown(key);
    await new Promise((r) => setTimeout(r, 80));
    b.keyUp(key);
    await window.__idle(300);
  };
  true`;

const READ = `(async () => {
  const b = window.saturnus.backend;
  const stack = (await b.stack()).map((l) => l.text);
  const line = await b.commandLine();
  return { stack, line: line.active ? line.text : null };
})()`;

test("the calculator keeps its state across a reload", { timeout: 180_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "sxrom-j") : null;
  const binary = findChrome();
  const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));
  if (!rom || !existsSync(rom) || !binary || !built) {
    t.skip(!rom || !existsSync(rom) ? "SATURNUS_ROM_DIR/sxrom-j not found" : !binary ? "no Chrome found (SATURNUS_CHROME=/path/to/chrome)" : "web/pkg not built (just web)");
    return;
  }
  const { server, port } = await serve(rom);
  const c = await chrome(binary);
  try {
    const { send, ev } = c;
    await send("Page.enable");
    await send("Runtime.enable");
    const open = async () => {
      await until(ev, "!!window.saturnus", 15_000, "the page started");
      await ev("window.saturnus.started");
      await ev(HELPERS);
    };
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    await open();

    // A cold boot from the ROM; NO to "Try To Recover Memory?".
    await ev(`(async () => {
      const bytes = await (await fetch("/__rom")).arrayBuffer();
      await window.saturnus.startWithRom(new File([bytes], "sxrom-j"));
      return true;
    })()`);
    await until(ev, "window.saturnus.store.state.booted === '48sx'", 15_000, "booted");
    await ev("window.__idle()");
    await ev("window.__press('f')");
    await ev("window.__idle()");
    const coldSaves = await ev("window.__saved");

    // 1 ENTER 2, through the backend as the page's keys send them.
    for (const k of ["1", "enter", "2"]) await ev(`window.__press(${JSON.stringify(k)})`);
    const typed = await ev(READ);
    assert.deepEqual(typed, { stack: ["1"], line: "2" });
    await until(ev, `window.__saved > ${coldSaves}`, 15_000, "the state saved after the delay");
    const saves = await ev("window.__saved");
    await sleep(7_000);
    assert.equal(await ev("window.__saved"), saves, "idle: nothing written");

    // Reloaded: the last model boots with the state it had.
    await send("Page.reload");
    await sleep(300);
    await open();
    await until(ev, "window.saturnus.store.state.booted === '48sx'", 15_000, "booted again");
    // The command line is open with "2": the ROM is past any question.
    assert.deepEqual(await ev(READ), typed, "the same stack and command line");
    await sleep(6_000);
    assert.equal(await ev("window.__saved"), 0, "a restore is not a change");

    // Start fresh: the page's own question, then a cold boot; the kept
    // state is gone, so a reload boots cold too.
    await ev("document.querySelector('sat-controls').startFresh(); true");
    await until(ev, "!!document.querySelector('.fresh-notice')", 5_000, "the question");
    assert.match(await ev("document.querySelector('.fresh-notice p').textContent"), /Start the HP 48SX fresh\?/);
    await ev("[...document.querySelectorAll('.fresh-notice button')].find((b) => b.textContent === 'Start fresh').click(); true");
    await until(ev, "window.saturnus.store.state.message === 'started fresh'", 15_000, "started fresh");
    assert.equal(await ev("!!document.querySelector('.fresh-notice')"), false);
    const kept = await ev(`new Promise((resolve) => {
      const req = indexedDB.open("saturnus", 1);
      req.onsuccess = () => {
        const get = req.result.transaction("states").objectStore("states").get("auto:48sx");
        get.onsuccess = () => resolve(Boolean(get.result));
      };
    })`);
    assert.equal(kept, false, "the kept state is forgotten");
    await ev("window.__idle()");
    assert.notDeepEqual(await ev(READ).catch(() => null), typed, "an empty memory");
  } finally {
    c.close();
    server.close();
  }
});
