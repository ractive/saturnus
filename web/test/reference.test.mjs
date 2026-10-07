// The command palette's lookup, ranking and Enter rule (web/reference.js)
// against the committed data (web/commands.json): `node --test web/test/`
// (just web-test).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  buildIndex, enterPlan, enterVerb, exampleResult, exampleText, flattenMenus, fromCodes, friendly,
  isSingleToken, keyLegend, manualLinks, menuCommands, placement, search, spaced, stackVerified, toCodes,
} from "../reference.js";
import { IndexWatch, PaletteModel, isPaletteChord, numberShortcut, shortcutDigit } from "../palette.js";

const data = JSON.parse(readFileSync(new URL("../commands.json", import.meta.url), "utf8"));
const sx = buildIndex(data, "48sx");
const g49 = buildIndex(data, "49g");
const names = (rows) => rows.map((r) => (r.kind === "command" ? r.name : `${r.kind}:${r.name}`));

test("codes and friendly spellings round-trip as saturnus ref does", () => {
  assert.equal(toCodes("→LIST"), "\\->LIST");
  assert.equal(fromCodes("\\.S"), "∫");
  assert.equal(fromCodes("plain"), null);
  assert.equal(friendly("Σ+"), "SIGMA+");
  for (const c of g49.commands) {
    const rows = search(g49, toCodes(c.name));
    assert.equal(rows[0].name, c.name, `${toCodes(c.name)} finds ${c.name} first`);
  }
});

test("an exact name wins; case, codes and friendly spellings find the name", () => {
  assert.equal(search(g49, "INT")[0].name, "INT");
  assert.equal(search(g49, "int")[0].name, "INT");
  assert.equal(search(g49, "\\.S")[0].name, "∫");
  assert.equal(search(g49, "SIGMA")[0].name, "SIGMA");
  assert.equal(search(g49, "\\GS")[0].name, "Σ");
  assert.equal(search(g49, "->list")[0].name, "→LIST");
  assert.equal(search(g49, "sigma+")[0].name, "Σ+");
  assert.equal(search(g49, "UPMATCH")[0].name, "↑MATCH");
  assert.equal(search(g49, "qr")[0].name, "qr");
  // A collision lists every candidate at the top.
  const qr = names(search(g49, "Qr"));
  assert.deepEqual(qr.slice(0, 2).sort(), ["QR", "qr"]);
});

test("INT on the 49G lists INT and ∫ at the top", () => {
  const rows = search(g49, "INT");
  assert.equal(rows[0].name, "INT");
  assert.equal(rows[1].name, "∫", `∫ second: ${names(rows).slice(0, 4)}`);
  assert.ok(rows.every((r) => r.kind !== "send"), "an exact match needs no send row");
});

test("prefix beats substring beats description; PL on the 49G is PLOT first, with its stack effect", () => {
  const rows = search(g49, "PL");
  assert.equal(rows[0].name, "PLOT");
  assert.equal(rows[0].stack, "'symb' →");
  assert.ok(rows.every((r, i) => i === 0 || r.score <= rows[i - 1].score));
  const cmds = rows.filter((r) => r.kind === "command").map((r) => r.name);
  assert.deepEqual(cmds.slice(0, 2), ["PLOT", "PLOTADD"], "the two PL- names first");
  assert.ok(cmds[2].includes("PL") && !cmds[2].startsWith("PL"), `then a substring match: ${cmds[2]}`);
  // The 48SX has no PLOT command, but a PLOT menu: offered as a row.
  const sxRows = search(sx, "PL");
  assert.equal(sxRows[0].kind, "menu");
  assert.equal(sxRows[0].name, "PLOT");
  assert.ok(sxRows[0].count > 5);
});

test("sto and store both find STO", () => {
  assert.equal(search(sx, "sto")[0].name, "STO");
  const store = search(sx, "store");
  assert.ok(store.some((r) => r.name === "STO"), `${names(store).slice(0, 12)}`);
  assert.equal(store[0].name, "STO", `${names(store).slice(0, 12)}`);
  // A description match is ordered by name, not by its length: the operators do not crowd the top.
  const int = names(search(g49, "INT").filter((r) => r.score < 500));
  assert.deepEqual(int.slice(0, 3), [...int.slice(0, 3)].sort((a, b) => a.localeCompare(b)));
});

test("variables rank with the commands, actions after them, and no match is an empty list plus the send row", () => {
  const variables = [{ name: "SINE", path: ["HOME"], type: "Program" }, { name: "A", path: ["HOME"], type: "Real Number" }];
  const actions = [{ id: "save", title: "Save state" }, { id: "speed", title: "Speed: max", keywords: "fast" }];
  const rows = search(sx, "SIN", { variables, actions });
  assert.equal(rows[0].name, "SIN");
  const vi = rows.findIndex((r) => r.kind === "variable" && r.name === "SINE");
  assert.ok(vi > 0 && vi < 4, `${names(rows).slice(0, 6)}`);
  assert.equal(search(sx, "A", { variables })[0].kind, "variable");
  // A command that starts with the letters comes before an action that does.
  const sa = search(sx, "sa", { actions });
  assert.equal(sa[0].name, "SAME");
  assert.equal(sa[1].kind, "action");
  assert.equal(search(sx, "fast", { actions })[0].name, "Speed: max");
  const none = search(sx, "zzqq");
  assert.deepEqual(names(none), ["send:zzqq"]);
  assert.deepEqual(search(sx, "zzqq", { typing: false }), []);
  assert.deepEqual(names(search(sx, "", { actions })), ["action:Save state", "action:Speed: max"]);
});

test("text that is not a single name is sent as typed, first", () => {
  assert.equal(isSingleToken("SIN"), true);
  assert.equal(isSingleToken("13 4 ^"), false);
  assert.equal(isSingleToken("« 1 2 + »"), false);
  assert.equal(isSingleToken("30"), false);
  assert.equal(isSingleToken("'X'"), false);
  assert.equal(isSingleToken("-"), true);
  const rows = search(sx, "13 4 ^");
  assert.equal(rows[0].kind, "send");
  assert.equal(rows[0].name, "13 4 ^");
  assert.equal(search(sx, "30")[0].kind, "send");
  // A bare token that names nothing exactly can still be sent, last.
  const pl = search(sx, "PR");
  assert.equal(pl.at(-1).kind, "send");
  assert.notEqual(pl[0].kind, "send");
});

test("the Enter rule mimics the keys and Cmd/Ctrl+Enter does the opposite", () => {
  const cmd = { kind: "command", name: "SIN" };
  const closed = { active: false, text: "", cursor: 0 };
  const open = { active: true, text: "30", cursor: 2 };
  assert.deepEqual(enterPlan(cmd, closed), { verb: "run", text: "SIN" });
  assert.deepEqual(enterPlan(cmd, open), { verb: "insert", text: " SIN" });
  assert.deepEqual(enterPlan(cmd, closed, true), { verb: "insert", text: "SIN" });
  assert.deepEqual(enterPlan(cmd, open, true), { verb: "run", text: " SIN" });
  assert.equal(enterVerb(cmd, null), "run");
  // A variable inserts its name; the opposite evaluates it.
  const v = { kind: "variable", name: "A" };
  assert.deepEqual(enterPlan(v, closed), { verb: "insert", text: "A" });
  assert.deepEqual(enterPlan(v, open), { verb: "insert", text: " A" });
  assert.deepEqual(enterPlan(v, closed, true), { verb: "run", text: "A" });
  // Typed text follows the command rule.
  const send = { kind: "send", name: "13 4 ^" };
  assert.deepEqual(enterPlan(send, closed), { verb: "run", text: "13 4 ^" });
  assert.deepEqual(enterPlan(send, open), { verb: "insert", text: " 13 4 ^" });
  assert.deepEqual(enterPlan({ kind: "action", action: { id: "x" } }, closed), { action: { id: "x" } });
  assert.equal(spaced("SIN", { text: "1 2", cursor: 1 }), " SIN");
  assert.equal(spaced("SIN", { text: "12", cursor: 1 }), " SIN ");
  assert.equal(spaced("SIN", { text: "1 ", cursor: 2 }), "SIN");
});

test("examples, placement and manual links read the data as saturnus ref prints them", () => {
  const sin = sx.byName.get("SIN");
  assert.equal(exampleText(sin.per.examples[0]), "DEG 30. SIN");
  assert.deepEqual(exampleResult(sin.per.examples[0]), { lines: [".5"] });
  assert.deepEqual(exampleResult({ error: "Bad input" }), { error: "Bad input" });
  assert.equal(stackVerified(sin), true);
  const keys = [{ name: "sin", label: "SIN", left: "ASIN", right: "∂" }];
  assert.equal(keyLegend(keys, "SIN"), "SIN key");
  assert.equal(keyLegend(keys, "ASIN"), "left shift, SIN key");
  assert.equal(keyLegend(keys, "COS"), null);
  const p = placement(sx, sin, keys);
  assert.deepEqual(p, { menus: [], key: null, keyboard: "SIN key", group: null });
  const abs = placement(sx, sx.byName.get("ABS"));
  assert.deepEqual(abs.menus, ["MTH PARTS", "MTH MATRX", "MTH VECTR"]);
  assert.equal(abs.key.category, "MTH");
  assert.match(abs.key.url, /hp48sx-om-en\.pdf#page=\d+$/);
  const drop = placement(sx, sx.byName.get("DROP"));
  assert.equal(drop.key.other, true);
  const links = manualLinks(sx, sin, ["48sx"]);
  assert.ok(links.length >= 1 && links.every((l) => l.url.includes("#page=")));
  assert.ok(links.every((l) => l.models.includes("48sx")));
  assert.equal(stackVerified(g49.byName.get("PLOT")), false);
});

test("the menu tree follows the ROM's menu paths, roots in key order", () => {
  const roots = sx.menus.filter((m) => m.fromRom).map((m) => m.name);
  assert.equal(roots[0], "MTH");
  assert.ok(roots.includes("PLOT") && roots.includes("PRG"));
  const mth = sx.menus.find((m) => m.name === "MTH");
  assert.ok(mth.children.some((c) => c.name === "BASE" && c.path === "MTH BASE"));
  assert.ok(menuCommands(mth).some((c) => c.name === "ABS"));
  const all = flattenMenus(sx.menus);
  const tvm = flattenMenus(buildIndex(data, "48gx").menus).find((m) => m.path === "MENU 74 TVM");
  assert.ok(tvm && tvm.depth === 1, "MENU 74 is one name of two words");
  assert.ok(all.some((m) => !m.fromRom && m.name === "Keyboard"));
  // Every command of the model is somewhere in the tree.
  const placed = new Set(all.flatMap((m) => m.commands.map((c) => c.name)));
  assert.equal(placed.size, sx.commands.length);
  assert.equal(buildIndex(data, "38g").supported, false);
});

test("ranking stays well under a frame on every keystroke", () => {
  const variables = Array.from({ length: 40 }, (_, i) => ({ name: `V${i}`, path: ["HOME"] }));
  const actions = Array.from({ length: 12 }, (_, i) => ({ id: `a${i}`, title: `Action ${i}` }));
  const queries = ["S", "ST", "STO", "sto", "store", "13 4 ^", "\\->", "INT", "z", "zz"];
  for (const idx of [sx, g49]) {
    const t0 = performance.now();
    for (let n = 0; n < 20; n++) for (const q of queries) search(idx, q, { variables, actions });
    const per = (performance.now() - t0) / (20 * queries.length);
    assert.ok(per < 4, `${idx.model}: ${per.toFixed(2)} ms per search`);
  }
});

// ------------------------------------------------------------ the model

/** A backend that records the verbs and answers like a 48 with a line open or not. */
function fakeBackend({ active = false, error = null } = {}) {
  const calls = [];
  const line = { active, text: active ? "30" : "", cursor: active ? 2 : 0 };
  return {
    calls,
    line,
    async commandLine() { return { ...line }; },
    async flags() { return { system: [], user: [], set: [] }; },
    async insert(text) { calls.push(["insert", text]); line.active = true; line.text += text; line.cursor = line.text.length; return { commandLine: { ...line } }; },
    async run(text) {
      calls.push(["run", text]);
      if (error) { line.active = true; line.text += text; return { closed: false, error, commandLine: { ...line } }; }
      line.active = false; line.text = ""; line.cursor = 0;
      return { closed: true, error: null, commandLine: { ...line } };
    },
  };
}

test("the palette model: Enter runs with no line open, inserts into one, and reports the calculator's error", async () => {
  const backend = fakeBackend();
  const m = new PaletteModel(backend, { model: "48sx", booted: "48sx" });
  m.setIndex(sx);
  await m.open();
  m.setQuery("SIN");
  assert.equal(m.rows[0].name, "SIN");
  const r = await m.choose(m.rows[0]);
  assert.deepEqual(backend.calls, [["run", "SIN"]]);
  assert.equal(r.close, true);
  assert.equal(m.notice, null);

  const b2 = fakeBackend({ active: true });
  const m2 = new PaletteModel(b2, { model: "48sx", booted: "48sx" });
  m2.setIndex(sx);
  await m2.open();
  assert.equal(m2.commandLine.active, true);
  m2.setQuery("SIN");
  await m2.choose(m2.rows[0]);
  assert.deepEqual(b2.calls, [["insert", " SIN"]]);
  await m2.choose(m2.rows[0], { opposite: true });
  assert.equal(b2.calls[1][0], "run");

  const b3 = fakeBackend({ error: "Invalid Syntax" });
  const m3 = new PaletteModel(b3, { model: "48sx", booted: "48sx" });
  m3.setIndex(sx);
  await m3.open();
  m3.setQuery("13 4 ^");
  assert.equal(m3.rows[0].kind, "send");
  const r3 = await m3.choose(m3.rows[0]);
  assert.equal(r3.close, false);
  assert.match(m3.notice.text, /Invalid Syntax/);
  assert.equal(m3.notice.error, true);
  assert.equal(m3.commandLine.active, true, "the line stays open: the hints follow");
});

test("the palette model: number shortcuts pick a row, arrows move, variables come from the memory tree", async () => {
  const backend = fakeBackend();
  const m = new PaletteModel(backend, { model: "48sx", booted: "48sx" });
  m.setIndex(sx);
  m.setVariables({ path: ["HOME", "D"], variables: [
    { name: "D", type: "Directory", variables: [{ name: "IN", type: "Real Number" }] },
    { name: "OUT", type: "Real Number" },
    { name: "E", type: "Directory", variables: [{ name: "HIDDEN", type: "Real Number" }] },
  ] });
  // The current directory and its parents, nearest first; not siblings' insides.
  assert.deepEqual(m.variables.map((v) => v.name), ["IN", "D", "OUT", "E"]);
  await m.open();
  m.setQuery("I");
  assert.equal(m.rows.find((r) => r.kind === "variable").name, "IN");
  m.move(1);
  assert.equal(m.selected, 1);
  m.move(-5);
  assert.equal(m.selected, 0);
  const third = m.rows[2];
  await m.chooseNumber(3);
  assert.equal(backend.calls.at(-1)[1], third.kind === "variable" ? third.name : third.name);
});

test("Cmd/Ctrl+K is one toggle: an event the palette handled is not the chord again", () => {
  const ev = (o) => ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, code: "KeyK", defaultPrevented: false, ...o });
  assert.equal(isPaletteChord(ev({ metaKey: true })), true);
  assert.equal(isPaletteChord(ev({ ctrlKey: true })), true);
  assert.equal(isPaletteChord(ev({ ctrlKey: true, defaultPrevented: true })), false, "closed inside the palette: the document must not reopen it");
  assert.equal(isPaletteChord(ev({ ctrlKey: true, shiftKey: true })), false);
  assert.equal(isPaletteChord(ev({ code: "KeyJ", metaKey: true })), false);
  assert.equal(isPaletteChord(ev({})), false);
});

test("number shortcuts: Cmd in the Mac app, Ctrl in Mac browsers, Alt in browsers elsewhere", () => {
  assert.equal(numberShortcut("tauri", true).key, "meta");
  assert.equal(numberShortcut("tauri", true).label(3), "⌘3");
  assert.equal(numberShortcut("worker", true).key, "ctrl");
  assert.equal(numberShortcut("worker", true).label(3), "⌃3");
  assert.equal(numberShortcut("worker", false).key, "alt");
  assert.equal(numberShortcut("worker", false).label(3), "Alt+3");
  assert.equal(numberShortcut("tauri", false).key, "alt");
  const ev = (o) => ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, code: "Digit3", ...o });
  assert.equal(shortcutDigit(ev({ altKey: true }), "alt"), 3);
  assert.equal(shortcutDigit(ev({ ctrlKey: true }), "alt"), null, "Ctrl+3 is the browser's tab on Windows and Linux");
  assert.equal(shortcutDigit(ev({ ctrlKey: true }), "ctrl"), 3);
  assert.equal(shortcutDigit(ev({ metaKey: true }), "meta"), 3);
  assert.equal(shortcutDigit(ev({ metaKey: true, ctrlKey: true }), "meta"), null);
  assert.equal(shortcutDigit(ev({ altKey: true, shiftKey: true }), "alt"), null);
  assert.equal(shortcutDigit(ev({ altKey: true, code: "Digit0" }), "alt"), null);
});

test("a failing app action is reported in the notice, not thrown", async () => {
  const backend = fakeBackend();
  const m = new PaletteModel(backend, { model: "48sx", booted: "48sx" }, {
    actions: [{ id: "bad", title: "Save state", run: async () => { throw new Error("the browser refused to store"); } }, { id: "ok", title: "Reset", run: async () => {} }],
  });
  m.setIndex(sx);
  await m.open();
  m.setQuery("save");
  const bad = m.rows.find((r) => r.kind === "action" && r.name === "Save state");
  const r = await m.choose(bad);
  assert.equal(r.close, false);
  assert.match(m.notice.text, /Save state: the browser refused to store/);
  assert.equal(m.notice.error, true);
  m.setQuery("reset");
  const ok = m.rows.find((r) => r.kind === "action");
  assert.deepEqual(await m.choose(ok), { close: true });
});

test("the Commands tab's index: a failed load is kept for its model only and tried again after a change", async () => {
  let fail = true;
  const calls = [];
  const loader = { async index(model) { calls.push(model); if (fail) throw new Error("HTTP 503"); return buildIndex(data, model); } };
  const w = new IndexWatch(loader);
  assert.deepEqual(w.state("48sx"), { loading: true });
  assert.deepEqual(await w.ensure("48sx"), { error: "HTTP 503" });
  assert.deepEqual(await w.ensure("48sx"), { error: "HTTP 503" }, "the same model: no second load, the error stands");
  assert.equal(calls.length, 1);
  fail = false;
  // Another model loads afresh; back to the first one, so does it.
  assert.equal((await w.ensure("49g")).index.model, "49g");
  assert.equal((await w.ensure("48sx")).index.model, "48sx");
  assert.deepEqual(w.state("48sx").index.model, "48sx");
  // A reset (a ROM booted) forgets a failure too.
  fail = true;
  w.reset();
  assert.deepEqual(await w.ensure("48gx"), { error: "HTTP 503" });
  w.reset();
  fail = false;
  assert.equal((await w.ensure("48gx")).index.model, "48gx");
  // Two callers during one load share it.
  const w2 = new IndexWatch(loader);
  const [a, b] = await Promise.all([w2.ensure("48sx"), w2.ensure("48sx")]);
  assert.equal(a.index, b.index);
});
