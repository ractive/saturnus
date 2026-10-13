// The clamshell's arrangements (web/fold.js): side by side on a wide
// stage, stacked on a tall one with the display half on top, the barrel
// across and the letter half's keyboard below; any other skin as it is.
// On a skin shaped as the 28S's (crates/saturnus-host/src/skins/hp28s.rs;
// the page test, hp28s-page.test.mjs, uses the real one). Pure:
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { STACK_GAP, aboveStack, foldGeometry, foldScale, halfAt, stacks } from "../fold.js";

/** Two halves of 836 x 1414 with a 14-unit gap, as the 28S's. */
const SKIN = {
  width: 1686,
  height: 1414,
  lcd: [1008, 351, 596, 174],
  glass: [1005, 337, 627, 203],
  logo: [1048, 76, 52, 52],
  fold: {
    letters: [0, 0, 836, 1414],
    display: [850, 0, 836, 1414],
    hinge: [776, 177, 52, 1146],
    stackedLetters: [29, 614, 715, 800],
    stackedDisplay: [950, 0, 728, 1414],
  },
};

test("stacked on a stage taller than wide, side by side otherwise", () => {
  assert.equal(stacks({ w: 1280, h: 860 }), false);
  assert.equal(stacks({ w: 900, h: 900 }), false, "square: side by side");
  assert.equal(stacks({ w: 374, h: 780 }), true);
  assert.equal(stacks({ w: 700, h: 1000 }), true, "a portrait tablet");
});

test("the 28S side by side: the drawing as it is", () => {
  const s = SKIN;
  const g = foldGeometry(s, false);
  assert.equal(g.stacked, false);
  assert.deepEqual([g.width, g.height, g.lcd, g.logo], [s.width, s.height, s.lcd, s.logo]);
  assert.deepEqual([g.letters, g.display], [[0, 0], [0, 0]]);
  assert.deepEqual(g.barrel, s.fold.hinge);
  assert.equal(halfAt(s.fold, 10, 10), "letters");
  assert.equal(halfAt(s.fold, s.fold.display[0] + 1, 10), "display");
  const room = { w: 1000, h: 800 };
  assert.equal(foldScale(s, g, room), Math.min(1000 / s.width, 800 / s.height));
});

test("the 28S stacked: display half on top, barrel across, the letter keys below", () => {
  const s = SKIN;
  const f = s.fold;
  const g = foldGeometry(s, true);
  assert.equal(g.stacked, true);
  const [, , dw, dh] = f.stackedDisplay;
  const [, , lw, lh] = f.stackedLetters;
  assert.equal(g.width, Math.max(dw, lw));
  assert.equal(g.height, dh + 2 * STACK_GAP + f.hinge[2] + lh);
  // The display half's shown part lands at the top, centred; the LCD moves with it.
  assert.equal(f.stackedDisplay[0] + g.display[0], (g.width - dw) / 2);
  assert.equal(f.stackedDisplay[1] + g.display[1], 0);
  assert.deepEqual(g.lcd, [s.lcd[0] + g.display[0], s.lcd[1] + g.display[1], s.lcd[2], s.lcd[3]]);
  // The barrel across, between the halves.
  const [bx, by, bw, bh] = g.barrel;
  assert.equal(by, dh + STACK_GAP);
  assert.equal(bh, f.hinge[2]);
  assert.ok(bx > 0 && bx + bw < g.width && bw > g.width / 2);
  // The letter half's shown part under the barrel, the card above it off.
  assert.equal(f.stackedLetters[1] + g.letters[1], by + bh + STACK_GAP);
  assert.ok(aboveStack(f, [42, 28, 704, f.stackedLetters[1] - 28]), "the card");
  assert.ok(!aboveStack(f, f.stackedLetters));
  assert.ok(!aboveStack(f, [f.display[0] + 10, 10, 10, 10]), "nothing of the display half");
  // Narrower than one half, so the keys get the phone's width.
  assert.ok(g.width < f.display[2]);
  // Width first on a phone; the display half alone fits a tall stage.
  assert.equal(foldScale(s, g, { w: 374, h: 780 }), 374 / g.width);
  assert.equal(foldScale(s, g, { w: 1000, h: 1200 }), 1200 / dh);
});

test("a skin without a fold is drawn as it is either way", () => {
  const s = { width: 674, height: 1235, lcd: [77, 258, 520, 95], logo: [74, 52, 52, 52] };
  const g = foldGeometry(s, true);
  assert.equal(g.stacked, false);
  assert.deepEqual([g.width, g.height, g.lcd, g.glass, g.barrel], [s.width, s.height, s.lcd, null, null]);
});
