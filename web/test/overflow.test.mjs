// The page on phones: no horizontal overflow on any view at any width,
// and the command palette usable as a sheet with the on-screen keyboard
// open. Runs the real page in headless Chrome with touch emulation over
// the DevTools protocol (no dependencies), served by a static server in
// this process. Skipped when no Chrome is found (`SATURNUS_CHROME` names
// one; `SATURNUS_AUDIT=1` makes the skip a failure, `just web-audit`) or
// the wasm package is not built (`just web`). No ROM is needed: every
// view has a state without one (the calculator's answers stubbed where
// a view needs a running one: a command line, the memory view).
// Fullscreen: on every model, upright and on its side, the keys take the
// width and the buttons cover nothing; with a mouse, the whole calculator
// fits the screen instead.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { WEB, session as open, sleep } from "./chrome.mjs";

const WIDTHS = [360, 390, 430, 768, 1280];
const HEIGHTS = { 360: 780, 390: 844, 430: 932, 768: 1024, 1280: 900 };
const KEYBOARD = 336; // an on-screen keyboard's height on a phone, roughly
/* Containers that may scroll sideways by design: trees (deep nesting),
   tables of a calculator object, the lists of variables and commands,
   the editor's lines (a program is not wrapped). */
const MAY_SCROLL = [".tree", ".cmds-menus", ".grid-wrap", ".list-wrap", ".nibbles pre", ".rpl-text"];
/** A program for the palette's editor, with a line wider than a phone. */
const PROGRAM = '« → N\n  « IF N 0 > THEN "a long string that runs well past the edge of a phone" END »\n»';
/** Stack levels (level 1 first) whose lines are wider than a phone. */
const LEVELS = [
  { type: "program", text: PROGRAM.replace(/\s+/g, " ") },
  { type: "string", text: '"another string, as long as a line of the calculator\'s display and longer"' },
  { type: "real", text: "3.14159265359" },
];
/**
 * The backend's answers for a running calculator, stubbed: a command
 * line is open, and the memory view reads `LEVELS` and edits level 1.
 */
const STUBS = `(() => {
  const b = window.saturnus.backend;
  b.commandLine = async () => ({ active: true, text: ${JSON.stringify(PROGRAM)}, cursor: 0 });
  b.watchMemory = async (on) => ({ supported: true });
  const v = (name, type, extra = {}) => ({ name, type, size: 16, checksum: 0x5B55, address: 0x7A000 + name.length, ...extra });
  b.memoryTree = async () => ({ path: ["HOME"], variables: [
    v("MYDIR", "Directory", { variables: [v("A", "Real Number")] }),
    v("AVERYLONGNAME", "Program"),
    v("X", "Real Number"),
  ] });
  b.stack = async () => ${JSON.stringify(LEVELS)};
  b.flags = async () => ({ system: [], user: [], set: [] });
  b.editText = async () => ({ text: ${JSON.stringify(LEVELS[0].text)}, was: "level 1" });
})()`;
/** Chrome and the server for a test; `SATURNUS_AUDIT` makes a skip a failure. */
const session = (t) => open(t, { audit: true });

const MEASURE = `(() => {
  const de = document.documentElement;
  const sel = (e) => e.tagName.toLowerCase() + (e.id ? "#" + e.id : "") + (typeof e.className === "string" && e.className ? "." + e.className.trim().split(/\\s+/).join(".") : "");
  const visible = (e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== "hidden"; };
  const scrollers = [];
  for (const e of document.querySelectorAll("body *")) {
    if (e.closest(".skin") || !visible(e)) continue;
    if (/auto|scroll/.test(getComputedStyle(e).overflowX) && e.scrollWidth > e.clientWidth + 1 && !e.matches(${JSON.stringify(MAY_SCROLL.join(","))})) scrollers.push(sel(e) + " +" + (e.scrollWidth - e.clientWidth));
  }
  return { page: de.scrollWidth - de.clientWidth, inner: innerWidth, scrollers };
})()`;

test("no horizontal overflow on any view at any width; the palette as a phone sheet", { timeout: 180_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, settled, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  const metrics = (width, height) => send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 2, mobile: width < 1000 });
  await metrics(390, 844);
  // persist() recorded (and refused), to see that the page never calls it by itself.
  await send("Page.addScriptToEvaluateOnNewDocument", { source: `
    window.__persist = [];
    navigator.storage.persist = () => { window.__persist.push(new Error().stack); return Promise.resolve(false); };
    navigator.storage.persisted = () => Promise.resolve(false);` });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  assert.ok(await ev("!!window.saturnus"), "the page started");
  await ev("window.saturnus.started");
  await sleep(300);
  assert.equal(await ev("window.__persist.length"), 0, "no persist() on load");
  await ev(STUBS);

  const reset = () => ev(`(() => {
    for (const d of document.querySelectorAll("dialog[open]")) d.close();
    document.body.classList.remove("sheet-open");
    document.getElementById("roms")?.removeAttribute("open");
    const s = window.saturnus.store;
    s.set({ storageOffer: null, cmdlineOpen: false });
    if (s.state.booted) s.set({ booted: null });
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    return window.saturnus.setLayer(false);
  })()`);
  /** Poll `expression` (at most `ms`) until it is true. */
  const until = async (expression, ms = 8000) => {
    for (let i = 0; i < ms / 100; i++) {
      if (await ev(expression).catch(() => false)) return true;
      await sleep(100);
    }
    return false;
  };
  const layer = async (tab) => {
    await ev(`window.saturnus.setLayer(true).then(() => window.saturnus.explorer.setTab(${JSON.stringify(tab)}))`);
    if (tab === "commands") assert.ok(await until(`!!window.saturnus.explorer.cmdIndex`), "the Commands tab read its index");
  };
  const palette = async (query) => {
    await ev(`window.saturnus.palette.open(${JSON.stringify(query)})`);
    assert.ok(await until(`document.querySelectorAll("dialog.palette .prow").length > 3`), "the palette listed its rows");
  };
  // A ROM shown as kept (no ROM is needed for the ROMs panel's look).
  const KEPT = `(() => { const s = window.saturnus.store; s.set({ storage: "best-effort", roms: { ...s.state.roms, slots: s.state.roms.slots.map((x, i) => i ? x : { ...x, fileName: "a-rom-file-with-a-long-name.bin", state: "ready" }) } }); })()`;
  const EDIT_SHOWN = `[...document.querySelectorAll("#bar-edit, #cmdline-edit")].some((b) => b.checkVisibility())`;
  /** What must still hold when a view is measured, or it measured something else. */
  const STILL = {
    "edit line": EDIT_SHOWN,
    "edit line, its editor": `${EDIT_SHOWN} && document.querySelector("dialog.palette .palette-box").classList.contains("editing")`,
    "variables, menu open": `(() => {
      const m = document.querySelector("sat-explorer .menu");
      if (!m) return false;
      const r = m.getBoundingClientRect();
      const coarse = matchMedia("(pointer: coarse)").matches;
      return r.left >= 0 && r.right <= innerWidth && r.top >= 0 && r.bottom <= innerHeight
        && [...m.querySelectorAll("[role=menuitem]")].every((b) => !coarse || b.getBoundingClientRect().height >= 44);
    })()`,
  };
  const VIEWS = {
    calculator: () => ev(`window.saturnus.store.set({ message: "A status line long enough to wrap in the panel of a narrow phone, with-a-very-long-unbroken-token-in-it" })`),
    sheet: () => ev(`document.body.classList.add("sheet-open")`),
    roms: () => ev(`document.body.classList.add("sheet-open"); document.getElementById("roms").open = true; ${KEPT}`),
    storage: () => ev(`window.saturnus.store.set({ storageOffer: "ask" })`),
    vars: () => layer("vars"),
    stack: () => layer("stack"),
    flags: () => layer("flags"),
    commands: () => layer("commands"),
    palette: () => palette("sto"),
    editor: async () => {
      await palette("sto");
      await ev(`window.saturnus.palette.enterEditor(${JSON.stringify(PROGRAM)})`);
    },
    // The calculator with a command line open (as after UP, VIEW on the
    // interactive stack): the Edit button in the bar, then its editor.
    // Booted, so the page reads the (stubbed) open line itself.
    "edit line": async () => {
      await ev(`window.saturnus.store.set({ booted: "48gx" })`);
      assert.ok(await until(EDIT_SHOWN), "an Edit button shows");
    },
    "edit line, its editor": async () => {
      await VIEWS["edit line"]();
      await ev(`[...document.querySelectorAll("#bar-edit, #cmdline-edit")].find((b) => b.checkVisibility()).click()`);
      assert.ok(await until(`document.querySelector("dialog.palette .palette-box")?.classList.contains("editing")`), "the editor opened");
    },
    // The memory view's variables of a running calculator (the stubbed
    // command line open: the bar's Edit button beside the tabs).
    "variables": async () => {
      await ev(`window.saturnus.store.set({ booted: "48gx" })`);
      await layer("vars");
      assert.ok(await until(`[...document.querySelectorAll("sat-explorer .pane-vars .tree [role=treeitem]")].length > 1`), "the tree");
    },
    // The same with a long name selected and its "⋯" menu open: inside
    // the window, its items a finger's height on a phone.
    "variables, menu open": async () => {
      await VIEWS.variables();
      await ev(`window.saturnus.explorer.select(["HOME"], "AVERYLONGNAME")`);
      await ev(`document.querySelector("sat-explorer .pane-vars .preview button.more").click()`);
      assert.ok(await until(`!!document.querySelector("sat-explorer .menu")`), "the menu opened");
    },
    // The memory view's stack of a running calculator: a level selected
    // with its Edit button, then the editor opened from it.
    "stack levels": async () => {
      await ev(`window.saturnus.store.set({ booted: "48gx" })`);
      await layer("stack");
      assert.ok(await until(`!!document.querySelector("sat-explorer .pane-stack button.edit:not([disabled])")?.checkVisibility()`), "the level's Edit button");
    },
    "stack level, its editor": async () => {
      await VIEWS["stack levels"]();
      await ev(`document.querySelector("sat-explorer .pane-stack button.edit").click()`);
      assert.ok(await until(`document.querySelector("dialog.palette .palette-box")?.classList.contains("editing")`), "the editor opened");
    },
    shortcuts: () => ev(`window.saturnus.shortcuts.open()`),
    about: () => ev(`document.querySelector("sat-about").open()`),
  };
  const failures = [];
  for (const w of WIDTHS) {
    for (const scheme of ["light", "dark"]) {
      await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
      for (const [view, show] of Object.entries(VIEWS)) {
        await metrics(w, HEIGHTS[w]);
        await reset();
        await show();
        await sleep(view === "about" ? 700 : 300);
        const m = await settled(MEASURE);
        if (STILL[view] && !(await ev(STILL[view]))) failures.push(`${view} at ${w}px ${scheme}: not in its state when measured`);
        if (m.page > 0) failures.push(`${view} at ${w}px ${scheme}: the page is ${m.page}px too wide (innerWidth ${m.inner})`);
        for (const s of m.scrollers) failures.push(`${view} at ${w}px ${scheme}: ${s} scrolls sideways`);
      }
    }
  }
  assert.deepEqual(failures, []);

  // Persistent storage asked for in context (kb iteration 28b): the
  // notice after a ROM is kept, Keep it, Not now, the ROMs panel's button.
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
  await metrics(390, 844);
  await reset();
  // The backend's answer to a chosen file, stubbed: a new ROM kept in
  // the first slot each time, or (`rejected`) the slots as they were.
  await ev(`(() => {
    const b = window.saturnus.backend;
    let n = 0;
    b.chooseRom = async (model, files) => {
      const roms = window.saturnus.store.state.roms;
      if (files[0].name === "rejected") return { ...roms, notice: "Not a ROM this page knows." };
      return { ...roms, slots: roms.slots.map((x, i) => i ? x : { ...x, fileName: "rom-" + ++n, state: "ready" }) };
    };
  })()`);
  const choose = (name = "rom") => ev(`window.saturnus.chooseRoms([new File(["x"], ${JSON.stringify(name)})])`);
  // Read once the layout has settled: the inside check is a measurement.
  const notice = () => settled(`(() => { const n = document.querySelector(".storage-notice"); if (!n) return null; const r = n.getBoundingClientRect(); return { text: n.querySelector("p").textContent, buttons: [...n.querySelectorAll("button")].map((b) => b.textContent), inPanel: !!n.closest("sat-controls"), inside: r.left >= 0 && r.right <= innerWidth && r.bottom <= innerHeight }; })()`);
  const click = (sel) => ev(`document.querySelector(${JSON.stringify(sel)}).click()`);
  const asked = () => ev(`localStorage.getItem("saturnus.storageAsk")`);
  await choose();
  assert.ok(await until(`!!document.querySelector(".storage-notice")`), "the notice after a ROM is kept");
  const ask = await notice();
  assert.match(ask.text, /^Keep this ROM on this device\?/);
  assert.deepEqual(ask.buttons, ["Not now", "Keep it"]);
  assert.ok(ask.inPanel, "in the panel (the sheet), not over the keys");
  await ev(`document.body.classList.add("sheet-open"); true`);
  assert.ok((await notice()).inside, "inside the viewport at 390 px with the sheet open");
  await ev(`document.body.classList.remove("sheet-open"); true`);
  await click(".storage-notice button.primary");
  assert.ok(await until(`window.__persist.length === 1`), "Keep it calls persist()");
  assert.ok(await until(`/said no/.test(document.querySelector(".storage-notice p")?.textContent)`), "the refusal in one line");
  await click(".storage-notice button");
  assert.equal(await notice(), null, "OK closes it");
  assert.equal(await asked(), "refused", "a refusal is remembered");
  await choose();
  await sleep(300);
  assert.equal(await notice(), null, "and the next ROM does not ask again");
  await ev(`localStorage.removeItem("saturnus.storageAsk")`);
  await choose("rejected");
  await sleep(300);
  assert.equal(await notice(), null, "a file that keeps nothing offers nothing");
  await choose();
  assert.ok(await until(`!!document.querySelector(".storage-notice")`));
  await click(".storage-notice button:not(.primary)");
  assert.equal(await notice(), null, "Not now closes it");
  assert.equal(await asked(), "not-now", "Not now is remembered");
  await choose();
  await sleep(300);
  assert.equal(await notice(), null, "and the notice is not offered again");
  await ev(`document.body.classList.add("sheet-open"); document.getElementById("roms").open = true; ${KEPT}`);
  assert.equal(await ev(`document.querySelector(".rom-storage .storage-state").textContent`), "May be cleared when space runs low.");
  await click("#rom-keep");
  assert.ok(await until(`window.__persist.length === 2`), "the ROMs panel's Keep permanently calls persist()");
  await ev(`localStorage.removeItem("saturnus.storageAsk")`);
  await reset();

  // The palette on a phone with the keyboard up: the sheet follows the
  // visual viewport (simulated by a shorter viewport), the input and the
  // list stay inside it, a tapped row opens its entry with the way
  // back and the buttons inside the viewport.
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
  await metrics(390, 844 - KEYBOARD);
  await reset();
  await palette("sto");
  await sleep(300);
  const sheet = await ev(`(() => {
    const d = document.querySelector("dialog.palette");
    const list = d.querySelector(".palette-list");
    const input = d.querySelector(".palette-input input");
    const rows = [...list.querySelectorAll(".prow")].map((r) => r.getBoundingClientRect().height);
    return { dialog: d.getBoundingClientRect().toJSON(), list: list.getBoundingClientRect().toJSON(), input: input.getBoundingClientRect().toJSON(), inner: innerHeight, vvh: d.style.getPropertyValue("--vvh"), rows: rows.length, minRow: Math.min(...rows) };
  })()`);
  assert.equal(sheet.vvh, `${844 - KEYBOARD}px`, "the sheet's height follows the visual viewport");
  assert.ok(sheet.dialog.bottom <= sheet.inner + 1, `the sheet ends inside the viewport (${sheet.dialog.bottom} of ${sheet.inner})`);
  assert.ok(sheet.input.bottom <= sheet.inner, "the input is visible");
  assert.ok(sheet.list.bottom <= sheet.inner + 1 && sheet.list.height > 100, `the list is inside the viewport (${JSON.stringify(sheet.list)})`);
  assert.ok(sheet.rows > 3 && sheet.minRow >= 44, `rows are tappable (${sheet.rows} rows, the smallest ${sheet.minRow}px)`);
  const [x, y] = await ev(`(() => { const r = document.querySelector(".prow").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
  await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await sleep(300);
  const detail = await ev(`(() => {
    const box = document.querySelector(".palette-box");
    const back = box.querySelector(".palette-back button");
    const actions = box.querySelector(".palette-actions");
    return { open: box.classList.contains("detail-open"), back: back?.getBoundingClientRect().toJSON(), actions: actions?.getBoundingClientRect().toJSON(), inner: innerHeight, name: box.querySelector(".palette-back-name")?.textContent };
  })()`);
  assert.ok(detail.open, "a tapped row opens its entry as a second step");
  assert.equal(detail.name, "STO");
  assert.ok(detail.back && detail.back.height >= 44 && detail.back.top >= 0, "the way back is a finger's size at the top");
  assert.ok(detail.actions && detail.actions.bottom <= detail.inner + 1, `the buttons are inside the viewport (${JSON.stringify(detail.actions)})`);

  // The editor mode with the keyboard up: the head, the text and the
  // buttons inside the viewport, the buttons a finger's size.
  await ev(`window.saturnus.palette.enterEditor(${JSON.stringify(PROGRAM)})`);
  await sleep(300);
  const editor = await ev(`(() => {
    const d = document.querySelector("dialog.palette");
    const r = (sel) => d.querySelector(sel).getBoundingClientRect().toJSON();
    return { head: r(".editor-head"), text: r(".rpl-text"), bar: r(".editor-bar"), primary: r('[data-ed="primary"]'), inner: innerHeight };
  })()`);
  assert.ok(editor.head.top >= 0 && editor.bar.bottom <= editor.inner + 1, `the editor is inside the viewport (${JSON.stringify(editor)})`);
  assert.ok(editor.text.height > 120, `the text has room (${editor.text.height}px)`);
  assert.ok(editor.primary.height >= 44, "the buttons are a finger's size");
  assert.deepEqual(c.errors, [], "no exception in the page");
});

/** Fullscreen on phones: the models and the sizes the face is checked at. */
const FS_MODELS = ["48sx", "48gx", "49g", "38g", "39g", "40g", "42s"];
const FS_SIZES = [[360, 780], [390, 844], [430, 932], [844, 390], [932, 430]];
const FS_MEASURE = `(() => {
  const box = (e) => { const r = e.getBoundingClientRect(); return { l: r.left, t: r.top, r: r.right, b: r.bottom }; };
  const keys = [...document.querySelectorAll("sat-calculator .skin g.skey .cap")].map(box);
  const span = keys.reduce((a, k) => ({ l: Math.min(a.l, k.l), r: Math.max(a.r, k.r) }), { l: Infinity, r: -Infinity });
  const buttons = [...document.querySelectorAll("button.fs-tool")].map(box);
  // The print and the logo where they show: inside the drawn part of the face.
  const view = box(document.querySelector("sat-calculator .skin svg"));
  const print = [...document.querySelectorAll("sat-calculator .skin g.print > *, sat-calculator .skin image.logo")]
    .filter((e) => getComputedStyle(e).display !== "none").map(box)
    .map((b) => ({ l: Math.max(b.l, view.l), t: Math.max(b.t, view.t), r: Math.min(b.r, view.r), b: Math.min(b.b, view.b) }))
    .filter((b) => b.r > b.l && b.b > b.t);
  const calc = document.querySelector("sat-calculator");
  const body = document.querySelector("sat-calculator .skin g.case");
  const shown = body && getComputedStyle(body).display !== "none";
  return {
    w: innerWidth, h: innerHeight, edge: calc.classList.contains("edge"), crop: calc.classList.contains("crop"), stageCrop: calc.parentElement.classList.contains("fs-crop"),
    keys, span, print, lcd: box(document.querySelector("sat-calculator canvas")), buttons,
    view, viewBox: document.querySelector("sat-calculator .skin svg").getAttribute("viewBox"),
    skin: { w: calc.skinData.width, h: calc.skinData.height }, case: shown ? box(body) : null,
  };
})()`;

test("fullscreen: the keys take a phone's width and the buttons cover nothing", { timeout: 180_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, settled, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  const metrics = (width, height) => send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 2.625, mobile: true });
  await metrics(390, 844);
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  // Fullscreen wants a user gesture.
  await send("Runtime.evaluate", { expression: `document.getElementById("bar-fullscreen").click()`, userGesture: true });
  await sleep(500);
  const inside = (a, b) => a.l >= b.l - 0.5 && a.t >= b.t - 0.5 && a.r <= b.r + 0.5 && a.b <= b.b + 0.5;
  const overlap = (a, b) => a.l < b.r && b.l < a.r && a.t < b.b && b.t < a.b;
  const failures = [];
  for (const model of FS_MODELS) {
    await ev(`(() => { const s = document.getElementById("model"); s.value = ${JSON.stringify(model)}; s.dispatchEvent(new Event("change", { bubbles: true })); })()`);
    await sleep(400);
    for (const [w, h] of FS_SIZES) {
      await metrics(w, h);
      await sleep(300);
      const m = await settled(FS_MEASURE);
      const at = `${model} at ${w}x${h}`;
      const screen = { l: 0, t: 0, r: m.w, b: m.h };
      if (!m.edge) failures.push(`${at}: not edge to edge`);
      if (m.case) failures.push(`${at}: the case is not cropped`);
      if (!m.stageCrop) failures.push(`${at}: the stage is not marked fs-crop`);
      if (m.keys.length < 30 || m.keys.some((k) => !inside(k, screen))) failures.push(`${at}: a key is off the screen`);
      if (!inside(m.lcd, screen)) failures.push(`${at}: the display is off the screen`);
      // Upright, the keys take the screen's width (the bezel and the rim
      // cropped), or as much of it as the screen's height leaves.
      if (w < h && m.span.r - m.span.l < 0.84 * w) failures.push(`${at}: the keys span ${Math.round(m.span.r - m.span.l)}px of ${w}`);
      for (const b of m.buttons) {
        if (Math.min(b.r - b.l, b.b - b.t) < 44) failures.push(`${at}: a button is under 44px`);
        if (overlap(b, m.lcd) || m.keys.some((k) => overlap(b, k))) failures.push(`${at}: a button covers the display or a key`);
        if (m.print.some((p) => overlap(b, p))) failures.push(`${at}: a button covers the print or the logo`);
      }
    }
  }
  assert.deepEqual(failures, []);
  assert.deepEqual(c.errors, [], "no exception in the page");
});

/** Fullscreen with a mouse: the screens the whole calculator is checked at. */
const FS_DESKTOP = [[1280, 900], [1920, 1080], [1024, 768], [500, 900]];

test("fullscreen with a mouse: the whole calculator, scaled to the screen", { timeout: 180_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, settled, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  const metrics = (width, height) => send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
  await metrics(1280, 900);
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  assert.equal(await ev(`matchMedia("(pointer: coarse)").matches`), false, "a fine pointer");
  await send("Runtime.evaluate", { expression: `document.getElementById("bar-fullscreen").click()`, userGesture: true });
  await sleep(500);
  const inside = (a, b) => a.l >= b.l - 0.5 && a.t >= b.t - 0.5 && a.r <= b.r + 0.5 && a.b <= b.b + 0.5;
  const overlap = (a, b) => a.l < b.r && b.l < a.r && a.t < b.b && b.t < a.b;
  const failures = [];
  for (const model of FS_MODELS) {
    await ev(`(() => { const s = document.getElementById("model"); s.value = ${JSON.stringify(model)}; s.dispatchEvent(new Event("change", { bubbles: true })); })()`);
    await sleep(400);
    for (const [w, h] of FS_DESKTOP) {
      await metrics(w, h);
      await sleep(300);
      const m = await settled(FS_MEASURE);
      const at = `${model} at ${w}x${h}`;
      const screen = { l: 0, t: 0, r: m.w, b: m.h };
      if (!m.edge) failures.push(`${at}: not fullscreen`);
      if (m.crop || m.stageCrop) failures.push(`${at}: cropped`);
      // The whole skin in view, its case drawn and on the screen.
      const whole = [0, 0, m.skin.w, m.skin.h];
      if (m.viewBox.split(" ").some((v, i) => Math.abs(v - whole[i]) > 1e-6)) failures.push(`${at}: the view is ${m.viewBox}, not the whole skin`);
      if (!m.case) failures.push(`${at}: the case is hidden`);
      else if (!inside(m.case, screen)) failures.push(`${at}: the case is off the screen`);
      // Its shape kept, centred across, and as large as the screen allows.
      const vw = m.view.r - m.view.l;
      const vh = m.view.b - m.view.t;
      if (Math.abs(vw / vh - m.skin.w / m.skin.h) > 0.01) failures.push(`${at}: the aspect is ${(vw / vh).toFixed(3)}`);
      if (Math.abs(m.view.l - (m.w - m.view.r)) > 1) failures.push(`${at}: not centred`);
      if (vw < 0.85 * m.w && vh < 0.85 * m.h) failures.push(`${at}: ${Math.round(vw)}x${Math.round(vh)} is small for the screen`);
      for (const b of m.buttons) {
        if (overlap(b, m.lcd) || m.keys.some((k) => overlap(b, k))) failures.push(`${at}: a button covers the display or a key`);
      }
    }
  }
  assert.deepEqual(failures, []);
  assert.deepEqual(c.errors, [], "no exception in the page");
});

/** The page sizes the no-ROM message is checked at: phones, tablets, desktops. */
const NOROM_SIZES = [[360, 780, true], [390, 844, true], [844, 390, true], [768, 1024, true], [1024, 768, false], [1280, 900, false], [1920, 1080, false]];

test("the no-ROM message fits every model's display at every size", { timeout: 180_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, settled, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  const metrics = (width, height, mobile) => send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: mobile ? 2 : 1, mobile });
  await metrics(1280, 900, false);
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const failures = [];
  for (const model of FS_MODELS) {
    await ev(`(() => { const s = document.getElementById("model"); s.value = ${JSON.stringify(model)}; s.dispatchEvent(new Event("change", { bubbles: true })); })()`);
    await sleep(400);
    for (const [w, h, mobile] of NOROM_SIZES) {
      await metrics(w, h, mobile);
      await sleep(200);
      const m = await settled(`(() => {
        const n = document.querySelector("sat-calculator .no-rom");
        if (n.hidden) return { hidden: true };
        const box = n.getBoundingClientRect();
        // What shows: each visible child within the message's box, and nothing cut.
        const out = [...n.querySelectorAll("p, button, a")].filter((e) => !e.closest("[hidden]") && e.getClientRects().length).map((e) => {
          const r = e.getBoundingClientRect();
          return { what: e.className || e.tagName, out: r.left < box.left - 0.5 || r.right > box.right + 0.5 || r.top < box.top - 0.5 || r.bottom > box.bottom + 0.5 };
        }).filter((x) => x.out).map((x) => x.what);
        return { hidden: false, cut: n.scrollHeight > n.clientHeight + 1 || n.scrollWidth > n.clientWidth + 1, out };
      })()`);
      const at = `${model} at ${w}x${h}`;
      if (m.hidden) failures.push(`${at}: no message`);
      else if (m.cut || m.out.length) failures.push(`${at}: the message overflows the display (${m.out.join(", ") || "cut"})`);
    }
    await metrics(1280, 900, false);
  }
  assert.deepEqual(failures, []);
  assert.deepEqual(c.errors, [], "no exception in the page");
});

test("About names the release and the build", { timeout: 120_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  // The workspace's version, the one place it is written.
  const version = /^\[workspace\.package\][^[]*?^version = "([^"]+)"/ms.exec(readFileSync(join(WEB, "..", "Cargo.toml"), "utf8"))?.[1];
  assert.ok(version, "the workspace version");
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const line = () => ev(`(() => { document.dispatchEvent(new CustomEvent("sat-about")); return document.querySelector(".about-version").textContent; })()`);
  // No build known yet (here: plain files, no service worker): neutral.
  await ev(`window.saturnus.store.set({ build: null }); true`);
  assert.equal(await line(), `saturnus ${version}, build unknown`);
  // The service worker answers while About is open: the line follows.
  await ev(`window.saturnus.store.set({ build: "3f2a9c1e0b7d4a55" }); true`);
  assert.equal(await ev(`document.querySelector(".about-version").textContent`), `saturnus ${version}, build 3f2a9c1e0b7d4a55`);
  await ev(`document.querySelector("dialog.about").close(); true`);
  assert.equal(await line(), `saturnus ${version}, build 3f2a9c1e0b7d4a55`);
  await ev(`document.querySelector("dialog.about").close(); window.saturnus.store.set({ host: "tauri" }); true`);
  assert.equal(await line(), `saturnus ${version}, desktop app`);
});

test("a no-ROM message cut short keeps the whole of it: tooltip, and a tap shows it", { timeout: 120_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Emulation.setDeviceMetricsOverride", { width: 844, height: 390, deviceScaleFactor: 2, mobile: true });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const pick = async (model) => {
    await ev(`(() => { const s = document.getElementById("model"); s.value = ${JSON.stringify(model)}; s.dispatchEvent(new Event("change", { bubbles: true })); })()`);
    await sleep(500);
  };
  const state = () => ev(`(() => { const n = document.querySelector(".no-rom"); return { cls: n.className, title: n.title, hint: n.querySelector(".no-rom-get").hidden ? null : n.querySelector(".no-rom-get").textContent }; })()`);
  // The 48SX on its side: the first sentence; the hint's words in the tooltip.
  await pick("48sx");
  const sx = await state();
  assert.match(sx.cls, /\bshort\b/);
  assert.ok(sx.hint, "the page has a hint for the 48SX");
  assert.equal(sx.title, `No ROM for the HP\u00a048SX. ${sx.hint}`);
  // The 42S: a tap on the text opens its whole message.
  await pick("42s");
  const s42 = await state();
  assert.match(s42.title, /HP never published it/);
  const [x, y] = await ev(`(() => { const r = document.querySelector(".no-rom-text").getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
  await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
  await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await sleep(400);
  const menu = await ev(`(() => { const m = document.querySelector(".menu"); return m && { note: m.querySelector(".menu-note")?.textContent, items: [...m.querySelectorAll("[role=menuitem]")].map((b) => b.textContent), inView: m.getBoundingClientRect().right <= innerWidth && m.getBoundingClientRect().bottom <= innerHeight }; })()`);
  assert.ok(menu, "a menu opened");
  assert.match(menu.note, /HP never published it: read the 64 KB ROM out of your own 42S/);
  assert.ok(menu.items.includes("Choose ROM…"));
  assert.equal(menu.inView, true, "on the screen");
});

test("the palette: Choose the ROM first without a calculator, Change it after the calculator's actions; off controls say why", { timeout: 120_000 }, async (t) => {
  const c = await session(t);
  if (!c) return;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 100 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  const rows = async () => {
    await ev(`window.saturnus.palette.open("")`);
    await sleep(300);
    const r = await ev(`({ titles: window.saturnus.palette.model.rows.slice(0, 5).map((r) => r.name), foot: document.querySelector(".palette-hints").textContent })`);
    await ev(`window.saturnus.palette.close(); true`);
    return r;
  };
  await ev(`window.saturnus.store.set({ model: "48gx", booted: null }); true`);
  let r = await rows();
  assert.equal(r.titles[0], "Choose the HP 48GX ROM…");
  const titles = await ev(`[...document.querySelectorAll("#reset, #save, #load, #copy-screen")].map((b) => [b.disabled, b.title])`);
  for (const [off, title] of titles) assert.deepEqual([off, title], [true, "Start the calculator first"]);
  // The model runs: its actions first, then Change.
  await ev(`window.saturnus.store.set({ booted: "48gx", running: true }); true`);
  r = await rows();
  assert.deepEqual(r.titles.slice(0, 4), ["Pause the calculator", "Reset the calculator", "Save state", "Change the HP 48GX ROM…"]);
  assert.ok(!r.titles.includes("Choose the HP 48GX ROM…"));
  assert.match(r.foot, /move · .*Enter.* run/);
  assert.deepEqual(await ev(`[document.getElementById("reset").disabled, document.getElementById("reset").title]`), [false, ""]);
  assert.equal(await ev(`document.getElementById("load").title`), "No saved state for this model");
  // "choose" still finds the running ROM's action.
  assert.match(await ev(`(() => { window.saturnus.palette.setActions(); return window.saturnus.palette.model.actions.find((a) => a.id === "rom").keywords; })()`), /\bchoose\b/);
  // The memory view's typing indicator stays in the page (a live region), out of sight while the keys are the calculator's.
  await ev(`window.saturnus.setLayer(true)`);
  await sleep(300);
  assert.deepEqual(await ev(`(() => { const k = document.querySelector(".layer-keys"); return [k.hidden, k.classList.contains("visually-hidden"), k.getAttribute("aria-live")]; })()`), [false, true, "polite"]);
});
