// Forget ROMs in the browser also deletes the saved states that hold a
// ROM (the 49G's carries its flash): `node --test web/test/` (just
// web-test). WorkerBackend runs against a fake Worker and IndexedDB.

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

// A fake Worker that answers every request with an empty success.
const sent = [];
globalThis.Worker = class {
  postMessage(m) {
    sent.push(m);
    if (m.id) later(() => this.onmessage({ data: { type: "reply", id: m.id, ok: true, result: {} } }));
  }
};

const { WorkerBackend, ROM_HOLDING_STATES } = await import("../backend.js");

test("Forget ROMs deletes the saved states that hold a ROM", async () => {
  assert.deepEqual(ROM_HOLDING_STATES, ["49g"]);
  const b = new WorkerBackend();
  const fill = () => {
    for (const m of ["48sx", "48gx", "49g"]) stored.set(m, { state: new Uint8Array(1) });
  };

  fill();
  await b.forgetRom();
  assert.deepEqual(sent.at(-1), { v: 1, id: sent.at(-1).id, cmd: "forgetRom" });
  assert.deepEqual([...stored.keys()].sort(), ["48gx", "48sx"]);
  assert.equal(await b.hasState("49g"), false);

  fill();
  await b.forgetRom("48sx");
  assert.deepEqual([...stored.keys()].sort(), ["48gx", "48sx", "49g"], "one other model's ROM keeps the 49G state");
  await b.forgetRom("49g");
  assert.deepEqual([...stored.keys()].sort(), ["48gx", "48sx"]);
});
