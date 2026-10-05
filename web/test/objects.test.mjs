// Tests of web/objects.js: `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import {
  checksumText, directoryAt, findVariables, flagRows, formatReal, indentRpl,
  previewOf, sizeText, summary, textOf, tokens,
} from "../objects.js";

test("reals are written as the calculator writes them", () => {
  assert.equal(formatReal(1), "1");
  assert.equal(formatReal(1, true), "1.");
  assert.equal(formatReal(0.5), ".5");
  assert.equal(formatReal(-0.25), "-.25");
  assert.equal(formatReal(3.14159265359), "3.14159265359");
  assert.equal(formatReal(1e25), "1.E25");
  assert.equal(formatReal(1.5e-13), "1.5E-13");
  assert.equal(formatReal("1.5E-400"), "1.5E-400");
  assert.equal(formatReal(0, true), "0.");
});

test("text forms of data objects", () => {
  assert.equal(textOf({ type: "string", value: "a→b" }), '"a→b"');
  assert.equal(textOf({ type: "name", value: "ΣDAT" }), "'ΣDAT'");
  assert.equal(textOf({ type: "complex", re: 1, im: -2 }), "(1,-2)");
  assert.equal(textOf({ type: "binary", value: 255, base: "hex", text: "# FFh" }), "# FFh");
  assert.equal(textOf({ type: "integer", value: "-1234567890123456789" }), "-1234567890123456789");
  assert.equal(
    textOf({ type: "list", items: [{ type: "real", value: 1 }, { type: "name", value: "A" }, { type: "list", items: [] }] }),
    "{ 1 A { } }",
  );
  assert.equal(textOf({ type: "tagged", tag: "x", object: { type: "real", value: 2 } }), ":x: 2");
  assert.equal(textOf({ type: "unit", value: 3, unit: "m/s^2" }), "3_m/s^2");
  assert.equal(
    textOf({ type: "array", dims: [2, 2], items: [[{ type: "real", value: 1 }, { type: "real", value: 2 }], [{ type: "real", value: 3 }, { type: "real", value: 4 }]] }),
    "[ [ 1 2 ] [ 3 4 ] ]",
  );
  assert.equal(textOf({ type: "array", dims: [3], items: [1, 2, 3].map((value) => ({ type: "real", value })) }), "[ 1 2 3 ]");
});

test("an object without text has no text form, and a list says where", () => {
  assert.equal(textOf({ type: "program" }), null);
  assert.equal(textOf({ type: "algebraic" }), null);
  assert.equal(textOf({ type: "unit", value: 3 }), null);
  assert.equal(textOf({ type: "command" }), null);
  const list = { type: "list", items: [{ type: "real", value: 1 }, { type: "program" }, { type: "command" }] };
  assert.equal(textOf(list), null);
  assert.deepEqual(summary(list), { text: "{ 1 « … » ‹command› }", complete: false });
  const p = previewOf(list);
  assert.equal(p.kind, "list");
  assert.equal(p.copy, null);
  assert.deepEqual(p.items.map((i) => i.complete), [true, false, false]);
});

test("a program without source is unavailable, with a plain reason", () => {
  const p = previewOf({ type: "program" });
  assert.equal(p.kind, "unavailable");
  assert.equal(p.copy, null);
  assert.match(p.reason, /could not be turned back into text/);
  assert.equal(previewOf({ type: "unit", value: 3 }).kind, "unavailable");
});

// The host's shapes: `source` on programs and algebraics, `unit` on
// units, `name` on commands (web/protocol.md, Typed objects).
test("a program with source is shown indented and copies as its source", () => {
  const source = "« → a b « IF a b > THEN a ELSE b END » »";
  const p = previewOf({ type: "program", source });
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
  const a = previewOf({ type: "algebraic", source: "'X^2+1'" });
  assert.deepEqual([a.kind, a.copy, a.lines], ["program", "'X^2+1'", [{ depth: 0, text: "'X^2+1'" }]]);
});

test("named commands and units complete a list", () => {
  const list = {
    type: "list",
    items: [{ type: "real", value: 1 }, { type: "command", name: "SIN" }, { type: "unit", value: 2, unit: "m" }, { type: "program", source: "« 1 »" }],
  };
  assert.equal(textOf(list), "{ 1 SIN 2_m « 1 » }");
  // An XLIB name of an unknown library is shown by its numbers; a ROM
  // object with neither name nor numbers has no text.
  assert.equal(textOf({ type: "command", library: 1234, command: 5 }), "XLIB 1234 5");
  assert.equal(textOf({ type: "command", address: 12345 }), null);
  assert.equal(previewOf(list).copy, "{ 1 SIN 2_m « 1 » }");
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
  const items = Array.from({ length: 1000 }, (_, i) => ({ type: "real", value: i }));
  const p = previewOf({ type: "list", items });
  assert.deepEqual([p.items.length, p.more, p.count], [200, 800, 1000]);
  const s = previewOf({ type: "string", value: "x".repeat(10000) });
  assert.deepEqual([s.text.length, s.more, s.length], [4000, 6000, 10000]);
  const row = Array.from({ length: 20 }, (_, i) => ({ type: "real", value: i }));
  const m = previewOf({ type: "array", dims: [30, 20], items: Array.from({ length: 30 }, () => row) });
  assert.deepEqual([m.rows.length, m.rows[0].length, m.moreRows, m.moreCols], [24, 8, 6, 12]);
  assert.ok(m.copy.startsWith("[ [ 0 1 2"));
});

test("an unknown object offers its nibbles, not as text", () => {
  const p = previewOf({ type: "unknown", prolog: "02B1E", kind: "Graphic", nibbles: 30, hex: "E1B20" });
  assert.deepEqual([p.kind, p.copy, p.hex, p.title], ["unavailable", null, "E1B20", "Graphic"]);
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
