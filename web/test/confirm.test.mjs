// The shared question before an action that cannot be undone
// (web/components/confirm.js), in the real page in headless Chrome: the
// answer for the action, Cancel, Escape and a click outside; Cancel has
// the focus and the focus goes back to the opener; the calculator takes
// no keys while it is open; on a phone it is a sheet at the bottom.
// Skipped without Chrome (`SATURNUS_CHROME` names one) or the wasm
// package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

async function page(t, width = 1280, height = 900, mobile = false) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  // An opener with the focus, and the question asked from it.
  const ask = () => ev(`(async () => {
    const m = await import("./components/confirm.js");
    let b = document.getElementById("opener");
    if (!b) { b = document.createElement("button"); b.id = "opener"; b.textContent = "Open"; document.body.append(b); }
    b.focus();
    window.__answer = undefined;
    m.confirmAction({ title: "Purge test?", body: "It is removed from HOME on the calculator. This can't be undone.", action: "Purge" }).then((a) => { window.__answer = a; });
    await new Promise((r) => setTimeout(r, 50));
    return true;
  })()`);
  const state = () => ev(`(() => {
    const d = document.querySelector("dialog.confirm");
    return { open: Boolean(d?.open), answer: window.__answer ?? null, focused: document.activeElement?.id || document.activeElement?.textContent || null,
      labelled: d ? document.getElementById(d.getAttribute("aria-labelledby"))?.textContent : null,
      described: d ? document.getElementById(d.getAttribute("aria-describedby"))?.textContent : null };
  })()`);
  const key = async (k, code, vk) => {
    await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: k, code, windowsVirtualKeyCode: vk });
    // Enter's character activates the focused button, as a keyboard's does.
    if (k === "Enter") await send("Input.dispatchKeyEvent", { type: "char", key: k, text: "\r" });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: vk });
    await sleep(100);
  };
  return { ...c, ask, state, key };
}

test("the answer: the action, Cancel, Escape, a click outside; the focus comes back", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ask();
  let s = await p.state();
  assert.deepEqual(s, { open: true, answer: null, focused: "Cancel", labelled: "Purge test?", described: "It is removed from HOME on the calculator. This can't be undone." }, "Cancel by default");
  assert.deepEqual(await p.ev(`[...document.querySelectorAll("dialog.confirm button")].map((b) => [b.textContent, b.className])`), [["Cancel", ""], ["Purge", "confirm-danger"]]);
  await p.ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === "Purge").click(), true`);
  await sleep(50);
  s = await p.state();
  assert.equal(s.answer, true);
  assert.equal(s.open, false);
  assert.equal(s.focused, "opener", "the focus back on the opener");

  await p.ask();
  await p.key("Enter", "Enter", 13); // Cancel has the focus
  assert.equal((await p.state()).answer, false, "Enter on Cancel");

  await p.ask();
  await p.key("Escape", "Escape", 27);
  s = await p.state();
  assert.equal(s.answer, false, "Escape cancels");
  assert.equal(s.focused, "opener");

  await p.ask();
  // A click on the backdrop: the dialog element itself, outside its box.
  await p.send("Input.dispatchMouseEvent", { type: "mousePressed", x: 10, y: 10, button: "left", clickCount: 1 });
  await p.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: 10, y: 10, button: "left", clickCount: 1 });
  await sleep(100);
  assert.equal((await p.state()).answer, false, "a click outside cancels");
  // A click inside the box does not.
  await p.ask();
  const [x, y] = await p.ev(`(() => { const r = document.querySelector(".confirm-box p").getBoundingClientRect(); return [r.x + 5, r.y + 5]; })()`);
  await p.send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
  await p.send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
  await sleep(100);
  assert.equal((await p.state()).open, true, "a click on the text keeps it");
  await p.key("Escape", "Escape", 27);
});

test("while it is open the calculator takes no keys", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`(() => {
    const s = window.saturnus.store;
    window.__keys = [];
    window.saturnus.backend.keyDown = (k) => window.__keys.push(k);
    window.saturnus.backend.keyUp = () => {};
    s.set({ booted: "48sx", model: "48sx", frame: { width: 131, height: 64, pixels: btoa("\\0".repeat(17 * 64)), annunciators: {}, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 } });
    return true;
  })()`);
  await p.ask();
  await p.key("1", "Digit1", 49);
  assert.deepEqual(await p.ev("window.__keys"), [], "no key reached the calculator");
  await p.key("Escape", "Escape", 27);
  await p.ev(`document.activeElement.blur(); true`);
  await p.key("1", "Digit1", 49);
  assert.deepEqual(await p.ev("window.__keys"), ["1"], "and afterwards they do");
});

test("on a phone it is a sheet at the bottom, inside the screen", { timeout: 120_000 }, async (t) => {
  const p = await page(t, 360, 780, true);
  if (!p) return;
  await p.ask();
  await sleep(400); // its opening animation
  const box = await p.ev(`(() => { const r = document.querySelector("dialog.confirm").getBoundingClientRect(); return { left: Math.round(r.left), right: Math.round(r.right), bottom: Math.round(r.bottom), w: innerWidth, h: innerHeight, over: document.documentElement.scrollWidth - document.documentElement.clientWidth, buttons: [...document.querySelectorAll("dialog.confirm button")].map((b) => Math.round(b.getBoundingClientRect().width)) }; })()`);
  assert.equal(box.left, 0);
  assert.equal(box.right, box.w);
  assert.equal(box.bottom, box.h, "at the bottom");
  assert.equal(box.over, 0);
  assert.equal(box.buttons[0], box.buttons[1], "the buttons share the width");
});
