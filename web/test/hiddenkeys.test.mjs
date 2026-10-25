// What the keys do beyond their print (web/hiddenkeys.js), per model:
// the tooltip lines (held-shift ones too), which slot takes a ghost
// label, and the ON combinations (web/onkeys.js): the tables, what a
// press on ON does, and a combination played as held keys. The tables
// against the real skins: web/test/hiddenkeys-skins.test.mjs. `node
// --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { HIDDEN_KEYS, ghostSlot, heldText, hiddenOf, menuLabels, shiftedText } from "../hiddenkeys.js";
import { ON_COMBOS, chordOf, onCombos, onPointerDown, playCombo, runCombo } from "../onkeys.js";
import { clickHints } from "../shiftclick.js";

// The 48GX skin's labels of the keys concerned (crates/saturnus-host/src/skins/hp48gx.rs).
const GX = {
  right: { name: "right", label: "▶", left: "SWAP", right: "" },
  down: { name: "down", label: "▼", left: "VIEW", right: "" },
  eex: { name: "eex", label: "EEX", left: "PURG", right: "ARG" },
  7: { name: "7", label: "7", left: "", right: "SOLVE" },
  3: { name: "3", label: "3", left: "", right: "EQ LIB" },
  prg: { name: "prg", label: "PRG", left: "", right: "CHARS" },
  nxt: { name: "nxt", label: "NXT", left: "PREV", right: "MENU" },
};
const TWO = new Set(["leftshift", "rightshift", "alpha"]);
const hints = (k, mac = true) => clickHints(k, TWO, mac, hiddenOf("48gx", k.name));

test("the table: entries of a known kind and shift; the 48GX's menu keys are its thirteen application keys", () => {
  for (const [model, list] of Object.entries(HIDDEN_KEYS)) {
    for (const e of list) {
      assert.ok(["unprinted", "note", "menu"].includes(e.kind), `${model} ${e.key}: kind`);
      assert.ok(["left", "right"].includes(e.shift), `${model} ${e.key}: shift`);
      assert.ok(e.label, `${model} ${e.key}: label`);
      assert.ok(!e.ghost || e.kind === "unprinted", `${model} ${e.key}: a ghost is for an unprinted function`);
    }
    for (const e of list.filter((e) => e.held)) assert.equal(e.kind, "unprinted", `${model} ${e.key}: a held function is unprinted`);
    const seen = list.map((e) => `${e.key} ${e.shift} ${Boolean(e.held)}`);
    assert.equal(new Set(seen).size, seen.length, `${model}: one entry per key, shift and held`);
  }
  assert.deepEqual(hiddenOf("42s", "right"), {});
  assert.deepEqual([...menuLabels("48gx").keys()].sort(), ["1", "2", "3", "4", "5", "6", "7", "8", "9", "cst", "prg", "up", "var"]);
  assert.deepEqual(menuLabels("48gx").get("7"), { shift: "left", label: "SOLVE" });
  assert.equal(menuLabels("49g").size, 0);
});

test("a one-shift model's entries take both modifiers", () => {
  const ONE = new Set(["shift", "alpha"]);
  const extra = { left: { key: "x", shift: "left", kind: "unprinted", label: "Something" } };
  assert.deepEqual(clickHints({ name: "x", label: "X", left: "" }, ONE, true, extra), ["⌃- or ⌥-click: Something (not printed)"]);
  assert.deepEqual(clickHints({ name: "x", label: "X", left: "Y" }, ONE, false, { left: { kind: "note", label: "Z" } }), ["Ctrl+click or Alt+click: Y (Z)"]);
});

test("the tooltips say what the print does not", () => {
  assert.deepEqual(hints(GX.right), ["⌃-click: SWAP", "⌥-click: Kermit server (not printed)"]);
  assert.deepEqual(hints(GX.right, false), ["Ctrl+click: SWAP", "Alt+click: Kermit server (not printed)"]);
  assert.deepEqual(hints(GX.down), ["⌃-click: VIEW", "⌥-click: REVIEW, the menu's labels in full (not printed)"]);
  assert.deepEqual(hints(GX.eex), ["⌃-click: PURG", "⌥-click: ARG (LASTARG)"]);
  assert.deepEqual(hints(GX[7]), ["⌃-click: SOLVE menu", "⌥-click: SOLVE"]);
  assert.deepEqual(hints(GX[3]), ["⌃-click: EQ LIB menu", "⌥-click: EQ LIB"]);
  assert.deepEqual(hints(GX.prg), ["⌃-click: CHARS menu", "⌥-click: CHARS"]);
  assert.deepEqual(hints(GX.nxt), ["⌃-click: PREV", "⌥-click: MENU"], "a key with nothing more");
  // An unprinted entry never covers a printed label, a note never stands alone.
  assert.equal(shiftedText({ right: "X" }, "right", { right: { kind: "unprinted", label: "Y" } }), "X");
  assert.equal(shiftedText({ right: "" }, "right", { right: { kind: "note", label: "Y" } }), null);
});

test("every entry fits the skin: unprinted ones in empty slots, notes and menus on printed labels", () => {
  for (const [name, k] of Object.entries(GX)) {
    const e = hiddenOf("48gx", name);
    for (const side of ["left", "right"]) {
      if (e[side]?.kind === "unprinted") assert.equal(k[side], "", `${name} ${side} is not printed`);
      if (e[side]?.kind === "note") assert.notEqual(k[side], "", `${name} ${side} is printed`);
      if (e[side]?.kind === "menu") assert.ok(!k[side] && [k.left, k.right].includes(e[side].label), `${name}: the application's label printed under the other shift`);
    }
  }
});

test("a ghost takes an empty slot only", () => {
  assert.equal(ghostSlot(GX.right, "right", hiddenOf("48gx", "right")), "SERVER");
  assert.equal(ghostSlot(GX.down, "right", hiddenOf("48gx", "down")), "REVIEW");
  assert.equal(ghostSlot(GX.right, "left", hiddenOf("48gx", "right")), null);
  assert.equal(ghostSlot({ right: "SWAP" }, "right", { right: { kind: "unprinted", label: "x", ghost: "X" } }), null, "a taken slot");
  assert.equal(ghostSlot(GX.eex, "right", hiddenOf("48gx", "eex")), null, "a note has no ghost");
  assert.equal(ghostSlot(GX[7], "left", hiddenOf("48gx", "7")), null, "a menu has no ghost");
});

test("the ON combinations per model, as the menu writes them", () => {
  const menu = (m) => onCombos(m).map((c) => `${c.title} ${chordOf(c)}`);
  assert.deepEqual(menu("48gx"), ["Restart ON+C", "Self-test ON+D", "Print display ON+1", "Reset memory… ON+A+F"]);
  assert.deepEqual(menu("48sx"), ["Restart ON+C", "Self-test ON+D", "Print display ON+MTH", "Reset memory… ON+A+F"]);
  assert.deepEqual(menu("49g"), ["Restart ON+F3", "Self-test ON+F4", "Reset memory… ON+F1+F6"]);
  assert.deepEqual(menu("28s"), ["Restart ON+▲", "Print display ON+L", "Clear memory… ON+INS+▶"]);
  assert.deepEqual(menu("38g"), ["Restart ON+menu key 3", "Clear memory… ON+menu keys 1 and 6"]);
  assert.deepEqual(menu("39g"), ["Restart ON+menu key 3", "Self-test ON+menu key 4", "Clear memory… ON+menu keys 1 and 6"]);
  assert.deepEqual(menu("40g"), menu("39g"));
  assert.deepEqual(menu("42s"), ["Restart EXIT+√x", "Self-test EXIT+LN", "Clear memory… EXIT+Σ++XEQ"]);
  for (const m of ["27s", "17b", "17bii"]) assert.deepEqual(menu(m), ["Restart CLR+menu key 3", "Clear memory… CLR+menu keys 1 and 6"]);
  assert.deepEqual(menu("19bii"), ["Restart ON+menu key 3", "Print display ON+PRNT", "Clear memory… ON+menu keys 1 and 6"]);
  assert.deepEqual(onCombos("nope"), []);
  assert.equal(Object.keys(ON_COMBOS).length, 12);
  for (const [m, list] of Object.entries(ON_COMBOS)) {
    assert.ok(list.every((c) => c.keys[0] === "on"), `${m}: ON first`);
    // Every combination that clears memory asks first, and only those.
    assert.deepEqual(list.filter((c) => c.ask).map((c) => c.id), ["reset-memory"], `${m}: only the memory reset asks`);
    assert.ok(list.find((c) => c.id === "reset-memory").description.endsWith("Asks first."), `${m}: says it asks`);
  }
  assert.equal(onCombos("48sx").find((c) => c.id === "reset-memory").hold >= 1000, true, "the 48SX's reset is held a second");
  assert.equal(onCombos("49g").find((c) => c.id === "reset-memory").ask.body.includes("NO (F6)"), true);
});

// The 48SX (wiki: hardware/hp48sx-keyboard), with its skin's labels (crates/saturnus-host/src/skins/hp48sx.rs).
const SX = {
  prg: { name: "prg", label: "PRG", left: "I/O", right: "" },
  cst: { name: "cst", label: "CST", left: "MODES", right: "" },
  9: { name: "9", label: "9", left: "ALGEBRA", right: "" },
  2: { name: "2", label: "2", left: "STACK", right: "ARG" },
  backspace: { name: "backspace", label: "⬅", left: "DROP", right: "CLR" },
  left: { name: "left", label: "◀", left: "GRAPH", right: "" },
  mth: { name: "mth", label: "MTH", left: "PRINT", right: "" },
};

test("the 48SX: application names on the left, their other menus and unprinted functions on the right", () => {
  const sx = (k) => clickHints(k, TWO, true, hiddenOf("48sx", k.name));
  assert.deepEqual(sx(SX.prg), ["⌃-click: I/O", "⌥-click: Kermit server (not printed)"]);
  assert.deepEqual(sx(SX.cst), ["⌃-click: MODES", "⌥-click: MODES customization menu"]);
  assert.deepEqual(sx(SX[9]), ["⌃-click: ALGEBRA", "⌥-click: Equation Catalog (not printed)"]);
  assert.deepEqual(sx(SX[2]), ["⌃-click: STACK (LAST STACK, the stack before the last command line)", "⌥-click: ARG (LAST ARG, the last command's arguments)"]);
  assert.deepEqual(sx(SX.backspace), ["⌃-click: DROP", "⌥-click: CLR (clears the whole stack, with no confirmation)"]);
  assert.deepEqual(sx(SX.left), ["⌃-click: GRAPH", "⌥-click: in a command line, the cursor to its start (not printed)"]);
  assert.deepEqual(sx(SX.mth), ["⌃-click: PRINT"], "→ MTH does nothing: no line");
  // ⌥ lights the left-printed application whose menu → opens, in green.
  const menus = menuLabels("48sx");
  assert.deepEqual([...menus.keys()].sort(), ["5", "6", "7", "8", "cst", "var"]);
  assert.deepEqual(menus.get("7"), { shift: "right", label: "SOLVE" });
  // Ghosts: in the empty right slot, none for a command-line function.
  assert.equal(ghostSlot(SX.prg, "right", hiddenOf("48sx", "prg")), "SERVER");
  assert.equal(ghostSlot(SX[9], "right", hiddenOf("48sx", "9")), "EQ CAT");
  assert.equal(ghostSlot(SX.left, "right", hiddenOf("48sx", "left")), null);
  assert.equal(ghostSlot(SX.cst, "right", hiddenOf("48sx", "cst")), null, "a menu has no ghost");
});

test("the 49G: a held shift is a tooltip line of its own, never a ghost or a glow", () => {
  const g = (k, mac = true) => clickHints(k, TWO, mac, hiddenOf("49g", k.name));
  assert.deepEqual(g({ name: "mode", label: "MODE", left: "CUSTOM", right: "END" }), [
    "⌃-click: CUSTOM", "⌥-click: END", "With the left shift held down: MODES menu (not printed)",
  ]);
  const right = { name: "right", label: "▶", left: "", right: "" };
  assert.deepEqual(g(right, false), [
    "Ctrl+click: SWAP, as ▶; in a command line, the cursor to its end (not printed)",
    "Alt+click: XModem server (not printed)",
    "With the right shift held down: Kermit server (not printed)",
  ]);
  assert.equal(ghostSlot(right, "right", hiddenOf("49g", "right")), "XSERV", "the tapped server's ghost");
  assert.equal(ghostSlot(right, "left", hiddenOf("49g", "right")), null, "SWAP as ▶: no ghost");
  assert.deepEqual(g({ name: "a", label: "F1", left: "Y=", right: "" }), ["⌃-click: Y= (algebraic mode; in RPN mode hold the left shift down)"]);
  assert.equal(heldText("left", hiddenOf("49g", "7")), null);
  assert.equal(heldText("right", hiddenOf("49g", "7")), "With the right shift held down: SOLVE menu (not printed)");
  assert.equal(menuLabels("49g").size, 0, "no glow for held menus");
});

test("one-shift models: the one shift, both modifiers; the SETUP bracket; the 19BII's OFF ghost", () => {
  const ONE = new Set(["shift", "alpha"]);
  const plot = { name: "plot", label: "PLOT", left: "" };
  assert.deepEqual(clickHints(plot, ONE, true, hiddenOf("38g", "plot")), ["⌃- or ⌥-click: SETUP-PLOT, the plot setup view (the SETUP bracket)"]);
  assert.equal(ghostSlot(plot, "left", hiddenOf("38g", "plot")), null, "named by the bracket: no ghost");
  assert.deepEqual(clickHints({ name: "up", label: "▲", left: "" }, ONE, false, hiddenOf("39g", "up")), ["Ctrl+click or Alt+click: the beginning of a list; in HOME, the oldest entry (not printed)"]);
  assert.match(hiddenOf("40g", "symb").left.label, /where the aplet has one/);
  const on = { name: "on", label: "ON", left: "" };
  assert.deepEqual(clickHints(on, ONE, true, hiddenOf("19bii", "on")), ["⌃- or ⌥-click: OFF (not printed)"]);
  assert.equal(ghostSlot(on, "left", hiddenOf("19bii", "on")), "OFF");
  assert.deepEqual(clickHints({ name: "alpha", label: "α", left: "MENUS" }, ONE, true, hiddenOf("28s", "alpha")), ["⌃- or ⌥-click: MENUS (Menu Lock: A to R select their menus unshifted until pressed again)"]);
  assert.deepEqual(clickHints({ name: "a", label: "", left: "" }, ONE, true, hiddenOf("28s", "a")), ["⌃- or ⌥-click: in a command line, deletes everything left of the cursor (not printed)"]);
  for (const m of ["42s", "27s", "17b", "17bii"]) assert.equal((HIDDEN_KEYS[m] ?? []).length, 0, `${m}: nothing unprinted`);
});

test("a press on ON: a right-click is the menu, a modifier click the shifted press, else it waits", () => {
  const ev = (o) => ({ pointerType: "mouse", button: 0, ...o });
  assert.equal(onPointerDown(ev({ button: 2 }), null), "menu");
  assert.equal(onPointerDown(ev({ button: 2 }), "ctrl"), "press", "a Mac's Ctrl+click is a secondary click: CONT");
  assert.equal(onPointerDown(ev({}), "alt"), "press");
  assert.equal(onPointerDown(ev({}), null), "wait");
  assert.equal(onPointerDown(ev({ button: 1 }), null), "press");
  assert.equal(onPointerDown({ pointerType: "touch", button: 0 }, null), "wait");
  assert.equal(onPointerDown({ pointerType: "pen", button: 0 }, null), "wait");
});

/** A backend and a store whose machine takes each key at once. */
function machine({ takes = true } = {}) {
  const sent = [];
  const down = new Set();
  const watchers = new Set();
  const store = {
    state: { booted: "48gx", running: true, keysDown: [] },
    watch(_keys, fn) { watchers.add(fn); return () => watchers.delete(fn); },
    set(p) { Object.assign(this.state, p); },
  };
  const changed = () => {
    store.state.keysDown = [...down];
    for (const fn of [...watchers]) fn(store.state);
  };
  const backend = {
    keyDown(k) { sent.push(`down ${k}`); if (takes) queueMicrotask(() => { down.add(k); changed(); }); },
    keyUp(k) { sent.push(`up ${k}`); queueMicrotask(() => { down.delete(k); changed(); }); },
    keyUpAll() { sent.push("up all"); },
  };
  return { sent, store, backend, watchers };
}

test("a combination: all down, the others released before ON", async () => {
  const m = machine();
  assert.equal(await playCombo(m.backend, m.store, ["on", "a", "f"]), true);
  assert.deepEqual(m.sent, ["down on", "down a", "down f", "up f", "up a", "up on"]);
  assert.equal(m.watchers.size, 0, "stopped watching");
});

test("a combination held longer: released only `hold` ms after the machine has them all", async () => {
  const m = machine();
  const timers = [];
  const p = playCombo(m.backend, m.store, ["on", "a", "f"], { hold: 1200, setTimer: (f, ms) => { timers.push([f, ms]); return timers.length; }, clearTimer: () => {} });
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.deepEqual(m.sent, ["down on", "down a", "down f"], "held");
  const held = timers.find(([, ms]) => ms === 1200);
  assert.ok(held, "a timer for the hold");
  held[0]();
  assert.equal(await p, true);
  assert.deepEqual(m.sent, ["down on", "down a", "down f", "up f", "up a", "up on"]);
});

test("a combination the machine does not take is released after the timeout", async () => {
  const m = machine({ takes: false });
  let fire = null;
  const p = playCombo(m.backend, m.store, ["on", "c"], { setTimer: (f) => { fire = f; return 1; }, clearTimer: () => {} });
  fire();
  assert.equal(await p, false);
  assert.deepEqual(m.sent, ["down on", "down c", "up c", "up on"], "its own keys, in reverse");
});

test("Reset memory asks first; a paused calculator refuses", async () => {
  const reset = onCombos("48gx").find((c) => c.id === "reset-memory");
  let m = machine();
  assert.equal(await runCombo(reset, m.backend, m.store, async () => false), false);
  assert.deepEqual(m.sent, [], "Cancel: nothing pressed");
  let asked = null;
  assert.equal(await runCombo(reset, m.backend, m.store, async (q) => { asked = q; return true; }), true);
  assert.equal(asked.action, "Reset memory");
  assert.deepEqual(m.sent.slice(0, 3), ["down on", "down a", "down f"]);
  m = machine();
  m.store.state.running = false;
  assert.equal(await runCombo(onCombos("48gx")[0], m.backend, m.store, async () => true), false);
  assert.deepEqual(m.sent, []);
  assert.equal(m.store.state.messageError, true);
});

test("a second combination is refused while one is being pressed", async () => {
  // The machine takes no key until the test says so: the first combination waits.
  const m = machine({ takes: false });
  const restart = onCombos("48gx")[0];
  const first = runCombo(restart, m.backend, m.store, async () => true);
  await Promise.resolve();
  assert.equal(await runCombo(restart, m.backend, m.store, async () => true), false);
  assert.equal(m.store.state.message, "An ON combination is still being pressed.");
  // Let the first finish: the machine takes the keys now.
  for (const k of ["on", "c"]) m.store.state.keysDown.push(k);
  for (const fn of [...m.watchers]) fn(m.store.state);
  m.store.state.keysDown = [];
  for (const fn of [...m.watchers]) fn(m.store.state);
  assert.equal(await first, true);
});
