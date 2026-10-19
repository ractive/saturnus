// Search on the HP 28S in the real page in headless Chrome (iteration
// 54): with `SATURNUS_ROM_DIR/hp28s.rom`, the calculator booted from the
// chosen file, a command chosen in Search typed on its keys: 9 then SQRT
// (√, a key) from Search shows what 9 √x pressed shows; →LIST (typed,
// then ENTER) with 1 2 2 on the line and the TRIG menu shown shows what
// the keys typed by hand show, the menu still there; Cmd/Ctrl+Enter on
// SIN inserts it into the open line.
// Under Menu Lock ABS is still typed whole. Skipped without Chrome or
// web/pkg (just web).
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
  window.__keys = async (keys) => { for (const k of keys) await window.__press(k); return true; };
  true`;
const SCREEN = "document.querySelector('sat-calculator').screenText()";
/** The display's pixel rows as text. */
const LINES = `${SCREEN}.split("\\n")`;
/** Whether the bottom line shows menu labels: the labels' top pixel row (25) dark under the first. */
const MENU = `${LINES}[25].slice(0, 22) === "#".repeat(22)`;

test("a 28S command chosen in Search is typed on its keys", { timeout: 300_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "hp28s.rom") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/hp28s.rom not found");
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
    await window.saturnus.startWithRom(new File([bytes], "hp28s.rom"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '28s'", 20_000, "booted");
  await ev("window.__idle(1500)");
  // "Memory Lost" stays until a key: ON clears it.
  await ev("window.__press('on')");
  const empty = await ev(SCREEN);
  const keys = (list) => ev(`window.__keys(${JSON.stringify(list)})`);
  /** An empty stack, no command line; the menu shown stays. */
  const clear = () => keys(["on", "shift", "0"]);
  /** The TRIG menu shown. */
  const trig = async () => {
    if (!(await ev(MENU))) await keys(["trig"]);
    assert.equal(await ev(MENU), true, "the TRIG menu shows");
  };

  /** Search for `query`, then Enter (Cmd/Ctrl+Enter with `opposite`) on the row named `name`. */
  const choose = async (query, name, opposite = false) => {
    await ev(`window.saturnus.palette.open(${JSON.stringify(query)}).then(() => true)`);
    await until(ev, `document.querySelector("dialog.palette[open] .prow-name")?.textContent === ${JSON.stringify(name)}`, 10_000, `${query}: ${name} first`);
    // The 28S can be sent to: no "Nothing can be sent" line.
    assert.equal(await ev(`document.querySelector("dialog.palette .palette-state").hidden`), true);
    await ev(`document.querySelector("dialog.palette input").focus(), true`);
    const modifiers = opposite ? 6 : 0; // Ctrl+Meta: one of them is the platform's.
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, text: "\r", modifiers });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, modifiers });
    await until(ev, `!document.querySelector("dialog.palette[open]")`, 15_000, `${query}: the palette closed`);
    await ev("window.__idle()");
  };

  // A key: 9, then √ from Search, is 9 √x pressed.
  await keys(["9"]);
  await choose("√", "√");
  const root = await ev(SCREEN);
  await clear();
  await keys(["9", "shift", "minus"]);
  assert.equal(await ev(SCREEN), root, "√ from Search shows what 9 √x shows");
  assert.notEqual(root, empty);

  // A name typed then ENTER, into an open line, with a menu shown: →LIST
  // runs the whole line, and TRIG stays.
  await clear();
  await trig();
  await keys(["1", "space", "2", "space", "2"]);
  await choose("→LIST", "→LIST");
  const list = await ev(SCREEN);
  assert.equal(await ev(MENU), true, "the TRIG menu is still there");
  await clear();
  await trig();
  await keys(["1", "space", "2", "space", "2", "space", "shift", "letteru", "letterl", "letteri", "letters", "lettert", "enter"]);
  assert.equal(await ev(SCREEN), list, "→LIST from Search shows what the keys typed by hand show");

  // Menu Lock on (shifted α): the letter keys A-R open menus, so Search
  // turns it off for the name and on again after.
  await clear();
  await keys(["shift", "alpha"]);
  await keys(["4", "neg"]);
  await choose("ABS", "ABS");
  const abs = await ev(SCREEN);
  await clear();
  await keys(["shift", "alpha", "4", "neg", "space", "lettera", "letterb", "letters", "enter", "shift", "alpha"]);
  assert.equal(await ev(SCREEN), abs, "ABS from Search under Menu Lock shows what -4 ABS typed shows");
  await keys(["shift", "alpha"]);

  // Insert: the name goes into the open line, no ENTER; then ENTER by hand.
  await clear();
  await keys(["3", "0"]);
  await choose("SIN", "SIN", true);
  assert.equal(await ev(`window.saturnus.store.state.frame.annunciators.busy`), false);
  await keys(["enter"]);
  const sin = await ev(SCREEN);
  await clear();
  await keys(["3", "0", "space", "letters", "letteri", "lettern", "enter"]);
  assert.notEqual(sin, empty);
  assert.equal(await ev(SCREEN), sin, "SIN inserted then ENTER shows what 30 SIN typed shows");
});
