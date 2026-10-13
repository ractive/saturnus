// The calculator's screen as an image: the pixels and the annunciator
// strip above them, as the page draws the LCD, scaled up with hard pixel
// edges. Two looks: the LCD's colours at its contrast ("lcd"), or pure
// black on white ("bw"). Copied to the clipboard as a PNG, or saved as a
// file where that is not possible (web/shared/png.js). Pure (no DOM):
// web/test/screenshot.test.mjs.

import { contrastDarkness, offTint } from "./contrast.js";
import "./shared/models.js";

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
 * 49G, 38G, 39G and 40G share six, the 42S has its own seven, the 27S,
 * 17BII and 17B six of the 42S's, the 28S seven of its own, the 19BII
 * six of the 28S's.
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
  circle: [".###.", "#...#", "#...#", "#...#", ".###."],
};

/** Letters for the 42S's word annunciators, 3 by 5. */
const LETTERS = {
  A: [".#.", "#.#", "###", "#.#", "#.#"],
  B: ["##.", "#.#", "##.", "#.#", "##."],
  D: ["##.", "#.#", "#.#", "#.#", "##."],
  G: [".##", "#..", "#.#", "#.#", ".##"],
  P: ["##.", "#.#", "##.", "#..", "#.."],
  R: ["##.", "#.#", "##.", "#.#", "#.#"],
  T: ["###", ".#.", ".#.", ".#.", ".#."],
  "2": ["##.", "..#", ".#.", "#..", "###"],
  "π": ["###", "#.#", "#.#", "#.#", "#.#"],
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

/**
 * The 27S's, 17BII's and 17B's strip at the 42S's positions (inferred):
 * print as "PRT", the alarm ((•)) at the 42S's busy position, no G.
 */
const STRIP_PIONEER = [
  ["updown", MARKS.updown],
  ["leftshift", MARKS.shift42],
  ["transmit", word("PRT")],
  ["busy", MARKS.alert],
  ["battery", word("BAT")],
  ["rad", word("RAD")],
];

/**
 * The 28S's strip, left to right as above its display (wiki:
 * hardware/hp28s "Annunciators"): a suspended program (a small circle),
 * the bent shift arrow, α, busy ((•)), battery, RAD as 2π, the printer.
 */
const STRIP_28S = [
  ["suspended", MARKS.circle],
  ["leftshift", MARKS.rightshift],
  ["alpha", MARKS.alpha],
  ["busy", MARKS.alert],
  ["battery", word("BAT")],
  ["rad", word("2π")],
  ["transmit", word("PRT")],
];

/**
 * The 19BII's strip, left to right as above its display (wiki:
 * hardware/hp19bii "Annunciators", OM Figure 1-1): the shift, α, the
 * appointment alarm ((•)), battery, RAD as 2π, the printer.
 */
const STRIP_19BII = [
  ["leftshift", MARKS.rightshift],
  ["alpha", MARKS.alpha],
  ["busy", MARKS.alert],
  ["battery", word("BAT")],
  ["rad", word("2π")],
  ["transmit", word("PRT")],
];

/** The strip of `model`. */
function stripOf(model) {
  if (model === "42s") return STRIP_42S;
  if (model === "28s") return STRIP_28S;
  if (model === "19bii") return STRIP_19BII;
  return globalThis.SATURNUS_PIONEERS.includes(model) ? STRIP_PIONEER : STRIP;
}

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
  const strip = stripOf(model);
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

/**
 * Whether the store's state has a screen to take: a ROM runs for the
 * model shown and a frame has come. The panel's buttons follow it.
 */
export function hasScreen(state) {
  return Boolean(state.booted && state.booted === state.model && state.frame);
}

/** What the page says when a screen image is asked for without one. */
export const NO_SCREEN = "No screen to take yet.";

/** The file name of a screen image: `48gx-2026-10-09-0142.png` (local time). */
export function screenFileName(model, date = new Date()) {
  const p = (n) => String(n).padStart(2, "0");
  return `${model}-${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())}-${p(date.getHours())}${p(date.getMinutes())}.png`;
}

/** The look a preference names, the default for anything else. */
export function lookOf(pref) {
  return LOOKS.includes(pref) ? pref : "lcd";
}
