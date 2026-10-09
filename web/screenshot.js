// The calculator's screen as an image: the pixels and the annunciator
// strip above them, as the page draws the LCD, scaled up with hard pixel
// edges. Two looks: the LCD's colours at its contrast ("lcd"), or pure
// black on white ("bw"). Copied to the clipboard as a PNG, or saved as a
// file where that is not possible. Pure (no DOM) but for `pngBlob`:
// web/test/screenshot.test.mjs.

import { contrastDarkness, offTint } from "./contrast.js";

/** Rows of the annunciator strip above the pixels (as sat-calculator.js draws it). */
export const ANN_H = 8;
/** The drawn LCD's background and ink, the same in both page themes. */
export const LCD_BG = [183, 194, 162];
export const LCD_INK = [16, 20, 12];
/** The looks of an image, and the default. */
export const LOOKS = ["lcd", "bw"];
/** How much the screen is scaled up. */
export const SCALE = 4;

/**
 * The annunciators' marks, in pixels (`#` on), in strip order: the 48,
 * 49G, 38G, 39G and 40G share six, the 42S has its own seven.
 */
const MARKS = {
  leftshift: [".#.....", "##.....", "#######", ".#....#", "......#", "......#", "......#"],
  rightshift: [".....#.", ".....##", "#######", "#....#.", "#......", "#......", "#......"],
  alpha: [".##..#", "#..#.#", "#...#.", "#..#.#", ".##..#"],
  alert: [".#.....#.", "#.......#", "#..###..#", "#..###..#", "#..###..#", "#.......#", ".#.....#."],
  busy: ["#####", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#####"],
  transmit: [".....#.", "#######", ".....#.", ".#.....", "#######", ".#....."],
  updown: ["..#...#####", ".###...###.", "#####...#.."],
  shift42: ["...#...", "..#.#..", ".#...#.", "###.###", "..#.#..", "..#.#..", "..###.."],
};

/** Letters for the 42S's word annunciators, 3 by 5. */
const LETTERS = {
  A: [".#.", "#.#", "###", "#.#", "#.#"],
  B: ["##.", "#.#", "##.", "#.#", "##."],
  D: ["##.", "#.#", "#.#", "#.#", "##."],
  G: [".##", "#..", "#.#", "#.#", ".##"],
  R: ["##.", "#.#", "##.", "#.#", "#.#"],
  T: ["###", ".#.", ".#.", ".#.", ".#."],
};

/** `word` in the 3 by 5 letters, one column apart. */
function word(text) {
  return [0, 1, 2, 3, 4].map((r) => [...text].map((c) => LETTERS[c][r]).join("."));
}

/** The strip of the 48, 49G, 38G, 39G and 40G: annunciator name and mark. */
const STRIP = [
  ["leftshift", MARKS.leftshift],
  ["rightshift", MARKS.rightshift],
  ["alpha", MARKS.alpha],
  ["alert", MARKS.alert],
  ["busy", MARKS.busy],
  ["transmit", MARKS.transmit],
];

/** The 42S's strip, in the order of its LCD. */
const STRIP_42S = [
  ["updown", MARKS.updown],
  ["leftshift", MARKS.shift42],
  ["transmit", MARKS.alert],
  ["busy", MARKS.busy],
  ["battery", word("BAT")],
  ["g", word("G")],
  ["rad", word("RAD")],
];

function mix(a, b, t) {
  return [0, 1, 2].map((i) => Math.round(a[i] + (b[i] - a[i]) * t));
}

/**
 * The colours of a pixel on and off: the LCD's at the frame's contrast
 * (as sat-calculator.js draws them), or black and white.
 */
export function screenColors(frame, look) {
  if (look === "bw") return { on: [0, 0, 0], off: [255, 255, 255] };
  const d = frame.contrastRange ? contrastDarkness(frame.contrast, frame.contrastRange, frame.contrastDefault) : 1;
  return { on: mix(LCD_BG, LCD_INK, d), off: mix(LCD_BG, LCD_INK, offTint(d)) };
}

/**
 * The screen at one image pixel per LCD pixel, `1` on: the annunciator
 * strip (`ANN_H` rows, each lit mark centred in its slot as the page
 * draws it) above the frame's rows. `pixels` is the frame decoded (one
 * byte per pixel, `decodeFrame`), `model` the running one (the 42S has
 * its own strip).
 */
export function screenBits(frame, pixels, model) {
  const w = frame.width;
  const h = ANN_H + frame.height;
  const bits = new Uint8Array(w * h);
  bits.set(pixels.subarray(0, w * frame.height), ANN_H * w);
  const strip = model === "42s" ? STRIP_42S : STRIP;
  const slot = w / strip.length;
  const ann = frame.annunciators ?? {};
  strip.forEach(([name, mark], i) => {
    if (!ann[name]) return;
    const mw = mark[0].length;
    const x0 = Math.round(slot * (i + 0.5) - mw / 2);
    const y0 = Math.floor((ANN_H - mark.length) / 2);
    mark.forEach((row, y) => {
      for (let x = 0; x < mw; x++) {
        if (row[x] === "#" && x0 + x >= 0 && x0 + x < w) bits[(y0 + y) * w + x0 + x] = 1;
      }
    });
  });
  return { width: w, height: h, bits };
}

/**
 * The screen as RGBA, `scale` image pixels per LCD pixel (nearest
 * neighbour), in `look`: `{width, height, data}` as an `ImageData` takes.
 */
export function screenRgba(frame, pixels, model, look = "lcd", scale = SCALE) {
  const { width: w, height: h, bits } = screenBits(frame, pixels, model);
  const { on, off } = screenColors(frame, look);
  const W = w * scale;
  const H = h * scale;
  const data = new Uint8ClampedArray(W * H * 4);
  for (let y = 0; y < H; y++) {
    const row = Math.floor(y / scale) * w;
    for (let x = 0; x < W; x++) {
      const c = bits[row + Math.floor(x / scale)] ? on : off;
      const i = (y * W + x) * 4;
      data[i] = c[0];
      data[i + 1] = c[1];
      data[i + 2] = c[2];
      data[i + 3] = 255;
    }
  }
  return { width: W, height: H, data };
}

/** The file name of a screen image: `48gx-2026-10-09-0142.png` (local time). */
export function screenFileName(model, date = new Date()) {
  const p = (n) => String(n).padStart(2, "0");
  return `${model}-${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())}-${p(date.getHours())}${p(date.getMinutes())}.png`;
}

/** The look a preference names, the default for anything else. */
export function lookOf(pref) {
  return LOOKS.includes(pref) ? pref : "lcd";
}

/**
 * Put a PNG on the clipboard. `blob` is a promise of it, so the item is
 * made at once, inside the click or key that asked (Safari needs that);
 * resolves to whether it worked. False where images cannot be copied
 * (no `ClipboardItem`, no `image/png` support) or the write failed.
 */
export async function copyPng(blob, { clipboard = globalThis.navigator?.clipboard, Item = globalThis.ClipboardItem } = {}) {
  if (!clipboard?.write || typeof Item !== "function") return false;
  if (typeof Item.supports === "function" && !Item.supports("image/png")) return false;
  try {
    await clipboard.write([new Item({ "image/png": blob })]);
    return true;
  } catch {
    return false;
  }
}

/** The RGBA image as a PNG blob, through a canvas (the browser's encoder). */
export function pngBlob({ width, height, data }) {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  canvas.getContext("2d").putImageData(new ImageData(data, width, height), 0, 0);
  return new Promise((resolve, reject) => {
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("the image could not be made"))), "image/png");
  });
}
