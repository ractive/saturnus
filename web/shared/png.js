// Pixel images as indexed PNGs, encoded here (no canvas): a palette of
// the image's colours and as few bits a pixel as it needs (1 for the two
// colours of a screen or a GROB), scaled up in the rows with hard pixel
// edges, the rows deflated by the browser's CompressionStream (a canvas's
// RGBA PNG where there is none). And the clipboard write. Pure but for
// that fallback (no DOM): web/test/png.test.mjs, `copyPng` in
// web/test/screenshot.test.mjs.

/**
 * Put a PNG on the clipboard. `blob` is a promise of it, so the item is
 * made at once, inside the click or key that asked (Safari needs that);
 * resolves to whether it worked. False where images cannot be copied
 * (no `ClipboardItem`, no `image/png` support) or the write failed.
 */
export async function copyPng(blob, { clipboard = globalThis.navigator?.clipboard, Item = globalThis.ClipboardItem } = {}) {
  // Observed on every path: a failed image unused here is no unhandled rejection.
  Promise.resolve(blob).catch(() => {});
  if (!clipboard?.write || typeof Item !== "function") return false;
  if (typeof Item.supports === "function" && !Item.supports("image/png")) return false;
  try {
    await clipboard.write([new Item({ "image/png": blob })]);
    return true;
  } catch {
    return false;
  }
}

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

/** The CRC-32 of `bytes` (PNG's, the zlib one). */
export function crc32(bytes, crc = 0) {
  let c = ~crc;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return ~c >>> 0;
}

const SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10];

/** A chunk: length, type, data, CRC over type and data. */
function chunk(type, data) {
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  for (let i = 0; i < 4; i++) out[4 + i] = type.charCodeAt(i);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

/** The bit depth for `n` palette entries (1, 2, 4 or 8). */
export function bitDepth(n) {
  if (n > 256) throw new Error("more than 256 colours");
  return n <= 2 ? 1 : n <= 4 ? 2 : n <= 16 ? 4 : 8;
}

/**
 * The raw rows of the image (each a filter byte, 0, then the packed
 * indices, leftmost pixel in the top bits), `scale` image pixels per
 * pixel of `index` (one palette index a pixel, row by row). A scaled
 * row is made once and repeated `scale` times.
 */
function rawRows(width, height, index, depth, scale) {
  const W = width * scale;
  const rowBytes = 1 + Math.ceil((W * depth) / 8);
  const raw = new Uint8Array(rowBytes * height * scale);
  const perByte = 8 / depth;
  for (let y = 0; y < height; y++) {
    const first = y * scale * rowBytes;
    for (let x = 0; x < W; x++) {
      const v = index[y * width + Math.floor(x / scale)];
      if (v) raw[first + 1 + Math.floor(x / perByte)] |= v << ((perByte - 1 - (x % perByte)) * depth);
    }
    for (let r = 1; r < scale; r++) raw.copyWithin(first + r * rowBytes, first, first + rowBytes);
  }
  return raw;
}

/** `bytes` through zlib's deflate (the browser's CompressionStream). */
async function deflate(bytes) {
  const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream("deflate"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

/**
 * Whether this browser deflates (CompressionStream with "deflate": not
 * in Safari before 16.4, Firefox before 113, an old WebKitGTK webview).
 */
function canDeflate() {
  if (typeof globalThis.CompressionStream !== "function") return false;
  try {
    new globalThis.CompressionStream("deflate");
    return true;
  } catch {
    return false;
  }
}

/**
 * The fallback where the browser cannot deflate: the same image as RGBA,
 * encoded by a canvas (an 8-bit RGBA PNG, larger, the same pixels).
 */
function canvasPng({ width, height, index, palette }, scale) {
  const W = width * scale;
  const H = height * scale;
  const data = new Uint8ClampedArray(W * H * 4);
  for (let y = 0; y < H; y++) {
    const row = Math.floor(y / scale) * width;
    for (let x = 0; x < W; x++) {
      const c = palette[index[row + Math.floor(x / scale)]];
      data.set([c[0], c[1], c[2], 255], (y * W + x) * 4);
    }
  }
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  canvas.getContext("2d").putImageData(new ImageData(data, W, H), 0, 0);
  return new Promise((resolve, reject) => {
    canvas.toBlob((b) => (b ? resolve(b.arrayBuffer().then((a) => new Uint8Array(a))) : reject(new Error("the image could not be made"))), "image/png");
  });
}

/**
 * An indexed PNG (colour type 3) of `width` by `height` pixels, each a
 * palette index in `index` (one byte a pixel), `palette` its colours
 * (`[r, g, b]`, 1 to 256 of them), scaled up `scale` times with hard
 * edges: a promise of the file's bytes. Where the browser cannot
 * deflate, an RGBA PNG from a canvas instead (`canvasPng`).
 */
export async function indexedPng({ width, height, index, palette }, scale = 1) {
  if (!palette.length) throw new Error("no colours");
  if (!canDeflate()) return canvasPng({ width, height, index, palette }, scale);
  const depth = bitDepth(palette.length);
  const W = width * scale;
  const H = height * scale;
  const ihdr = new Uint8Array(13);
  const view = new DataView(ihdr.buffer);
  view.setUint32(0, W);
  view.setUint32(4, H);
  ihdr.set([depth, 3, 0, 0, 0], 8);
  const plte = new Uint8Array(palette.flatMap((c) => [c[0], c[1], c[2]]));
  const idat = await deflate(rawRows(width, height, index, depth, scale));
  const parts = [Uint8Array.from(SIGNATURE), chunk("IHDR", ihdr), chunk("PLTE", plte), chunk("IDAT", idat), chunk("IEND", new Uint8Array(0))];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

/**
 * A two-colour picture as a PNG's bytes (a promise): `bits` one byte a
 * pixel, 1 on, in `colors` `{on, off}` (`screenColors`), `scale` times.
 */
export function twoColourPng({ width, height, bits }, colors, scale = 1) {
  return indexedPng({ width, height, index: bits, palette: [colors.off, colors.on] }, scale);
}

/** The PNG's bytes (or a promise of them) as a blob (a promise). */
export async function pngBlob(bytes) {
  return new Blob([await bytes], { type: "image/png" });
}

/** The PNG's bytes (or a promise of them) as base64 (a promise). */
export async function pngBase64(bytes) {
  const b = await bytes;
  let s = "";
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return btoa(s);
}
