// The screen as an image in the real page in headless Chrome over the
// DevTools protocol (no dependencies), with real mouse and key events:
// the panel's Copy screen writes an image/png to the clipboard (stubbed)
// and Save screen hands a PNG of the right size to the backend's save,
// on the 48GX, the 49G and the 42S; the shortcut copies; the display's
// menu opens on a right-click with its four items; where images cannot
// be copied, the image is saved and the page says so. No ROM: the page
// is made to believe a model runs (`booted` and `model` set, a frame).
// Skipped without Chrome (`SATURNUS_CHROME` names one) or the wasm
// package (`just web`).
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

/** How long Chrome may take to start, and any one DevTools call to answer. */
const START_MS = 30_000;
const CALL_MS = 30_000;

/**
 * Headless Chrome with one page over the DevTools protocol. Chrome runs
 * in its own process group (POSIX), and `close` kills the whole group:
 * a wrapper script's children, the crashpad handler and the helpers
 * included. A Chrome left running keeps this file's process, and with it
 * `node --test`, alive for good. Every call fails after `CALL_MS`, so no
 * await outlives a dead or stuck browser.
 */
async function chrome(binary) {
  const dir = mkdtempSync(join(tmpdir(), "saturnus-chrome-"));
  const group = process.platform !== "win32";
  const proc = spawn(binary, [
    "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${dir}`, "--no-first-run", "--hide-scrollbars",
    "--window-size=1280,900", ...(process.env.CI ? ["--no-sandbox"] : []), "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"], detached: group });
  let exited = null;
  let stderr = "";
  proc.on("exit", (code, signal) => { exited = signal ?? code; });
  proc.on("error", (e) => { exited = e.message; });
  proc.stderr.on("data", (d) => { stderr = (stderr + d).slice(-2000); });
  let ws = null;
  let closed = false;
  const pending = new Map();
  // The group even when Chrome itself is gone: its helpers may not be.
  const kill = () => {
    try {
      if (group) process.kill(-proc.pid, "SIGKILL");
      else if (exited === null) proc.kill("SIGKILL");
    } catch { /* already gone */ }
  };
  // Even when the process ends some other way (the runner's own timeout).
  process.on("exit", kill);
  const close = async () => {
    if (closed) return;
    closed = true;
    for (const [, [, j]] of pending) j(new Error("Chrome closed"));
    pending.clear();
    try { ws?.close(); } catch { /* closing */ }
    kill();
    for (let i = 0; i < 50 && exited === null; i++) await sleep(100);
    proc.stderr.destroy();
    process.off("exit", kill);
    try { rmSync(dir, { recursive: true, force: true, maxRetries: 3 }); } catch { /* Chrome still letting go of it */ }
  };
  try {
    const portFile = join(dir, "DevToolsActivePort");
    for (let i = 0; i < START_MS / 100 && exited === null && !existsSync(portFile); i++) await sleep(100);
    if (!existsSync(portFile)) {
      throw new Error(`Chrome did not start in ${START_MS / 1000} s (${exited === null ? "still running" : `exited: ${exited}`}): ${stderr.trim().slice(-800)}`);
    }
    const dport = readFileSync(portFile, "utf8").split("\n")[0];
    const targets = await (await fetch(`http://127.0.0.1:${dport}/json`, { signal: AbortSignal.timeout(CALL_MS) })).json();
    ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
    await new Promise((r, j) => {
      const timer = setTimeout(() => j(new Error("the DevTools socket did not open")), CALL_MS);
      ws.onopen = () => { clearTimeout(timer); r(); };
      ws.onerror = () => { clearTimeout(timer); j(new Error("the DevTools socket failed")); };
    });
  } catch (e) {
    await close();
    throw e;
  }
  let id = 0;
  const errors = [];
  ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) {
      const [r, j] = pending.get(m.id);
      pending.delete(m.id);
      m.error ? j(new Error(JSON.stringify(m.error))) : r(m.result);
    } else if (m.method === "Runtime.exceptionThrown") errors.push(JSON.stringify(m.params.exceptionDetails).slice(0, 300));
  };
  // Why later calls fail at once once the socket is gone (a crash, say),
  // rather than each waiting out CALL_MS.
  let gone = null;
  ws.onclose = () => {
    gone = `the DevTools socket closed (Chrome ${exited === null ? "still running" : `exited: ${exited}`}): ${stderr.trim().slice(-800)}`;
    for (const [, [, j]] of pending) j(new Error(gone));
    pending.clear();
  };
  const send = (method, params = {}) => {
    if (closed) return Promise.reject(new Error("Chrome closed"));
    if (gone || ws.readyState !== WebSocket.OPEN) return Promise.reject(new Error(gone ?? "the DevTools socket is not open"));
    const n = ++id;
    return new Promise((r, j) => {
      const timer = setTimeout(() => {
        pending.delete(n);
        j(new Error(`${method} got no answer in ${CALL_MS / 1000} s`));
      }, CALL_MS);
      const done = (f) => (v) => { clearTimeout(timer); f(v); };
      pending.set(n, [done(r), done(j)]);
      ws.send(JSON.stringify({ id: n, method, params }));
    });
  };
  const ev = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 500));
    return r.result.value;
  };
  /**
   * `expression` once the layout has settled: evaluated after two frames,
   * until two readings in a row agree. A viewport just resized is
   * laid out a frame before the calculator's ResizeObserver refits the
   * skin to it; a fixed sleep can read that frame on a busy machine.
   */
  const settled = async (expression) => {
    const frame = `new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(${expression}))))`;
    let last = await ev(frame);
    for (let i = 0; i < 30; i++) {
      const next = await ev(frame);
      if (JSON.stringify(next) === JSON.stringify(last)) return next;
      last = next;
    }
    return last;
  };
  return { send, ev, settled, errors, close };
}

/**
 * Chrome and the server for a test, or null when the test skipped (no
 * Chrome, no wasm package). Both are closed when the test ends, passed,
 * failed or timed out (the test's signal), so neither outlives it.
 */
async function session(t) {
  const binary = findChrome();
  const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));
  if (!binary || !built) {
    const why = !binary ? "no Chrome found (SATURNUS_CHROME=/path/to/chrome)" : "web/pkg not built (just web)";
    if (process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return null;
  }
  const { server, port } = await serve();
  const stop = () => {
    server.closeAllConnections();
    server.close();
  };
  let c;
  try {
    c = await chrome(binary);
  } catch (e) {
    stop();
    throw e;
  }
  let closed = false;
  const close = async () => {
    if (closed) return;
    closed = true;
    await c.close();
    stop();
  };
  // A timed-out test's body may still be waiting: end it from here.
  t.signal.addEventListener("abort", () => { close(); }, { once: true });
  t.after(close);
  return { ...c, port };
}

/**
 * Make the page believe `model` runs with a frame (a diagonal line, the
 * busy annunciator on), stub the clipboard (`window.copied`: the types
 * and the PNG's size) and the backend's save (`window.saved`).
 */
const fake = (model, { clipboard = true } = {}) => `(async () => {
  const s = window.saturnus;
  const rows = ${model === "42s" ? 16 : 64};
  const px = new Uint8Array(17 * rows);
  for (let y = 0; y < rows; y++) px[y * 17 + (y >> 3)] |= 0x80 >> (y & 7);
  const ann = { leftshift: false, rightshift: false, alpha: false, alert: false, busy: true, transmit: false, updown: false, battery: false, g: false, rad: false };
  s.store.set({ booted: "${model}", model: "${model}", frame: { width: 131, height: rows, pixels: btoa(String.fromCharCode(...px)), annunciators: ann, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 } });
  const size = async (blob) => {
    const b = new Uint8Array(await blob.arrayBuffer());
    const v = new DataView(b.buffer);
    const sig = [137, 80, 78, 71].every((x, i) => b[i] === x);
    return { png: sig, type: blob.type, width: v.getUint32(16), height: v.getUint32(20) };
  };
  window.copied = null;
  window.saved = null;
  if (${clipboard}) {
    window.ClipboardItem = class { constructor(parts) { this.parts = parts; } static supports(t) { return t === "image/png"; } };
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { write: async (items) => {
      const parts = items[0].parts;
      window.copied = { types: Object.keys(parts), ...(await size(await parts["image/png"])) };
    } } });
  } else {
    window.ClipboardItem = undefined;
  }
  s.backend.saveFile = async (name, blob) => { window.saved = { name, ...(await size(blob)) }; return name; };
  const calc = document.querySelector("sat-calculator");
  for (let i = 0; i < 100 && !(calc.skinModel === "${model}" && calc.screenReady()); i++) await new Promise((r) => setTimeout(r, 50));
  return calc.skinModel;
})()`;

/** The page in headless Chrome at `width`, or null when the test skipped. */
async function page(t, width = 1280) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  /** A real click in the middle of `selector`'s element (`button`: "left", "right"). */
  const click = async (selector, button = "left") => {
    const [x, y] = await ev(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount: 1 });
    await sleep(100);
  };
  const until = async (expr, what) => {
    for (let i = 0; i < 50; i++) {
      const v = await ev(expr);
      if (v) return v;
      await sleep(100);
    }
    assert.fail(`timed out: ${what}`);
  };
  return { ...c, click, until };
}

test("Copy screen puts a 4x PNG on the clipboard, Save screen saves one, on three models", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  for (const [model, height] of [["48gx", 288], ["49g", 288], ["42s", 96]]) {
    assert.equal(await p.ev(fake(model)), model);
    await p.click("#copy-screen");
    const copied = await p.until("window.copied", `${model} copied`);
    assert.deepEqual(copied, { types: ["image/png"], png: true, type: "image/png", width: 524, height }, model);
    assert.match(await p.ev("window.saturnus.store.state.message"), /copied/);
    await p.click("#save-screen");
    const saved = await p.until("window.saved", `${model} saved`);
    assert.equal(saved.png, true);
    assert.deepEqual([saved.width, saved.height], [524, height]);
    assert.match(saved.name, new RegExp(`^${model}-\\d{4}-\\d\\d-\\d\\d-\\d{4}\\.png$`));
  }
});

test("the shortcut copies; the look follows the panel's choice", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48gx")), "48gx");
  // Black on white: the first pixel of the frame is black in the image.
  await p.click('#screen-look button[data-look="bw"]');
  assert.equal(await p.ev("window.saturnus.store.state.screenLook"), "bw");
  await p.ev(`window.__pixel = null; const orig = navigator.clipboard.write; navigator.clipboard.write = async (items) => {
    const blob = await items[0].parts["image/png"];
    const bmp = await createImageBitmap(blob);
    const c = new OffscreenCanvas(bmp.width, bmp.height).getContext("2d");
    c.drawImage(bmp, 0, 0);
    window.__pixel = [...c.getImageData(1, 33, 1, 1).data, ...c.getImageData(10, 33, 1, 1).data];
    return orig(items);
  }; true`);
  // Alt+Shift+C (Alt 1, Shift 8).
  await p.send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "C", code: "KeyC", windowsVirtualKeyCode: 67, modifiers: 1 | 8 });
  await p.send("Input.dispatchKeyEvent", { type: "keyUp", key: "C", code: "KeyC", windowsVirtualKeyCode: 67, modifiers: 1 | 8 });
  await p.until("window.copied", "copied by the shortcut");
  assert.deepEqual(await p.ev("window.__pixel"), [0, 0, 0, 255, 255, 255, 255, 255], "black on white");
});

test("a right-click on the display opens its menu; a choice copies", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("49g")), "49g");
  await p.click("#lcd", "right");
  const items = await p.ev(`[...document.querySelectorAll(".menu [role=menuitem]")].map((b) => b.textContent)`);
  assert.deepEqual(items, ["Copy image", "Copy image (black on white)", "Save image…", "Save image (black on white)…"]);
  await p.click(".menu [role=menuitem]:nth-of-type(2)");
  const copied = await p.until("window.copied", "copied from the menu");
  assert.equal(copied.width, 524);
  assert.equal(await p.ev(`document.querySelector(".menu")`), null, "the menu closed");
  // A right-click on a key opens nothing.
  await p.click('.skey[data-key="enter"] .cap', "right");
  assert.equal(await p.ev(`document.querySelector(".menu")`), null);
});

test("where images cannot be copied, the screen is saved and the page says so", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48gx", { clipboard: false })), "48gx");
  await p.click("#copy-screen");
  const saved = await p.until("window.saved", "saved instead");
  assert.deepEqual([saved.width, saved.height], [524, 288]);
  assert.match(await p.ev("window.saturnus.store.state.message"), /cannot copy images.*saved as 48gx-/);
});

test("a long press on the display opens its menu on a phone; a short tap does not", { timeout: 120_000 }, async (t) => {
  const p = await page(t, 390);
  if (!p) return;
  await p.send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 1 });
  assert.equal(await p.ev(fake("48gx")), "48gx");
  const [x, y] = await p.ev(`(() => { const r = document.querySelector("#lcd").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  const touch = (type) => p.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y }] });
  await touch("touchStart");
  await sleep(150);
  await touch("touchEnd");
  await sleep(600);
  assert.equal(await p.ev(`document.querySelector(".menu")`), null, "a tap opens nothing");
  await touch("touchStart");
  await sleep(700);
  await touch("touchEnd");
  await p.until(`document.querySelectorAll(".menu [role=menuitem]").length === 4`, "the menu after a long press");
  await sleep(400);
  assert.equal(await p.ev(`document.querySelectorAll(".menu [role=menuitem]").length`), 4, "still open once the finger is up");
  // No wider than the phone.
  assert.ok(await p.ev(`document.querySelector(".menu").getBoundingClientRect().right <= innerWidth`));
});

test("without a screen to take the buttons are off, and the keys say so", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const off = () => p.ev(`[document.querySelector("#copy-screen").disabled, document.querySelector("#save-screen").disabled]`);
  assert.deepEqual(await off(), [true, true], "nothing runs");
  assert.equal(await p.ev(fake("48gx")), "48gx");
  assert.deepEqual(await off(), [false, false], "the 48GX runs");
  // Another model chosen: its skin is shown, the 48GX's screen is not.
  await p.ev(`window.saturnus.store.set({ model: "49g" }); true`);
  assert.deepEqual(await off(), [true, true], "another model shown");
  await p.send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "C", code: "KeyC", windowsVirtualKeyCode: 67, modifiers: 1 | 8 });
  await p.send("Input.dispatchKeyEvent", { type: "keyUp", key: "C", code: "KeyC", windowsVirtualKeyCode: 67, modifiers: 1 | 8 });
  await p.until(`window.saturnus.store.state.message === "No screen to take yet."`, "the status line says why");
  assert.equal(await p.ev("window.copied"), null, "nothing copied");
  await p.ev(`window.saturnus.store.set({ model: "48gx", message: null }); true`);
  assert.deepEqual(await off(), [false, false]);
  // Booted, no frame yet.
  await p.ev(`window.saturnus.store.set({ frame: null }); true`);
  assert.deepEqual(await off(), [true, true], "no frame yet");
  await p.ev(`document.querySelector("sat-calculator").saveScreen("lcd")`);
  assert.equal(await p.ev("window.saturnus.store.state.message"), "No screen to take yet.");
  assert.equal(await p.ev("window.saved"), null, "nothing saved");
});
