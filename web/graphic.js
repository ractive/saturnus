// A GROB's picture (the host's `graphic` on an object: `{width, height,
// rows}`, rows packed one bit a pixel as hex, leftmost pixel in the top
// bit, 1 dark; saturnus_objects::graphic, wiki: protocols/hp-object-format)
// as pixels and as RGBA, in the screen images' colours. Pure (no DOM):
// web/test/graphic.test.mjs.

/** The picture as one byte a pixel, row by row, 1 dark. */
export function graphicBits({ width, height, rows }) {
  const bits = new Uint8Array(width * height);
  const rowBytes = Math.ceil(width / 8);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const at = 2 * (y * rowBytes + (x >> 3));
      const byte = parseInt(rows.slice(at, at + 2), 16);
      if (byte & (0x80 >> (x & 7))) bits[y * width + x] = 1;
    }
  }
  return bits;
}

/**
 * The picture as RGBA, `scale` image pixels a picture pixel (nearest
 * neighbour), `colors` `{on, off}` as the screen images have them
 * (`screenColors`): `{width, height, data}` as an `ImageData` takes.
 */
export function graphicRgba(g, colors, scale = 1, bits = graphicBits(g)) {
  const W = g.width * scale;
  const H = g.height * scale;
  const data = new Uint8ClampedArray(W * H * 4);
  for (let y = 0; y < H; y++) {
    const row = Math.floor(y / scale) * g.width;
    for (let x = 0; x < W; x++) {
      const c = bits[row + Math.floor(x / scale)] ? colors.on : colors.off;
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
 * The whole-number scale that shows a `width` by `height` picture
 * within `maxWidth` by `maxHeight`: up to `most` (a screen-sized GROB
 * at 2 to 4 times), at least 1 (a large one is shown at its size and
 * scrolls).
 */
export function graphicScale(width, height, maxWidth, maxHeight = 320, most = 4) {
  const fit = Math.floor(Math.min(maxWidth / Math.max(width, 1), maxHeight / Math.max(height, 1)));
  return Math.max(1, Math.min(most, fit));
}

/** "131×64": a picture's size for its meta line. */
export function graphicSize(g) {
  return `${g.width}×${g.height}`;
}

/**
 * "Graphic 131 × 64": a graphic as the calculator writes it on its stack
 * and in a list (spaced, as the ROM's display of a GROB has it).
 */
export function graphicText(g) {
  return `Graphic ${g.width} × ${g.height}`;
}

/** The file name of a picture saved from variable or level `name`: `PIC.png`, `level-1.png`. */
export function graphicFileName(name) {
  const base = String(name ?? "").replace(/[\\/:*?"<>|\s]+/g, "-").replace(/^-+|-+$/g, "") || "graphic";
  return `${base}.png`;
}
