// The calculators' tools through WebMCP by the client library's mirror
// (web/ai.js, iteration 78: `webmcp: true`, in place of iteration 71's
// web/ai-webmcp.js), with the vendored client itself on a fake
// `ModelContext`, given by a `modelContext` getter on `Document.prototype`
// (the one the library reads) that keeps the draft's rules (a name once,
// withdrawn by its signal, the result as JSON): which tools the browser
// gets and when they go, what a call gives the agent (its fields, the
// display as text unless an image is asked for, an image as a data URL,
// a failure as `{error}`, `list_calculators` with the instructions), a
// consequential tool run with no question in the page, a cancelled call, a
// tool withdrawn while its call runs, and the calls in "The AI's calls" as
// "Your browser's AI" or the AI app the tab-mcp extension vouched for.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { aiTools, FAMILY_TOOLS, INSTRUCTIONS } from "../ai-tools.js";
import { ALWAYS, AiAccess, hasWebMcp, WEBMCP_INSTRUCTIONS } from "../ai.js";

const turn = () => new Promise((r) => setTimeout(r, 0));
const PACE = { first: 0, poll: 0, limit: 200 };
const FRAME = { width: 131, height: 64, pixels: Buffer.alloc(17 * 64).toString("base64"), annunciators: {} };
const GUIDE = JSON.parse(readFileSync(new URL("../guide.json", import.meta.url), "utf8"));

/** A fake `ModelContext`: registered tools by name with their signals, the draft's checks, `run(name, input)` as an agent's call. */
class FakeContext {
  constructor() {
    this.tools = new Map();
    this.registrations = [];
  }

  registerTool(tool, { signal } = {}) {
    if (this.tools.has(tool.name)) throw new DOMException(`${tool.name} is registered`, "InvalidStateError");
    if (signal?.aborted) throw signal.reason;
    JSON.stringify(tool.inputSchema);
    this.registrations.push({ name: tool.name, signal });
    this.tools.set(tool.name, tool);
    signal?.addEventListener("abort", () => {
      if (this.tools.get(tool.name) === tool) this.tools.delete(tool.name);
    }, { once: true });
  }

  names() {
    return [...this.tools.keys()].sort();
  }

  /** An agent's call: the input as JSON in, the result as JSON out, parsed. */
  async run(name, input = {}, signal = new AbortController().signal) {
    const tool = this.tools.get(name);
    if (!tool) throw new DOMException(name, "NotFoundError");
    const out = await tool.execute(JSON.parse(JSON.stringify(input)), { signal });
    return JSON.parse(JSON.stringify(out));
  }
}

/** The document the library sees, fresh per test (the extension's handshake is per document), with WebMCP's getter on its prototype. */
class FakeDocument extends EventTarget {
  constructor() {
    super();
    this.documentElement = { setAttribute() {} };
  }
}
let current = null;
Object.defineProperty(FakeDocument.prototype, "modelContext", { get: () => current, configurable: true });
globalThis.Document = FakeDocument;

/** A fake 48GX for the tools: answers the protocol's commands, records them; `gate` holds `stack` until it resolves. */
function calculator(model = "48gx") {
  const calls = [];
  const c = { id: model, model, calls, gate: null };
  const answers = {
    stats: { loop: "sleep" },
    uiState: { kind: "stack", stackInControl: true, halted: 0, commandLine: false },
    commandLine: { active: false },
    stack: [{ type: "real", text: "5" }],
    memoryTree: { path: ["HOME"], variables: [] },
    storeText: {},
    skin: { keys: [{ name: "1", label: "1" }, { name: "enter", label: "ENTER" }] },
  };
  c.backend = new Proxy({}, {
    get(_, cmd) {
      return async (...args) => {
        calls.push([cmd, ...args]);
        if (cmd === "typeKeys") return undefined;
        if (cmd === "stack" && c.gate) await c.gate;
        return answers[cmd] ?? null;
      };
    },
  });
  c.store = { state: { frame: FRAME, keysDown: [], busy: false, booted: model, romName: "gxrom-r" } };
  return c;
}

/** AI access without a relay on `c` (shown in Calculator), its tools over fakes, the vendored client mirroring into a fresh fake WebMCP. */
async function mirrored(c = calculator()) {
  globalThis.document = new FakeDocument();
  const context = new FakeContext();
  current = context;
  const ai = new AiAccess({
    calculatorView: async () => ({ ai: async () => ({ backend: c.backend, store: c.store, reference: null }) }),
    relay: null,
    storage: null,
    hub: { running: () => false, client: () => null },
    roms: { kept: async () => [], bytes: async () => null },
  });
  ai.toolList = aiTools({
    calculators: () => ai.calculators(),
    target: (id) => ai.target(id),
    existing: (id) => ai.existing(id),
    reference: { index: async () => ({ supported: false }) },
    png: async () => "iVBORw0KGgo=",
    guide: async () => GUIDE,
    pace: PACE,
  });
  for (const tool of ai.toolList) if (tool.annotations?.consequentialHint) ai.consequential.add(tool.name);
  assert.equal(ai.webmcp, true, "the getter is there");
  ai.start();
  await ai.loading;
  if (c.store.state.booted) ai.shown(c.store.state.booted);
  await registered(context, (n) => n.includes("list_calculators"));
  return { ai, context, c };
}

/** Until the mirror's registrations (a turn each) settle on what `ok` wants. */
async function registered(context, ok) {
  for (let i = 0; i < 50; i++) {
    if (ok(context.names())) return;
    await turn();
  }
  assert.fail(`not registered: ${context.names().join(" ")}`);
}

const sorted = (names) => [...names].sort();
const RPL = sorted(new Set([...ALWAYS, ...FAMILY_TOOLS.rpl]));
const calls = (ai) => ai.log.entries.filter((e) => e.kind === "call");

test("WebMCP is the getter on Document.prototype the library reads, giving a ModelContext; else none", () => {
  current = { registerTool() {} };
  assert.equal(hasWebMcp(new FakeDocument()), true);
  current = {};
  assert.equal(hasWebMcp(new FakeDocument()), false);
  current = null;
  assert.equal(hasWebMcp(new FakeDocument()), false);
});

test("the browser gets the tools of the running calculators, and loses those no calculator wants by their signals", async () => {
  const { ai, context } = await mirrored();
  await registered(context, (n) => n.length === RPL.length);
  assert.deepEqual(context.names(), RPL);
  const runRpl = context.registrations.find((r) => r.name === "run_rpl");
  ai.shown("42s");
  await registered(context, (n) => n.includes("run_function") && !n.includes("run_rpl"));
  assert.equal(runRpl.signal.aborted, true, "withdrawn by its signal");
  assert.deepEqual(context.names(), sorted(new Set([...ALWAYS, ...FAMILY_TOOLS.keys])));
  // Back: registered anew, once.
  ai.shown("48gx");
  await registered(context, (n) => n.includes("run_rpl"));
  assert.equal(context.registrations.filter((r) => r.name === "run_rpl").length, 2);
  assert.equal(context.registrations.filter((r) => r.name === "list_calculators").length, 1, "what stays is not registered again");
});

test("the tools as WebMCP gets them: WebMCP's hints, an output schema, at most 1000 characters, nothing about who asks", async () => {
  const { context } = await mirrored();
  await registered(context, (n) => n.length === RPL.length);
  for (const tool of context.tools.values()) {
    assert.equal(tool.outputSchema?.type, "object", tool.name);
    assert.ok(tool.title, tool.name);
    assert.deepEqual(Object.keys(tool.annotations).filter((k) => tool.annotations[k] !== undefined && !["readOnlyHint", "untrustedContentHint", "consequentialHint", "debugging"].includes(k)), [], tool.name);
    // tab-mcp's extension cuts a page tool's description at 1000 characters.
    assert.ok(tool.description.length <= 1000, `${tool.name}: ${tool.description.length} characters`);
    assert.doesNotMatch(tool.description, /approve|in the app first|may ask the user/i, tool.name);
  }
  assert.equal(context.tools.get("store_variable").annotations.consequentialHint, true);
  assert.equal(context.tools.get("purge_variable").annotations.consequentialHint, true);
  assert.equal(context.tools.get("read_stack").annotations.readOnlyHint, true);
  assert.equal(context.tools.get("read_stack").annotations.untrustedContentHint, true);
  assert.match(context.tools.get("list_calculators").description, /result's "instructions"/);
  assert.match(context.tools.get("read_display").inputSchema.properties.format.description, /Default: both; through WebMCP .* text/);
});

test("a call gives the agent the tool's fields, the display as text unless it asks for an image, a failure as {error}; all in the log as Your browser's AI, never asking", async () => {
  const { ai, context, c } = await mirrored();
  await registered(context, (n) => n.length === RPL.length);
  const stack = await context.run("read_stack", {});
  assert.equal(stack.depth, 1);
  assert.deepEqual(stack.levels, [{ level: 1, type: "real", text: "5" }]);
  const read = await context.run("read_display", {});
  assert.equal(read.image, undefined, "text by default");
  assert.ok(Array.isArray(read.stack));
  const shown = await context.run("read_display", { format: "both" });
  assert.equal(shown.image, "data:image/png;base64,iVBORw0KGgo=");
  assert.equal(shown.screen, "stack");
  assert.equal((await context.run("press_keys", { keys: ["1"] })).image, undefined);
  assert.equal((await context.run("press_keys", { keys: ["1"], display: "image" })).image, "data:image/png;base64,iVBORw0KGgo=");
  // Wrong arguments: refused with why (the library checks the schema), nothing sent.
  const before = c.calls.length;
  const bad = await context.run("press_keys", { keys: "1" });
  assert.match(bad.error, /^Input validation error: .*keys/);
  assert.equal(c.calls.length, before);
  // The tool's own refusal with its words.
  assert.match((await context.run("press_keys", { keys: ["sin"] })).error, /has no key "sin"/);
  assert.deepEqual(calls(ai).map((e) => [e.tool, e.outcome, e.app, e.asks]), [
    ["read_stack", "ok", "Your browser's AI", false],
    ["read_display", "ok", "Your browser's AI", false],
    ["read_display", "ok", "Your browser's AI", false],
    ["press_keys", "ok", "Your browser's AI", false],
    ["press_keys", "ok", "Your browser's AI", false],
    ["press_keys", "error", "Your browser's AI", false],
    ["press_keys", "error", "Your browser's AI", false],
  ]);
  // The log names the image; its base64 is not in it.
  assert.match(calls(ai)[2].result, /\[image, image\/png\]/);
  assert.doesNotMatch(calls(ai)[2].result, /data:image/);
  assert.equal(ai.webmcpCalled, true);
});

test("a consequential call runs without a question in the page; the hint stays for the browser", async () => {
  const { ai, context, c } = await mirrored();
  await registered(context, (n) => n.includes("store_variable"));
  assert.equal((await context.run("store_variable", { name: "X", text: "1" })).stored, true);
  assert.deepEqual(c.calls.filter(([cmd]) => cmd === "storeText").map(([, a]) => a.name), ["X"]);
  const purged = await context.run("purge_variable", { name: "X" });
  assert.equal(purged.error, undefined, JSON.stringify(purged));
  assert.deepEqual(c.calls.filter(([cmd]) => cmd === "purge").map(([, , name]) => name), ["X"]);
  assert.deepEqual(calls(ai).map((e) => [e.tool, e.outcome, e.asks]), [["store_variable", "ok", false], ["purge_variable", "ok", false]]);
});

test("a call the agent cancelled before it ran sends nothing", async () => {
  const { ai, context, c } = await mirrored();
  await registered(context, (n) => n.includes("purge_variable"));
  const abort = new AbortController();
  abort.abort();
  await assert.rejects(context.run("purge_variable", { name: "X" }, abort.signal), { name: "AbortError" });
  assert.ok(!c.calls.some(([cmd]) => cmd === "purge"));
  assert.equal(calls(ai).at(-1).outcome, "cancelled");
});

test("a tool withdrawn while its call runs: the call ends with its result", async () => {
  const c = calculator();
  const { ai, context } = await mirrored(c);
  await registered(context, (n) => n.includes("read_stack"));
  let open;
  c.gate = new Promise((r) => { open = r; });
  const reading = context.run("read_stack", {});
  while (!c.calls.some(([cmd]) => cmd === "stack")) await turn();
  // The 48GX goes: its family's tools are withdrawn while read_stack waits for the stack.
  ai.shown(null);
  await registered(context, (n) => !n.includes("read_stack"));
  open();
  const r = await reading;
  assert.equal(r.error, undefined, JSON.stringify(r));
  assert.deepEqual(r.levels, [{ level: 1, type: "real", text: "5" }]);
  assert.equal(calls(ai).at(-1).outcome, "ok");
});

test("list_calculators' result carries the WebMCP instructions on every call, which do not promise that the app asks", async () => {
  const { context } = await mirrored();
  for (let i = 0; i < 2; i++) {
    const list = await context.run("list_calculators");
    assert.equal(list.instructions, WEBMCP_INSTRUCTIONS, `call ${i + 1}`);
    assert.deepEqual(list.calculators.map((x) => [x.id, x.shown, x.family]), [["48gx", true, "rpl"]]);
  }
  assert.notEqual(WEBMCP_INSTRUCTIONS, INSTRUCTIONS);
  assert.match(WEBMCP_INSTRUCTIONS, /the app does not ask the user about them here: confirm with the user/);
  assert.doesNotMatch(WEBMCP_INSTRUCTIONS, /approves each/);
  await registered(context, (n) => n.includes("read_stack"));
  assert.equal((await context.run("read_stack")).instructions, undefined, "only list_calculators");
});

test("read_guide with no calculator shown: a topic's text; an unknown topic as {error}", async () => {
  const c = calculator();
  c.store.state.booted = null;
  const { context } = await mirrored(c);
  assert.deepEqual(context.names(), sorted(ALWAYS));
  const r = await context.run("read_guide", { topic: "rpl" });
  assert.equal(r.text, GUIDE.topics.rpl);
  assert.match((await context.run("read_guide", { topic: "nope" })).error, /^Input validation error: .*"skill","rpl","models","workflows"/);
});

test("a call the tab-mcp extension vouched for is the AI app's in the log, and asks nothing in the page", async () => {
  const { ai, context, c } = await mirrored();
  await registered(context, (n) => n.includes("store_variable"));
  const args = { name: "Y", text: "2" };
  document.dispatchEvent(new CustomEvent("tab-mcp-extension", { detail: JSON.stringify({ v: 1, type: "vouch", tool: "store_variable", arguments: args, nonce: "n1", app: { name: "Claude", host: "claude.ai" } }) }));
  assert.equal((await context.run("store_variable", args)).stored, true);
  assert.deepEqual(c.calls.filter(([cmd]) => cmd === "storeText").map(([, a]) => a.name), ["Y"]);
  assert.deepEqual(calls(ai).map((e) => [e.tool, e.app, e.asks]), [["store_variable", "Claude", false]]);
});
