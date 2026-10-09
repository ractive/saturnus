// Tests of web/editor.js, the palette's editor mode without the DOM:
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import {
  EditSession, History, afterSave, classify, completions, digraph, editButtonState, editFocusStep, editTarget, editorKey, format, fromDigraphs, highlight, indentAfter, lineIndent,
  matchBracket, openAt, reindent, saveEdit, saveSession, pullEdit, targetTitle, tokenize, unclosed, wordAt,
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
  const s = new EditSession({ kind: "variable", dir: ["HOME"], name: "P" }, "« 1 »", "10:ABCD");
  assert.equal(s.dirty, false);
  assert.equal(s.was, "10:ABCD");
  s.text = "« 2 »";
  assert.equal(s.dirty, true);
  s.savedAs("12:0001");
  assert.equal(s.dirty, false);
  assert.equal(s.was, "12:0001");
  assert.equal(new EditSession(null, "x").dirty, false, "free text is never dirty");
  assert.equal(new EditSession({ kind: "cmdline" }, "x", "1:0").was, null, "a command line has no identity");
  assert.equal(targetTitle({ kind: "variable", dir: ["HOME", "D"], name: "P" }), "P in HOME › D");
  assert.equal(targetTitle({ kind: "level", level: 2 }), "stack level 2");
  assert.equal(targetTitle({ kind: "cmdline" }), "the command line");
});

test("one transport: replace for the command line, storeText for objects", async () => {
  const calls = [];
  const backend = {
    replace: async (text) => { calls.push(["replace", text]); return { commandLine: { active: true, text } }; },
    storeText: async (a) => { calls.push(["storeText", a]); return a.text.includes(")") ? { error: "Invalid Syntax" } : { emulatedMs: 1 }; },
    editText: async (a) => { calls.push(["editText", a]); return { text: "« 1 »", was: "10:ABCD" }; },
  };
  assert.deepEqual(await saveEdit(backend, { kind: "cmdline" }, "« 2 »"), { ok: true, commandLine: { active: true, text: "« 2 »" } });
  assert.deepEqual(await saveEdit(backend, { kind: "variable", dir: ["HOME"], name: "P" }, "« 2 »", "10:ABCD"), { ok: true });
  assert.deepEqual(await saveEdit(backend, { kind: "level", level: 1 }, "« ) »", "5:0001"), { ok: false, error: "Invalid Syntax", calculator: true });
  assert.deepEqual(await pullEdit(backend, { kind: "level", level: 3 }), { text: "« 1 »", was: "10:ABCD" });
  // A string left open never leaves the page.
  assert.deepEqual(await saveEdit(backend, { kind: "variable", dir: ["HOME"], name: "P" }, '« "a »', "10:ABCD"),
    { ok: false, error: 'a string is not closed (a " is missing)', calculator: false });
  assert.deepEqual(calls, [
    ["replace", "« 2 »"],
    ["storeText", { dir: ["HOME"], name: "P", text: "« 2 »", was: "10:ABCD" }],
    ["storeText", { level: 1, text: "« ) »", was: "5:0001" }],
    ["editText", { level: 3 }],
  ]);
  const failing = { storeText: async () => { throw new Error("P changed on the calculator since it was opened: open it again"); } };
  assert.deepEqual(await saveEdit(failing, { kind: "variable", dir: ["HOME"], name: "P" }, "1", "2"),
    { ok: false, error: "P changed on the calculator since it was opened: open it again", calculator: false });
});

test("after a save the session checks against the new object, or asks for a reopen", async () => {
  const target = { kind: "variable", dir: ["HOME"], name: "P" };
  const sent = [];
  let reread = true;
  const backend = {
    storeText: async (a) => { sent.push(a.was); return { emulatedMs: 1 }; },
    editText: async () => { if (!reread) throw new Error("the memory is not set up"); return { text: "« 2 »", was: "12:0002" }; },
  };
  const s = new EditSession(target, "« 1 »", "10:0001");
  s.text = "« 2 »";
  assert.deepEqual(await saveSession(backend, s, "« 2 »"), { ok: true });
  assert.equal(s.dirty, false);
  assert.equal(s.was, "12:0002", "the next save checks the saved object");
  assert.equal(s.broken, null);
  // The read after a save fails: saved, but no further save until reopened.
  reread = false;
  const r = await saveSession(backend, s, "« 3 »");
  assert.equal(r.ok, true);
  assert.equal(r.reread, "the memory is not set up");
  assert.match(s.broken, /^Saved, but P in HOME could not be read back .*Open it again before saving again\./);
  assert.deepEqual(sent, ["10:0001", "12:0002"]);
  // A failed save changes nothing.
  const refused = { storeText: async () => ({ error: "Invalid Syntax" }), editText: backend.editText };
  const t = new EditSession(target, "« 1 »", "10:0001");
  t.text = "« ) »";
  assert.deepEqual(await saveSession(refused, t, "« ) »"), { ok: false, error: "Invalid Syntax", calculator: true });
  assert.equal(t.dirty, true);
  assert.equal(t.was, "10:0001");
});

test("Cmd/Ctrl+S is the Save button; a save that went through closes the editor, a refused one keeps it open with the text", async () => {
  const target = { kind: "variable", dir: ["HOME"], name: "P" };
  const key = (k, mods = {}) => ({ key: k, code: `Key${k.toUpperCase()}`, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...mods });
  // The shortcut does what the button does (`data-ed="primary"`), on a Mac and elsewhere.
  assert.equal(editorKey(key("s", { metaKey: true }), target), "primary");
  assert.equal(editorKey(key("s", { ctrlKey: true }), target), "primary");
  assert.equal(editorKey(key("Enter", { ctrlKey: true }), target), "primary");
  assert.equal(editorKey(key("s", { metaKey: true }), null), "none", "free text: taken, nothing saved");
  assert.equal(editorKey(key("Enter", { metaKey: true, shiftKey: true }), null), "secondary");
  assert.equal(editorKey(key("ArrowUp", { altKey: true }), target), "older");
  assert.equal(editorKey(key("f", { altKey: true, shiftKey: true }), target), "format");
  assert.equal(editorKey(key("s"), target), null, "a plain s is typed");

  let refuse = false;
  const backend = {
    storeText: async () => (refuse ? { error: "Invalid Syntax" } : { emulatedMs: 1 }),
    editText: async () => ({ text: "« 2 »", was: "12:0002" }),
    replace: async (text) => ({ commandLine: { active: true, text } }),
  };
  // Saved: the editor closes; the status line says what was saved.
  const s = new EditSession(target, "« 1 »", "10:0001");
  s.text = "« 2 »";
  assert.deepEqual(afterSave(target, await saveSession(backend, s, s.text), 420), { close: true, message: "Saved P in HOME in 0.42 s." });
  assert.equal(s.dirty, false, "nothing unsaved stops the close");
  const level = { kind: "level", level: 1 };
  assert.deepEqual(afterSave(level, await saveSession(backend, new EditSession(level, "1", "5:0001"), "2"), 50), { close: true, message: "Saved stack level 1 in 0.05 s." });
  const line = { kind: "cmdline" };
  assert.deepEqual(afterSave(line, await saveSession(backend, new EditSession(line, "1"), "2"), 5), { close: true, message: null });
  // Saved but not read back: still saved, still closes.
  assert.equal(afterSave(target, { ok: true, reread: "the memory is not set up" }, 10).close, true);

  // Refused by the calculator: open, its error shown, the text kept (unsaved).
  refuse = true;
  const t = new EditSession(target, "« 1 »", "10:0001");
  t.text = "« ) »";
  const r = afterSave(target, await saveSession(backend, t, t.text), 300);
  assert.deepEqual(r, { close: false, notice: { text: "The calculator says: Invalid Syntax. Nothing was changed.", error: true, calculator: true } });
  assert.equal(t.text, "« ) »");
  assert.equal(t.dirty, true);
  // Refused in the page or by the host (busy, an open string): open too.
  const u = new EditSession(target, "« 1 »", "10:0001");
  u.text = '« "a »';
  const open = afterSave(target, await saveSession(backend, u, u.text), 1);
  assert.equal(open.close, false);
  assert.match(open.notice.text, /^Not saved: a string is not closed/);
  assert.equal(u.text, '« "a »');
  const busy = { storeText: async () => { throw new Error("the calculator is busy"); } };
  u.text = "« 3 »";
  assert.deepEqual(afterSave(target, await saveSession(busy, u, u.text), 1),
    { close: false, notice: { text: "Not saved: the calculator is busy.", error: true, calculator: false } });
});

test("Cmd/Ctrl+E follows the keys: the view's selection there, else the command line, else stack level 1", () => {
  const prog = { type: "program", text: "« 1 »" };
  const variable = { kind: "variable", dir: ["HOME", "D"], name: "P" };
  const stack = [{ type: "real", text: "42" }, prog];
  const picked = { target: variable, object: prog };
  // The keys in the memory view: its selected variable, over the stack and the command line.
  assert.deepEqual(editTarget({ writable: true, inView: true, picked, cmdline: true, stack }), variable);
  // The selected stack level.
  assert.deepEqual(editTarget({ writable: true, inView: true, picked: { target: { kind: "level", level: 2 }, object: prog }, stack }), { kind: "level", level: 2 });
  // The keys on the calculator: its command line, else level 1, whatever the view has selected.
  assert.deepEqual(editTarget({ writable: true, inView: false, picked, cmdline: true, stack }), { kind: "cmdline" });
  assert.deepEqual(editTarget({ writable: true, inView: false, picked, cmdline: false, stack }), { kind: "level", level: 1 });
  // In the view with nothing selected: the calculator's.
  assert.deepEqual(editTarget({ writable: true, inView: true, picked: null, cmdline: true, stack }), { kind: "cmdline" });
  assert.deepEqual(editTarget({ writable: true, inView: true, picked: null, stack }), { kind: "level", level: 1 });
  // Nothing to edit: an empty or unread stack, a selection not arrived or without text, no editor now.
  assert.equal(editTarget({ writable: true, stack: [] }), null);
  assert.equal(editTarget({ writable: true, stack: null }), null);
  assert.equal(editTarget({ writable: true, inView: true, picked: { target: variable, object: null }, stack }), null, "not level 1 instead");
  assert.equal(editTarget({ writable: true, inView: true, picked: { target: variable, object: { type: "library" } }, stack }), null);
  assert.equal(editTarget({ writable: true, stack: [{ type: "graphic" }] }), null);
  assert.equal(editTarget({ writable: false, inView: true, picked, cmdline: true, stack }), null, "a 38G, a write running");
});

test("a save that closes the editor reads nothing back; one it stays open after does", async () => {
  const target = { kind: "variable", dir: ["HOME"], name: "P" };
  const calls = [];
  const backend = {
    storeText: async (a) => { calls.push(["storeText", a.was]); return { emulatedMs: 1 }; },
    editText: async () => { calls.push(["editText"]); return { text: "« 2 »", was: "12:0002" }; },
  };
  const s = new EditSession(target, "« 1 »", "10:0001");
  assert.deepEqual(await saveSession(backend, s, "« 2 »", { keep: () => false }), { ok: true });
  assert.deepEqual(calls, [["storeText", "10:0001"]], "no editText");
  assert.equal(s.dirty, false);
  calls.length = 0;
  await saveSession(backend, s, "« 3 »", { keep: () => true });
  assert.deepEqual(calls, [["storeText", "10:0001"], ["editText"]]);
  assert.equal(s.was, "12:0002");
});

test("typed while it saved: open, unsaved, with the save's message or why the next save cannot go", () => {
  const target = { kind: "variable", dir: ["HOME"], name: "P" };
  assert.deepEqual(afterSave(target, { ok: true }, 250, { changed: true }),
    { close: false, notice: { text: "Saved P in HOME in 0.25 s.", error: false } });
  const broken = "Saved, but P in HOME could not be read back (busy). Open it again before saving again.";
  assert.deepEqual(afterSave(target, { ok: true, reread: "busy" }, 250, { changed: true, broken }),
    { close: false, notice: { text: broken, error: true } }, "not a success while Save is off");
  assert.deepEqual(afterSave({ kind: "cmdline" }, { ok: true }, 5, { changed: true }), { close: false, notice: { text: "Sent back.", error: false } });
  assert.equal(afterSave(target, { ok: true }, 5, { changed: false, broken }).close, true, "nothing typed: closes");
});

test("after a keyboard edit the focus waits for an enabled Edit, then falls back to the selected row", () => {
  // Redrawn and enabled: there, at every redraw while it settles.
  assert.equal(editFocusStep({ editEnabled: true, focusFree: true, expired: false }), "edit");
  // Missing until the object is read again, or disabled while a write runs: wait.
  assert.equal(editFocusStep({ editEnabled: false, focusFree: true, expired: false }), "wait");
  // None by the end: the selected row.
  assert.equal(editFocusStep({ editEnabled: false, focusFree: true, expired: true }), "row");
  assert.equal(editFocusStep({ editEnabled: true, focusFree: true, expired: true }), "edit");
  // The user put the focus elsewhere: left alone.
  assert.equal(editFocusStep({ editEnabled: true, focusFree: false, expired: false }), "drop");
});

test("the Edit button: off with the reason, else naming what Cmd/Ctrl+E edits", () => {
  const prog = { type: "program", text: "« 1 »" };
  const variable = { kind: "variable", dir: ["HOME"], name: "PRG" };
  const on = { booted: "48gx", supported: true, busy: false, key: "⌘E" };
  assert.deepEqual(editButtonState({ booted: null }), { off: true, title: "Start the calculator first" });
  assert.deepEqual(editButtonState({ booted: "38g", supported: false }), { off: true, title: "The HP 38G has no editor here (48SX, 48GX and 49G only)" });
  assert.deepEqual(editButtonState({ ...on, busy: true, cmdline: true }), { off: true, title: "Wait: the calculator is busy" });
  assert.deepEqual(editButtonState({ ...on }), { off: true, title: "Nothing to edit: the stack is empty and no command line is open" });
  assert.deepEqual(editButtonState({ ...on, level1: { type: "real", text: "42" } }), { off: false, title: "Edit stack level 1 (⌘E)" });
  assert.deepEqual(editButtonState({ ...on, cmdline: true, level1: prog }), { off: false, title: "Edit the command line (⌘E)" });
  assert.deepEqual(editButtonState({ ...on, inView: true, picked: { target: variable, object: prog }, cmdline: true }), { off: false, title: "Edit PRG in HOME (⌘E)" });
  // The keys back on the calculator: the selection no longer counts.
  assert.deepEqual(editButtonState({ ...on, inView: false, picked: null, cmdline: true }), { off: false, title: "Edit the command line (⌘E)" });
  assert.deepEqual(editButtonState({ ...on, inView: true, picked: { target: variable, object: null } }), { off: true, title: "Still reading the selection from the calculator" });
  assert.deepEqual(editButtonState({ ...on, inView: true, picked: { target: variable, object: { type: "library" } } }), { off: true, title: "PRG in HOME has no text form to edit" });
  assert.deepEqual(editButtonState({ ...on, level1: { type: "graphic" } }), { off: true, title: "Stack level 1 has no text form to edit" });
  assert.deepEqual(editButtonState({ ...on, key: "", level1: { text: "1" } }), { off: false, title: "Edit stack level 1" });
});
