// AI access's session (web/ai.js, iteration 55) on a fake hub: the
// calculators other than Calculator's are attached once per id, and every
// hub connection it opens is closed again (an "ai" connection never says
// hidden, so one left open keeps its calculator computing).
import { test } from "node:test";
import assert from "node:assert/strict";
import { AiAccess } from "../ai.js";

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
