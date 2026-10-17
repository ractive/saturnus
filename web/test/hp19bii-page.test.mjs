// The HP 19BII in the real page in headless Chrome (iteration 44): the
// clamshell side by side on a wide stage and stacked on a phone (no
// sideways scroll), the 137x32 display, the no-ROM text; with
// `SATURNUS_ROM_DIR/hp19bii.rom`, the calculator booted from the chosen
// file, English chosen at its language prompt, 2 + 3 = clicked with the
// mouse on the drawn keys giving what 5 gives, letters typed on the
// computer keyboard going to the letter keys, and a saved state restored.
// Skipped without Chrome or web/pkg (just web).
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
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

/** The page at `width` x `height` (a phone with touch when `phone`). */
async function open(t, width, height, phone = false, files = undefined) {
  const c = await session(t, files ? { files } : undefined);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: phone ? 2 : 1, mobile: phone });
  if (phone) await send("Emulation.setTouchEmulationEnabled", { enabled: true });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started.then(() => true)");
  return c;
}

/** Show `model` (no ROM needed) and wait for its skin. */
async function show(ev, model) {
  await ev(`window.saturnus.store.set({ model: ${JSON.stringify(model)} }); true`);
  await until(ev, `!!document.querySelector('[data-key]') && document.querySelector('.skin svg').getAttribute('aria-label') === ${JSON.stringify(`HP ${model.toUpperCase()} keyboard`)}`, 10_000, `the ${model} skin`);
  await sleep(300);
}

const box = (name) => `(() => { const r = document.querySelector('[data-key="${name}"]').getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })()`;

test("the 19BII's clamshell: side by side on a wide stage, stacked on a phone", { timeout: 120_000 }, async (t) => {
  const wide = await open(t, 1280, 860);
  if (!wide) return;
  await show(wide.ev, "19bii");
  assert.equal(await wide.ev("document.querySelector('.skin svg').classList.contains('stacked')"), false);
  const [ax, ay] = await wide.ev(box("lettera"));
  const [mx, my] = await wide.ev(box("a"));
  assert.ok(ax < mx && Math.abs(ay - my) < 200, "the letter half left of the display half");
  const [w, h] = await wide.ev("[document.querySelector('#lcd').width, document.querySelector('#lcd').height]");
  assert.ok(Math.abs(w / h - 137 / 40) < 0.05, `${w} x ${h}`);
  // No download and no way to read it out: the page says so.
  const text = await wide.ev("document.querySelector('sat-calculator').shadowRoot?.textContent ?? document.body.textContent");
  assert.match(text, /saturnus has no way to get this ROM for you/);
  await wide.close();

  const phone = await open(t, 390, 844, true);
  await show(phone.ev, "19bii");
  assert.equal(await phone.ev("document.querySelector('.skin svg').classList.contains('stacked')"), true);
  assert.ok(await phone.ev("document.scrollingElement.scrollWidth <= innerWidth"), "no sideways scroll");
  const [lx, ly] = await phone.ev(box("lettera"));
  const [, my2] = await phone.ev(box("a"));
  assert.ok(ly > my2, "the letter half below the display half");
  assert.ok(lx >= 0 && lx < 390);
  assert.equal(await phone.ev("[...document.querySelectorAll('.skin .card')].some((e) => e.getBoundingClientRect().width > 0)"), false);
});

const HELPERS = `
  window.__idle = async (ms = 600) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (;;) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  };
  window.__press = async (key) => {
    const b = window.saturnus.backend;
    b.keyDown(key);
    await new Promise((r) => setTimeout(r, 80));
    b.keyUp(key);
    await window.__idle();
  };
  true`;
const SCREEN = "document.querySelector('sat-calculator').screenText()";

test("the 19BII boots from its file, computes by mouse, types letters, restores a state", { timeout: 240_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "hp19bii.rom") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/hp19bii.rom not found");
    return;
  }
  const c = await open(t, 1280, 860, false, { "/__rom": rom });
  if (!c) return;
  const { send, ev } = c;
  await ev(HELPERS);
  await ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "hp19bii.rom"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '19bii'", 20_000, "booted");
  await ev("window.__idle(1500)");
  const frame = await ev("[window.saturnus.store.state.frame.width, window.saturnus.store.state.frame.height]");
  assert.deepEqual(frame, [137, 32]);

  /** A click of the mouse on drawn key `name`. */
  const click = async (name) => {
    const [x, y, w, h] = await ev(box(name));
    const at = { x: x + w / 2, y: y + h / 2, button: "left", clickCount: 1 };
    await send("Input.dispatchMouseEvent", { type: "mousePressed", ...at });
    await sleep(80);
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", ...at });
    await ev("window.__idle()");
  };
  // The language prompt: the sixth menu key is English; then 0.00.
  await click("f");
  const zero = await ev(SCREEN);
  for (const k of ["2", "plus", "3", "equals"]) await click(k);
  const sum = await ev(SCREEN);
  assert.notEqual(sum, zero, "a result");
  // The same screen as 5 = gives: 2 + 3 is 5.
  for (const k of ["shift", "backspace"]) await click(k);
  for (const k of ["5", "equals"]) await click(k);
  assert.equal(await ev(SCREEN), sum, "2 + 3 = shows what 5 = does");

  // Letters from the computer keyboard go to the letter keys, a
  // lowercase letter as its capital: in TEXT (the sixth MAIN menu),
  // "Ab" types the same as the drawn A and B.
  for (const k of ["shift", "exit", "f"]) await click(k);
  const type = async (key, code, shift = false) => {
    const k = { key, code, modifiers: shift ? 8 : 0 };
    await send("Input.dispatchKeyEvent", { type: "keyDown", ...k, text: key });
    await send("Input.dispatchKeyEvent", { type: "keyUp", ...k });
    await ev("window.__idle()");
  };
  // The text line's cursor blinks: a line is compared until it matches.
  const shows = (want, what) => until(ev, `${SCREEN} === ${JSON.stringify(want)}`, 4_000, what);
  const before = await ev(SCREEN);
  await type("A", "KeyA", true);
  await type("b", "KeyB");
  const typed = await ev(SCREEN);
  assert.notEqual(typed, before, "letters on the line");
  await click("backspace");
  await click("backspace");
  await shows(before, "the letters deleted");
  for (const k of ["lettera", "letterb"]) await click(k);
  await shows(typed, "typed as the drawn letter keys");

  // A saved state restores: save, change, load.
  const controls = "document.querySelector('sat-controls')";
  await ev(`${controls}.saveState()`);
  assert.equal(await ev("window.saturnus.store.state.message"), "State saved.");
  await ev("window.__press('letterc')");
  assert.notEqual(await ev(SCREEN), typed);
  await ev(`${controls}.loadState(); true`);
  await until(ev, "!!document.querySelector('dialog.confirm[open]')", 5_000, "the question");
  await ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === "Load").click(), true`);
  await until(ev, "/^Loaded the state saved on/.test(window.saturnus.store.state.message)", 10_000, "loaded");
  await ev("window.__idle()");
  await shows(typed, "the saved line");
});
