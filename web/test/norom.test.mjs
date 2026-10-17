// The calculator without a ROM for the model shown (web/norom.js): the
// empty state's words, a key press without a ROM pulses it, what dropped
// files that are no ROM are told. `node --test
// web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { MODEL_ORDER, WRITABLE_MODELS, dropNotice, emptyText, isLive, keyAction, noRomText, notCalculatorFiles, orderModels } from "../norom.js";

test("without a ROM a key pulses the empty state", () => {
  const keys = new Set(["on", "enter"]);
  assert.equal(keyAction({ booted: null, model: "48sx" }, "on", keys), "pulse");
  assert.equal(keyAction({ booted: "48sx", model: "49g" }, "on", keys), "pulse", "another model runs");
  assert.equal(keyAction({ booted: "48sx", model: "48sx" }, "on", keys), "press");
  assert.equal(keyAction({ booted: "48sx", model: "48sx" }, "sigmaplus", keys), "ignore");
  assert.equal(keyAction({ booted: null, model: "48sx" }, null, keys), "ignore");
  assert.equal(isLive({ booted: null, model: null }), false);
});

test("the empty state: none asked for, not running, or no ROM; the 42S needs a dump", () => {
  assert.equal(emptyText("48sx", "HP 48SX"), "No calculator is running.", "the route names no model");
  assert.equal(emptyText("48sx", "HP 48SX", { asked: true, kept: true }), "The HP 48SX is not running.");
  assert.equal(emptyText("49g", "HP 49G", { asked: true }), "No ROM for the HP 49G.");
  assert.equal(noRomText("49g", "HP 49G"), "No ROM for the HP 49G.");
  assert.match(noRomText("42s", "HP 42S"), /read the ROM out of your own calculator/);
  assert.match(noRomText("19bii", "HP 19BII"), /no download, and saturnus has no way to get this ROM for you/);
  assert.doesNotMatch(noRomText("19bii", "HP 19BII"), /own calculator/);
  for (const m of ["27s", "17bii", "17b", "28s"]) {
    assert.match(noRomText(m, m), /HP never published it: read the ROM out of your own calculator/);
  }
});

test("dropped files: the home page's notice, Calculator's refusal", () => {
  const r = { slots: [], offers: [] };
  assert.equal(dropNotice(["test.txt"], r, "file"), "test.txt is not a ROM.");
  assert.equal(dropNotice(["a.txt", "b.png", "c.pdf"], r, "file"), "a.txt, b.png and c.pdf are not ROMs.");
  assert.equal(dropNotice(["sx"], { ...r, start: "48sx" }, "file"), null, "a ROM was kept");
  assert.equal(dropNotice(["gx"], { ...r, offers: [{ id: 1 }] }, "file"), null, "a ROM was offered");
  assert.equal(dropNotice(["x"], null, "file"), null, "its error is shown already");
  assert.equal(dropNotice(["sx"], null, "dialog"), "sx was not used. In the app, choose ROMs with Choose….");
  assert.equal(notCalculatorFiles(["sxrom-j"]), "sxrom-j is not a calculator file. ROMs are chosen on the home page.");
  assert.equal(notCalculatorFiles(["a", "b"]), "a and b are not calculator files. ROMs are chosen on the home page.");
});

test("the memory view takes files on the models with a Kermit server", () => {
  assert.deepEqual([...WRITABLE_MODELS].sort(), ["48gx", "48sx", "49g"]);
  assert.equal(WRITABLE_MODELS.has("38g"), false);
  assert.equal(WRITABLE_MODELS.has(null), false, "no calculator running");
});

test("models are listed 48SX, 48GX, 49G, 38G, 39G, 40G, 42S, 27S, 17BII, 17B, 28S, 19BII; unknown ones last", () => {
  const host = ["48sx", "48gx", "38g", "49g", "39g", "40g", "42s", "27s", "17bii", "17b", "28s", "19bii"];
  assert.deepEqual(orderModels(host), MODEL_ORDER);
  assert.deepEqual(orderModels(["x", "42s", "48sx", "y"]), ["48sx", "42s", "x", "y"]);
  const slots = host.map((model) => ({ model }));
  assert.deepEqual(orderModels(slots, (s) => s.model).map((s) => s.model), MODEL_ORDER);
});
