// Headless Chrome for the page tests, over the DevTools protocol (no
// dependencies), and a static server for web/. Not a test itself (the
// runner takes `*.test.mjs`). Hardened after a hung CI run (kb
// decision-log, 2026-10-08, no hung page tests): Chrome runs in its own
// process group and is killed on every path, the launch and every call
// have their own timeout, and a failed launch says why.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

/** web/ as a filesystem path ending in a separator (no %20, no "/C:/" on Windows). */
export const WEB = fileURLToPath(new URL("../", import.meta.url));
const MIME = { ".html": "text/html", ".css": "text/css", ".js": "text/javascript", ".mjs": "text/javascript", ".json": "application/json", ".svg": "image/svg+xml", ".wasm": "application/wasm" };
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** How long Chrome may take to start, and any one DevTools call to answer. */
export const START_MS = 30_000;
export const CALL_MS = 30_000;

/** `SATURNUS_CHROME`, a Chrome on the PATH or the macOS app; null when none. */
export function findChrome() {
  if (process.env.SATURNUS_CHROME) return process.env.SATURNUS_CHROME;
  const names = ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "chrome"];
  for (const dir of (process.env.PATH ?? "").split(delimiter)) {
    for (const n of names) if (dir && existsSync(join(dir, n))) return join(dir, n);
  }
  const mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
  return existsSync(mac) ? mac : null;
}

/**
 * A static server for web/ on a free port; resolves to the server and
 * its port. `files` maps further URL paths to files on disk, served as
 * bytes (a ROM, say).
 */
export function serve(files = {}) {
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
    const extra = Object.hasOwn(files, path) ? files[path] : null;
    if (extra) {
      res.writeHead(200, { "content-type": "application/octet-stream" });
      res.end(readFileSync(extra));
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

/** Chrome's start failed before it opened a DevTools endpoint: worth one more try. */
class NotStarted extends Error {}

/**
 * Headless Chrome with one page over the DevTools protocol. Chrome runs
 * in its own process group (POSIX), and `close` kills the whole group:
 * a wrapper script's children, the crashpad handler and the helpers
 * included. A Chrome left running keeps this file's process, and with it
 * `node --test`, alive for good. Every call fails after `CALL_MS`, so no
 * await outlives a dead or stuck browser.
 */
async function start(binary) {
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
    // Chrome creates the file before it writes the port into it: read
    // only a first line that is a port (an empty one fetched port 80).
    const portFile = join(dir, "DevToolsActivePort");
    const readPort = () => {
      try {
        const line = readFileSync(portFile, "utf8").split("\n")[0].trim();
        return /^\d+$/.test(line) ? line : null;
      } catch {
        return null; // not there yet
      }
    };
    let dport = readPort();
    for (let i = 0; i < START_MS / 100 && exited === null && !dport; i++) {
      await sleep(100);
      dport = readPort();
    }
    if (!dport) {
      throw new NotStarted(`Chrome did not start in ${START_MS / 1000} s (${exited === null ? "still running" : `exited: ${exited}`}): ${stderr.trim().slice(-800)}`);
    }
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
 * `start`, tried once more with a fresh profile when Chrome opened no
 * DevTools endpoint in `START_MS` (a cold CI runner can be that slow);
 * `log` hears of the retry. At most two starts, each closed if it fails.
 */
export async function chrome(binary, log = console.error) {
  try {
    return await start(binary);
  } catch (e) {
    if (!(e instanceof NotStarted)) throw e;
    log(`retrying the Chrome start once: ${e.message}`);
    return await start(binary);
  }
}

/**
 * Chrome and the server for a test, or null when the test skipped (no
 * Chrome, no wasm package; with `audit`, `SATURNUS_AUDIT` makes the skip
 * a failure). Both are closed when the test ends, passed, failed or
 * timed out (the test's signal), so neither outlives it; `close` may be
 * called earlier too. `files` as for `serve`.
 */
export async function session(t, { files = {}, audit = false } = {}) {
  const binary = findChrome();
  const built = existsSync(join(WEB, "pkg", "saturnus_web_bg.wasm"));
  if (!binary || !built) {
    const why = !binary ? "no Chrome found (SATURNUS_CHROME=/path/to/chrome)" : "web/pkg not built (just web)";
    if (audit && process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return null;
  }
  const { server, port } = await serve(files);
  const stop = () => {
    server.closeAllConnections();
    server.close();
  };
  let c;
  try {
    c = await chrome(binary, (m) => t.diagnostic(m));
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
  return { ...c, close, port };
}
