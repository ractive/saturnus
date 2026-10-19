// A key held at 2x, 4x or Max runs at 1x until it is released, so a tap is
// a tap at every speed. Before, a 250 ms click at Max lasted seconds of
// emulated time: the 42S showed NULL and dropped the key (the ROM cancels
// a key held about a second), the 48 and 49 repeated it. With
// `SATURNUS_ROM_DIR/hp42s.rom`, in the real page in headless Chrome.
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

const SCREEN = "document.querySelector('sat-calculator').screenText()";

test("a 250 ms tap at Max is a tap on the 42S, not a NULL", { timeout: 180_000 }, async (t) => {
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
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  await until(ev, "!!window.saturnus", 15_000, "the page started");
  await ev("window.saturnus.started.then(() => true)");
  await ev(`(async () => {
    window.__idle = async (ms = 600) => {
      const b = window.saturnus.backend;
      const t0 = (await b.stats()).emulatedMs;
      for (;;) {
        const s = await b.stats();
        if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
        await new Promise((r) => setTimeout(r, 50));
      }
    };
    window.__tap = async (key) => {
      const b = window.saturnus.backend;
      b.keyDown(key);
      await new Promise((r) => setTimeout(r, 250));
      b.keyUp(key);
      await window.__idle();
    };
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "hp42s.rom"));
    return true;
  })()`);
  await until(ev, "window.saturnus.store.state.booted === '42s'", 20_000, "booted");
  await ev("window.__idle(1500)");
  await ev("window.saturnus.setSpeed('max'); true");
  await until(ev, "window.saturnus.speed === 'max'", 5_000, "speed Max");
  for (const k of ["1", "enter", "2", "enter"]) await ev(`window.__tap(${JSON.stringify(k)})`);
  const before = await ev(SCREEN);
  await ev("window.__tap('plus')");
  assert.notEqual(await ev(SCREEN), before, "+ added: Y 1, X 4 instead of NULL and 2, 2");
});
