// The 48GX's unprinted key functions on the drawn skin and the ON key's
// menu (iteration 65, web/hiddenkeys.js, web/onkeys.js), in the real
// page in headless Chrome with real mouse and key events. No ROM: the
// page believes a 48GX runs (as web/test/shiftclick-page.test.mjs) and
// the backend's key presses are recorded, the machine taking each key at
// once. Checked: a ghost label (SERVER above ▶) shows only while ⌥ is
// held, at full size, the printed SWAP hidden meanwhile; ⌃ lights SOLVE on 7;
// the tooltips; a right-click or a long press on ON opens its menu and
// sends no ON, a tap sends ON, ⌃-click CONT; the menu plays ON+C as held
// keys; Reset memory asks first. With
// `SATURNUS_ROM_DIR` (gxrom-r), the 48GX ROM's answers to the menu's
// Restart, Print display and Reset memory. The other models:
// web/test/hiddenkeys-models-page.test.mjs. Skipped without Chrome or
// web/pkg (`just web`). Screenshots of ⌃ and ⌥ held at 1280 and 390 wide
// go to `SATURNUS_SHOTS` when it names a folder.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { sleep } from "./chrome.mjs";
import { fake, page, shot } from "./hiddenkeys-helpers.mjs";

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
