// A GROB's picture in the page (web/graphic.js). `node --test web/test/`
// (just web-test).
import assert from "node:assert/strict";
import { test } from "node:test";
import { graphicBits, graphicFileName, graphicRgba, graphicScale, graphicSize, graphicText } from "../graphic.js";

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
