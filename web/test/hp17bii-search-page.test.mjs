// Search on the HP 17BII in the real page in headless Chrome (iteration
// 54): with `SATURNUS_ROM_DIR/hp17bii.rom`, the calculator booted from
// the chosen file, a function chosen in Search pressed on it. The 17BII
// always shows a menu (MAIN at the top), so Search first goes back to
// MAIN (⇧ MAIN) and then presses the menu keys: PMT (FIN TVM PMT) chosen
// from inside another menu shows what those keys pressed from MAIN show;
// N! (⇧ MATH N!) works on the number keyed in. Skipped without Chrome or
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

test("a 17BII function chosen in Search goes back to MAIN and presses its menu keys", { timeout: 240_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "hp17bii.rom") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/hp17bii.rom not found");
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
    await window.saturnus.startWithRom(new File([bytes], "hp17bii.rom"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '17bii'", 20_000, "booted");
  await ev("window.__idle(1500)");
  const press = async (keys) => {
    for (const k of keys) await ev(`window.__press(${JSON.stringify(k)})`);
  };
  // The language menu after a memory reset: ENGL, then EXIT clears "MEMORY LOST".
  await press(["b", "exit"]);

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

  // A loan in FIN TVM by the keys: N=360, I%YR=6, PV=100,000, FV=0.
  await press(["a", "a", "3", "6", "0", "a", "6", "b", "1", "0", "0", "0", "0", "0", "c", "0", "e"]);
  // Somewhere else (BUS %CHG), so FIN TVM's keys are not under the menu keys.
  await press(["shift", "exit", "b", "a"]);
  const elsewhere = await ev(SCREEN);
  await choose("PMT", "FIN TVM PMT");
  const payment = await ev(SCREEN);
  assert.notEqual(payment, elsewhere, "the payment was worked out");
  // The same keys pressed by hand from MAIN show the same screen.
  await press(["shift", "exit", "a", "a", "d"]);
  assert.equal(await ev(SCREEN), payment, "PMT from Search shows what MAIN FIN TVM PMT shows");

  // A shifted menu: 5, then N! from Search.
  await press(["shift", "exit", "5"]);
  await choose("N!", "⇧ MATH N!");
  const fact = await ev(SCREEN);
  await press(["shift", "exit", "5", "shift", "percent", "e"]);
  assert.equal(await ev(SCREEN), fact, "N! from Search shows what ⇧ MATH N! shows");
});
