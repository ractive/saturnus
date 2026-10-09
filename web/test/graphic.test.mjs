// A GROB's picture in the page (web/graphic.js). `node --test web/test/`
// (just web-test).
import assert from "node:assert/strict";
import { test } from "node:test";
import { exportScale, graphicBits, graphicDrawable, graphicFileName, graphicRgba, graphicScale, graphicSize, graphicText, graphicThumb } from "../graphic.js";

test("a picture's rows unpack leftmost pixel first, odd widths included", () => {
  assert.deepEqual([...graphicBits({ width: 1, height: 1, rows: "80" })], [1]);
  // 5 wide (one byte a row): #.#.# / .#.#. / ....#
  assert.deepEqual([...graphicBits({ width: 5, height: 3, rows: "A85008" })], [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1]);
  // 9 wide: two bytes a row, the ninth pixel in the second byte's top bit.
  assert.deepEqual([...graphicBits({ width: 9, height: 2, rows: "00808000" })], [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
  assert.equal(graphicSize({ width: 131, height: 64 }), "131×64");
  assert.equal(graphicText({ width: 131, height: 64 }), "Graphic 131 × 64");
});

test("a picture is drawn in the given colours, scaled with hard edges", () => {
  const on = [16, 20, 12];
  const off = [183, 194, 162];
  const img = graphicRgba({ width: 2, height: 1, rows: "80" }, { on, off }, 2);
  assert.equal(img.width, 4);
  assert.equal(img.height, 2);
  const px = (x, y) => [...img.data.slice((y * 4 + x) * 4, (y * 4 + x) * 4 + 4)];
  assert.deepEqual([px(0, 0), px(1, 1), px(2, 0), px(3, 1)], [[...on, 255], [...on, 255], [...off, 255], [...off, 255]]);
});

test("the preview's scale: whole, 1 to 4, fitting the room", () => {
  assert.equal(graphicScale(131, 64, 400), 3);
  assert.equal(graphicScale(131, 64, 600), 4);
  assert.equal(graphicScale(131, 64, 200), 1);
  assert.equal(graphicScale(800, 600, 300), 1, "a large one at its size");
  assert.equal(graphicScale(8, 8, 300, 320), 4);
  assert.equal(graphicScale(131, 64, 1000, 100), 1, "the height counts too");
  assert.equal(graphicFileName("PIC"), "PIC.png");
  assert.equal(graphicFileName("Level 1"), "Level-1.png");
  assert.equal(graphicFileName(""), "graphic.png");
});

test("an exported image stays within about 16 Mpixels: the scale is lowered, never below 1", () => {
  assert.equal(exportScale(131, 64, 4), 4, "the screen's size at 4 times");
  assert.equal(exportScale(1024, 1024, 4), 3, "3072×3072 fits, 4096×4096 does not");
  assert.equal(exportScale(2048, 2048, 4), 1);
  assert.equal(exportScale(4000, 4000, 4), 1, "at least once");
  for (const [w, h] of [[131, 64], [1000, 1000], [2048, 1500], [300, 4000]]) {
    const s = exportScale(w, h, 4);
    assert.ok(s === 1 || w * s * h * s <= 16_000_000, `${w}×${h} at ${s}`);
  }
});

test("a thumbnail shrinks to its height and keeps one-pixel lines; no pixels, nothing to draw", () => {
  // 8×8 with a diagonal, to 2 high: each 4×4 block with a dark pixel is dark.
  const n = 8;
  const bits = new Uint8Array(n * n);
  for (let i = 0; i < n; i++) bits[i * n + i] = 1;
  const t = graphicThumb({ width: n, height: n }, 2, bits);
  assert.deepEqual([t.width, t.height, [...t.bits]], [2, 2, [1, 0, 0, 1]]);
  // Already small enough: as it is.
  const small = graphicThumb({ width: 5, height: 3, rows: "A85008" }, 20);
  assert.deepEqual([small.width, small.height], [5, 3]);
  // Wide and flat: at least one pixel each way.
  const flat = graphicThumb({ width: 100, height: 40 }, 1, new Uint8Array(4000));
  assert.deepEqual([flat.width, flat.height], [3, 1]);
  assert.equal(graphicDrawable({ width: 0, height: 0, rows: "" }), false);
  assert.equal(graphicDrawable({ width: 0, height: 5, rows: "" }), false);
  assert.equal(graphicDrawable({ width: 1, height: 1, rows: "80" }), true);
  assert.equal(graphicDrawable(null), false);
});
