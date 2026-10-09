// The phone's Edit button on a real ROM: a 48GX at 390 px (touch), keys
// tapped on the drawn calculator. After the boot (an empty stack) Edit is
// there but off; a digit typed opens the command line and Edit edits it;
// 1 ENTER, ▲ (the interactive stack) and VIEW open level 1 in the
// calculator's editor, and a tap on Edit opens the page's editor on that
// line, inside the screen. Skipped unless `SATURNUS_ROM_DIR` holds
// `gxrom-r`, Chrome and web/pkg exist; edit-button.test.mjs checks the
// same without a ROM.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

test("phone, 48GX ROM: Edit off on an empty stack; it edits a typed line and a level opened by ▲ VIEW", { timeout: 180_000 }, async (t) => {
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
    return true;
  })()`);
  for (let i = 0; i < 150 && (await ev("window.saturnus.store.state.booted")) !== "48gx"; i++) await sleep(100);
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
  const tapAt = async (x, y) => {
    await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
    await sleep(80);
    await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  };
  const center = (expr) => ev(`(() => { const r = ${expr}.getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; })()`);
  /** A finger's tap on the drawn key `name`, then the calculator idle. */
  const tap = async (name) => {
    const [x, y] = await center(`document.querySelector("sat-calculator").skinKey(${JSON.stringify(name)}).querySelector(".cap")`);
    await tapAt(x, y);
    await ev("window.__idle(400)");
  };
  /** The bar's Edit, once the page has read the calculator (250 ms after the screen). */
  const edit = async () => {
    await sleep(700);
    return ev(`(() => { const b = document.getElementById("bar-edit"); const r = b.getBoundingClientRect(); return { shown: b.checkVisibility(), off: b.getAttribute("aria-disabled") === "true", title: b.title.replace(/ \\(.*\\)$/, ""), inside: r.left >= 0 && r.right <= innerWidth }; })()`);
  };
  /** A tap on the bar's Edit: the editor's title, its box inside the screen, then closed. */
  const openEdit = async () => {
    const [x, y] = await center(`document.getElementById("bar-edit")`);
    await tapAt(x, y);
    for (let i = 0; i < 50 && !(await ev(`!!document.querySelector("dialog.palette .palette-box")?.classList.contains("editing")`)); i++) await sleep(100);
    const r = await ev(`(() => {
      const box = document.querySelector("dialog.palette .palette-box");
      const r = box.getBoundingClientRect();
      return { editing: box.classList.contains("editing"), title: document.querySelector(".editor-title").textContent,
        inside: r.left >= 0 && r.right <= innerWidth + 0.5 && r.top >= 0 && r.bottom <= innerHeight + 0.5,
        overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth };
    })()`);
    await ev("window.saturnus.palette.close(); window.saturnus.palette.close(); true");
    await sleep(300);
    return r;
  };
  // NO to "Try To Recover Memory?": an empty stack, Edit there and off.
  await tap("f");
  assert.deepEqual(await edit(), { shown: true, off: true, title: "Nothing to edit: the stack is empty and no command line is open", inside: true });
  // A digit typed: the command line, Edit on it.
  await tap("1");
  assert.deepEqual(await edit(), { shown: true, off: false, title: "Edit the command line", inside: true });
  assert.deepEqual(await openEdit(), { editing: true, title: "Editing the command line", inside: true, overflow: 0 });
  // ENTER, ▲ (the interactive stack), VIEW: level 1 in the calculator's editor.
  for (const k of ["enter", "up", "b"]) await tap(k);
  assert.equal(await ev("window.saturnus.backend.commandLine().then((l) => l.active)"), true, "VIEW opened a command line");
  assert.deepEqual(await edit(), { shown: true, off: false, title: "Edit the command line", inside: true });
  assert.deepEqual(await openEdit(), { editing: true, title: "Editing the command line", inside: true, overflow: 0 });
  // Out of it again: 1 on the stack, Edit on level 1.
  await tap("on");
  await tap("on");
  assert.deepEqual(await edit(), { shown: true, off: false, title: "Edit stack level 1", inside: true });
});
