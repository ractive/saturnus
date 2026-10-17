// The serial port on both backends (web/protocol.md, "The serial port"):
// the commands each sends, the bytes as a Uint8Array to the Worker and as
// base64 to the Tauri app, and the `serial` event's bytes a Uint8Array
// from either. `node --test web/test/` (just web-test). The backends run
// against a fake Worker and a fake Tauri.

import assert from "node:assert/strict";
import { test } from "node:test";

const later = (f) => setTimeout(f, 0);

// A fake Worker that answers every request with null.
const posted = [];
let worker = null;
globalThis.Worker = class {
  constructor() {
    worker = this;
  }
  postMessage(m) {
    posted.push(m);
    if (m.id) later(() => this.onmessage({ data: { type: "reply", id: m.id, ok: true, result: null } }));
  }
};

/** The view's join to the hub (shared/calc-hub.js joinHub), over the fake Worker as its port. */
const fakeJoin = (_who, onMessage) => {
  const port = new globalThis.Worker();
  port.onmessage = (e) => onMessage(e.data);
  return Promise.resolve({ port });
};

const { WorkerBackend, TauriBackend, fromJson } = await import("../backend.js");

test("the Worker gets the bytes as they are, and gives them as they are", async () => {
  const b = new WorkerBackend({ join: fakeJoin });
  const bytes = new Uint8Array([1, 0x41, 0xff]);
  await b.serialOpen();
  await b.serialWrite(bytes);
  await b.serialClose();
  assert.deepEqual(
    posted.map(({ id, ...m }) => m),
    [
      { v: 1, cmd: "serialOpen" },
      { v: 1, cmd: "serialWrite", bytes },
      { v: 1, cmd: "serialClose" },
    ],
  );
  assert.equal(posted[1].bytes, bytes, "not copied, not transferred");
  const got = [];
  b.addEventListener("serial", (e) => got.push(e.detail.bytes));
  worker.onmessage({ data: { type: "serial", bytes: new Uint8Array([0x01, 0x23]) } });
  assert.deepEqual(got, [new Uint8Array([0x01, 0x23])]);
});

test("the Tauri app gets base64, and its events' base64 becomes bytes", async () => {
  const invoked = [];
  let listener = null;
  const tauri = {
    core: { invoke: async (_, { msg }) => (invoked.push(msg), null) },
    event: { listen: async (_, f) => { listener = f; } },
  };
  const b = new TauriBackend(tauri);
  await b.serialOpen();
  await b.serialWrite(new Uint8Array([1, 0x41, 0xff]));
  await b.serialClose();
  assert.deepEqual(
    invoked.map(({ session, seq, ...m }) => m),
    [
      { v: 1, cmd: "serialOpen" },
      { v: 1, cmd: "serialWrite", bytes: "AUH/" },
      { v: 1, cmd: "serialClose" },
    ],
  );
  const got = [];
  b.addEventListener("serial", (e) => got.push(e.detail));
  b.addEventListener("status", (e) => got.push(e.detail));
  listener({ payload: { type: "serial", bytes: "AUH/" } });
  listener({ payload: { type: "status", busy: false } });
  assert.deepEqual(got, [
    { type: "serial", bytes: new Uint8Array([1, 0x41, 0xff]) },
    { type: "status", busy: false },
  ]);
  // Bytes already bytes, and other events, pass as they are.
  const raw = { type: "serial", bytes: new Uint8Array([2]) };
  assert.equal(fromJson(raw), raw);
  assert.equal(fromJson(null), null);
});
