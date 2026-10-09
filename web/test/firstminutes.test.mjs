// The first minutes with the page, in headless Chrome over the DevTools
// protocol (no dependencies): Forget ROMs asks first; the status line
// speaks plainly and an outcome goes after a while; the fullscreen
// refusal is the page's sentence, not the browser's. With
// `SATURNUS_ROM_DIR` (`gxrom-r`, `rom-2.10.49g`): a ROM's first boot
// lands on an empty stack that takes typing, with no key pressed, and
// the offer to keep the ROM waits in the panel until the calculator is
// idle. Skipped without Chrome (`SATURNUS_CHROME` names one) or the wasm
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
    // The owner's ROMs, by name, when SATURNUS_ROM_DIR is set.
    const rom = /^[/\\]__rom[/\\]([\w.-]+)$/.exec(path)?.[1];
    if (rom && process.env.SATURNUS_ROM_DIR && existsSync(join(process.env.SATURNUS_ROM_DIR, rom))) {
      res.writeHead(200, { "content-type": "application/octet-stream" });
      res.end(readFileSync(join(process.env.SATURNUS_ROM_DIR, rom)));
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


/** The page in headless Chrome at `width` x `height`, or null when the test skipped. */
async function page(t, width = 1280, height = 900) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: width < 500 });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const until = async (expr, what, ms = 10_000) => {
    const end = Date.now() + ms;
    for (;;) {
      const v = await ev(expr).catch(() => null);
      if (v) return v;
      if (Date.now() > end) assert.fail(`timed out: ${what}`);
      await sleep(100);
    }
  };
  return { ...c, until };
}

test("Forget ROMs asks first; Cancel keeps them", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`(() => {
    window.__forgot = 0;
    const s = window.saturnus.store;
    window.saturnus.backend.forgetRom = async () => { window.__forgot++; return s.state.roms; };
    // A ROM kept in the first slot: Forget ROMs is on.
    s.set({ roms: { ...s.state.roms, slots: s.state.roms.slots.map((x, i) => (i ? x : { ...x, fileName: "sxrom-j", state: "ready" })) } });
    document.getElementById("roms").open = true;
    return true;
  })()`);
  assert.equal(await p.ev(`document.getElementById("rom-forget").disabled`), false);
  assert.equal(await p.ev(`document.getElementById("rom-forget").textContent`), "Forget ROMs…");
  await p.ev(`document.getElementById("rom-forget").click(); true`);
  const card = await p.until(`(() => { const c = document.querySelector(".fresh-notice"); return c && { text: c.querySelector("p").textContent, buttons: [...c.querySelectorAll("button")].map((b) => b.textContent) }; })()`, "the question");
  assert.equal(card.text, "Forget all kept ROMs? The saved 49G state goes too, as it contains the ROM. Other saved states stay.");
  assert.deepEqual(card.buttons, ["Cancel", "Forget ROMs"]);
  await p.ev(`[...document.querySelectorAll(".fresh-notice button")].find((b) => b.textContent === "Cancel").click(); true`);
  await sleep(200);
  assert.equal(await p.ev("window.__forgot"), 0, "Cancel forgets nothing");
  await p.ev(`document.getElementById("rom-forget").click(); true`);
  await p.until(`!!document.querySelector(".fresh-notice")`, "asked again");
  await p.ev(`[...document.querySelectorAll(".fresh-notice button")].find((b) => b.textContent === "Forget ROMs").click(); true`);
  await p.until("window.__forgot === 1", "forgotten");
  assert.match(await p.ev("window.saturnus.store.state.message"), /^ROMs and the saved 49G state forgotten\. Other saved states stay\.$/);
});

test("the status line: plain words, an outcome goes, an error waits for the next action", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  // The browser's refusal of fullscreen, in the page's words.
  await p.ev(`document.getElementById("stage").requestFullscreen = () => Promise.reject(new TypeError("Failed to execute 'requestFullscreen' on 'Element': API can only be initiated by a user gesture.")); true`);
  await p.ev(`document.getElementById("fullscreen").click(); true`);
  await p.until(`window.saturnus.store.state.message === "Fullscreen is not allowed here."`, "the refusal");
  assert.equal(await p.ev(`document.getElementById("status").textContent`), "Fullscreen is not allowed here.");
  assert.equal(await p.ev("window.saturnus.store.state.messageError"), true, "an error: it waits for the next action");
  // An outcome goes after about 6 s.
  await p.ev(`window.saturnus.store.set({ message: "Screen copied as an image.", messageError: false }); true`);
  await sleep(6_500);
  assert.equal(await p.ev("window.saturnus.store.state.message"), "", "gone after 6 s");
  // An error stays, and goes with the next click.
  await p.ev(`window.saturnus.store.set({ message: "Could not save the state: quota", messageError: true }); true`);
  await sleep(7_000);
  assert.equal(await p.ev("window.saturnus.store.state.message"), "Could not save the state: quota");
  await p.send("Input.dispatchMouseEvent", { type: "mousePressed", x: 5, y: 5, button: "left", clickCount: 1 });
  await p.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: 5, y: 5, button: "left", clickCount: 1 });
  assert.equal(await p.ev("window.saturnus.store.state.message"), "");
});

for (const [model, file] of [["48gx", "gxrom-r"], ["49g", "rom-2.10.49g"]]) {
  test(`the ${model}'s first boot lands on an empty stack; the keep offer waits in the panel`, { timeout: 180_000 }, async (t) => {
    const dir = process.env.SATURNUS_ROM_DIR;
    if (!dir || !existsSync(join(dir, file))) {
      t.skip(`SATURNUS_ROM_DIR/${file} not found`);
      return;
    }
    const p = await page(t);
    if (!p) return;
    // When the screen last changed, and how long it had been still when the offer came.
    await p.ev(`(() => {
      window.__lastFrame = performance.now();
      window.__stillFor = null;
      window.saturnus.store.watch(["frame"], () => { window.__lastFrame = performance.now(); });
      new MutationObserver(() => {
        if (window.__stillFor === null && document.querySelector(".storage-notice")) window.__stillFor = performance.now() - window.__lastFrame;
      }).observe(document.body, { childList: true, subtree: true });
      return true;
    })()`);
    await p.ev(`window.__keys = []; const b = window.saturnus.backend; const down = b.keyDown.bind(b); b.keyDown = (...a) => { window.__keys.push(a[0]); return down(...a); }; true`);
    await p.ev(`(async () => {
      window.saturnus.store.set({ model: "${model}" });
      const bytes = await (await fetch("/__rom/${file}")).arrayBuffer();
      await window.saturnus.startWithRom(new File([bytes], "${file}"));
      return true;
    })()`);
    await p.until(`window.saturnus.store.state.booted === "${model}"`, "booted", 30_000);
    // No key from the page: the host answered the question itself.
    await p.until(`window.saturnus.backend.stack().then((s) => Array.isArray(s) && s.length === 0, () => false)`, "an empty stack", 60_000);
    // Settled: asleep for a while with no key (the 49G's "Memory Clear" dismissed too).
    await p.ev(`(async () => {
      const b = window.saturnus.backend;
      const t0 = (await b.stats()).emulatedMs;
      for (;;) {
        const s = await b.stats();
        if (s.emulatedMs - t0 >= 1500 && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
        await new Promise((r) => setTimeout(r, 50));
      }
    })()`);
    assert.deepEqual(await p.ev("window.__keys"), [], "no key pressed by the page");
    const r = await p.ev(`window.saturnus.backend.run("7")`);
    assert.equal(r.error, null);
    assert.equal(await p.ev(`window.saturnus.backend.stack().then((s) => s[0].text)`), "7");
    // The offer to keep the ROM, in the panel, once the screen is still.
    const offer = await p.until(`(() => { const n = document.querySelector(".storage-notice"); return n && { inPanel: !!n.closest("sat-controls") }; })()`, "the keep offer", 30_000);
    assert.equal(offer.inPanel, true);
    assert.ok(await p.ev("window.__stillFor") >= 1900, "offered once the screen had been still for 2 s");
  });
}
