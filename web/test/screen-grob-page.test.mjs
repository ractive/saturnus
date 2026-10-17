// The screen as a graphic (GROB) in the real page in headless Chrome over
// the DevTools protocol (no dependencies), with real mouse and key
// events: Screen to stack in the panel sends the frame the page shows;
// Screen to a variable… opens the memory view's name field in the
// calculator's current directory and stores the frame it was asked
// with, a taken name after "Replace X?", never over a directory; the
// display's menu has both; on a model without RPL memory (the 42S) and
// with the command line open they are off and say why. No ROM: the page
// is made to believe a model runs (`booted`, `model`, a frame) and the
// memory reads and the write are answered by the test (`window.__screens`
// keeps the writes asked for). Skipped without Chrome (`SATURNUS_CHROME`
// names one) or the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

/**
 * Make the page believe `model` runs with a frame (a diagonal line) and
 * HOME holds the real number A and the directory D; `storeScreen`
 * answers after 50 ms.
 */
const fake = (model) => `(async () => {
  const s = window.saturnus;
  const b = s.backend;
  const rows = ${model === "42s" ? 16 : 64};
  const px = new Uint8Array(17 * rows);
  for (let y = 0; y < rows; y++) px[y * 17 + (y >> 3)] |= 0x80 >> (y & 7);
  const ann = { leftshift: false, rightshift: false, alpha: false, alert: false, busy: false, transmit: false, updown: false, battery: false, g: false, rad: false };
  const v = (name, type, extra = {}) => ({ name, type, size: 16, checksum: 0x5B55, address: 0x7A000 + name.length, ...extra });
  const tree = { path: ["HOME"], variables: [v("D", "Directory", { variables: [] }), v("A", "Real Number")] };
  b.watchMemory = async () => ({ supported: ${model !== "42s"} });
  b.memoryTree = async () => structuredClone(tree);
  b.stack = async () => [];
  b.flags = async () => ({ system: [], user: [], set: [] });
  b.commandLine = async () => ({ active: false });
  b.stackTop = async () => ({ depth: 0, level1: null });
  window.__screens = [];
  b.storeScreen = async (args) => {
    window.__screens.push(args);
    await new Promise((r) => setTimeout(r, 50));
    return { name: args.name ?? undefined, emulatedMs: 1, keys: false };
  };
  s.store.set({ booted: "${model}", model: "${model}", frame: { width: 131, height: rows, pixels: btoa(String.fromCharCode(...px)), annunciators: ann, contrast: 12, contrastRange: [0, 31], contrastDefault: 12 } });
  const calc = document.querySelector("sat-calculator");
  for (let i = 0; i < 100 && !(calc.skinModel === "${model}" && calc.screenReady()); i++) await new Promise((r) => setTimeout(r, 50));
  return calc.skinModel;
})()`;

/** The page in headless Chrome at `width`, or null when the test skipped. */
async function page(t, width = 1280) {
  const c = await session(t, { audit: true });
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  /** A real click in the middle of `selector`'s element (`button`: "left", "right"). */
  const click = async (selector, button = "left") => {
    // In view first: the controls panel may be taller than the window.
    const [x, y] = await ev(`(() => { const e = document.querySelector(${JSON.stringify(selector)}); e.scrollIntoView({ block: "nearest" }); const r = e.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount: 1 });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount: 1 });
    await sleep(150);
  };
  const until = async (expr, what) => {
    for (let i = 0; i < 50; i++) {
      const v = await ev(expr).catch(() => null);
      if (v) return v;
      await sleep(100);
    }
    assert.fail(`timed out: ${what}`);
  };
  /** Text typed into the focused element, then Enter. */
  const typeEnter = async (text) => {
    await send("Input.insertText", { text });
    await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
    await send("Input.dispatchKeyEvent", { type: "char", key: "Enter", text: "\r" });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
    await sleep(150);
  };
  return { ...c, click, until, typeEnter };
}

const FIELD = `document.querySelector("sat-explorer .vars-making .edit-row")`;

test("Screen to stack sends the frame the page shows", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48gx")), "48gx");
  await p.until(`!document.querySelector("#screen-to-stack").disabled`, "the button on");
  const shown = await p.ev("window.saturnus.store.state.frame.pixels");
  await p.click("#screen-to-stack");
  const sent = await p.until("window.__screens[0]", "the write");
  assert.deepEqual(Object.keys(sent), ["frame"]);
  assert.equal(sent.frame.pixels, shown);
  await p.until(`/Screen put on stack level 1/.test(window.saturnus.store.state.writeMessage?.text)`, "the message");
});

test("Screen to a variable… asks for the name, and before replacing", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48gx")), "48gx");
  await p.until(`!document.querySelector("#screen-to-var").disabled`, "the button on");
  const shown = await p.ev("window.saturnus.store.state.frame.pixels");
  await p.click("#screen-to-var");
  await p.until(`${FIELD}?.querySelector("input") === document.activeElement`, "the name field, focused");
  assert.equal(await p.ev(`${FIELD}.querySelector(".edit-label").textContent`), "Screen to a variable in HOME:");
  // The screen changes meanwhile: what was asked for is stored.
  await p.ev(`window.saturnus.store.set({ frame: { ...window.saturnus.store.state.frame, pixels: btoa("\\0".repeat(17 * 64)) } }); true`);
  // A directory's name is refused in the field.
  await p.typeEnter("D");
  assert.equal(await p.ev(`${FIELD}.querySelector(".edit-error").textContent`), "D is a directory in HOME. Choose another name.");
  assert.equal(await p.ev("window.__screens.length"), 0);
  // A variable's asks first; Cancel keeps the field.
  await p.ev(`(() => { const i = ${FIELD}.querySelector("input"); i.value = ""; i.dispatchEvent(new Event("input")); return true; })()`);
  await p.typeEnter("A");
  await p.until(`document.querySelector("dialog[open]")?.textContent.includes("Replace A?")`, "the question");
  await p.ev(`[...document.querySelectorAll("dialog[open] button")].find((b) => b.textContent === "Cancel").click(); true`);
  await sleep(200);
  assert.equal(await p.ev("window.__screens.length"), 0, "not replaced");
  assert.ok(await p.ev(`!!${FIELD}`), "the field stays");
  await p.ev(`${FIELD}.querySelector("input").focus(); true`);
  await p.typeEnter("");
  await p.until(`document.querySelector("dialog[open]")?.textContent.includes("Replace A?")`, "the question again");
  await p.ev(`[...document.querySelectorAll("dialog[open] button")].find((b) => b.textContent === "Replace").click(); true`);
  const sent = await p.until("window.__screens[0]", "the write");
  assert.deepEqual(sent.dir, ["HOME"]);
  assert.equal(sent.name, "A");
  assert.equal(sent.replace, true);
  assert.equal(sent.frame.pixels, shown, "the screen when asked");
  await p.until(`!${FIELD}`, "the field closed");
  // A new name stores without asking.
  await p.click("#screen-to-var");
  await p.until(`${FIELD}?.querySelector("input") === document.activeElement`, "the field again");
  await p.typeEnter("PIC");
  const second = await p.until("window.__screens[1]", "the second write");
  assert.deepEqual([second.name, second.replace], ["PIC", false]);
  await p.until(`/Screen stored as PIC in HOME/.test(window.saturnus.store.state.writeMessage?.text)`, "the message");
  // New directory in the same directory replaces an open screen field: its
  // Enter creates a directory, it stores no screen.
  await p.click("#screen-to-var");
  await p.until(`${FIELD}?.querySelector("input") === document.activeElement`, "the screen field");
  await p.ev(`window.__dirs = []; window.saturnus.backend.createDir = async (dir, name) => { window.__dirs.push([dir, name]); return { emulatedMs: 1, keys: false }; }; document.querySelector("sat-explorer").openNewDir(["HOME"]); true`);
  await p.until(`${FIELD}?.querySelector("input") === document.activeElement`, "the field again");
  assert.equal(await p.ev(`${FIELD}.getAttribute("aria-label")`), "New directory in HOME");
  await p.typeEnter("NEWD");
  await p.until("window.__dirs.length", "the directory created");
  assert.deepEqual(await p.ev("window.__dirs"), [[["HOME"], "NEWD"]]);
  assert.equal(await p.ev("window.__screens.length"), 2, "no screen stored");
});

test("the display's menu has both; off where the memory cannot be written, saying why", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("49g")), "49g");
  await p.click("#lcd", "right");
  const item = (text) => `[...document.querySelectorAll(".menu [role=menuitem]")].find((b) => b.textContent === ${JSON.stringify(text)})`;
  assert.equal(await p.ev(`${item("Screen to stack")}.getAttribute("aria-disabled")`), null);
  await p.ev(`${item("Screen to stack")}.click(); true`);
  await p.until("window.__screens[0]", "the write from the menu");
  await p.until("!window.saturnus.store.state.writing", "the write done");
  // The command line open: off, and why.
  await p.ev(`window.saturnus.store.set({ cmdlineOpen: true }); true`);
  assert.equal(await p.ev(`document.querySelector("#screen-to-var").disabled`), true);
  assert.match(await p.ev(`document.querySelector("#screen-to-var").title`), /command line is open/);
  await p.ev(`window.saturnus.store.set({ cmdlineOpen: false }); true`);
  // The 42S: no RPL memory.
  assert.equal(await p.ev(fake("42s")), "42s");
  for (const id of ["#screen-to-var", "#screen-to-stack"]) {
    assert.equal(await p.ev(`document.querySelector("${id}").disabled`), true, id);
    assert.equal(await p.ev(`document.querySelector("${id}").title`), "The 42S has no memory to keep a graphic in: only the 48SX, 48GX and 49G do");
  }
  await p.click("#lcd", "right");
  assert.equal(await p.ev(`${item("Screen to a variable…")}.getAttribute("aria-disabled")`), "true");
  assert.match(await p.ev(`${item("Screen to a variable…")}.title`), /^The 42S has no memory/);
  await p.ev(`${item("Screen to stack")}.click(); true`);
  await sleep(200);
  assert.equal(await p.ev("window.__screens.length"), 0, "nothing written");
});
