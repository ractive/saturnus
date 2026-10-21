// AI access's session (web/ai.js, iteration 55) on a fake hub: the
// calculators other than Calculator's are attached once per id, and every
// hub connection it opens is closed again (an "ai" connection never says
// hidden, so one left open keeps its calculator computing).
import { test } from "node:test";
import assert from "node:assert/strict";
import { AiAccess, keepsRunning, retryDelay, transientFailure } from "../ai.js";

/** A fake hub: `engines` lists `models`, `attach` waits for `release()` (or fails with `fail`). */
function fakeHub(models = ["48gx"]) {
  const clients = [];
  const gates = [];
  const hub = {
    running: () => true,
    client: () => {
      const c = new EventTarget();
      c.closed = 0;
      c.attaches = 0;
      c.close = () => { c.closed++; };
      c.request = (cmd) => {
        if (cmd === "engines") return Promise.resolve(models.map((model) => ({ model, romName: "rom", views: ["tx"] })));
        if (cmd === "attach") {
          c.attaches++;
          return new Promise((resolve, reject) => gates.push({ resolve: () => resolve({ attached: true }), reject }));
        }
        return Promise.resolve(null);
      };
      clients.push(c);
      return c;
    },
  };
  const roms = { kept: async () => models.map((model) => ({ model, name: "rom", sha256: "s" })), bytes: async () => new Uint8Array(4) };
  return { hub, roms, clients, gates, attachers: () => clients.filter((c) => c.attaches) };
}

const access = (f) => new AiAccess({ calculatorView: () => null, relay: "http://127.0.0.1:1", storage: null, hub: f.hub, roms: f.roms });
const turn = () => new Promise((r) => setTimeout(r, 0));

test("parallel calls for one calculator share one attach", { timeout: 2000 }, async () => {
  const f = fakeHub();
  const ai = access(f);
  const a = ai.target("48gx");
  const b = ai.target("48gx");
  while (!f.gates.length) await turn();
  await turn();
  f.gates.forEach((g) => g.resolve());
  const [ta, tb] = await Promise.all([a, b]);
  assert.equal(ta, tb);
  assert.equal(f.attachers().length, 1, "one connection attached");
  assert.equal(await ai.target("48gx"), ta, "and kept for the next call");
});

test("turned off while an attach waits: its connection is closed, the call fails", { timeout: 2000 }, async () => {
  const f = fakeHub();
  const ai = access(f);
  const a = ai.target("48gx");
  while (!f.gates.length) await turn();
  ai.release();
  f.gates[0].resolve();
  await assert.rejects(a, /turned off/);
  assert.ok(f.attachers()[0].closed > 0);
});

test("a refused attach and a calculator gone close their connections", { timeout: 2000 }, async () => {
  const f = fakeHub();
  const ai = access(f);
  const a = ai.target("48gx");
  while (!f.gates.length) await turn();
  f.gates[0].reject(new Error("no"));
  await assert.rejects(a, /no/);
  assert.ok(f.attachers()[0].closed > 0);
  const b = ai.target("48gx");
  while (f.gates.length < 2) await turn();
  f.gates[1].resolve();
  await b;
  const c = f.attachers()[1];
  c.dispatchEvent(new CustomEvent("gone", { detail: "restarted" }));
  assert.ok(c.closed > 0);
  const next = ai.target("48gx");
  while (f.gates.length < 3) await turn();
  f.gates[2].resolve();
  assert.notEqual(await next, await b, "a new connection after it went");
});

test("release closes the attached connections and the lobby", { timeout: 2000 }, async () => {
  const f = fakeHub();
  const ai = access(f);
  const a = ai.target("48gx");
  while (!f.gates.length) await turn();
  f.gates[0].resolve();
  await a;
  ai.release();
  await turn();
  assert.ok(f.clients.every((c) => c.closed > 0));
});

test("the calculators keep running in a hidden tab only while an AI app is connected, not for the relay session alone", () => {
  const ai = (state, grants) => ({ state, mcp: { grants } });
  assert.equal(keepsRunning(ai("connected", [])), false);
  assert.equal(keepsRunning(ai("connected", [{ grantId: "g" }])), true);
  assert.equal(keepsRunning(ai("connecting", [{ grantId: "g" }])), false);
  assert.equal(keepsRunning({ state: "disconnected", mcp: null }), false);
});

test("a connect the browser refused for lack of a click says to press Connect, not the raw error", { timeout: 2000 }, async () => {
  class FakeTabMcp extends EventTarget {
    constructor() { super(); this.state = "disconnected"; this.grants = []; }
    registerTool() { return Promise.resolve(); }
    connect() { return Promise.reject(new DOMException("connect() must be called from a user action, such as a click", "NotAllowedError")); }
  }
  const f = fakeHub();
  const ai = new AiAccess({ calculatorView: () => null, relay: "http://127.0.0.1:1", storage: null, hub: f.hub, roms: f.roms, client: async () => ({ TabMcp: FakeTabMcp }) });
  await assert.rejects(ai.connect());
  assert.equal(ai.error, "Press Connect to turn on AI access.");
});

// Reconnecting (iteration 56): a fake TabMcp whose connects succeed or fail as the test says, fake timers.

/** A fake `TabMcp`: `connect` takes the next of `outcomes` (true, or an Error), `drop(reason)` breaks the session. */
function fakeMcpClass(outcomes) {
  return class FakeTabMcp extends EventTarget {
    constructor() {
      super();
      this.state = "disconnected";
      this.grants = [];
      this.connects = 0;
      FakeTabMcp.last = this;
    }
    registerTool() { return Promise.resolve(); }
    connect() {
      this.connects++;
      const next = outcomes.shift() ?? true;
      if (next instanceof Error) {
        this.state = "disconnected";
        return Promise.reject(next);
      }
      this.state = "connected";
      this.address = "https://relay.example/a/secret/mcp";
      this.dispatchEvent(new CustomEvent("grantschange", { detail: { grants: this.grants } }));
      this.dispatchEvent(new CustomEvent("connected", { detail: {} }));
      return Promise.resolve({});
    }
    disconnect() {
      if (this.state === "disconnected") return;
      this.drop("user");
    }
    drop(reason) {
      this.state = "disconnected";
      this.dispatchEvent(new CustomEvent("disconnected", { detail: { reason, code: 1006 } }));
    }
  };
}

/** Fake timers: `fire()` runs the one waiting; `now` moves with it. */
function fakeTimers() {
  const t = { now: 1_000_000, waiting: [], cleared: 0 };
  t.api = {
    set: (fn, ms) => {
      const h = { fn, ms };
      t.waiting.push(h);
      return h;
    },
    clear: (h) => {
      const i = t.waiting.indexOf(h);
      if (i >= 0) {
        t.waiting.splice(i, 1);
        t.cleared++;
      }
    },
    now: () => t.now,
  };
  t.fire = async () => {
    assert.equal(t.waiting.length, 1, "one retry waits");
    const h = t.waiting.shift();
    t.now += h.ms;
    h.fn();
    for (let i = 0; i < 5; i++) await turn();
    return h.ms;
  };
  return t;
}

function reconnecting(outcomes = []) {
  const f = fakeHub();
  const timers = fakeTimers();
  const Mcp = fakeMcpClass(outcomes);
  const ai = new AiAccess({ calculatorView: () => null, relay: "https://relay.example", storage: null, hub: f.hub, roms: f.roms, client: async () => ({ TabMcp: Mcp }), timers: timers.api });
  return { ai, timers, Mcp };
}

const DOWN = () => new Error("Could not connect to the relay (refused, unreachable, or this page's origin is not allowed).");

test("the waits between retries: 5, 10, 20, 40 s, then every 60 s", () => {
  assert.deepEqual([1, 2, 3, 4, 5, 6, 20].map(retryDelay), [5000, 10000, 20000, 40000, 60000, 60000, 60000]);
});

test("which failed connects are worth another try", () => {
  assert.equal(transientFailure(DOWN()), true, "the relay away");
  assert.equal(transientFailure(new Error("The relay did not answer in time.")), true);
  assert.equal(transientFailure(new Error("The relay ended the connection: shutdown")), true);
  assert.equal(transientFailure(new Error("The relay refused the connection: forbidden origin")), false, "a refused address");
  assert.equal(transientFailure(new Error("The relay ended the connection: replaced")), false);
  assert.equal(transientFailure(new DOMException("no click", "NotAllowedError")), false);
  assert.equal(transientFailure(new DOMException("cancelled", "AbortError")), false);
});

test("a broken connection is tried again with growing waits until it is back, without Keep AI access on", { timeout: 2000 }, async () => {
  const { ai, timers, Mcp } = reconnecting([true, DOWN(), DOWN(), DOWN(), DOWN(), DOWN()]);
  await ai.connect();
  assert.equal(ai.keepOn, false);
  Mcp.last.drop("shutdown");
  assert.equal(ai.lost, true);
  assert.equal(ai.retrying, timers.now + 5000, "the next try's time");
  assert.equal(ai.error, "The relay restarted.");
  const waits = [];
  for (let i = 0; i < 6; i++) waits.push(await timers.fire());
  assert.deepEqual(waits, [5000, 10000, 20000, 40000, 60000, 60000]);
  // The sixth try worked: back, and the waits start from 5 s again.
  assert.equal(ai.state, "connected");
  assert.equal(ai.lost, false);
  assert.equal(ai.retrying, null);
  assert.equal(ai.error, null);
  assert.equal(timers.waiting.length, 0);
  Mcp.last.drop("heartbeat");
  assert.equal(timers.waiting[0].ms, 5000);
});

test("turning AI access off ends the retries; a refusal or another tab taking over ends them too", { timeout: 2000 }, async () => {
  {
    const { ai, timers, Mcp } = reconnecting([true]);
    await ai.connect();
    Mcp.last.drop("network");
    assert.equal(timers.waiting.length, 1);
    ai.disconnect();
    assert.equal(timers.waiting.length, 0, "the waiting retry cancelled");
    assert.equal(ai.lost, false);
    assert.equal(ai.retrying, null);
    assert.equal(ai.error, null);
  }
  {
    const { ai, timers, Mcp } = reconnecting([true, new Error("The relay refused the connection: unknown address")]);
    await ai.connect();
    Mcp.last.drop("network");
    await timers.fire();
    assert.equal(timers.waiting.length, 0, "no more tries");
    assert.equal(ai.lost, false);
    assert.match(ai.error, /refused the connection/);
  }
  {
    const { ai, timers, Mcp } = reconnecting([true]);
    await ai.connect();
    Mcp.last.drop("replaced");
    assert.equal(timers.waiting.length, 0);
    assert.equal(ai.lost, false);
    assert.match(ai.error, /Another tab took over/);
  }
});

test("turned off while a retry connects: no error, no further retry", { timeout: 2000 }, async () => {
  let fail;
  const { ai, timers, Mcp } = reconnecting([true]);
  await ai.connect();
  Mcp.last.drop("network");
  Mcp.last.connect = () => new Promise((_, reject) => { fail = reject; });
  timers.waiting.shift().fn();
  await turn();
  ai.disconnect();
  fail(new DOMException("The connection was cancelled", "AbortError"));
  for (let i = 0; i < 5; i++) await turn();
  assert.equal(ai.error, null);
  assert.equal(timers.waiting.length, 0);
});
