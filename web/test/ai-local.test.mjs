// AI access in the desktop app (web/ai-local.js, iteration 58) with a
// fake Tauri bridge: what the window attaches, the tool list and calls it
// answers for the app's endpoint (mcp-call / mcp_reply), the confirmation
// with "Always allow", declined and cancelled calls, the AI's calls
// (toolcall / toolcallend), and the setting.
//
// And the owner's rule that the browser and the desktop app offer the same
// tools: web/ai.js's AiAccess, connected through the vendored tab-mcp
// client to a fake relay (a WebSocket that speaks the relay's side of
// tab-mcp.v1), lists and answers exactly what ai-local.js does.
import { test } from "node:test";
import assert from "node:assert/strict";
import { CALL_LIMIT_MS, CONFIRM_LIMIT_MS, LocalAiAccess, invalidArguments, listTools, mapAnnotations, toCallToolResult, validate } from "../ai-local.js";
import { AiAccess } from "../ai.js";
import { aiTools, INSTRUCTIONS } from "../ai-tools.js";

const turn = () => new Promise((r) => setTimeout(r, 0));

/** A fake `window.__TAURI__`: the commands it got, the event handlers, `status` the app's state. */
function fakeTauri(status = {}) {
  const calls = [];
  const handlers = new Map();
  const replies = new Map();
  const t = {
    calls,
    replies,
    status: { enabled: false, running: false, port: 4842, defaultPort: 4842, address: null, error: null, apps: [], ...status },
    core: {
      invoke: async (cmd, args) => {
        calls.push([cmd, args]);
        if (cmd === "mcp_status") return { ...t.status };
        if (cmd === "mcp_set") {
          t.status = { ...t.status, enabled: args.enabled, port: args.port ?? t.status.port, running: args.enabled };
          t.status.address = args.enabled ? `http://127.0.0.1:${t.status.port}/mcp` : null;
          if (args.enabled) t.status.authorization = `Bearer ${"a".repeat(64)}`;
          else delete t.status.authorization;
          return { ...t.status };
        }
        if (cmd === "mcp_reply") {
          replies.get(args.id)?.(args);
          return null;
        }
        return null;
      },
    },
    event: {
      listen: async (name, fn) => {
        handlers.set(name, fn);
        return () => handlers.delete(name);
      },
    },
    emit(name, payload) {
      handlers.get(name)?.({ payload });
    },
    /** An AI app's call through the endpoint: resolves to the window's reply. */
    call(id, method, params, client = "claude-code") {
      const p = new Promise((resolve) => replies.set(id, resolve));
      t.emit("mcp-call", { id, method, params, client });
      return p;
    },
  };
  return t;
}

/** Calculator's parts for the tools: a 48GX whose backend answers `run`. */
function fakeView() {
  const store = { state: { booted: "48gx", romName: "gxrom-r", frame: null, keysDown: [], busy: false } };
  const backend = {
    run: async (text) => ({ ok: true, text }),
    stack: async () => ({ items: [] }),
    storeText: async () => ({ ok: true }),
    entryModes: async () => ({}),
  };
  return { ai: async () => ({ backend, store, reference: null }) };
}

/** A confirmation that answers `answer` and counts the questions. */
function fakeConfirm(answer) {
  const asked = [];
  return { asked, module: async () => ({ confirmDialog: async (q) => { asked.push(q); return typeof answer === "function" ? answer(q) : answer; } }) };
}

async function loaded(opts = {}) {
  const tauri = fakeTauri(opts.status);
  const confirm = fakeConfirm(opts.answer ?? "allow");
  const ai = new LocalAiAccess({ calculatorView: () => fakeView(), tauri, confirm: confirm.module, doc: {} });
  await ai.load();
  return { ai, tauri, confirm };
}

test("the window attaches the web app's instructions and a time limit per tool", async () => {
  const { tauri } = await loaded();
  const attach = tauri.calls.find(([c]) => c === "mcp_attach")[1];
  assert.equal(attach.instructions, INSTRUCTIONS);
  const names = aiTools({ calculators: async () => [], target: async () => null, reference: null, png: null }).map((t) => t.name);
  assert.deepEqual(Object.keys(attach.limits), names);
  assert.equal(attach.limits.run_rpl, CALL_LIMIT_MS);
  assert.equal(attach.limits.store_variable, CONFIRM_LIMIT_MS, "it asks the user first");
});

test("tools/list and tools/call through the bridge, with the AI's calls", async () => {
  const { ai, tauri } = await loaded();
  const list = await tauri.call(1, "tools/list", {});
  assert.equal(list.error, null);
  assert.ok(list.result.tools.some((t) => t.name === "run_rpl"));
  const events = [];
  ai.addEventListener("toolcall", (e) => events.push(["start", e.detail]));
  ai.addEventListener("toolcallend", (e) => events.push(["end", e.detail]));
  const r = await tauri.call(2, "tools/call", { name: "list_calculators", arguments: {} }, "Claude Code");
  assert.equal(r.result.structuredContent.calculators[0].model, "48gx");
  assert.equal(events[0][0], "start");
  assert.equal(events[0][1].client.name, "Claude Code");
  assert.equal(events[1][1].outcome, "ok");
  assert.equal(typeof events[1][1].durationMs, "number");
  assert.equal(ai.log.entries.at(-1).app, "Claude Code", "The AI's calls has it");
  // Unknown tools and bad arguments are error results, logged as errors.
  const unknown = await tauri.call(3, "tools/call", { name: "nope" });
  assert.equal(unknown.result.isError, true);
  const bad = await tauri.call(4, "tools/call", { name: "run_rpl", arguments: { text: 5 } });
  assert.equal(bad.result.isError, true);
  assert.match(bad.result.content[0].text, /Invalid arguments for tool run_rpl: \/text must be string/);
  assert.equal(events.at(-1)[1].outcome, "error");
  const other = await tauri.call(5, "resources/list", {});
  assert.match(other.error, /not served/);
});

test("a consequential tool asks first; Always allow stops asking for that app and tool", async () => {
  const { ai, tauri, confirm } = await loaded({ answer: "always", status: { running: true, enabled: true } });
  const store = { name: "store_variable", arguments: { name: "X", text: "1" } };
  await tauri.call(1, "tools/call", store, "Cursor");
  assert.equal(confirm.asked.length, 1);
  assert.equal(confirm.asked[0].client.name, "Cursor");
  assert.equal(confirm.asked[0].client.host, "this computer");
  assert.equal(confirm.asked[0].tool.title, "Store a variable");
  await tauri.call(2, "tools/call", store, "Cursor");
  assert.equal(confirm.asked.length, 1, "always allowed for Cursor");
  await tauri.call(3, "tools/call", store, "Claude Code");
  assert.equal(confirm.asked.length, 2, "another app is asked");
  // Turning AI access off forgets it.
  await ai.set(false);
  await ai.set(true);
  await tauri.call(4, "tools/call", store, "Cursor");
  assert.equal(confirm.asked.length, 3);
});

test("declined, and cancelled by the endpoint while the question is open", async () => {
  const tauri = fakeTauri();
  let pending = null;
  const confirm = fakeConfirm((q) => new Promise((resolve) => {
    pending = resolve;
    q.signal.addEventListener("abort", () => resolve("deny"));
  }));
  const ai = new LocalAiAccess({ calculatorView: () => fakeView(), tauri, confirm: confirm.module, doc: {} });
  await ai.load();
  const ends = [];
  ai.addEventListener("toolcallend", (e) => ends.push(e.detail.outcome));
  const purge = { name: "purge_variable", arguments: { name: "X" } };
  const declined = tauri.call(1, "tools/call", purge);
  while (!pending) await turn();
  assert.equal(ai.keepsRunning, true, "the calculator keeps running while a call runs");
  pending("deny");
  assert.equal((await declined).result.content[0].text, "The user declined");
  pending = null;
  const cancelled = tauri.call(2, "tools/call", purge);
  while (!pending) await turn();
  tauri.emit("mcp-cancel", { id: 2 });
  await cancelled;
  assert.deepEqual(ends, ["declined", "cancelled"]);
  assert.equal(ai.keepsRunning, false);
});

test("the setting, the state and the log's notes", async () => {
  const { ai, tauri } = await loaded();
  assert.equal(ai.state, "disconnected");
  const notes = [];
  ai.addEventListener("note", (e) => notes.push(e.detail));
  await ai.connect();
  assert.deepEqual(tauri.calls.at(-1), ["mcp_set", { enabled: true }]);
  assert.equal(ai.state, "connected");
  assert.equal(ai.mcp.address, "http://127.0.0.1:4842/mcp");
  assert.deepEqual(Object.keys(ai.mcp.headers), ["Authorization"]);
  assert.equal(notes[0].what, "listening");
  tauri.status.apps = [{ name: "Claude Code", secondsAgo: 3 }];
  tauri.emit("mcp-status", null);
  await turn();
  assert.equal(ai.mcp.grants[0].client.name, "Claude Code");
  assert.equal(ai.keepsRunning, true, "an AI app is connected");
  assert.equal(notes.at(-1).what, "paired");
  await ai.set(true, 5000);
  assert.deepEqual(tauri.calls.at(-1), ["mcp_set", { enabled: true, port: 5000 }]);
  ai.disconnect();
  await turn();
  assert.equal(ai.state, "disconnected");
  assert.equal(notes.at(-1).what, "off");
  // Never the token in a note or the log.
  assert.ok(!JSON.stringify(notes).includes("a".repeat(64)));
  assert.ok(!ai.log.text().includes("a".repeat(64)));
  // A refused command shows as the error.
  tauri.core.invoke = async () => { throw new Error("Port 5000 is in use by pid 1 (x). Choose another port."); };
  await ai.set(true);
  assert.match(ai.error, /in use/);
});

test("MCP's shape of tools and results, as tab-mcp's client makes it", () => {
  assert.deepEqual(mapAnnotations({ readOnlyHint: true }), { annotations: { readOnlyHint: true, destructiveHint: false }, meta: { readOnlyHint: true } });
  assert.deepEqual(mapAnnotations({ consequentialHint: true, destructiveHint: true }), { annotations: { destructiveHint: true }, meta: { consequentialHint: true } });
  assert.deepEqual(mapAnnotations({}), {});
  assert.deepEqual(toCallToolResult("hi"), { content: [{ type: "text", text: "hi" }] });
  assert.deepEqual(toCallToolResult(undefined), { content: [] });
  assert.deepEqual(toCallToolResult({ a: 1 }), { content: [{ type: "text", text: '{"a":1}' }], structuredContent: { a: 1 } });
  assert.deepEqual(validate({ type: "object", properties: { n: { type: "integer", minimum: 1 } }, required: ["n"] }, { n: 0 }), ["/n must be at least 1"]);
  assert.deepEqual(validate({ type: "array", items: { type: "string" }, minItems: 1 }, []), ["/ must have at least 1 items"]);
  assert.equal(invalidArguments({ name: "t", inputSchema: { type: "object", properties: { s: { type: "string", maxLength: 2 } } } }, { s: "«»" }), undefined, "characters, not UTF-16 units");
  assert.equal(listTools([{ name: "x", description: "d", execute() {} }])[0].inputSchema.type, "object");
});

// --- The same tools in the browser and the desktop app.

/** A relay for the vendored tab-mcp client: answers its hello with a session, passes requests in, collects the answers. */
class FakeRelay {
  constructor() {
    this.socket = null;
    this.answers = new Map();
    this.next = 0;
  }

  WebSocket() {
    const relay = this;
    return class FakeWebSocket extends EventTarget {
      static OPEN = 1;
      constructor() {
        super();
        this.readyState = 0;
        this.protocol = "tab-mcp.v1";
        relay.socket = this;
        setTimeout(() => {
          this.readyState = 1;
          this.dispatchEvent(new Event("open"));
        }, 0);
      }

      send(text) {
        relay.receive(JSON.parse(text));
      }

      close() {
        this.readyState = 3;
      }

      deliver(msg) {
        this.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(msg) }));
      }
    };
  }

  receive(msg) {
    if (msg.type === "hello") {
      this.socket.deliver({
        type: "session", sessionId: "s1", address: "https://relay.test/a/x/mcp", origin: "http://127.0.0.1", expiresAt: new Date(Date.now() + 3600e3).toISOString(),
        limits: { maxMessageBytes: 1 << 20, maxBodyBytes: 1 << 20, requestTimeoutMs: 60000, idleTimeoutMs: 600000, heartbeatMs: 600000, keepAliveMs: 600000, maxConcurrentRequests: 8 },
        grants: [{ grantId: "g1", createdAt: new Date().toISOString(), client: { id: "https://claude.ai/oauth", name: "Claude Code", host: "claude.ai" } }],
      });
      return;
    }
    const a = this.answers.get(msg.id);
    if (!a) return;
    if (msg.type === "http.response") a.resolve(msg.body);
    else if (msg.type === "http.response.chunk") a.body += msg.data;
    else if (msg.type === "http.response.end") a.resolve(a.body);
  }

  /** An AI app's request of the stateless revision; the JSON-RPC answer. */
  async mcp(method, params = {}) {
    const id = `r${++this.next}`;
    const body = JSON.stringify({ jsonrpc: "2.0", id: 1, method, params: { ...params, _meta: { "io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientCapabilities": {} } } });
    const headers = { "content-type": "application/json", accept: "application/json, text/event-stream", "mcp-protocol-version": "2026-07-28", "mcp-method": method };
    if (params.name) headers["mcp-name"] = params.name;
    const text = await new Promise((resolve) => {
      this.answers.set(id, { body: "", resolve });
      this.socket.deliver({ type: "http.request", id, method: "POST", path: "/mcp", headers, body, grant: { grantId: "g1", clientId: "https://claude.ai/oauth", scopes: [] } });
    });
    const json = text.startsWith("event:") ? text.split("\n").find((l) => l.startsWith("data: ")).slice(6) : text;
    return JSON.parse(json);
  }
}

test("the browser (tab-mcp) and the desktop app offer exactly the same tools, schemas and instructions", { timeout: 10_000 }, async () => {
  const relay = new FakeRelay();
  const saved = globalThis.WebSocket;
  globalThis.WebSocket = relay.WebSocket();
  try {
    const web = new AiAccess({
      calculatorView: () => fakeView(),
      relay: "ws://127.0.0.1:9",
      storage: null,
      hub: { running: () => false, client: () => null },
      roms: { kept: async () => [], bytes: async () => null },
      client: () => import("../vendor/tab-mcp-client/tab-mcp-client.min.js"),
    });
    await web.connect({ resume: true });
    const { ai: desktop, tauri } = await loaded();

    const webList = (await relay.mcp("tools/list")).result.tools;
    const desktopList = (await tauri.call(1, "tools/list", {})).result.tools;
    assert.ok(webList.length >= 10);
    assert.deepEqual(desktopList, webList);

    const discover = (await relay.mcp("server/discover")).result;
    const attach = tauri.calls.find(([c]) => c === "mcp_attach")[1];
    assert.equal(attach.instructions, discover.instructions);

    // A call's result, and arguments the schema refuses, the same both ways.
    const call = { name: "list_calculators", arguments: {} };
    const webCall = (await relay.mcp("tools/call", call)).result;
    const desktopCall = (await tauri.call(2, "tools/call", call)).result;
    delete webCall.resultType;
    delete webCall._meta;
    assert.deepEqual(desktopCall, webCall);
    const bad = { name: "read_stack", arguments: { levels: 0 } };
    const webBad = (await relay.mcp("tools/call", bad)).result;
    const desktopBad = (await tauri.call(3, "tools/call", bad)).result;
    assert.equal(webBad.isError, true);
    assert.equal(desktopBad.isError, true);
    assert.equal(desktopBad.content[0].text.split(":")[0], webBad.content[0].text.split(":")[0]);
    web.disconnect();
    void desktop;
  } finally {
    globalThis.WebSocket = saved;
  }
});
