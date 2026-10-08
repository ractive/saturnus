// Tests of web/editor.js, the palette's editor mode without the DOM:
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import {
  EditSession, History, classify, completions, digraph, format, fromDigraphs, highlight, indentAfter, lineIndent, matchBracket,
  openAt, reindent, saveEdit, pullEdit, targetTitle, tokenize, unclosed, wordAt,
} from "../editor.js";

const classes = (text, ctx) => classify(tokenize(text), ctx).filter((t) => t.cls).map((t) => `${t.cls}:${t.text}`);

test("tokens cover the text and keep strings, comments and tags whole", () => {
  const text = '« :T:5 "a « b" @ note @ X 2 ^ »';
  const toks = tokenize(text);
  assert.equal(toks.map((t) => t.text).join(""), text);
  assert.deepEqual(toks.filter((t) => t.type !== "space").map((t) => `${t.type}:${t.text}`), [
    "delim:«", "tag::T:", "word:5", 'string:"a « b"', "comment:@ note @", "word:X", "word:2", "word:^", "delim:»",
  ]);
  assert.equal(tokenize('"open').at(-1).open, true);
  assert.equal(tokenize("@ to the end\nX")[0].text, "@ to the end");
});

test("highlighting tells delimiters, numbers, strings, names, commands and structure words apart", () => {
  const ctx = { commands: new Set(["SIN", "DUP", "+", "→LIST"]), variables: new Set(["PRG"]) };
  assert.deepEqual(classes("« → X « IF X 0 > THEN X SIN ELSE PRG END » »", ctx), [
    "delim:«", "kw:→", "name:X", "delim:«", "kw:IF", "name:X", "num:0", "name:>", "kw:THEN", "name:X", "cmd:SIN",
    "kw:ELSE", "var:PRG", "kw:END", "delim:»", "delim:»",
  ]);
  assert.deepEqual(classes("1.5E-3 -2 # 1Fh 2_m/s^2 (1,2) →LIST", ctx), [
    "num:1.5E-3", "num:-2", "num:#", "num:1Fh", "num:2_m/s^2", "delim:(", "num:1", "delim:,", "num:2", "delim:)", "cmd:→LIST",
  ]);
  // Inside an algebraic, names and operators part.
  assert.deepEqual(classes("'X^2+SIN(Y)'", ctx), [
    "delim:'", "name:X", "op:^", "num:2", "op:+", "cmd:SIN", "delim:(", "name:Y", "delim:)", "delim:'",
  ]);
  const html = highlight('« "<b>" »', ctx, [0, 8]);
  assert.equal(html, '<span class="t-delim"><mark class="bm">«</mark></span> <span class="t-str">"&lt;b&gt;"</span> <span class="t-delim"><mark class="bm">»</mark></span>');
  assert.ok(highlight("1\n").endsWith("\n "), "a last newline keeps its line");
});

test("brackets match across nesting, not inside strings", () => {
  const text = '« { 1 "}" } [ 2 ] »';
  assert.deepEqual(matchBracket(text, 0), [0, text.length - 1]);
  assert.deepEqual(matchBracket(text, text.length), [0, text.length - 1]);
  assert.deepEqual(matchBracket(text, 2), [2, 10]);
  assert.equal(matchBracket(text, 6), null, "inside the string");
  assert.equal(matchBracket("« 1", 0), null);
});

test("what is open: delimiters, strings and structure words", () => {
  assert.deepEqual(openAt("« IF 1 THEN { 2").stack, ["«", "IF", "{"]);
  assert.deepEqual(openAt("« CASE 1 THEN 2 END 3").stack, ["«", "CASE"]);
  assert.equal(openAt('"abc').string, true);
  assert.equal(unclosed("« 1 2 +"), true);
  assert.equal(unclosed("'X+1"), true);
  assert.equal(unclosed('"a'), true);
  assert.equal(unclosed("« 1 2 + »"), false);
  assert.equal(unclosed("1 2 +"), false);
});

test("nested programs and structure words are indented", () => {
  const flat = [
    "«",
    "→ N",
    "«",
    "IF N 0 >",
    "THEN",
    "1 N FOR I",
    "I",
    "NEXT",
    "ELSE",
    "CASE",
    "N 0 == THEN \"zero\" END",
    "\"neg\"",
    "END",
    "END",
    "\"multi",
    "  line\"",
    "»",
    "»",
  ].join("\n");
  assert.equal(reindent(flat), [
    "«",
    "  → N",
    "  «",
    "    IF N 0 >",
    "    THEN",
    "      1 N FOR I",
    "        I",
    "      NEXT",
    "    ELSE",
    "      CASE",
    "        N 0 == THEN \"zero\" END",
    "        \"neg\"",
    "      END",
    "    END",
    "    \"multi",
    "  line\"",
    "  »",
    "»",
  ].join("\n"));
  // Indenting twice changes nothing; tokens never change.
  assert.equal(reindent(reindent(flat)), reindent(flat));
  const words = (t) => tokenize(t).filter((x) => x.type !== "space").map((x) => x.text);
  assert.deepEqual(words(reindent(flat)), words(flat));
  assert.equal(indentAfter("« IF X THEN"), "    ");
  assert.equal(indentAfter('« "a'), "");
  // A line closing its construct takes the construct's level.
  const closing = "«\n  IF 1\n  THEN 2\n    END";
  assert.equal(lineIndent(closing, closing.length), "  ");
  assert.equal(lineIndent("«\n  1\n  »", 9), "");
});

test("a program on one line is laid out by its structure", () => {
  const one = '« → N « IF N 0 > THEN 1 N FOR I I 2 ^ NEXT ELSE "a  b" END CASE N 1 == THEN 1 END 2 END { 1 2 } \'X+1\' » »';
  const out = format(one);
  assert.equal(out, [
    "«",
    "  → N",
    "  «",
    "    IF N 0 >",
    "    THEN",
    "      1 N FOR I",
    "        I 2 ^",
    "      NEXT",
    "    ELSE",
    '      "a  b"',
    "    END",
    "    CASE",
    "      N 1 == THEN",
    "        1",
    "      END",
    "      2",
    "    END",
    "    { 1 2 } 'X+1'",
    "  »",
    "»",
  ].join("\n"));
  assert.equal(format(out), out, "formatting twice changes nothing");
  const words = (t) => tokenize(t).filter((x) => x.type !== "space").map((x) => x.text);
  assert.deepEqual(words(out), words(one));
  assert.equal(format("1 2 +"), "1 2 +");
  assert.equal(format("« 1 @ note\n 2 »"), "«\n  1 @ note\n  2\n»");
  assert.equal(format("{ « 1 » 2 }"), "{ « 1 » 2 }", "a program in a list stays inline");
  assert.equal(format("« DO 1 UNTIL 1 END »"), "«\n  DO\n    1\n  UNTIL 1\n  END\n»");
});

test("digraphs and translation codes become the calculator's characters", () => {
  assert.deepEqual(digraph("<<", 2), { text: "«", cursor: 1 });
  assert.deepEqual(digraph("« 1 >> X", 6), { text: "« 1 » X", cursor: 5 });
  assert.deepEqual(digraph("1 \\->", 5), { text: "1 →", cursor: 3 });
  assert.deepEqual(digraph("1 ->", 4), { text: "1 →", cursor: 3 });
  assert.deepEqual(digraph("\\GS", 3), { text: "Σ", cursor: 1 });
  assert.equal(digraph('"a->', 4), null, "not in a string");
  assert.equal(digraph("@ a->", 5), null, "not in a comment");
  assert.equal(digraph("a-", 2), null);
  assert.equal(fromDigraphs('<< \\->STR "a->b" >>'), '« →STR "a->b" »');
});

test("completion of commands and variables at the cursor", () => {
  const index = {
    commands: [
      { name: "SIN", upper: "SIN", codes: "SIN", friendly: null, stack: "x → sin(x)" },
      { name: "SINH", upper: "SINH", codes: "SINH", friendly: null, stack: "x → sinh(x)" },
      { name: "→LIST", upper: "→LIST", codes: "\\->LIST", friendly: "->LIST", stack: "obj… n → {list}" },
    ],
  };
  const vars = [{ name: "SINE" }];
  assert.deepEqual(completions(index, vars, "SI").map((c) => c.name), ["SIN", "SINE", "SINH"]);
  assert.deepEqual(completions(index, vars, "->L").map((c) => c.name), ["→LIST"]);
  assert.deepEqual(completions(index, vars, "\\->").map((c) => c.name), ["→LIST"]);
  assert.deepEqual(completions(index, vars, "S"), [], "one letter is not enough");
  assert.deepEqual(completions(index, vars, "12"), []);
  assert.deepEqual(completions(index, [], "SINH"), [], "an exact name alone");
  assert.deepEqual(wordAt("« 1 SI", 6), { start: 4, end: 6, word: "SI" });
  assert.deepEqual(wordAt("'X+SI", 5), { start: 3, end: 5, word: "SI" });
});

test("the history keeps sent text, newest first, once", () => {
  const mem = new Map();
  const storage = { getItem: (k) => mem.get(k) ?? null, setItem: (k, v) => mem.set(k, v) };
  const h = new History(storage, { max: 3 });
  for (const t of ["1", "2", "3", "2", "4", "  "]) h.push(t);
  assert.deepEqual(h.list(), ["4", "2", "3"]);
  assert.equal(h.move(-1, "draft"), "4");
  assert.equal(h.move(-1, "4"), "2");
  assert.equal(h.move(-1, "2"), "3");
  assert.equal(h.move(-1, "3"), null, "the oldest");
  assert.equal(h.move(1, "3"), "2");
  assert.equal(h.move(1, "2"), "4");
  assert.equal(h.move(1, "4"), "draft");
  assert.equal(h.move(1, "draft"), null);
  // Blocked storage: no history, no error.
  const blocked = new History({ getItem() { throw new Error("no"); }, setItem() { throw new Error("no"); } });
  blocked.push("x");
  assert.deepEqual(blocked.list(), []);
});

test("sessions are dirty when changed, clean after a save", () => {
  const s = new EditSession({ kind: "variable", dir: ["HOME"], name: "P" }, "« 1 »");
  assert.equal(s.dirty, false);
  s.text = "« 2 »";
  assert.equal(s.dirty, true);
  s.savedAs("« 2 »");
  assert.equal(s.dirty, false);
  assert.equal(s.was, "« 2 »");
  assert.equal(new EditSession(null, "x").dirty, false, "free text is never dirty");
  assert.equal(targetTitle({ kind: "variable", dir: ["HOME", "D"], name: "P" }), "P in HOME › D");
  assert.equal(targetTitle({ kind: "level", level: 2 }), "Stack level 2");
});

test("one transport: replace for the command line, storeText for objects", async () => {
  const calls = [];
  const backend = {
    replace: async (text) => { calls.push(["replace", text]); return { commandLine: { active: true, text } }; },
    storeText: async (a) => { calls.push(["storeText", a]); return a.text.includes(")") ? { error: "Invalid Syntax" } : { emulatedMs: 1 }; },
    editText: async (a) => { calls.push(["editText", a]); return { text: "« 1 »" }; },
  };
  assert.deepEqual(await saveEdit(backend, { kind: "cmdline" }, "« 2 »"), { ok: true, commandLine: { active: true, text: "« 2 »" } });
  assert.deepEqual(await saveEdit(backend, { kind: "variable", dir: ["HOME"], name: "P" }, "« 2 »", "« 1 »"), { ok: true });
  assert.deepEqual(await saveEdit(backend, { kind: "level", level: 1 }, "« ) »", "1"), { ok: false, error: "Invalid Syntax", calculator: true });
  assert.equal(await pullEdit(backend, { kind: "level", level: 3 }), "« 1 »");
  assert.deepEqual(calls, [
    ["replace", "« 2 »"],
    ["storeText", { dir: ["HOME"], name: "P", text: "« 2 »", was: "« 1 »" }],
    ["storeText", { level: 1, text: "« ) »", was: "1" }],
    ["editText", { level: 3 }],
  ]);
  const failing = { storeText: async () => { throw new Error("P changed on the calculator since it was opened: open it again"); } };
  assert.deepEqual(await saveEdit(failing, { kind: "variable", dir: ["HOME"], name: "P" }, "1", "2"),
    { ok: false, error: "P changed on the calculator since it was opened: open it again", calculator: false });
});
