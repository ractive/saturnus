// The ROM table ("ROMs of every model") in the real page in headless
// Chrome: a primary button per row that acts at once and a "⋯" for the
// rest, lined up, rows of one height; the primaries and menus per host and
// slot; Remove… asks in the shared modal, takes the 39G's and 40G's shared
// file together, and stops the model that runs from it; no overflow on a
// phone. The page's ROM slots and the backend's answers are
// stubbed. Skipped without Chrome (`SATURNUS_CHROME` names one) or the
// wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

/** The slots shown, `romSource` set, the backend's removes recorded (`__removed`, `__unloaded`). */
const SLOTS = (source) => `(() => {
  const s = window.saturnus.store;
  const b = window.saturnus.backend;
  b.romSource = ${JSON.stringify(source)};
  const d = (file) => ({ file, size: 1, url: "https://www.hpcalc.org/x.zip", page: "https://www.hpcalc.org/details/1", revision: "rev" });
  const slots = [
    { model: "48sx", fileName: "sxrom-j", revision: "J", state: "ready", download: d("sxrom-j") },
    { model: "48gx", fileName: "gxrom-r", revision: "R", state: "ready", download: d("gxrom-r") },
    { model: "38g", fileName: null, revision: null, state: "empty", download: d("38G_A167.ROM") },
    { model: "49g", fileName: "rom.49g", revision: "2.10", state: "ready", download: d("rom.49g") },
    { model: "39g", fileName: "rom.39g", revision: "C", state: "ready", download: d("rom.39g") },
    { model: "40g", fileName: "rom.39g", revision: "C", state: "ready", download: d("rom.39g") },
    { model: "42s", fileName: null, revision: null, state: "empty", download: null },
  ];
  window.__removed = [];
  window.__unloaded = 0;
  b.forgetRom = async (model) => {
    window.__removed.push(model);
    const r = s.state.roms;
    return { ...r, slots: r.slots.map((x) => (x.model === model ? { ...x, fileName: null, revision: null, state: "empty" } : x)) };
  };
  b.unload = async () => { window.__unloaded++; s.set({ booted: null }); return null; };
  // What the primaries start, recorded instead of a file chooser or a download.
  window.__acted = [];
  const c = document.querySelector("sat-controls");
  c.chooseFor = (m) => window.__acted.push(["choose", m]);
  c.downloadFor = (m) => window.__acted.push(["download", m]);
  s.set({ roms: { slots, offers: [], lastModel: null, bootLast: true, remembered: true, note: null } });
  document.getElementById("roms").open = true;
  return true;
})()`;

/**
 * Each row: the model, its primary's text, width and left edge, its ⋯
 * (size and label, or null), its height; the name cell's two lines (the
 * model's and the file's tops and texts) and the buttons' middle against
 * the cell's.
 */
const ROWS = `[...document.querySelectorAll(".rom-slots tr")].map((tr) => {
  const b = tr.querySelector("td.rom-act .rom-row-button");
  const m = tr.querySelector("td.rom-act button.more");
  const r = b.getBoundingClientRect();
  const mr = m?.getBoundingClientRect();
  return { model: tr.querySelector(".rom-model").textContent, buttons: tr.querySelectorAll("td.rom-act button").length, label: b.textContent,
    width: Math.round(r.width), left: Math.round(r.left), buttonHeight: Math.round(r.height),
    more: m ? { w: Math.round(mr.width), h: Math.round(mr.height), label: m.getAttribute("aria-label"), menu: m.getAttribute("aria-haspopup") } : null,
    height: Math.round(tr.getBoundingClientRect().height),
    file: tr.querySelector(".rom-file-name").textContent,
    lines: [tr.querySelector(".rom-model"), tr.querySelector(".rom-file-name")].map((e) => Math.round(e.getBoundingClientRect().top)),
    cut: (() => { const f = tr.querySelector(".rom-file-name"); return f.scrollWidth > f.clientWidth; })(),
    centred: (() => { const c = tr.querySelector("th").getBoundingClientRect(); return Math.abs((r.top + r.height / 2) - (c.top + c.height / 2)) <= 1; })() };
})`;

/** Every row's name in two lines, the file under the model, the buttons centred against them. */
function twoLines(rows) {
  for (const r of rows) {
    assert.ok(r.lines[1] > r.lines[0], `${r.model}: the file under the model ${r.lines}`);
    assert.ok(r.centred, `${r.model}: the buttons centred`);
  }
}

/** Click `model`'s primary button. */
const PRIMARY = (model) => `document.querySelector('.rom-row-button[data-model="${model}"]').click(), true`;

async function page(t, width = 1280, height = 900, mobile = false) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  if (mobile) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  /** The items of `model`'s row menu, opened by its button. */
  const menu = async (model) => {
    await ev(`document.querySelector('button[data-rom-menu="${model}"]').click(); true`);
    await sleep(150);
    return ev(`[...document.querySelectorAll(".menu [role=menuitem], .menu [role=separator]")].map((e) => e.getAttribute("role") === "separator" ? "-" : e.textContent)`);
  };
  const choose = (text) => ev(`[...document.querySelectorAll(".menu [role=menuitem]")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);
  return { ...c, menu, choose };
}

test("a primary and a ⋯ per row, lined up, one height; the browser's", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(SLOTS("file"));
  const rows = await p.ev(ROWS);
  assert.equal(new Set(rows.map((r) => r.height)).size, 1, `one height: ${rows.map((r) => r.height)}`);
  assert.equal(new Set(rows.map((r) => r.width)).size, 1, `one width: ${rows.map((r) => r.width)}`);
  assert.equal(new Set(rows.map((r) => r.left)).size, 1, `lined up: ${rows.map((r) => r.left)}`);
  twoLines(rows);
  const sizes = rows.filter((r) => r.more).map((r) => `${r.more.w}x${r.more.h}`);
  assert.equal(new Set(sizes).size, 1, `one ⋯ size: ${sizes}`);
  assert.equal(sizes[0], `${rows[0].buttonHeight}x${rows[0].buttonHeight}`, "the ⋯ is square, as high as the primary");
  const by = Object.fromEntries(rows.map((r) => [r.model, r]));
  // Empty in the browser: Choose… alone, the row links to hpcalc.org.
  assert.equal(by["HP 38G"].label, "Choose…");
  assert.equal(by["HP 38G"].more, null);
  assert.equal(by["HP 38G"].buttons, 1);
  assert.equal(await p.ev(`[...document.querySelectorAll(".rom-slots tr")].find((tr) => tr.querySelector(".rom-model").textContent === "HP 38G").querySelector("a")?.textContent`), "Download");
  assert.equal(by["HP 42S"].label, "Choose…");
  assert.equal(by["HP 42S"].more, null);
  assert.equal(await p.ev(`[...document.querySelectorAll(".rom-slots tr")].find((tr) => tr.querySelector(".rom-model").textContent === "HP 42S").querySelector(".rom-file-name").textContent`), "none");
  // Filled: Change… at once; the ⋯ holds Remove… alone, never a bare button.
  assert.equal(by["HP 48GX"].label, "Change…");
  assert.deepEqual(by["HP 48GX"].more, { ...by["HP 48GX"].more, label: "More for HP 48GX", menu: "menu" });
  assert.deepEqual(await p.menu("48gx"), ["Remove…"]);
  await p.ev(`document.activeElement.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })); true`);
  await sleep(100);
  await p.ev(PRIMARY("48gx"));
  await p.ev(PRIMARY("38g"));
  assert.deepEqual(await p.ev("window.__acted"), [["choose", "48gx"], ["choose", "38g"]]);
  assert.equal(await p.ev(`document.querySelectorAll(".menu [role=menuitem]").length`), 0, "a primary opens no menu");
  assert.equal(await p.ev(`document.getElementById("rom-forget").textContent`), "Remove ROMs…");
});

test("the app's: Download… at once with a file in the ⋯; Change… with Download again…", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(SLOTS("dialog"));
  const rows = await p.ev(ROWS);
  assert.equal(new Set(rows.map((r) => r.width)).size, 1, `one width: ${rows.map((r) => r.width)}`);
  assert.equal(new Set(rows.map((r) => r.left)).size, 1, `lined up: ${rows.map((r) => r.left)}`);
  twoLines(rows);
  const by = Object.fromEntries(rows.map((r) => [r.model, r]));
  assert.equal(by["HP 38G"].label, "Download…");
  assert.equal(by["HP 38G"].file, "none");
  assert.deepEqual(rows.filter((r) => r.cut).map((r) => r.model), [], "the app's file names fit at 1280");
  assert.deepEqual(await p.menu("38g"), ["Choose a file…"]);
  await p.choose("Choose a file…");
  await sleep(100);
  assert.deepEqual(await p.menu("48gx"), ["Download again…", "-", "Remove…"]);
  await p.choose("Download again…");
  await sleep(100);
  await p.ev(PRIMARY("38g"));
  await p.ev(PRIMARY("49g"));
  assert.deepEqual(await p.ev("window.__acted"), [["choose", "38g"], ["download", "48gx"], ["download", "38g"], ["choose", "49g"]]);
  assert.equal(by["HP 42S"].label, "Choose…", "no download for the 42S");
  assert.equal(by["HP 42S"].more, null);
});

test("Remove… asks in the modal; the 39G and 40G go together; a running model stops", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(SLOTS("file"));
  const modal = () => p.ev(`(() => { const d = document.querySelector("dialog.confirm"); return d && d.open ? { title: d.querySelector("h2").textContent, body: d.querySelector("p").textContent, buttons: [...d.querySelectorAll("button")].map((b) => b.textContent) } : null; })()`);
  const answer = (text) => p.ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);
  await p.menu("49g");
  await p.choose("Remove…");
  await sleep(100);
  assert.deepEqual(await modal(), {
    title: "Remove the HP 49G ROM?",
    body: "It is deleted from this browser, with its saved state, as that contains the ROM. The file on your computer stays.",
    buttons: ["Cancel", "Remove"],
  });
  await answer("Cancel");
  assert.equal(await modal(), null, "Cancel closes it");
  assert.deepEqual(await p.ev("window.__removed"), []);

  await p.menu("40g");
  await p.choose("Remove…");
  await sleep(100);
  assert.equal((await modal()).title, "Remove the HP 39G and HP 40G ROM?");
  assert.match((await modal()).body, /^They use the same file, so both go\./);
  await answer("Remove");
  await sleep(300);
  assert.deepEqual(await p.ev("window.__removed"), ["39g", "40g"]);
  assert.equal(await p.ev("window.__unloaded"), 0, "nothing ran from it");
  assert.match(await p.ev("window.saturnus.store.state.message"), /^The HP 39G and HP 40G ROM is removed from this browser\.$/);

  // The 48GX runs: removing its ROM stops it, and its display says there is none.
  await p.ev(`window.saturnus.store.set({ booted: "48gx", model: "48gx" }); true`);
  await p.menu("48gx");
  await p.choose("Remove…");
  await sleep(100);
  assert.equal((await modal()).body, "It is deleted from this browser and the calculator stops. The file on your computer stays.");
  await answer("Remove");
  await sleep(300);
  assert.equal(await p.ev("window.__unloaded"), 1);
  assert.equal(await p.ev(`document.querySelector("sat-calculator .no-rom").hidden`), false, "the empty state is shown");
  const after = (await p.ev(ROWS)).find((r) => r.model === "HP 48GX");
  assert.equal(after.label, "Choose…");
  assert.equal(after.more, null, "an empty row in the browser has no ⋯");
});

test("while a send types, nothing is removed and the message says why", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(SLOTS("file"));
  // The engine refuses `unload` as while a send types (REFUSED_WHILE_TYPING).
  await p.ev(`(() => {
    window.__rejections = [];
    addEventListener("unhandledrejection", (e) => window.__rejections.push(String(e.reason)));
    window.saturnus.backend.unload = async () => { window.__unloaded++; throw new Error("typing is in progress (releaseAll stops it)"); };
    window.saturnus.store.set({ booted: "48gx", model: "48gx" });
    return true;
  })()`);
  const answer = (text) => p.ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);
  await p.menu("48gx");
  await p.choose("Remove…");
  await sleep(100);
  await answer("Remove");
  await sleep(300);
  assert.equal(await p.ev("window.__unloaded"), 1, "asked to stop");
  assert.deepEqual(await p.ev("window.__removed"), [], "not forgotten");
  assert.equal((await p.ev(ROWS)).find((r) => r.model === "HP 48GX").label, "Change…", "still listed");
  assert.equal(await p.ev("window.saturnus.store.state.message"), "The HP 48GX is typing a send; remove its ROM when it's done.");
  assert.equal(await p.ev("window.saturnus.store.state.messageError"), true);

  // Remove ROMs… the same.
  await p.ev(`document.getElementById("rom-forget").click(); true`);
  await sleep(100);
  await answer("Remove");
  await sleep(300);
  assert.deepEqual(await p.ev("window.__removed"), []);
  assert.equal(await p.ev("window.saturnus.store.state.message"), "The HP 48GX is typing a send; remove the ROMs when it's done.");
  assert.deepEqual(await p.ev("window.__rejections"), [], "no unhandled rejection");
});

test("the table fits a phone's sheet", { timeout: 120_000 }, async (t) => {
  const p = await page(t, 360, 780, true);
  if (!p) return;
  await p.ev(SLOTS("file"));
  await p.ev(`document.querySelector("#bar-menu")?.click(); true`);
  await sleep(300);
  assert.equal(await p.ev("document.documentElement.scrollWidth - document.documentElement.clientWidth"), 0);
  const rows = await p.ev(ROWS);
  assert.equal(new Set(rows.map((r) => r.height)).size, 1, `one height: ${rows.map((r) => r.height)}`);
  twoLines(rows);
  const inside = await p.ev(`(() => { const t = document.querySelector(".rom-slots").getBoundingClientRect(); return t.right <= innerWidth; })()`);
  assert.equal(inside, true);
});
