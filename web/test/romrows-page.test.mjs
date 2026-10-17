// The home page's calculators and their ROMs (iteration 53) in the real
// page in headless Chrome: a kept calculator's ⋯ (Change ROM…, Download
// again… in the app, Remove…), New calculator's models with Choose… and
// where to download (the app's Download…), the hints per host and
// pointer; Remove… asks in the shared modal, takes the 39G's and 40G's
// shared file together and says when the calculator stops; a removal
// refused while a send types says why and removes nothing; Remove ROMs…
// asks first; a choice with a notice or an offer stays on the page; the
// start setting offers no keeping; nothing overflows a phone. The ROM commands
// (web/home-roms.js) are stubbed. Skipped without Chrome
// (`SATURNUS_CHROME` names one) or the wasm package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

/** The home page's ROM commands stubbed for `source`, the removes recorded (`__removed`), the primaries' actions too (`__acted`). */
const STUB = (source, running = []) => `(() => {
  const h = saturnusApp.home;
  const d = (file) => ({ file, size: 1, url: "https://www.hpcalc.org/x.zip", page: "https://www.hpcalc.org/details/1", revision: "rev" });
  let slots = { slots: [
    { model: "48sx", fileName: "sxrom-j", revision: "J", state: "ready", download: d("sxrom-j") },
    { model: "48gx", fileName: "gxrom-r", revision: "R", state: "ready", download: d("gxrom-r") },
    { model: "49g", fileName: "rom.49g", revision: "2.10", state: "ready", download: d("rom.49g") },
    { model: "38g", fileName: null, revision: null, state: "empty", download: d("38G_A167.ROM") },
    { model: "39g", fileName: "rom.39g", revision: "C", state: "ready", download: d("rom.39g") },
    { model: "40g", fileName: "rom.39g", revision: "C", state: "ready", download: d("rom.39g") },
    { model: "42s", fileName: null, revision: null, state: "empty", download: null },
    { model: "28s", fileName: null, revision: null, state: "empty", download: null },
  ], offers: [], lastModel: null, bootLast: true, remembered: true, note: null };
  window.__removed = [];
  window.__acted = [];
  window.__refuse = null;
  h.roms.romSource = ${JSON.stringify(source)};
  h.app = ${JSON.stringify(source)} === "dialog";
  h.roms.started = () => true;
  h.roms.romSlots = async () => slots;
  h.roms.running = async () => ${JSON.stringify(running)};
  h.roms.forgetRom = async (model) => {
    if (window.__refuse) throw new Error(window.__refuse);
    window.__removed.push(model);
    slots = { ...slots, slots: slots.slots.map((x) => (model === null || x.model === model ? { ...x, fileName: null, revision: null, state: "empty" } : x)) };
    return slots;
  };
  h.choose = (m) => window.__acted.push(["choose", m]);
  h.download = (m) => window.__acted.push(["download", m]);
  return h.render().then(() => true);
})()`;

async function page(t, { width = 1280, height = 900, mobile = false } = {}) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  if (mobile) await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnusApp?.home").catch(() => false)); i++) await sleep(100);
  /** The items of `model`'s ⋯ menu, opened by its button. */
  const menu = async (model) => {
    await ev(`document.querySelector('#home-list button[data-menu="${model}"]').click(); true`);
    await sleep(150);
    return ev(`[...document.querySelectorAll(".menu [role=menuitem], .menu [role=separator]")].map((e) => e.getAttribute("role") === "separator" ? "-" : e.textContent)`);
  };
  const choose = (text) => ev(`[...document.querySelectorAll(".menu [role=menuitem]")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);
  const modal = () => ev(`(() => { const d = document.querySelector("dialog.confirm"); return d && d.open ? { title: d.querySelector("h2").textContent, body: d.querySelector("p").textContent, buttons: [...d.querySelectorAll("button")].map((b) => b.textContent) } : null; })()`);
  const answer = (text) => ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);
  const message = () => ev("saturnusApp.home.store.state.message");
  /** New calculator's rows: the model, its buttons, its line under the name (and its link). */
  const fresh = async () => {
    if (await ev(`document.getElementById("home-new-section").hidden`)) await ev(`document.getElementById("home-new").click(); true`);
    await sleep(200);
    return ev(`[...document.querySelectorAll("#home-models li")].map((li) => ({ model: li.dataset.model, buttons: [...li.querySelectorAll("button")].map((b) => b.textContent), line: li.querySelector(".home-rom").textContent, link: li.querySelector(".home-rom a")?.getAttribute("href") ?? null }))`);
  };
  return { ...c, menu, choose, modal, answer, message, fresh };
}

test("the browser's: a kept calculator's ⋯, New calculator's Choose… and Download link, the hints", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(STUB("file"));
  assert.deepEqual(await p.ev(`[...document.querySelectorAll("#home-list li")].map((li) => li.dataset.model)`), ["48sx", "48gx", "49g", "39g", "40g"]);
  assert.deepEqual(await p.menu("48gx"), ["Change ROM…", "-", "Remove…"]);
  await p.choose("Change ROM…");
  await sleep(100);
  assert.deepEqual(await p.ev("window.__acted"), [["choose", "48gx"]]);
  const rows = await p.fresh();
  assert.deepEqual(rows, [
    { model: "38g", buttons: ["Choose…"], line: "Download from hpcalc.org", link: "https://www.hpcalc.org/details/1" },
    { model: "42s", buttons: ["Choose…"], line: "HP never published it: read it out of your own calculator.", link: null },
    { model: "28s", buttons: ["Choose…"], line: "HP never published it: read it out of your own calculator.", link: null },
  ]);
  await p.ev(`document.querySelector('#home-models button[data-model="38g"]').click(); true`);
  await sleep(100);
  assert.deepEqual(await p.ev("window.__acted"), [["choose", "48gx"], ["choose", "38g"]]);
  assert.equal(await p.ev(`document.getElementById("home-new-hint").textContent`), "Choose the model's ROM file, or drop it on this page. Several files at once each go to their model.");
  assert.match(await p.ev(`document.getElementById("home-source-hint").textContent`), /^Download opens the model's page on hpcalc\.org/);
  assert.match(await p.ev(`document.getElementById("home-kept-hint").textContent`), /^Kept in this browser/);
  assert.equal(await p.ev(`document.querySelector("#home-settings .home-boot-last-label").textContent`), "Start the last calculator when the app opens");
  assert.equal(await p.ev(`document.getElementById("home-boot-last").checked`), true);
  assert.equal(await p.ev(`document.getElementById("home-forget").textContent`), "Remove ROMs…");
});

test("the app's: Download again… in the ⋯; Download… and Choose… for a new calculator", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(STUB("dialog"));
  assert.deepEqual(await p.menu("48gx"), ["Change ROM…", "Download again…", "-", "Remove…"]);
  await p.choose("Download again…");
  await sleep(100);
  const rows = await p.fresh();
  assert.deepEqual(rows.find((r) => r.model === "38g"), { model: "38g", buttons: ["Download…", "Choose…"], line: "38G_A167.ROM, from hpcalc.org", link: null });
  assert.deepEqual(rows.find((r) => r.model === "42s").buttons, ["Choose…"], "no download for the 42S");
  await p.ev(`document.querySelector('#home-models button[data-act="download"][data-model="38g"]').click(); true`);
  await sleep(100);
  assert.deepEqual(await p.ev("window.__acted"), [["download", "48gx"], ["download", "38g"]]);
});

test("Remove… asks in the modal; the 39G and 40G go together; the running calculator stops", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(STUB("file", ["48gx"]));
  await p.menu("49g");
  await p.choose("Remove…");
  await sleep(200);
  assert.deepEqual(await p.modal(), {
    title: "Remove the HP 49G ROM?",
    body: "It is deleted from this browser, with its saved state, as that contains the ROM. The file on your computer stays.",
    buttons: ["Cancel", "Remove"],
  });
  await p.answer("Cancel");
  await sleep(100);
  assert.equal(await p.modal(), null, "Cancel closes it");
  assert.deepEqual(await p.ev("window.__removed"), []);

  await p.menu("40g");
  await p.choose("Remove…");
  await sleep(200);
  assert.equal((await p.modal()).title, "Remove the HP 39G and HP 40G ROM?");
  assert.match((await p.modal()).body, /^They use the same file, so both go\./);
  await p.answer("Remove");
  await sleep(300);
  assert.deepEqual(await p.ev("window.__removed"), ["39g", "40g"]);
  assert.equal(await p.message(), "The HP 39G and HP 40G ROM is removed from this browser.");

  // The 48GX runs: the question says it stops.
  await p.menu("48gx");
  await p.choose("Remove…");
  await sleep(200);
  assert.equal((await p.modal()).body, "It is deleted from this browser and the calculator stops. The file on your computer stays.");
  await p.answer("Remove");
  await sleep(300);
  assert.deepEqual(await p.ev("window.__removed"), ["39g", "40g", "48gx"]);
  assert.deepEqual(await p.ev(`[...document.querySelectorAll("#home-list li")].map((li) => li.dataset.model)`), ["48sx", "49g"]);
  assert.ok(await p.ev(`[...document.querySelectorAll("#home-models li")].some((li) => li.dataset.model === "48gx")`), "a new calculator again");
});

test("while a send types, nothing is removed and the message says why; Remove ROMs… asks first", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(STUB("file", ["48gx"]));
  await p.ev(`(() => {
    window.__rejections = [];
    addEventListener("unhandledrejection", (e) => window.__rejections.push(String(e.reason)));
    window.__refuse = "typing is in progress (releaseAll stops it)";
    return true;
  })()`);
  await p.menu("48gx");
  await p.choose("Remove…");
  await sleep(200);
  await p.answer("Remove");
  await sleep(300);
  assert.deepEqual(await p.ev("window.__removed"), [], "not forgotten");
  assert.ok(await p.ev(`!!document.querySelector('#home-list li[data-model="48gx"]')`), "still listed");
  assert.equal(await p.message(), "The HP 48GX is typing a send; remove its ROM when it's done.");
  assert.equal(await p.ev("saturnusApp.home.store.state.messageError"), true);

  // Remove ROMs… asks first: Cancel keeps them; while the send types, the same refusal.
  await p.ev(`document.getElementById("home-forget").click(); true`);
  await sleep(200);
  assert.deepEqual(await p.modal(), {
    title: "Remove all ROMs?",
    body: "They are deleted from this browser, with the saved 49G state, as it contains the ROM. Other saved states and the files on your computer stay.",
    buttons: ["Cancel", "Remove"],
  });
  await p.answer("Cancel");
  await sleep(200);
  assert.deepEqual(await p.ev("window.__removed"), [], "Cancel removes nothing");
  await p.ev(`document.getElementById("home-forget").click(); true`);
  await sleep(200);
  await p.answer("Remove");
  await sleep(300);
  assert.deepEqual(await p.ev("window.__removed"), []);
  assert.equal(await p.message(), "The HP 48GX is typing a send; remove the ROMs when it's done.");
  // The send done: removed, said.
  await p.ev(`window.__refuse = null; true`);
  await p.ev(`document.getElementById("home-forget").click(); true`);
  await sleep(200);
  await p.answer("Remove");
  await sleep(300);
  assert.deepEqual(await p.ev("window.__removed"), [null]);
  assert.equal(await p.message(), "ROMs and the saved 49G state removed. Other saved states stay.");
  assert.equal(await p.ev(`document.getElementById("home-settings").hidden`), true, "no ROM kept: no settings");
  assert.deepEqual(await p.ev("window.__rejections"), [], "no unhandled rejection");
});

test("a phone: nothing overflows, and a finger's screen is not told to drop a file", { timeout: 120_000 }, async (t) => {
  const p = await page(t, { width: 360, height: 780, mobile: true });
  if (!p) return;
  assert.equal(await p.ev(`matchMedia("(pointer: coarse)").matches`), true, "a coarse pointer");
  await p.ev(STUB("file"));
  await p.fresh();
  await sleep(300);
  assert.equal(await p.ev("document.documentElement.scrollWidth - document.documentElement.clientWidth"), 0);
  const out = await p.ev(`[...document.querySelectorAll("#home-view .home-calc :is(button, a.button)")].filter((b) => { const r = b.getBoundingClientRect(); return r.left < 0 || r.right > innerWidth; }).length`);
  assert.equal(out, 0, "every button inside the window");
  assert.doesNotMatch(await p.ev(`document.getElementById("home-new-section").textContent`), /drop/i);
});

test("a choice with a notice or an offer stays on the home page; a clean one opens its calculator; the setting offers no keeping", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  await p.ev(STUB("file"));
  // The start setting: a ROM command that keeps nothing (the slots before it read from the store, with no revision): no offer to keep.
  await p.ev(`(() => { const h = saturnusApp.home; h.store.set({ roms: { ...h.store.state.roms, slots: h.store.state.roms.slots.map((x) => ({ ...x, revision: null })) } }); return true; })()`);
  await p.ev(`(() => { const h = saturnusApp.home; h.roms.started = () => true; h.roms.romSettings = async (on) => ({ ...(await h.roms.romSlots()), bootLast: on }); return true; })()`);
  await p.ev(`document.getElementById("home-boot-last").click(); true`);
  await sleep(300);
  assert.equal(await p.ev(`document.getElementById("home-boot-last").checked`), false);
  assert.equal(await p.ev(`!!document.querySelector(".storage-notice")`), false, "no offer to keep a ROM");
  assert.equal(await p.ev("location.hash"), "#/");
  // Several files: one to start, a notice about the others: the page stays and shows it.
  await p.ev(`(() => { const h = saturnusApp.home; h.roms.chooseRom = async () => ({ ...h.store.state.roms, start: "48gx", notice: "rom.49g went to the HP 49G." }); return true; })()`);
  await p.ev(`saturnusApp.home.chooseFiles("48gx", [new File(["x"], "gx")]).then(() => true)`);
  assert.equal(await p.ev("location.hash"), "#/");
  assert.equal(await p.ev(`document.querySelector(".home-notice-text").textContent`), "rom.49g went to the HP 49G.");
  assert.equal(await p.ev(`document.getElementById("home-notice").hidden`), false);
  // A clean choice: its calculator opens.
  await p.ev(`(() => { const h = saturnusApp.home; h.roms.chooseRom = async () => ({ ...h.store.state.roms, start: "48gx", notice: "" }); return true; })()`);
  await p.ev(`saturnusApp.home.chooseFiles("48gx", [new File(["x"], "gx")]).then(() => true)`);
  assert.equal(await p.ev("location.hash"), "#/calculator/48gx");
});
