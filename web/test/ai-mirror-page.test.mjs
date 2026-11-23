// The calculators' tools through WebMCP in the real page (iteration 78):
// tab-mcp's client mirrors its registry into WebMCP, here a fake one
// given by a `modelContext` getter on `Document.prototype` before the
// app's scripts run (the getter the library reads; it records each
// `registerTool` and its signal, and calls a tool as an agent would):
// - on load without a relay (127.0.0.1 has none) the client loads at once
//   and offers list_calculators, lookup_command and read_guide; the
//   sidebar offers no Connect, "Calls" once an agent called; the dialog's
//   WebMCP note;
// - with the ROMs (SATURNUS_ROM_DIR: gxrom-r): the 48GX booted, its
//   family's tools; run_rpl, read_display with an image, store_variable
//   with no question in the page.
// Skipped without Chrome or the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

const dir = process.env.SATURNUS_ROM_DIR;

/** A fake WebMCP before the app's scripts: `window.webmcp` has its tools, its registrations and an agent's `run`. */
const FAKE_WEBMCP = `(() => {
  const tools = new Map();
  const registrations = [];
  const context = {
    registerTool(tool, { signal } = {}) {
      if (tools.has(tool.name)) throw new DOMException(tool.name + " is registered", "InvalidStateError");
      if (signal?.aborted) throw signal.reason;
      registrations.push({ name: tool.name, signal });
      tools.set(tool.name, tool);
      signal?.addEventListener("abort", () => { if (tools.get(tool.name) === tool) tools.delete(tool.name); }, { once: true });
    },
  };
  Object.defineProperty(Document.prototype, "modelContext", { get: () => context, configurable: true });
  window.webmcp = {
    tools,
    registrations,
    names: () => [...tools.keys()].sort().join(" "),
    run: async (name, input = {}) => {
      const tool = tools.get(name);
      if (!tool) throw new Error("no tool " + name);
      return JSON.parse(JSON.stringify(await tool.execute(JSON.parse(JSON.stringify(input)), {})));
    },
  };
})();`;

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

/** The page with the fake WebMCP, or null (skipped). */
async function webMcpPage(t, { late = false, route = "#/calculator", relay = null, ...opts } = {}) {
  const c = await session(t, opts);
  if (!c) return null;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 860, deviceScaleFactor: 1, mobile: false });
  if (!late) await c.send("Page.addScriptToEvaluateOnNewDocument", { source: FAKE_WEBMCP });
  await c.send("Page.navigate", { url: `http://127.0.0.1:${c.port}/index.html${route}` });
  await until(c.ev, "!!window.saturnusApp", 15_000, "the page started");
  if (relay) {
    await c.ev(`localStorage.setItem("saturnus.aiRelay", ${JSON.stringify(relay)}), true`);
    await c.send("Page.reload");
    await sleep(300);
    await until(c.ev, "!!window.saturnusApp", 15_000, "the page started again");
  }
  return c;
}

/**
 * The tab-mcp extension giving the page WebMCP after load (a click on its
 * icon): the getter appears, then its event (client 0.24's WEBMCP_EVENT).
 */
const GIVE_WEBMCP = `(${FAKE_WEBMCP.replace(/;$/, "")}, document.dispatchEvent(new Event("tab-mcp-webmcp")), true)`;

const run = (name, input = {}) => `window.webmcp.run(${JSON.stringify(name)}, ${JSON.stringify(input)})`;
/** The scripts the page loaded, by file name. */
const LOADED = "performance.getEntriesByType('resource').map((e) => new URL(e.name).pathname.split('/').pop())";
/** Whether the page shows a confirmation (tab-mcp's dialog) now. */
const ASKING = "!!document.querySelector('dialog.tab-mcp-confirm')";

test("the client mirrors the tools into WebMCP on load, without a relay; Calls once an agent called; the dialog's note", { timeout: 60_000 }, async (t) => {
  const c = await webMcpPage(t);
  if (!c) return;
  const { ev } = c;
  assert.equal(await until(ev, "window.webmcp.names().includes('read_guide') && window.webmcp.names()", 15_000, "the tools offered"), "list_calculators lookup_command read_guide");
  const loaded = await ev(LOADED);
  assert.ok(loaded.includes("tab-mcp-client.min.js") && loaded.some((f) => /^webmcp-.*\.min\.js$/.test(f)), loaded.join(" "));
  assert.ok(!loaded.some((f) => /^connection-/.test(f)), "the call's chunk only with a call");
  assert.ok(!loaded.includes("ai-local.js") && !loaded.some((f) => /^allowed/.test(f)), "neither the desktop app's code nor the relay's Always allow store");
  const tools = await ev("[...window.webmcp.tools.values()].map((x) => ({ name: x.name, title: x.title, description: x.description, annotations: x.annotations }))");
  for (const tool of tools) assert.ok(tool.description.length <= 1000, `${tool.name}: ${tool.description.length} characters`);
  const list = tools.find((x) => x.name === "list_calculators");
  assert.equal(list.title, "List calculators");
  assert.equal(list.annotations.readOnlyHint, true);

  const none = await ev(run("list_calculators"));
  assert.deepEqual(none.calculators, []);
  assert.match(none.note, /No calculator is running/);
  assert.match(none.instructions, /^Saturnus runs emulated HP calculators/);
  const guide = await ev(run("read_guide", { topic: "workflows" }));
  assert.match(guide.text, /^# Workflows\n/);
  const sin = await ev(run("lookup_command", { model: "48gx", name: "SIN" }));
  assert.equal(sin.name, "SIN");

  // No relay: nothing to connect, the entry stays hidden; the line below it offers the calls.
  assert.equal(await ev("document.getElementById('ai-connect').hidden"), true);
  await until(ev, "[...document.querySelectorAll('#ai-chip button')].some((b) => b.textContent === 'Calls')", 5_000, "Calls in the sidebar");
  await ev("[...document.querySelectorAll('#ai-chip button')].find((b) => b.textContent === 'Calls').click(), true");
  await until(ev, "!document.getElementById('ai-log').hidden", 5_000, "the drawer open");
  const rows = await ev("[...document.querySelectorAll('#ai-log .al-entry')].map((li) => li.textContent)");
  assert.equal(rows.length, 3);
  for (const row of rows) assert.match(row, /Your browser's AI/);

  // "Connect your AI app" says the browser's AI has the tools too.
  await ev(`Promise.all([window.saturnusApp.ai, import("/ai-dialog.js")]).then(([a, d]) => { d.openAiDialog(a); return true; })`);
  const note = await until(ev, "(() => { const n = document.querySelector('dialog.ai .ai-webmcp'); return n && !n.hidden && n.textContent; })()", 5_000, "the dialog's WebMCP note");
  assert.match(note, /^This browser also offers the calculators to its own AI \(WebMCP\)/);
});

test("an agent reads the 48GX's display, runs RPL and stores, the family's tools offered once it boots", { timeout: 180_000 }, async (t) => {
  if (!dir || !existsSync(join(dir, "gxrom-r"))) {
    t.skip("needs SATURNUS_ROM_DIR with gxrom-r");
    return;
  }
  const c = await webMcpPage(t, { files: { "/__gx": join(dir, "gxrom-r") } });
  if (!c) return;
  const { ev } = c;
  // Calculator sets window.saturnus once mounted: under load that comes after the page's first script.
  await until(ev, "window.saturnus?.started.then(() => true)", 30_000, "Calculator mounted");
  await ev(`(async () => {
    const bytes = await (await fetch("/__gx")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "gxrom-r"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '48gx'", 30_000, "the 48GX booted");
  await until(ev, "window.saturnus.backend.stack().then(() => window.saturnus.backend.stats()).then((s) => s.loop === 'sleep' && s.emulatedMs > 4000, () => false)", 30_000, "the 48GX settled");
  await until(ev, "window.webmcp.names().includes('run_rpl')", 10_000, "the RPL tools");
  assert.equal(await ev("window.webmcp.names()"), "list_calculators list_keys list_variables lookup_command press_keys purge_variable read_display read_guide read_stack read_variable run_rpl store_variable to_stack");

  const list = await ev(run("list_calculators"));
  assert.deepEqual(list.calculators.map((x) => [x.id, x.shown, x.family]), [["48gx", true, "rpl"]]);
  const ran = await ev(run("run_rpl", { text: "2 3 +" }));
  assert.equal(ran.error, undefined, JSON.stringify(ran));
  assert.equal(ran.stack[0].text, "5");
  assert.equal(ran.image, undefined, "text by default");
  const shown = await ev(run("read_display", { format: "both" }));
  assert.equal(shown.screen, "stack");
  assert.match(shown.image, /^data:image\/png;base64,.{100}/);

  assert.equal((await ev(run("store_variable", { name: "AIT", text: "« 2 * »" }))).stored, true);
  assert.equal(await ev(ASKING), false);
  assert.equal((await ev(run("read_variable", { name: "AIT" }))).text.replace(/\s+/g, " "), "« 2 * »");
  assert.equal((await ev(run("purge_variable", { name: "AIT" }))).purged, true);
  assert.equal(await ev(ASKING), false);
  assert.deepEqual(await ev("window.saturnusApp.ai.then((a) => a.log.entries.filter((e) => e.tool === 'store_variable').map((e) => [e.app, e.asks, e.outcome]))"), [["Your browser's AI", false, "ok"]]);
});

test("WebMCP given after load (the tab-mcp extension's event): AI access is made then, the tools offered, Calculator's sidebar and Calls follow", { timeout: 60_000 }, async (t) => {
  const c = await webMcpPage(t, { late: true });
  if (!c) return;
  const { ev } = c;
  // Calculator sets window.saturnus once mounted: under load that comes after the page's first script.
  await until(ev, "window.saturnus?.started.then(() => true)", 30_000, "Calculator mounted");
  assert.equal(await ev("window.saturnusApp.ai"), null, "no relay, no WebMCP at load: no AI access");
  assert.equal(await ev(GIVE_WEBMCP), true);
  assert.equal(await until(ev, "window.webmcp.names().includes('read_guide') && window.webmcp.names()", 15_000, "the tools offered"), "list_calculators lookup_command read_guide");
  assert.equal(await ev("window.saturnusApp.ai.then((a) => a.webmcp)"), true);
  // Calculator, mounted before, has it now: a call shows Calls in its sidebar.
  assert.equal((await ev(run("read_guide", { topic: "rpl" }))).topic, "rpl");
  await until(ev, "[...document.querySelectorAll('#ai-chip button')].some((b) => b.textContent === 'Calls')", 5_000, "Calls in the sidebar");
  assert.equal(await ev("document.getElementById('ai-connect').hidden"), true, "no relay: nothing to connect");
});

test("WebMCP given after load on a relay page whose client is not made yet (the home page): the client loads and mirrors", { timeout: 60_000 }, async (t) => {
  // Port 9 (discard): a relay nobody runs, never connected.
  const c = await webMcpPage(t, { late: true, route: "#/", relay: "http://127.0.0.1:9" });
  if (!c) return;
  const { ev } = c;
  assert.deepEqual(await ev("window.saturnusApp.ai.then((a) => [a.available, a.webmcp, Boolean(a.mcp)])"), [true, false, false]);
  assert.equal(await ev(GIVE_WEBMCP), true);
  assert.equal(await until(ev, "window.webmcp.names().includes('read_guide') && window.webmcp.names()", 15_000, "the tools offered"), "list_calculators lookup_command read_guide");
  assert.deepEqual(await ev("window.saturnusApp.ai.then((a) => [a.webmcp, a.state])"), [true, "disconnected"]);
});

test("WebMCP given after load with the 48GX running: its family's tools at once", { timeout: 180_000 }, async (t) => {
  if (!dir || !existsSync(join(dir, "gxrom-r"))) {
    t.skip("needs SATURNUS_ROM_DIR with gxrom-r");
    return;
  }
  const c = await webMcpPage(t, { late: true, files: { "/__gx": join(dir, "gxrom-r") } });
  if (!c) return;
  const { ev } = c;
  // Calculator sets window.saturnus once mounted: under load that comes after the page's first script.
  await until(ev, "window.saturnus?.started.then(() => true)", 30_000, "Calculator mounted");
  await ev(`(async () => {
    const bytes = await (await fetch("/__gx")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "gxrom-r"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '48gx'", 30_000, "the 48GX booted");
  assert.equal(await ev(GIVE_WEBMCP), true);
  await until(ev, "window.webmcp.names().includes('run_rpl')", 15_000, "the RPL tools");
  assert.deepEqual((await ev(run("list_calculators"))).calculators.map((x) => [x.id, x.shown]), [["48gx", true]]);
});
