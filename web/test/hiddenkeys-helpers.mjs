// The page harness of the hidden-keys page tests
// (web/test/hiddenkeys-page.test.mjs, hiddenkeys-models-page.test.mjs):
// the real page in headless Chrome (web/test/chrome.mjs) with a Mac's
// signs in the tooltips, real mouse and key events on the drawn keys, a
// fake running model whose backend records the presses, and screenshots
// to `SATURNUS_SHOTS`.
import assert from "node:assert/strict";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { session as open, sleep } from "./chrome.mjs";

const session = (t, rom = null) => open(t, { files: rom ? { "/__rom": rom } : {}, audit: true });

/** Make the page believe `model` runs; the backend records presses and its machine takes them (`keysDown`). */
export const fake = (model) => `(async () => {
  const s = window.saturnus;
  const b = s.backend;
  window.sent = [];
  const down = new Set();
  const show = () => setTimeout(() => s.store.set({ keysDown: [...down] }), 5);
  b.keyDown = (k, shift = null) => { window.sent.push("down " + k + (shift ? " after " + shift : "")); down.add(k); show(); };
  b.keyUp = (k) => { window.sent.push("up " + k); down.delete(k); show(); };
  b.keyUpAll = () => { window.sent.push("up all"); down.clear(); show(); };
  const annunciators = { leftshift: false, rightshift: false, alpha: false, alert: false, busy: false, transmit: false };
  s.store.set({ booted: "${model}", model: "${model}", running: true, frame: { width: 131, height: 64, pixels: btoa("\\0".repeat(17 * 64)), annunciators, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 } });
  const calc = document.querySelector("sat-calculator");
  for (let i = 0; i < 100 && !(calc.skinModel === "${model}" && calc.skinKey("on")); i++) await new Promise((r) => setTimeout(r, 50));
  // A blank frame of the model's own display size.
  const w = calc.idleColumns, h = calc.idleRows;
  s.store.set({ frame: { ...s.store.state.frame, width: w, height: h, pixels: btoa("\\0".repeat(Math.ceil(w / 8) * h)) } });
  calc.fit();
  await new Promise((r) => setTimeout(r, 100));
  return calc.skinModel;
})()`;

export async function page(t, { width = 1280, height = 900, mobile = false, rom = null } = {}) {
  const c = await session(t, rom);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: mobile ? 2 : 1, mobile });
  // The Mac's signs in the tooltips, on any host.
  const ua = await ev("navigator.userAgent");
  await send("Emulation.setUserAgentOverride", { userAgent: ua, platform: "MacIntel" });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const MOD = { alt: 1, ctrl: 2 };
  const center = (name) => ev(`(() => { const r = document.querySelector("sat-calculator").skinKey(${JSON.stringify(name)}).querySelector(".cap").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  /** A mouse press on key `name`, held `ms`, with `mod` held, by `button`. */
  const click = async (name, { mod = null, button = "left", ms = 0 } = {}) => {
    const [x, y] = await center(name);
    const modifiers = mod ? MOD[mod] : 0;
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, modifiers });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount: 1, modifiers });
    if (ms) await sleep(ms);
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount: 1, modifiers });
    await sleep(80);
  };
  const KEYS = { ctrl: ["Control", "ControlLeft", 17], alt: ["Alt", "AltLeft", 18] };
  const modifier = async (mod, down) => {
    const [key, code, vk] = KEYS[mod];
    await send("Input.dispatchKeyEvent", { type: down ? "rawKeyDown" : "keyUp", key, code, windowsVirtualKeyCode: vk, modifiers: down ? MOD[mod] : 0 });
  };
  const lit = async (want) => {
    for (let i = 0; i < 100; i++) {
      const cls = await ev(`document.querySelector(".skin svg").getAttribute("class") ?? ""`);
      if (want ? cls.includes(want) : !/glow-/.test(cls)) return;
      await sleep(50);
    }
    assert.fail(`glow ${want}`);
  };
  /** Computed visibility of the first element of `selector` in the skin. */
  const visible = (selector) => ev(`(() => { const n = document.querySelector(".skin svg " + ${JSON.stringify(selector)}); return n ? getComputedStyle(n).visibility === "visible" : null; })()`);
  const menu = () => ev(`(() => { const m = document.querySelector('.menu[aria-label$=" combinations"]'); return m ? [...m.querySelectorAll('[role=menuitem]')].map((b) => b.textContent) : null; })()`);
  const pick = async (text) => {
    const [x, y] = await ev(`(() => { const b = [...document.querySelectorAll('.menu [role=menuitem]')].find((b) => b.textContent.startsWith(${JSON.stringify(text)})); const r = b.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
  };
  return { ...c, click, modifier, lit, visible, menu, pick, sent: () => ev("window.sent"), clear: () => ev("window.sent = []") };
}

/** A screenshot to `SATURNUS_SHOTS/name` when it is set. */
export async function shot(p, name) {
  const dir = process.env.SATURNUS_SHOTS;
  if (!dir) return;
  const { data } = await p.send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(dir, name), Buffer.from(data, "base64"));
}
