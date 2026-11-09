// The calculators' tools through WebMCP (web/ai-webmcp.js, iteration 71)
// in headless Chrome with WebMCP switched on by its command-line switch
// (`--enable-features=WebMCPTesting`, the feature behind
// chrome://flags/#enable-webmcp-testing), called as an agent would
// through `document.modelContext.getTools()` and `executeTool()`:
// - the app registers its tools on load with no relay (127.0.0.1 has
//   none) and the sidebar offers no Connect: list_calculators and
//   lookup_command;
// - AI access over a fake calculator: a read tool answers, store_variable
//   and purge_variable run with no question in the page (the browser
//   asks), all in the log;
// - with the ROMs (SATURNUS_ROM_DIR: gxrom-r): the 48GX booted, its
//   family's tools, read_display, run_rpl, store_variable.
// Skipped without Chrome, the wasm package (`just web`), or a Chrome
// whose switch gives no `document.modelContext`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

const ARGS = ["--enable-features=WebMCPTesting"];
const dir = process.env.SATURNUS_ROM_DIR;

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

/** A session in Chrome with WebMCP, or null (skipped) where this Chrome has none. */
async function webMcpSession(t, opts = {}) {
  const c = await session(t, { ...opts, args: ARGS });
  if (!c) return null;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Page.navigate", { url: `http://127.0.0.1:${c.port}/shared/mcp-install.js` });
  await sleep(300);
  if (!(await c.ev("typeof document.modelContext?.registerTool === 'function'"))) {
    t.skip("this Chrome has no WebMCP behind --enable-features=WebMCPTesting");
    return null;
  }
  return c;
}

/** An agent's call: the tool by name from getTools, executeTool, its JSON parsed. */
const run = (name, input = {}) => `(async () => {
  const tool = (await document.modelContext.getTools()).find((t) => t.name === ${JSON.stringify(name)});
  if (!tool) throw new Error("no tool ${name}");
  return JSON.parse(await document.modelContext.executeTool(tool, ${JSON.stringify(input)}));
})()`;

/** The names getTools gives, sorted. */
const NAMES = "document.modelContext.getTools().then((ts) => ts.map((t) => t.name).sort().join(' '))";

/** Whether the page shows a confirmation (tab-mcp's dialog) now. */
const ASKING = "!!document.querySelector('dialog.tab-mcp-confirm')";

test("the app registers its tools with WebMCP on load, without a relay", { timeout: 60_000 }, async (t) => {
  const c = await webMcpSession(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  assert.equal(await until(ev, `${NAMES}.then((n) => n.includes('list_calculators') && n)`, 15_000, "the tools registered"), "list_calculators lookup_command");
  const tools = await ev("document.modelContext.getTools().then((ts) => ts.map((t) => ({ name: t.name, title: t.title, annotations: t.annotations, origin: t.origin })))");
  const list = tools.find((x) => x.name === "list_calculators");
  assert.equal(list.title, "List calculators");
  assert.equal(list.annotations.readOnlyHint, true);
  assert.equal(list.origin, `http://127.0.0.1:${port}`);
  const none = await ev(run("list_calculators"));
  assert.deepEqual(none.calculators, []);
  assert.match(none.note, /No calculator is running/);
  const sin = await ev(run("lookup_command", { model: "48gx", name: "SIN" }));
  assert.equal(sin.name, "SIN");
  assert.equal(sin.model, "48gx");
  // No relay here: nothing to connect, so the entry stays hidden.
  assert.equal(await ev("document.getElementById('ai-connect').hidden"), true);
});

test("a read tool answers; store_variable and purge_variable run without asking in the page", { timeout: 60_000 }, async (t) => {
  const c = await webMcpSession(t);
  if (!c) return;
  const { ev } = c;
  assert.equal(await ev(`(async () => {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = "/shared/components.css";
    document.head.append(link);
    const t = { stored: [], purged: [] };
    const store = { state: { booted: "48gx", romName: "gxrom-r", frame: null, keysDown: [], busy: false } };
    const backend = {
      stats: async () => ({ loop: "sleep" }),
      stack: async () => [{ type: "real", text: "42" }],
      memoryTree: async () => ({ path: ["HOME"], variables: [] }),
      storeText: async (a) => { t.stored.push(a.name); return {}; },
      purge: async (dir, name) => { t.purged.push(name); },
    };
    const { AiAccess } = await import("/ai.js");
    const { WebMcp } = await import("/ai-webmcp.js");
    t.ai = new AiAccess({ calculatorView: async () => ({ ai: async () => ({ backend, store, reference: null }) }), relay: null });
    t.webmcp = new WebMcp({ ai: t.ai });
    t.ai.webmcp = t.webmcp;
    await t.webmcp.start();
    window.t = t;
    return true;
  })()`), true);
  assert.match(await ev(NAMES), /read_stack.*store_variable/);
  const stack = await ev(run("read_stack"));
  assert.deepEqual(stack.levels, [{ level: 1, type: "real", text: "42" }]);

  // The browser asks before a call (Chrome's testing switch does not); the page adds no question.
  const tools = await ev("document.modelContext.getTools().then((ts) => Object.fromEntries(ts.map((x) => [x.name, x.annotations])))");
  assert.equal(tools.store_variable.consequentialHint, true);
  assert.equal(tools.purge_variable.consequentialHint, true);
  const stored = await ev(run("store_variable", { name: "AIT", text: "« 2 * »" }));
  assert.equal(stored.stored, true);
  assert.equal(await ev(ASKING), false);
  assert.deepEqual(await ev("window.t.stored"), ["AIT"]);

  const purged = await ev(run("purge_variable", { name: "AIT" }));
  assert.equal(purged.error, undefined, JSON.stringify(purged));
  assert.equal(await ev(ASKING), false);
  assert.deepEqual(await ev("window.t.purged"), ["AIT"]);
  assert.deepEqual(await ev("window.t.ai.log.entries.filter((e) => e.kind === 'call').map((e) => [e.tool, e.outcome, e.app, e.asks])"), [
    ["read_stack", "ok", "Your browser's AI", false],
    ["store_variable", "ok", "Your browser's AI", false],
    ["purge_variable", "ok", "Your browser's AI", false],
  ]);
  // "Connect your AI app" says the browser's AI has the tools too.
  await ev(`import("/ai-dialog.js").then((d) => { d.openAiDialog(window.t.ai); return true; })`);
  const note = await until(ev, "(() => { const n = document.querySelector('dialog.ai .ai-webmcp'); return n && !n.hidden && n.textContent; })()", 5_000, "the dialog's WebMCP note");
  assert.match(note, /^This browser also offers the calculators to its own AI \(WebMCP\)/);
  await ev("document.querySelector('dialog.ai').close(), true");
  // Withdrawn: the agent sees none.
  await ev("window.t.webmcp.withdraw(), true");
  assert.equal(await ev(NAMES), "");
});

test("an agent in Chrome reads the 48GX's display, runs RPL and stores", { timeout: 180_000 }, async (t) => {
  if (!dir || !existsSync(join(dir, "gxrom-r"))) {
    t.skip("needs SATURNUS_ROM_DIR with gxrom-r");
    return;
  }
  const c = await webMcpSession(t, { files: { "/__gx": join(dir, "gxrom-r") } });
  if (!c) return;
  const { send, ev, port } = c;
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 860, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started.then(() => true)");
  await ev(`(async () => {
    const bytes = await (await fetch("/__gx")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "gxrom-r"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '48gx'", 30_000, "the 48GX booted");
  await until(ev, "window.saturnus.backend.stack().then(() => window.saturnus.backend.stats()).then((s) => s.loop === 'sleep' && s.emulatedMs > 4000, () => false)", 30_000, "the 48GX settled");
  // The 48GX's family: registered again for it.
  await until(ev, `${NAMES}.then((n) => n.includes('run_rpl'))`, 10_000, "the RPL tools");
  assert.equal(await ev(NAMES), "list_calculators list_keys list_variables lookup_command press_keys purge_variable read_display read_stack read_variable run_rpl store_variable to_stack");

  const list = await ev(run("list_calculators"));
  assert.deepEqual(list.calculators.map((x) => [x.id, x.shown, x.family]), [["48gx", true, "rpl"]]);
  const ran = await ev(run("run_rpl", { text: "2 3 +" }));
  assert.equal(ran.error, undefined, JSON.stringify(ran));
  assert.equal(ran.stack[0].text, "5");
  const shown = await ev(run("read_display", { format: "both" }));
  assert.equal(shown.screen, "stack");
  assert.equal(shown.stack[0].text, "5");
  assert.match(shown.image, /^data:image\/png;base64,.{100}/);

  assert.equal((await ev(run("store_variable", { name: "AIT", text: "« 2 * »" }))).stored, true);
  assert.equal(await ev(ASKING), false);
  assert.equal((await ev(run("read_variable", { name: "AIT" }))).text.replace(/\s+/g, " "), "« 2 * »");
  // The calls are in the log, and the sidebar offers it.
  assert.ok(await ev("window.saturnusApp.ai.then((a) => a.log.entries.filter((e) => e.kind === 'call').length >= 5)"));
});
