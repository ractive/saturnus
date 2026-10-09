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

/** Whether a picture has pixels to draw (a GROB can be 0 wide or high). */
export function graphicDrawable(g) {
  return Boolean(g) && g.width > 0 && g.height > 0;
}

/**
 * The scale a picture is exported at: `want` (the screen images'), or
 * less so that the image stays within `maxPixels` (about 16 Mpixels:
 * Safari draws no larger canvas, and 4 bytes a pixel is 64 MB); at
 * least 1.
 */
export function exportScale(width, height, want, maxPixels = 16_000_000) {
  let s = Math.max(1, Math.floor(want));
  while (s > 1 && width * s * height * s > maxPixels) s--;
  return s;
}

/**
 * A picture shrunk to `maxHeight` rows for a thumbnail (its own size if it
 * is that small): each thumbnail pixel dark if any pixel it covers is, so
 * a line one pixel wide does not vanish. `{width, height, bits}`, at
 * least 1 by 1.
 */
export function graphicThumb(g, maxHeight, bits = graphicBits(g)) {
  if (g.height <= maxHeight) return { width: g.width, height: g.height, bits };
  const f = g.height / maxHeight;
  const height = Math.max(1, maxHeight);
  const width = Math.max(1, Math.round(g.width / f));
  const out = new Uint8Array(width * height);
  for (let y = 0; y < g.height; y++) {
    const ty = Math.min(height - 1, Math.floor(y / f));
    for (let x = 0; x < g.width; x++) {
      if (bits[y * g.width + x]) out[ty * width + Math.min(width - 1, Math.floor(x / f))] = 1;
    }
  }
  return { width, height, bits: out };
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
