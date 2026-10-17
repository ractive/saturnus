// Forget ROMs in the browser (the home page's, web/home-roms.js) also
// deletes the saved states that hold a ROM (the 49G's carries its
// flash): `node --test web/test/` (just web-test); and the user's slot
// keeps its key and record (iteration 34). WorkerBackend runs against a
// fake Worker (its port to the hub), the home page's commands against a
// fake hub, both over a fake IndexedDB.

import assert from "node:assert/strict";
import { test } from "node:test";

// A fake IndexedDB: one database of one store, a Map.
const stored = new Map();
const later = (f) => setTimeout(f, 0);
globalThis.indexedDB = {
  open() {
    const req = {};
    later(() => {
      req.result = {
        close() {},
        transaction() {
          const tx = {};
          const done = (value) => {
            const r = { result: value };
            later(() => {
              r.onsuccess?.();
              tx.oncomplete?.();
            });
            return r;
          };
          tx.objectStore = () => ({
            get: (k) => done(stored.get(k)),
            put: (v, k) => done(stored.set(k, v)),
            delete: (k) => done(stored.delete(k)),
          });
          return tx;
        },
      };
      req.onsuccess?.();
    });
    return req;
  },
};

// A fake Worker that answers every request with an empty success, and
// `saveState` with a state of three bytes.
const sent = [];
globalThis.Worker = class {
  postMessage(m) {
    sent.push(m);
    const result = m.cmd === "saveState" ? { state: new Uint8Array([1, 2, 3]), cycles: 7 } : {};
    if (m.id) later(() => this.onmessage({ data: { type: "reply", id: m.id, ok: true, result } }));
  }
};

/** The view's join to the hub (shared/calc-hub.js joinHub), over the fake Worker as its port. */
const fakeJoin = (_who, onMessage) => {
  const port = new globalThis.Worker();
  port.onmessage = (e) => onMessage(e.data);
  return Promise.resolve({ port });
};

const { WorkerBackend } = await import("../backend.js");
const { hubRoms } = await import("../home-roms.js");
const { ROM_HOLDING_STATES } = await import("../states.js");

test("Forget ROMs deletes the saved states that hold a ROM", async () => {
  assert.deepEqual(ROM_HOLDING_STATES, ["49g"]);
  const b = hubRoms({ client: () => ({ request: async (cmd, args) => { sent.push({ v: 1, id: sent.length + 1, cmd, ...args }); return {}; } }) });
  const states = new WorkerBackend({ join: fakeJoin });
  const fill = () => {
    for (const m of ["48sx", "48gx", "49g"]) stored.set(m, { state: new Uint8Array(1) });
  };

  fill();
  await b.forgetRom();
  assert.deepEqual(sent.at(-1), { v: 1, id: sent.at(-1).id, cmd: "forgetRom" });
  assert.deepEqual([...stored.keys()].sort(), ["48gx", "48sx"]);
  assert.deepEqual(await states.stateSlot("49g"), { saved: null });

  fill();
  await b.forgetRom("48sx");
  assert.deepEqual([...stored.keys()].sort(), ["48gx", "48sx", "49g"], "one other model's ROM keeps the 49G state");
  await b.forgetRom("49g");
  assert.deepEqual([...stored.keys()].sort(), ["48gx", "48sx"]);
});

// Iteration 34: Save state and Load state use the same slot as before
// (key `model`, `{state, saved, cycles}`), so a state saved by an older
// page still loads.
test("the browser's slot keeps its key and record", async () => {
  stored.clear();
  const b = new WorkerBackend({ join: fakeJoin });
  assert.deepEqual(await b.stateSlot("48gx"), { saved: null });
  const old = { state: new Uint8Array([9]), saved: 1_700_000_000_000, cycles: 1 };
  stored.set("48gx", old);
  assert.deepEqual(await b.stateSlot("48gx"), { saved: old.saved });
  await b.loadStateSlot("48gx");
  assert.equal(sent.at(-1).cmd, "loadState");
  assert.deepEqual([...sent.at(-1).state], [9]);
  const before = Date.now();
  const { saved } = await b.saveStateSlot("48gx");
  assert.ok(saved >= before);
  const rec = stored.get("48gx");
  assert.deepEqual([...rec.state], [1, 2, 3]);
  assert.equal(rec.saved, saved);
  assert.equal(rec.cycles, 7);
  assert.equal(stored.has("auto:48gx"), false, "never the auto slot");
  await assert.rejects(b.loadStateSlot("48sx"), /no saved state/);
});
