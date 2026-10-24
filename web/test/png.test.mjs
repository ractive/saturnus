// The PNG encoder (web/shared/png.js) and the screen images it makes
// (web/screenshot.js `screenPng`): each file decoded here (chunks, CRCs,
// zlib, the packed rows) and compared with the screen's RGBA, pixel for
// pixel, in both looks and on several models; the palette, the IHDR, the
// bit depth for more colours. `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { crc32 as zlibCrc32, inflateSync } from "node:zlib";
import { ANN_H, screenColors, screenPng, screenRgba } from "../screenshot.js";
import { bitDepth, crc32, indexedPng, pngBase64, pngBlob, twoColourPng } from "../shared/png.js";

const OFF = { leftshift: false, rightshift: false, alpha: false, alert: false, busy: false, transmit: false, updown: false, battery: false, g: false, rad: false, suspended: false };

/** A frame `width` by `rows`, a deterministic scatter of pixels on, some annunciators lit. */
function frame(width, rows, ann = {}, seed = 7) {
  const f = { width, height: rows, annunciators: { ...OFF, ...ann }, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 };
  const pixels = new Uint8Array(width * rows);
  let s = seed;
  for (let i = 0; i < pixels.length; i++) {
    s = (s * 1103515245 + 12345) & 0x7fffffff;
    pixels[i] = (s >> 16) % 3 === 0 ? 1 : 0;
  }
  return { f, pixels };
}

/**
 * A PNG decoded: every chunk's CRC checked (against node:zlib's), the
 * IHDR's fields, the palette, and the pixels as RGB (filter 0 rows only,
 * as the encoder writes).
 */
function decode(bytes) {
  const b = Buffer.from(bytes);
  assert.deepEqual([...b.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10], "the signature");
  const chunks = [];
  for (let at = 8; at < b.length;) {
    const len = b.readUInt32BE(at);
    const type = b.toString("latin1", at + 4, at + 8);
    const data = b.subarray(at + 8, at + 8 + len);
    const crc = b.readUInt32BE(at + 8 + len);
    assert.equal(zlibCrc32(b.subarray(at + 4, at + 8 + len)), crc, `${type}'s CRC`);
    chunks.push({ type, data });
    at += 12 + len;
  }
  assert.deepEqual(chunks.map((c) => c.type), ["IHDR", "PLTE", "IDAT", "IEND"]);
  const h = chunks[0].data;
  const ihdr = { width: h.readUInt32BE(0), height: h.readUInt32BE(4), depth: h[8], colourType: h[9], compression: h[10], filter: h[11], interlace: h[12] };
  const p = chunks[1].data;
  const palette = [];
  for (let i = 0; i < p.length; i += 3) palette.push([p[i], p[i + 1], p[i + 2]]);
  const raw = inflateSync(chunks[2].data);
  const rowBytes = 1 + Math.ceil((ihdr.width * ihdr.depth) / 8);
  assert.equal(raw.length, rowBytes * ihdr.height, "the rows' length");
  const rgb = new Uint8Array(ihdr.width * ihdr.height * 3);
  const per = 8 / ihdr.depth;
  for (let y = 0; y < ihdr.height; y++) {
    assert.equal(raw[y * rowBytes], 0, "filter type 0");
    for (let x = 0; x < ihdr.width; x++) {
      const byte = raw[y * rowBytes + 1 + Math.floor(x / per)];
      const v = (byte >> ((per - 1 - (x % per)) * ihdr.depth)) & ((1 << ihdr.depth) - 1);
      assert.ok(v < palette.length, "an index within the palette");
      rgb.set(palette[v], (y * ihdr.width + x) * 3);
    }
  }
  return { ihdr, palette, rgb, size: b.length };
}

/** RGBA (`screenRgba`) without its alpha, every pixel opaque. */
function rgbOf(img) {
  const rgb = new Uint8Array(img.width * img.height * 3);
  for (let i = 0; i < img.width * img.height; i++) {
    assert.equal(img.data[i * 4 + 3], 255);
    rgb.set(img.data.subarray(i * 4, i * 4 + 3), i * 3);
  }
  return rgb;
}

/** The number of distinct colours in RGBA. */
function colours(img) {
  const seen = new Set();
  for (let i = 0; i < img.data.length; i += 4) seen.add((img.data[i] << 16) | (img.data[i + 1] << 8) | img.data[i + 2]);
  return seen.size;
}

const MODELS = [
  ["48gx", 131, 64, { leftshift: true, busy: true }],
  ["49g", 131, 80, { alpha: true, transmit: true }],
  ["38g", 131, 64, { rightshift: true, alert: true }],
  ["42s", 131, 16, { updown: true, rad: true, battery: true }],
  ["28s", 137, 32, { suspended: true, rad: true }],
];

test("the screen as an indexed PNG: one bit a pixel, its two colours, the same pixels as the RGBA", async () => {
  for (const [model, w, rows, ann] of MODELS) {
    const { f, pixels } = frame(w, rows, ann);
    for (const look of ["lcd", "bw"]) {
      for (const scale of [4, 3, 1]) {
        const img = screenRgba(f, pixels, model, look, scale);
        assert.equal(colours(img), 2, `${model} ${look}: two colours, the strip included`);
        const png = decode(await screenPng(f, pixels, model, look, scale));
        const what = `${model} ${look} ${scale}x`;
        assert.deepEqual(png.ihdr, { width: w * scale, height: (ANN_H + rows) * scale, depth: 1, colourType: 3, compression: 0, filter: 0, interlace: 0 }, what);
        assert.equal(png.ihdr.width, img.width, what);
        assert.equal(png.ihdr.height, img.height, what);
        const { on, off } = screenColors(f, look);
        assert.deepEqual(png.palette, [off, on], `${what}: background, then ink`);
        assert.ok(Buffer.from(png.rgb).equals(Buffer.from(rgbOf(img))), `${what}: the pixels`);
      }
    }
  }
});

test("the LCD look keeps the LCD's colours at the contrast; black on white is black on white", async () => {
  const { f, pixels } = frame(131, 64, {});
  const dark = { ...f, contrast: 25 };
  assert.deepEqual(decode(await screenPng(dark, pixels, "48gx", "lcd")).palette, [screenColors(dark, "lcd").off, screenColors(dark, "lcd").on]);
  assert.deepEqual(decode(await screenPng(dark, pixels, "48gx", "bw")).palette, [[255, 255, 255], [0, 0, 0]]);
});

test("the rows: scaled with hard edges, odd widths padded within the byte", async () => {
  // 3 by 2: #.. / .## at 3x: 9 wide (two bytes a row, 7 bits of padding).
  const bits = Uint8Array.from([1, 0, 0, 0, 1, 1]);
  const png = decode(await twoColourPng({ width: 3, height: 2, bits }, { on: [1, 2, 3], off: [250, 251, 252] }, 3));
  assert.equal(png.ihdr.width, 9);
  assert.equal(png.ihdr.height, 6);
  const px = (x, y) => [...png.rgb.subarray((y * 9 + x) * 3, (y * 9 + x) * 3 + 3)];
  for (let y = 0; y < 6; y++) {
    for (let x = 0; x < 9; x++) assert.deepEqual(px(x, y), bits[Math.floor(y / 3) * 3 + Math.floor(x / 3)] ? [1, 2, 3] : [250, 251, 252], `(${x}, ${y})`);
  }
});

test("more colours, more bits: 2, 4 and 8 bits a pixel", async () => {
  assert.deepEqual([1, 2, 3, 4, 5, 16, 17, 256].map(bitDepth), [1, 1, 2, 2, 4, 4, 8, 8]);
  assert.throws(() => bitDepth(257));
  for (const n of [3, 4, 9, 200]) {
    const palette = Array.from({ length: n }, (_, i) => [i, 255 - i, (i * 7) & 255]);
    const index = Uint8Array.from({ length: 5 * 3 }, (_, i) => (i * 5) % n);
    const png = decode(await indexedPng({ width: 5, height: 3, index, palette }, 2));
    assert.equal(png.ihdr.depth, bitDepth(n));
    assert.deepEqual(png.palette, palette);
    for (let y = 0; y < 6; y++) {
      for (let x = 0; x < 10; x++) {
        const want = palette[index[Math.floor(y / 2) * 5 + Math.floor(x / 2)]];
        assert.deepEqual([...png.rgb.subarray((y * 10 + x) * 3, (y * 10 + x) * 3 + 3)], want, `${n} colours (${x}, ${y})`);
      }
    }
  }
});

test("without CompressionStream: the same image as RGBA through a canvas", async () => {
  const saved = { cs: globalThis.CompressionStream, doc: globalThis.document, id: globalThis.ImageData };
  const drawn = [];
  let blobOut = new Blob([Uint8Array.from([1, 2, 3])]);
  globalThis.CompressionStream = undefined;
  globalThis.ImageData = class { constructor(data, w, h) { Object.assign(this, { data, width: w, height: h }); } };
  globalThis.document = {
    createElement: (tag) => {
      assert.equal(tag, "canvas");
      const canvas = {
        getContext: () => ({ putImageData: (img, x, y) => drawn.push({ img, x, y, canvas: [canvas.width, canvas.height] }) }),
        toBlob: (cb, type) => { assert.equal(type, "image/png"); cb(blobOut); },
      };
      return canvas;
    },
  };
  try {
    const { f, pixels } = frame(131, 16, { rad: true });
    const want = screenRgba(f, pixels, "42s", "lcd", 4);
    const bytes = await screenPng(f, pixels, "42s", "lcd", 4);
    assert.deepEqual([...bytes], [1, 2, 3], "the canvas's PNG");
    assert.equal(drawn.length, 1);
    assert.deepEqual(drawn[0].canvas, [want.width, want.height]);
    assert.deepEqual([drawn[0].img.width, drawn[0].img.height, drawn[0].x, drawn[0].y], [want.width, want.height, 0, 0]);
    assert.ok(Buffer.from(drawn[0].img.data).equals(Buffer.from(want.data)), "the screen's RGBA, pixel for pixel");
    // The blob and base64 forms follow; a canvas that makes nothing rejects.
    assert.equal((await pngBlob(screenPng(f, pixels, "42s", "bw"))).size, 3);
    blobOut = null;
    await assert.rejects(screenPng(f, pixels, "42s", "bw"), /could not be made/);
  } finally {
    globalThis.CompressionStream = saved.cs;
    globalThis.document = saved.doc;
    globalThis.ImageData = saved.id;
  }
});

test("the CRC is zlib's; the bytes as a blob and as base64", async () => {
  const sample = new TextEncoder().encode("IEND");
  assert.equal(crc32(sample), zlibCrc32(sample));
  assert.equal(crc32(sample), 0xae426082);
  const { f, pixels } = frame(131, 64, { busy: true });
  const bytes = await screenPng(f, pixels, "48gx", "bw", 3);
  const blob = await pngBlob(screenPng(f, pixels, "48gx", "bw", 3));
  assert.equal(blob.type, "image/png");
  assert.ok(Buffer.from(await blob.arrayBuffer()).equals(Buffer.from(bytes)));
  assert.equal(await pngBase64(bytes), Buffer.from(bytes).toString("base64"));
});
