// A drawn key held with the mouse stays down until the button comes up,
// in the real page in headless Chrome over the DevTools protocol, with
// real mouse events. The Lewis models show some functions only while
// their key is held (the 27S's MEM and SHOW, the 42S's function preview
// and NULL), so a press must not end before the button does, not even
// when frames arrive meanwhile. No ROM: the page believes a 42S runs and
// records the key presses the backend is sent. A guard: the page was not
// at fault in the held-key bug (the emulated chip was). Skipped without Chrome
// (`SATURNUS_CHROME` names one) or the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

/** A frame of the 42S's 131 x 16 display with every pixel `fill`. */
const frame = (fill) => `({ width: 131, height: 16, pixels: btoa(String.fromCharCode(${fill}).repeat(17 * 16)), annunciators: { leftshift: false, rightshift: false, alpha: false, alert: false, busy: false, transmit: false, updown: false, battery: false, g: false, rad: false }, contrast: 22, contrastRange: [15, 31], contrastDefault: 22 })`;

test("a key held with the mouse stays down until the button is released", { timeout: 120_000 }, async (t) => {
  const c = await session(t, { audit: true });
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const model = await ev(`(async () => {
    const s = window.saturnus;
    const b = s.backend;
    window.sent = [];
    b.keyDown = (k, shift = null) => window.sent.push("down " + k + (shift ? " after " + shift : ""));
    b.keyUp = (k) => window.sent.push("up " + k);
    b.keyUpAll = () => window.sent.push("up all");
    s.store.set({ booted: "42s", model: "42s", frame: ${frame(0)} });
    const calc = document.querySelector("sat-calculator");
    for (let i = 0; i < 100 && !(calc.skinModel === "42s" && calc.skinKey("sin")); i++) await new Promise((r) => setTimeout(r, 50));
    return calc.skinModel;
  })()`);
  assert.equal(model, "42s");
  const [x, y] = await ev(`(() => { const r = document.querySelector("sat-calculator").skinKey("sin").querySelector(".cap").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
  // Held for 1.5 s (the 42S's NULL comes after about a second), with
  // new frames arriving as the running calculator's would.
  for (let i = 0; i < 15; i++) {
    await ev(`window.saturnus.store.set({ frame: ${frame(i % 2 ? 255 : 0)} })`);
    await sleep(100);
  }
  assert.deepEqual(await ev("window.sent"), ["down sin"], "still held: no release before the button comes up");
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
  for (let i = 0; i < 50 && (await ev("window.sent.length")) < 2; i++) await sleep(50);
  assert.deepEqual(await ev("window.sent"), ["down sin", "up sin"], "released with the button");
});
