// The menu keys' tooltip lines (web/menukeys.js): commands with the
// reference's full name, submenus, variables, blanks, and the key's own
// lines kept under them; the full names from the real web/commands.json.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { MENU_KEYS, longNames, menuKeyIndex, menuKeyLine, menuKeyTitle, wantsReference } from "../menukeys.js";
import { buildIndex } from "../reference.js";

const data = JSON.parse(readFileSync(new URL("../commands.json", import.meta.url), "utf8"));

test("a command names itself and its full name", () => {
  const long = longNames(buildIndex(data, "48gx"));
  assert.equal(menuKeyLine({ label: "EXPAN", command: "EXPAN" }, long), "EXPAN · Expand products");
  assert.equal(menuKeyLine({ label: "ABS", command: "ABS" }, long), "ABS · Absolute value");
  // A label other than the command's name says which it runs.
  assert.equal(menuKeyLine({ label: "T", command: "TAN" }, (c) => (c === "TAN" ? "Tangent" : null)), "T: TAN · Tangent");
  // Not in the reference: the name alone.
  assert.equal(menuKeyLine({ label: "XYZZY", command: "XYZZY" }, long), "XYZZY");
  assert.equal(menuKeyLine({ label: "EXPAN", command: "EXPAN" }), "EXPAN");
});

test("submenus, variables, labels and blanks", () => {
  assert.equal(menuKeyLine({ label: "VECTR", submenu: true }), "VECTR menu");
  assert.equal(menuKeyLine({ label: "A7", type: "Real Number" }), "A7 · variable (real number)");
  assert.equal(menuKeyLine({ label: "SUBD", type: "Directory" }), "SUBD · directory");
  assert.equal(menuKeyLine({ label: "NEW" }), "NEW");
  assert.equal(menuKeyLine(null), null);
  assert.equal(menuKeyLine({ label: "" }), null);
});

test("the title puts the line over the key's own lines, for the menu keys only", () => {
  const keys = [{ label: "VECTR", submenu: true }, null, { label: "EXPAN", command: "EXPAN" }, null, null, null];
  const base = "A (alpha A)\n⌃-click: …";
  assert.equal(menuKeyTitle("48gx", "a", base, keys), `VECTR menu\n${base}`);
  assert.equal(menuKeyTitle("48gx", "b", "B (alpha B)", keys), "B (alpha B)", "a blank key keeps its own");
  assert.equal(menuKeyTitle("49g", "c", "F3 (alpha C)", keys, () => "Expand products"), "EXPAN · Expand products\nF3 (alpha C)");
  assert.equal(menuKeyTitle("48gx", "a", base, null), base, "keys not known");
  assert.equal(menuKeyTitle("48gx", "mth", "MTH", keys), "MTH", "not a menu key");
  assert.equal(menuKeyTitle("42s", "a", "Σ+", keys), "Σ+", "a model whose menu keys are not read");
  assert.equal(menuKeyIndex("48sx", "f"), 5);
  assert.equal(menuKeyIndex("28s", "a"), -1);
  assert.deepEqual(Object.keys(MENU_KEYS), ["48sx", "48gx", "49g"]);
});

test("the reference is wanted only for a menu of commands", () => {
  assert.equal(wantsReference([{ label: "VECTR", submenu: true }, null]), false);
  assert.equal(wantsReference([{ label: "A1", type: "Real Number" }]), false);
  assert.equal(wantsReference([null, { label: "ABS", command: "ABS" }]), true);
  assert.equal(wantsReference(null), false);
});

test("the 48SX's and 49G's full names come from their own entries", () => {
  for (const model of ["48sx", "49g"]) {
    const long = longNames(buildIndex(data, model));
    assert.equal(menuKeyLine({ label: "EXPAN", command: "EXPAN" }, long), "EXPAN · Expand products", model);
  }
});
