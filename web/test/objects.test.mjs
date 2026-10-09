// Tests of web/objects.js: `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import {
  checksumText, directoryAt, findVariables, flagRows, indentRpl,
  previewOf, sizeText, summary, textOf, tokens,
} from "../objects.js";

// The host supplies the calculator's text on every object (`text`); these
// objects carry it as a host would send it.
const real = (value, text = String(value)) => ({ type: "real", value, text });
const withText = (obj, text) => ({ ...obj, text });

test("every text is the host's: nothing is formatted here", () => {
  // Texts a formatter in the page would get wrong: the 49G's trailing
  // point, a whole-number unit, a small real, a name quoted in an array.
  assert.equal(textOf(real(1, "1.")), "1.");
  assert.equal(textOf(withText({ type: "unit", value: 2, unit: "m" }, "2_m")), "2_m");
  assert.equal(textOf(real(1.23456789012e-5, "1.23456789012E-5")), "1.23456789012E-5");
  const array = withText({ type: "array", dims: [1], items: [withText({ type: "name", value: "A" }, "'A'")] }, "[ 'A' ]");
  const p = previewOf(array);
  assert.deepEqual([p.kind, p.copy, p.rows], ["matrix", "[ 'A' ]", [["'A'"]]]);
  // Without a text there is none, whatever the fields say.
  assert.equal(textOf({ type: "real", value: 1 }), null);
  assert.equal(textOf({ type: "unit", value: 2, unit: "m" }), null);
  assert.equal(previewOf({ type: "real", value: 1 }).kind, "unavailable");
});

test("an object without text has none, and a list says where", () => {
  const list = { type: "list", items: [real(1), { type: "program" }, { type: "command", address: 12345 }] };
  assert.equal(textOf(list), null);
  assert.deepEqual(summary(list), { text: "{ 1 « … » ‹command› }", complete: false });
  const p = previewOf(list);
  assert.equal(p.kind, "list");
  assert.equal(p.copy, null);
  assert.deepEqual(p.items.map((i) => i.complete), [true, false, false]);
  const prog = previewOf({ type: "program" });
  assert.deepEqual([prog.kind, prog.copy], ["unavailable", null]);
  assert.match(prog.reason, /has no text form/);
});

test("a program is shown indented and copies as the host's text", () => {
  const source = "« → a b « IF a b > THEN a ELSE b END » »";
  const p = previewOf({ type: "program", source, text: source });
  assert.equal(p.kind, "program");
  assert.equal(p.copy, source);
  assert.deepEqual(p.lines, [
    { depth: 0, text: "«" },
    { depth: 1, text: "→ a b" },
    { depth: 1, text: "«" },
    { depth: 2, text: "IF a b >" },
    { depth: 2, text: "THEN a" },
    { depth: 2, text: "ELSE b" },
    { depth: 2, text: "END" },
    { depth: 1, text: "»" },
    { depth: 0, text: "»" },
  ]);
  const a = previewOf({ type: "algebraic", source: "'X^2+1'", text: "'X^2+1'" });
  assert.deepEqual([a.kind, a.copy, a.lines], ["program", "'X^2+1'", [{ depth: 0, text: "'X^2+1'" }]]);
});

test("a list is laid out by element with the elements' own texts", () => {
  const list = {
    type: "list",
    text: "{ 1 SIN 2_m :x: (1,-2) }",
    items: [
      real(1),
      { type: "command", name: "SIN", text: "SIN" },
      { type: "unit", value: 2, unit: "m", text: "2_m" },
      { type: "tagged", tag: "x", text: ":x: (1,-2)", object: { type: "complex", re: 1, im: -2, text: "(1,-2)" } },
    ],
  };
  const p = previewOf(list);
  assert.equal(p.copy, "{ 1 SIN 2_m :x: (1,-2) }");
  assert.deepEqual(p.items.map((i) => [i.text, i.type]), [
    ["1", "Real number"], ["SIN", "Command"], ["2_m", "Unit object"], [":x: (1,-2)", "Tagged object"],
  ]);
  assert.deepEqual(summary(list), { text: "{ 1 SIN 2_m :x: (1,-2) }", complete: true });
  assert.equal(previewOf({ type: "string", value: "a→b", text: "\"a→b\"" }).copy, "\"a→b\"");
});

test("program layout: loops, strings and nesting", () => {
  assert.deepEqual(tokens('« "a « b" \'X+1\' DUP»'), ["«", '"a « b"', "'X+1'", "DUP", "»"]);
  const lines = indentRpl('« 1 10 FOR i i SQ NEXT "done" »').map((l) => "  ".repeat(l.depth) + l.text);
  assert.deepEqual(lines, ["«", "  1 10", "  FOR i i SQ", "  NEXT", '  "done"', "»"]);
  const w = indentRpl("« WHILE DUP 1 > REPEAT 2 / END »").map((l) => "  ".repeat(l.depth) + l.text);
  assert.deepEqual(w, ["«", "  WHILE DUP 1 >", "  REPEAT 2 /", "  END", "»"]);
  // A long run of words wraps; nothing is lost.
  const long = `« ${Array.from({ length: 60 }, (_, i) => `W${i}`).join(" ")} »`;
  const out = indentRpl(long);
  assert.ok(out.length > 3);
  assert.equal(out.map((l) => l.text).join(" "), long);
});

test("large objects are cut with a count", () => {
  const items = Array.from({ length: 1000 }, (_, i) => real(i));
  const p = previewOf({ type: "list", items, text: "{ … }" });
  assert.deepEqual([p.items.length, p.more, p.count], [200, 800, 1000]);
  const s = previewOf({ type: "string", value: "x".repeat(10000), text: "\"…\"" });
  assert.deepEqual([s.text.length, s.more, s.length], [4000, 6000, 10000]);
  const row = Array.from({ length: 20 }, (_, i) => real(i));
  const m = previewOf({ type: "array", dims: [30, 20], items: Array.from({ length: 30 }, () => row), text: "[[ 0 1 … ]]" });
  assert.deepEqual([m.rows.length, m.rows[0].length, m.moreRows, m.moreCols], [24, 8, 6, 12]);
  assert.deepEqual(m.rows[0].slice(0, 3), ["0", "1", "2"]);
});

test("an unknown object offers its nibbles, not as text", () => {
  const p = previewOf({ type: "unknown", prolog: "02B1E", kind: "Graphic", nibbles: 30, hex: "E1B20" });
  assert.deepEqual([p.kind, p.copy, p.hex, p.title], ["unavailable", null, "E1B20", "Graphic"]);
});

test("a graphic with a picture is previewed as one, its nibbles kept", () => {
  const graphic = { width: 1, height: 1, rows: "80" };
  const p = previewOf({ type: "unknown", prolog: "02B1E", kind: "Graphic", nibbles: 22, hex: "E1B20", graphic });
  assert.deepEqual([p.kind, p.copy, p.graphic, p.hex, p.nibbles, p.title], ["graphic", null, graphic, "E1B20", 22, "Graphic"]);
});

test("sizes, checksums, directories and search", () => {
  assert.equal(sizeText(12.5), "12.5 bytes");
  assert.equal(sizeText(1), "1 byte");
  assert.equal(checksumText(0x3a5f), "# 3A5Fh");
  const tree = [
    { name: "A", type: "Directory", variables: [{ name: "Xa", type: "Real Number" }, { name: "B", type: "Directory", variables: [] }] },
    { name: "xa2", type: "String" },
  ];
  assert.equal(directoryAt(tree, ["HOME"]), tree);
  assert.equal(directoryAt(tree, ["HOME", "A"]).length, 2);
  assert.deepEqual(directoryAt(tree, ["HOME", "A", "B"]), []);
  assert.equal(directoryAt(tree, ["HOME", "Z"]), null);
  assert.equal(directoryAt(tree, ["HOME", "xa2"]), null);
  assert.deepEqual(findVariables(tree, "XA").map((f) => [...f.path, f.variable.name].join("/")), ["HOME/A/Xa", "HOME/xa2"]);
});

test("flag rows follow the flags read from the calculator", () => {
  const entry = {
    system: [
      { first: -1, last: -1, topic: "Math", name: "Principal solution", clear: "general", set: "principal", status: "known" },
      { first: -4, last: -4, topic: "Other", name: "Not used", status: "unused" },
      { first: -5, last: -10, topic: "Binary integers", name: "Word size", field: "six flags", status: "known" },
      { first: -95, last: -95, topic: "Keyboard and entry", name: "Algebraic or RPN", clear: "polarity not stated", status: "unknown" },
      { first: -96, last: -97, topic: "Other", name: "Not documented", status: "unknown" },
    ],
  };
  const r = flagRows(entry, ["Math", "Binary integers", "Keyboard and entry", "Other"], { set: [-1, -5, -10, -97, 3] });
  assert.deepEqual(r.topics.map((t) => t.topic), ["Math", "Binary integers", "Keyboard and entry"]);
  const [math, bin, kbd] = r.topics.map((t) => t.rows[0]);
  assert.deepEqual([math.label, math.set, math.now, math.other], ["-1", true, "principal", "general"]);
  assert.deepEqual([bin.label, bin.set, bin.bits], ["-5 … -10", null, [true, false, false, false, false, true]]);
  assert.deepEqual([kbd.set, kbd.now, kbd.status], [false, "polarity not stated", "unknown"]);
  assert.deepEqual(r.undocumented, [
    { flag: -4, set: false, status: "unused" },
    { flag: -96, set: false, status: "unknown" },
    { flag: -97, set: true, status: "unknown" },
  ]);
});
