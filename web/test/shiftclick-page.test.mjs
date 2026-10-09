// Ctrl+click and Option/Alt+click on the drawn keys, and the shift glow,
// in the real page in headless Chrome over the DevTools protocol (no
// dependencies), with real mouse and key events. No ROM: the test makes
// the page believe a model runs (`booted` and `model` set, a blank frame
// with the annunciators it wants) and records the key presses the
// backend is sent. Skipped without Chrome (`SATURNUS_CHROME` names one)
// or the wasm package (`just web`). With `SATURNUS_ROM_DIR`, a 48SX booted
// from its `sxrom-j` takes Ctrl+click and Alt+click on √x as x² and ˣ√y.
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
function serve(rom = null) {
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
    if (rom && path === "/__rom") {
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
async function session(t, rom = null) {
  const binary = findChrome();
  const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));
  if (!binary || !built) {
    const why = !binary ? "no Chrome found (SATURNUS_CHROME=/path/to/chrome)" : "web/pkg not built (just web)";
    if (process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return null;
  }
  const { server, port } = await serve(rom);
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
 * Make the page believe `model` runs, with the shift annunciators of
 * `ann`, and record what the backend is sent (`window.sent`).
 */
const fake = (model, ann = {}) => `(async () => {
  const s = window.saturnus;
  const b = s.backend;
  window.sent = [];
  b.keyDown = (k) => window.sent.push("down " + k);
  b.keyUp = (k) => window.sent.push("up " + k);
  b.keyUpAll = () => {};
  const rows = ${model === "42s" ? 16 : 64};
  const annunciators = { leftshift: false, rightshift: false, alpha: false, alert: false, busy: false, transmit: false, updown: false, battery: false, g: false, rad: false, ...${JSON.stringify(ann)} };
  s.store.set({ booted: "${model}", model: "${model}", frame: { width: 131, height: rows, pixels: btoa("\\0".repeat(17 * rows)), annunciators, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 } });
  const calc = document.querySelector("sat-calculator");
  for (let i = 0; i < 100 && !(calc.skinModel === "${model}" && calc.skinKey("${model === "49g" ? "nxt" : "enter"}")); i++) await new Promise((r) => setTimeout(r, 50));
  return calc.skinModel;
})()`;

/** The page in headless Chrome at 1280 x 900, or null when the test skipped. */
async function page(t, rom = null) {
  const c = await session(t, rom);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  /** Modifier bits of the DevTools protocol: Alt 1, Ctrl 2, Meta 4, Shift 8. */
  const MOD = { alt: 1, ctrl: 2 };
  /** A real mouse click in the middle of key `name`, with `mod` ("ctrl", "alt") held. */
  const click = async (name, mod = null, button = "left") => {
    const [x, y] = await ev(`(() => { const r = document.querySelector("sat-calculator").skinKey(${JSON.stringify(name)}).querySelector(".cap").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    const modifiers = mod ? MOD[mod] : 0;
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, modifiers });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount: 1, modifiers });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount: 1, modifiers });
    await sleep(50);
  };
  const KEYS = { ctrl: ["Control", "ControlLeft", 17], alt: ["Alt", "AltLeft", 18] };
  /** Modifier `mod` down or up. */
  const modifier = async (mod, down) => {
    const [key, code, vk] = KEYS[mod];
    await send("Input.dispatchKeyEvent", { type: down ? "rawKeyDown" : "keyUp", key, code, windowsVirtualKeyCode: vk, modifiers: down ? MOD[mod] : 0 });
  };
  /** A letter key down and up with `mod` held. */
  const chord = async (mod, letter) => {
    const code = `Key${letter.toUpperCase()}`;
    const vk = letter.toUpperCase().charCodeAt(0);
    await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: letter, code, windowsVirtualKeyCode: vk, modifiers: MOD[mod] });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: letter, code, windowsVirtualKeyCode: vk, modifiers: MOD[mod] });
  };
  const sent = () => ev("window.sent");
  const glow = () => ev(`["glow-left", "glow-right"].filter((c) => document.querySelector(".skin svg").classList.contains(c))`);
  return { ...c, click, modifier, chord, sent, glow };
}

test("Ctrl+click and Alt+click tap the shift first, unless it is on", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("49g")), "49g");
  await p.click("nxt", "ctrl");
  assert.deepEqual(await p.sent(), ["down leftshift", "up leftshift", "down nxt", "up nxt"], "Ctrl+click NXT: left shift, then NXT");

  await p.ev("window.sent = []");
  await p.click("nxt", "alt");
  assert.deepEqual(await p.sent(), ["down rightshift", "up rightshift", "down nxt", "up nxt"], "Alt+click NXT: right shift, then NXT");

  // A Mac's Ctrl+click is a secondary click: the same press.
  await p.ev("window.sent = []");
  await p.click("nxt", "ctrl", "right");
  assert.deepEqual(await p.sent(), ["down leftshift", "up leftshift", "down nxt", "up nxt"], "a secondary Ctrl+click");

  await p.ev("window.sent = []");
  await p.click("nxt");
  assert.deepEqual(await p.sent(), ["down nxt", "up nxt"], "a plain click");

  // The left shift on already: only the key.
  await p.ev(fake("49g", { leftshift: true }));
  await p.click("nxt", "ctrl");
  assert.deepEqual(await p.sent(), ["down nxt", "up nxt"], "the shift is not pressed twice");
  // The right one is not the left one.
  await p.ev("window.sent = []");
  await p.click("nxt", "alt");
  assert.deepEqual(await p.sent(), ["down rightshift", "up rightshift", "down nxt", "up nxt"]);
});

test("on the 39G both modifiers tap its one shift", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("39g")), "39g");
  await p.click("enter", "ctrl");
  assert.deepEqual(await p.sent(), ["down shift", "up shift", "down enter", "up enter"]);
  await p.ev("window.sent = []");
  await p.click("enter", "alt");
  assert.deepEqual(await p.sent(), ["down shift", "up shift", "down enter", "up enter"]);
  // Its shift on (whichever annunciator the ROM lights): only the key.
  await p.ev(fake("39g", { leftshift: true }));
  await p.click("enter", "alt");
  assert.deepEqual(await p.sent(), ["down enter", "up enter"]);
});

test("a held modifier lights its labels; a chord and a blur do not keep them", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("49g")), "49g");
  await p.modifier("ctrl", true);
  assert.deepEqual(await p.glow(), [], "not at once");
  await sleep(250);
  assert.deepEqual(await p.glow(), ["glow-left"], "Ctrl held: the left labels");
  assert.ok(await p.ev(`getComputedStyle(document.querySelector(".skin .shift-left")).filter.includes("glow-left")`), "the left labels glow");
  await p.modifier("ctrl", false);
  assert.deepEqual(await p.glow(), [], "out on its release");

  await p.modifier("alt", true);
  await sleep(250);
  assert.deepEqual(await p.glow(), ["glow-right"], "Alt held: the right labels");
  // The window losing the focus puts it out.
  await p.ev(`window.dispatchEvent(new Event("blur")), true`);
  assert.deepEqual(await p.glow(), [], "out on blur");
  await p.modifier("alt", false);

  // A quick chord never lights.
  await p.modifier("ctrl", true);
  await p.chord("ctrl", "k");
  await sleep(250);
  assert.deepEqual(await p.glow(), [], "Ctrl+K does not glow");
  await p.modifier("ctrl", false);
  await p.ev(`document.querySelector("sat-palette dialog")?.open && document.querySelector("sat-palette").close?.(); true`);

  // A mouse event without the modifier puts it out (its keyup was missed).
  await p.modifier("alt", true);
  await sleep(250);
  assert.deepEqual(await p.glow(), ["glow-right"]);
  await p.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: 5, y: 5, modifiers: 0 });
  assert.deepEqual(await p.glow(), [], "out when a move shows Alt up");
  await p.modifier("alt", false);

  // The 39G: either modifier lights its one shift's labels.
  assert.equal(await p.ev(fake("39g")), "39g");
  await p.modifier("alt", true);
  await sleep(250);
  assert.deepEqual(await p.glow(), ["glow-left"], "Alt on the 39G: its shift's labels");
  await p.modifier("alt", false);
});

test("a lone Alt's release is kept from the menu bar; typing in a field gets no glow", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("49g")), "49g");
  await p.ev(`window.altUp = null; window.addEventListener("keyup", (e) => { if (e.key === "Alt") window.altUp = e.defaultPrevented; }); true`);
  await p.modifier("alt", true);
  await p.modifier("alt", false);
  assert.equal(await p.ev("window.altUp"), true, "a lone Alt's keyup prevented");
  await p.modifier("alt", true);
  await p.chord("alt", "x");
  await p.modifier("alt", false);
  assert.equal(await p.ev("window.altUp"), false, "Alt+X's release left alone");

  // In a text field the keys are the field's.
  await p.ev(`(() => { const i = document.createElement("input"); i.id = "field"; document.body.append(i); i.focus(); return true; })()`);
  await p.modifier("ctrl", true);
  await sleep(250);
  assert.deepEqual(await p.glow(), [], "no glow while a field has the keys");
  await p.modifier("ctrl", false);
});

test("the 48SX ROM takes Ctrl+click √x as x² and Alt+click as ˣ√y", { timeout: 180_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "sxrom-j") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/sxrom-j not found");
    return;
  }
  const p = await page(t, rom);
  if (!p) return;
  // Idle: the CPU asleep with no key down, `ms` emulated ms on.
  await p.ev(`window.__idle = async (ms = 1500) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (;;) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  }; true`);
  await p.ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "sxrom-j"));
    return true;
  })()`);
  for (let i = 0; i < 150 && (await p.ev("window.saturnus.store.state.booted")) !== "48sx"; i++) await sleep(100);
  await p.ev("window.__idle()");
  // NO to "Try To Recover Memory?", then 3 ENTER, by clicks.
  for (const k of ["f", "3", "enter"]) {
    await p.click(k);
    await p.ev("window.__idle(300)");
  }
  const stack = () => p.ev("window.saturnus.backend.stack().then((s) => s.map((l) => l.text))");
  assert.deepEqual(await stack(), ["3"]);
  await p.click("sqrt", "ctrl");
  await p.ev("window.__idle(300)");
  assert.deepEqual(await stack(), ["9"], "Ctrl+click √x: x²");
  await p.click("2");
  await p.ev("window.__idle(300)");
  await p.click("sqrt", "alt");
  await p.ev("window.__idle(300)");
  assert.deepEqual(await stack(), ["3"], "Alt+click √x: the square root of 9");
});
