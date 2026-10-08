// The Variables list in the real page in headless Chrome over the
// DevTools protocol (no dependencies), with real mouse events: a
// double-click on a directory row opens it, one on another variable
// selects it; the "New directory…" field takes the focus once, gives up
// none it does not have, gives way to Rename and Purge, and keeps a name
// the calculator refused. No ROM: the page's memory reads are answered by the test
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
// (with A in it) and the real number X. `createDir` refuses "1A" as the
// host does and adds any other name to HOME.
const FAKE_MEMORY = `(() => {
  const b = window.saturnus.backend;
  const v = (name, type, extra = {}) => ({ name, type, size: 16, checksum: 0x5B55, address: 0x7A000 + name.length, ...extra });
  const tree = { path: ["HOME"], variables: [v("MYDIR", "Directory", { variables: [v("A", "Real Number")] }), v("X", "Real Number")] };
  b.watchMemory = async () => ({ supported: true });
  b.memoryTree = async () => structuredClone(tree);
  b.stack = async () => [];
  b.flags = async () => ({ set: [] });
  b.objectAt = async () => { throw new Error("not read in this test"); };
  b.createDir = async (dir, name) => {
    await new Promise((r) => setTimeout(r, 50));
    if (name === "1A") throw new Error('"1A" is not a plain variable name');
    tree.variables.unshift(v(name, "Directory", { variables: [] }));
    window.saturnus.memory.refresh();
    return { emulatedMs: 1, keys: false };
  };
  window.saturnus.store.set({ booted: "48sx" });
  return window.saturnus.setLayer(true).then(() => window.saturnus.explorer.setTab("vars")).then(() => true);
})()`;

/** The page with the fake memory in headless Chrome, or null when the test skipped. */
async function page(t) {
  const binary = findChrome();
  const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));
  if (!binary || !built) {
    t.skip(!binary ? "no Chrome found (SATURNUS_CHROME=/path/to/chrome)" : "web/pkg not built (just web)");
    return null;
  }
  const { server, port } = await serve();
  const c = await chrome(binary);
  t.after(() => {
    c.close();
    server.close();
  });
  const { send, ev } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started");
  await ev(FAKE_MEMORY);
  await until(ev, row("MYDIR"), 5_000, "the list shows MYDIR");
  /** A real mouse click in the middle of `selector`'s element. */
  const click = async (selector, clickCount = 1) => {
    const [x, y] = await ev(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [r.x + Math.min(30, r.width / 2), r.y + r.height / 2]; })()`);
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    for (let n = 1; n <= clickCount; n++) {
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: n });
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: n });
    }
    await sleep(150);
  };
  /** Text typed into the focused element, as the keyboard does. */
  const type = async (text) => {
    await send("Input.insertText", { text });
    await sleep(50);
  };
  /** A named key (Enter). */
  const press = async (key, code = key, vk = 0) => {
    await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key, code, windowsVirtualKeyCode: vk });
    await send("Input.dispatchKeyEvent", { type: "char", key, text: "\r" });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key, code, windowsVirtualKeyCode: vk });
    await sleep(150);
  };
  /** What has the focus: a short name for the assertions. */
  const focused = () => ev(`(() => {
    const a = document.activeElement;
    if (a?.closest(".vars-new")) return "new";
    if (a?.matches(".vars-find")) return "find";
    if (a?.closest(".preview .edit-row")) return "rename";
    return a?.tagName ?? null;
  })()`);
  return { ev, click, type, press, focused };
}

const row = (name) => `document.querySelector('.list tbody tr[data-name="${name}"]')`;
const rowSel = (name) => `.list tbody tr[data-name="${name}"] td`;

test("a double-click on a directory row opens it; on a variable it selects it", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const browse = () => p.ev("window.saturnus.explorer.browse");
  // The first click draws the list again: the second lands on a new row.
  await p.click(rowSel("MYDIR"), 2);
  assert.deepEqual(await browse(), ["HOME", "MYDIR"], "MYDIR opened");
  assert.ok(await p.ev(row("A")), "the list shows what MYDIR holds");

  await p.ev(`window.saturnus.explorer.go(["HOME"])`);
  await p.click(rowSel("X"), 2);
  assert.deepEqual(await browse(), ["HOME"], "a variable opens nothing");
  assert.equal(await p.ev("window.saturnus.explorer.selected?.name"), "X", "X selected");
});

test("the New directory field takes the focus once and steals none", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(".vars-mkdir");
  assert.equal(await p.focused(), "new", "the field has the focus when it opens");
  await p.type("AB");

  // Typing in the search field while the field is open: the letters stay there.
  await p.click(".vars-find");
  await p.type("my");
  assert.equal(await p.focused(), "find");
  assert.equal(await p.ev(`document.querySelector(".vars-find").value`), "my");
  await p.ev(`document.querySelector(".vars-find").value = ""; document.querySelector(".vars-find").dispatchEvent(new Event("input")); true`);
  // A memory refresh and a row selected leave the focus where it is.
  await p.ev("window.saturnus.memory.refresh().then(() => true)");
  await p.click(rowSel("X"));
  assert.equal(await p.focused(), "find", "neither took the focus into the field");
  assert.equal(await p.ev(`document.querySelector(".vars-new input").value`), "AB", "the name kept");

  // Back in the field, a render keeps its focus and its caret.
  await p.click(".vars-new input");
  await p.ev(`document.querySelector(".vars-new input").setSelectionRange(1, 1); true`);
  await p.ev("window.saturnus.explorer.renderVars(); true");
  assert.equal(await p.focused(), "new");
  assert.deepEqual(await p.ev(`(() => { const i = document.querySelector(".vars-new input"); return [i.value, i.selectionStart]; })()`), ["AB", 1]);
});

test("Rename and New directory close each other; Rename gets the focus", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(".vars-mkdir");
  await p.click(rowSel("X"));
  await p.ev(`[...document.querySelectorAll(".preview-actions button")].find((b) => b.textContent === "Rename").click(); true`);
  await sleep(100);
  assert.equal(await p.focused(), "rename", "the rename field has the focus");
  assert.equal(await p.ev(`document.querySelector(".vars-new").children.length`), 0, "New directory closed");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row input").value`), "X");

  await p.click(".vars-mkdir");
  assert.equal(await p.focused(), "new");
  assert.equal(await p.ev(`document.querySelector(".preview .edit-row")`), null, "the rename closed");
});

test("a name the calculator refuses stays in the field with the reason", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click(".vars-mkdir");
  await p.type("1A");
  await p.press("Enter", "Enter", 13);
  await until(p.ev, "!window.saturnus.store.state.writing && window.saturnus.store.state.writeMessage", 5_000, "the refusal");
  await sleep(100);
  const field = await p.ev(`(() => { const i = document.querySelector(".vars-new input"); return i && { value: i.value, readOnly: i.readOnly, error: document.querySelector(".vars-new .edit-error")?.textContent }; })()`);
  assert.deepEqual(field, { value: "1A", readOnly: false, error: 'Creating 1A failed: "1A" is not a plain variable name' });
  assert.equal(await p.focused(), "new", "the field has the focus again");

  // Corrected, it is created and the field closes.
  await p.ev(`(() => { const i = document.querySelector(".vars-new input"); i.select(); return true; })()`);
  await p.type("NEWD");
  await p.press("Enter", "Enter", 13);
  await until(p.ev, row("NEWD"), 5_000, "NEWD listed");
  await until(p.ev, `document.querySelector(".vars-new").children.length === 0`, 2_000, "the field closed");
  assert.equal(await p.ev("window.saturnus.explorer.selected?.name"), "NEWD");
});
