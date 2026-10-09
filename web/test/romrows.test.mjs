// The ROM table's rows (web/romrows.js): one button per row and what its
// menu holds, per host and slot; which models one removal takes (the
// 39G and 40G share a file); the remove question's words.
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { removeQuestion, removedMessage, rowAction, sharedModels } from "../romrows.js";

const title = (m) => `HP ${m.toUpperCase()}`;
const d = { file: "x", page: "https://www.hpcalc.org/details/1" };
const ids = (a) => (a.items ?? []).map((i) => (i === "-" ? "-" : i.id));

test("one button per row: Choose…, Add or Change with its menu", () => {
  // The 42S: no download anywhere.
  assert.deepEqual(rowAction({ model: "42s", fileName: null, download: null }, true), { kind: "choose", label: "Choose…" });
  // An empty slot in the browser: Choose… (the row links to hpcalc.org).
  assert.equal(rowAction({ model: "48gx", fileName: null, download: d }, false).kind, "choose");
  // In the app: Add, with the download first.
  const add = rowAction({ model: "48gx", fileName: null, download: d }, true);
  assert.equal(add.label, "Add");
  assert.deepEqual(ids(add), ["download", "choose"]);
  assert.equal(add.items[0].text, "Download from hpcalc.org…");
  // A filled slot: Change, and Remove after a rule.
  const change = rowAction({ model: "48gx", fileName: "gxrom-r", download: d }, true);
  assert.equal(change.label, "Change");
  assert.deepEqual(ids(change), ["choose", "download", "-", "remove"]);
  assert.equal(change.items.at(-1).danger, true);
  assert.deepEqual(ids(rowAction({ model: "48gx", fileName: "gxrom-r", download: d }, false)), ["choose", "-", "remove"], "the browser downloads nothing itself");
  assert.deepEqual(ids(rowAction({ model: "42s", fileName: "hp42s.rom", download: null }, true)), ["choose", "-", "remove"]);
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
  assert.equal(removeQuestion(["48gx"], title, false), "Remove the HP 48GX ROM from this browser? The file on your computer stays.");
  assert.equal(removeQuestion(["49g"], title, false), "Remove the HP 49G ROM from this browser? Its saved state goes too, as it contains the ROM. The file on your computer stays.");
  assert.equal(removeQuestion(["48gx"], title, true), "Remove the HP 48GX ROM from the list? The file stays.");
  assert.equal(removeQuestion(["39g", "40g"], title, false), "Remove the HP 39G and HP 40G ROM from this browser? They use the same file, so both go. The file on your computer stays.");
  assert.equal(removeQuestion(["39g", "40g"], title, true), "Remove the HP 39G and HP 40G ROM from the list? They use the same file, so both go. The file stays.");
  assert.equal(removedMessage(["49g"], title, false), "The HP 49G ROM is removed from this browser, with its saved state.");
  assert.equal(removedMessage(["48gx"], title, true), "The HP 48GX ROM is removed from the list.");
});
