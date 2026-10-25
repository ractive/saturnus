// What the 48GX's keys do beyond their print (web/hiddenkeys.js): the
// data against the skin's labels, the tooltip lines, which slot takes a
// ghost label and where it goes, and the ON combinations
// (web/onkeys.js): the table, what a press on ON does, and a combination
// played as held keys. `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { GHOST_MIN_SCALE, GHOST_PAD, HIDDEN_KEYS, ghostPlacement, ghostSlot, hiddenOf, menuKeys, shiftedText } from "../hiddenkeys.js";
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

test("only the 48GX has a table; its menu keys are the thirteen application keys", () => {
  assert.deepEqual(Object.keys(HIDDEN_KEYS), ["48gx"]);
  assert.deepEqual(hiddenOf("48sx", "right"), {});
  assert.deepEqual([...menuKeys("48gx")].sort(), ["1", "2", "3", "4", "5", "6", "7", "8", "9", "cst", "prg", "up", "var"]);
  assert.equal(menuKeys("49g").size, 0);
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
  assert.equal(shiftedText({ right: "X" }, "right", { right: { unprinted: "Y" } }), "X");
  assert.equal(shiftedText({ right: "" }, "right", { right: { note: "Y" } }), null);
  assert.equal(shiftedText({ right: "" }, "left", { left: { menu: true } }), null, "a menu needs the printed application");
});

test("every entry fits the skin: unprinted ones in empty slots, notes and menus on printed labels", () => {
  for (const [name, k] of Object.entries(GX)) {
    const e = hiddenOf("48gx", name);
    for (const side of ["left", "right"]) {
      if (e[side]?.unprinted) assert.equal(k[side], "", `${name} ${side} is not printed`);
      if (e[side]?.note) assert.notEqual(k[side], "", `${name} ${side} is printed`);
      if (e[side]?.menu) assert.ok(k.right && !k.left, `${name}: only the right label printed`);
    }
  }
});

test("a ghost takes an empty slot only", () => {
  assert.equal(ghostSlot(GX.right, "right", hiddenOf("48gx", "right")), "SERVER");
  assert.equal(ghostSlot(GX.down, "right", hiddenOf("48gx", "down")), "REVIEW");
  assert.equal(ghostSlot(GX.right, "left", hiddenOf("48gx", "right")), null);
  assert.equal(ghostSlot({ right: "SWAP" }, "right", { right: { unprinted: "x", ghost: "X" } }), null, "a taken slot");
  assert.equal(ghostSlot(GX.eex, "right", hiddenOf("48gx", "eex")), null, "a note has no ghost");
  assert.equal(ghostSlot(GX[7], "left", hiddenOf("48gx", "7")), null, "a menu has no ghost");
});

test("a ghost beside a printed label: the pair's sides, squeezed alike, or none", () => {
  const key = { x: 559, w: 64 };
  // SWAP 56 and SERVER 75 wide: 78 units of room, both squeezed by 78/131.
  const p = ghostPlacement({ ...key, side: "right", ghostWidth: 75, mateWidth: 56 });
  assert.deepEqual(p.ghost, { x: 559 + 64 + GHOST_PAD, anchor: "end", max: 75 * (78 / 131) });
  assert.deepEqual(p.mate, { x: 559 - GHOST_PAD, anchor: "start", max: 56 * (78 / 131) });
  // Short enough: not squeezed.
  assert.deepEqual(ghostPlacement({ ...key, side: "left", ghostWidth: 30, mateWidth: 30 }), {
    ghost: { x: 559 - GHOST_PAD, anchor: "start", max: null }, mate: { x: 559 + 64 + GHOST_PAD, anchor: "end", max: null },
  });
  // Too long: the tooltip only.
  assert.equal(ghostPlacement({ ...key, side: "right", ghostWidth: 120, mateWidth: 60 }), null);
  assert.ok(78 / 180 < GHOST_MIN_SCALE);
  // Alone: centred in a single label's room.
  assert.deepEqual(ghostPlacement({ ...key, side: "right", ghostWidth: 50 }), { ghost: { x: 591, anchor: "middle", max: null }, mate: null });
  assert.deepEqual(ghostPlacement({ ...key, side: "right", ghostWidth: 120 }), { ghost: { x: 591, anchor: "middle", max: 98 }, mate: null });
  // Not measured (the skin not on screen): not yet.
  assert.equal(ghostPlacement({ ...key, side: "right", ghostWidth: 0, mateWidth: 56 }), null);
  assert.equal(ghostPlacement({ ...key, side: "right", ghostWidth: 75, mateWidth: 0 }), null);
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
  assert.deepEqual(m.sent, ["down on", "down c", "up all"]);
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
