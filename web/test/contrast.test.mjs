// The LCD contrast (web/contrast.js): each model's power-on contrast draws
// properly dark, the darkness rises with the register over the ON+/ON-
// range, and "Darker display" / "Lighter display" send ON + / ON -.
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { contrastDarkness, offTint, stepContrast } from "../contrast.js";
import { Store } from "../store.js";

/** `Model::contrast_range` and `Model::default_contrast`, per model. */
const MODELS = {
  "48sx": [[3, 19], 11],
  "48gx": [[9, 24], 14],
  "38g": [[9, 24], 14],
  "49g": [[9, 24], 14],
  "39g": [[9, 24], 12],
  "40g": [[9, 24], 12],
  "42s": [[15, 31], 22],
};

test("the power-on contrast is dark and every step shows", () => {
  for (const [model, [range, def]] of Object.entries(MODELS)) {
    const at = (c) => contrastDarkness(c, range, def);
    assert.ok(at(def) >= 0.85, `${model}: ${at(def)} at its default`);
    for (let c = range[0]; c < range[1]; c++) {
      assert.ok(at(c + 1) > at(c), `${model}: not rising from ${c}`);
    }
    // ON- and ON+ from the default change what is drawn.
    assert.ok(at(def) - at(def - 1) > 0.02, `${model}: ON- barely shows`);
    assert.ok(offTint(at(def + 1)) - offTint(at(def)) > 0.01, `${model}: ON+ barely shows`);
    assert.ok(at(range[0]) < 0.3, `${model}: the low end fades`);
    assert.equal(at(range[1]), 1);
  }
  // A host that sends no default: the middle of the range.
  assert.ok(Math.abs(contrastDarkness(17, [9, 24]) - 0.9) < 1e-9);
});

/** A backend that records key commands and moves the machine's keys. */
function fakeMachine(store) {
  const calls = [];
  const down = new Set();
  const show = () => setTimeout(() => store.set({ keysDown: [...down] }), 5);
  return {
    calls,
    keyDown(key) {
      calls.push(["keyDown", key]);
      down.add(key);
      show();
    },
    keyUp(key) {
      calls.push(["keyUp", key]);
      down.delete(key);
      show();
    },
  };
}

test("Darker and Lighter display send the ON chord", async () => {
  for (const [darker, key] of [[true, "plus"], [false, "minus"]]) {
    const store = new Store();
    const backend = fakeMachine(store);
    await stepContrast(backend, store, darker);
    assert.deepEqual(backend.calls, [
      ["keyDown", "on"],
      ["keyDown", key],
      ["keyUp", key],
      ["keyUp", "on"],
    ]);
  }
});

test("ON comes up only after the machine let go of +", async () => {
  const store = new Store();
  const backend = fakeMachine(store);
  const seen = [];
  const keyUp = backend.keyUp;
  backend.keyUp = (key) => {
    seen.push([key, [...store.state.keysDown]]);
    keyUp(key);
  };
  await stepContrast(backend, store, true);
  assert.deepEqual(seen, [
    ["plus", ["on", "plus"]],
    ["on", ["on"]],
  ]);
});
