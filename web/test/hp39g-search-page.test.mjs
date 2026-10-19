// Search on the HP 39G in the real page in headless Chrome (iteration
// 54): with `SATURNUS_ROM_DIR/rom.39g`, the calculator booted from the
// chosen file, a function chosen in Search goes to HOME and presses its
// keys: COMB (MATH P P ▶ ENTER) puts COMB( on the edit line, and with
// the arguments and ENTER the history shows what the same keys pressed
// on the drawn keyboard show; with the MATH menu open, Search closes it
// first, so SIN still goes on the edit line. Skipped without Chrome or
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
  true`;
const SCREEN = "document.querySelector('sat-calculator').screenText()";
/**
 * HOME's history as two entries of 18 dot rows each (input and result),
 * the older first: rows 8-25 and 26-43 of the display. The edit line,
 * whose cursor blinks, is left out.
 */
const ENTRIES = `(() => { const t = ${SCREEN}.split("\\n"); return [t.slice(8, 26).join("\\n"), t.slice(26, 44).join("\\n")]; })()`;
/** Whether a choose box shows: its top border across row 4. */
const BOX = `(() => { const t = ${SCREEN}.split("\\n"); return t[4].slice(8, 118) === "#".repeat(110); })()`;

test("a 39G function chosen in Search goes on HOME's edit line", { timeout: 240_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "rom.39g") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/rom.39g not found");
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
  // rom.39g also runs the 40G: the model chosen first says which.
  await ev(`(async () => {
    window.saturnus.store.set({ model: "39g" });
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "rom.39g"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '39g'", 30_000, "booted");
  await ev("window.__idle(2000)");
  // "Memory Clear" stays until a key: ENTER clears it, HOME shows HOME.
  for (const k of ["enter", "home"]) await ev(`window.__press(${JSON.stringify(k)})`);
  const press = async (keys) => {
    for (const k of keys) await ev(`window.__press(${JSON.stringify(k)})`);
  };

  /** Search with `query`, then Enter on the first row; resolves once the palette closed and the calculator settled. */
  const choose = async (query, keys) => {
    await ev(`window.saturnus.palette.open(${JSON.stringify(query)}).then(() => true)`);
    await until(ev, `document.querySelector("dialog.palette[open] .prow-command .prow-keys")?.textContent === ${JSON.stringify(keys)}`, 10_000, `${query}: its keys in the first row`);
    assert.match(await ev(`document.querySelector("dialog.palette .palette-state").textContent`), /goes to HOME and presses its keys/);
    await ev(`document.querySelector("dialog.palette input").focus(), true`);
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, text: "\r" });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
    await until(ev, `!document.querySelector("dialog.palette[open]")`, 15_000, `${query}: the palette closed`);
    await ev("window.__idle()");
  };

  // COMB from Search, then 5,2) ENTER typed: the history shows COMB(5,2)
  // and 10. The same by the keys: both entries look the same.
  await choose("COMB", "MATH P P ▶ ENTER");
  await press(["5", "comma", "2", "rparen", "enter"]);
  const [, fromSearch] = await ev(ENTRIES);
  assert.match(fromSearch, /#/, "an entry");
  await press(["math", "7", "7", "right", "enter", "5", "comma", "2", "rparen", "enter"]);
  const [older, newer] = await ev(ENTRIES);
  assert.equal(older, fromSearch, "the entry scrolled up");
  assert.equal(newer, fromSearch, "COMB from Search makes the entry its keys make");

  // With the MATH menu open, Search closes it (CANCEL) before HOME and
  // the keys: SIN( goes on the edit line, not into the menu.
  await press(["math"]);
  assert.equal(await ev(BOX), true, "the MATH menu shows");
  await choose("SIN", "SIN");
  assert.equal(await ev(BOX), false, "the menu was closed");
  await press(["0", "rparen", "enter"]);
  const [, sin] = await ev(ENTRIES);
  await press(["sin", "0", "rparen", "enter"]);
  assert.equal((await ev(ENTRIES))[1], sin, "SIN from Search makes the entry the SIN key makes");
});
