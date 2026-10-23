// "The AI's calls" (web/ai-log.js, iteration 56) without a page: the
// entries AI access's events make (a call and how it ended, the
// connection's events), the last 200 kept, Copy log's text without the
// address, a token or a pairing code; what Calculator's sidebar says in
// each state (`aiView`); and AI access (web/ai.js) handing its client's
// events to the log. The drawer itself: ai-log-page.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { AiLog, aiView, argPairs, durationText, foldOf, noteText, resultText } from "../ai-log.js";
import { AiAccess } from "../ai.js";

const CLIENT = { grantId: "g1", id: "https://claude.ai/cimd.json", host: "claude.ai", name: "Claude" };

/** A stand-in for AiAccess: its events and the tools that ask first. */
function fakeAi() {
  const ai = new EventTarget();
  ai.consequential = new Set(["store_variable"]);
  ai.emit = (type, detail) => ai.dispatchEvent(new CustomEvent(type, { detail }));
  return ai;
}

test("a call is one entry, its end completes it in place: outcome, result, duration", () => {
  const ai = fakeAi();
  let t = Date.UTC(2026, 9, 21, 10, 0, 0);
  const log = new AiLog(ai, { now: () => t });
  const changes = [];
  log.addEventListener("change", (e) => changes.push(e.detail));
  ai.emit("toolcall", { id: 1, tool: "run_rpl", title: "Run RPL", arguments: { text: "2 3 +", calculator: "48gx" }, client: CLIENT });
  ai.emit("toolcall", { id: 2, tool: "store_variable", arguments: { name: "AIT", text: "« 2 * »" }, client: { grantId: "g2", id: "https://x.example/c.json", host: "x.example" } });
  assert.equal(log.entries.length, 2);
  const [a, b] = log.entries;
  assert.deepEqual([a.app, a.title, a.asks, a.outcome], ["Claude", "Run RPL", false, null]);
  assert.deepEqual(a.args, [["text", "2 3 +"], ["calculator", "48gx"]]);
  assert.deepEqual([b.app, b.title, b.asks], ["x.example", "store_variable", true], "no name: the host; no title: the tool's name; it asks first");
  t += 120;
  ai.emit("toolcallend", { id: 1, tool: "run_rpl", client: CLIENT, outcome: "ok", result: { content: [{ type: "text", text: '{"stack":[{"text":"5"}]}' }, { type: "image", mimeType: "image/png", data: "…" }] }, durationMs: 120 });
  ai.emit("toolcallend", { id: 2, tool: "store_variable", client: CLIENT, outcome: "declined", result: { content: [{ type: "text", text: "The user declined." }], isError: true }, durationMs: 2400 });
  assert.equal(log.entries.length, 2, "ends add no entries");
  assert.equal(a.outcome, "ok");
  assert.equal(a.result, '{"stack":[{"text":"5"}]}\n[image, image/png]');
  assert.equal(a.durationMs, 120);
  assert.deepEqual([b.outcome, b.result], ["declined", ""]);
  assert.equal(log.pending.size, 0);
  assert.equal(changes.filter((c) => c.ended).length, 2);
});

test("the connection's events in words", () => {
  assert.equal(noteText({ what: "connected", host: "tab-mcp.ractive.ch" }), "Connected to the relay tab-mcp.ractive.ch");
  assert.equal(noteText({ what: "lost", text: "The relay restarted." }), "Connection lost. The relay restarted.");
  assert.equal(noteText({ what: "retrying", attempt: 3 }), "Retrying to connect… (attempt 3)");
  assert.equal(noteText({ what: "wait", seconds: 20 }), "Next try in 20 s");
  assert.equal(noteText({ what: "paired", app: "Claude" }), "Claude connected");
  assert.equal(noteText({ what: "unpaired", app: "Claude" }), "Claude disconnected");
  assert.equal(noteText({ what: "off" }), "AI access turned off");
  assert.equal(durationText(85), "85 ms");
  assert.equal(durationText(1234), "1.2 s");
  assert.deepEqual(argPairs({ keys: ["4", "enter"], n: 2 }), [["keys", '["4","enter"]'], ["n", "2"]]);
  assert.equal(resultText({ content: [], structuredContent: { a: 1 } }), '{"a":1}');
});

test("the last 200 entries are kept; Clear empties the log", () => {
  const ai = fakeAi();
  const log = new AiLog(ai);
  for (let i = 0; i < 205; i++) ai.emit("toolcall", { id: i, tool: "read_stack", arguments: {}, client: CLIENT });
  assert.equal(log.entries.length, 200);
  assert.equal(log.entries[0].id, 5);
  assert.equal(log.pending.size, 200, "the dropped calls are forgotten");
  log.clear();
  assert.equal(log.entries.length, 0);
  assert.equal(log.text(), "");
});

test("a long outage's connection events go before the calls", () => {
  const ai = fakeAi();
  const log = new AiLog(ai);
  for (let i = 0; i < 10; i++) ai.emit("toolcall", { id: i, tool: "read_stack", arguments: {}, client: CLIENT });
  for (let i = 1; i <= 300; i++) ai.emit("note", { what: "retrying", attempt: i });
  assert.equal(log.entries.length, 200);
  assert.equal(log.entries.filter((e) => e.kind === "call").length, 10, "every call kept");
  assert.match(log.entries.at(-1).text, /300/);
});

test("folded: whole hidden lines are counted; a head cut inside a line says more", () => {
  assert.equal(foldOf(["a", "b", "c", "d", "e", "f"].join("\n")).more, "3 more lines");
  assert.equal(foldOf(["y".repeat(500), "b", "c", "d", "e"].join("\n")).more, "more");
  assert.equal(foldOf("a\nb").rest, "");
});

test("Copy log's text: every entry in full, nothing folded", () => {
  const ai = fakeAi();
  const log = new AiLog(ai, { now: () => new Date(2026, 9, 21, 9, 5, 7).getTime() });
  ai.emit("note", { what: "connected", host: "relay.example" });
  ai.emit("toolcall", { id: 1, tool: "store_variable", title: "Store a variable", arguments: { name: "P", text: "«\n1 2 +\n»" }, client: CLIENT });
  ai.emit("toolcallend", { id: 1, tool: "store_variable", client: CLIENT, outcome: "error", result: { content: [{ type: "text", text: "Bad argument type" }], isError: true }, durationMs: 30 });
  ai.emit("toolcall", { id: 2, tool: "read_stack", arguments: {}, client: CLIENT });
  assert.equal(log.text(), [
    "09:05:07  Connected to the relay relay.example",
    "09:05:07  Claude  Store a variable  error, 30 ms",
    "          name: P",
    "          text: «",
    "            1 2 +",
    "            »",
    "          → Bad argument type",
    "09:05:07  Claude  read_stack  running",
  ].join("\n"));
});

test("the sidebar says the state: off, connecting, on without an app, connected apps, lost", () => {
  const v = (a) => aiView({ lost: false, error: null, mcp: null, state: "disconnected", ...a });
  assert.deepEqual(v({}), { entry: "Connect your AI app…", note: null, said: "" });
  assert.equal(v({ error: "Another tab took over this address. AI access is on there now." }).note, "Another tab took over this address. AI access is on there now.");
  assert.equal(v({ state: "connecting" }).entry, "AI access: connecting…");
  assert.deepEqual(v({ state: "connected", mcp: { grants: [] } }), { entry: "AI access on · no app yet", actions: "show", said: "AI access on. No app connected yet." });
  const one = v({ state: "connected", mcp: { grants: [{ grantId: "g", client: { name: "Claude", host: "claude.ai" } }] } });
  assert.deepEqual([one.entry, one.dot, one.actions], ["Claude is connected", "on", "connected"]);
  const two = v({ state: "connected", mcp: { grants: [{ grantId: "g", client: { name: "", host: "claude.ai" } }, { grantId: "h", client: { name: "ChatGPT", host: "chatgpt.com" } }] } });
  assert.equal(two.entry, "2 AI apps are connected");
  assert.equal(v({ state: "connected", mcp: { grants: [{ grantId: "g", client: { name: "", host: "claude.ai" } }] } }).entry, "claude.ai is connected");
  for (const state of ["disconnected", "connecting"]) {
    const lost = v({ state, lost: true, error: "The relay restarted." });
    assert.deepEqual([lost.entry, lost.dot, lost.actions], ["AI access lost · reconnecting…", "lost", "lost"]);
  }
});

test("AI access hands the calls and the connection's events to its log, never the address or the pairing code", { timeout: 2000 }, async () => {
  const outcomes = [true];
  let mcp;
  class FakeTabMcp extends EventTarget {
    constructor() {
      super();
      mcp = this;
      this.state = "disconnected";
      this.grants = [{ grantId: "g1", client: { id: CLIENT.id, name: "Claude", host: "claude.ai" } }];
      this.address = "https://relay.example/a/abcdefsecret/mcp";
      this.pairingCode = { code: "XKCD-4242", expiresAt: "" };
    }
    registerTool() { return Promise.resolve(); }
    connect() {
      const ok = outcomes.shift() ?? true;
      if (!ok) return Promise.reject(new Error("down"));
      this.state = "connected";
      this.dispatchEvent(new CustomEvent("grantschange", { detail: { grants: this.grants } }));
      this.dispatchEvent(new CustomEvent("connected", { detail: { address: this.address } }));
      return Promise.resolve({});
    }
    disconnect() {
      if (this.reconnecting) {
        this.reconnecting = undefined;
        this.dispatchEvent(new CustomEvent("reconnecting", { detail: null }));
      }
      if (this.state === "disconnected") return;
      this.state = "disconnected";
      this.dispatchEvent(new CustomEvent("disconnected", { detail: { reason: "user", code: 1000 } }));
    }
  }
  const ai = new AiAccess({ calculatorView: () => null, relay: "https://relay.example", storage: null, client: async () => ({ TabMcp: FakeTabMcp }), now: () => 0, hub: { running: () => false, client: () => null }, roms: { kept: async () => [], bytes: async () => null } });
  await ai.connect();
  assert.ok(ai.consequential.has("store_variable") && ai.consequential.has("purge_variable"), "the tools that ask first");
  // A second app pairs, the first goes.
  const second = { grantId: "g2", client: { id: "https://chatgpt.com/c.json", name: "ChatGPT", host: "chatgpt.com" } };
  mcp.grants = [mcp.grants[0], second];
  mcp.dispatchEvent(new CustomEvent("grantschange", { detail: { grants: mcp.grants } }));
  mcp.grants = [second];
  mcp.dispatchEvent(new CustomEvent("grantschange", { detail: { grants: mcp.grants } }));
  mcp.dispatchEvent(new CustomEvent("toolcall", { detail: { id: 7, tool: "store_variable", title: "Store a variable", arguments: { name: "A", text: "1" }, client: CLIENT } }));
  mcp.dispatchEvent(new CustomEvent("toolcallend", { detail: { id: 7, tool: "store_variable", client: CLIENT, outcome: "ok", result: { content: [{ type: "text", text: '{"stored":true}' }] }, durationMs: 900 } }));
  // The connection breaks; the client says it reconnects (its `reconnecting`).
  mcp.state = "disconnected";
  mcp.dispatchEvent(new CustomEvent("disconnected", { detail: { reason: "network", code: 1006 } }));
  mcp.reconnecting = { reason: "network", attempt: 1, at: 5000 };
  mcp.dispatchEvent(new CustomEvent("reconnecting", { detail: mcp.reconnecting }));
  ai.disconnect();
  const lines = ai.log.entries.map((e) => (e.kind === "event" ? e.text : `${e.app} ${e.title} ${e.outcome} ${e.asks}`));
  assert.deepEqual(lines, [
    "Connected to the relay relay.example",
    "ChatGPT connected",
    "Claude disconnected",
    "Claude Store a variable ok true",
    "Connection lost. The connection to the relay broke.",
    "Next try in 5 s",
    "AI access turned off",
  ]);
  const text = ai.log.text();
  assert.ok(!text.includes("abcdefsecret") && !text.includes("XKCD-4242"), text);
});

test("the drawer goes beside the calculator where the stage has room, else along the bottom", async () => {
  const { besideBox, BESIDE_MIN } = await import("../ai-log.js");
  const stage = (width, height) => ({ left: 320, top: 0, right: 320 + width, bottom: height, width, height });
  // 1366x683 with the panel: the stage is 1046 wide.
  assert.deepEqual(besideBox(stage(1046, 683), { below: 58 }), { left: 915, top: 70, width: 439, height: 601 });
  // 1920x1080: at most 640 wide.
  assert.equal(besideBox(stage(1600, 1080)).width, 640);
  assert.equal(besideBox(stage(BESIDE_MIN - 1, 900)), null, "too narrow: a sheet");
  assert.equal(besideBox(stage(1046, 683), { phone: true }), null);
});
