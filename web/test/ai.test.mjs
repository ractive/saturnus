// AI access's session (web/ai.js, iteration 55) on a fake hub: the
// calculators other than Calculator's are attached once per id, and every
// hub connection it opens is closed again (an "ai" connection never says
// hidden, so one left open keeps its calculator computing).
import { test } from "node:test";
import assert from "node:assert/strict";
import { AiAccess, keepsRunning, screenPng } from "../ai.js";

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

test("existing never attaches: null for a background calculator until a tool attached it", { timeout: 2000 }, async () => {
  const f = fakeHub(["49g"]);
  const ai = access(f);
  assert.equal(await ai.existing("49g"), null);
  assert.equal(f.attachers().length, 0, "nothing attached");
  assert.ok(!f.clients.some((c) => c.attaches), "no attach sent");
  const a = ai.target("49g");
  while (!f.gates.length) await turn();
  await turn();
  f.gates.forEach((g) => g.resolve());
  const t = await a;
  assert.equal(await ai.existing("49g"), t, "the attached one");
  assert.equal(f.attachers().length, 1);
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

// Reconnecting: the client library (tab-mcp 0.4 on) connects again by
// itself; AI access shows it. A fake TabMcp that models the library's
// `reconnecting` (its getter and event, its waits) on fake timers; its
// connects succeed or fail as the test says.

/** A connect that never settles (the test ends it). */
const HANG = Symbol("hang");

/**
 * A fake `TabMcp` that does what the library (0.5.1) does: `connect` takes
 * the next of `outcomes` (true, an Error, HANG, or a promise the test
 * settles), a connect while one runs shares it, `drop(reason)` breaks the
 * session.
 */
function fakeMcpClass(outcomes, timers) {
  const DELAYS = [5000, 10000, 20000, 40000, 60000];
  const AGAIN = new Set(["network", "heartbeat", "shutdown"]);
  return class FakeTabMcp extends EventTarget {
    constructor(options) {
      super();
      this.options = options;
      this.state = "disconnected";
      this.grants = [];
      this.connects = 0;
      this.reconnecting = undefined;
      this.timer = null;
      /** The running attempt's promise; `pageWaits`: a connect() of the page has it. */
      this.connecting = null;
      this.pageWaits = false;
      FakeTabMcp.last = this;
    }
    registerTool() { return Promise.resolve(); }
    emit(type, detail) { this.dispatchEvent(new CustomEvent(type, { detail })); }
    setState(state) {
      if (this.state === state) return;
      this.state = state;
      this.emit("statechange", { state });
    }
    connect() {
      if (this.state === "connected") return Promise.resolve({});
      if (this.connecting) {
        if (this.reconnecting) this.pageWaits = true;
        return this.connecting;
      }
      return this.attempt(true);
    }
    /** One attempt; while reconnecting, a failure that may pass schedules the next, any other stops (with `error` when no connect() of the page waits). */
    attempt(byPage) {
      timers.api.clear(this.timer);
      this.timer = null;
      this.connects++;
      const info = this.reconnecting;
      this.pageWaits = byPage;
      this.setState("connecting");
      const next = outcomes.shift() ?? true;
      const ready = next === HANG ? new Promise(() => {}) : Promise.resolve(next).then((v) => { if (v instanceof Error) throw v; });
      const connecting = ready.then(
        () => {
          if (this.connecting !== connecting) return {};
          this.connecting = null;
          this.stop();
          this.setState("connected");
          this.address = "https://relay.example/a/secret/mcp";
          this.emit("grantschange", { grants: this.grants });
          this.emit("connected", {});
          return {};
        },
        (err) => {
          if (this.connecting !== connecting) throw err;
          this.connecting = null;
          this.setState("disconnected");
          const pageWaits = this.pageWaits;
          this.pageWaits = false;
          if (info && this.reconnecting === info) {
            if (err.reason === undefined || AGAIN.has(err.reason)) this.schedule(info.reason, info.attempt + 1);
            else this.stop(pageWaits ? undefined : err);
          }
          throw err;
        },
      );
      this.connecting = connecting;
      return connecting;
    }
    disconnect() {
      this.stop();
      this.connecting = null;
      const was = this.state === "connected";
      this.setState("disconnected");
      if (was) this.emit("disconnected", { reason: "user", code: 1000 });
    }
    drop(reason) {
      this.setState("disconnected");
      this.emit("disconnected", { reason, code: 1006 });
      if (AGAIN.has(reason) && this.state === "disconnected") this.schedule(reason, 1);
    }
    schedule(reason, attempt) {
      const ms = DELAYS[Math.min(attempt, DELAYS.length) - 1];
      this.timer = timers.api.set(() => {
        this.timer = null;
        this.attempt(false).catch(() => {});
      }, ms);
      this.reconnecting = { reason, attempt, at: timers.now + ms };
      this.emit("reconnecting", this.reconnecting);
    }
    stop(why) {
      timers.api.clear(this.timer);
      this.timer = null;
      if (this.reconnecting === undefined) return;
      this.reconnecting = undefined;
      if (why) this.emit("error", { error: why });
      this.emit("reconnecting", null);
    }
  };
}

/** Fake timers: `fire()` runs the one waiting; `now` moves with it. */
function fakeTimers() {
  const t = { now: 1_000_000, waiting: [] };
  t.api = {
    set: (fn, ms) => {
      const h = { fn, ms };
      t.waiting.push(h);
      return h;
    },
    clear: (h) => {
      const i = t.waiting.indexOf(h);
      if (i >= 0) t.waiting.splice(i, 1);
    },
  };
  t.fire = async () => {
    assert.equal(t.waiting.length, 1, "one attempt waits");
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
  const Mcp = fakeMcpClass(outcomes, timers);
  const ai = new AiAccess({ calculatorView: () => null, relay: "https://relay.example", storage: null, hub: f.hub, roms: f.roms, client: async () => ({ TabMcp: Mcp }), now: () => timers.now });
  return { ai, timers, Mcp };
}

const DOWN = () => Object.assign(new Error("Could not connect to the relay (refused, unreachable, or this page's origin is not allowed)."), { reason: "network" });
const REFUSED = () => Object.assign(new Error("The relay refused the connection: unknown address"), { reason: "policy" });
/** The log's connection events. */
const notes = (ai) => ai.log.entries.filter((e) => e.kind === "event").map((e) => e.text);

test("the library's own reconnect is on: the page keeps no retries of its own", async () => {
  const { ai, Mcp } = reconnecting([true]);
  await ai.connect();
  assert.equal(Mcp.last.options.reconnect, undefined, "the library's default");
});

test("a broken connection: lost while the library tries again, each wait and attempt in the log, until it is back; without Keep AI access on", { timeout: 2000 }, async () => {
  const { ai, timers, Mcp } = reconnecting([true, DOWN(), DOWN(), DOWN(), DOWN(), DOWN()]);
  await ai.connect();
  assert.equal(ai.keepOn, false);
  assert.equal(ai.lost, false);
  Mcp.last.drop("shutdown");
  assert.equal(ai.lost, true);
  assert.equal(ai.error, "The relay restarted.");
  const waits = [];
  for (let i = 0; i < 6; i++) {
    waits.push(await timers.fire());
    if (i < 5) assert.equal(ai.lost, true, "still lost between the attempts");
  }
  assert.deepEqual(waits, [5000, 10000, 20000, 40000, 60000, 60000]);
  // The sixth attempt worked.
  assert.equal(ai.state, "connected");
  assert.equal(ai.lost, false);
  assert.equal(ai.error, null);
  assert.equal(timers.waiting.length, 0);
  assert.deepEqual(notes(ai), [
    "Connected to the relay relay.example",
    "Connection lost. The relay restarted.",
    "Next try in 5 s",
    "Retrying to connect… (attempt 1)",
    "Next try in 10 s",
    "Retrying to connect… (attempt 2)",
    "Next try in 20 s",
    "Retrying to connect… (attempt 3)",
    "Next try in 40 s",
    "Retrying to connect… (attempt 4)",
    "Next try in 60 s",
    "Retrying to connect… (attempt 5)",
    "Next try in 60 s",
    "Retrying to connect… (attempt 6)",
    "Connected to the relay relay.example",
  ]);
});

test("Connect while lost makes the library's next attempt at once", { timeout: 2000 }, async () => {
  const { ai, timers, Mcp } = reconnecting([true, DOWN(), true]);
  await ai.connect();
  Mcp.last.drop("network");
  await timers.fire();
  assert.equal(timers.waiting[0].ms, 10000);
  await ai.connect({ resume: true });
  assert.equal(ai.state, "connected");
  assert.equal(ai.lost, false);
  assert.equal(timers.waiting.length, 0);
  assert.equal(notes(ai).at(-2), "Retrying to connect… (attempt 2)");
});

test("a failed attempt the library tries again later leaves the error at why it was lost", { timeout: 2000 }, async () => {
  const { ai, Mcp } = reconnecting([true, DOWN()]);
  await ai.connect();
  Mcp.last.drop("heartbeat");
  await assert.rejects(ai.connect());
  assert.equal(ai.lost, true);
  assert.equal(ai.error, "The connection to the relay broke.");
});

test("turning AI access off ends the reconnecting; a refusal or another tab taking over ends it too", { timeout: 2000 }, async () => {
  {
    const { ai, timers, Mcp } = reconnecting([true]);
    await ai.connect();
    Mcp.last.drop("network");
    assert.equal(timers.waiting.length, 1);
    ai.disconnect();
    assert.equal(timers.waiting.length, 0, "the waiting attempt cancelled");
    assert.equal(ai.lost, false);
    assert.equal(ai.error, null);
    assert.equal(notes(ai).at(-1), "AI access turned off");
  }
  {
    const { ai, timers, Mcp } = reconnecting([true, REFUSED()]);
    await ai.connect();
    Mcp.last.drop("network");
    await timers.fire();
    assert.equal(timers.waiting.length, 0, "no more attempts");
    assert.equal(ai.lost, false);
    assert.equal(ai.error, "Could not connect to the relay: The relay refused the connection: unknown address", "the relay's words, from the client's error (0.5.1)");
  }
  {
    // Refused on Connect: the relay's words.
    const { ai, timers, Mcp } = reconnecting([true, REFUSED()]);
    await ai.connect();
    Mcp.last.drop("network");
    await assert.rejects(ai.connect());
    assert.equal(timers.waiting.length, 0);
    assert.equal(ai.lost, false);
    assert.equal(ai.error, "Could not connect to the relay: The relay refused the connection: unknown address");
  }
  {
    const { ai, timers, Mcp } = reconnecting([true]);
    await ai.connect();
    Mcp.last.drop("replaced");
    assert.equal(timers.waiting.length, 0);
    assert.equal(ai.lost, false);
    assert.match(ai.error, /Another tab took over/);
    // Then who (tab-mcp 0.8, `replaced` after `disconnected`): a sibling tab of this page, nothing to do.
    Mcp.last.emit("replaced", { sibling: true });
    assert.deepEqual(ai.replaced, { sibling: true });
    assert.match(ai.error, /^Opened in another tab\./);
    // Connecting again clears it.
    await ai.connect();
    assert.equal(ai.replaced, null);
    assert.equal(ai.error, null);
  }
  {
    // Another tab or program with the secret: the warning; a new address takes it back.
    const { ai, Mcp } = reconnecting([true, true]);
    await ai.connect();
    Mcp.last.drop("replaced");
    Mcp.last.emit("replaced", { sibling: false });
    assert.deepEqual(ai.replaced, { sibling: false });
    assert.equal(ai.error, "Another tab or program took over this page.");
    Mcp.last.newAddress = () => Mcp.last.connect();
    const made = ai.newAddress();
    assert.equal(ai.replaced, null);
    assert.equal(ai.error, null, "the takeover's words go as soon as the address is taken back");
    await made;
    assert.equal(ai.replaced, null);
    assert.equal(ai.error, null);
    // Taken over again while it was taken back: the client's reason, in the dialog's words.
    Mcp.last.drop("replaced");
    Mcp.last.newAddress = () => Promise.reject(Object.assign(new Error("taken"), { reason: "replaced" }));
    await assert.rejects(ai.newAddress());
    assert.equal(ai.error, "It was taken over again before its AI apps were disconnected. Try again.");
  }
});

test("turned off while an attempt connects: no error, no further attempt", { timeout: 2000 }, async () => {
  const { ai, timers, Mcp } = reconnecting([true, HANG]);
  await ai.connect();
  Mcp.last.drop("network");
  await timers.fire();
  assert.equal(ai.state, "connecting");
  assert.equal(ai.lost, true);
  ai.disconnect();
  for (let i = 0; i < 5; i++) await turn();
  assert.equal(ai.lost, false);
  assert.equal(ai.error, null);
  assert.equal(timers.waiting.length, 0);
});

test("a new address while lost: not taken for a refusal", { timeout: 2000 }, async () => {
  const { ai, Mcp } = reconnecting([true, HANG]);
  await ai.connect();
  // As the library's: the reconnecting stops, then it connects with the new address.
  Mcp.last.newAddress = function () {
    this.stop();
    return this.connect();
  };
  Mcp.last.drop("network");
  ai.newAddress();
  assert.equal(ai.state, "connecting");
  assert.equal(ai.error, null);
});

test("Connect while the library's own attempt runs joins it: one attempt, one Retrying note", { timeout: 2000 }, async () => {
  {
    let settle;
    const { ai, timers, Mcp } = reconnecting([true, new Promise((r) => { settle = r; })]);
    await ai.connect();
    Mcp.last.drop("network");
    await timers.fire();
    assert.equal(ai.state, "connecting");
    const joined = ai.connect();
    for (let i = 0; i < 5; i++) await turn();
    assert.equal(Mcp.last.connects, 2, "the first connect and the one attempt");
    settle(true);
    await joined;
    assert.equal(ai.state, "connected");
    assert.equal(ai.lost, false);
    assert.equal(notes(ai).filter((n) => n.startsWith("Retrying")).length, 1);
  }
  {
    // Refused while Connect waits on it: the client's promise says why, no `error`.
    let settle;
    const { ai, timers, Mcp } = reconnecting([true, new Promise((r) => { settle = r; })]);
    await ai.connect();
    Mcp.last.drop("network");
    await timers.fire();
    let errors = 0;
    Mcp.last.addEventListener("error", () => errors++);
    const joined = ai.connect();
    settle(REFUSED());
    await assert.rejects(joined);
    assert.equal(errors, 0);
    assert.equal(Mcp.last.connects, 2);
    assert.equal(ai.lost, false);
    assert.equal(ai.error, "Could not connect to the relay: The relay refused the connection: unknown address");
  }
});

test("no stale lost text once the reconnecting stops: on every change while it comes back, and while a refused Connect settles", { timeout: 2000 }, async () => {
  {
    const { ai, timers, Mcp } = reconnecting([true]);
    await ai.connect();
    Mcp.last.drop("shutdown");
    const seen = [];
    ai.addEventListener("change", () => seen.push([ai.lost, ai.state, ai.error]));
    await timers.fire();
    assert.equal(ai.state, "connected");
    for (const [lost, state, error] of seen) {
      if (lost) assert.equal(error, "The relay restarted.", `lost while ${state}: why`);
      else assert.equal(error, null, `not lost while ${state}: no error`);
    }
    assert.ok(seen.some(([lost, state]) => !lost && state === "connecting"), "the client stops reconnecting before it says connected");
  }
  {
    // A refused attempt Connect made: no `error` from the client, the catch says why; nothing stale between.
    const { ai, Mcp } = reconnecting([true, REFUSED()]);
    await ai.connect();
    Mcp.last.drop("shutdown");
    const seen = [];
    ai.addEventListener("change", () => seen.push([ai.lost, ai.error]));
    await assert.rejects(ai.connect());
    assert.ok(seen.every(([lost, error]) => lost || error !== "The relay restarted."), JSON.stringify(seen));
    assert.equal(ai.error, "Could not connect to the relay: The relay refused the connection: unknown address");
  }
});

test("screenPng in a look: the LCD's colours or black on white; AiAccess takes the setting when a call names none", async () => {
  const frame = { width: 131, height: 64, pixels: Buffer.alloc(17 * 64).toString("base64"), annunciators: {} };
  const plte = (b64) => { const png = Buffer.from(b64, "base64"); const at = png.indexOf("PLTE"); return [...png.subarray(at + 4, at + 4 + png.readUInt32BE(at - 4))]; };
  const bw = await screenPng(frame, "48gx", "bw");
  const lcd = await screenPng(frame, "48gx", "lcd");
  assert.ok(["255,255,255,0,0,0", "0,0,0,255,255,255"].includes(plte(bw).join()), plte(bw).join());
  assert.notDeepEqual(plte(lcd), plte(bw));
  assert.equal(await screenPng(frame, "48gx"), lcd, "LCD by default");
  let setting = "bw";
  const f = fakeHub();
  const ai = new AiAccess({ calculatorView: () => null, relay: "http://127.0.0.1:1", storage: null, hub: f.hub, roms: f.roms, look: () => setting });
  assert.equal(await ai.png(frame, "48gx"), bw);
  setting = "lcd";
  assert.equal(await ai.png(frame, "48gx"), lcd);
  assert.equal(await ai.png(frame, "48gx", "bw"), bw, "the call's look over the setting");
});

test("the AI's LCD image ignores the calculator's contrast: turned light, the ink stays as at power-on", async () => {
  const base = { width: 131, height: 64, pixels: Buffer.alloc(17 * 64, 0x0f).toString("base64"), annunciators: {}, contrastRange: [0, 31], contrastDefault: 13 };
  const normal = await screenPng({ ...base, contrast: 13 }, "48gx", "lcd");
  assert.equal(await screenPng({ ...base, contrast: 0 }, "48gx", "lcd"), normal, "lightest");
  assert.equal(await screenPng({ ...base, contrast: 31 }, "48gx", "lcd"), normal, "darkest");
  assert.equal(await screenPng({ ...base, contrastRange: undefined }, "48gx", "lcd"), normal, "no contrast sent");
});

test("screenPng: the display's PNG as base64; null where it cannot be made, never a rejection", async () => {
  const frame = { width: 131, height: 64, pixels: Buffer.alloc(17 * 64).toString("base64"), annunciators: {} };
  const png = Buffer.from(await screenPng(frame, "48gx"), "base64");
  assert.deepEqual([...png.subarray(1, 4)], [80, 78, 71]);
  assert.equal(await screenPng(null, "48gx"), null);
  const saved = globalThis.CompressionStream;
  globalThis.CompressionStream = undefined; // and no canvas in node: the fallback fails too
  try {
    assert.equal(await screenPng(frame, "48gx"), null);
  } finally {
    globalThis.CompressionStream = saved;
  }
});
