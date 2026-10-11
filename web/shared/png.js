// An RGBA image as a PNG: made through a canvas, put on the clipboard.
// `copyPng` is tested by web/test/screenshot.test.mjs.

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
