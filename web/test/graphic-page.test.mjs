// A graphic (GROB) in the memory view of the real page in headless
// Chrome over the DevTools protocol: the picture in the preview at a
// whole scale with the nibbles folded below, its size in the meta line,
// Copy image and Save as image… in the "⋯" menu and the context menu
// (the PNG four times the picture), Edit off with its reason; on the
// Stack tab a thumbnail in the level's row beside "Graphic 131 × 64", the
// same preview on selecting it, and no overflow at 360 px. No ROM: the
// memory reads are answered by the test (a 48GX with HOME holding the
// graphic PIC, the stack the same graphic). Skipped without Chrome
// (`SATURNUS_CHROME` names one) or the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

// A 131×64 picture: a frame round the edge and the diagonal from the
// top left, as the host sends it (17 bytes a row, leftmost pixel in the
// top bit).
const W = 131;
const H = 64;
function rows() {
  let hex = "";
  for (let y = 0; y < H; y++) {
    const bytes = new Uint8Array(Math.ceil(W / 8));
    for (let x = 0; x < W; x++) {
      if (y === 0 || y === H - 1 || x === 0 || x === W - 1 || x === y) bytes[x >> 3] |= 0x80 >> (x & 7);
    }
    hex += [...bytes].map((b) => b.toString(16).toUpperCase().padStart(2, "0")).join("");
  }
  return hex;
}
const GRAPHIC = { type: "unknown", prolog: "02B1E", kind: "Graphic", nibbles: 2196, hex: "E1B20F8800040003800", truncated: true, graphic: { width: W, height: H, rows: rows() } };

const FAKE_MEMORY = `(() => {
  const b = window.saturnus.backend;
  const g = ${JSON.stringify(GRAPHIC)};
  const tree = { path: ["HOME"], variables: [{ name: "PIC", type: "Graphic", size: 1107.5, checksum: 0x1893, address: 0x7A000 }, { name: "X", type: "Real Number", size: 10.5, checksum: 0x5B55, address: 0x7A100 }] };
  window.__saved = [];
  b.watchMemory = async () => ({ supported: true });
  b.memoryTree = async () => structuredClone(tree);
  b.stack = async () => [{ type: "real", text: "42" }, structuredClone(g)];
  b.flags = async () => ({ set: [] });
  b.objectAt = async (address) => (address === 0x7A000 ? structuredClone(g) : { type: "real", text: "42" });
  b.saveFile = async (name, blob) => {
    const bmp = await createImageBitmap(blob);
    const c = new OffscreenCanvas(bmp.width, bmp.height).getContext("2d");
    c.drawImage(bmp, 0, 0);
    const px = (x, y) => [...c.getImageData(x, y, 1, 1).data.slice(0, 3)].join(",");
    window.__saved.push({ name, type: blob.type, width: bmp.width, height: bmp.height, corner: px(1, 1), inside: px(20, 10) });
    return name;
  };
  window.saturnus.store.set({ booted: "48gx", screenLook: "bw" });
  return window.saturnus.setLayer(true).then(() => window.saturnus.explorer.setTab("vars")).then(() => true);
})()`;

async function page(t, { width = 1280, height = 900, mobile = false } = {}) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  if (mobile) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started");
  await ev(FAKE_MEMORY);
  await until(ev, `document.querySelector('.list tbody tr[data-name="PIC"]')`, 5_000, "the list shows PIC");
  const click = async (selector, button = "left") => {
    const [x, y] = await ev(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [r.x + Math.min(30, r.width / 2), r.y + r.height / 2]; })()`);
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount: 1 });
    await sleep(150);
  };
  const items = () => ev(`[...document.querySelectorAll(".menu [role=menuitem]")].map((b) => b.querySelector(".menu-text").textContent)`);
  const choose = async (text) => {
    const id = await ev(`(() => { const b = [...document.querySelectorAll(".menu [role=menuitem]")].find((b) => b.querySelector(".menu-text").textContent === ${JSON.stringify(text)}); b.id ||= "it-" + Math.random().toString(36).slice(2); return "#" + b.id; })()`);
    await click(id);
  };
  /** The preview of `pane`: its meta line, the picture's canvas and the folded nibbles. */
  const preview = (pane) => ev(`(() => {
    const box = document.querySelector(".pane-${pane} .preview");
    const c = box.querySelector("canvas.graphic");
    const edit = box.querySelector(".preview-actions button.edit");
    const r = c?.getBoundingClientRect();
    return {
      meta: box.querySelector(".preview-meta")?.textContent ?? null,
      canvas: c ? [c.width, c.height, Math.round(r.width), Math.round(r.height), Number(c.dataset.scale)] : null,
      label: c?.getAttribute("aria-label") ?? null,
      fits: c ? r.right <= box.getBoundingClientRect().right + 0.5 : null,
      nibbles: box.querySelector("details.nibbles") ? !box.querySelector("details.nibbles").open : null,
      edit: edit ? [edit.disabled, edit.title] : null,
    };
  })()`);
  return { send, ev, click, items, choose, preview };
}

test("a graphic variable: its picture at a whole scale, its size, Copy image and Save as image", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.click('.list tbody tr[data-name="PIC"] td');
  await until(p.ev, `document.querySelector(".pane-vars .preview canvas.graphic")`, 3_000, "PIC read");
  const v = await p.preview("vars");
  assert.match(v.meta, /^Graphic\s*·\s*131×64\s*·\s*1107\.5 bytes\s*·\s*# 1893h$/, v.meta);
  const [w, h, cssW, cssH, scale] = v.canvas;
  assert.deepEqual([w, h], [W, H], "one canvas pixel a picture pixel");
  assert.ok(scale >= 2 && scale <= 4, `scale ${scale}`);
  assert.deepEqual([cssW, cssH], [W * scale, H * scale], "a whole scale");
  assert.equal(v.fits, true);
  assert.equal(v.label, "Graphic 131 × 64");
  assert.equal(v.nibbles, true, "the nibbles folded below");
  assert.deepEqual(v.edit, [true, "A graphic has no text form to edit"]);
  // The pixels drawn: the frame's corner dark, inside it light, in the LCD's colours.
  const px = await p.ev(`(() => { const c = document.querySelector(".pane-vars canvas.graphic").getContext("2d"); const at = (x, y) => [...c.getImageData(x, y, 1, 1).data.slice(0, 3)]; return [at(0, 0), at(5, 2)]; })()`);
  assert.ok(px[1][0] - px[0][0] > 60, JSON.stringify(px));

  await p.click(".pane-vars .preview button.more");
  const menu = await p.items();
  assert.ok(menu.includes("Copy image") && menu.includes("Save as image…"), JSON.stringify(menu));
  assert.ok(!menu.includes("Copy text"));
  await p.choose("Save as image…");
  await until(p.ev, "window.__saved.length", 3_000, "the image saved");
  const saved = await p.ev("window.__saved[0]");
  // Four times the picture, in the look of the screen images (black on white here).
  assert.deepEqual(saved, { name: "PIC.png", type: "image/png", width: W * 4, height: H * 4, corner: "0,0,0", inside: "255,255,255" });

  // The context menu has them too.
  await p.click('.list tbody tr[data-name="PIC"] td', "right");
  const ctx = await p.items();
  assert.ok(ctx.includes("Copy image") && ctx.includes("Save as image…"), JSON.stringify(ctx));
});

test("a graphic on the stack: a thumbnail in its row, the picture when selected", { timeout: 120_000 }, async (t) => {
  for (const [width, mobile] of [[1280, false], [360, true]]) {
    const p = await page(t, { width, height: 780, mobile });
    if (!p) return;
    await p.ev(`window.saturnus.explorer.setTab("stack")`);
    await until(p.ev, `document.querySelector('.levels li[data-level="2"] canvas.thumb')`, 3_000, "the stack drawn");
    const row = await p.ev(`(() => {
      const li = document.querySelector('.levels li[data-level="2"]');
      const c = li.querySelector("canvas.thumb");
      const r = c.getBoundingClientRect();
      const lr = li.getBoundingClientRect();
      return { text: li.querySelector(".obj").textContent, partial: li.classList.contains("partial"), size: [c.width, c.height],
        h: r.height, rowH: lr.height, inside: r.right <= lr.right && r.bottom <= lr.bottom + 0.5,
        over: document.documentElement.scrollWidth > innerWidth, liOver: li.scrollWidth > li.clientWidth };
    })()`);
    assert.equal(row.text, "Graphic 131 × 64", `${width}`);
    assert.equal(row.partial, false);
    // Drawn at the row's height (shrunk once), not the whole picture.
    assert.ok(row.size[1] <= row.rowH && row.size[1] < H, `${width}: a ${row.size} thumbnail in a row of ${row.rowH}`);
    assert.equal(row.size[0], Math.round(W / (H / row.size[1])), `${width}: its proportions`);
    assert.ok(row.h > 10 && row.h < row.rowH, `${width}: thumb ${row.h} in a row of ${row.rowH}`);
    assert.equal(row.inside, true, `${width}: the thumbnail inside its row`);
    assert.equal(row.over, false, `${width}: no page overflow`);
    assert.equal(row.liOver, false, `${width}: no row overflow`);

    await p.click('.levels li[data-level="2"]');
    await until(p.ev, `document.querySelector(".pane-stack .preview canvas.graphic")`, 3_000, "level 2 shown");
    const v = await p.preview("stack");
    assert.match(v.meta, /^Graphic\s*·\s*131×64\s*·\s*1098 bytes$/, v.meta);
    assert.ok(v.canvas[4] >= 1 && v.canvas[4] <= 4);
    assert.equal(v.fits, true, `${width}: the picture fits`);
    assert.equal(v.nibbles, true);
    assert.deepEqual(v.edit, [true, "A graphic has no text form to edit"]);
    if (width === 1280) assert.ok(v.canvas[4] >= 2, "a desktop preview at least twice");
    // Under touch emulation a synthetic mouse press does not open it: the button's own click.
    if (mobile) await p.ev(`document.querySelector(".pane-stack .preview button.more").click()`);
    else await p.click(".pane-stack .preview button.more");
    assert.deepEqual(await p.items(), ["Copy image", "Save as image…"], `${width}`);
    await p.choose("Save as image…");
    await until(p.ev, "window.__saved.length", 3_000, "the image saved");
    assert.equal((await p.ev("window.__saved[0]")).name, "Level-2.png");
    assert.equal(await p.ev("document.documentElement.scrollWidth > innerWidth"), false, `${width}: no overflow with the preview`);
  }
});

/** The stack as given (level 1 first), the page's errors kept. */
const STACK = (levels) => `(() => {
  window.__errors = [];
  window.addEventListener("error", (e) => window.__errors.push(String(e.message)));
  window.addEventListener("unhandledrejection", (e) => window.__errors.push(String(e.reason?.message ?? e.reason)));
  window.saturnus.backend.stack = async () => ${JSON.stringify(levels)};
  return window.saturnus.memory.refresh().then(() => window.saturnus.explorer.setTab("stack")).then(() => true);
})()`;

test("a graphic with no pixels (#0 #0 BLANK) is shown by its nibbles; nothing breaks", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const empty = { ...GRAPHIC, nibbles: 20, hex: "E1B20F0000000000000", truncated: false, graphic: { width: 0, height: 0, rows: "" } };
  await p.ev(STACK([{ type: "real", text: "42" }, empty]));
  await until(p.ev, `document.querySelector('.levels li[data-level="2"]')`, 3_000, "the stack drawn");
  await p.click('.levels li[data-level="2"]');
  await sleep(300);
  const v = await p.ev(`(() => {
    const box = document.querySelector(".pane-stack .preview");
    return {
      canvas: !!document.querySelector(".pane-stack canvas"),
      nibbles: !!box.querySelector("details.nibbles"),
      items: [...box.querySelectorAll(".preview-actions > button")].map((b) => b.textContent || "⋯"),
      errors: window.__errors,
    };
  })()`);
  assert.equal(v.canvas, false, "no picture to draw");
  assert.equal(v.nibbles, true, "its nibbles instead");
  assert.ok(!v.items.includes("Copy image"), JSON.stringify(v.items));
  assert.deepEqual(v.errors, []);
  // The stack still follows the calculator.
  await p.ev(STACK([{ type: "real", text: "7" }, empty]));
  await until(p.ev, `document.querySelector('.levels li[data-level="1"] .obj')?.textContent === "7"`, 3_000, "the stack updated");
  assert.deepEqual(await p.ev("window.__errors"), []);
});

test("a large graphic: a thumbnail no higher than its row, an image within a browser's limits", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  // 1024×1024 with a one-pixel diagonal: the thumbnail keeps the line.
  const n = 1024;
  const big = await p.ev(`(() => {
    const n = ${n};
    const bytes = new Uint8Array(n * n / 8);
    for (let i = 0; i < n; i++) bytes[(i * n + i) >> 3] |= 0x80 >> (i & 7);
    let hex = "";
    for (const b of bytes) hex += b.toString(16).toUpperCase().padStart(2, "0");
    return { type: "unknown", prolog: "02B1E", kind: "Graphic", nibbles: n * n / 4 + 20, hex: "E1B20", truncated: true, graphic: { width: n, height: n, rows: hex } };
  })()`);
  await p.ev(STACK([big]));
  await until(p.ev, `document.querySelector('.levels li[data-level="1"] canvas.thumb')`, 5_000, "the thumbnail");
  const r = await p.ev(`(() => {
    const li = document.querySelector('.levels li[data-level="1"]');
    const c = li.querySelector("canvas.thumb");
    const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data;
    let dark = 0;
    for (let i = 0; i < d.length; i += 4) if (d[i] < 100) dark++;
    return { size: [c.width, c.height], rowH: li.getBoundingClientRect().height, dark };
  })()`);
  assert.ok(r.size[1] <= r.rowH, `a ${r.size} thumbnail in a row of ${r.rowH}`);
  assert.equal(r.size[0], r.size[1], "square");
  assert.ok(r.dark >= r.size[1], `the diagonal kept: ${r.dark} dark pixels`);
  // Saved: at a scale that keeps it within 16 Mpixels (not 4096×4096).
  await p.click(".pane-stack .preview button.more");
  await p.choose("Save as image…");
  await until(p.ev, "window.__saved.length", 10_000, "the image saved");
  const saved = await p.ev("window.__saved[0]");
  assert.ok(saved.width * saved.height <= 16_000_000, `${saved.width}×${saved.height}`);
  assert.equal(saved.width, 3 * n, "three times, the most that fits");
  assert.deepEqual(await p.ev("window.__errors"), []);
});
