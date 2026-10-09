// The ROM table ("ROMs of every model") in the real page in headless
// Chrome: one button per row and single-height rows; the Add and Change
// menus per host and slot; Remove… asks in the row, takes the 39G's and
// 40G's shared file together, and stops the model that runs from it; no
// overflow on a phone. The page's ROM slots and the backend's answers are
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
  s.set({ roms: { slots, offers: [], lastModel: null, bootLast: true, remembered: true, note: null } });
  document.getElementById("roms").open = true;
  return true;
})()`;

/** Each row: the model, its button's text and whether it opens a menu, its height. */
const ROWS = `[...document.querySelectorAll(".rom-slots tr:not(.rom-remove)")].map((tr) => {
  const b = tr.querySelectorAll("td.rom-act button");
  return { model: tr.querySelector("th").textContent, buttons: b.length, label: b[0]?.textContent, menu: b[0]?.getAttribute("aria-haspopup") === "menu", height: Math.round(tr.getBoundingClientRect().height), width: Math.round(b[0]?.getBoundingClientRect().width ?? 0) };
})`;

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

test("one button per row, one height; the browser's menus", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(SLOTS("file"));
  const rows = await p.ev(ROWS);
  assert.ok(rows.every((r) => r.buttons === 1), JSON.stringify(rows));
  assert.equal(new Set(rows.map((r) => r.height)).size, 1, `one height: ${rows.map((r) => r.height)}`);
  assert.equal(new Set(rows.map((r) => r.width)).size, 1, `one width: ${rows.map((r) => r.width)}`);
  const by = Object.fromEntries(rows.map((r) => [r.model, r]));
  assert.equal(by["HP 38G"].label, "Choose…", "empty in the browser: Choose…, the row links to hpcalc.org");
  assert.equal(by["HP 38G"].menu, false);
  assert.equal(await p.ev(`[...document.querySelectorAll(".rom-slots tr")].find((tr) => tr.querySelector("th").textContent === "HP 38G").querySelector("a")?.textContent`), "Download");
  assert.equal(by["HP 42S"].label, "Choose…");
  assert.equal(await p.ev(`[...document.querySelectorAll(".rom-slots tr")].find((tr) => tr.querySelector("th").textContent === "HP 42S").querySelector(".rom-file-name").textContent`), "none");
  assert.equal(by["HP 48GX"].menu, true);
  assert.deepEqual(await p.menu("48gx"), ["Choose another file…", "-", "Remove…"]);
  assert.equal(await p.ev(`document.getElementById("rom-forget").textContent`), "Remove ROMs…");
});

test("the app's menus: Add downloads or chooses; Change also downloads again", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(SLOTS("dialog"));
  const by = Object.fromEntries((await p.ev(ROWS)).map((r) => [r.model, r]));
  assert.equal(by["HP 38G"].label, "Add");
  assert.deepEqual(await p.menu("38g"), ["Download from hpcalc.org…", "Choose a file…"]);
  await p.ev(`document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })); true`);
  await sleep(100);
  assert.deepEqual(await p.menu("48gx"), ["Choose another file…", "Download again…", "-", "Remove…"]);
  assert.equal(by["HP 42S"].label, "Choose…", "no download for the 42S");
});

test("Remove… asks in the row; the 39G and 40G go together; a running model stops", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(SLOTS("file"));
  const question = () => p.ev(`document.querySelector(".rom-remove p")?.textContent ?? null`);
  await p.menu("49g");
  await p.choose("Remove…");
  assert.equal(await question(), "Remove the HP 49G ROM from this browser? Its saved state goes too, as it contains the ROM. The file on your computer stays.");
  assert.deepEqual(await p.ev(`[...document.querySelectorAll(".rom-remove button")].map((b) => b.textContent)`), ["Cancel", "Remove"]);
  await p.ev(`document.querySelector('.rom-remove button[data-answer="no"]').click(); true`);
  assert.equal(await question(), null, "Cancel takes the question away");
  assert.deepEqual(await p.ev("window.__removed"), []);

  await p.menu("40g");
  await p.choose("Remove…");
  assert.match(await question(), /^Remove the HP 39G and HP 40G ROM from this browser\? They use the same file, so both go\./);
  await p.ev(`document.querySelector('.rom-remove button[data-answer="yes"]').click(); true`);
  await sleep(300);
  assert.deepEqual(await p.ev("window.__removed"), ["39g", "40g"]);
  assert.equal(await p.ev("window.__unloaded"), 0, "nothing ran from it");
  assert.match(await p.ev("window.saturnus.store.state.message"), /^The HP 39G and HP 40G ROM is removed from this browser\.$/);

  // The 48GX runs: removing its ROM stops it, and its display says there is none.
  await p.ev(`window.saturnus.store.set({ booted: "48gx", model: "48gx" }); true`);
  await p.menu("48gx");
  await p.choose("Remove…");
  await p.ev(`document.querySelector('.rom-remove button[data-answer="yes"]').click(); true`);
  await sleep(300);
  assert.equal(await p.ev("window.__unloaded"), 1);
  assert.equal(await p.ev(`document.querySelector("sat-calculator .no-rom").hidden`), false, "the empty state is shown");
  assert.equal((await p.ev(ROWS)).find((r) => r.model === "HP 48GX").label, "Choose…");
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
  const inside = await p.ev(`(() => { const t = document.querySelector(".rom-slots").getBoundingClientRect(); return t.right <= innerWidth; })()`);
  assert.equal(inside, true);
});
