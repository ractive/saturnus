// Ctrl+click on a drawn key in Firefox, whose macOS build turns it into a
// secondary click: `mousedown` (button 2) and `contextmenu`, no
// `pointerdown`. The real page in headless Firefox over WebDriver BiDi
// (no dependencies); the page believes a 48GX runs and records what the
// backend is sent. Skipped without Firefox (`SATURNUS_FIREFOX` names
// one) or the wasm package (`just web`). On other systems Firefox sends
// a plain `pointerdown`; the expectations are the same.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const WEB = fileURLToPath(new URL("../", import.meta.url));
const MIME = { ".html": "text/html", ".css": "text/css", ".js": "text/javascript", ".json": "application/json", ".svg": "image/svg+xml", ".wasm": "application/wasm" };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findFirefox() {
  if (process.env.SATURNUS_FIREFOX) return process.env.SATURNUS_FIREFOX;
  for (const dir of (process.env.PATH ?? "").split(delimiter)) {
    if (dir && existsSync(join(dir, "firefox"))) return join(dir, "firefox");
  }
  const mac = "/Applications/Firefox.app/Contents/MacOS/firefox";
  return existsSync(mac) ? mac : null;
}

test("Ctrl+click in Firefox presses the key's left-shifted function once", { timeout: 120_000 }, async (t) => {
  const binary = findFirefox();
  if (!binary || !existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"))) {
    t.skip("no Firefox or no web/pkg");
    return;
  }
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
    const file = join(WEB, path === "/" ? "index.html" : path);
    if (!file.startsWith(WEB) || !existsSync(file) || !statSync(file).isFile()) return void res.writeHead(404).end();
    res.writeHead(200, { "content-type": MIME[extname(file)] ?? "application/octet-stream" });
    res.end(readFileSync(file));
  });
  server.unref();
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const profile = mkdtempSync(join(tmpdir(), "saturnus-firefox-"));
  const port = 20000 + Math.floor(Math.random() * 20000);
  const group = process.platform !== "win32";
  const proc = spawn(binary, ["--headless", "--remote-debugging-port", String(port), "--profile", profile, "--no-remote"], { stdio: "ignore", detached: group });
  let ws = null;
  t.after(() => {
    ws?.close();
    try { group ? process.kill(-proc.pid, "SIGKILL") : proc.kill("SIGKILL"); } catch { /* gone */ }
    server.close();
    setTimeout(() => rmSync(profile, { recursive: true, force: true }), 500);
  });
  for (let i = 0; i < 150 && !ws; i++) {
    try {
      const s = new WebSocket(`ws://127.0.0.1:${port}/session`);
      await new Promise((r, j) => { s.onopen = r; s.onerror = j; });
      ws = s;
    } catch { await sleep(200); }
  }
  if (!ws) {
    t.skip("Firefox's remote protocol did not answer");
    return;
  }
  let id = 0;
  const pending = new Map();
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); }
  };
  const send = (method, params) => new Promise((resolve, reject) => {
    const n = ++id;
    const timer = setTimeout(() => { pending.delete(n); reject(new Error(`${method} timed out`)); }, 30_000);
    pending.set(n, (d) => { clearTimeout(timer); resolve(d); });
    ws.send(JSON.stringify({ id: n, method, params }));
  });
  await send("session.new", { capabilities: {} });
  const ctx = (await send("browsingContext.getTree", {})).result.contexts[0].context;
  await send("browsingContext.setViewport", { context: ctx, viewport: { width: 1280, height: 900 } });
  await send("browsingContext.navigate", { context: ctx, url: `http://127.0.0.1:${server.address().port}/index.html`, wait: "complete" });
  const ev = async (expression) => {
    const r = await send("script.evaluate", { expression, target: { context: ctx }, awaitPromise: true });
    if (r.result?.type === "exception") throw new Error(JSON.stringify(r.result.exceptionDetails).slice(0, 400));
    return r.result?.result?.value;
  };
  for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started.then(() => true)");
  await ev(`(async () => {
    const s = window.saturnus;
    const b = s.backend;
    window.sent = [];
    b.keyDown = (k, shift = null) => window.sent.push("down " + k + (shift ? " after " + shift : ""));
    b.keyUp = (k) => window.sent.push("up " + k);
    b.keyUpAll = () => {};
    const annunciators = { leftshift: false, rightshift: false, alpha: false, alert: false, busy: false, transmit: false, updown: false, battery: false, g: false, rad: false };
    s.store.set({ booted: "48gx", model: "48gx", frame: { width: 131, height: 64, pixels: btoa("\\0".repeat(17 * 64)), annunciators, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 } });
    const calc = document.querySelector("sat-calculator");
    for (let i = 0; i < 100 && !(calc.skinModel === "48gx" && calc.skinKey("nxt")); i++) await new Promise((r) => setTimeout(r, 50));
    return true;
  })()`);
  const [x, y] = JSON.parse(await ev(`JSON.stringify((() => { const r = document.querySelector("sat-calculator").skinKey("nxt").querySelector(".cap").getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; })())`));
  const KEYS = { ctrl: "\uE009", alt: "\uE00A" };
  const click = async (mod, button = 0, keep = false) => {
    if (!keep) await ev("window.sent.length = 0");
    const key = mod ? [{ type: "key", id: "kb", actions: [{ type: "keyDown", value: KEYS[mod] }, { type: "pause", duration: 0 }, { type: "pause", duration: 0 }, { type: "pause", duration: 0 }, { type: "keyUp", value: KEYS[mod] }] }] : [];
    await send("input.performActions", { context: ctx, actions: [...key, { type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [...(mod ? [{ type: "pause", duration: 0 }] : []), { type: "pointerMove", x, y }, { type: "pointerDown", button }, { type: "pointerUp", button }, { type: "pause", duration: 0 }] }] });
    await send("input.releaseActions", { context: ctx });
    await sleep(300);
    return JSON.parse(await ev("JSON.stringify(window.sent)"));
  };
  assert.deepEqual(await click("ctrl"), ["down nxt after leftshift", "up nxt"], "Ctrl+click: once, left-shifted");
  assert.deepEqual(await click("alt"), ["down nxt after rightshift", "up nxt"], "Alt+click");
  assert.deepEqual(await click(null, 2), ["down nxt", "up nxt"], "a right-click stays a plain press");
  assert.deepEqual(await click(null), ["down nxt", "up nxt"], "a click");
  // A click, then at once a Ctrl+click on the same key: both.
  await click(null);
  assert.deepEqual(await click("ctrl", 0, true), ["down nxt", "up nxt", "down nxt after leftshift", "up nxt"], "click, then Ctrl+click");
});
