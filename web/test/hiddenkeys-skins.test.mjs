// Every model's table of unprinted key functions and ON combinations
// (web/hiddenkeys.js, web/onkeys.js) against the model's real skin, from
// the wasm package through worker.js (web/pkg, `just web`): each entry's
// key exists, an unprinted function sits in an empty slot, a note on a
// printed label, a menu on an empty slot with the application printed
// under the other shift; a one-shift model uses its one shift ("left");
// every combination's keys exist, ON first. `node --test web/test/`
// (just web-test).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { HIDDEN_KEYS, ghostSlot, hiddenOf } from "../hiddenkeys.js";
import { ON_COMBOS } from "../onkeys.js";

const pkg = await import("../pkg/saturnus_web.js");
pkg.initSync({ module: readFileSync(new URL("../pkg/saturnus_web_bg.wasm", import.meta.url)) });

const posted = [];
globalThis.self = { postMessage: (m) => posted.push(m) };
await import("../worker.js");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let nextId = 1;

/** The skin JSON of `model`, as the page gets it. */
async function skin(model) {
  const id = nextId++;
  self.onmessage({ data: { v: 1, id, cmd: "skin", model } });
  for (let i = 0; i < 2000; i++) {
    const r = posted.find((m) => m.type === "reply" && m.id === id);
    if (r) {
      assert.ok(r.ok, `skin ${model}: ${r.error}`);
      return r.result;
    }
    await sleep(1);
  }
  assert.fail(`no skin for ${model}`);
}

const MODELS = pkg.model_names();

test("every model has a table of ON combinations; the tables name only known models", () => {
  for (const m of Object.keys(HIDDEN_KEYS)) assert.ok(MODELS.includes(m), m);
  assert.deepEqual(Object.keys(ON_COMBOS).sort(), [...MODELS].sort());
});

for (const model of MODELS) {
  test(`${model}: the entries fit the skin's keys and labels`, async () => {
    const s = await skin(model);
    const keys = new Map(s.keys.map((k) => [k.name, { left: "", right: "", ...k }]));
    const oneShift = keys.has("shift");
    for (const e of HIDDEN_KEYS[model] ?? []) {
      const k = keys.get(e.key);
      const what = `${model} ${e.key} ${e.shift}${e.held ? " held" : ""}`;
      assert.ok(k, `${what}: a key of the skin`);
      if (oneShift) assert.equal(e.shift, "left", `${what}: a one-shift model's shift is "left"`);
      else assert.ok(keys.has(`${e.shift}shift`), `${what}: the skin has that shift`);
      if (e.held) {
        assert.equal(e.kind, "unprinted", `${what}: a held function is a tooltip line`);
        assert.ok(!e.ghost, `${what}: a held function has no ghost`);
        continue;
      }
      if (e.kind === "unprinted") assert.equal(k[e.shift], "", `${what}: not printed`);
      if (e.kind === "note") assert.notEqual(k[e.shift], "", `${what}: printed`);
      if (e.kind === "menu") {
        const other = e.shift === "left" ? "right" : "left";
        assert.equal(k[e.shift], "", `${what}: the menu's own slot is empty`);
        assert.equal(k[other], e.label, `${what}: the application printed under the other shift`);
      }
      if (e.ghost) assert.equal(ghostSlot(k, e.shift, hiddenOf(model, e.key)), e.ghost, `${what}: its ghost has the slot`);
    }
    for (const c of ON_COMBOS[model]) {
      assert.equal(c.keys[0], "on", `${model} ${c.id}: ON first`);
      for (const n of c.keys) assert.ok(keys.has(n), `${model} ${c.id}: ${n} is a key`);
    }
  });
}
