// The home page's ROM actions (web/romrows.js): a kept calculator's menu
// and a new one's buttons, per host and slot; which models one removal takes (the
// 39G and 40G share a file); the remove question's words.
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { keptMenu, newActions, removeQuestion, removeRefusal, removedMessage, sharedModels } from "../romrows.js";

const title = (m) => `HP ${m.toUpperCase()}`;
const d = { file: "x", page: "https://www.hpcalc.org/details/1" };
const ids = (menu) => menu.map((i) => (i === "-" ? "-" : i.id));

test("a kept calculator's ⋯ menu; a new one's buttons", () => {
  // Kept: Change ROM…, Download again… in the app where hpcalc.org has it, then Remove….
  assert.deepEqual(ids(keptMenu({ model: "48gx", fileName: "gxrom-r", download: d }, true)), ["choose", "download", "-", "remove"]);
  assert.deepEqual(ids(keptMenu({ model: "48gx", fileName: "gxrom-r", download: d }, false)), ["choose", "-", "remove"], "the browser downloads nothing itself");
  assert.deepEqual(ids(keptMenu({ model: "42s", fileName: "hp42s.rom", download: null }, true)), ["choose", "-", "remove"]);
  const menu = keptMenu({ model: "48gx", fileName: "gxrom-r", download: d }, true);
  assert.equal(menu[0].text, "Change ROM…");
  assert.equal(menu.at(-1).danger, true);
  // New: Choose… alone in the browser and for the 42S; Download… first in the app.
  assert.deepEqual(newActions({ model: "48gx", fileName: null, download: d }, false), [{ id: "choose", label: "Choose…" }]);
  assert.deepEqual(newActions({ model: "42s", fileName: null, download: null }, true), [{ id: "choose", label: "Choose…" }]);
  assert.deepEqual(newActions({ model: "48gx", fileName: null, download: d }, true), [{ id: "download", label: "Download…" }, { id: "choose", label: "Choose…" }]);
});

test("the 39G and 40G with the same file are removed together", () => {
  const slots = [
    { model: "48gx", fileName: "gxrom-r" },
    { model: "39g", fileName: "rom.39g" },
    { model: "40g", fileName: "rom.39g" },
  ];
  assert.deepEqual(sharedModels(slots, "39g"), ["39g", "40g"]);
  assert.deepEqual(sharedModels(slots, "40g"), ["39g", "40g"]);
  assert.deepEqual(sharedModels(slots, "48gx"), ["48gx"]);
  // Different files, or the other one empty: alone.
  assert.deepEqual(sharedModels([{ model: "39g", fileName: "a" }, { model: "40g", fileName: "b" }], "40g"), ["40g"]);
  assert.deepEqual(sharedModels([{ model: "39g", fileName: "a" }, { model: "40g", fileName: null }], "39g"), ["39g"]);
});

test("the question and the outcome say what goes and what stays", () => {
  assert.deepEqual(removeQuestion(["48gx"], title, false), { title: "Remove the HP 48GX ROM?", body: "It is deleted from this browser. The file on your computer stays.", action: "Remove" });
  assert.equal(removeQuestion(["49g"], title, false).body, "It is deleted from this browser, with its saved state, as that contains the ROM. The file on your computer stays.");
  assert.deepEqual(removeQuestion(["48gx"], title, true), { title: "Remove the HP 48GX ROM?", body: "It is taken off the list. The file stays.", action: "Remove" });
  assert.equal(removeQuestion(["39g", "40g"], title, false).title, "Remove the HP 39G and HP 40G ROM?");
  assert.equal(removeQuestion(["39g", "40g"], title, false).body, "They use the same file, so both go. It is deleted from this browser. The file on your computer stays.");
  assert.equal(removeQuestion(["39g", "40g"], title, true).body, "They use the same file, so both go. It is taken off the list. The file stays.");
  // The running model's ROM: the question says the calculator stops.
  assert.equal(removeQuestion(["48gx"], title, false, "48gx").body, "It is deleted from this browser and the calculator stops. The file on your computer stays.");
  assert.equal(removeQuestion(["48gx"], title, true, "48gx").body, "It is taken off the list and the calculator stops. The file stays.");
  assert.equal(removeQuestion(["49g"], title, false, "49g").body, "It is deleted from this browser, with its saved state, as that contains the ROM, and the calculator stops. The file on your computer stays.");
  assert.equal(removeQuestion(["48gx"], title, false, "48sx").body, "It is deleted from this browser. The file on your computer stays.", "another model runs");
  assert.equal(removedMessage(["49g"], title, false), "The HP 49G ROM is removed from this browser, with its saved state.");
  assert.equal(removedMessage(["48gx"], title, true), "The HP 48GX ROM is removed from the list.");
});

test("a refused removal says why in plain words", () => {
  assert.equal(removeRefusal(new Error("typing is in progress (releaseAll stops it)"), "HP 48GX", "its ROM"), "The HP 48GX is typing a send; remove its ROM when it's done.");
  assert.equal(removeRefusal(new Error("a transfer is in progress"), "HP 49G", "the ROMs"), "The HP 49G is in a transfer; remove the ROMs when it's done.");
  assert.equal(removeRefusal(new Error("disk full"), "HP 49G", "its ROM"), "Could not remove its ROM: disk full");
});
