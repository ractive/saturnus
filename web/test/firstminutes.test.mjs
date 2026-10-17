// The first minutes with the page, in headless Chrome over the DevTools
// protocol (no dependencies): the status line speaks plainly and an
// outcome goes after a while; the fullscreen refusal is the page's
// sentence, not the browser's. With `SATURNUS_ROM_DIR` (`gxrom-r`,
// `rom-2.10.49g`): a ROM's first boot lands on an empty stack that takes
// typing, with no key pressed. (Remove ROMs and the offer to keep a ROM
// are the home page's: romrows-page.test.mjs, overflow.test.mjs.) Skipped without Chrome (`SATURNUS_CHROME` names one) or the wasm
// package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { session as open, sleep } from "./chrome.mjs";

/** The owner's ROMs at /__rom/NAME, when SATURNUS_ROM_DIR is set. */
function roms() {
  const dir = process.env.SATURNUS_ROM_DIR;
  if (!dir || !existsSync(dir)) return {};
  return Object.fromEntries(readdirSync(dir).filter((f) => /^[\w.-]+$/.test(f)).map((f) => [`/__rom/${f}`, join(dir, f)]));
}

/** Chrome and the server for a test, or null when the test skipped (`SATURNUS_AUDIT` makes the skip a failure). */
const session = (t) => open(t, { files: roms(), audit: true });

/** The page in headless Chrome at `width` x `height`, or null when the test skipped. */
async function page(t, width = 1280, height = 900) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: width < 500 });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const until = async (expr, what, ms = 10_000) => {
    const end = Date.now() + ms;
    for (;;) {
      const v = await ev(expr).catch(() => null);
      if (v) return v;
      if (Date.now() > end) assert.fail(`timed out: ${what}`);
      await sleep(100);
    }
  };
  return { ...c, until };
}

test("a result's message: plain words, an outcome goes, an error waits for the next action", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  // The browser's refusal of fullscreen, in the page's words.
  await p.ev(`document.getElementById("stage").requestFullscreen = () => Promise.reject(new TypeError("Failed to execute 'requestFullscreen' on 'Element': API can only be initiated by a user gesture.")); true`);
  await p.ev(`document.getElementById("fullscreen").click(); true`);
  await p.until(`window.saturnus.store.state.message === "Fullscreen is not allowed here."`, "the refusal");
  await p.until(`document.querySelector(".toast[role=alert] p")?.textContent === "Fullscreen is not allowed here."`, "the refusal's toast");
  assert.equal(await p.ev("window.saturnus.store.state.messageError"), true, "an error: it waits for the next action");
  // An outcome goes after about 4 s, with its toast.
  await p.ev(`window.saturnus.store.set({ message: "Screen copied as an image.", messageError: false }); true`);
  await sleep(4_500);
  assert.equal(await p.ev("window.saturnus.store.state.message"), "", "gone after 4 s");
  // An error stays, and goes with the next click.
  await p.ev(`window.saturnus.store.set({ message: "Could not save the state: quota", messageError: true }); true`);
  await sleep(7_000);
  assert.equal(await p.ev("window.saturnus.store.state.message"), "Could not save the state: quota");
  await p.send("Input.dispatchMouseEvent", { type: "mousePressed", x: 5, y: 5, button: "left", clickCount: 1 });
  await p.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: 5, y: 5, button: "left", clickCount: 1 });
  assert.equal(await p.ev("window.saturnus.store.state.message"), "");
});

for (const [model, file] of [["48gx", "gxrom-r"], ["49g", "rom-2.10.49g"]]) {
  test(`the ${model}'s first boot lands on an empty stack`, { timeout: 180_000 }, async (t) => {
    const dir = process.env.SATURNUS_ROM_DIR;
    if (!dir || !existsSync(join(dir, file))) {
      t.skip(`SATURNUS_ROM_DIR/${file} not found`);
      return;
    }
    const p = await page(t);
    if (!p) return;
    await p.ev(`window.__keys = []; const b = window.saturnus.backend; const down = b.keyDown.bind(b); b.keyDown = (...a) => { window.__keys.push(a[0]); return down(...a); }; true`);
    await p.ev(`(async () => {
      window.saturnus.store.set({ model: "${model}" });
      const bytes = await (await fetch("/__rom/${file}")).arrayBuffer();
      await window.saturnus.startWithRom(new File([bytes], "${file}"));
      return true;
    })()`);
    await p.until(`window.saturnus.store.state.booted === "${model}"`, "booted", 30_000);
    // No key from the page: the host answered the question itself.
    await p.until(`window.saturnus.backend.stack().then((s) => Array.isArray(s) && s.length === 0, () => false)`, "an empty stack", 60_000);
    // Settled: asleep for a while with no key (the 49G's "Memory Clear" dismissed too).
    await p.ev(`(async () => {
      const b = window.saturnus.backend;
      const t0 = (await b.stats()).emulatedMs;
      for (;;) {
        const s = await b.stats();
        if (s.emulatedMs - t0 >= 1500 && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
        await new Promise((r) => setTimeout(r, 50));
      }
    })()`);
    assert.deepEqual(await p.ev("window.__keys"), [], "no key pressed by the page");
    const r = await p.ev(`window.saturnus.backend.run("7")`);
    assert.equal(r.error, null);
    assert.equal(await p.ev(`window.saturnus.backend.stack().then((s) => s[0].text)`), "7");
  });
}
