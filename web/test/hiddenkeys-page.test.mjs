// The 48GX's unprinted key functions on the drawn skin and the ON key's
// menu (iteration 65, web/hiddenkeys.js, web/onkeys.js), in the real
// page in headless Chrome with real mouse and key events. No ROM: the
// page believes a 48GX runs (as web/test/shiftclick-page.test.mjs) and
// the backend's key presses are recorded, the machine taking each key at
// once. Checked: a ghost label (SERVER above ▶) shows only while ⌥ is
// held, at full size, the printed SWAP hidden meanwhile; ⌃ lights SOLVE on 7;
// the tooltips; a right-click or a long press on ON opens its menu and
// sends no ON, a tap sends ON, ⌃-click CONT; the menu plays ON+C as held
// keys; Reset memory asks first; the 48SX keeps a plain ON. With
// `SATURNUS_ROM_DIR` (gxrom-r), the 48GX ROM's answers to the menu's
// Restart, Print display and Reset memory. Skipped without Chrome or
// web/pkg (`just web`). Screenshots of ⌃ and ⌥ held at 1280 and 390 wide
// go to `SATURNUS_SHOTS` when it names a folder.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { session as open, sleep } from "./chrome.mjs";

const session = (t, rom = null) => open(t, { files: rom ? { "/__rom": rom } : {}, audit: true });

/** Make the page believe `model` runs; the backend records presses and its machine takes them (`keysDown`). */
const fake = (model) => `(async () => {
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
  for (let i = 0; i < 100 && !(calc.skinModel === "${model}" && calc.skinKey("enter")); i++) await new Promise((r) => setTimeout(r, 50));
  await new Promise((r) => setTimeout(r, 100));
  return calc.skinModel;
})()`;

async function page(t, { width = 1280, height = 900, mobile = false, rom = null } = {}) {
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
  const menu = () => ev(`(() => { const m = document.querySelector('.menu[aria-label="ON combinations"]'); return m ? [...m.querySelectorAll('[role=menuitem]')].map((b) => b.textContent) : null; })()`);
  const pick = async (text) => {
    const [x, y] = await ev(`(() => { const b = [...document.querySelectorAll('.menu [role=menuitem]')].find((b) => b.textContent.startsWith(${JSON.stringify(text)})); const r = b.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
  };
  return { ...c, click, modifier, lit, visible, menu, pick, sent: () => ev("window.sent"), clear: () => ev("window.sent = []") };
}

/** A screenshot to `SATURNUS_SHOTS/name` when it is set. */
async function shot(p, name) {
  const dir = process.env.SATURNUS_SHOTS;
  if (!dir) return;
  const { data } = await p.send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(dir, name), Buffer.from(data, "base64"));
}

test("⌥ shows SERVER above ▶ at full size only while held, SWAP hidden meanwhile; ⌃ lights SOLVE on 7", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48gx")), "48gx");
  assert.equal(await p.visible('[data-ghost="right"]'), false, "no ghost at rest");
  assert.equal(await p.visible('[data-ghost="down"]'), false);
  assert.equal(await p.ev(`document.querySelector('.skin [data-ghost="right"]').textContent`), "SERVER");
  assert.equal(await p.ev(`document.querySelector('.skin [data-ghost="right"]').getAttribute("fill")`), "none", "outlined, not print");

  await p.modifier("alt", true);
  await p.lit("glow-right");
  assert.equal(await p.visible('[data-ghost="right"]'), true, "⌥: SERVER");
  assert.equal(await p.visible('[data-ghost="down"]'), true, "⌥: REVIEW above ▼");
  const swap = `[...document.querySelectorAll(".skin .print text")].find((n) => n.textContent === "SWAP")`;
  assert.equal(await p.ev(`getComputedStyle(${swap}).visibility`), "hidden", "SWAP hidden while ⌥ is held");
  // At full size in the key's whole label area: centred on the key, not squeezed.
  const box = await p.ev(`(() => {
    const t = document.querySelector('.skin [data-ghost="right"]');
    const g = t.getBBox();
    const k = document.querySelector("sat-calculator").skinKey("right").querySelector(".cap path").getBBox();
    return { squeezed: t.hasAttribute("textLength"), size: t.getAttribute("font-size"), mid: g.x + g.width / 2, keyMid: k.x + k.width / 2, width: g.width, keyWidth: k.width };
  })()`);
  assert.equal(box.squeezed, false, "not squeezed");
  assert.equal(box.size, await p.ev(`${swap}.getAttribute("font-size")`), "the print's size");
  assert.ok(Math.abs(box.mid - box.keyMid) < 1, `centred: ${JSON.stringify(box)}`);
  assert.ok(box.width > box.keyWidth, "wider than the key, as SERVER is at full size");
  await shot(p, "hidden-keys-alt-1280.png");
  await p.modifier("alt", false);
  await p.lit(null);
  assert.equal(await p.visible('[data-ghost="right"]'), false, "gone on release");
  assert.equal(await p.ev(`getComputedStyle(${swap}).visibility`), "visible", "SWAP back on release");

  await p.modifier("ctrl", true);
  await p.lit("glow-left");
  assert.equal(await p.visible('[data-ghost="right"]'), false, "⌃: no right ghost");
  const solve = `[...document.querySelectorAll(".skin .print text")].find((n) => n.textContent === "SOLVE")`;
  assert.ok(await p.ev(`getComputedStyle(${solve}).filter.includes("glow-left")`), "⌃: SOLVE lit in the left shift's colour");
  assert.equal(await p.ev(`getComputedStyle(${solve}).opacity`), "1", "and not faded");
  // The fade's transition.
  await sleep(600);
  const matrix = `[...document.querySelectorAll(".skin .print text")].find((n) => n.textContent === "MATRIX")`;
  assert.equal(await p.ev(`getComputedStyle(${matrix}).opacity`), "0.4", "a right label with no menu fades");
  await shot(p, "hidden-keys-ctrl-1280.png");
  await p.modifier("ctrl", false);

  const title = (k) => p.ev(`document.querySelector("sat-calculator").skinKey(${JSON.stringify(k)}).querySelector("title").textContent`);
  assert.equal(await title("right"), "▶ (RIGHT, alpha R)\n⌃-click: SWAP\n⌥-click: Kermit server (not printed)");
  assert.equal(await title("7"), "7\n⌃-click: SOLVE menu\n⌥-click: SOLVE");
  assert.equal(await title("eex"), "EEX (alpha Z)\n⌃-click: PURG\n⌥-click: ARG (LASTARG)");
  assert.equal(await title("on"), "ON\n⌃-click: CONT\n⌥-click: OFF\nLong press or right-click: Restart, Self-test, Print display, Reset memory");
});

test("the phone width: ⌃ and ⌥ held", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 390, height: 844, mobile: true });
  if (!p) return;
  assert.equal(await p.ev(fake("48gx")), "48gx");
  await p.modifier("alt", true);
  await p.lit("glow-right");
  assert.equal(await p.visible('[data-ghost="right"]'), true, "the ghost fits at any width (skin units)");
  await shot(p, "hidden-keys-alt-390.png");
  await p.modifier("alt", false);
  await p.modifier("ctrl", true);
  await p.lit("glow-left");
  await shot(p, "hidden-keys-ctrl-390.png");
  await p.modifier("ctrl", false);
});

test("ON: a right-click or a long press opens its menu and sends nothing; a tap is ON", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48gx")), "48gx");
  await p.click("on", { button: "right" });
  assert.deepEqual(await p.menu(), ["RestartON+C", "Self-testON+D", "Print displayON+1", "Reset memory…ON+A+F"]);
  assert.deepEqual(await p.sent(), [], "a right-click sends no ON");
  await p.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await sleep(100);
  assert.equal(await p.menu(), null);

  await p.click("on", { ms: 800 });
  assert.notEqual(await p.menu(), null, "a long press opens it");
  assert.deepEqual(await p.sent(), [], "and sends no ON");
  await p.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
  await sleep(100);

  await p.click("on");
  assert.deepEqual(await p.sent(), ["down on", "up on"], "a tap: ON");
  assert.equal(await p.menu(), null);
  await p.clear();
  await p.click("on", { mod: "ctrl" });
  assert.deepEqual(await p.sent(), ["down on after leftshift", "up on"], "⌃-click: CONT");
  await p.clear();
  await p.click("on", { mod: "ctrl", button: "right" });
  assert.deepEqual(await p.sent(), ["down on after leftshift", "up on"], "a Mac's ⌃-click (a secondary click): CONT, no menu");
  assert.equal(await p.menu(), null);

  // The menu plays ON+C as held keys: C down while ON is, released first.
  await p.clear();
  await p.click("on", { button: "right" });
  await p.pick("Restart");
  for (let i = 0; i < 50 && (await p.sent()).length < 4; i++) await sleep(50);
  assert.deepEqual(await p.sent(), ["down on", "down c", "up c", "up on"]);

  // Reset memory asks first; Cancel presses nothing.
  await p.clear();
  await p.click("on", { button: "right" });
  await p.pick("Reset memory");
  for (let i = 0; i < 50 && !(await p.ev(`!!document.querySelector("dialog.confirm[open]")`)); i++) await sleep(50);
  assert.equal(await p.ev(`document.querySelector("dialog.confirm h2").textContent`), "Reset the calculator's memory?");
  await p.ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === "Cancel").click(), true`);
  await sleep(200);
  assert.deepEqual(await p.sent(), []);

  // The same four in the command palette.
  await p.ev(`(async () => {
    const pal = window.saturnus.palette;
    await pal.open();
    const i = pal.querySelector("input");
    i.value = "on key combination";
    i.dispatchEvent(new Event("input"));
    return true;
  })()`);
  await sleep(300);
  const names = await p.ev(`[...document.querySelectorAll(".palette .prow-action .prow-name")].map((n) => n.textContent)`);
  for (const name of ["Restart (ON+C)", "Self-test (ON+D)", "Print display (ON+1)", "Reset memory (ON+A+F)…"]) assert.ok(names.includes(name), `${name} in ${JSON.stringify(names)}`);
  await p.ev("window.saturnus.palette.close(), true");
});

test("a hand holding ON: another key pressed while ON is down, also after its menu opened, sends ON held, then the key", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48gx")), "48gx");
  const [ox, oy] = await p.ev(`(() => { const r = document.querySelector("sat-calculator").skinKey("on").querySelector(".cap").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  const [cx, cy] = await p.ev(`(() => { const r = document.querySelector("sat-calculator").skinKey("c").querySelector(".cap").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  // Two fingers: ON held 700 ms (its menu opens), then C.
  await p.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: ox, y: oy, id: 1 }] });
  await sleep(700);
  assert.notEqual(await p.menu(), null, "the menu opened");
  assert.deepEqual(await p.sent(), []);
  await p.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: ox, y: oy, id: 1 }, { x: cx, y: cy, id: 2 }] });
  await sleep(100);
  assert.deepEqual((await p.sent()).slice(0, 2), ["down on", "down c"], "ON held, then C");
  assert.equal(await p.menu(), null, "the menu closed");
  await p.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [{ x: ox, y: oy, id: 1 }] });
  await p.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await sleep(150);
  assert.deepEqual((await p.sent()).slice(2).sort(), ["up c", "up on"]);

  // The keyboard: the mouse holds ON past its menu, F3 (the C menu key) comes.
  await p.clear();
  await p.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: ox, y: oy });
  await p.send("Input.dispatchMouseEvent", { type: "mousePressed", x: ox, y: oy, button: "left", clickCount: 1 });
  await sleep(700);
  assert.notEqual(await p.menu(), null);
  await p.send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "F3", code: "F3", windowsVirtualKeyCode: 114 });
  await sleep(100);
  assert.deepEqual((await p.sent()).slice(0, 2), ["down on", "down c"], "ON held, then C from the keyboard");
  await p.send("Input.dispatchKeyEvent", { type: "keyUp", key: "F3", code: "F3", windowsVirtualKeyCode: 114 });
  await p.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: ox, y: oy, button: "left", clickCount: 1 });
  await sleep(150);
  assert.deepEqual((await p.sent()).slice(2).sort(), ["up c", "up on"]);

  // A blur drops a waiting press: no menu opens in the background.
  await p.clear();
  await p.send("Input.dispatchMouseEvent", { type: "mousePressed", x: ox, y: oy, button: "left", clickCount: 1 });
  await p.ev(`window.dispatchEvent(new Event("blur")), true`);
  await sleep(700);
  assert.equal(await p.menu(), null, "no menu after a blur");
  await p.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: ox, y: oy, button: "left", clickCount: 1 });
  await sleep(100);
  assert.ok(!(await p.sent()).includes("down on"), "and no ON");
});

test("the 48SX has no ON menu: a right-click on ON is a press as before", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48sx")), "48sx");
  await p.click("on", { button: "right" });
  assert.equal(await p.menu(), null);
  assert.deepEqual(await p.sent(), ["down on", "up on"]);
  assert.equal(await p.ev(`document.querySelectorAll(".skin .ghost").length`), 0);
});

test("the 48GX ROM: Restart clears the stack and keeps the variables, Print display transmits, Reset memory asks the ROM's question", { timeout: 240_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "gxrom-r") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/gxrom-r not found");
    return;
  }
  const p = await page(t, { rom });
  if (!p) return;
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
    await window.saturnus.startWithRom(new File([bytes], "gxrom-r"));
    return true;
  })()`);
  for (let i = 0; i < 150 && (await p.ev("window.saturnus.store.state.booted")) !== "48gx"; i++) await sleep(100);
  await p.ev("window.__idle(3000)");
  const stack = () => p.ev("window.saturnus.backend.stack().then((s) => s.map((l) => l.text))");
  const vars = () => p.ev("window.saturnus.backend.memoryTree().then((t) => t.variables.map((v) => v.name))");
  await p.ev(`window.saturnus.backend.run("42 'SATX' STO 1 2")`);
  await p.ev("window.__idle(500)");
  assert.deepEqual(await stack(), ["2", "1"]);
  assert.ok((await vars()).includes("SATX"));

  // Restart (ON+C): the stack cleared, the variables kept.
  await p.click("on", { button: "right" });
  await p.pick("Restart");
  await sleep(500);
  await p.ev("window.__idle(2000)");
  assert.deepEqual(await stack(), [], "ON+C cleared the stack");
  assert.ok((await vars()).includes("SATX"), "and kept SATX");

  // Print display (ON+1): the transmit annunciator lights.
  await p.ev(`window.__tx = false; window.saturnus.store.watch(["frame"], (s) => { if (s.frame?.annunciators?.transmit) window.__tx = true; }); true`);
  await p.click("on", { button: "right" });
  await p.pick("Print display");
  for (let i = 0; i < 100 && !(await p.ev("window.__tx")); i++) await sleep(50);
  assert.equal(await p.ev("window.__tx"), true, "ON+1 transmits");
  await p.ev("window.__idle(2000)");

  // Reset memory (ON+A+F), asked first; NO to the ROM's "Try To Recover Memory?" clears it.
  await p.click("on", { button: "right" });
  await p.pick("Reset memory");
  for (let i = 0; i < 50 && !(await p.ev(`!!document.querySelector("dialog.confirm[open]")`)); i++) await sleep(50);
  await p.ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === "Reset memory").click(), true`);
  await sleep(1000);
  await p.ev("window.__idle(3000)");
  await p.click("f");
  await p.ev("window.__idle(3000)");
  assert.ok(!(await vars()).includes("SATX"), "SATX gone after NO");
});
