// The phone's way into the editor, on a real ROM: on a 48GX at phone
// width (touch), 1 ENTER, ▲ (the interactive stack) and VIEW open level 1
// in the calculator's own editor, a command line; the top bar then shows
// Edit, and a tap on it opens the page's editor on that line, inside the
// screen. ▲ alone opens no command line, so Edit, always there, is off
// on the empty stack (found while checking a report that Edit had gone:
// it never showed for ▲ alone before it was always there). Skipped
// unless `SATURNUS_ROM_DIR` holds `gxrom-r`, Chrome and web/pkg exist;
// the same layout without a ROM, with a stubbed command line, is
// overflow.test.mjs's "edit line" views.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

test("phone: 1 ENTER ▲ VIEW on the 48GX shows Edit in the bar; its editor fits the screen", { timeout: 180_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "gxrom-r") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/gxrom-r not found");
    return;
  }
  const c = await session(t, { files: { "/__rom": rom } });
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
  await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started.then(() => true)");
  await ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "gxrom-r"));
    window.saturnus.store.set({ storageOffer: null });
    return true;
  })()`);
  for (let i = 0; i < 150 && (await ev("window.saturnus.store.state.booted")) !== "48gx"; i++) await sleep(100);
  // Idle: the CPU asleep with no key down for `ms` emulated ms.
  await ev(`window.__idle = async (ms = 1500) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (;;) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  }; true`);
  await ev("window.__idle()");
  // The storage offer comes after the boot and would cover the bottom keys.
  await ev("window.saturnus.store.set({ storageOffer: null }); true");
  /** A finger's tap on the drawn key `name`, then the calculator idle. */
  const tap = async (name) => {
    const [x, y] = await ev(`(() => { const r = document.querySelector("sat-calculator").skinKey(${JSON.stringify(name)}).querySelector(".cap").getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; })()`);
    await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
    await sleep(80);
    await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    await ev("window.__idle(400)");
  };
  const edit = `document.getElementById("bar-edit")`;
  /** Whether the bar's Edit shows and is on, once the page has read the calculator (250 ms after the screen). */
  const editShown = async () => {
    await sleep(600);
    return ev(`${edit}.checkVisibility() && ${edit}.getAttribute("aria-disabled") === "false"`);
  };
  // NO to "Try To Recover Memory?", then ▲ alone: no command line and an
  // empty stack, so Edit is there but off, saying why.
  await tap("f");
  await tap("up");
  await sleep(600);
  assert.deepEqual(await ev(`[${edit}.checkVisibility(), ${edit}.getAttribute("aria-disabled"), ${edit}.title]`),
    [true, "true", "Nothing to edit: the stack is empty and no command line is open"], "▲ alone opens no command line");
  await tap("on");
  // 1 ENTER ▲ VIEW: level 1 in the calculator's editor.
  for (const k of ["1", "enter", "up", "b"]) await tap(k);
  assert.equal(await ev("window.saturnus.backend.commandLine().then((l) => l.active)"), true, `VIEW opened a command line:\n${await ev("window.saturnus.screenText()")}`);
  assert.equal(await editShown(), true, "the bar's Edit shows");
  assert.equal(await ev(`(() => { const r = ${edit}.getBoundingClientRect(); return r.left >= 0 && r.right <= innerWidth && r.top >= 0; })()`), true, "inside the bar");
  assert.equal(await ev("document.documentElement.scrollWidth - document.documentElement.clientWidth"), 0, "no page overflow");
  // A tap on it: the page's editor on that line, inside the screen.
  const [ex, ey] = await ev(`(() => { const r = ${edit}.getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; })()`);
  await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: ex, y: ey }] });
  await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  for (let i = 0; i < 50 && !(await ev(`!!document.querySelector("dialog.palette .palette-box")?.classList.contains("editing")`)); i++) await sleep(100);
  assert.deepEqual(await ev(`(() => {
    const box = document.querySelector("dialog.palette .palette-box");
    const r = box.getBoundingClientRect();
    return { editing: box.classList.contains("editing"), title: document.querySelector(".editor-title").textContent,
      inside: r.left >= 0 && r.right <= innerWidth + 0.5 && r.top >= 0 && r.bottom <= innerHeight + 0.5,
      overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth };
  })()`), { editing: true, title: "Editing the command line", inside: true, overflow: 0 });
});
