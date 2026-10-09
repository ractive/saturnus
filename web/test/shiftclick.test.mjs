// Ctrl+click and Option/Alt+click on a drawn key (web/shiftclick.js): which
// shift each modifier stands for on each model, which shift a click asks
// for, the tooltips, and the glow's state machine (its delay, chords,
// blur, a missed keyup, the lone Alt). `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { ContextClicks, GLOW_DELAY, ModifierGlow, awaitsContextMenu, clickHints, glowSide, modifierOf, shiftFor, shiftKeyFor } from "../shiftclick.js";

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

/**
 * A key's events as the component handles them (sat-calculator.js): a
 * `pointerdown` presses (a modifier-click with its shift), a
 * `contextmenu` the `ContextClicks` lets through presses as a tap. The
 * presses made, as "shift key" or "key".
 */
function keySim() {
  const clicks = new ContextClicks();
  const presses = [];
  const press = (key, mod) => presses.push(mod ? `${mod} ${key}` : key);
  return {
    presses,
    pointerdown: (key, e) => {
      clicks.pointerdown(key, { pointerType: "mouse", ...e });
      press(key, modifierOf(e));
    },
    contextmenu: (key, e) => {
      const mod = clicks.contextmenu(key, e);
      if (mod) press(key, mod);
    },
  };
}
const NONE = { ctrlKey: false, altKey: false, metaKey: false, shiftKey: false };
const CTRL = { ...NONE, ctrlKey: true };

test("a contextmenu that follows a pointerdown is that press's; Firefox's alone presses shifted", () => {
  // Chrome on macOS: pointerdown with Ctrl (button 2 or 0), then contextmenu.
  let k = keySim();
  k.pointerdown("nxt", { ...CTRL, button: 2 });
  k.contextmenu("nxt", { ...CTRL, button: 2 });
  assert.deepEqual(k.presses, ["ctrl nxt"], "Chrome: once");
  // Firefox on macOS: contextmenu only.
  k = keySim();
  k.contextmenu("nxt", { ...CTRL, button: 2 });
  assert.deepEqual(k.presses, ["ctrl nxt"], "Firefox: once, shifted");
  // A plain click, then at once a Firefox Ctrl+click on the same key: both.
  k = keySim();
  k.pointerdown("nxt", { ...NONE, button: 0 });
  k.contextmenu("nxt", { ...CTRL, button: 2 });
  assert.deepEqual(k.presses, ["nxt", "ctrl nxt"], "a plain click leaves no token");
  // Windows: Ctrl+right-click held 2 s, contextmenu after mouseup: once.
  k = keySim();
  k.pointerdown("nxt", { ...CTRL, button: 2 });
  k.contextmenu("nxt", { ...CTRL, button: 2 });
  assert.deepEqual(k.presses, ["ctrl nxt"], "Windows: once, however long held");
  // A token whose contextmenu never came (Chrome elsewhere, Ctrl+left) is
  // dropped by the next pointerdown, and taken by the next contextmenu.
  k = keySim();
  k.pointerdown("nxt", { ...CTRL, button: 0 });
  k.pointerdown("nxt", { ...NONE, button: 0 });
  k.contextmenu("nxt", { ...CTRL, button: 2 });
  assert.deepEqual(k.presses, ["ctrl nxt", "nxt", "ctrl nxt"]);
  // A plain right-click presses through its pointerdown only; Option+click sets no token.
  k = keySim();
  k.pointerdown("nxt", { ...NONE, button: 2 });
  k.contextmenu("nxt", { ...NONE, button: 2 });
  k.pointerdown("sin", { ...NONE, altKey: true, button: 0 });
  k.contextmenu("sin", { ...CTRL, button: 2 });
  assert.deepEqual(k.presses, ["nxt", "alt sin", "ctrl sin"]);
  // Tokens are per key; Cmd+Ctrl and no modifier press nothing from a contextmenu alone.
  k = keySim();
  k.pointerdown("nxt", { ...CTRL, button: 2 });
  k.contextmenu("sin", { ...CTRL, button: 2 });
  k.contextmenu("enter", { ...CTRL, metaKey: true, button: 2 });
  k.contextmenu("enter", { ...NONE, button: 2 });
  assert.deepEqual(k.presses, ["ctrl nxt", "ctrl sin"]);
  assert.equal(awaitsContextMenu({ pointerType: "touch", button: 0, ctrlKey: true }), false);
});
