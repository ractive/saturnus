// Tests of web/romstore.js with a fake store and fake identification:
// `node --test web/test/` (just web-test). The assignment rules themselves
// are the wasm core's (crates/saturnus-host/src/romid.rs, tested there);
// here a stand-in assigns each file to its first model.

import assert from "node:assert/strict";
import { test } from "node:test";
import { RomStore } from "../romstore.js";

const MODELS = ["48sx", "48gx", "38g", "49g", "39g", "40g", "42s"];

/** "sha" is the bytes' first value; byte 1 names the model, 0 none. */
const identify = (bytes) => {
  if (bytes[1] === 9) return { sha256: `sha${bytes[0]}`, kind: "fits", models: ["48gx", "38g"], revision: null };
  const model = MODELS[bytes[1] - 1];
  return { sha256: `sha${bytes[0]}`, kind: model ? "exact" : "unknown", models: model ? [model] : [], revision: model ? `${model} test` : null };
};
const plan = ({ selected, files }) => {
  // A file of byte 1 = 9 could be the 48GX or the 38G: offered.
  const offer = files.flatMap((f, file) => (f.id.kind === "fits" ? [{ models: f.id.models, file }] : []));
  const assign = files.flatMap((f, file) => (f.id.kind === "exact" ? [{ model: f.id.models[0], file }] : []));
  const boot = assign.find((a) => a.model === selected)?.model ?? assign[0]?.model ?? null;
  return { assign, offer, boot, notice: offer.length ? "offered" : assign.length ? "" : "not a ROM" };
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

function rig(store = memoryStore(), failBoot = false) {
  const boots = [];
  const roms = new RomStore({
    identify,
    plan,
    download: (model) => (model === "42s" ? null : { file: `${model}.rom` }),
    models: MODELS,
    store: store.api,
    boot: (model, bytes, name) => {
      if (failBoot) throw new Error("the machine refused it");
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
  assert.deepEqual(s.slots[0], { model: "48sx", fileName: "sxrom-j", revision: "48sx test", state: "ready", download: { file: "48sx.rom" } });
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

test("an offer has a number for its id, and taking it boots", async () => {
  const { roms, boots } = rig();
  const r = await roms.chooseRom("48sx", [{ name: "x.rom", rom: rom(5, 9) }]);
  assert.equal(r.booted, null);
  assert.equal(r.notice, "offered");
  assert.equal(r.offers.length, 1);
  const o = r.offers[0];
  assert.equal(typeof o.id, "number");
  assert.deepEqual([o.models, o.fileName], [["48gx", "38g"], "x.rom"]);
  const t = await roms.chooseRom("38g", undefined, o.id);
  assert.equal(t.booted.model, "38g");
  assert.deepEqual(boots, ["38g:x.rom:4"]);
  assert.equal(t.slots.find((x) => x.model === "38g").fileName, "x.rom");
  // Still open for the 48GX, under the same number.
  assert.deepEqual(t.offers.map((x) => [x.id, x.models]), [[o.id, ["48gx"]]]);
  await assert.rejects(roms.chooseRom("38g", undefined, o.id), /no longer open/);
});

test("a failing boot after a choice keeps the files and tells the error with the notice", async () => {
  const { roms } = rig(memoryStore(), true);
  const r = await roms.chooseRom("48sx", [{ name: "sx", rom: rom(1, 1) }, { name: "x", rom: rom(5, 9) }]);
  assert.equal(r.booted, null);
  assert.equal(r.bootError, "the machine refused it");
  assert.equal(r.notice, "offered");
  assert.equal(r.slots[0].fileName, "sx");
  const t = await roms.chooseRom("48gx", undefined, r.offers[0].id);
  assert.equal(t.bootError, "the machine refused it");
  assert.equal(t.slots[1].fileName, "x");
  // `bootModel` changed nothing: its failure is the command's error.
  await assert.rejects(roms.bootModel("48sx"), /refused/);
});

test("selecting a model asks for the file again only when it is missing or changed", async () => {
  globalThis.HTMLElement ??= class {};
  globalThis.customElements ??= { define() {} };
  const { SatControls } = await import("../components/sat-controls.js");
  for (const [state, asks] of [["missing", true], ["changed", true], ["ready", false]]) {
    const asked = [];
    const slots = { slots: [{ model: "48gx", fileName: "gxrom-r", revision: null, state: "ready" }], offers: [] };
    const after = { ...slots, slots: [{ ...slots.slots[0], state }] };
    const fake = Object.assign(Object.create(SatControls.prototype), {
      backend: {
        romSource: "dialog",
        bootModel: async () => { throw new Error("cannot start"); },
        romSlots: async () => after,
      },
      store: { state: { roms: slots, romNotice: "" }, set(p) { Object.assign(this.state, p); } },
      prefs: { set() {} },
      chooseFor: async (m) => asked.push(m),
    });
    await fake.bootSelected("48gx", true);
    assert.deepEqual(asked, asks ? ["48gx"] : [], state);
    assert.equal(fake.store.state.messageError, true);
  }
});

test("each slot tells where its model's ROM is offered; the 42S has none", async () => {
  const s = await rig().roms.slots();
  assert.deepEqual(s.slots.find((x) => x.model === "39g").download, { file: "39g.rom" });
  assert.equal(s.slots.find((x) => x.model === "42s").download, null);
});
