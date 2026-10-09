// Ctrl+click and Option/Alt+click on a drawn key (web/shiftclick.js): which
// shift each modifier stands for on each model, which shift a click asks
// for, the tooltips, and the glow's state machine (its delay, chords,
// blur, a missed keyup, the lone Alt). `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { CONTEXT_SAME_PRESS_MS, GLOW_DELAY, ModifierGlow, clickHints, contextClickModifier, glowSide, modifierOf, shiftFor, shiftKeyFor } from "../shiftclick.js";

const TWO = new Set(["leftshift", "rightshift", "alpha", "nxt", "enter"]);
const ONE = new Set(["shift", "alpha", "enter"]);
const ev = (mods = {}, key = "") => ({ key, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, repeat: false, ...mods });

test("exactly one of Ctrl and Alt is a modifier click", () => {
  assert.equal(modifierOf(ev({ ctrlKey: true })), "ctrl");
  assert.equal(modifierOf(ev({ altKey: true })), "alt");
  assert.equal(modifierOf(ev()), null);
  assert.equal(modifierOf(ev({ ctrlKey: true, altKey: true })), null, "AltGr on Windows");
  assert.equal(modifierOf(ev({ ctrlKey: true, shiftKey: true })), null);
  assert.equal(modifierOf(ev({ altKey: true, metaKey: true })), null);
});

test("Ctrl is the left shift, Alt the right; one shift takes both", () => {
  assert.equal(shiftKeyFor("ctrl", TWO), "leftshift");
  assert.equal(shiftKeyFor("alt", TWO), "rightshift");
  assert.equal(shiftKeyFor("ctrl", ONE), "shift");
  assert.equal(shiftKeyFor("alt", ONE), "shift");
  assert.equal(shiftKeyFor(null, TWO), null);
  assert.equal(shiftKeyFor("ctrl", new Set(["enter"])), null);
  assert.equal(glowSide("ctrl", TWO), "left");
  assert.equal(glowSide("alt", TWO), "right");
  assert.equal(glowSide("alt", ONE), "left", "one shift prints on the left");
  assert.equal(glowSide(null, ONE), null);
});

test("a click asks for its modifier's shift, but not on a shift key", () => {
  // Whether the shift is tapped (it may be on) is the host's, when the press plays.
  assert.equal(shiftFor("ctrl", "nxt", TWO), "leftshift");
  assert.equal(shiftFor("alt", "nxt", TWO), "rightshift");
  assert.equal(shiftFor(null, "nxt", TWO), null, "a plain click");
  assert.equal(shiftFor("ctrl", "leftshift", TWO), null, "a click on a shift key itself");
  assert.equal(shiftFor("alt", "shift", ONE), null);
  assert.equal(shiftFor("alt", "enter", ONE), "shift");
  assert.equal(shiftFor("ctrl", "enter", ONE), "shift");
});

test("the tooltips name the clicks and their labels", () => {
  assert.deepEqual(clickHints({ name: "nxt", left: "PREV", right: "MENU" }, TWO, true), ["Control-click: PREV", "Option-click: MENU"]);
  assert.deepEqual(clickHints({ name: "nxt", left: "PREV", right: "MENU" }, TWO, false), ["Ctrl+click: PREV", "Alt+click: MENU"]);
  assert.deepEqual(clickHints({ name: "nxt", right: "MENU" }, TWO, false), ["Alt+click: MENU"]);
  assert.deepEqual(clickHints({ name: "enter", left: "ANS" }, ONE, true), ["Control-click or option-click: ANS"]);
  assert.deepEqual(clickHints({ name: "leftshift" }, TWO, false), []);
  assert.deepEqual(clickHints({ name: "enter" }, ONE, false), []);
});

/** A glow with a hand-driven clock: `tick()` fires the pending timer. */
function glow() {
  let pending = null;
  const seen = [];
  const g = new ModifierGlow({
    setTimer: (f, ms) => { assert.equal(ms, GLOW_DELAY); pending = f; return 1; },
    clearTimer: () => { pending = null; },
    onChange: (lit) => seen.push(lit),
  });
  return { g, seen, tick: () => { const f = pending; pending = null; f?.(); }, waiting: () => pending !== null };
}

test("a modifier held alone lights after the delay and goes out on its release", () => {
  const { g, seen, tick } = glow();
  g.keydown(ev({ ctrlKey: true }, "Control"), true);
  assert.equal(g.lit, null, "not at once");
  tick();
  assert.equal(g.lit, "ctrl");
  g.keydown({ ...ev({ ctrlKey: true }, "Control"), repeat: true }, true);
  assert.equal(g.lit, "ctrl", "the key's repeat keeps it");
  g.keyup(ev({}, "Control"));
  assert.equal(g.lit, null);
  assert.deepEqual(seen, ["ctrl", null]);
});

test("a chord never lights, and another key puts the glow out", () => {
  const { g, tick, waiting } = glow();
  g.keydown(ev({ ctrlKey: true }, "Control"), true);
  g.keydown(ev({ ctrlKey: true }, "k"), true);
  assert.equal(waiting(), false, "the wait is cancelled");
  tick();
  assert.equal(g.lit, null);
  g.keyup(ev({}, "Control"));

  g.keydown(ev({ altKey: true }, "Alt"), true);
  tick();
  assert.equal(g.lit, "alt");
  g.keydown(ev({ altKey: true }, "l"), true);
  assert.equal(g.lit, null, "a chord after the light");
  // Ctrl joining Alt (AltGr) is no modifier click.
  g.keydown(ev({ ctrlKey: true }, "Control"), true);
  g.keydown(ev({ ctrlKey: true, altKey: true }, "AltGraph"), true);
  tick();
  assert.equal(g.lit, null);
});

test("blur, a hidden page and events without the modifier put it out", () => {
  const { g, tick } = glow();
  g.keydown(ev({ altKey: true }, "Alt"), true);
  tick();
  g.clear();
  assert.equal(g.lit, null, "the window lost the focus");

  g.keydown(ev({ altKey: true }, "Alt"), true);
  tick();
  g.sync(ev({ altKey: true }));
  assert.equal(g.lit, "alt", "a move with Alt still held");
  g.sync(ev());
  assert.equal(g.lit, null, "a move without it: its keyup was missed");

  g.keydown(ev({ ctrlKey: true }, "Control"), true);
  g.sync(ev());
  tick();
  assert.equal(g.lit, null, "out before it lit");
});

test("a modifier pressed while the calculator does not own the keys stays dark", () => {
  const { g, tick, waiting } = glow();
  g.keydown(ev({ ctrlKey: true }, "Control"), false);
  assert.equal(waiting(), false);
  tick();
  assert.equal(g.lit, null);
});

test("a lone Alt's release is reported; an Alt+click keeps it lone, a chord does not", () => {
  const { g } = glow();
  g.keydown(ev({ altKey: true }, "Alt"), true);
  assert.equal(g.keyup(ev({}, "Alt")), true, "lone");
  g.keydown(ev({ altKey: true }, "Alt"), true);
  g.keydown(ev({ altKey: true }, "x"), true);
  assert.equal(g.keyup(ev({}, "Alt")), false, "after Alt+X the release is left alone");
  g.keydown(ev({ altKey: true }, "Alt"), true);
  g.sync(ev({ altKey: true }));
  assert.equal(g.keyup(ev({}, "Alt")), true, "a click with Alt held keeps it lone");
  g.keydown(ev({ ctrlKey: true }, "Control"), true);
  assert.equal(g.keyup(ev({}, "Control")), false, "only Alt");
});

test("a Ctrl+click's contextmenu presses the key only where no pointerdown did (Firefox on macOS)", () => {
  const ctrl = { ctrlKey: true, altKey: false, metaKey: false, shiftKey: false, button: 2 };
  // Chrome: pointerdown with Ctrl pressed the key; its contextmenu comes while it is held.
  assert.equal(contextClickModifier(ctrl, { held: true, sinceMs: 0 }), null);
  // ... or just after the release.
  assert.equal(contextClickModifier(ctrl, { held: false, sinceMs: 120 }), null);
  // A pointerdown with button 2 (handled as a press) is the same.
  assert.equal(contextClickModifier(ctrl, { held: false, sinceMs: CONTEXT_SAME_PRESS_MS - 1 }), null);
  // Firefox on macOS: mousedown (button 2) and contextmenu, no pointerdown.
  assert.equal(contextClickModifier(ctrl, { held: false, sinceMs: null }), "ctrl");
  // ... also when the key was pressed long before.
  assert.equal(contextClickModifier(ctrl, { held: false, sinceMs: 5000 }), "ctrl");
  // A plain right-click (pressed by its pointerdown) and Cmd+Ctrl press nothing here.
  assert.equal(contextClickModifier({ ...ctrl, ctrlKey: false }, { held: false, sinceMs: null }), null);
  assert.equal(contextClickModifier({ ...ctrl, metaKey: true }, { held: false, sinceMs: null }), null);
  // Option with a secondary click, should a browser send it alone: the right shift.
  assert.equal(contextClickModifier({ ...ctrl, ctrlKey: false, altKey: true }, { held: false, sinceMs: null }), "alt");
});
