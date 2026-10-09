// Ctrl+click and Option/Alt+click on the drawn keys, and the shift glow,
// in the real page in headless Chrome over the DevTools protocol (no
// dependencies), with real mouse and key events. No ROM: the test makes
// the page believe a model runs (`booted` and `model` set, a blank frame
// with the annunciators it wants) and records the key presses the
// backend is sent. Skipped without Chrome (`SATURNUS_CHROME` names one)
// or the wasm package (`just web`). With `SATURNUS_ROM_DIR`, a 48SX booted
// from its `sxrom-j` takes Ctrl+click and Alt+click on √x as x² and ˣ√y,
// and quick clicks one after the other each get their shift once.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session as open, sleep } from "./chrome.mjs";
import { GLOW_DELAY } from "../shiftclick.js";

/**
 * Chrome and the server for a test (the ROM at /__rom when given), or
 * null when it skipped; `SATURNUS_AUDIT` makes a skip a failure.
 */
const session = (t, rom = null) => open(t, { files: rom ? { "/__rom": rom } : {}, audit: true });

/**
 * Make the page believe `model` runs, with the shift annunciators of
 * `ann`, and record what the backend is sent (`window.sent`).
 */
const fake = (model, ann = {}) => `(async () => {
  const s = window.saturnus;
  const b = s.backend;
  window.sent = [];
  b.keyDown = (k, shift = null) => window.sent.push("down " + k + (shift ? " after " + shift : ""));
  b.keyUp = (k) => window.sent.push("up " + k);
  b.keyUpAll = () => {};
  const rows = ${model === "42s" ? 16 : 64};
  const annunciators = { leftshift: false, rightshift: false, alpha: false, alert: false, busy: false, transmit: false, updown: false, battery: false, g: false, rad: false, ...${JSON.stringify(ann)} };
  s.store.set({ booted: "${model}", model: "${model}", frame: { width: 131, height: rows, pixels: btoa("\\0".repeat(17 * rows)), annunciators, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 } });
  const calc = document.querySelector("sat-calculator");
  for (let i = 0; i < 100 && !(calc.skinModel === "${model}" && calc.skinKey("${model === "49g" ? "nxt" : "enter"}")); i++) await new Promise((r) => setTimeout(r, 50));
  return calc.skinModel;
})()`;

/**
 * The page in headless Chrome at 1280 x 900, or null when the test
 * skipped. `platform` is `navigator.platform` (the page's Mac test):
 * "MacIntel" or "Linux x86_64", so both key conventions run on any host.
 */
async function page(t, rom = null, platform = null) {
  const c = await session(t, rom);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  if (platform) {
    const ua = await ev("navigator.userAgent");
    await send("Emulation.setUserAgentOverride", { userAgent: ua, platform });
  }
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  /** Modifier bits of the DevTools protocol: Alt 1, Ctrl 2, Meta 4, Shift 8. */
  const MOD = { alt: 1, ctrl: 2 };
  /** A real mouse click in the middle of key `name`, with `mod` ("ctrl", "alt") held. */
  const click = async (name, mod = null, button = "left") => {
    const [x, y] = await ev(`(() => { const r = document.querySelector("sat-calculator").skinKey(${JSON.stringify(name)}).querySelector(".cap").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    const modifiers = mod ? MOD[mod] : 0;
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, modifiers });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount: 1, modifiers });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount: 1, modifiers });
    await sleep(50);
  };
  const KEYS = { ctrl: ["Control", "ControlLeft", 17], alt: ["Alt", "AltLeft", 18] };
  /** Modifier `mod` down or up. */
  const modifier = async (mod, down) => {
    const [key, code, vk] = KEYS[mod];
    await send("Input.dispatchKeyEvent", { type: down ? "rawKeyDown" : "keyUp", key, code, windowsVirtualKeyCode: vk, modifiers: down ? MOD[mod] : 0 });
  };
  /** A letter key down and up with `mod` held. */
  const chord = async (mod, letter) => {
    const code = `Key${letter.toUpperCase()}`;
    const vk = letter.toUpperCase().charCodeAt(0);
    await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: letter, code, windowsVirtualKeyCode: vk, modifiers: MOD[mod] });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: letter, code, windowsVirtualKeyCode: vk, modifiers: MOD[mod] });
  };
  const sent = () => ev("window.sent");
  const glow = () => ev(`["glow-left", "glow-right"].filter((c) => document.querySelector(".skin svg").classList.contains(c))`);
  // The page's own clock: when a key last went down or up (`__keyAt`),
  // and when the glow lit (`__litAt`), so a slow machine can neither
  // fail a positive check nor pass a negative one by chance.
  await ev(`(() => {
    window.__keyAt = 0;
    window.__litAt = null;
    for (const type of ["keydown", "keyup"]) window.addEventListener(type, () => { window.__keyAt = performance.now(); }, true);
    new MutationObserver(() => {
      const lit = /glow-(left|right)/.test(document.querySelector(".skin svg").getAttribute("class") ?? "");
      if (lit && window.__litAt === null) window.__litAt = performance.now();
      if (!lit) window.__litAt = null;
    }).observe(document.querySelector(".skin svg"), { attributes: true, attributeFilter: ["class"] });
    return true;
  })()`);
  /** The glow once it is `want` (polled, at most 5 s). */
  const glowIs = async (want, what) => {
    const end = Date.now() + 5_000;
    let got = await glow();
    while (JSON.stringify(got) !== JSON.stringify(want) && Date.now() < end) {
      await sleep(50);
      got = await glow();
    }
    assert.deepEqual(got, want, what);
  };
  /** Wait until twice the glow's delay has passed on the page's clock since the last key. */
  const pastDelay = () => ev(`new Promise((r) => {
    const tick = () => (performance.now() - window.__keyAt >= ${2 * GLOW_DELAY} ? r(true) : setTimeout(tick, 20));
    tick();
  })`);
  return { ...c, click, modifier, chord, sent, glow, glowIs, pastDelay };
}

test("Ctrl+click and Alt+click press the key with the left or right shift", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("49g")), "49g");
  await p.click("nxt", "ctrl");
  assert.deepEqual(await p.sent(), ["down nxt after leftshift", "up nxt"], "Ctrl+click NXT: left shift, then NXT");

  await p.ev("window.sent = []");
  await p.click("nxt", "alt");
  assert.deepEqual(await p.sent(), ["down nxt after rightshift", "up nxt"], "Alt+click NXT: right shift, then NXT");

  // A Mac's Ctrl+click is a secondary click: the same press.
  await p.ev("window.sent = []");
  await p.click("nxt", "ctrl", "right");
  assert.deepEqual(await p.sent(), ["down nxt after leftshift", "up nxt"], "a secondary Ctrl+click");

  await p.ev("window.sent = []");
  await p.click("nxt");
  assert.deepEqual(await p.sent(), ["down nxt", "up nxt"], "a plain click");

  // A shift key itself is a plain press.
  await p.ev("window.sent = []");
  await p.click("leftshift", "ctrl");
  assert.deepEqual(await p.sent(), ["down leftshift", "up leftshift"]);

  // The shift on in the frame shown: the page still asks, the host's
  // queue decides when the press plays (a frame lags the queue).
  await p.ev(fake("49g", { leftshift: true }));
  await p.click("nxt", "ctrl");
  assert.deepEqual(await p.sent(), ["down nxt after leftshift", "up nxt"]);
});

test("on the 39G both modifiers ask for its one shift", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("39g")), "39g");
  await p.click("enter", "ctrl");
  assert.deepEqual(await p.sent(), ["down enter after shift", "up enter"]);
  await p.ev("window.sent = []");
  await p.click("enter", "alt");
  assert.deepEqual(await p.sent(), ["down enter after shift", "up enter"]);
});

for (const platform of ["MacIntel", "Linux x86_64"]) test(`a held modifier lights its labels; a chord and a blur do not keep them (${platform})`, { timeout: 120_000 }, async (t) => {
  const p = await page(t, null, platform);
  if (!p) return;
  assert.equal(await p.ev("navigator.platform"), platform);
  assert.equal(await p.ev(fake("49g")), "49g");
  await p.modifier("ctrl", true);
  await p.glowIs(["glow-left"], "Ctrl held: the left labels");
  // Not at once: lit no sooner than the delay after the key, on the page's clock.
  const lag = await p.ev("window.__litAt - window.__keyAt");
  assert.ok(lag >= GLOW_DELAY - 5, `lit ${lag} ms after the key`);
  assert.ok(await p.ev(`getComputedStyle(document.querySelector(".skin .shift-left")).filter.includes("glow-left")`), "the left labels glow");
  await p.modifier("ctrl", false);
  assert.deepEqual(await p.glow(), [], "out on its release");

  await p.modifier("alt", true);
  await p.glowIs(["glow-right"], "Alt held: the right labels");
  // The window losing the focus puts it out.
  await p.ev(`window.dispatchEvent(new Event("blur")), true`);
  assert.deepEqual(await p.glow(), [], "out on blur");
  await p.pastDelay();
  assert.deepEqual(await p.glow(), [], "and it stays out");
  await p.modifier("alt", false);

  // A quick chord never lights. Ctrl+J: bound to nothing on any
  // platform (Ctrl+K, the palette's key elsewhere, would open it and
  // take the keys from the calculator).
  await p.modifier("ctrl", true);
  await p.chord("ctrl", "j");
  await p.pastDelay();
  assert.deepEqual(await p.glow(), [], "Ctrl+J does not glow");
  await p.modifier("ctrl", false);
  assert.equal(await p.ev(`document.querySelectorAll("dialog[open]").length`), 0, "no dialog took the keys");

  // A mouse event without the modifier puts it out (its keyup was missed).
  await p.modifier("alt", true);
  await p.glowIs(["glow-right"], "Alt held again");
  await p.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: 5, y: 5, modifiers: 0 });
  assert.deepEqual(await p.glow(), [], "out when a move shows Alt up");
  await p.modifier("alt", false);

  // The 39G: either modifier lights its one shift's labels.
  assert.equal(await p.ev(fake("39g")), "39g");
  await p.modifier("alt", true);
  await p.glowIs(["glow-left"], "Alt on the 39G: its shift's labels");
  await p.modifier("alt", false);
});

test("a lone Alt's release is kept from the menu bar; typing in a field gets no glow", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("49g")), "49g");
  await p.ev(`window.altUp = null; window.addEventListener("keyup", (e) => { if (e.key === "Alt") window.altUp = e.defaultPrevented; }); true`);
  await p.modifier("alt", true);
  await p.modifier("alt", false);
  assert.equal(await p.ev("window.altUp"), true, "a lone Alt's keyup prevented");
  await p.modifier("alt", true);
  await p.chord("alt", "x");
  await p.modifier("alt", false);
  assert.equal(await p.ev("window.altUp"), false, "Alt+X's release left alone");

  // In a text field the keys are the field's.
  await p.ev(`(() => { const i = document.createElement("input"); i.id = "field"; document.body.append(i); i.focus(); return true; })()`);
  await p.modifier("ctrl", true);
  await p.pastDelay();
  assert.deepEqual(await p.glow(), [], "no glow while a field has the keys");
  await p.modifier("ctrl", false);
});

test("the 48SX ROM takes Ctrl+click √x as x² (twice in quick succession too) and Alt+click as ˣ√y", { timeout: 180_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "sxrom-j") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/sxrom-j not found");
    return;
  }
  const p = await page(t, rom);
  if (!p) return;
  // Idle: the CPU asleep with no key down, `ms` emulated ms on.
  await p.ev(`window.__idle = async (ms = 1500) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (;;) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  }; true`);
  await p.ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "sxrom-j"));
    return true;
  })()`);
  for (let i = 0; i < 150 && (await p.ev("window.saturnus.store.state.booted")) !== "48sx"; i++) await sleep(100);
  await p.ev("window.__idle()");
  // NO to "Try To Recover Memory?", then 3 ENTER, by clicks.
  for (const k of ["f", "3", "enter"]) {
    await p.click(k);
    await p.ev("window.__idle(300)");
  }
  const stack = () => p.ev("window.saturnus.backend.stack().then((s) => s.map((l) => l.text))");
  assert.deepEqual(await stack(), ["3"]);
  await p.click("sqrt", "ctrl");
  await p.ev("window.__idle(300)");
  assert.deepEqual(await stack(), ["9"], "Ctrl+click √x: x²");
  await p.click("2");
  await p.ev("window.__idle(300)");
  await p.click("sqrt", "alt");
  await p.ev("window.__idle(300)");
  assert.deepEqual(await stack(), ["3"], "Alt+click √x: the square root of 9");
  // Two quick Ctrl+clicks, the second before the first has played: x² twice.
  await p.click("sqrt", "ctrl");
  await p.click("sqrt", "ctrl");
  await p.ev("window.__idle(300)");
  await p.ev("window.__idle(300)");
  assert.deepEqual(await stack(), ["81"], "3 squared twice");
  // A click on the left shift, then at once a Ctrl+click: one shift, not two.
  await p.click("leftshift");
  await p.click("sqrt", "ctrl");
  await p.ev("window.__idle(300)");
  await p.ev("window.__idle(300)");
  assert.deepEqual(await stack(), ["6561"], "81 squared");
});
