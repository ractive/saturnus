// The ROM table's rows (web/romrows.js): one button per row and what its
// menu holds, per host and slot; which models one removal takes (the
// 39G and 40G share a file); the remove question's words.
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { removeQuestion, removedMessage, rowAction, sharedModels } from "../romrows.js";

const title = (m) => `HP ${m.toUpperCase()}`;
const d = { file: "x", page: "https://www.hpcalc.org/details/1" };
const ids = (a) => a.menu.map((i) => (i === "-" ? "-" : i.id));

test("a primary button and a ⋯ menu for the rest", () => {
  // The 42S: no download anywhere, and nothing else to do.
  assert.deepEqual(rowAction({ model: "42s", fileName: null, download: null }, true), { primary: { id: "choose", label: "Choose…" }, menu: [] });
  // An empty slot in the browser: Choose… alone (the row links to hpcalc.org).
  assert.deepEqual(rowAction({ model: "48gx", fileName: null, download: d }, false), { primary: { id: "choose", label: "Choose…" }, menu: [] });
  // In the app: Download… at once, a file from the ⋯.
  const add = rowAction({ model: "48gx", fileName: null, download: d }, true);
  assert.deepEqual(add.primary, { id: "download", label: "Download…" });
  assert.deepEqual(ids(add), ["choose"]);
  assert.equal(add.menu[0].text, "Choose a file…");
  // A filled slot: Change… at once; Download again… and, after a rule, Remove….
  const change = rowAction({ model: "48gx", fileName: "gxrom-r", download: d }, true);
  assert.deepEqual(change.primary, { id: "choose", label: "Change…" });
  assert.deepEqual(ids(change), ["download", "-", "remove"]);
  assert.equal(change.menu.at(-1).danger, true);
  // Remove… is never a bare button: the ⋯ stays for it alone.
  assert.deepEqual(ids(rowAction({ model: "48gx", fileName: "gxrom-r", download: d }, false)), ["remove"], "the browser downloads nothing itself");
  assert.deepEqual(ids(rowAction({ model: "42s", fileName: "hp42s.rom", download: null }, true)), ["remove"]);
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
