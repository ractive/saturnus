// The screen as a graphic on a real ROM, through the Worker and the wasm
// host: a 48GX with 1 and 2 on the stack; the panel's Screen to stack
// puts the frame the page shows on level 1 as a GROB of the same pixels,
// and Screen to a variable… stores the screen as asked for in the named
// variable, the stack as it was. Skipped unless `SATURNUS_ROM_DIR` holds
// `gxrom-r`, Chrome and web/pkg exist; screen-grob-page.test.mjs checks
// the page without a ROM, saturnus-host's tests/transfer.rs the pixels
// against the ROM's LCD→.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

test("48GX ROM: the screen to the stack and to a variable, the frame's pixels", { timeout: 180_000 }, async (t) => {
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
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
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
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep") return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  }; true`);
  await ev("window.__idle()");
  await ev("window.saturnus.store.set({ storageOffer: null }); true");
  const b = "window.saturnus.backend";
  await ev(`${b}.run("1 2").then(() => window.__idle(500))`);
  const until = async (expr, what, n = 100) => {
    for (let i = 0; i < n; i++) {
      const v = await ev(expr).catch(() => null);
      if (v) return v;
      await sleep(100);
    }
    assert.fail(`timed out: ${what}`);
  };
  // The page's frame as the GROB's rows (hex): the frame's packing.
  const rows = `(() => { const f = window.saturnus.store.state.frame; return [...atob(f.pixels)].map((c) => c.charCodeAt(0).toString(16).toUpperCase().padStart(2, "0")).join(""); })()`;
  await until(`!document.querySelector("#screen-to-stack").disabled`, "the button on");
  await sleep(500);
  const shown = await ev(rows);
  await ev(`document.querySelector("#screen-to-stack").click(); true`);
  await until(`/Screen put on stack level 1/.test(window.saturnus.store.state.writeMessage?.text)`, "the write", 300);
  const stack = await ev(`${b}.stack()`);
  assert.equal(stack.length, 3);
  assert.deepEqual([stack[0].graphic.width, stack[0].graphic.height], [131, 64]);
  assert.equal(stack[0].graphic.rows, shown, "level 1 is the screen shown");
  assert.deepEqual(stack.slice(1).map((l) => l.text), ["2", "1"]);
  // To a variable: the name field, the stack as it was.
  await sleep(500);
  const now = await ev(rows);
  await ev(`document.querySelector("#screen-to-var").click(); true`);
  await until(`document.querySelector("sat-explorer .vars-making input") === document.activeElement`, "the name field");
  await send("Input.insertText", { text: "PIC" });
  await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
  await until(`/Screen stored as PIC in HOME/.test(window.saturnus.store.state.writeMessage?.text)`, "the write", 300);
  const tree = await ev(`${b}.memoryTree()`);
  const pic = tree.variables.find((v) => v.name === "PIC");
  assert.ok(pic, JSON.stringify(tree.variables.map((v) => v.name)));
  assert.ok(!tree.variables.some((v) => v.name.startsWith("SATSCRN")), "no temporary name left");
  const obj = await ev(`${b}.objectAt(${pic.address})`);
  assert.equal(obj.graphic.rows, now, "the variable is the screen shown");
  assert.equal((await ev(`${b}.stack()`)).length, 3, "the stack as it was");
});
