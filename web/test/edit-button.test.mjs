// The always-present Edit button (the top bar's on a phone, the one over
// the calculator on a wide window) in the real page in headless Chrome,
// with the backend's reads stubbed (no ROM): off with the reason until
// something can be edited, its tooltip naming the target and the edit
// shortcut, a click editing what Cmd/Ctrl+E edits, also after the keys
// move between the memory view and the calculator; at 360 px it fits the
// bar, a finger's size. Skipped without Chrome or web/pkg.
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

// The calculator's reads: a command line (open or not), a stack, HOME
// with the program PRG; the editor's opening recorded, not shown.
const STUBS = `(() => {
  const s = window.saturnus;
  const b = s.backend;
  window.__line = false;
  window.__stack = [];
  window.__edited = [];
  b.commandLine = async () => ({ active: window.__line, text: window.__line ? "1" : "", cursor: 0 });
  b.stack = async () => window.__stack;
  b.stackTop = async () => { window.__topReads = (window.__topReads ?? 0) + 1; return { depth: window.__stack.length, level1: window.__stack[0] ?? null }; };
  b.watchMemory = async () => ({ supported: true });
  b.memoryTree = async () => ({ path: ["HOME"], variables: [{ name: "PRG", type: "Program", size: 20, checksum: 1, address: 0x7A000 }] });
  b.flags = async () => ({ system: [], user: [], set: [] });
  b.objectAt = async () => ({ type: "program", text: "« 1 »" });
  s.palette.openEditor = async (t) => { window.__edited.push(t.kind === "variable" ? t.name : t.kind === "level" ? "level " + t.level : t.kind); };
  return true;
})()`;

async function page(t, { width, height, mobile }) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: mobile ? 2 : 1, mobile });
  if (mobile) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started.then(() => true)");
  await ev(STUBS);
  const button = mobile ? "#bar-edit" : "#cmdline-edit";
  /** The visible Edit button's `{off, title}` once the page has read the calculator (250 ms after a change). */
  const state = async () => {
    await sleep(500);
    return ev(`(() => { const b = document.querySelector(${JSON.stringify(button)}); return { shown: b.checkVisibility(), off: b.getAttribute("aria-disabled") === "true", title: b.title }; })()`);
  };
  /** A click (a tap on a phone) at the middle of `selector`. */
  const click = async (selector) => {
    const [x, y] = await ev(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [Math.round(r.x + r.width / 2), Math.round(r.y + r.height / 2)]; })()`);
    if (mobile) {
      await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
      await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    } else {
      await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
    }
    await sleep(200);
  };
  const edited = async () => {
    const e = await ev("window.__edited.join()");
    await ev("window.__edited.length = 0");
    return e;
  };
  /** A screen change, which has the page read the command line and the stack again. */
  const changed = () => ev(`window.saturnus.store.set({ frame: { ...(window.saturnus.store.state.frame ?? {}), n: Math.random() } }); true`);
  return { ...c, button, state, click, edited, changed };
}

for (const [label, size] of [["desktop", { width: 1280, height: 900, mobile: false }], ["phone at 360 px", { width: 360, height: 780, mobile: true }]]) {
  test(`the Edit button, ${label}: always there, off with the reason, else what Cmd/Ctrl+E edits`, { timeout: 120_000 }, async (t) => {
    const p = await page(t, size);
    if (!p) return;
    const key = await p.ev(`window.saturnus.bindings.labelOf("edit")`);
    // No calculator; a model without the editor.
    assert.deepEqual(await p.state(), { shown: true, off: true, title: "Start the calculator first" });
    await p.ev(`window.saturnus.store.set({ booted: "38g", model: "38g" }); true`);
    assert.deepEqual(await p.state(), { shown: true, off: true, title: "The HP 38G has no editor. Editing works on the 48SX, 48GX and 49G." });
    // A 48GX: an empty stack, then 42 on it, then a command line open.
    await p.ev(`window.saturnus.store.set({ booted: "48gx", model: "48gx" }); true`);
    assert.deepEqual(await p.state(), { shown: true, off: true, title: "Nothing to edit yet: the stack is empty." });
    await p.click(p.button);
    assert.equal(await p.edited(), "", "an off button does nothing");
    await p.ev(`window.__stack = [{ type: "real", text: "42" }]; true`);
    await p.changed();
    assert.deepEqual(await p.state(), { shown: true, off: false, title: `Edit stack level 1 (${key})` });
    await p.click(p.button);
    assert.equal(await p.edited(), "level 1");
    // Its shortcut for assistive technology, in ARIA's names.
    assert.match(await p.ev(`document.querySelector(${JSON.stringify(p.button)}).getAttribute("aria-keyshortcuts")`), /^(Control|Meta)\+E$/);
    if (!size.mobile) assert.match(await p.ev(`document.getElementById("palette-show").getAttribute("aria-keyshortcuts")`), /^(Control|Meta)\+K$/);
    // A program running: its frames read nothing; its end is read once.
    await p.ev(`window.saturnus.store.set({ loop: "frame" }); true`);
    const reads = await p.ev("window.__topReads");
    for (let i = 0; i < 3; i++) {
      await p.changed();
      await sleep(300);
    }
    assert.equal(await p.ev("window.__topReads"), reads, "no reads while it computes");
    await p.ev(`window.saturnus.store.set({ loop: "sleep" }); true`);
    await sleep(500);
    assert.equal(await p.ev("window.__topReads"), reads + 1, "one read once it waits");
    // Another machine: nothing of the old one's, at once.
    await p.ev(`window.saturnus.store.set({ booted: "48sx", model: "48sx" }); true`);
    assert.equal(await p.ev(`document.querySelector(${JSON.stringify(p.button)}).title`), "Reading the calculator…");
    assert.deepEqual(await p.state(), { shown: true, off: false, title: `Edit stack level 1 (${key})` });
    await p.ev("window.__line = true");
    await p.changed();
    assert.deepEqual(await p.state(), { shown: true, off: false, title: `Edit the command line (${key})` });
    await p.click(p.button);
    assert.equal(await p.edited(), "cmdline");
    // A write running: off until it is done.
    await p.ev(`window.saturnus.store.set({ writing: "Storing…" }); true`);
    assert.deepEqual(await p.state(), { shown: true, off: true, title: "Wait: the calculator is busy" });
    await p.ev(`window.saturnus.store.set({ writing: null }); true`);
    // The memory view: PRG selected there, then the keys back on the calculator.
    await p.ev(`window.saturnus.setLayer(true).then(() => window.saturnus.explorer.setTab("vars")).then(() => true)`);
    await sleep(500);
    await p.click(`.list tbody tr[data-name="PRG"] td`);
    await sleep(500);
    assert.deepEqual(await p.state(), { shown: true, off: false, title: `Edit PRG in HOME (${key})` });
    // On a phone the view lies over the calculator: back to it first, as a finger would.
    if (size.mobile) {
      await p.click(p.button);
      assert.equal(await p.edited(), "PRG", "the selection, as Cmd/Ctrl+E");
      await p.click(".layer-back");
    } else {
      await p.click(p.button);
      assert.equal(await p.edited(), "PRG", "the selection, as Cmd/Ctrl+E");
    }
    await p.click("sat-calculator canvas");
    assert.deepEqual(await p.state(), { shown: true, off: false, title: `Edit the command line (${key})` }, "the keys back on the calculator");
    await p.click(p.button);
    assert.equal(await p.edited(), "cmdline");
    if (size.mobile) {
      assert.deepEqual(await p.ev(`(() => { const r = document.querySelector("#bar-edit").getBoundingClientRect(); return { w: r.width >= 44, h: r.height >= 44, inside: r.left >= 0 && r.right <= innerWidth, overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth }; })()`),
        { w: true, h: true, inside: true, overflow: 0 }, "a finger's size, inside the bar, no overflow");
    }
  });
}
