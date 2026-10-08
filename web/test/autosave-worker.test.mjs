// The Worker's side of auto-save (iteration 27), against a fake of the
// wasm bindings' `Host` and a fake IndexedDB: `node --test web/test/`
// (just web-test). When and whether to save is the state machine's,
// tested in Rust (crates/saturnus-host/src/protocol/autosave.rs and
// tests.rs, the ROM-gated crates/saturnus-host/tests/autosave.rs); here:
// the state it hands out goes into the model's auto slot (never the
// user's), the page hears `autoSaved` and never sees the bytes; a boot
// gets the kept state, a fresh one deletes it; after Forget ROMs the
// 49G's is not written again.

import assert from "node:assert/strict";
import { register } from "node:module";
import { test } from "node:test";

const fake = `
export const log = [];
export const state = { out: [] };
export default async function init() {}
export const model_names = () => ["48sx", "49g"];
export const identify_rom = (bytes) => ({ sha256: "s" + bytes.length });
export const plan_roms = (input) => {
  const { selected } = JSON.parse(input);
  return { assign: [{ model: selected, file: 0 }], offer: [], boot: selected, notice: "" };
};
export const rom_download = () => null;
export class Host {
  constructor() {}
  command(json, bytes, tag) {
    const m = JSON.parse(json);
    log.push(["command", m]);
    if (tag !== undefined) state.out.push({ type: "reply", tag, ok: true, result: null });
  }
  check() {}
  boot(model, rom, name, kept) {
    log.push(["boot", model, kept ? [...kept] : kept]);
    return kept && kept[0] === 0xEE ? { model, romName: name, restoreError: "not a saturnus state" } : { model, romName: name, ...(kept ? { restored: true } : {}) };
  }
  timer() {}
  deadline() { return undefined; }
  drain() { const o = state.out; state.out = []; return o; }
}
`;
const hooks = `
export async function resolve(spec, ctx, next) {
  if (spec.endsWith("/pkg/saturnus_web.js")) {
    return { url: "data:text/javascript," + encodeURIComponent(${JSON.stringify(fake)}), shortCircuit: true };
  }
  return next(spec, ctx);
}`;
register("data:text/javascript," + encodeURIComponent(hooks));

// A fake IndexedDB: every database one Map per store.
const dbs = new Map();
const later = (f) => setTimeout(f, 0);
globalThis.indexedDB = {
  open(name) {
    const req = {};
    later(() => {
      const isNew = !dbs.has(name);
      if (isNew) dbs.set(name, new Map());
      const stores = dbs.get(name);
      const db = {
        createObjectStore(s) { stores.set(s, new Map()); },
        close() {},
        transaction(names) {
          const tx = {};
          let open = 0;
          const done = (value) => {
            const r = { result: value };
            open++;
            later(() => {
              r.onsuccess?.();
              if (--open === 0) later(() => tx.oncomplete?.());
            });
            return r;
          };
          tx.objectStore = (s) => {
            const m = stores.get(s);
            return {
              get: (k) => done(m.get(k)),
              getAll: () => done([...m.values()]),
              getAllKeys: () => done([...m.keys()]),
              put: (v, k) => done(void m.set(k, v)),
              delete: (k) => done(void m.delete(k)),
              clear: () => done(void m.clear()),
            };
          };
          tx.abort = () => {};
          return tx;
        },
      };
      req.result = db;
      if (isNew) req.onupgradeneeded?.();
      req.onsuccess?.();
    });
    return req;
  },
};
const states = () => dbs.get("saturnus")?.get("states") ?? new Map();

const posted = [];
globalThis.self = { postMessage: (m) => posted.push(m) };
await import("../worker.js");
const pkg = await import("../pkg/saturnus_web.js");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let id = 0;

async function send(msg) {
  self.onmessage({ data: { v: 1, id: ++id, ...msg } });
  for (let i = 0; i < 20 && !posted.some((m) => m.id === id); i++) await sleep(5);
  return posted.find((m) => m.id === id);
}

/** The engine hands out a state with the next drain (a command's). */
async function engineSaves(model, state) {
  pkg.state.out.push({ type: "autoSave", model, romName: "r", cycles: 7, state: new Uint8Array(state) });
  await send({ cmd: "stats" });
  await sleep(30);
}

test("a state the engine hands out goes into the auto slot, the page hears autoSaved", async () => {
  states().clear?.();
  await engineSaves("48sx", [1, 2, 3]);
  assert.ok(!posted.some((m) => m.type === "autoSave"), "the bytes never reach the page");
  assert.deepEqual(posted.filter((m) => m.type === "autoSaved"), [{ type: "autoSaved", model: "48sx", cycles: 7 }]);
  const rec = states().get("auto:48sx");
  assert.deepEqual([...rec.state], [1, 2, 3]);
  assert.equal(rec.romName, "r");
  assert.equal(states().has("48sx"), false, "the user's slot is untouched");
});

test("a boot gets the kept state; a fresh one deletes it and boots cold", async () => {
  states().set("48sx", { state: new Uint8Array([9]) });
  const chose = await send({ cmd: "chooseRom", model: "48sx", files: [{ name: "rom", rom: new Uint8Array(4) }] });
  assert.equal(chose.ok, true, JSON.stringify(chose));
  assert.deepEqual(pkg.log.findLast((c) => c[0] === "boot"), ["boot", "48sx", [1, 2, 3]]);
  assert.equal(chose.result.booted.restored, true);
  const fresh = await send({ cmd: "bootModel", model: "48sx", fresh: true });
  assert.equal(fresh.ok, true, JSON.stringify(fresh));
  assert.deepEqual(pkg.log.findLast((c) => c[0] === "boot"), ["boot", "48sx", undefined]);
  assert.equal(states().has("auto:48sx"), false, "forgotten");
  assert.deepEqual([...states().get("48sx").state], [9], "the user's state stays");
  await send({ cmd: "bootModel", model: "48sx" });
  assert.deepEqual(pkg.log.findLast((c) => c[0] === "boot"), ["boot", "48sx", undefined], "nothing kept now");
});

test("a kept state that does not load boots cold, without an error", async () => {
  await engineSaves("48sx", [0xEE]);
  const r = await send({ cmd: "bootModel", model: "48sx" });
  assert.equal(r.ok, true);
  assert.equal(r.result.booted.restoreError, "not a saturnus state");
});

test("after Forget ROMs the 49G's state is not written again until it boots", async () => {
  await send({ cmd: "chooseRom", model: "49g", files: [{ name: "rom49", rom: new Uint8Array(8) }] });
  await send({ cmd: "forgetRom" });
  states().delete("auto:49g");
  await engineSaves("49g", [4]);
  assert.equal(states().has("auto:49g"), false, "holds the ROM");
  await engineSaves("48sx", [5]);
  assert.deepEqual([...states().get("auto:48sx").state], [5], "the others are kept");
});
