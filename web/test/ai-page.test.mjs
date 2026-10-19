// AI access in the real page (iteration 55): a local tab-mcp relay
// (`bun packages/tab-mcp/src/main.ts` from the tab-mcp checkout, sqlite
// store), the page pointed at it (localStorage `saturnus.aiRelay` on
// 127.0.0.1), connected by a click on the panel's "Connect your AI tool",
// and the MCP SDK client (tab-mcp's own test dependency) calling the tools
// with a token put into the relay's store as tab-mcp's tests do (its OAuth
// with the pairing code comes in its iteration 3). On a booted 48GX:
// run_rpl, press_keys, the stack, store_variable allowed in the page's
// confirmation, read_variable, purge_variable refused there, the hidden tab
// kept running while connected. On a booted 42S: run_function shows what
// the keys show. Skipped without Chrome, web/pkg, Bun, the tab-mcp
// checkout (TAB_MCP_DIR, default ~/devel/tab-mcp, with `bun install` done)
// or the ROMs (SATURNUS_ROM_DIR: gxrom-r, hp42s.rom).
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { createServer } from "node:net";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

const TAB_MCP = process.env.TAB_MCP_DIR ?? join(homedir(), "devel", "tab-mcp");
const RELAY_MAIN = join(TAB_MCP, "packages/tab-mcp/src/main.ts");
const STORE = join(TAB_MCP, "packages/tab-mcp/src/store/sqlite.ts");
const CLIENT_PKG = join(TAB_MCP, "packages/tab-mcp-client/package.json");
const dir = process.env.SATURNUS_ROM_DIR;

/** Why this file cannot run here, or null. */
function missing() {
  if (!dir || !existsSync(join(dir, "gxrom-r")) || !existsSync(join(dir, "hp42s.rom"))) return "SATURNUS_ROM_DIR with gxrom-r and hp42s.rom";
  if (spawnSync("bun", ["--version"]).status !== 0) return "Bun on the PATH";
  if (!existsSync(RELAY_MAIN) || !existsSync(CLIENT_PKG)) return `the tab-mcp checkout (TAB_MCP_DIR, ${TAB_MCP})`;
  try {
    createRequire(CLIENT_PKG).resolve("@modelcontextprotocol/client");
  } catch {
    return "tab-mcp's dependencies (bun install in TAB_MCP_DIR)";
  }
  return null;
}

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

/** A free TCP port on 127.0.0.1. */
function freePort() {
  return new Promise((resolve) => {
    const s = createServer();
    s.listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });
}

/** The relay as tab-mcp ships it, for the page at `pageOrigin`; stopped by the test. */
async function startRelay(t, pageOrigin) {
  const port = await freePort();
  const tmp = mkdtempSync(join(tmpdir(), "saturnus-relay-"));
  const db = join(tmp, "tab-mcp.db");
  const relay = spawn("bun", [RELAY_MAIN], {
    env: {
      ...process.env,
      NODE_ENV: "development",
      TAB_MCP_PUBLIC_URL: `http://127.0.0.1:${port}`,
      TAB_MCP_PORT: String(port),
      TAB_MCP_HOST: "127.0.0.1",
      TAB_MCP_ALLOWED_ORIGINS: pageOrigin,
      TAB_MCP_STORE: "sqlite",
      TAB_MCP_SQLITE_PATH: db,
      TAB_MCP_LOG_LEVEL: "warn",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  relay.stderr.on("data", (d) => t.diagnostic(`relay: ${String(d).trim()}`));
  t.after(() => {
    relay.kill("SIGTERM");
    rmSync(tmp, { recursive: true, force: true });
  });
  const url = `http://127.0.0.1:${port}`;
  for (let i = 0; ; i++) {
    try {
      if ((await fetch(`${url}/healthz`)).ok) break;
    } catch {
      // not up yet
    }
    if (i > 100) assert.fail("the relay did not start");
    await sleep(100);
  }
  return { url, db };
}

/** An access token for `address`, put into the relay's store as tab-mcp's tests do. */
function tokenFor(db, address) {
  const id = /\/a\/([a-z2-7]{52})\/mcp$/.exec(address)?.[1];
  assert.ok(id, `an address: ${address}`);
  const script = `
    import { SqliteTokenStore } from ${JSON.stringify(STORE)};
    const token = "tmcp_at_" + crypto.getRandomValues(new Uint8Array(32)).toBase64({ alphabet: "base64url", omitPadding: true });
    const hash = new Bun.CryptoHasher("sha256").update(token).digest().toBase64({ alphabet: "base64url", omitPadding: true });
    const now = Date.now();
    await new SqliteTokenStore(${JSON.stringify(db)}).put({ hash, kind: "access", grantId: "g_test", addressId: ${JSON.stringify(id)},
      clientId: "https://client.example/cimd.json", clientName: "Test client", scopes: ["tools"], grantCreatedAt: now, issuedAt: now, expiresAt: now + 3600000 });
    process.stdout.write(token);`;
  const r = spawnSync("bun", ["-e", script], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout;
}

/** The SDK client pinned to 2026-07-28 with a bearer token (tab-mcp's tests' `connectClient`). */
async function mcpClient(address, token) {
  const { Client, StreamableHTTPClientTransport } = createRequire(CLIENT_PKG)("@modelcontextprotocol/client");
  const client = new Client({ name: "saturnus-test", version: "1.0.0" }, { versionNegotiation: { mode: { pin: "2026-07-28" } } });
  await client.connect(new StreamableHTTPClientTransport(new URL(address), { requestInit: { headers: { Authorization: `Bearer ${token}` } } }));
  return client;
}

/** A tool's result: its JSON (the text block or structuredContent), and whether it is an error. */
async function use(client, name, args = {}) {
  const r = await client.callTool({ name, arguments: args });
  const text = r.content.find((c) => c.type === "text")?.text ?? "";
  let json = r.structuredContent ?? null;
  if (!json) {
    try {
      json = JSON.parse(text);
    } catch {
      json = null;
    }
  }
  return { r, json, text, error: Boolean(r.isError), image: r.content.find((c) => c.type === "image") ?? null };
}

/** Answer the page's confirmation: Allow once it is enabled, or Don't allow. */
function answer(ev, allow) {
  return ev(`new Promise((resolve) => {
    const look = () => {
      const d = document.querySelector('dialog.tab-mcp-confirm');
      if (!d) return setTimeout(look, 20);
      const b = [...d.querySelectorAll('button')].find((x) => x.textContent === ${JSON.stringify(allow ? "Allow" : "Don't allow")});
      const click = () => { const text = d.textContent; b.click(); resolve(text); };
      if (!b.disabled) return click();
      new MutationObserver((_, o) => { if (!b.disabled) { o.disconnect(); click(); } }).observe(b, { attributes: true });
    };
    look();
  })`);
}

test("an AI app drives the 48GX and the 42S through a local tab-mcp relay", { timeout: 300_000 }, async (t) => {
  const why = missing();
  if (why) {
    t.skip(`needs ${why}`);
    return;
  }
  const c = await session(t, { files: { "/__gx": join(dir, "gxrom-r"), "/__42s": join(dir, "hp42s.rom") } });
  if (!c) return;
  const { send, ev, port } = c;
  const origin = `http://127.0.0.1:${port}`;
  const relay = await startRelay(t, origin);
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 860, deviceScaleFactor: 1, mobile: false });
  // The development relay is named on the page's origin first, then the page loads with it.
  await send("Page.navigate", { url: `${origin}/index.html#/calculator` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev(`localStorage.setItem("saturnus.aiRelay", ${JSON.stringify(relay.url)}), true`);
  await send("Page.reload");
  await sleep(300);
  await until(ev, "!!window.saturnus && !!window.saturnusApp?.ai", 15_000, "the page started with AI access");
  await ev("window.saturnus.started.then(() => true)");
  await ev(`(async () => {
    const bytes = await (await fetch("/__gx")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "gxrom-r"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '48gx'", 30_000, "the 48GX booted");
  // Settled: its memory set up (the boot's question answered) and asleep, waiting for keys.
  await until(ev, "window.saturnus.backend.stack().then(() => window.saturnus.backend.stats()).then((s) => s.loop === 'sleep' && s.emulatedMs > 4000, () => false)", 30_000, "the 48GX settled");

  // Before a click, nothing of the client is loaded.
  assert.equal(await ev("performance.getEntriesByType('resource').some((e) => e.name.includes('/vendor/tab-mcp-client/'))"), false);
  assert.equal(await ev("document.getElementById('ai-connect').hidden"), false, "the panel offers it");
  // A real click on the panel's link: the dialog opens and connects.
  const box = await ev("(() => { const r = document.getElementById('ai-connect').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()");
  for (const type of ["mousePressed", "mouseReleased"]) await send("Input.dispatchMouseEvent", { type, x: box.x, y: box.y, button: "left", clickCount: 1 });
  await until(ev, "document.querySelector('dialog.ai[open]') && window.saturnusApp.ai.then((a) => a.state === 'connected')", 15_000, "connected from the dialog");
  const address = await ev("window.saturnusApp.ai.then((a) => a.mcp.address)");
  assert.equal(await ev("document.querySelector('dialog.ai .ai-address').value"), address, "the dialog shows the address");
  assert.match(await ev("document.querySelector('dialog.ai .ai-status').textContent"), /AI access is on/);
  await ev("document.querySelector('dialog.ai').close(), true");
  await until(ev, "!document.getElementById('ai-chip').hidden", 5_000, "the panel says AI access is on");

  const client = await mcpClient(address, tokenFor(relay.db, address));
  t.after(() => client.close());
  const names = (await client.listTools()).tools.map((x) => x.name);
  assert.deepEqual(names.sort(), ["list_calculators", "list_keys", "lookup_command", "press_keys", "purge_variable", "read_display", "read_stack", "read_variable", "run_function", "run_rpl", "store_variable"].concat("list_variables").sort());

  const list = await use(client, "list_calculators");
  assert.deepEqual(list.json.calculators.map((x) => [x.id, x.shown, x.family]), [["48gx", true, "rpl"]]);

  const ran = await use(client, "run_rpl", { text: "2 3 +" });
  assert.equal(ran.error, false, ran.text);
  assert.equal(ran.json.error, undefined);
  assert.equal(ran.json.stack[0].text, "5");

  const pressed = await use(client, "press_keys", { keys: ["4", "enter"] });
  assert.equal(pressed.error, false, pressed.text);
  assert.ok(pressed.image && pressed.image.mimeType === "image/png" && pressed.image.data.length > 100, "an image of the display");
  assert.deepEqual((await use(client, "read_stack", { levels: 2 })).json.levels.map((l) => l.text), ["4", "5"]);
  assert.equal((await use(client, "press_keys", { keys: ["nokey"] })).error, true);

  // store_variable waits for Allow in the page.
  const store = use(client, "store_variable", { name: "AIT", text: "« 2 * »" });
  assert.match(await answer(ev, true), /AIT/);
  const stored = await store;
  assert.equal(stored.json.stored, true, stored.text);
  assert.equal((await use(client, "read_variable", { name: "AIT" })).json.text.replace(/\s+/g, " "), "« 2 * »");
  assert.ok((await use(client, "list_variables")).json.home.some((v) => v.name === "AIT"));
  assert.equal((await use(client, "run_rpl", { text: "21 AIT" })).json.stack[0].text, "42");
  // purge_variable refused in the page: nothing purged.
  const purge = use(client, "purge_variable", { name: "AIT" });
  await answer(ev, false);
  assert.equal((await purge).error, true);
  assert.ok((await use(client, "list_variables")).json.home.some((v) => v.name === "AIT"));

  const look = await use(client, "lookup_command", { name: "->LIST" });
  assert.equal(look.json.name, "→LIST");

  // Hidden while connected: the calculator is not told to stop.
  await ev(`(() => {
    const b = window.saturnus.backend;
    window.__vis = [];
    const orig = b.visibility.bind(b);
    b.visibility = (h) => { window.__vis.push(h); orig(h); };
    Object.defineProperty(document, "hidden", { configurable: true, get: () => true });
    document.dispatchEvent(new Event("visibilitychange"));
    return true;
  })()`);
  assert.deepEqual(await ev("window.__vis"), [false]);
  const looped = await use(client, "run_rpl", { text: "« 1 300 START NEXT » EVAL 7" });
  assert.equal(looped.json.stack[0].text, "7", "it ran while the tab was hidden");
  assert.equal(looped.json.running, false);

  // The 42S in Calculator: run_function presses √x's keys, as pressing them shows.
  await ev(`(() => { delete document.hidden; document.dispatchEvent(new Event("visibilitychange")); return true; })()`);
  // Files keeps the 48GX running meanwhile: the AI app reaches it by its id.
  await ev(`(location.hash = "#/files/48gx", true)`);
  await until(ev, "window.saturnusApp.view === 'files' && document.querySelector('#files-view [data-side=left], #files-view .pane') && window.saturnusApp.mounted('files').then(() => true)", 15_000, "Files shown");
  await sleep(3000);
  await ev(`(location.hash = "#/calculator", true)`);
  await until(ev, "window.saturnusApp.view === 'calculator'", 10_000, "Calculator shown again");
  await ev(`(async () => {
    const bytes = await (await fetch("/__42s")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "hp42s.rom"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '42s'", 30_000, "the 42S booted");
  await sleep(1500);
  assert.deepEqual((await use(client, "list_calculators")).json.calculators.map((x) => [x.id, x.shown, x.family]), [["42s", true, "keys"], ["48gx", false, "rpl"]]);
  const other = await use(client, "run_rpl", { calculator: "48gx", text: "6 7 *" });
  assert.equal(other.error, false, other.text);
  assert.equal(other.json.stack[0].text, "42", "the 48GX Files shows, through the AI app's own connection");
  assert.equal(other.json.calculator, "48gx");
  assert.equal((await use(client, "run_rpl", { text: "1" })).error, true, "no RPL command line on the 42S");
  // "Memory Clear" stays until a key: EXIT.
  await use(client, "press_keys", { keys: ["on"], display: "none" });
  await use(client, "press_keys", { keys: ["9"], display: "none" });
  const fn = await use(client, "run_function", { name: "SQRT", display: "text" });
  assert.equal(fn.error, false, fn.text);
  assert.equal(fn.json.keys, "√x");
  const after = (await use(client, "read_display", { format: "text", pixels: true })).json.pixels;
  await use(client, "press_keys", { keys: ["on", "9", "sqrt"], display: "none" });
  const byHand = (await use(client, "read_display", { format: "text", pixels: true })).json.pixels;
  // The x line (rows 8-15; y differs: the first result was lifted).
  assert.deepEqual(after.slice(8), byHand.slice(8), "run_function shows what its keys show");
  assert.notDeepEqual(after.slice(8), (await use(client, "press_keys", { keys: ["9"], display: "text" }), (await use(client, "read_display", { format: "text", pixels: true })).json.pixels.slice(8)));
  const shot = await use(client, "read_display", { format: "image" });
  assert.equal(shot.image?.mimeType, "image/png");

  // Turned off in the page: the calculator is told about the hidden tab again.
  await ev("window.saturnusApp.ai.then((a) => (a.disconnect(), true))");
  await until(ev, "window.saturnusApp.ai.then((a) => a.state === 'disconnected')", 5_000, "disconnected");
  assert.equal(await ev("document.getElementById('ai-chip').hidden"), true);
});
