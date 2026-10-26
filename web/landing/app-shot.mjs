// The app image of the landing page: the whole app (#/calculator) in
// headless Chrome (the page tests' chrome.mjs), light theme, 1280x800 at
// device scale 2, an emulated HP 48GX on its real ROM showing the
// saturnus logo on its LCD. The logo is put there by the calculator: the
// page rasterises web/logo.svg to 1 bit on a canvas (the ring's front arc
// drawn with a white edge, so it reads in front of the planet), this
// script packs the pixels into an HP 48 graphics object as text,
// "GROB 131 64 <hex>" (each row 17 bytes, low nibble first, the low bit
// of each nibble the leftmost pixel), and RPL on the calculator compiles
// it, stores it as PICT, writes "saturnus" beside the planet in the
// calculator's own large font (→GROB, REPL) and shows PICT full screen
// with { } PVIEW. Writes web/landing/raw/app.png (gitignored); the landing
// page does not use this plain variant. With MEMORY=1, the one it uses: the variant with the memory view open on the right, at
// 1440x900 (at 1280 the three columns are too cramped), AREA selected: a few
// variables stored in HOME first by RPL (programs, a number, a list, a
// directory), the same logo on the LCD; writes raw/app-memory.png, then
//   for w in 640 1280 2000; do magick raw/app-memory.png -resize ${w}x -quality 82 img/app-memory-$w.webp; done
// SCREEN=FILE also writes the LCD as text (`#` dark, `.` light).
// Usage: SATURNUS_ROM_DIR=../../roms [MEMORY=1] [SCREEN=FILE] node app-shot.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { serve, chrome, findChrome, sleep } from "../test/chrome.mjs";

const ROMS = process.env.SATURNUS_ROM_DIR;
if (!ROMS) throw new Error("SATURNUS_ROM_DIR names the ROM directory");
const OUT = new URL("raw/", import.meta.url).pathname;
mkdirSync(OUT, { recursive: true });
const ROM = "gxrom-r";
const MEMORY = Boolean(process.env.MEMORY);
const VIEW = MEMORY ? { width: 1440, height: 900 } : { width: 1280, height: 800 };
const W = 131, H = 64;
/** MEMORY=1: RPL run first, so the memory view lists something (Unicode « » →: run takes the characters). */
const VARS = [
  "HOME",
  "'GAMES' CRDIR",
  "GAMES « RAND 6 * IP 1 + » 'DICE' STO 0 'HISCORE' STO HOME",
  "9.80665 'G' STO",
  "{ (0,0) (1,2) (3,1) (4,4) } 'PTS' STO",
  "« → r « 'π*r^2' →NUM » » 'AREA' STO",
  "« → a b « a b * a b + / » » 'PAR' STO",
  "\"Exam on Friday: bring spare batteries\" 'NOTES' STO",
  "CLEAR",
];
/** The logo's square on the LCD: its size and left edge (the word goes to its right). */
const LOGO = { size: 62, x: 8 };

/** HP 48 GROB text: rows of ceil(w/8) bytes, each byte low nibble first, bit 0 of a nibble the leftmost pixel. */
function grob(bits, w, h) {
  const nibbles = Math.ceil(w / 8) * 2;
  let hex = "";
  for (let y = 0; y < h; y++)
    for (let n = 0; n < nibbles; n++) {
      let v = 0;
      for (let b = 0; b < 4; b++) {
        const x = n * 4 + b;
        if (x < w && bits[y * w + x]) v |= 1 << b;
      }
      hex += v.toString(16).toUpperCase();
    }
  return `GROB ${w} ${h} ${hex}`;
}

const { server, port } = await serve({ [`/__rom/${ROM}`]: join(ROMS, ROM) });
const binary = findChrome();
if (!binary) throw new Error("no Chrome");
const c = await chrome(binary);
const { send, ev } = c;
const until = async (expr, ms, what) => {
  const end = Date.now() + ms;
  while (!(await ev(expr).catch(() => false))) {
    if (Date.now() > end) throw new Error(`${what}: not within ${ms} ms`);
    await sleep(100);
  }
};
try {
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { ...VIEW, deviceScaleFactor: 2, mobile: false });
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  await until("!!window.saturnus", 15_000, "the page");
  await ev("window.saturnus.started.then(() => true)");
  await ev(`window.saturnus.store.set({ model: "48gx" }); true`);
  await sleep(200);
  await ev(`(async () => {
    const bytes = await (await fetch("/__rom/${ROM}")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "${ROM}"));
    return true;
  })()`);
  await until(`window.saturnus.store.state.booted === "48gx"`, 30_000, "the 48GX booted");
  await ev(`window.__idle = async (ms = 1500) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (let i = 0; i < 600; i++) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
      await new Promise((r) => setTimeout(r, 50));
    }
    return false;
  }; true`);
  await ev("window.__idle(3000)");
  for (const t of MEMORY ? VARS : []) {
    await ev(`window.saturnus.backend.run(${JSON.stringify(t)}).then(() => true)`);
    await ev("window.__idle(600)");
  }
  if (MEMORY) await ev("Promise.resolve(window.saturnus.setLayer(true)).then(() => true)");

  // The logo, 1 bit: the back arc, the planet with a white edge over it,
  // the front arc with a white edge where it crosses the planet (so the
  // ring's ends stay whole), a thinner ring than the SVG's, thresholded.
  const bits = await ev(`(async () => {
    const S = ${LOGO.size}, X = ${LOGO.x}, k = S / 32;
    const ring = (d, stroke, width) => '<g fill="none" stroke="' + stroke + '" stroke-width="' + width + '" stroke-linecap="round" transform="rotate(-24 16 16)"><path d="' + d + '"/></g>';
    const back = "M 1.6 16 A 14.4 4.6 0 0 1 30.4 16", front = "M 30.4 16 A 14.4 4.6 0 0 1 1.6 16";
    const gap = 2 / k, rw = 2.2;
    const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="' + S + '" height="' + S + '">' +
      '<clipPath id="p"><circle cx="16" cy="16" r="' + (8.6 + gap) + '"/></clipPath>' +
      ring(back, "#000", rw) +
      '<circle cx="16" cy="16" r="' + (8.6 + gap) + '" fill="#fff"/><circle cx="16" cy="16" r="8.6" fill="#000"/>' +
      '<g clip-path="url(#p)">' + ring(front, "#fff", rw + 2 * gap) + '</g>' + ring(front, "#000", rw) + '</svg>';
    const img = new Image();
    img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
    await img.decode();
    const cv = document.createElement("canvas");
    cv.width = ${W}; cv.height = ${H};
    const g = cv.getContext("2d");
    g.fillStyle = "#fff"; g.fillRect(0, 0, ${W}, ${H});
    g.drawImage(img, X, Math.round((${H} - S) / 2), S, S);
    const d = g.getImageData(0, 0, ${W}, ${H}).data;
    const out = [];
    for (let i = 0; i < ${W * H}; i++) out.push(d[i * 4] < 128 ? 1 : 0);
    return out;
  })()`);
  const text = grob(bits, W, H);
  if (process.env.DEBUG)
    for (let y = 0; y < H; y++) console.log(bits.slice(y * W, y * W + W).map((b) => (b ? "#" : ".")).join(""));

  // On the calculator: the GROB as PICT, the word beside the planet, PICT full screen.
  const wordX = LOGO.x + LOGO.size + 2;
  const RPL = [
    text,
    "PICT STO",
    `PICT { #${wordX}d #28d } "saturnus" 3 →GROB REPL`,
    "{ } PVIEW",
  ];
  for (const t of RPL) {
    await ev(`window.saturnus.backend.run(${JSON.stringify(t)}).then(() => true)`);
    await ev("window.__idle(800)");
    if (process.env.DEBUG) console.log(t.slice(0, 40), "\n" + (await ev("window.saturnus.screenText()")).split("\n").slice(0, 10).join("\n"));
  }
  await sleep(500);
  // SCREEN=FILE: the LCD as text (`#` dark, `.` light), for a pixel copy of the logo.
  if (process.env.SCREEN) writeFileSync(process.env.SCREEN, await ev("window.saturnus.screenText()"));
  // MEMORY=1: AREA selected, so the preview below the list shows a program.
  if (MEMORY)
    await until(`(() => {
      const tr = document.querySelector('sat-explorer tr[data-name="AREA"]');
      tr?.click();
      return Boolean(tr);
    })()`, 10_000, "the AREA row");
  await sleep(800);
  await ev("window.saturnus.store.set({ storageOffer: null }); true");
  // No toasts or offers over the app, no focus ring or caret.
  await ev(`(() => {
    const s = document.createElement("style");
    s.textContent = "sat-toast, .toast, .toasts, .tour-offer { display: none !important; } * { caret-color: transparent !important; }";
    document.head.append(s);
    document.activeElement?.blur?.();
    getSelection().removeAllRanges();
    return true;
  })()`);
  await sleep(500);
  const shot = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(OUT, MEMORY ? "app-memory.png" : "app.png"), Buffer.from(shot.data, "base64"));
  console.log("saved");
} finally {
  await c.close();
  server.closeAllConnections();
  server.close();
}
