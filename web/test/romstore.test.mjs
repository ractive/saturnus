// Tests of web/romstore.js with a fake store and fake identification:
// `node --test web/test/` (just web-test). The assignment rules themselves
// are the wasm core's (crates/saturnus-web/src/romid.rs, tested there);
// here a stand-in assigns each file to its first model.

import assert from "node:assert/strict";
import { test } from "node:test";
import { RomStore } from "../romstore.js";

const MODELS = ["48sx", "48gx", "38g", "49g", "39g", "40g", "42s"];

/** "sha" is the bytes' first value; byte 1 names the model, 0 none. */
const identify = (bytes) => {
  const model = MODELS[bytes[1] - 1];
  return { sha256: `sha${bytes[0]}`, kind: model ? "exact" : "unknown", models: model ? [model] : [], revision: model ? `${model} test` : null };
};
const plan = ({ selected, files }) => {
  const assign = files.flatMap((f, file) => f.id.models.slice(0, 1).map((model) => ({ model, file })));
  const boot = assign.find((a) => a.model === selected)?.model ?? assign[0]?.model ?? null;
  return { assign, offer: [], boot, notice: assign.length ? "" : "not a ROM" };
};

/** An in-memory store like indexedDbStore; `fail` makes every call throw. */
function memoryStore() {
  const st = { slots: new Map(), images: new Map(), settings: null, fail: null };
  const check = () => {
    if (st.fail) throw st.fail;
  };
  st.api = {
    async load() {
      check();
      return { settings: st.settings, slots: Object.fromEntries(st.slots) };
    },
    async image(sha) {
      check();
      return st.images.get(sha);
    },
    async put(model, rec, bytes) {
      check();
      st.slots.set(model, rec);
      st.images.set(rec.sha256, bytes);
    },
    async settings(s) {
      check();
      st.settings = s;
    },
    async remove(model) {
      check();
      if (model === null) {
        st.slots.clear();
        st.images.clear();
        if (st.settings) st.settings = { ...st.settings, lastModel: null };
      } else st.slots.delete(model);
    },
  };
  return st;
}

function rig(store = memoryStore()) {
  const boots = [];
  const roms = new RomStore({
    identify,
    plan,
    models: MODELS,
    store: store.api,
    boot: (model, bytes, name) => {
      boots.push(`${model}:${name}:${bytes.length}`);
      return { model, romName: name };
    },
  });
  return { roms, boots, store };
}

const rom = (sha, modelIndex) => new Uint8Array([sha, modelIndex, 0, 0]);

test("a chosen ROM is kept, and boots by model after a reload", async () => {
  const a = rig();
  const r = await a.roms.chooseRom("48sx", [{ name: "sxrom-j", rom: rom(1, 1) }]);
  assert.deepEqual(r.booted, { model: "48sx", romName: "sxrom-j" });
  assert.equal(r.remembered, true);
  assert.equal(r.slots[0].state, "ready");
  assert.equal(r.lastModel, "48sx");
  // A new page: a new RomStore over the same store.
  const b = rig(a.store);
  const s = await b.roms.slots();
  assert.equal(s.lastModel, "48sx");
  assert.deepEqual(s.slots[0], { model: "48sx", fileName: "sxrom-j", revision: "48sx test", state: "ready" });
  const booted = await b.roms.bootModel("48sx");
  assert.equal(booted.booted.model, "48sx");
  assert.deepEqual(b.boots, ["48sx:sxrom-j:4"]);
  await assert.rejects(b.roms.bootModel("48gx"), /no ROM is kept for the 48GX/);
});

test("several files at once go to their models; an unknown one to none", async () => {
  const { roms, boots } = rig();
  const r = await roms.chooseRom("48sx", [
    { name: "gx", rom: rom(2, 2) },
    { name: "sx", rom: rom(1, 1) },
    { name: "notes", rom: rom(3, 0) },
  ]);
  assert.deepEqual(r.slots.filter((s) => s.fileName).map((s) => `${s.model}:${s.fileName}`), ["48sx:sx", "48gx:gx"]);
  assert.deepEqual(boots, ["48sx:sx:4"]);
});

test("a stored image that changed is reported, and nothing boots", async () => {
  const a = rig();
  await a.roms.chooseRom("48sx", [{ name: "sx", rom: rom(1, 1) }]);
  a.store.images.set("sha1", rom(9, 1));
  const b = rig(a.store);
  await assert.rejects(b.roms.bootModel("48sx"), /has changed/);
  assert.equal((await b.roms.slots()).slots[0].state, "changed");
  assert.deepEqual(b.boots, []);
});

test("forget removes every ROM and keeps the setting", async () => {
  const { roms, store } = rig();
  await roms.chooseRom("48sx", [{ name: "sx", rom: rom(1, 1) }]);
  await roms.settings(false);
  const r = await roms.forget(null);
  assert.equal(store.slots.size, 0);
  assert.equal(store.images.size, 0);
  assert.equal(r.bootLast, false);
  assert.equal(r.lastModel, null);
  assert.ok(r.slots.every((s) => s.state === "empty"));
});

test("a refused store degrades to this page only, with a note", async () => {
  const store = memoryStore();
  store.fail = Object.assign(new Error("blocked"), { name: "SecurityError" });
  const { roms, boots } = rig(store);
  const s = await roms.slots();
  assert.equal(s.remembered, false);
  assert.match(s.note, /does not keep ROMs.*SecurityError/);
  // The ROM still boots, and boots again by model while the page lives.
  await roms.chooseRom("48sx", [{ name: "sx", rom: rom(1, 1) }]);
  await roms.bootModel("48sx");
  assert.equal(boots.length, 2);
  await roms.forget(null);
});

test("a quota error on the first put keeps the slot for this page", async () => {
  const store = memoryStore();
  const { roms } = rig(store);
  await roms.slots();
  store.fail = Object.assign(new Error("full"), { name: "QuotaExceededError" });
  const r = await roms.chooseRom("48sx", [{ name: "sx", rom: rom(1, 1) }]);
  assert.equal(r.remembered, false);
  assert.match(r.note, /QuotaExceededError/);
  assert.equal(r.slots[0].state, "ready");
  assert.equal(r.booted.model, "48sx");
});
