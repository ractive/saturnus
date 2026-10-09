// The calculator keeps its state across a reload (iteration 27), in the
// real page in headless Chrome over the DevTools protocol (no
// dependencies): a 48SX booted from `SATURNUS_ROM_DIR/sxrom-j`, `1 ENTER
// 2` typed through `window.saturnus.backend`, the state auto-saved after
// the delay and not again while idle, the page reloaded: the same stack
// and command line, no "Try To Recover Memory?". Start fresh asks in the
// page, cold-boots and forgets the kept state. Skipped without the ROM,
// without Chrome (`SATURNUS_CHROME` names one) or without the wasm
// package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

/** Poll `expression` in the page until it is truthy, at most `ms`. */
async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

// In the page: count the `autoSaved` events; wait for the CPU to sleep
// with no key down (the ROM waits for a key); press a key as the page does.
const HELPERS = `
  window.__saved = 0;
  window.saturnus.backend.addEventListener("autoSaved", () => window.__saved++);
  window.__idle = async (ms = 1500) => {
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
    await window.__idle(300);
  };
  true`;

const READ = `(async () => {
  const b = window.saturnus.backend;
  const stack = (await b.stack()).map((l) => l.text);
  const line = await b.commandLine();
  return { stack, line: line.active ? line.text : null };
})()`;

test("the calculator keeps its state across a reload", { timeout: 180_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "sxrom-j") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/sxrom-j not found");
    return;
  }
  const c = await session(t, { files: { "/__rom": rom } });
  if (!c) return;
  const { port } = c;
  try {
    const { send, ev } = c;
    await send("Page.enable");
    await send("Runtime.enable");
    const open = async () => {
      await until(ev, "!!window.saturnus", 15_000, "the page started");
      await ev("window.saturnus.started");
      await ev(HELPERS);
    };
    await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
    await open();

    // A cold boot from the ROM; the page's host answers NO to "Try To
    // Recover Memory?" itself.
    await ev(`(async () => {
      const bytes = await (await fetch("/__rom")).arrayBuffer();
      await window.saturnus.startWithRom(new File([bytes], "sxrom-j"));
      return true;
    })()`);
    await until(ev, "window.saturnus.store.state.booted === '48sx'", 15_000, "booted");
    await ev("window.__idle()");
    await ev("window.__idle()");
    const coldSaves = await ev("window.__saved");

    // 1 ENTER 2, through the backend as the page's keys send them.
    for (const k of ["1", "enter", "2"]) await ev(`window.__press(${JSON.stringify(k)})`);
    const typed = await ev(READ);
    assert.deepEqual(typed, { stack: ["1"], line: "2" });
    await until(ev, `window.__saved > ${coldSaves}`, 15_000, "the state saved after the delay");
    const saves = await ev("window.__saved");
    await sleep(7_000);
    assert.equal(await ev("window.__saved"), saves, "idle: nothing written");

    // Reloaded: the last model boots with the state it had.
    await send("Page.reload");
    await sleep(300);
    await open();
    await until(ev, "window.saturnus.store.state.booted === '48sx'", 15_000, "booted again");
    // The command line is open with "2": the ROM is past any question.
    assert.deepEqual(await ev(READ), typed, "the same stack and command line");
    await sleep(6_000);
    assert.equal(await ev("window.__saved"), 0, "a restore is not a change");

    // Start fresh: the page's own question, then a cold boot; the kept
    // state is gone, so a reload boots cold too.
    await ev("document.querySelector('sat-controls').startFresh(); true");
    await until(ev, "!!document.querySelector('.fresh-notice')", 5_000, "the question");
    assert.match(await ev("document.querySelector('.fresh-notice p').textContent"), /Start the HP 48SX fresh\?/);
    await ev("[...document.querySelectorAll('.fresh-notice button')].find((b) => b.textContent === 'Start fresh').click(); true");
    await until(ev, "window.saturnus.store.state.message === 'Started fresh, with an empty memory.'", 15_000, "started fresh");
    assert.equal(await ev("!!document.querySelector('.fresh-notice')"), false);
    const kept = await ev(`new Promise((resolve) => {
      const req = indexedDB.open("saturnus", 1);
      req.onsuccess = () => {
        const get = req.result.transaction("states").objectStore("states").get("auto:48sx");
        get.onsuccess = () => resolve(Boolean(get.result));
      };
    })`);
    assert.equal(kept, false, "the kept state is forgotten");
    await ev("window.__idle()");
    assert.notDeepEqual(await ev(READ).catch(() => null), typed, "an empty memory");
  } finally {
    await c.close();
  }
});
