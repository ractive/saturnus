// Edge to edge (web/edge.js): the face's core fills the room's width,
// stays whole and in the room, and leaves the overlay buttons their band.
// `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { BUTTON_BAND, edgeLayout } from "../edge.js";

/** A tall face (a 48-like skin) and its core: the window and the keys. */
const FACE = [20, 20, 460, 1060];
const CORE = [30, 180, 440, 890];

/** The core's box in the room, in CSS pixels. */
function coreInRoom({ f, view, x, y }) {
  const [vx, vy] = view;
  const [cx, cy, cw, ch] = CORE;
  return { l: x + (cx - vx) * f, t: y + (cy - vy) * f, r: x + (cx + cw - vx) * f, b: y + (cy + ch - vy) * f };
}

test("upright, the core takes the width below the buttons' band", () => {
  for (const [w, h] of [[360, 780], [390, 844], [430, 932], [412, 1000]]) {
    const l = edgeLayout(FACE, CORE, { w, h });
    const c = coreInRoom(l);
    assert.ok(c.l >= -1e-9 && c.r <= w + 1e-9 && c.b <= h + 1e-9, `${w}x${h}: the core is in the room`);
    assert.ok(c.t >= BUTTON_BAND - 1e-9, `${w}x${h}: the core starts below the band`);
    // Either the width or the height left by the band binds.
    const full = Math.abs(c.r - c.l - w) < 1e-6 || Math.abs(c.b - c.t - (h - BUTTON_BAND)) < 1e-6;
    assert.ok(full, `${w}x${h}: the core is as large as the room allows`);
    // The view stays on the face.
    const [vx, vy, vw, vh] = l.view;
    assert.ok(vx >= FACE[0] && vy >= FACE[1] && vx + vw <= FACE[0] + FACE[2] + 1e-9 && vy + vh <= FACE[1] + FACE[3] + 1e-9);
  }
});

test("a short room crops the face's top, not the keys", () => {
  const l = edgeLayout(FACE, CORE, { w: 390, h: 900 });
  const [, vy, , vh] = l.view;
  assert.ok(vy > FACE[1], "the lettering above the core is cropped");
  assert.equal(vy + vh, FACE[1] + FACE[3], "the face's bottom stays");
});

test("a tall room shows the whole face, centred, below the band", () => {
  const l = edgeLayout(FACE, CORE, { w: 390, h: 1400 });
  assert.equal(l.view[1], FACE[1]);
  assert.equal(l.view[3], FACE[3]);
  assert.ok(Math.abs(l.y - (1400 - FACE[3] * l.f) / 2) < 1e-9 && l.y >= BUTTON_BAND);
});

test("on its side, the buttons sit beside the face and the core takes the height", () => {
  const w = 844;
  const h = 390;
  const l = edgeLayout(FACE, CORE, { w, h });
  const c = coreInRoom(l);
  assert.ok(Math.abs(c.b - c.t - h) < 1e-6, "the core takes the height");
  assert.ok(l.x >= BUTTON_BAND && l.x + l.view[2] * l.f <= w - BUTTON_BAND + 1e-9, "the face clears the buttons");
});

test("the snap only shrinks the scale", () => {
  const plain = edgeLayout(FACE, CORE, { w: 390, h: 844 });
  const snapped = edgeLayout(FACE, CORE, { w: 390, h: 844 }, (f) => f * 0.98);
  assert.ok(Math.abs(snapped.f - plain.f * 0.98) < 1e-12);
  const c = coreInRoom(snapped);
  assert.ok(c.l >= 0 && c.r <= 390 && c.t >= BUTTON_BAND && c.b <= 844);
});
