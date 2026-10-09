// A control that is off says why (web/disable.js). `node --test web/test/`
// (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { setOff } from "../disable.js";

test("an off control says why; on again, it has its own tooltip back", () => {
  const b = { dataset: {}, title: "ON and +: a darker display", disabled: false };
  setOff(b, true, "Start the calculator first");
  assert.deepEqual([b.disabled, b.title], [true, "Start the calculator first"]);
  setOff(b, true, "No saved state for this model");
  assert.equal(b.title, "No saved state for this model", "the reason follows the state");
  setOff(b, false, "Start the calculator first");
  assert.deepEqual([b.disabled, b.title], [false, "ON and +: a darker display"]);
  const plain = { dataset: {}, title: "", disabled: true };
  setOff(plain, false, "x");
  assert.equal(plain.title, "", "a control without a tooltip gets none when on");
});
