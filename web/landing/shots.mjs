// The calculator images of the landing page: each model with its real
// ROM running, drawn by the emulator page in headless Chrome (the page
// tests' chrome.mjs), the case on a transparent background at device
// scale 2, into web/landing/raw/ (gitignored). Then, for img/:
//   for m in 48sx 48gx 49g 38g 39g 40g 42s; do magick raw/$m.png -trim +repage t.png;
//     for w in 320 640; do magick t.png -resize ${w}x -quality 82 -define webp:alpha-quality=90 img/$m-$w.webp; done; done
//   for w in 560 1120; do magick raw/48gx.png -trim +repage -resize ${w}x -quality 84 -define webp:alpha-quality=90 img/48gx-hero-$w.webp; done
// Usage: SATURNUS_ROM_DIR=../../roms node shots.mjs [model ...]
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { serve, chrome, findChrome, sleep } from "../test/chrome.mjs";

const ROMS = process.env.SATURNUS_ROM_DIR;
if (!ROMS) throw new Error("SATURNUS_ROM_DIR names the ROM directory");
const OUT = new URL("raw/", import.meta.url).pathname;
mkdirSync(OUT, { recursive: true });

/** Per model: the ROM file, and what to do once it has booted (the page answers the boot prompts itself). */
const SCENES = {
  "48sx": { rom: "sxrom-j", run: ["'X^2+1' 355 113 / 6 7 *"] },
  "48gx": { rom: "gxrom-r", run: ["RAD 'SIN(X)' STEQ ERASE DRAX DRAW PICTURE"] },
  "49g": { rom: "rom.49g", run: ["CF(-95)", "'X^2+1' 355 113 / 6 7 *"] },
  "38g": { rom: "38G_A167.ROM", keys: ["2", "plus", "3", "enter", "sqrt", "2", "enter"] },
  "39g": { rom: "rom.39g", keys: ["2", "plus", "3", "enter", "3", "square", "enter"] },
  "40g": { rom: "rom.39g", keys: ["2", "plus", "3", "enter", "3", "square", "enter"] },
  "42s": { rom: "hp42s-c.rom", keys: ["3", "5", "5", "enter", "1", "1", "3", "divide"] },
};

const models = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(SCENES);
const files = Object.fromEntries(Object.values(SCENES).map((s) => [`/__rom/${s.rom}`, join(ROMS, s.rom)]));
const { server, port } = await serve(files);
const binary = findChrome();
if (!binary) throw new Error("no Chrome");

for (const model of models) {
  const scene = SCENES[model];
  const c = await chrome(binary);
  const { send, ev } = c;
  try {
    await send("Page.enable");
    await send("Runtime.enable");
    await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 1400, deviceScaleFactor: 2, mobile: false });
    await send("Emulation.setDefaultBackgroundColorOverride", { color: { r: 0, g: 0, b: 0, a: 0 } });
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
    await ev("window.saturnus.started.then(() => true)");
    await ev(`window.saturnus.store.set({ model: ${JSON.stringify(model)} }); true`);
    await sleep(200);
    await ev(`(async () => {
      const bytes = await (await fetch("/__rom/${scene.rom}")).arrayBuffer();
      await window.saturnus.startWithRom(new File([bytes], ${JSON.stringify(scene.rom)}));
      return true;
    })()`);
    for (let i = 0; i < 300 && (await ev("window.saturnus.store.state.booted")) !== model; i++) await sleep(100);
    const booted = await ev("window.saturnus.store.state.booted");
    if (booted !== model) throw new Error(`${model}: booted ${booted}`);
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
    await sleep(500);
    await ev("window.__idle(1000)");
    for (const text of scene.run ?? []) {
      await ev(`window.saturnus.backend.run(${JSON.stringify(text)}).then(() => true)`);
      await ev("window.__idle(800)");
    }
    for (const k of scene.keys ?? []) {
      await ev(`window.saturnus.backend.keyDown(${JSON.stringify(k)}); true`);
      await sleep(120);
      await ev(`window.saturnus.backend.keyUp(${JSON.stringify(k)}); true`);
      await ev("window.__idle(500)");
    }
    await sleep(800);
    await ev("window.saturnus.store.set({ storageOffer: null }); true");
    // Transparent around the case; no toasts, no cursor blink mid-frame.
    await ev(`(() => {
      const s = document.createElement("style");
      s.textContent = "html, body, .stage, #stage { background: transparent !important; } sat-toast, .toast, .toasts { display: none !important; }";
      document.head.append(s);
      return true;
    })()`);
    await sleep(300);
    console.log(model, "screen:\n" + (await ev("window.saturnus.screenText()")).split("\n").slice(0, 12).join("\n"));
    const r = await ev(`(() => { const r = document.querySelector(".skin svg").getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })()`);
    const pad = 36;
    const shot = await send("Page.captureScreenshot", { format: "png", clip: { x: r[0] - pad, y: r[1] - pad, width: r[2] + 2 * pad, height: r[3] + 2 * pad, scale: 1 }, captureBeyondViewport: true });
    writeFileSync(join(OUT, `${model}.png`), Buffer.from(shot.data, "base64"));
    console.log(model, "saved", r.map(Math.round));
  } catch (e) {
    console.error(model, "failed:", e.message);
  } finally {
    await c.close();
  }
}
server.closeAllConnections();
server.close();
