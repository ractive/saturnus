// The saved states in the real page in headless Chrome (iteration 34):
// Save state at one click with the message of what it replaced; Load
// state off with its reason, then asking in the shared modal, Cancel
// leaving the machine untouched; the "⋯" menu's file commands, a file
// named after the model and the time, a file's question naming it, a
// state of another model and a broken file refused in plain words; the
// four in the palette. Both hosts' page (the browser's file input, the
// app's dialogs); the backend's state calls are stubbed and recorded.
// Skipped without Chrome (`SATURNUS_CHROME` names one) or the wasm
// package (`just web`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { session, sleep } from "./chrome.mjs";

/**
 * The page with a booted 48GX and `romSource` set; the backend's state
 * calls answer from `window.__slot` (the save's time, or null) and
 * record into `window.__calls`. `openStateFile` answers `window.__open`.
 */
const STUB = (source) => `(() => {
  const s = window.saturnus.store;
  const b = window.saturnus.backend;
  b.romSource = ${JSON.stringify(source)};
  window.__slot = null;
  window.__calls = [];
  window.__open = null;
  b.stateSlot = async (m) => ({ saved: window.__slot });
  b.saveStateSlot = async (m) => { window.__calls.push(["save", m]); window.__slot = Date.now(); return { saved: window.__slot }; };
  b.loadStateSlot = async (m) => { window.__calls.push(["load", m]); };
  b.saveStateFile = async (m, name) => { window.__calls.push(["saveFile", m, name]); return name; };
  b.openStateFile = async () => window.__open;
  b.loadStateBytes = async (bytes) => { window.__calls.push(["loadBytes", bytes.length]); };
  /** A state's first bytes for the model code \`code\` (crates/saturnus-core/src/state.rs). */
  window.__state = (code, magic = "SATURNUS") => {
    const x = new Uint8Array(64);
    for (let i = 0; i < magic.length; i++) x[i] = magic.charCodeAt(i);
    x[8] = 2;
    x[10] = code;
    return x;
  };
  s.set({ booted: "48gx", model: "48gx" });
  return true;
})()`;

const MODAL = `(() => { const d = document.querySelector("dialog.confirm"); return d && d.open ? { title: d.querySelector("h2").textContent, body: d.querySelector("p").textContent, buttons: [...d.querySelectorAll("button")].map((b) => b.textContent) } : null; })()`;

async function page(t, source) {
  const c = await session(t);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started");
  await ev(STUB(source));
  await sleep(200);
  const click = async (sel) => {
    await ev(`document.querySelector(${JSON.stringify(sel)}).click(), true`);
    await sleep(200);
  };
  const answer = async (text) => {
    await ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);
    await sleep(200);
  };
  const menu = async () => {
    await click("#state-more");
    return ev(`[...document.querySelectorAll(".menu [role=menuitem]")].map((e) => e.textContent)`);
  };
  const choose = async (text) => {
    await ev(`[...document.querySelectorAll(".menu [role=menuitem]")].find((b) => b.textContent === ${JSON.stringify(text)}).click(), true`);
    await sleep(200);
  };
  const message = () => ev("window.saturnus.store.state.message");
  const calls = () => ev("window.__calls");
  return { ...c, click, answer, menu, choose, message, calls, modal: () => ev(MODAL) };
}

test("Save state replaces at once and says what; Load state asks, Cancel leaves the machine", { timeout: 120_000 }, async (t) => {
  const p = await page(t, "file");
  if (!p) return;
  assert.equal(await p.ev(`document.getElementById("save").title`), "Save the calculator as it is now, in this browser.");
  assert.equal(await p.ev(`document.getElementById("reset").title`), "Restart the calculator. Its memory is kept.");
  // No save yet: Load state is off and says why; the ⋯ is on.
  assert.deepEqual(await p.ev(`[document.getElementById("load").disabled, document.getElementById("load").title, document.getElementById("state-more").disabled]`), [true, "No saved state for this model", false]);
  await p.click("#save");
  assert.equal(await p.message(), "State saved.");
  assert.deepEqual(await p.calls(), [["save", "48gx"]]);
  assert.equal(await p.ev(`document.getElementById("load").disabled`), false, "a save turns Load state on");
  assert.match(await p.ev(`document.getElementById("load").title`), /^Go back to the state saved \d{1,2} [A-Z][a-z]{2} \d{4}, \d\d:\d\d\. What you did since is lost\.$/);
  assert.match(await p.ev(`document.getElementById("save").title`), /^Save the calculator as it is now\. Replaces the state saved \d{1,2} [A-Z][a-z]{2} \d{4}, \d\d:\d\d\.$/);

  // A second save names the first one's time.
  await p.ev(`window.__slot = new Date(new Date().getFullYear(), 9, 9, 14, 2).getTime(); true`);
  await p.click("#save");
  assert.equal(await p.message(), "State saved; it replaces the one from 9 Oct, 14:02.");

  // Load state asks, naming the save; Cancel loads nothing.
  await p.ev(`window.__slot = new Date(2026, 9, 9, 16, 40).getTime(); window.__calls = []; true`);
  await p.click("#load");
  assert.deepEqual(await p.modal(), {
    title: "Load the state saved on 9 Oct 2026, 16:40?",
    body: "It replaces the calculator's memory, stack and screen.",
    buttons: ["Cancel", "Load"],
  });
  await p.answer("Cancel");
  assert.equal(await p.modal(), null);
  assert.deepEqual(await p.calls(), [], "Cancel leaves the machine untouched");
  await p.click("#load");
  await p.answer("Load");
  assert.deepEqual(await p.calls(), [["load", "48gx"]]);
  assert.equal(await p.message(), "Loaded the state saved on 9 Oct 2026, 16:40.");

  // Nothing running: everything is off and says so.
  await p.ev(`window.saturnus.store.set({ booted: null }); true`);
  await sleep(200);
  assert.deepEqual(await p.ev(`["save", "load", "state-more"].map((id) => [document.getElementById(id).disabled, document.getElementById(id).title])`), [
    [true, "Start the calculator first"], [true, "Start the calculator first"], [true, "Start the calculator first"],
  ]);
});

test("the ⋯ menu's files in the browser: a dated download; a file refused, asked, loaded", { timeout: 120_000 }, async (t) => {
  const p = await page(t, "file");
  if (!p) return;
  assert.deepEqual(await p.menu(), ["Save state to a file…", "Load state from a file…"]);
  assert.match(await p.ev(`document.querySelector(".menu [role=menuitem]").title`), /A 49G state contains the 49G's ROM\./);
  await p.choose("Save state to a file…");
  const [[kind, model, name]] = await p.calls();
  assert.deepEqual([kind, model], ["saveFile", "48gx"]);
  assert.match(name, /^48gx-\d{4}-\d\d-\d\d-\d{4}\.sat$/);
  assert.equal(await p.message(), `State saved as ${name}.`);
  assert.equal(await p.ev(`document.getElementById("state-file").accept`), ".sat,.state");

  // A file picked: through the page's file input.
  const pick = (bytes, fileName) => p.ev(`(() => {
    const dt = new DataTransfer();
    dt.items.add(new File([${bytes}], ${JSON.stringify(fileName)}));
    const input = document.getElementById("state-file");
    input.files = dt.files;
    input.dispatchEvent(new Event("change"));
    return true;
  })()`);
  await p.ev("window.__calls = []; true");
  await pick("window.__state(0)", "sx.sat");
  await sleep(300);
  assert.equal(await p.modal(), null, "another model's state is not asked about");
  assert.equal(await p.message(), "This state is from an HP 48SX. Switch to the HP 48SX first, then load it.");
  assert.equal(await p.ev("window.saturnus.store.state.messageError"), true);
  await pick(`new TextEncoder().encode("not a state at all, just text")`, "notes.txt");
  await sleep(300);
  assert.equal(await p.message(), "This file is not a saturnus state.");
  assert.deepEqual(await p.calls(), []);

  await pick("window.__state(1)", "48gx-2026-10-09-1640.sat");
  await sleep(300);
  assert.equal((await p.modal()).title, "Load the state in 48gx-2026-10-09-1640.sat?");
  await p.answer("Cancel");
  assert.deepEqual(await p.calls(), [], "Cancel leaves the machine untouched");
  await pick("window.__state(1)", "48gx-2026-10-09-1640.sat");
  await sleep(300);
  await p.answer("Load");
  assert.deepEqual(await p.calls(), [["loadBytes", 64]]);
  assert.equal(await p.message(), "Loaded the state in 48gx-2026-10-09-1640.sat.");

  // The core's refusal of another ROM, in plain words.
  await p.ev(`window.saturnus.backend.loadStateBytes = async () => { throw new Error("saved state was made with a different ROM"); }; true`);
  await pick("window.__state(1)", "other.sat");
  await sleep(300);
  await p.answer("Load");
  assert.equal(await p.message(), "This state was saved with another ROM of the HP 48GX; it loads only with that one.");
});

test("the app's page: the open dialog's file is checked and asked about; the palette has all four", { timeout: 120_000 }, async (t) => {
  const p = await page(t, "dialog");
  if (!p) return;
  await p.ev(`window.__open = { name: "old.state", bytes: window.__state(1, "SATURNUZ") }; true`);
  await p.menu();
  await p.choose("Load state from a file…");
  await sleep(200);
  assert.equal(await p.message(), "This file is not a saturnus state.");
  await p.ev(`window.__open = { name: "old.state", bytes: window.__state(1) }; true`);
  await p.menu();
  await p.choose("Load state from a file…");
  await sleep(200);
  assert.equal((await p.modal()).title, "Load the state in old.state?");
  await p.answer("Load");
  assert.deepEqual(await p.calls(), [["loadBytes", 64]]);
  // The app could not read the file: said, not swallowed.
  await p.ev(`window.saturnus.backend.openStateFile = async () => { throw new Error("big.sat is larger than 4194304 bytes"); }; window.__calls = []; true`);
  await p.menu();
  await p.choose("Load state from a file…");
  await sleep(200);
  assert.equal(await p.message(), "This file is damaged or not a saturnus state.");
  assert.equal(await p.ev("window.saturnus.store.state.messageError"), true);
  await p.ev(`window.saturnus.backend.openStateFile = async () => { throw new Error("cannot read x.sat: Permission denied"); }; true`);
  await p.menu();
  await p.choose("Load state from a file…");
  await sleep(200);
  assert.equal(await p.message(), "Could not load the state: cannot read x.sat: Permission denied");
  assert.equal(await p.modal(), null);
  assert.deepEqual(await p.calls(), []);
  await p.ev(`window.saturnus.backend.openStateFile = async () => window.__open; true`);
  // Cancelled dialog: nothing.
  await p.ev(`window.__open = null; window.__calls = []; true`);
  await p.menu();
  await p.choose("Load state from a file…");
  assert.equal(await p.modal(), null);
  assert.deepEqual(await p.calls(), []);

  const ids = (await p.ev(`window.saturnus.palette.actions().map((a) => a.id)`));
  for (const id of ["save", "save-file", "load", "load-file"]) assert.ok(ids.includes(id), `${id} in ${ids}`);
  const load = `window.saturnus.palette.actions().find((a) => a.id === "load").off`;
  assert.equal(await p.ev(load), "No saved state for this model", "off without a save, and says why");
  await p.ev(`window.__slot = Date.now(); true`);
  await p.click("#save");
  assert.equal(await p.ev(load), null);
});
