// The calculators' tools through WebMCP (web/ai-webmcp.js, iteration 71)
// on a fake `document.modelContext` that keeps the draft's rules (a name
// once, withdrawn by its signal, the result as JSON): which tools are
// registered for the shown calculator and again when its family changes,
// withdrawn on page hide and back on show, what a call gives the agent
// (its fields, an image as a data URL, a failure as `{error}`), the
// arguments checked, a consequential tool confirmed (declined, allowed,
// cancelled), and the calls in "The AI's calls".
import { test } from "node:test";
import assert from "node:assert/strict";
import { aiTools, INSTRUCTIONS } from "../ai-tools.js";
import { AiLog } from "../ai-log.js";
import { modelContextOf, WebMcp, webMcpAnnotations, webMcpResult } from "../ai-webmcp.js";
import { readFileSync } from "node:fs";
import { buildIndex } from "../reference.js";

const turn = () => new Promise((r) => setTimeout(r, 0));
const PACE = { first: 0, poll: 0, limit: 200 };
const FRAME = { width: 131, height: 64, pixels: Buffer.alloc(17 * 64).toString("base64"), annunciators: {} };

/** A fake `ModelContext`: registered tools by name, the draft's checks, `run(name, input)` as an agent's call. */
class FakeContext extends EventTarget {
  constructor() {
    super();
    this.tools = new Map();
    this.registrations = 0;
  }

  registerTool(tool, { signal } = {}) {
    if (this.tools.has(tool.name)) return Promise.reject(new DOMException(`${tool.name} is registered`, "InvalidStateError"));
    if (!tool.description) return Promise.reject(new DOMException("no description", "InvalidStateError"));
    if (signal?.aborted) return Promise.reject(signal.reason);
    JSON.stringify(tool.inputSchema);
    JSON.stringify(tool.outputSchema);
    this.registrations++;
    this.tools.set(tool.name, tool);
    signal?.addEventListener("abort", () => {
      if (this.tools.get(tool.name) === tool) this.tools.delete(tool.name);
    }, { once: true });
    return Promise.resolve();
  }

  names() {
    return [...this.tools.keys()].sort();
  }

  /** An agent's call: the input as JSON in, the result as JSON out (executeTool's string), parsed. */
  async run(name, input = {}, signal = new AbortController().signal) {
    const tool = this.tools.get(name);
    if (!tool) throw new DOMException(name, "NotFoundError");
    const out = await tool.execute(JSON.parse(JSON.stringify(input)), { signal });
    return JSON.parse(JSON.stringify(out));
  }
}

/** A fake 48GX for the tools: answers the protocol's commands, records them. */
function calculator(model = "48gx") {
  const calls = [];
  const answers = {
    stats: { loop: "sleep" },
    uiState: { kind: "stack", stackInControl: true, halted: 0, commandLine: false },
    commandLine: { active: false },
    stack: [{ type: "real", text: "5" }],
    memoryTree: { path: ["HOME"], variables: [] },
    storeText: {},
    skin: { keys: [{ name: "1", label: "1" }, { name: "enter", label: "ENTER" }] },
  };
  const backend = new Proxy({}, {
    get(_, cmd) {
      return (...args) => {
        calls.push([cmd, ...args]);
        if (cmd === "typeKeys") return undefined;
        return Promise.resolve(answers[cmd] ?? null);
      };
    },
  });
  return { id: model, model, backend, store: { state: { frame: FRAME, keysDown: [], busy: false, booted: model } }, calls };
}

/** A fake AI access over calculator `c`: its tools (the real ones), its view, its events and its log. */
function access(c = calculator()) {
  const ai = new EventTarget();
  ai.changes = 0;
  ai.changed = () => { ai.changes++; };
  ai.consequential = new Set(["store_variable", "purge_variable"]);
  ai.shown = c.model;
  ai.view = async () => ({ store: { state: { booted: ai.shown } } });
  ai.tools = aiTools({
    calculators: async () => [{ id: c.id, model: c.model, title: c.model, rom: "gxrom-r", shown: true }],
    target: async () => c,
    existing: async () => c,
    reference: { index: async () => ({ supported: false }) },
    png: async () => "iVBORw0KGgo=",
    pace: PACE,
  });
  ai.log = new AiLog(ai);
  return ai;
}

/** A WebMcp on `context` for `ai`, with a fake confirmation answering `answer` (recording its questions). */
function webMcp(ai, { context = new FakeContext(), answer = async () => true, win = new EventTarget() } = {}) {
  const asked = [];
  const w = new WebMcp({
    ai,
    context,
    win,
    doc: {},
    confirm: async () => ({ confirmInPage: (host, question, doc, options) => { asked.push({ host, question, options }); return answer(question); } }),
  });
  return { w, context, asked, win };
}

const RPL = ["list_calculators", "list_keys", "list_variables", "lookup_command", "press_keys", "purge_variable", "read_display", "read_stack", "read_variable", "run_rpl", "store_variable", "to_stack"];

test("WebMCP is found on the document, else on the navigator of early Chrome builds, else not", () => {
  const ctx = { registerTool() {} };
  assert.equal(modelContextOf({ modelContext: ctx }, {}), ctx);
  assert.equal(modelContextOf({}, { modelContext: ctx }), ctx);
  assert.equal(modelContextOf({ modelContext: ctx }, { modelContext: { registerTool() {} } }), ctx);
  assert.equal(modelContextOf({}, {}), null);
  assert.equal(modelContextOf({ modelContext: {} }, {}), null);
  assert.equal(modelContextOf(undefined, undefined), null);
});

test("the shown calculator's tools are registered, and again when its family changes", async () => {
  const ai = access();
  ai.shown = null;
  const { w, context } = webMcp(ai);
  await w.start();
  assert.deepEqual(context.names(), ["list_calculators", "lookup_command"], "no calculator shown");
  assert.equal(w.active, true);
  await w.update("48gx");
  assert.deepEqual(context.names(), RPL);
  const before = context.registrations;
  // The same family: nothing registered again.
  await w.update("49g");
  assert.equal(context.registrations, before);
  await w.update("28s");
  assert.deepEqual(context.names(), ["list_calculators", "list_keys", "lookup_command", "press_keys", "read_display", "run_rpl"]);
  await w.update("42s");
  assert.deepEqual(context.names(), ["list_calculators", "list_keys", "lookup_command", "press_keys", "read_display", "run_function"]);
  // A quick change while the last registration still runs: the newer wins, nothing twice.
  const a = w.update("48gx");
  const b = w.update(null);
  await Promise.all([a, b]);
  assert.deepEqual(context.names(), ["list_calculators", "lookup_command"]);
  assert.ok(ai.changes > 0, "the sidebar hears of it");
});

test("the tools as WebMCP gets them: WebMCP's hints only, an output schema, the instructions with list_calculators", async () => {
  const ai = access();
  const { w, context } = webMcp(ai);
  await w.start();
  for (const tool of context.tools.values()) {
    assert.ok(tool.outputSchema?.type === "object", tool.name);
    assert.ok(tool.title, tool.name);
    assert.deepEqual(Object.keys(tool.annotations).filter((k) => !["readOnlyHint", "untrustedContentHint", "consequentialHint", "debugging"].includes(k)), [], tool.name);
  }
  assert.deepEqual(context.tools.get("purge_variable").annotations, { consequentialHint: true });
  assert.deepEqual(context.tools.get("read_stack").annotations, { readOnlyHint: true, untrustedContentHint: true });
  assert.deepEqual(context.tools.get("list_calculators").annotations, { readOnlyHint: true, untrustedContentHint: true });
  assert.deepEqual(context.tools.get("lookup_command").annotations, { readOnlyHint: true });
  assert.ok(context.tools.get("list_calculators").description.endsWith(INSTRUCTIONS));
  assert.ok(!context.tools.get("read_stack").description.includes(INSTRUCTIONS));
  assert.deepEqual(webMcpAnnotations({ readOnlyHint: true, destructiveHint: true, idempotentHint: false }), { readOnlyHint: true });
});

test("withdrawn when the page is hidden, back when it shows again", async () => {
  const ai = access();
  const { w, context, win } = webMcp(ai);
  await w.start();
  assert.deepEqual(context.names(), RPL);
  win.dispatchEvent(new Event("pagehide"));
  assert.deepEqual(context.names(), []);
  assert.equal(w.active, false);
  // A model change while hidden waits for the show.
  await w.update("42s");
  assert.deepEqual(context.names(), []);
  win.dispatchEvent(new Event("pageshow"));
  await turn();
  assert.deepEqual(context.names(), ["list_calculators", "list_keys", "lookup_command", "press_keys", "read_display", "run_function"]);
  // Hidden and shown again with no change: the same tools.
  win.dispatchEvent(new Event("pagehide"));
  win.dispatchEvent(new Event("pageshow"));
  await turn();
  assert.equal(context.names().length, 6);
});

test("a call gives the agent the tool's fields; an image as a data URL; a failure as {error}; all in the log", async () => {
  const c = calculator();
  const ai = access(c);
  const { w, context } = webMcp(ai);
  await w.start();
  const stack = await context.run("read_stack", {});
  assert.equal(stack.depth, 1);
  assert.deepEqual(stack.levels, [{ level: 1, type: "real", text: "5" }]);
  const shown = await context.run("read_display", { format: "both" });
  assert.equal(shown.image, "data:image/png;base64,iVBORw0KGgo=");
  assert.equal(shown.calculator, "48gx");
  assert.equal(shown.screen, "stack");
  // Wrong arguments: refused with why, nothing sent.
  const before = c.calls.length;
  const bad = await context.run("press_keys", { keys: "1" });
  assert.match(bad.error, /^Input validation error: .*\/keys must be array/);
  assert.equal(c.calls.length, before);
  // The tool's own refusal comes back with its words, not as a bare failure.
  const nokey = await context.run("press_keys", { keys: ["sin"] });
  assert.match(nokey.error, /has no key "sin"/);
  const entries = ai.log.entries.filter((e) => e.kind === "call");
  assert.deepEqual(entries.map((e) => [e.tool, e.outcome, e.app]), [
    ["read_stack", "ok", "Your browser's AI"],
    ["read_display", "ok", "Your browser's AI"],
    ["press_keys", "error", "Your browser's AI"],
    ["press_keys", "error", "Your browser's AI"],
  ]);
  assert.match(entries[1].result, /\[image, image\/png\]/);
  assert.equal(w.called, true);
  assert.deepEqual(webMcpResult("text"), "text");
  assert.deepEqual(webMcpResult({ content: [{ type: "text", text: "hi" }] }), { text: "hi" });
});

test("a consequential call asks the user first, every time: declined runs nothing, allowed runs", async () => {
  const c = calculator();
  const ai = access(c);
  const answers = [false, true];
  const { w, context, asked } = webMcp(ai, { answer: async () => answers.shift() });
  await w.start();
  const declined = await context.run("store_variable", { name: "X", text: "1" });
  assert.deepEqual(declined, { error: "The user declined." });
  assert.ok(!c.calls.some(([cmd]) => cmd === "storeText"));
  const stored = await context.run("store_variable", { name: "X", text: "1" });
  assert.equal(stored.stored, true);
  assert.deepEqual(c.calls.filter(([cmd]) => cmd === "storeText").map(([, a]) => a.name), ["X"]);
  assert.equal(asked.length, 2);
  assert.deepEqual(asked[0].options, { alwaysAllow: false });
  assert.equal(asked[0].question.client.name, "Your browser's AI");
  assert.equal(asked[0].question.client.host, "WebMCP");
  assert.equal(asked[0].question.tool.title, "Store a variable");
  assert.deepEqual(asked[0].question.arguments, { name: "X", text: "1" });
  // Read tools never ask.
  await context.run("read_stack", {});
  assert.equal(asked.length, 2);
  const calls = ai.log.entries.filter((e) => e.kind === "call");
  assert.deepEqual(calls.map((e) => [e.tool, e.outcome, e.asks]), [["store_variable", "declined", true], ["store_variable", "ok", true], ["read_stack", "ok", false]]);
});

test("a call the agent cancels: while it is asked the question closes; while it waits it stops", async () => {
  const c = calculator();
  const ai = access(c);
  const { w, context } = webMcp(ai, {
    answer: (q) => new Promise((resolve) => q.signal.addEventListener("abort", () => resolve(false), { once: true })),
  });
  await w.start();
  const abort = new AbortController();
  const asking = context.run("purge_variable", { name: "X" }, abort.signal);
  await turn();
  abort.abort();
  assert.deepEqual(await asking, { error: "The user declined." });
  assert.ok(!c.calls.some(([cmd]) => cmd === "purge"));
  assert.equal(ai.log.entries.at(-1).outcome, "cancelled");
  // Keys pressed, then cancelled while the calculator still runs: no endless wait.
  const busy = calculator();
  const answers = busy.backend;
  const slow = access(busy);
  const s = webMcp(slow);
  await s.w.start();
  let looks = 0;
  const stats = new Proxy(answers, {
    get(target, cmd) {
      if (cmd === "stats") return () => { looks++; return Promise.resolve({ loop: "frame" }); };
      return target[cmd];
    },
  });
  busy.backend = stats;
  const stop = new AbortController();
  const pressing = s.context.run("press_keys", { keys: ["1"] }, stop.signal);
  await new Promise((r) => setTimeout(r, 20));
  stop.abort();
  const r = await pressing;
  assert.equal(r.error, "Cancelled");
  const after = looks;
  await new Promise((r2) => setTimeout(r2, 20));
  assert.equal(looks, after, "it stopped looking");
});

test("the early navigator.modelContext: withdrawn with its unregisterTool", async () => {
  const ai = access();
  const gone = [];
  const context = new FakeContext();
  context.unregisterTool = (name) => {
    gone.push(name);
    context.tools.delete(name);
  };
  const { w } = webMcp(ai, { context });
  await w.start();
  w.withdraw();
  assert.deepEqual(gone.sort(), RPL);
});

test("without WebMCP nothing is registered", async () => {
  const ai = access();
  const w = new WebMcp({ ai, context: null, win: new EventTarget() });
  await w.start();
  await w.update("48gx");
  assert.equal(w.active, false);
});

test("a registerTool that throws (early builds) leaves the others, rejects nothing, and is tried again at the next change", async () => {
  const ai = access();
  const context = new FakeContext();
  const take = context.registerTool.bind(context);
  let refuse = () => true;
  context.registerTool = (tool, options) => {
    if (refuse(tool.name)) throw new TypeError(`no ${tool.name}`);
    return take(tool, options);
  };
  const warn = console.warn;
  console.warn = () => {};
  try {
    const { w } = webMcp(ai, { context });
    // Every registration refused: nothing claimed, no rejection.
    await w.start();
    assert.deepEqual(context.names(), []);
    assert.equal(w.active, false);
    // One refused: the others are there, and the state says only those.
    refuse = (name) => name === "run_rpl";
    await w.update("49g");
    assert.deepEqual(context.names(), RPL.filter((n) => n !== "run_rpl"));
    assert.deepEqual([...w.names].sort(), context.names());
    // The same family again: what was refused is tried again.
    refuse = () => false;
    await w.update("48gx");
    assert.deepEqual(context.names(), RPL);
  } finally {
    console.warn = warn;
  }
});

test("list_calculators registers the tools of every calculator it names, hidden ones too", async () => {
  const gx = calculator("48gx");
  const s42 = calculator("42s");
  const ai = access(gx);
  let running = [gx, s42];
  ai.tools = aiTools({
    calculators: async () => running.map((c) => ({ id: c.id, model: c.model, title: c.model, rom: "rom", shown: c === gx })),
    target: async (id) => running.find((c) => c.id === (id ?? "48gx")),
    existing: async (id) => running.find((c) => c.id === id) ?? null,
    reference: { index: async (m) => buildIndex(JSON.parse(readFileSync(new URL("../commands.json", import.meta.url), "utf8")), m) },
    png: async () => "iVBORw0KGgo=",
    pace: PACE,
  });
  const { w, context } = webMcp(ai);
  await w.start();
  assert.ok(!context.names().includes("run_function"), "only the shown calculator's family before a listing");
  const list = await context.run("list_calculators");
  assert.deepEqual(list.calculators.map((c) => c.id), ["48gx", "42s"]);
  for (const c of list.calculators) for (const name of c.tools) assert.ok(context.names().includes(name), `${c.id}: ${name}`);
  // The hidden 42S's own tool is there and reaches it.
  const r = await context.run("run_function", { calculator: "42s", name: "NOPE" });
  assert.match(r.error, /The HP 42S reference has no function "NOPE"|The 42s reference has no function "NOPE"/);
  // Once the 42S has gone, the next listing withdraws its family's tool; the shown calculator's stay.
  running = [gx];
  const before = context.registrations;
  await context.run("list_calculators");
  assert.deepEqual(context.names(), RPL);
  assert.equal(context.registrations, before, "nothing registered again");
  assert.match(context.tools.get("list_calculators").description, /call list_calculators first/);
});

test("through WebMCP the display comes as text unless the call asks for an image", async () => {
  const ai = access();
  const { w, context } = webMcp(ai);
  await w.start();
  const read = await context.run("read_display", {});
  assert.equal(read.image, undefined);
  assert.ok(Array.isArray(read.stack));
  const pressed = await context.run("press_keys", { keys: ["1"] });
  assert.equal(pressed.image, undefined);
  assert.equal(pressed.pressed, 1);
  assert.equal((await context.run("press_keys", { keys: ["1"], display: "image" })).image, "data:image/png;base64,iVBORw0KGgo=");
  for (const [name, key] of [["read_display", "format"], ["press_keys", "display"], ["run_rpl", "display"], ["to_stack", "display"]]) {
    assert.match(context.tools.get(name).inputSchema.properties[key].description, /Default: text; "image" or "both" adds the display as a PNG/, name);
    assert.doesNotMatch(context.tools.get(name).inputSchema.properties[key].description, /Default: (both|text)\. Default/, name);
  }
  // tab-mcp's tools keep their own defaults.
  assert.match(ai.tools.find((t) => t.name === "read_display").inputSchema.properties.format.description, /Default: both/);
});
