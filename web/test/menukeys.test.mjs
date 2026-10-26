// The menu keys' tooltip lines (web/menukeys.js): commands with the
// reference's full name, submenus, variables, blanks, and the key's own
// lines kept under them; the 42S's and the Pioneers' labels named from the
// key reference, the 28S's from its own entries; the full names from the
// real web/commands.json.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { KEYED, MENU_KEYS, keyedLine, labelIndex, longNames, lookupLine, menuKeyCommand, menuKeyIndex, menuKeyLine, menuKeyTitle, sameLabel, wantsReference } from "../menukeys.js";
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
  assert.equal(menuKeyTitle("38g", "a", "A", keys), "A", "a model whose menu keys are not read");
  assert.equal(menuKeyIndex("48sx", "f"), 5);
  assert.equal(menuKeyIndex("42s", "xeq"), 5);
  assert.equal(menuKeyIndex("42s", "a"), -1);
  assert.equal(menuKeyIndex("38g", "a"), -1);
  assert.deepEqual(Object.keys(MENU_KEYS), ["48sx", "48gx", "49g", "28s", "42s", "27s", "17bii", "17b", "19bii"]);
});

test("the reference is wanted only for a menu of commands", () => {
  assert.equal(wantsReference([{ label: "VECTR", submenu: true }, null]), false);
  assert.equal(wantsReference([{ label: "A1", type: "Real Number" }]), false);
  assert.equal(wantsReference([null, { label: "ABS", command: "ABS" }]), true);
  assert.equal(wantsReference(null), false);
  // A key-driven model's labels are named from the key reference.
  assert.equal(wantsReference([{ label: "N" }], "17bii"), true);
  assert.equal(wantsReference([{ label: "N" }], "48gx"), false);
  assert.equal(wantsReference([null], "42s"), false);
});

const row = (...labels) => labels.map((l) => (l ? { label: l } : null));
const lines = (model, labels) => {
  const li = labelIndex(buildIndex(data, model));
  const keys = row(...labels);
  return keys.map((_, i) => keyedLine(keys, i, li));
};

test("the 42S's labels name their functions from the key reference", () => {
  assert.deepEqual(lines("42s", ["→DEG", "→RAD", "→HR", "→HMS", "→REC", "→POL"]), [
    "→DEG · To degrees",
    "→RAD · To radians",
    "→HR · To hours",
    "→HMS · To hours-minutes-seconds",
    "→REC · To rectangular",
    "→POL · To polar",
  ]);
  // TRANS is cut to TRAN in the box and in the reference; the BASE menu's
  // DECM holds no mark; LOGIC opens a menu; X≠0? from its code.
  const matrix = lines("42s", ["NEW", "INV", "DET", "TRANS", "SIMQ", "EDIT"]);
  assert.equal(matrix[3], "TRANS · Transpose");
  const base = lines("42s", ["A...F", "HEXM", "DECM", "OCTM", "BINM", "LOGIC"]);
  assert.match(base[2], /^DECM · Decimal mode/);
  assert.equal(base[5], "LOGIC menu");
  assert.match(lines("42s", ["X=0?", "X≠0?", "X<0?", "X>0?", "X≤0?", "X≥0?"])[1], /^X≠0\? · Tests whether x ≠ 0/);
  // A program's label in the XEQ menu: not in the reference.
  assert.deepEqual(lines("42s", ["S", null, null, null, null, null]), [null, null, null, null, null, null]);
  // Not the printed functions of the top row: √x is no menu label.
  assert.equal(lines("42s", ["√x", null, null, null, null, null])[0], null);
});

test("the Pioneers' labels, a label of several menus told apart by its row", () => {
  assert.deepEqual(lines("17bii", ["N", "I%YR", "PV", "PMT", "FV", "OTHER"]).slice(0, 2), ["N · Number of periods", "I%YR · Interest per year"]);
  assert.equal(lines("17bii", ["FIN", "BUS", "SUM", "TIME", "SOLVE", null])[0], "FIN · Finance");
  // CALC in CFLO and in SUM: each list's own NAME.
  assert.match(lines("17bii", ["CALC", "INSR", "DELET", "NAME", "GET", "#T?"])[3], /cash-flow list/);
  assert.match(lines("17bii", ["CALC", "INSR", "DELET", "NAME", "GET", "TOTAL"])[3], /SUM list/);
  // Σ from the business models' code.
  assert.equal(lines("17b", ["ΣX", "ΣY", "ΣX2", "ΣY2", "ΣXY", "MORE"])[0], "ΣX · Sum of x");
  // The buffer's spaces and the 19BII's degrees and TEXT menus.
  assert.equal(lines("27s", ["X", "Y", "C X,Y", "P X,Y", "N!", "RAN#"])[2], "C X,Y · Combinations");
  assert.equal(lines("19bii", ["°F", "°C", "°R", "°K", null, null])[1], "°C · Degrees Celsius");
  assert.equal(lines("19bii", ["FIND", "EDIT", "PREV", "NEXT", "SORT", "MORE"])[2], "PREV · Previous record");
  // The Solver's variables A and B are not the forecast's B.
  assert.deepEqual(lines("17bii", ["A", "B", null, null, null, null]).slice(0, 2), [null, null]);
  // Another language: the key keeps its own tooltip.
  assert.deepEqual(lines("17bii", ["FIN", "COM", "SOMME", "TEMPS", "SOLVE", null]).slice(1, 4), [null, null, null]);
  assert.equal(sameLabel("RANGE", "RANG"), true);
  assert.equal(sameLabel("RA", "RANGE"), false);
  assert.ok([..."42s 27s 17bii 17b 19bii".split(" ")].every((m) => KEYED.has(m)));
});

test("the title of a key-driven model's menu key", () => {
  const index = buildIndex(data, "17bii");
  const keys = row("N", "I%YR", "PV", "PMT", "FV", "OTHER");
  assert.equal(menuKeyTitle("17bii", "a", "A", keys, longNames(index), index), "N · Number of periods\nA");
  assert.equal(menuKeyTitle("17bii", "a", "A", row("ZINS"), longNames(index), index), "A", "not in the reference");
  assert.equal(menuKeyTitle("17bii", "a", "A", keys, longNames(null), null), "A", "no reference loaded");
});

test("the 28S's keys name their commands from its own entries", () => {
  const long = longNames(buildIndex(data, "28s"));
  assert.equal(menuKeyTitle("28s", "a", "A", [{ label: "DUP", command: "DUP" }], long), "DUP · Duplicate\nA");
  assert.match(menuKeyTitle("28s", "b", "B", [null, { label: "UNTI", command: "UNTIL" }], long), /^UNTI: UNTIL/);
  assert.equal(menuKeyTitle("28s", "a", "A", [{ label: "CMD" }], long), "CMD\nA", "a mode, no command");
});

test("the 48SX's and 49G's full names come from their own entries", () => {
  for (const model of ["48sx", "49g"]) {
    const long = longNames(buildIndex(data, model));
    assert.equal(menuKeyLine({ label: "EXPAN", command: "EXPAN" }, long), "EXPAN · Expand products", model);
  }
});

test("a key that runs a command says how to look it up, with the palette's key as bound", () => {
  const index48 = buildIndex(data, "48gx");
  const keys = [{ label: "VECTR", submenu: true }, { label: "EXPAN", command: "EXPAN" }, { label: "A7", type: "Real Number" }, null, null, null];
  assert.equal(lookupLine("EXPAN", "⌘K"), "⌘K: look up EXPAN");
  assert.equal(lookupLine("EXPAN", ""), null, "unbound");
  assert.equal(menuKeyTitle("48gx", "b", "B", keys, longNames(index48), index48, "⌘K"), "EXPAN · Expand products\n⌘K: look up EXPAN\nB");
  assert.equal(menuKeyTitle("48gx", "b", "B", keys, longNames(index48), index48, "Ctrl+K"), "EXPAN · Expand products\nCtrl+K: look up EXPAN\nB");
  assert.equal(menuKeyTitle("48gx", "b", "B", keys, longNames(index48), index48, ""), "EXPAN · Expand products\nB", "unbound: no line");
  // Submenus and variables have nothing to look up.
  assert.equal(menuKeyTitle("48gx", "a", "A", keys, longNames(index48), index48, "⌘K"), "VECTR menu\nA");
  assert.equal(menuKeyTitle("48gx", "c", "C", keys, longNames(index48), index48, "⌘K"), "A7 · variable (real number)\nC");
  assert.equal(menuKeyCommand("48gx", "b", keys), "EXPAN");
  assert.equal(menuKeyCommand("48gx", "a", keys), null);
  // The key-driven models: the reference's name of the function, none for a submenu.
  const index = buildIndex(data, "17bii");
  const tvm = row("N", "I%YR", "PV", "PMT", "FV", "OTHER");
  assert.equal(menuKeyTitle("17bii", "a", "A", tvm, longNames(index), index, "⌘K"), "N · Number of periods\n⌘K: look up N\nA");
  const main = row("FIN", "BUS", "SUM", "TIME", "SOLVE", null);
  assert.equal(menuKeyCommand("17bii", "a", main, index), "FIN");
  const cflo = row("CALC", "INSR", "DELET", "NAME", "GET", "#T?");
  assert.equal(menuKeyCommand("17bii", "a", cflo, index), "CALC (CFLO)");
  // The 28S: the command, not the label cut short.
  assert.equal(menuKeyCommand("28s", "b", [null, { label: "UNTI", command: "UNTIL" }]), "UNTIL");
  assert.equal(menuKeyCommand("28s", "a", [{ label: "CMD" }]), null);
});
