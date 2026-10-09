// The calculator without a ROM for the model shown (web/norom.js): a model
// switch draws the new model and boots, resumes or pauses through the
// backend; a key press without a ROM pulses the empty state. `node --test
// web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { WRITABLE_MODELS, getRomLink, isLive, keyAction, noRomText, switchModel } from "../norom.js";

/** A backend that records its calls. */
function fakeBackend() {
  const calls = [];
  return { calls, pause: async (paused) => { calls.push(["pause", paused]); return null; } };
}

test("a model switch resumes, boots or pauses", async () => {
  const b = fakeBackend();
  // 48SX running, the 49G chosen with no ROM: the 48SX pauses.
  const running = { booted: "48sx", model: "49g", running: true };
  assert.equal(await switchModel(b, running, "49g", null), "no-rom");
  assert.equal(await switchModel(b, running, "49g", { state: "empty" }), "no-rom");
  assert.deepEqual(b.calls, [["pause", true], ["pause", true]]);
  // Back to the 48SX: it resumes rather than boots again.
  assert.equal(await switchModel(b, { booted: "48sx", model: "48sx", running: false }, "48sx", null), "resumed");
  assert.deepEqual(b.calls.at(-1), ["pause", false]);
  // A remembered ROM boots (the caller sends bootModel).
  b.calls.length = 0;
  assert.equal(await switchModel(b, running, "49g", { state: "ready" }), "boot");
  // Nothing running: nothing to pause.
  assert.equal(await switchModel(b, { booted: null, model: "42s", running: false }, "42s", null), "no-rom");
  assert.deepEqual(b.calls, []);
});

test("without a ROM a key pulses the empty state", () => {
  const keys = new Set(["on", "enter"]);
  assert.equal(keyAction({ booted: null, model: "48sx" }, "on", keys), "pulse");
  assert.equal(keyAction({ booted: "48sx", model: "49g" }, "on", keys), "pulse", "another model runs");
  assert.equal(keyAction({ booted: "48sx", model: "48sx" }, "on", keys), "press");
  assert.equal(keyAction({ booted: "48sx", model: "48sx" }, "sigmaplus", keys), "ignore");
  assert.equal(keyAction({ booted: null, model: "48sx" }, null, keys), "ignore");
  assert.equal(isLive({ booted: null, model: null }), false);
});

test("the empty state names the model; the 42S needs a dump", () => {
  assert.equal(noRomText("49g", "HP 49G"), "No ROM for the HP 49G.");
  assert.match(noRomText("42s", "HP 42S"), /dump the ROM from your own calculator/);
});

test("the page links an empty slot to hpcalc.org with the file to expect; the app downloads", () => {
  const download = { file: "sxrom-j", page: "https://www.hpcalc.org/details/4371" };
  const l = getRomLink(download, "file");
  assert.equal(`${l.before}${l.link}${l.after}`, "Get sxrom-j from hpcalc.org, unzip it and drop the file here.");
  assert.equal(l.href, download.page);
  assert.equal(getRomLink(download, "dialog"), null);
  assert.equal(getRomLink(null, "file"), null, "no download for the 42S");
});

test("the memory view takes files on the models with a Kermit server", () => {
  assert.deepEqual([...WRITABLE_MODELS].sort(), ["48gx", "48sx", "49g"]);
  assert.equal(WRITABLE_MODELS.has("38g"), false);
  assert.equal(WRITABLE_MODELS.has(null), false, "no calculator running");
});
