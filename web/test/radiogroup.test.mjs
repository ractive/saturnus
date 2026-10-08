// The Speed control's radio group pattern (web/radiogroup.js): one tab
// stop, arrows move with wrap-around, Home and End, other keys ignored.
import { test } from "node:test";
import assert from "node:assert/strict";
import { radioStep, radioTabIndexes } from "../radiogroup.js";

const SPEEDS = ["1", "2", "4", "max"];

test("arrows move the selection and wrap around", () => {
  assert.equal(radioStep(SPEEDS, "1", "ArrowRight"), "2");
  assert.equal(radioStep(SPEEDS, "1", "ArrowDown"), "2");
  assert.equal(radioStep(SPEEDS, "max", "ArrowRight"), "1");
  assert.equal(radioStep(SPEEDS, "1", "ArrowLeft"), "max");
  assert.equal(radioStep(SPEEDS, "4", "ArrowUp"), "2");
});

test("Home and End go to the ends; other keys do nothing", () => {
  assert.equal(radioStep(SPEEDS, "4", "Home"), "1");
  assert.equal(radioStep(SPEEDS, "2", "End"), "max");
  assert.equal(radioStep(SPEEDS, "2", "Enter"), null);
  assert.equal(radioStep(SPEEDS, "2", "a"), null);
  assert.equal(radioStep([], "2", "ArrowRight"), null);
});

test("an unknown current value steps from the first radio", () => {
  assert.equal(radioStep(SPEEDS, "8", "ArrowRight"), "2");
  assert.equal(radioStep(SPEEDS, "8", "ArrowLeft"), "max");
});

test("one tab stop: the checked radio, else the first", () => {
  assert.deepEqual(radioTabIndexes(SPEEDS, "4"), [-1, -1, 0, -1]);
  assert.deepEqual(radioTabIndexes(SPEEDS, "8"), [0, -1, -1, -1]);
});
