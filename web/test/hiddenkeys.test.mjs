// What the 48GX's keys do beyond their print (web/hiddenkeys.js): the
// data against the skin's labels, the tooltip lines, which slot takes a
// ghost label, and the ON combinations
// (web/onkeys.js): the table, what a press on ON does, and a combination
// played as held keys. `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { HIDDEN_KEYS, ghostSlot, hiddenOf, menuLabels, shiftedText } from "../hiddenkeys.js";
import { ON_COMBOS, onCombos, onPointerDown, playCombo, runCombo } from "../onkeys.js";
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
    const seen = list.map((e) => `${e.key} ${e.shift}`);
    assert.equal(new Set(seen).size, seen.length, `${model}: one entry per key and shift`);
  }
  assert.deepEqual(hiddenOf("48sx", "right"), {});
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

test("the ON combinations of the 48GX, and none elsewhere", () => {
  assert.deepEqual(onCombos("48gx").map((c) => [c.title, c.keys.join("+")]), [
    ["Restart", "on+c"], ["Self-test", "on+d"], ["Print display", "on+1"], ["Reset memory…", "on+a+f"],
  ]);
  assert.deepEqual(onCombos("48sx"), []);
  assert.deepEqual(Object.keys(ON_COMBOS), ["48gx"]);
  assert.ok(onCombos("48gx").every((c) => c.keys[0] === "on"));
  assert.deepEqual(onCombos("48gx").filter((c) => c.ask).map((c) => c.id), ["reset-memory"], "only Reset memory asks");
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
