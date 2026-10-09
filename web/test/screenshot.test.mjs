// The screen as an image (web/screenshot.js): the annunciator strip above
// the pixels, both looks, the 4x scale with hard edges, the 42S's own
// strip and rows, the file name, and the clipboard write with its
// fallback. `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { contrastDarkness, offTint } from "../contrast.js";
import { ANN_H, LCD_BG, LCD_INK, copyPng, lookOf, screenBits, screenColors, screenFileName, screenRgba } from "../screenshot.js";

const OFF = { leftshift: false, rightshift: false, alpha: false, alert: false, busy: false, transmit: false, updown: false, battery: false, g: false, rad: false };
/** A frame of `rows` rows with the pixels `on` ([x, y]) dark. */
function frame(rows, on = [], ann = {}) {
  const f = { width: 131, height: rows, annunciators: { ...OFF, ...ann }, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 };
  const pixels = new Uint8Array(131 * rows);
  for (const [x, y] of on) pixels[y * 131 + x] = 1;
  return { f, pixels };
}
const at = (img, x, y) => [...img.data.subarray((y * img.width + x) * 4, (y * img.width + x) * 4 + 4)];
const lit = (bits, w, x0, y0, x1, y1) => {
  let n = 0;
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) n += bits[y * w + x];
  return n;
};

test("the pixels sit under the annunciator strip; a lit annunciator is drawn in its slot", () => {
  const { f, pixels } = frame(64, [[0, 0], [130, 63]]);
  const plain = screenBits(f, pixels, "48gx");
  assert.equal(plain.width, 131);
  assert.equal(plain.height, ANN_H + 64);
  assert.equal(plain.bits[ANN_H * 131], 1, "the first pixel, below the strip");
  assert.equal(plain.bits[(ANN_H + 63) * 131 + 130], 1, "the last pixel");
  assert.equal(lit(plain.bits, 131, 0, 0, 131, ANN_H), 0, "no annunciator on");

  const shifted = screenBits({ ...f, annunciators: { ...OFF, leftshift: true, busy: true } }, pixels, "48gx");
  const slot = 131 / 6;
  assert.ok(lit(shifted.bits, 131, 0, 0, Math.floor(slot), ANN_H) > 5, "left shift in the first slot");
  assert.equal(lit(shifted.bits, 131, Math.ceil(slot), 0, Math.floor(4 * slot), ANN_H), 0, "nothing in the slots of the annunciators off");
  assert.ok(lit(shifted.bits, 131, Math.ceil(4 * slot), 0, Math.floor(5 * slot), ANN_H) > 5, "busy in the fifth slot");
  // The left shift's mark: its stem on the right, its head top left.
  const x0 = Math.round(slot / 2 - 7 / 2);
  assert.equal(shifted.bits[2 * 131 + x0], 1);
  assert.equal(shifted.bits[6 * 131 + x0 + 6], 1);
});

test("the 42S: 16 rows and its own seven annunciators", () => {
  const { f, pixels } = frame(16, [[5, 15]], { rad: true });
  const b = screenBits(f, pixels, "42s");
  assert.equal(b.height, ANN_H + 16);
  assert.equal(b.bits[(ANN_H + 15) * 131 + 5], 1);
  const slot = 131 / 7;
  assert.ok(lit(b.bits, 131, Math.ceil(6 * slot), 0, 131, ANN_H) > 10, "RAD in the last slot");
  assert.equal(lit(b.bits, 131, 0, 0, Math.floor(6 * slot), ANN_H), 0);
  // The same flags read with the 48's strip: no slot for RAD there.
  assert.equal(lit(screenBits(f, pixels, "48sx").bits, 131, 0, 0, 131, ANN_H), 0);
});

test("4x with hard edges, in the LCD's colours or black on white", () => {
  const { f, pixels } = frame(64, [[1, 0]], { alpha: true });
  const bw = screenRgba(f, pixels, "49g", "bw");
  assert.equal(bw.width, 524);
  assert.equal(bw.height, 288);
  assert.equal(bw.data.length, 524 * 288 * 4);
  // LCD pixel (1, 0) is image pixels 4..7 by 32..35, all black; its neighbours white.
  for (const [x, y] of [[4, 32], [7, 35], [5, 33]]) assert.deepEqual(at(bw, x, y), [0, 0, 0, 255]);
  for (const [x, y] of [[3, 32], [8, 32], [4, 31], [4, 36]]) assert.deepEqual(at(bw, x, y), [255, 255, 255, 255]);

  const lcd = screenRgba(f, pixels, "49g", "lcd");
  const d = contrastDarkness(12, [0, 31], 12);
  const mix = (t) => [0, 1, 2].map((i) => Math.round(LCD_BG[i] + (LCD_INK[i] - LCD_BG[i]) * t));
  assert.deepEqual(at(lcd, 4, 32), [...mix(d), 255], "on: the ink at the contrast");
  assert.deepEqual(at(lcd, 0, 40), [...mix(offTint(d)), 255], "off: the faint tint");
  assert.deepEqual(screenColors(f, "lcd"), { on: mix(d), off: mix(offTint(d)) });
  // A darker contrast, darker pixels; black on white ignores it.
  const dark = { ...f, contrast: 25 };
  assert.ok(screenColors(dark, "lcd").on[0] < screenColors(f, "lcd").on[0]);
  assert.deepEqual(screenColors(dark, "bw"), { on: [0, 0, 0], off: [255, 255, 255] });
  // The annunciators in the image too.
  let inStrip = 0;
  for (let y = 0; y < ANN_H * 4; y++) for (let x = 0; x < 524; x++) inStrip += at(bw, x, y)[0] === 0 ? 1 : 0;
  assert.ok(inStrip > 0, "alpha drawn");
});

test("the file name: model, date and time", () => {
  assert.equal(screenFileName("48gx", new Date(2026, 9, 9, 1, 42)), "48gx-2026-10-09-0142.png");
  assert.equal(screenFileName("42s", new Date(2026, 0, 2, 13, 5)), "42s-2026-01-02-1305.png");
});

test("the look preference", () => {
  assert.equal(lookOf("bw"), "bw");
  assert.equal(lookOf("lcd"), "lcd");
  assert.equal(lookOf(null), "lcd");
  assert.equal(lookOf("sepia"), "lcd");
});

test("copying: the item is made at once; false where images cannot be copied", async () => {
  const made = [];
  class Item {
    constructor(parts) { made.push(parts); }
    static supports(type) { return type === "image/png"; }
  }
  const written = [];
  const clipboard = { write: async (items) => { written.push(items); } };
  const blob = Promise.resolve("png");
  const p = copyPng(blob, { clipboard, Item });
  assert.equal(made.length, 1, "made before the first await (the click's turn)");
  assert.equal(made[0]["image/png"], blob, "the promise itself, as Safari wants");
  assert.equal(await p, true);
  assert.equal(written.length, 1);

  assert.equal(await copyPng(blob, { clipboard, Item: undefined }), false, "no ClipboardItem");
  assert.equal(await copyPng(blob, { clipboard: {}, Item }), false, "no clipboard.write");
  class NoPng extends Item { static supports() { return false; } }
  assert.equal(await copyPng(blob, { clipboard, Item: NoPng }), false, "no image/png");
  const refusing = { write: async () => { throw new Error("NotAllowedError"); } };
  assert.equal(await copyPng(blob, { clipboard: refusing, Item }), false, "the write refused");
});
