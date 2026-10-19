// Search on the HP 42S in the real page in headless Chrome (iteration
// 54): with `SATURNUS_ROM_DIR/hp42s.rom`, the calculator booted from the
// chosen file, a function chosen in Search pressed on it: 9, then √x from
// Search, shows what 9 √x pressed on the drawn keys shows; with a menu
// open, Search leaves it first; →POL (⇧ CONVERT →POL) shows what its
// keys show; a pending shift is cancelled first. Skipped without Chrome or web/pkg (just web).
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
/** Whether the display shows a menu: the boxes' border rows (9 and 15) dark under the first box. */
const MENU = `(() => { const t = ${SCREEN}.split("\\n"); return [9, 15].every((y) => t[y].slice(0, 21) === "#".repeat(21)); })()`;

test("a 42S function chosen in Search is pressed on the running calculator", { timeout: 240_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "hp42s.rom") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/hp42s.rom not found");
    return;
  }
  const c = await session(t, { files: { "/__rom": rom } });
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 860, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started.then(() => true)");
  await ev(HELPERS);
  await ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "hp42s.rom"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '42s'", 20_000, "booted");
  await ev("window.__idle(1500)");
  // "Memory Clear" stays until a key: EXIT clears it.
  await ev("window.__press('on')");
  await ev("window.__press('9')");
  const nine = await ev(SCREEN);

  /** Search with `query`, then Enter on the first row; resolves once the palette closed and the calculator settled. */
  const choose = async (query, keys) => {
    await ev(`window.saturnus.palette.open(${JSON.stringify(query)}).then(() => true)`);
    await until(ev, `document.querySelector("dialog.palette[open] .prow-command .prow-keys")?.textContent === ${JSON.stringify(keys)}`, 10_000, `${query}: its keys in the first row`);
    assert.match(await ev(`document.querySelector("dialog.palette .palette-state").textContent`), /presses its keys/);
    await ev(`document.querySelector("dialog.palette input").focus(), true`);
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, text: "\r" });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
    await until(ev, `!document.querySelector("dialog.palette[open]")`, 15_000, `${query}: the palette closed`);
    await ev("window.__idle()");
  };

  await choose("SQRT", "√x");
  const root = await ev(SCREEN);
  assert.notEqual(root, nine, "a result");
  // The same screen as 9 √x pressed on the keys: ⬅ clears x (no lift), then 9 √x.
  for (const k of ["backspace", "9", "sqrt"]) await ev(`window.__press(${JSON.stringify(k)})`);
  assert.equal(await ev(SCREEN), root, "√x from Search shows what the √x key shows");

  // With the CONVERT menu open, the top row is its menu keys: Search
  // leaves the menu first, so √x is still the square root.
  for (const k of ["shift", "5", "9"]) await ev(`window.__press(${JSON.stringify(k)})`);
  assert.equal(await ev(MENU), true, "the CONVERT menu shows");
  await choose("SQRT", "√x");
  assert.equal(await ev(MENU), false, "the menu was left");
  const fromMenu = await ev(SCREEN);
  for (const k of ["9", "sqrt"]) await ev(`window.__press(${JSON.stringify(k)})`);
  assert.equal(await ev(SCREEN), fromMenu, "9 √x again gives the same screen: 3 over 3");

  // A shifted menu function: 4 ENTER 3, →POL from Search opens CONVERT and presses it.
  for (const k of ["4", "enter", "3"]) await ev(`window.__press(${JSON.stringify(k)})`);
  const before = await ev(SCREEN);
  await choose("->POL", "⇧ CONVERT →POL");
  const polar = await ev(SCREEN);
  assert.notEqual(polar, before, "converted");
  // Back and again by the keys: →REC, then ⇧ CONVERT →POL pressed, the same screen.
  for (const k of ["shift", "5", "ln", "shift", "5", "xeq"]) await ev(`window.__press(${JSON.stringify(k)})`);
  assert.equal(await ev(SCREEN), polar, "→POL from Search shows what its keys show");

  // A shift pending with a menu open: Search cancels the shift before
  // EXIT (a shifted EXIT is OFF), so the calculator stays on and √x runs.
  for (const k of ["backspace", "shift", "5", "9", "shift"]) await ev(`window.__press(${JSON.stringify(k)})`);
  assert.equal(await ev("Boolean(window.saturnus.store.state.frame.annunciators?.leftshift)"), true, "the shift is pending");
  await choose("SQRT", "√x");
  const shifted = await ev(SCREEN);
  assert.match(shifted, /#/, "the calculator stays on");
  assert.equal(await ev("Boolean(window.saturnus.store.state.frame.annunciators?.leftshift)"), false, "no shift left");
  for (const k of ["backspace", "9", "sqrt"]) await ev(`window.__press(${JSON.stringify(k)})`);
  assert.equal(await ev(SCREEN), shifted, "the same screen as 9 √x by hand");
});
