// The Files image of the landing page's "File transfer" section: the app's
// Files view (#/files/48gx) in headless Chrome (the page tests'
// chrome.mjs), light theme, 1280 px wide at device scale 2. The left pane
// is an emulated HP 48GX on its real ROM, a few variables stored on it by
// RPL (a directory among them); the right pane a folder "HP48" with .hp
// files and two subdirectories. That folder lives in the origin private
// file system and is handed to the page as a folder kept from an earlier
// visit (web/tx/folder.js, `handleStore`), so it shows as a folder by its
// name; its files are an HP binary header and filler bytes (only names
// and sizes show). Writes web/landing/raw/files.png (gitignored). Then,
// for img/:
//   for w in 640 1280 2000; do magick raw/files.png -resize ${w}x -quality 82 img/files-$w.webp; done
// Usage: SATURNUS_ROM_DIR=../../roms node files-shot.mjs
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { serve, chrome, findChrome, sleep } from "../test/chrome.mjs";

const ROMS = process.env.SATURNUS_ROM_DIR;
if (!ROMS) throw new Error("SATURNUS_ROM_DIR names the ROM directory");
const OUT = new URL("raw/", import.meta.url).pathname;
mkdirSync(OUT, { recursive: true });
const ROM = "gxrom-r";

/** RPL run on the calculator, in order (Unicode « » → π: run takes the characters). */
const RPL = [
  "HOME",
  "'GAMES' CRDIR",
  "GAMES « RAND 6 * IP 1 + » 'DICE' STO 0 'HISCORE' STO HOME",
  "'UTILS' CRDIR",
  "UTILS « → x « x 1 + x / » » 'INVX' STO HOME",
  "9.80665 'G' STO",
  "[[ 2 1 ][ 1 3 ]] 'M' STO",
  "{ (0,0) (1,2) (3,1) (4,4) } 'PTS' STO",
  "\"Exam on Friday: bring spare batteries\" 'NOTES' STO",
  "« → r « 'π*r^2' →NUM » » 'AREA' STO",
  "« → a b « a b * a b + / » » 'PAR' STO",
  "CLEAR",
];

/** The folder on the computer: path -> size in bytes (a trailing "/" makes a directory). */
const FOLDER = {
  "GAMES/": 0,
  "GAMES/BLOCKS.hp": 3412,
  "GAMES/MINES.hp": 2187,
  "GAMES/SNAKE.hp": 1845,
  "BACKUP/": 0,
  "BACKUP/HOME-2026-09-14.hp": 18734,
  "BACKUP/HOME-2026-10-21.hp": 21096,
  "SOLVE.hp": 1288,
  "UNITS.hp": 2604,
  "STATS.hp": 946,
  "MATRIX.hp": 1530,
  "CIRCUIT.hp": 3187,
  "PERIODIC.hp": 7421,
  "README.txt": 412,
};

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
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 2, mobile: false });
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
  // No folder picker: the kept folder below is the right pane's.
  await send("Page.addScriptToEvaluateOnNewDocument", { source: "delete Window.prototype.showDirectoryPicker; delete window.showDirectoryPicker;" });

  // The calculator: the 48GX on its ROM in the Calculator view (as shots.mjs).
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
  for (const text of RPL) {
    await ev(`window.saturnus.backend.run(${JSON.stringify(text)}).then(() => true)`);
    await ev("window.__idle(600)");
    if (process.env.DEBUG) console.log(text, "\n" + (await ev("window.saturnus.screenText()")).split("\n").slice(0, 16).join("\n"));
  }
  console.log("screen:\n" + (await ev("window.saturnus.screenText()")));

  // The folder "HP48" in the origin private file system, kept as if picked on an earlier visit.
  await ev(`import("./tx/folder.js").then(async (m) => {
    const folder = ${JSON.stringify(FOLDER)};
    const root = await navigator.storage.getDirectory();
    await root.removeEntry("HP48", { recursive: true }).catch(() => {});
    const top = await root.getDirectoryHandle("HP48", { create: true });
    for (const [path, size] of Object.entries(folder)) {
      const parts = path.split("/").filter(Boolean);
      let dir = top;
      for (const p of parts.slice(0, path.endsWith("/") ? parts.length : -1)) dir = await dir.getDirectoryHandle(p, { create: true });
      if (path.endsWith("/")) continue;
      const name = parts.at(-1);
      const bytes = new Uint8Array(size);
      if (name.endsWith(".hp")) bytes.set(new TextEncoder().encode("HPHP48-R"));
      for (let i = 8; i < size; i++) bytes[i] = (i * 2654435761) >>> 24;
      const w = await (await dir.getFileHandle(name, { create: true })).createWritable();
      await w.write(bytes);
      await w.close();
    }
    await m.handleStore().put("kept", { handle: top, name: "HP48", used: Date.now() });
    return true;
  })`);

  // Files on that calculator, the folder on the right.
  await ev(`location.hash = "#/files/48gx"; true`);
  await until("!!window.tx", 15_000, "Files");
  await ev("tx.started.then(() => true)");
  await until("tx.idle(0)", 60_000, "the calculator listed");
  await until(`tx.panes[1].place?.kind === "folder" && tx.idle(1)`, 15_000, "the folder listed");
  await sleep(500);
  // No toasts or notes over the panes, nothing selected, no focus ring.
  await ev(`(() => {
    const s = document.createElement("style");
    s.textContent = "sat-toast, .toast, .toasts, .tour-offer { display: none !important; } * { caret-color: transparent !important; }";
    document.head.append(s);
    document.activeElement?.blur?.();
    getSelection().removeAllRanges();
    return true;
  })()`);
  // The panes fill the window: a shorter one leaves no empty area below the rows.
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 468, deviceScaleFactor: 2, mobile: false });
  await sleep(600);
  console.log("left:", await ev("tx.names(0)"), "\nright:", await ev("tx.names(1)"));
  const r = await ev(`(() => {
    const panes = [...document.querySelectorAll(".pane[data-pane]")].map((p) => p.getBoundingClientRect());
    const x = Math.min(...panes.map((p) => p.x)), y = Math.min(...panes.map((p) => p.y));
    return [x, y, Math.max(...panes.map((p) => p.right)) - x, Math.max(...panes.map((p) => p.bottom)) - y];
  })()`);
  const pad = 12;
  const shot = await send("Page.captureScreenshot", { format: "png", clip: { x: r[0] - pad, y: r[1] - pad, width: r[2] + 2 * pad, height: r[3] + 2 * pad, scale: 1 } });
  writeFileSync(join(OUT, "files.png"), Buffer.from(shot.data, "base64"));
  console.log("saved", r.map(Math.round));
} finally {
  await c.close();
  server.closeAllConnections();
  server.close();
}
