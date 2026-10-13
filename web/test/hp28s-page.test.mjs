// The HP 28S in the real page in headless Chrome (iteration 43b): the
// clamshell side by side on a wide stage and stacked on a phone (no
// sideways scroll, keys no smaller than the 42S's there), the 137x32
// display; with `SATURNUS_ROM_DIR/hp28s.rom`, the calculator booted from
// the chosen file, 2 ENTER 3 + clicked with the mouse on the drawn keys,
// letters typed on the computer keyboard going to the letter keys, and a
// saved state restored. Skipped without Chrome or web/pkg (just web).
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
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
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

test("the clamshell: side by side on a wide stage, stacked on a phone", { timeout: 120_000 }, async (t) => {
  const wide = await open(t, 1280, 860);
  if (!wide) return;
  await show(wide.ev, "28s");
  assert.equal(await wide.ev("document.querySelector('.skin svg').classList.contains('stacked')"), false);
  const [ax, ay] = await wide.ev(box("lettera"));
  const [mx, my] = await wide.ev(box("a"));
  assert.ok(ax < mx && Math.abs(ay - my) < 200, "the letter half left of the display half");
  // The idle canvas has the 28S's shape: 137 columns, 32 rows and the strip.
  const [w, h] = await wide.ev("[document.querySelector('#lcd').width, document.querySelector('#lcd').height]");
  assert.ok(Math.abs(w / h - 137 / 40) < 0.05, `${w} x ${h}`);
  // Stacked on the same page once the stage is taller than wide.
  await wide.send("Emulation.setDeviceMetricsOverride", { width: 700, height: 1000, deviceScaleFactor: 1, mobile: false });
  await until(wide.ev, "document.querySelector('.skin svg').classList.contains('stacked')", 5_000, "stacked");
  await wide.close();

  const phone = await open(t, 390, 844, true);
  await show(phone.ev, "42s");
  const [, , w42] = await phone.ev(box("7"));
  await show(phone.ev, "28s");
  assert.equal(await phone.ev("document.querySelector('.skin svg').classList.contains('stacked')"), true);
  assert.ok(await phone.ev("document.scrollingElement.scrollWidth <= innerWidth"), "no sideways scroll");
  const [lx, ly] = await phone.ev(box("lettera"));
  const [, my2] = await phone.ev(box("a"));
  const [, , w28] = await phone.ev(box("7"));
  assert.ok(ly > my2, "the letter half below the display half");
  assert.ok(lx >= 0 && lx < 390);
  assert.ok(w28 >= w42 * 0.98, `the 28S's 7 is ${w28} px wide, the 42S's ${w42}`);
  // The card is left off stacked: nothing of it in the layout.
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

test("the 28S boots from its file, computes by mouse, types letters, restores a state", { timeout: 240_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "hp28s.rom") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/hp28s.rom not found");
    return;
  }
  const c = await open(t, 1280, 860, false, { "/__rom": rom });
  if (!c) return;
  const { send, ev } = c;
  await ev(HELPERS);
  await ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "hp28s.rom"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '28s'", 20_000, "booted");
  await ev("window.__idle(1500)");
  const frame = await ev("[window.saturnus.store.state.frame.width, window.saturnus.store.state.frame.height]");
  assert.deepEqual(frame, [137, 32]);
  // "Memory Lost" stays until a key: ON clears it.
  await ev("window.__press('on')");
  const empty = await ev(SCREEN);

  /** A click of the mouse on drawn key `name`. */
  const click = async (name) => {
    const [x, y, w, h] = await ev(box(name));
    const at = { x: x + w / 2, y: y + h / 2, button: "left", clickCount: 1 };
    await send("Input.dispatchMouseEvent", { type: "mousePressed", ...at });
    await sleep(80);
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", ...at });
    await ev("window.__idle()");
  };
  for (const k of ["2", "enter", "3", "plus"]) await click(k);
  const sum = await ev(SCREEN);
  assert.notEqual(sum, empty, "a result on the stack");
  // The same screen as 5 keyed in: 2 + 3 is 5.
  await click("drop");
  assert.equal(await ev(SCREEN), empty);
  for (const k of ["5", "enter"]) await click(k);
  assert.equal(await ev(SCREEN), sum, "2 ENTER 3 + shows what 5 ENTER does");

  // Letters from the computer keyboard go to the letter keys: A, b
  // (LC, B, LC), then Enter; the same as the drawn keys clicked.
  await click("drop");
  await click("drop");
  const type = async (key, code, shift = false) => {
    const k = { key, code, modifiers: shift ? 8 : 0 };
    await send("Input.dispatchKeyEvent", { type: "keyDown", ...k, text: key === "Enter" ? "\r" : key });
    await send("Input.dispatchKeyEvent", { type: "keyUp", ...k });
    await ev("window.__idle()");
  };
  await type("A", "KeyA", true);
  await type("b", "KeyB");
  await type("Enter", "Enter");
  const typed = await ev(SCREEN);
  assert.notEqual(typed, empty, "a name on the stack");
  await click("drop");
  for (const k of ["lettera", "lc", "letterb", "lc", "enter"]) await click(k);
  assert.equal(await ev(SCREEN), typed, "typed as the drawn letter keys");

  // A saved state restores: save, change, load.
  const controls = "document.querySelector('sat-controls')";
  await ev(`${controls}.saveState()`);
  assert.equal(await ev("window.saturnus.store.state.message"), "State saved.");
  await ev("window.__press('drop')");
  assert.notEqual(await ev(SCREEN), typed);
  await ev(`${controls}.loadState(); true`);
  await until(ev, "!!document.querySelector('dialog.confirm[open]')", 5_000, "the question");
  await ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === "Load").click(), true`);
  await until(ev, "/^Loaded the state saved on/.test(window.saturnus.store.state.message)", 10_000, "loaded");
  await ev("window.__idle()");
  assert.equal(await ev(SCREEN), typed, "the saved stack");

  // `,` types the 28S's comma, not its point: (3,4) is a complex number.
  await click("drop");
  await type("(", "Digit9", true);
  await type("3", "Digit3");
  await type(",", "Comma");
  await type("4", "Digit4");
  await type("Enter", "Enter");
  const complex = await ev(SCREEN);
  await click("drop");
  for (const k of ["lparen", "3", "comma", "4", "enter"]) await click(k);
  assert.equal(await ev(SCREEN), complex, "typed as the drawn comma");
  await click("drop");
  for (const k of ["lparen", "3", "point", "4", "enter"]) await click(k);
  assert.notEqual(await ev(SCREEN), complex, "not the point");
});
