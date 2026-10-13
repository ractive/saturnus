// The saved states' words and checks (web/savedstates.js, iteration
// 34): the dates, the file name, the replace message, the questions, the
// header check (a broken file, another model) and the core's errors in
// plain words. `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { loadError, loadFileQuestion, loadQuestion, savedDate, savedMessage, stateFileName, stateRefusal } from "../savedstates.js";

const title = (m) => `HP ${m.toUpperCase()}`;
// Local times, as the page shows them.
const at = (y, mo, d, h, mi) => new Date(y, mo - 1, d, h, mi).getTime();

test("dates and the file name", () => {
  assert.equal(savedDate(at(2026, 10, 9, 16, 40)), "9 Oct 2026, 16:40");
  assert.equal(savedDate(at(2026, 10, 9, 14, 2), { now: at(2026, 12, 31, 0, 0), year: false }), "9 Oct, 14:02");
  assert.equal(savedDate(at(2025, 1, 3, 9, 5), { now: at(2026, 10, 9, 0, 0), year: false }), "3 Jan 2025, 09:05", "another year keeps it");
  assert.equal(stateFileName("48gx", at(2026, 10, 9, 16, 40)), "48gx-2026-10-09-1640.sat");
  assert.equal(stateFileName("49g", at(2027, 1, 2, 3, 4)), "49g-2027-01-02-0304.sat");
});

test("Save state says what it replaced", () => {
  const now = at(2026, 10, 9, 18, 0);
  assert.equal(savedMessage(null, now), "State saved.");
  assert.equal(savedMessage(at(2026, 10, 9, 14, 2), now), "State saved; it replaces the one from 9 Oct, 14:02.");
});

test("the questions before a load", () => {
  assert.deepEqual(loadQuestion(at(2026, 10, 9, 16, 40)), {
    title: "Load the state saved on 9 Oct 2026, 16:40?",
    body: "It replaces the calculator's memory, stack and screen.",
    action: "Load",
  });
  const q = loadFileQuestion("48gx-2026-10-09-1640.sat");
  assert.equal(q.title, "Load the state in 48gx-2026-10-09-1640.sat?");
  assert.equal(q.action, "Load");
});

/** A state's first bytes: the magic, version 2, the model's code, then a body. */
function header(code, magic = "SATURNUS") {
  const b = new Uint8Array(64);
  for (let i = 0; i < magic.length; i++) b[i] = magic.charCodeAt(i);
  b[8] = 2;
  b[10] = code;
  return b;
}

test("a broken file and another model are refused before the question", () => {
  const broken = "This file is not a saturnus state.";
  assert.equal(stateRefusal(header(1), "48gx", title), null);
  assert.equal(stateRefusal(header(2), "49g", title), null);
  assert.equal(stateRefusal(header(6), "42s", title), null);
  assert.equal(stateRefusal(header(0), "48gx", title), "This state is from an HP 48SX. Switch to the HP 48SX first, then load it.");
  assert.equal(stateRefusal(header(1, "SATURNUZ"), "48gx", title), broken);
  assert.equal(stateRefusal(header(7), "27s", title), null);
  assert.equal(stateRefusal(header(8), "17bii", title), null);
  assert.equal(stateRefusal(header(9), "17b", title), null);
  assert.equal(stateRefusal(header(8), "17b", title), "This state is from an HP 17BII. Switch to the HP 17BII first, then load it.");
  assert.equal(stateRefusal(header(10), "28s", title), null);
  assert.equal(stateRefusal(header(10), "48gx", title), "This state is from an HP 28S. Switch to the HP 28S first, then load it.");
  assert.equal(stateRefusal(header(11), "19bii", title), null);
  assert.equal(stateRefusal(header(11), "28s", title), "This state is from an HP 19BII. Switch to the HP 19BII first, then load it.");
  assert.equal(stateRefusal(header(12), "48gx", title), broken, "no such model");
  assert.equal(stateRefusal(new TextEncoder().encode("SATURNUS"), "48gx", title), broken, "too short");
  assert.equal(stateRefusal(new Uint8Array(0), "48gx", title), broken);
  assert.equal(stateRefusal(null, "48gx", title), broken, "too large to read");
  assert.equal(stateRefusal(new Uint8Array(4 * 1024 * 1024 + 1), "48gx", title), broken);
});

test("the core's refusals in plain words", () => {
  assert.equal(loadError(new Error("saved state was made with a different ROM"), "48gx", title), "This state was saved with another ROM of the HP 48GX; it loads only with that one.");
  assert.equal(loadError(new Error("invalid saved state at byte 12: unsupported state version"), "48gx", title), "This state is from another version of saturnus and cannot be loaded.");
  assert.equal(loadError(new Error("invalid saved state at byte 900: trailing bytes"), "48gx", title), "This file is damaged or not a saturnus state.");
  assert.equal(loadError(new Error("no ROM loaded"), "48gx", title), "Could not load the state: no ROM loaded");
});
