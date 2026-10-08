// The Variables list in the real page in headless Chrome over the
// DevTools protocol (no dependencies), with real mouse events: a
// double-click on a directory row opens it, one on another variable
// selects it. No ROM: the page's memory reads are answered by the test
// (a 48SX with HOME holding MYDIR and X). Skipped without Chrome
// (`SATURNUS_CHROME` names one) or the wasm package (`just web`).
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

/** web/ on a free port. */
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

// The memory reads answered in the page: HOME holds the directory MYDIR
// (with A in it) and the real number X.
const FAKE_MEMORY = `(() => {
  const b = window.saturnus.backend;
  const v = (name, type, extra = {}) => ({ name, type, size: 16, checksum: 0x5B55, address: 0x7A000 + name.length, ...extra });
  const tree = { path: ["HOME"], variables: [v("MYDIR", "Directory", { variables: [v("A", "Real Number")] }), v("X", "Real Number")] };
  b.watchMemory = async () => ({ supported: true });
  b.memoryTree = async () => tree;
  b.stack = async () => [];
  b.flags = async () => ({ set: [] });
  b.objectAt = async () => { throw new Error("not read in this test"); };
  window.saturnus.store.set({ booted: "48sx" });
  return window.saturnus.setLayer(true).then(() => window.saturnus.explorer.setTab("vars")).then(() => true);
})()`;

test("a double-click on a directory row opens it; on a variable it selects it", { timeout: 120_000 }, async (t) => {
  const binary = findChrome();
  const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));
  if (!binary || !built) {
    t.skip(!binary ? "no Chrome found (SATURNUS_CHROME=/path/to/chrome)" : "web/pkg not built (just web)");
    return;
  }
  const { server, port } = await serve();
  const c = await chrome(binary);
  try {
    const { send, ev } = c;
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    await until(ev, "!!window.saturnus", 15_000, "the page started");
    await ev("window.saturnus.started");
    await ev(FAKE_MEMORY);
    const row = (name) => `document.querySelector('.list tbody tr[data-name="${name}"]')`;
    await until(ev, row("MYDIR"), 5_000, "the list shows MYDIR");

    /** Two clicks as a mouse sends a double-click (click counts 1 and 2). */
    const doubleClick = async (name) => {
      const [x, y] = await ev(`(() => { const r = ${row(name)}.getBoundingClientRect(); return [r.x + 30, r.y + r.height / 2]; })()`);
      await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      for (const clickCount of [1, 2]) {
        await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount });
        await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount });
      }
      await sleep(200);
    };
    const browse = () => ev("window.saturnus.explorer.browse");

    // The first click draws the list again: the second lands on a new row.
    await doubleClick("MYDIR");
    assert.deepEqual(await browse(), ["HOME", "MYDIR"], "MYDIR opened");
    assert.ok(await ev(row("A")), "the list shows what MYDIR holds");

    await ev(`window.saturnus.explorer.go(["HOME"])`);
    await doubleClick("X");
    assert.deepEqual(await browse(), ["HOME"], "a variable opens nothing");
    assert.equal(await ev("window.saturnus.explorer.selected?.name"), "X", "X selected");
  } finally {
    c.close();
    server.close();
  }
});
