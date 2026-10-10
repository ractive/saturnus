// The toast (web/components/toast.js) in the real page in headless
// Chrome: an outcome goes after TOAST_MS, an error stays until closed
// with its × or Escape (one Tab reaches its ×, which gives the focus
// back), a newer one replaces the shown one, the roles; the panel's
// results (a saved state) seen with the panel hidden and on a phone, at
// the stage's top where it keeps off the calculator's keys, in light and
// dark; the memory view's results while it is closed; the status line
// keeps only what runs. The ROM panel's texts: the hint about a kept ROM
// only once one is, and a finger's screen is not told to drop a file.
// The backend's answers are stubbed. Skipped without Chrome
// (`SATURNUS_CHROME` names one) or the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

async function page(t, { width = 1280, height = 900, mobile = false, scheme = "light" } = {}) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  if (mobile) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile });
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const until = async (expr, what, ms = 5000) => {
    for (let i = 0; i < ms / 50; i++) {
      const v = await ev(expr).catch(() => null);
      if (v) return v;
      await sleep(50);
    }
    assert.fail(`timed out waiting for ${what}`);
  };
  return { ...c, until };
}

/** The toasts in the page: text, role, rect. */
const TOASTS = `[...document.querySelectorAll(".toast")].map((e) => {
  const r = e.getBoundingClientRect();
  return { text: e.querySelector("p").textContent, role: e.getAttribute("role"), close: !!e.querySelector(".toast-close"),
    l: r.left, t: r.top, r: r.right, b: r.bottom, bg: getComputedStyle(e).backgroundColor };
})`;

/** How many of the calculator's keys the rect `x` overlaps. */
const KEYS_UNDER = (x) => `[...document.querySelectorAll("sat-calculator .skey")].map((k) => k.getBoundingClientRect())
  .filter((k) => k.width && !(k.right <= ${x.l} || k.left >= ${x.r} || k.bottom <= ${x.t} || k.top >= ${x.b})).length`;

/** The model runs (stubbed) and Save state answers as the backend does. */
const RUNNING = `(() => {
  const s = window.saturnus.store;
  window.saturnus.backend.saveState = async () => "State saved in this browser.";
  s.set({ booted: "48sx", model: "48sx", romName: "sxrom-j", running: true });
  return true;
})()`;

test("an outcome goes after 4 s, an error stays for its click or ×, a newer one replaces it", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`import("/components/toast.js").then((m) => { window.__toast = m; return true; })`);
  await p.ev(`window.__toast.toast("Screen copied as an image."), true`);
  await p.until(`document.querySelector(".toast p")?.textContent`, "the toast's text");
  let shown = await p.ev(TOASTS);
  assert.equal(shown.length, 1);
  assert.deepEqual([shown[0].text, shown[0].role, shown[0].close], ["Screen copied as an image.", "status", false]);
  await sleep(3_000);
  assert.equal((await p.ev(TOASTS)).length, 1, "still there at 3 s");
  await sleep(1_500);
  assert.equal((await p.ev(TOASTS)).length, 0, "gone after 4 s");

  // One at a time: the newer replaces the shown one.
  await p.ev(`window.__toast.toast("First."), window.__toast.toast("Second."), true`);
  await p.until(`document.querySelector(".toast p")?.textContent`, "the toast's text");
  shown = await p.ev(TOASTS);
  assert.deepEqual(shown.map((x) => x.text), ["Second."]);

  // An error: role alert, a ×, still there after 5 s; a click on its
  // text (selecting it, to copy) leaves it; its × closes it.
  await p.ev(`window.__toast.toast("Could not save the state: quota", { error: true }), true`);
  await p.until(`document.querySelector(".toast p")?.textContent`, "the error's text");
  shown = await p.ev(TOASTS);
  assert.deepEqual([shown.length, shown[0].role, shown[0].close], [1, "alert", true]);
  await sleep(5_000);
  assert.equal((await p.ev(TOASTS)).length, 1, "an error stays");
  const { l, t: top, b } = (await p.ev(TOASTS))[0];
  const y = (top + b) / 2;
  await p.send("Input.dispatchMouseEvent", { type: "mousePressed", x: l + 14, y, button: "left", clickCount: 1 });
  await p.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: l + 120, y, button: "left" });
  await p.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: l + 120, y, button: "left", clickCount: 1 });
  await sleep(200);
  assert.equal((await p.ev(TOASTS)).length, 1, "a click or a selection on its text leaves it");
  assert.ok((await p.ev("String(getSelection())")).length > 0, "its text selected");
  await p.ev(`document.querySelector(".toast-close").click(), true`);
  assert.equal((await p.ev(TOASTS)).length, 0, "closed by its ×");

  // A step in progress stays until the next message, and "" ends it.
  await p.ev(`window.saturnus.store.set({ message: "Downloading the HP 48GX ROM from hpcalc.org…", messageError: false }), true`);
  await p.until(`document.querySelector(".toast p")?.textContent`, "the step's text");
  await sleep(4_500);
  assert.equal((await p.ev(TOASTS)).length, 1, "a step stays");
  await p.ev(`window.saturnus.store.set({ message: "" }), true`);
  assert.equal((await p.ev(TOASTS)).length, 0, "ended by an empty message");
});

/** Press Tab. */
const TAB = { type: "rawKeyDown", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 };
/** Press Escape. */
const ESC = { type: "rawKeyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 };

test("an error from the keyboard: the next Tab reaches its ×, Escape closes it, the focus goes back", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`(() => { const b = document.getElementById("about"); b.focus(); return document.activeElement === b; })()`);
  await p.ev(`document.querySelector("sat-controls").message("Could not save the state: quota", true), true`);
  await p.until(`document.querySelector(".toast p")?.textContent`, "the error");
  assert.equal(await p.ev(`document.activeElement?.id`), "about", "the focus is not taken");
  // An Escape something else takes (the memory view's, a binding to ON) is not the toast's.
  await p.ev(`document.getElementById("about").addEventListener("keydown", (e) => { if (e.key === "Escape") e.preventDefault(); }), true`);
  await p.send("Input.dispatchKeyEvent", ESC);
  await p.send("Input.dispatchKeyEvent", { ...ESC, type: "keyUp" });
  assert.equal((await p.ev(TOASTS)).length, 1, "an Escape taken elsewhere leaves it");
  await p.send("Input.dispatchKeyEvent", TAB);
  await p.send("Input.dispatchKeyEvent", { ...TAB, type: "keyUp" });
  assert.equal(await p.ev(`document.activeElement?.className`), "icon toast-close", "one Tab reaches the ×");
  await p.send("Input.dispatchKeyEvent", ESC);
  await p.send("Input.dispatchKeyEvent", { ...ESC, type: "keyUp" });
  assert.equal((await p.ev(TOASTS)).length, 0, "Escape on its × closes it");
  assert.equal(await p.ev(`document.activeElement?.id`), "about", "the focus is back");
  assert.equal(await p.ev("window.saturnus.store.state.message"), "", "closing it empties the message");
  // Within the grace after it showed, the same error again shows again.
  await p.ev(`document.querySelector("sat-controls").message("Could not save the state: quota", true), true`);
  await p.until(`document.querySelector(".toast p")?.textContent`, "the error again");
  await p.ev(`document.querySelector(".toast-close").click(), true`);
  await p.ev(`document.querySelector("sat-controls").message("Could not save the state: quota", true), true`);
  assert.equal(await p.until(`document.querySelector(".toast p")?.textContent`, "the error a third time"), "Could not save the state: quota");
  // The same outcome twice in a row: two toasts.
  await p.ev(`document.querySelector("sat-controls").message("State saved in this browser."), true`);
  await p.until(`document.querySelector(".toast p")?.textContent === "State saved in this browser."`, "the outcome");
  await p.ev(`import("/components/toast.js").then((m) => (m.closeToast(), true))`);
  assert.equal((await p.ev(TOASTS)).length, 0);
  await p.ev(`document.querySelector("sat-controls").message("State saved in this browser."), true`);
  assert.equal(await p.until(`document.querySelector(".toast p")?.textContent`, "the outcome again"), "State saved in this browser.");
});

test("a shown error follows the stage: into fullscreen, and centred when the panel hides", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(`window.saturnus.store.set({ message: "Could not save the state: quota", messageError: true }), true`);
  await p.until(`document.querySelector(".toast p")?.textContent`, "the error");
  const centred = `(() => { const a = document.querySelector(".toast").getBoundingClientRect(); const s = document.getElementById("stage").getBoundingClientRect(); return Math.abs((a.left + a.right) / 2 - (s.left + s.right) / 2) <= 1; })()`;
  assert.equal(await p.ev(centred), true);
  await p.ev(`document.getElementById("panel-hide").click(), true`);
  await sleep(300);
  assert.equal(await p.ev(centred), true, "centred again after the panel hid");
  await p.send("Runtime.evaluate", { expression: `document.getElementById("stage").requestFullscreen()`, userGesture: true, awaitPromise: true });
  await p.until(`document.fullscreenElement?.id === "stage"`, "fullscreen");
  await sleep(200);
  assert.equal(await p.ev(`document.querySelector(".toast")?.parentElement?.id`), "stage", "inside the fullscreen element");
  await p.ev(`document.exitFullscreen().then(() => true)`);
  await sleep(200);
  assert.equal(await p.ev(`document.querySelector(".toast")?.parentElement === document.body`), true, "back in the page");
});

test("a key's emptying ends its own step only", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  const s = "window.saturnus.store";
  await p.ev(`${s}.set({ message: "Downloading the HP 48GX ROM from hpcalc.org…" }), true`);
  await p.until(`document.querySelector(".toast p")?.textContent`, "the step");
  await p.ev(`${s}.set({ layer: true, writeMessage: { text: "Setting flag -14…", error: false } }), ${s}.set({ layer: false }), ${s}.set({ writeMessage: null }), true`);
  assert.equal(await p.ev(`document.querySelector(".toast p")?.textContent`), "Downloading the HP 48GX ROM from hpcalc.org…", "the memory view's emptying leaves the download");
  await p.ev(`${s}.set({ writeMessage: { text: "Setting flag -14…", error: false } }), true`);
  await p.until(`document.querySelector(".toast p")?.textContent === "Setting flag -14…"`, "the write's step");
  await p.ev(`${s}.set({ message: "" }), true`);
  assert.equal((await p.ev(TOASTS)).length, 1, "the message's emptying leaves the write's step");
  await p.ev(`${s}.set({ writeMessage: null }), true`);
  assert.equal((await p.ev(TOASTS)).length, 0, "its own emptying ends it");
});

test("the panel's results with the panel hidden; the status line keeps what runs", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(RUNNING);
  await p.ev(`document.getElementById("panel-hide").click(), true`);
  assert.equal(await p.ev(`document.body.classList.contains("panel-hidden")`), true);
  await p.ev(`document.getElementById("save").click(), true`);
  const text = await p.until(`document.querySelector(".toast p")?.textContent`, "the toast");
  assert.equal(text, "State saved in this browser.");
  await sleep(300);
  const [x] = await p.ev(TOASTS);
  const stage = await p.ev(`(() => { const r = document.getElementById("stage").getBoundingClientRect(); return { l: r.left, r: r.right, t: r.top }; })()`);
  assert.ok(Math.abs((x.l + x.r) / 2 - (stage.l + stage.r) / 2) <= 1, "centred on the stage");
  assert.ok(x.t >= stage.t && x.t - stage.t <= 16, `at the stage's top: ${x.t} of ${stage.t}`);
  assert.equal(await p.ev(KEYS_UNDER(x)), 0, "covers no key");
  assert.equal(await p.ev(`document.getElementById("status").textContent`), "HP 48SX · sxrom-j", "the line keeps the model and ROM");

  // The memory view's result while the view is closed; open, its own row shows it.
  await p.ev(`window.saturnus.store.set({ writeMessage: { text: "Copied P to HOME › D.", error: false } }), true`);
  assert.equal(await p.until(`document.querySelector(".toast p")?.textContent === "Copied P to HOME › D." && "ok"`, "the write's toast"), "ok");
  await p.ev(`window.saturnus.store.set({ layer: true, writeMessage: { text: "Saved P.", error: false } }), true`);
  await sleep(200);
  assert.notEqual(await p.ev(`document.querySelector(".toast p")?.textContent ?? ""`), "Saved P.", "not while the view is open");
});

for (const scheme of ["light", "dark"]) {
  test(`on a phone (${scheme}): the toast is seen with the sheet closed and keeps off the keys`, { timeout: 120_000 }, async (t) => {
    const p = await page(t, { width: 390, height: 844, mobile: true, scheme });
    if (!p) return;
    await p.ev(RUNNING);
    assert.equal(await p.ev(`document.body.classList.contains("sheet-open")`), false);
    await p.ev(`document.getElementById("save").click(), true`);
    await p.until(`document.querySelector(".toast p")?.textContent`, "the toast");
    await sleep(300);
    const [x] = await p.ev(TOASTS);
    assert.equal(x.text, "State saved in this browser.");
    const bar = await p.ev(`document.querySelector(".bar").getBoundingClientRect().bottom`);
    assert.ok(x.t >= bar, `under the top bar: ${x.t} vs ${bar}`);
    assert.ok(x.l >= 0 && x.r <= 390 && x.b <= 844, "on the screen");
    assert.equal(await p.ev(KEYS_UNDER(x)), 0, "covers no key");
    assert.ok(await p.ev(`document.querySelectorAll("sat-calculator .skey").length`) > 0, "the keys are drawn");
    assert.equal(x.bg, scheme === "dark" ? "rgb(30, 30, 28)" : "rgb(251, 250, 246)", "the page's paper");
  });
}

/** ROM slots, `kept` the 48SX's file or none. */
const SLOTS = (kept) => `(() => {
  const s = window.saturnus.store;
  const d = (file) => ({ file, size: 1, url: "https://www.hpcalc.org/x.zip", page: "https://www.hpcalc.org/details/1", revision: "rev" });
  const slots = [
    { model: "48sx", fileName: ${kept ? '"sxrom-j"' : "null"}, revision: ${kept ? '"J"' : "null"}, state: ${kept ? '"ready"' : '"empty"'}, download: d("sxrom-j") },
    { model: "38g", fileName: null, revision: null, state: "empty", download: d("38G_A167.ROM") },
  ];
  s.set({ model: "38g" });
  s.set({ roms: { slots, offers: [], lastModel: null, bootLast: true, remembered: true, note: null }, model: "48sx" });
  return true;
})()`;

test("the ROM hints: 'Kept in this browser' once a ROM is; short help under the table", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(SLOTS(false));
  assert.equal(await p.ev(`document.querySelector(".rom-hint").hidden`), true, "nothing kept: no hint");
  await p.ev(SLOTS(true));
  assert.equal(await p.ev(`document.querySelector(".rom-hint").hidden`), false, "kept: the hint");
  assert.match(await p.ev(`document.querySelector(".rom-hint").textContent`), /^Kept in this browser/);
  await p.ev(`window.saturnus.store.set({ model: "38g" }), true`);
  assert.equal(await p.ev(`document.querySelector(".rom-hint").hidden`), true, "the model shown has none");
  assert.equal(await p.ev(`document.querySelector(".source-hint").textContent`),
    "Download opens the model's page on hpcalc.org. Unzip the file and choose it here. The ROMs are HP's software; hpcalc.org hosts them with HP's permission.");
  assert.equal(await p.ev(`document.querySelector(".forget-hint").textContent`), "You can choose several files at once; each goes to its model.");
  assert.equal(await p.ev(`document.querySelector("sat-calculator .no-rom-get").textContent`), "Download 38G_A167.ROM from hpcalc.org, unzip it and drop the file here.");
});

test("a finger's screen is told to choose the file, not to drop it", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 390, height: 844, mobile: true });
  if (!p) return;
  assert.equal(await p.ev(`matchMedia("(pointer: coarse)").matches`), true, "a coarse pointer");
  await p.ev(SLOTS(false));
  await sleep(200);
  assert.equal(await p.ev(`document.querySelector("sat-calculator .no-rom-get").textContent`), "Download sxrom-j from hpcalc.org, unzip it and choose the file.");
  assert.doesNotMatch(await p.ev(`document.getElementById("roms").textContent`), /drop/i);
});
