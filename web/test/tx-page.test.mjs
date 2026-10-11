// The transfer page (web/tx.html) in headless Chrome, without a ROM.
//
// - A FakeLink plays the emulated 48GX of the recorded page session
//   (web/test/fixtures/tx-page.trace, `just tx-page-trace`): connect,
//   list HOME, a text file dropped, a plain file as a string after
//   asking, the text file again after "Replace PRG?", NOTE previewed, PRG
//   downloaded, disconnect, writing exactly the recorded bytes.
// - The two panes with a fake calculator (web/test/tx-fake-calc.js, in
//   place of the protocol Worker) and a folder in memory (the fake
//   folder): rows dragged pane to pane and files dropped from outside,
//   "Replace X?" on both sides (Cancel keeps the old), purge and delete
//   with the count of what a directory holds, rename and new directory on
//   both sides, a copy between two calculators of different families
//   with the plan's warning before anything is sent, the queue (one job
//   per place at a time, the waiting ones cancellable, progress in
//   bytes), backup into the folder and a restore with its question, the
//   restart and the reconnect.
// - No horizontal overflow at 360, 390, 430, 768 and 1280 px in both
//   themes on every view (the panes, the stacked switch, the menus, the
//   dialogs, the copy plan's warning, the queue, the restore question).
// - A browser without Web Serial gets the plain message.
// - Folders only in the right pane: the left's "Change" menu and card
//   offer calculators only, the right's a folder and a calculator, and a
//   kept folder comes back on the right only.
// Skipped without Chrome or the wasm packages (`just web tx-web`;
// `SATURNUS_AUDIT=1` makes the skip a failure).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { session, sleep } from "./chrome.mjs";
import { PAGE_FILES, PAGE_SEED, pageSession, txBuilt, txPage } from "./tx-chrome.mjs";

const TRACE = readFileSync(new URL("./fixtures/tx-page.trace", import.meta.url), "utf8");
const WIDTHS = [360, 390, 430, 768, 1280];
const HEIGHTS = { 360: 780, 390: 844, 430: 932, 768: 1024, 1280: 900 };
/* Containers that may scroll sideways by design: the list (wide names), the tree, a matrix, a picture. */
const MAY_SCROLL = [".dlist-wrap", ".dtree", "device-tree", ".grid-wrap", ".graphic-wrap"];

const MEASURE = `(() => {
  const de = document.documentElement;
  const sel = (e) => e.tagName.toLowerCase() + (e.id ? "#" + e.id : "") + (typeof e.className === "string" && e.className ? "." + e.className.trim().split(/\\s+/).join(".") : "");
  const visible = (e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== "hidden"; };
  const scrollers = [];
  const outside = [];
  for (const e of document.querySelectorAll("body *")) {
    if (!visible(e) || e.closest("svg")) continue;
    if (/auto|scroll/.test(getComputedStyle(e).overflowX) && e.scrollWidth > e.clientWidth + 1 && !e.matches(${JSON.stringify(MAY_SCROLL.join(","))})) scrollers.push(sel(e) + " +" + (e.scrollWidth - e.clientWidth));
  }
  for (const e of document.querySelectorAll("dialog[open], .menu")) {
    const r = e.getBoundingClientRect();
    if (r.left < -0.5 || r.right > innerWidth + 0.5) outside.push(sel(e));
  }
  return { page: de.scrollWidth - de.clientWidth, inner: innerWidth, scrollers, outside };
})()`;

/** A Chrome session for the page, or null (skipped). */
async function open(t) {
  if (!txBuilt()) {
    if (process.env.SATURNUS_AUDIT) assert.fail("web/pkg or web/tx/pkg not built (just web tx-web)");
    t.skip("web/pkg or web/tx/pkg not built (just web tx-web)");
    return null;
  }
  return session(t, { audit: true });
}

const title = (side) => `document.querySelector(".pane[data-pane=${side}] .pane-title").textContent`;

test("the page with a recorded calculator: connect, list, drop, replace, preview, download, disconnect", { timeout: 180_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  const p = txPage(c);
  await p.open(c.port);
  assert.equal(await c.ev(title("left")), "No calculator");
  await c.ev(`window.__link = new tx.links.FakeLink(${JSON.stringify(TRACE)}); tx.app.deviceOptions = { seed: ${PAGE_SEED} }; tx.panes[0].connectLink(__link, { model: "48gx" }); true`);
  await p.until("!!tx.panes[0].place?.device.connection", 60_000, "connected");
  assert.equal(await c.ev(title("left")), "HP 48G/GX on test link");
  // The live region hears steps, not packets: progress goes beside it.
  await c.ev(`(() => {
    window.__live = 0; window.__progress = 0;
    const pane = document.querySelector(".pane[data-pane=left]");
    new MutationObserver(() => __live++).observe(pane.querySelector(".pane-status-text"), { childList: true, characterData: true, subtree: true });
    new MutationObserver(() => __progress++).observe(pane.querySelector(".pane-progress"), { childList: true, characterData: true, subtree: true });
    return true;
  })()`);
  const seen = await pageSession(c);
  const live = await c.ev(`({ live: __live, progress: __progress, outside: document.querySelector(".pane[data-pane=left] .pane-progress").closest("[role=status]") === null })`);
  assert.ok(live.outside, "the progress is not in the live region");
  assert.ok(live.progress > 0, "the progress was shown");
  assert.ok(live.live < 40, `the live region changed ${live.live} times in the session`);
  assert.deepEqual(seen.home, ["IOPAR"]);
  assert.deepEqual([...seen.after].sort(), ["IOPAR", "NOTE", "PRG"]);
  assert.equal(seen.preview, PAGE_FILES.note);
  assert.equal(seen.download.name, "PRG.hp");
  assert.equal(String.fromCharCode(...seen.download.bytes.slice(0, 7)), "HPHP48-");
  assert.equal(seen.note, "The calculator has left server mode.");
  assert.equal(await c.ev("__link.failure"), null);
  assert.equal(await c.ev("__link.next === __link.steps.length"), true, "every recorded step was played");
  assert.deepEqual(c.errors, []);
});

// ---------------------------------------------------------------- the fake calculator and folder

/**
 * The page with the fake calculator module loaded (`__fake`), and helpers:
 * `__calc(i, model, opts)` puts a fake calculator of `model` into pane `i`
 * (its requests in `__calcs[i].requests`), `__folder(i)` a folder in memory.
 */
async function fakePage(t, width = 1280) {
  const c = await open(t);
  if (!c) return null;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width, height: width < 1000 ? 844 : 900, deviceScaleFactor: 1, mobile: width < 1000 });
  const p = txPage(c);
  await p.open(c.port);
  await c.ev(`import("./test/tx-fake-calc.js").then((m) => {
    window.__fake = m;
    window.__calcs = [];
    window.__calc = async (i, model = "48gx", opts = {}) => {
      const calc = m.fakeCalc({ model, ...opts });
      __calcs[i] = calc;
      tx.app.deviceOptions = { worker: calc.worker };
      const link = m.stubLink(opts.label ?? "stub link");
      await tx.panes[i].connectLink(link, { model });
      return true;
    };
    window.__folder = (i, title = "hp-files") => tx.memoryFolder(i, title).then(() => true);
    return true;
  })`);
  return { c, p };
}

/** Put a few variables on the fake calculator of pane `i`: PRG, X, and a directory GAMES holding two. */
const SEED_CALC = (i = 0) => `(async () => {
  const pane = tx.panes[${i}];
  const d = pane.place.device;
  const enc = (t) => new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n" + t + "\\r\\n");
  await d.write(["HOME"], "PRG", enc("\\\\<< 1 2 + \\\\>>"));
  await d.write(["HOME"], "X", enc("42"));
  await d.mkdir(["HOME"], "GAMES");
  await d.write(["HOME", "GAMES"], "TETRIS", enc("\\\\<< 4 \\\\>>"));
  await d.write(["HOME", "GAMES"], "SNAKE", enc("\\\\<< 5 \\\\>>"));
  return true;
})()`;

/** Drag the rows `names` of pane `from` and drop them on pane `to` (onto directory row `into`, if given). */
const DRAG = (from, names, to, into = null) => `(async () => {
  const src = tx.panes[${from}].ui.list;
  const dt = new DataTransfer();
  src.select(null);
  for (const n of ${JSON.stringify(names)}) src.select(n, { add: true });
  const row = src.querySelector('tr[data-name="${names[0]}"]');
  row.dispatchEvent(new DragEvent("dragstart", { dataTransfer: dt, bubbles: true }));
  const target = ${into ? `tx.panes[${to}].ui.list.querySelector('tr[data-name="${into}"]')` : `tx.panes[${to}].ui.list`};
  target.dispatchEvent(new DragEvent("dragover", { dataTransfer: dt, bubbles: true, cancelable: true }));
  target.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  return true;
})()`;

/** Drop files from outside the page (`{name: text}`) on pane `to`, through a DataTransfer. */
const DROP_FILES = (files, to) => `(() => {
  const dt = new DataTransfer();
  for (const [n, text] of Object.entries(${JSON.stringify(files)})) dt.items.add(new File([text], n));
  const target = tx.panes[${to}].ui.list;
  target.dispatchEvent(new DragEvent("dragover", { dataTransfer: dt, bubbles: true, cancelable: true }));
  target.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  return true;
})()`;

const folderNames = (i = 1) => `(tx.panes[${i}].ui.list.listing ? tx.panes[${i}].ui.list.listing.entries.map((e) => e.name).sort() : null)`;
const folderFile = (path, name, i = 1) => `tx.panes[${i}].place.folder.read(${JSON.stringify(path)}, ${JSON.stringify(name)}).then((f) => new TextDecoder().decode(f.data))`;

/** Answer the open question with Cancel, after checking its title. */
async function cancel(p, c, titleText) {
  await p.until(`!!document.querySelector("dialog.confirm[open]")`, 10_000, `the question "${titleText}"`);
  assert.equal(await c.ev(`document.querySelector("dialog.confirm h2").textContent`), titleText);
  await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:first-child").click()`);
}

test("two panes: drag each way, drop from outside, Replace X? on both sides, Cancel keeps the old", { timeout: 120_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`__calc(0)`);
  await c.ev(`__folder(1)`);
  await p.idle(10_000, "the calculator listed");
  await p.idle(10_000, "the folder listed", 1);
  assert.equal(await c.ev(title("right")), "Folder hp-files");
  await c.ev(SEED_CALC(0));
  await p.idle(10_000, "seeded");
  assert.deepEqual((await p.names()).sort(), ["GAMES", "PRG", "X"]);

  // Calculator to folder: a variable and a directory (as a folder of .hp files).
  await c.ev(DRAG(0, ["PRG", "GAMES"], 1));
  await p.until(`${folderNames()}?.join() === "GAMES,PRG.hp"`, 10_000, "PRG and GAMES copied to the folder");
  const prg = await c.ev(folderFile(["hp-files"], "PRG.hp"));
  assert.match(prg, /^HPHP48-/);
  assert.deepEqual(await c.ev(`tx.panes[1].place.folder.list(["hp-files", "GAMES"]).then((l) => l.entries.map((e) => e.name).sort())`), ["SNAKE.hp", "TETRIS.hp"]);

  // Again: "Replace PRG.hp?" first; Cancel keeps the folder's file, nothing is got.
  await c.ev(`__calcs[0].requests.length = 0; window.__copy = null; tx.panes[0].copySelection([tx.entry("PRG")]).then((r) => { __copy = r ?? "none"; }); true`);
  await cancel(p, c, "Replace PRG.hp?");
  await p.until("__copy !== null", 5_000, "the copy ended");
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "get").length`), 0, "nothing got after Cancel");
  // And Replace replaces.
  await c.ev(`tx.panes[1].place.folder.write(["hp-files"], "PRG.hp", new TextEncoder().encode("old"), { replace: true }).then(() => true)`);
  const replaced = c.ev(`tx.panes[0].copySelection([tx.entry("PRG")]).then(() => true)`);
  await p.answer("Replace PRG.hp?");
  await replaced;
  assert.match(await c.ev(folderFile(["hp-files"], "PRG.hp")), /^HPHP48-/);

  // Folder to calculator: a file whose name is in use asks "Replace X?"; Cancel keeps it.
  await c.ev(`tx.panes[1].place.folder.write(["hp-files"], "X.txt", new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n99\\r\\n")).then(() => true)`);
  await p.until(`${folderNames()}?.includes("X.txt")`, 5_000, "X.txt listed");
  const before = await c.ev(`tx.entry("X").checksum`);
  await c.ev(`__calcs[0].requests.length = 0; true`);
  await c.ev(DRAG(1, ["X.txt"], 0));
  await cancel(p, c, "Replace X?");
  await sleep(300);
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "put").length`), 0, "nothing put after Cancel");
  assert.equal(await c.ev(`tx.entry("X").checksum`), before);
  await c.ev(DRAG(1, ["X.txt"], 0));
  await p.answer("Replace X?");
  await p.until(`tx.entry("X")?.checksum !== ${before}`, 10_000, "X replaced");
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "put" && r.replace).length`), 1);

  // A folder from the folder to the calculator: a directory with its contents.
  await c.ev(`tx.panes[1].place.folder.mkdir(["hp-files"], "NEW").then(() => tx.panes[1].place.folder.write(["hp-files", "NEW"], "A.txt", new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n1\\r\\n"))).then(() => true)`);
  await p.until(`${folderNames()}?.includes("NEW")`, 5_000, "NEW listed");
  await c.ev(DRAG(1, ["NEW"], 0));
  await p.until(`tx.entry("NEW")?.directory === true`, 10_000, "NEW on the calculator");
  assert.deepEqual(await c.ev(`tx.panes[0].place.device.list(["HOME", "NEW"]).then((l) => l.entries.map((e) => e.name))`), ["A"]);

  // Files dropped from outside the page: onto the calculator and onto the folder.
  await c.ev(DROP_FILES({ "Y.txt": "%%HP: T(3)A(R)F(.);\r\n7\r\n" }, 0));
  await p.until(`!!tx.entry("Y")`, 10_000, "Y put from a drop");
  await c.ev(DROP_FILES({ "notes.md": "hello" }, 1));
  await p.until(`${folderNames()}?.includes("notes.md")`, 10_000, "notes.md in the folder");
  // Onto a directory's row: into it.
  await c.ev(DRAG(1, ["notes.md"], 0, "GAMES"));
  await p.answer("Store it as a string?");
  await p.until(`tx.panes[0].place.device.list(["HOME", "GAMES"]).then((l) => l.entries.some((e) => e.name === "notes"))`, 10_000, "notes in GAMES");
  // A drop on its own pane does nothing.
  await c.ev(DRAG(1, ["notes.md"], 1));
  await sleep(200);
  assert.equal(await c.ev(`!!document.querySelector("dialog.confirm[open]")`), false);
  assert.deepEqual(c.errors, []);
});

test("purge and delete ask with the count; rename and new directory on both sides; F2, F5 and Delete", { timeout: 120_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`__calc(0)`);
  await c.ev(`__folder(1)`);
  await p.idle(10_000);
  await c.ev(SEED_CALC(0));
  await p.idle(10_000);
  const key = async (k, code, vk) => {
    await c.send("Input.dispatchKeyEvent", { type: "keyDown", key: k, code, windowsVirtualKeyCode: vk });
    await c.send("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: vk });
  };
  const focusRow = (name, i = 0) => c.ev(`(() => { const l = tx.panes[${i}].ui.list; l.select(${JSON.stringify(name)}, { focus: true }); return document.activeElement?.dataset.name === ${JSON.stringify(name)}; })()`);

  // Delete on a directory: the question names what it holds; Cancel keeps it.
  assert.equal(await focusRow("GAMES"), true);
  await c.ev(`__calcs[0].requests.length = 0; true`);
  await key("Delete", "Delete", 46);
  await p.until(`!!document.querySelector("dialog.confirm[open]")`, 5_000, "the purge question");
  assert.match(await c.ev(`document.querySelector("dialog.confirm p").textContent`), /It holds 2 variables, which go too/);
  await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:first-child").click()`);
  await sleep(200);
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "purge").length`), 0, "nothing purged after Cancel");
  // Purge with the count.
  await c.ev(`tx.panes[0].act("remove", [tx.entry("GAMES")]); true`);
  await p.answer("Purge GAMES?");
  await p.until(`!tx.entry("GAMES")`, 10_000, "GAMES purged");
  assert.deepEqual(await c.ev(`__calcs[0].requests.filter((r) => r.type === "purge").map((r) => [r.name, r.count])`), [["GAMES", 2]]);

  // F2 renames on the calculator; a name in use is refused, not replaced.
  assert.equal(await focusRow("PRG"), true);
  await key("F2", "F2", 113);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the rename dialog");
  assert.equal(await c.ev(`document.getElementById("name-title").textContent`), "Rename PRG");
  await c.ev(`(() => { document.getElementById("name-input").value = "X"; document.getElementById("name-ok").click(); return true; })()`);
  await p.until(`[...document.querySelectorAll(".toast")].some((e) => /X is already there/.test(e.textContent))`, 5_000, "the clash said");
  assert.ok(await c.ev(`!!tx.entry("PRG") && !!tx.entry("X")`));
  await c.ev(`tx.panes[0].rename(tx.entry("PRG")); true`);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the rename dialog again");
  await c.ev(`(() => { document.getElementById("name-input").value = "PROG"; document.getElementById("name-ok").click(); return true; })()`);
  await p.until(`!!tx.entry("PROG") && !tx.entry("PRG")`, 5_000, "renamed");

  // New directory on the calculator, new folder in the folder.
  await c.ev(`tx.panes[0].mkdir(); true`);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "new directory");
  await c.ev(`(() => { document.getElementById("name-input").value = "WORK"; document.getElementById("name-ok").click(); return true; })()`);
  await p.until(`tx.entry("WORK")?.directory === true`, 5_000, "WORK made");
  await c.ev(`tx.panes[1].mkdir(); true`);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "new folder");
  await c.ev(`(() => { document.getElementById("name-input").value = "saved"; document.getElementById("name-ok").click(); return true; })()`);
  await p.until(`tx.entry("saved", 1)?.directory === true`, 5_000, "saved made");

  // F5 copies the selection to the other pane: into the folder.
  assert.equal(await focusRow("PROG"), true);
  await key("F5", "F5", 116);
  await p.until(`!!tx.entry("PROG.hp", 1)`, 10_000, "PROG copied by F5");
  // Rename and delete in the folder; a folder that is not empty asks with its count.
  await c.ev(`tx.panes[1].place.folder.write(["hp-files", "saved"], "a.hp", new Uint8Array([1])).then(() => true)`);
  await c.ev(`tx.panes[1].rename(tx.entry("PROG.hp", 1)); true`);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "rename in the folder");
  await c.ev(`(() => { document.getElementById("name-input").value = "prog.hp"; document.getElementById("name-ok").click(); return true; })()`);
  await p.until(`!!tx.entry("prog.hp", 1)`, 5_000, "renamed in the folder");
  await c.ev(`tx.panes[1].act("remove", [tx.entry("saved", 1), tx.entry("prog.hp", 1)]); true`);
  await p.until(`!!document.querySelector("dialog.confirm[open]")`, 5_000, "the delete question");
  assert.equal(await c.ev(`document.querySelector("dialog.confirm h2").textContent`), "Delete 2 items?");
  assert.match(await c.ev(`document.querySelector("dialog.confirm p").textContent`), /They hold 1 item, which go too/);
  await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:last-child").click()`);
  await p.until(`!tx.entry("saved", 1) && !tx.entry("prog.hp", 1)`, 5_000, "deleted");
  assert.deepEqual(c.errors, []);
});

test("between two calculators: the plan's warning before anything is sent; Cancel sends nothing", { timeout: 120_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`__calc(0, "48gx")`);
  await c.ev(`__calc(1, "49g", { label: "second stub" })`);
  await p.idle(10_000);
  await p.idle(10_000, "the 49G listed", 1);
  await c.ev(SEED_CALC(0));
  await p.idle(10_000);
  await c.ev(`__calcs[0].requests.length = 0; __calcs[1].requests.length = 0; window.__copy = null; tx.panes[0].copySelection([tx.entry("PRG"), tx.entry("GAMES")]).then((r) => { __copy = r ?? "none"; }); true`);
  await p.until(`!!document.querySelector("dialog.confirm[open]")`, 10_000, "the plan's warning");
  const q = await c.ev(`({ title: document.querySelector("dialog.confirm h2").textContent, text: document.querySelector("dialog.confirm .confirm-box").textContent })`);
  assert.equal(q.title, "Copy 2 items to HP 49G on second stub?");
  assert.match(q.text, /HP 48 objects\. They go to the HP 49G as text/);
  assert.deepEqual(await c.ev(`[...document.querySelectorAll("dialog.confirm .confirm-list li")].map((e) => e.textContent)`), ["PRG", "GAMES (a directory, with all it holds)"], "the items the warning is about");
  assert.equal((q.text.match(/other calculator family/g) ?? []).length, 0, "the family said once, in the warning");
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type !== "list").length + __calcs[1].requests.filter((r) => r.type !== "list").length`), 0, "nothing sent before the answer");
  await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:first-child").click()`);
  await p.until("__copy !== null", 5_000, "cancelled");
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "get" || r.type === "get-tree").length`), 0);
  // Copy: the text forms go.
  const copy = c.ev(`tx.panes[0].copySelection([tx.entry("PRG"), tx.entry("GAMES")]).then((r) => r)`);
  await p.answer("Copy 2 items to HP 49G on second stub?");
  const r = await copy;
  assert.deepEqual(r.copied.sort(), ["GAMES", "PRG"]);
  assert.deepEqual(await c.ev(`__calcs[0].requests.filter((r) => r.type === "get" || r.type === "get-tree").map((r) => r.format)`), ["text", "text"]);
  await p.until(`!!tx.entry("PRG", 1) && tx.entry("GAMES", 1)?.directory`, 10_000, "on the 49G");
  // A 48G/GX to a 48S/SX: its own warning; a directory there with the name is left out.
  await c.ev(`__calc(1, "48sx", { label: "third stub" })`);
  await p.idle(10_000, "the 48SX listed", 1);
  await c.ev(`tx.panes[1].place.device.mkdir(["HOME"], "GAMES").then(() => true)`);
  await p.until(`tx.entry("GAMES", 1)?.directory === true`, 5_000, "GAMES on the 48SX");
  const sx = c.ev(`tx.panes[0].copySelection([tx.entry("X"), tx.entry("GAMES")]).then((r) => r)`);
  await p.until(`!!document.querySelector("dialog.confirm[open]")`, 5_000, "the GX to SX warning");
  assert.match(await c.ev(`document.querySelector("dialog.confirm .confirm-box").textContent`), /HP 48G\/GX[\s\S]*Left out: a directory has the name on one side[\s\S]*GAMES/);
  await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:last-child").click()`);
  assert.deepEqual((await sx).copied, ["X"]);
  assert.deepEqual(c.errors, []);
});

test("the queue: one job per place at a time, the waiting ones shown and cancellable, progress in bytes", { timeout: 120_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`__calc(0, "48gx", { delay: (r) => (r.type === "put" ? 1500 : 10) })`);
  await c.ev(`__folder(1)`);
  await p.idle(10_000);
  for (const n of ["A", "B", "C"]) await c.ev(`tx.panes[1].place.folder.write(["hp-files"], "${n}.txt", new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n${"1".repeat(400)}\\r\\n")).then(() => true)`);
  await p.until(`${folderNames()}?.length === 3`, 5_000, "three files");
  await c.ev(`window.__q = tx.panes[1].copySelection(tx.panes[1].ui.list.listing.entries); true`);
  await p.until(`document.querySelectorAll("#queue-list .queue-job").length === 3`, 5_000, "three jobs in the queue");
  const q = await c.ev(`[...document.querySelectorAll("#queue-list .queue-job")].map((e) => ({ cls: e.className, text: e.textContent }))`);
  assert.match(q[0].cls, /running/);
  assert.match(q[1].cls, /queued/);
  assert.match(q[1].text, /Waiting/);
  assert.equal(await c.ev(`tx.queue.running.length`), 1, "one job per place at a time");
  assert.equal(await c.ev(`document.getElementById("cancel-waiting").hidden`), false);
  await p.until(`/ of /.test(document.querySelector("#queue-list .queue-job.running .queue-state")?.textContent ?? "")`, 5_000, "the progress in bytes");
  assert.match(await c.ev(`document.querySelector("#queue-list .queue-job.running .queue-state").textContent`), /\d+ bytes of \d+ bytes/);
  assert.equal(await c.ev(`!!document.querySelector("#queue-list .queue-job.running progress")`), true);
  // Cancel the last one before it starts.
  await c.ev(`document.querySelector('#queue-list .queue-job.queued:last-child .queue-cancel').click(); true`);
  const r = await c.ev(`__q`);
  assert.deepEqual(r.copied.sort(), ["A.txt", "B.txt"]);
  assert.deepEqual(r.skipped, ["C.txt"]);
  assert.deepEqual(await c.ev(`__calcs[0].requests.filter((r) => r.type === "put").map((r) => r.name)`), ["A", "B"]);
  await p.until(`document.getElementById("queue").hidden`, 5_000, "the queue hides when empty");
  assert.deepEqual(c.errors, []);
});

test("backup into the folder, restore with its question, the restart and the reconnect", { timeout: 120_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`__calc(0)`);
  await c.ev(`__folder(1)`);
  await p.idle(10_000);
  await c.ev(SEED_CALC(0));
  await p.idle(10_000);
  await c.ev(`tx.panes[0].refresh(); true`);
  await p.idle(10_000, "read again");
  const home = await c.ev(`tx.panes[0].ui.list.listing.entries.map((e) => [e.name, e.size, e.checksum]).sort()`);
  await c.ev(`tx.app.now = () => new Date(2026, 9, 11, 14, 32); tx.panes[0].backup(); true`);
  await p.until(`document.getElementById("backup-dialog").open`, 5_000, "the backup dialog");
  assert.match(await c.ev(`document.getElementById("backup-memory").textContent`), /Free memory.*HOME holds.*A backup needs/);
  assert.equal(await c.ev(`document.getElementById("backup-go").textContent`), "Back up to hp-files");
  await c.ev(`document.getElementById("backup-go").click(); true`);
  await p.until(`!!tx.entry("48GX-backup-2026-10-11-1432.hp", 1)`, 10_000, "the backup file, named with the model and the time");
  // A file that is no backup is refused before any question.
  await c.ev(`tx.panes[0].restore(new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n1\\r\\n"), "NOTES.hp"); true`);
  await p.until(`[...document.querySelectorAll(".toast")].some((e) => /NOTES\\.hp is not a backup: nothing was restored/.test(e.textContent))`, 5_000, "the refusal");
  assert.equal(await c.ev(`!!document.querySelector("dialog.confirm[open]")`), false);
  // A purge, then the restore from the folder's file: the question first.
  await c.ev(`tx.panes[0].place.device.purge(["HOME"], "X").then(() => true)`);
  await p.until(`!tx.entry("X")`, 5_000, "X purged");
  await c.ev(`__calcs[0].requests.length = 0; tx.panes[1].act("restoreFrom", [tx.entry("48GX-backup-2026-10-11-1432.hp", 1)]); true`);
  await p.until(`!!document.querySelector("dialog.confirm[open]")`, 5_000, "the restore question");
  assert.equal(await c.ev(`document.querySelector("dialog.confirm h2").textContent`), "Replace everything on the calculator?");
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "restore").length`), 0, "nothing sent before the answer");
  await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:last-child").click()`);
  await p.until(`!!document.querySelector('.pane[data-pane=left] [data-action="reconnect"]')`, 10_000, "the restart and the reconnect offer");
  assert.equal(await c.ev(`document.querySelector(".pane[data-pane=left] .start h2").textContent`), "The calculator restarted");
  await c.ev(`document.querySelector('.pane[data-pane=left] [data-action="reconnect"]').click(); true`);
  await p.idle(10_000, "reconnected and listed");
  assert.deepEqual(await c.ev(`tx.panes[0].ui.list.listing.entries.map((e) => [e.name, e.size, e.checksum]).sort()`), home, "the same HOME");
  // No room: the dialog says so and offers the folder copy.
  await c.ev(`__calc(0, "48sx", { free: 10 })`);
  await p.idle(10_000);
  await c.ev(SEED_CALC(0));
  await p.idle(10_000);
  await c.ev(`tx.panes[0].backup(); true`);
  await p.until(`document.getElementById("backup-dialog").open`, 5_000, "the backup dialog without room");
  assert.equal(await c.ev(`document.getElementById("backup-go").disabled`), true);
  assert.match(await c.ev(`document.getElementById("backup-text").textContent`), /There is not room/);
  await c.ev(`document.getElementById("backup-copy").click(); true`);
  await p.until(`!!tx.entry("48SX-HOME-2026-10-11-1432", 1)`, 10_000, "everything copied to a folder");
  assert.deepEqual(await c.ev(`tx.panes[1].place.folder.list(["hp-files", "48SX-HOME-2026-10-11-1432"]).then((l) => l.entries.map((e) => e.name).sort())`), ["GAMES", "PRG.hp", "X.hp"]);
  assert.deepEqual(c.errors, []);
});

test("no horizontal overflow at five widths in both themes; dialogs and menus inside the window", { timeout: 400_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  const { send, ev, settled } = c;
  await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  const metrics = (width, height) => send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 2, mobile: width < 1000 });
  await ev(`__calc(0, "48gx", { label: "USB serial port with a long name" })`);
  await ev(`__folder(1, "a-folder-with-a-rather-long-name-indeed")`);
  await p.idle(10_000);
  await ev(`tx.panes[0].place.device.write(["HOME"], "AVERYLONGVARIABLENAMEINDEED", new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n\\\\<< \\"a string long enough to wrap in the preview of a narrow phone, with-a-very-long-unbroken-token-in-it-that-goes-on-and-on\\" \\\\>>")).then(() => tx.panes[0].place.device.mkdir(["HOME"], "GAMES")).then(() => true)`);
  await p.idle(10_000);
  await ev(`tx.panes[1].place.folder.write(["a-folder-with-a-rather-long-name-indeed"], "A-FILE-WITH-A-LONG-NAME-2026-10-11.hp", new Uint8Array(30)).then(() => true)`);
  const reset = () => ev(`(() => {
    for (const d of document.querySelectorAll("dialog[open]")) { d.dispatchEvent(new Event("cancel")); if (d.open) d.close(); }
    document.querySelector(".menu")?.remove();
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    return true;
  })()`);
  const question = (q) => ev(`(() => { import("./shared/components/confirm.js").then((m) => m.confirmAction(${JSON.stringify(q)})); return true; })()`).then(() => p.until(`!!document.querySelector("dialog.confirm[open]")`, 5_000, "the question"));
  const VIEWS = {
    panes: async () => { await ev(`tx.setActive("left"); true`); },
    right: async () => { await ev(`tx.setActive("right"); true`); },
    preview: async () => {
      await ev(`tx.setActive("left"); tx.select("AVERYLONGVARIABLENAMEINDEED"); true`);
      await p.until(`!!document.querySelector(".pane[data-pane=left] .preview pre.obj")`, 10_000, "the preview");
    },
    menu: async () => {
      await ev(`tx.setActive("left"); document.querySelector('.pane[data-pane=left] device-list tr[data-name="GAMES"] button.more').click(); true`);
      await p.until(`!!document.querySelector(".menu")`, 5_000, "the row's menu");
    },
    paneMenu: async () => {
      await ev(`tx.setActive("left"); document.querySelector('.pane[data-pane=left] .pane-more').click(); true`);
      await p.until(`!!document.querySelector(".menu")`, 5_000, "the pane's menu");
    },
    switcher: async () => {
      await ev(`tx.setActive("right"); document.querySelector('.pane[data-pane=right] .pane-change').click(); true`);
      await p.until(`!!document.querySelector(".menu")`, 5_000, "the place switcher");
    },
    plan: () => question({ title: "Copy 2 items to HP 49G on USB serial port with a long name?", body: "Before anything is copied:", action: "Copy", danger: false, sections: [{ text: "These are HP 48 objects. They go to the HP 49G as text, which it compiles; a program may not run the same there.", items: ["PROGRAMWITHALONGNAME", "GAMES (a directory, with all it holds)"] }, { text: "Left out: these have no text form the other calculator can read.", items: ["AVERYLONGVARIABLENAMEINDEED", "LIBRARY1234"] }] }),
    restore: () => question({ title: "Replace everything on the calculator?", body: "HP 48G/GX on USB serial port with a long name gets the contents of 48GX-backup-2026-10-11-1432.hp: what HOME holds now is lost. The calculator restarts and leaves server mode.", action: "Restore" }),
    queue: async () => {
      await ev(`(() => { if (!window.__queued) { window.__queued = true; for (const n of ["ONE", "TWO", "THREE-WITH-A-LONG-NAME"]) tx.queue.add({ label: "Copy " + n + " to a-folder-with-a-rather-long-name-indeed", places: ["x"], total: 3400, run: ({ report }) => { report(1200, 3400); return new Promise(() => {}); } }).catch(() => {}); } return true; })()`);
      await p.until(`document.querySelectorAll("#queue-list .queue-job").length >= 3`, 5_000, "the queue");
    },
    backup: async () => {
      await ev(`tx.app.backupDialog({ memory: { free: 1000, home: 30000, needed: 61024, fits: false }, model: "48sx", folder: "a-folder-with-a-rather-long-name-indeed" }); true`);
      await p.until(`document.getElementById("backup-dialog").open`, 5_000, "the backup dialog");
    },
    name: async () => {
      await ev(`tx.app.askName({ title: "Rename AVERYLONGVARIABLENAMEINDEED", why: "The new name on the calculator:", suggested: "AVERYLONGVARIABLENAMEINDEED", action: "Rename" }); true`);
      await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name dialog");
    },
    connect: () => ev(`tx.app.openConnect(tx.panes[0]); true`).then(() => p.until(`document.getElementById("connect-dialog").open`, 5_000, "the connect dialog")),
    settings: () => ev(`(() => { document.getElementById("open-settings").click(); return true; })()`).then(() => p.until(`document.getElementById("settings-dialog").open`, 5_000, "the settings")),
  };
  const failures = [];
  for (const w of WIDTHS) {
    for (const scheme of ["light", "dark"]) {
      await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
      for (const [view, show] of Object.entries(VIEWS)) {
        await metrics(w, HEIGHTS[w]);
        await reset();
        await show();
        await sleep(150);
        const m = await settled(MEASURE);
        if (m.page > 0) failures.push(`${view} at ${w}px ${scheme}: the page is ${m.page}px too wide (innerWidth ${m.inner})`);
        for (const s of m.scrollers) failures.push(`${view} at ${w}px ${scheme}: ${s} scrolls sideways`);
        for (const s of m.outside) failures.push(`${view} at ${w}px ${scheme}: ${s} is outside the window`);
      }
    }
  }
  assert.deepEqual(failures, []);
  // Stacked below 760 px: one pane at a time, the switch shown.
  await metrics(390, 844);
  await reset();
  await ev(`tx.setActive("left"); true`);
  const narrow = await settled(`({ sw: getComputedStyle(document.getElementById("pane-switch")).display, left: document.querySelector(".pane[data-pane=left]").checkVisibility(), right: document.querySelector(".pane[data-pane=right]").checkVisibility() })`);
  assert.deepEqual(narrow, { sw: "flex", left: true, right: false });
  await metrics(1280, 900);
  const wide = await settled(`({ sw: getComputedStyle(document.getElementById("pane-switch")).display, left: document.querySelector(".pane[data-pane=left]").checkVisibility(), right: document.querySelector(".pane[data-pane=right]").checkVisibility() })`);
  assert.deepEqual(wide, { sw: "none", left: true, right: true });
  // The themes are the tokens': dark when asked, light otherwise.
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "dark" }] });
  assert.equal(await ev(`getComputedStyle(document.body).backgroundColor`), "rgb(28, 28, 27)");
  await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
  assert.equal(await ev(`getComputedStyle(document.body).backgroundColor`), "rgb(218, 215, 206)");
  assert.deepEqual(c.errors, []);
});

test("a browser without Web Serial or a folder picker says so plainly and still offers the emulated calculator", { timeout: 60_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Page.addScriptToEvaluateOnNewDocument", { source: "Object.defineProperty(Navigator.prototype, 'serial', { get: () => undefined, configurable: true }); delete Window.prototype.showDirectoryPicker; delete window.showDirectoryPicker;" });
  const p = txPage(c);
  await p.open(c.port);
  const s = await c.ev(`(() => {
    const left = document.querySelector(".pane[data-pane=left]");
    return {
      serial: !!navigator.serial,
      note: left.querySelector(".start-note")?.textContent ?? null,
      add: !!left.querySelector('[data-action="add-serial"]'),
      emulated: !!left.querySelector('[data-action="add-emulated"]'),
      primary: left.querySelector('[data-action="add-emulated"]')?.classList.contains("primary"),
      right: document.querySelector(".pane[data-pane=right] .pane-title").textContent,
    };
  })()`);
  assert.equal(s.serial, false);
  assert.match(s.note, /cannot reach a serial port.*Chrome or Edge.*emulated calculator works here/);
  assert.equal(s.add, false);
  assert.equal(s.emulated, true);
  assert.equal(s.primary, true);
  // No folder picker: the right pane holds the page's own files.
  assert.equal(s.right, "This page's files");
  // The emulated calculator's dialog opens (no ROM kept here: it says so).
  await c.ev(`document.querySelector('.pane[data-pane=left] [data-action="add-emulated"]').click(); true`);
  await p.until(`document.getElementById("emulated-dialog").open`, 5_000, "the emulated dialog");
  assert.equal(await c.ev(`document.getElementById("emulated-none").hidden`), false);
  assert.deepEqual(c.errors, []);
});

// ---------------------------------------------------------------- folders only in the right pane

/** The texts of pane `side`'s "Change" menu, separators as "-", the menu closed again. */
const changeMenu = (side) => `(async () => {
  document.querySelector(".menu")?.remove();
  document.querySelector('.pane[data-pane=${side}] .pane-change').click();
  for (let i = 0; i < 50 && !document.querySelector(".menu"); i++) await new Promise((r) => setTimeout(r, 20));
  const items = [...document.querySelector(".menu").querySelectorAll("[role=menuitem], [role=separator]")].map((e) => (e.getAttribute("role") === "separator" ? "-" : e.textContent.trim()));
  document.querySelector(".menu").remove();
  return items;
})()`;

test("folders only in the right pane: the left's Change menu and card offer calculators only; a kept folder never comes back on the left", { timeout: 60_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  const p = txPage(c);
  await p.open(c.port);
  const left = await c.ev(changeMenu("left"));
  assert.deepEqual(left, ["A calculator on a cable…", "An emulated calculator…"]);
  assert.ok(!left.some((s) => /folder|files/i.test(s)), `no folder entry on the left: ${left}`);
  const right = await c.ev(changeMenu("right"));
  assert.deepEqual(right, ["A folder on this computer…", "This page's files", "-", "A calculator on a cable…", "An emulated calculator…"]);
  // The start cards: the left's ways in are calculators, the right's a folder first, then a calculator.
  const cards = await c.ev(`(() => {
    const ways = (side) => [...document.querySelectorAll('.pane[data-pane=' + side + '] .pane-empty [data-action]')].map((b) => b.dataset.action);
    return { left: ways("left"), right: ways("right"), heading: document.querySelector(".pane[data-pane=left] .pane-empty h2").textContent };
  })()`);
  assert.deepEqual(cards.left, ["add-serial", "add-emulated"]);
  assert.deepEqual(cards.right, ["open-folder", "use-files", "add-serial", "add-emulated"]);
  assert.equal(cards.heading, "A calculator");
  // No path puts a folder on the left.
  assert.deepEqual(await c.ev(`Promise.all([tx.memoryFolder(0), tx.panes[0].useFiles(), tx.panes[0].openFolder(), tx.panes[0].offerKept()]).then((r) => [...r, tx.panes[0].place])`), [null, null, null, false, null]);
  // A folder kept from an older visit comes back on the right only.
  await c.ev(`import("./tx/folder.js").then(async (m) => {
    const root = await navigator.storage.getDirectory();
    await m.handleStore().set(await root.getDirectoryHandle("kept-on-the-left", { create: true }));
    return true;
  })`);
  await p.open(c.port);
  await p.until(`tx.panes[1].place?.kind === "folder" || !!tx.panes[1].pending`, 10_000, "the kept folder on the right");
  const after = await c.ev(`({ left: tx.panes[0].place, pending: tx.panes[0].pending, title: document.querySelector(".pane[data-pane=left] .pane-title").textContent })`);
  assert.deepEqual(after, { left: null, pending: null, title: "No calculator" });
  assert.deepEqual(c.errors, []);
});

// ---------------------------------------------------------------- the review's cases (PR 130)

test("a newer connect in a pane wins over a stale one; the pane's ways in are off meanwhile", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  // The first connect is slow: the fake calculator answers it after 1.5 s.
  await c.ev(`(() => {
    const slow = __fake.fakeCalc({ model: "48gx", delay: (r) => (r.type === "connect" ? 1500 : 10) });
    tx.app.deviceOptions = { worker: slow.worker };
    window.__first = __fake.stubLink("first");
    window.__a = tx.panes[0].connectLink(__first, { model: "48gx" });
    return true;
  })()`);
  await p.until(`document.querySelector(".pane[data-pane=left] .pane-title").textContent !== "No calculator"`, 5_000, "connecting");
  assert.equal(await c.ev(`!!document.querySelector('.pane[data-pane=left] [data-action="add-emulated"]')`), false, "no second way in while it connects");
  await c.ev(`(() => {
    const fast = __fake.fakeCalc({ model: "49g" });
    tx.app.deviceOptions = { worker: fast.worker };
    window.__second = __fake.stubLink("second");
    window.__b = tx.panes[0].connectLink(__second, { model: "49g" });
    return true;
  })()`);
  await p.idle(10_000, "the second connected and listed");
  await c.ev(`__a.then((d) => { window.__aResult = d; }); true`);
  await p.until("window.__aResult !== undefined", 5_000, "the first connect settled");
  const s = await c.ev(`({ a: __aResult, firstClosed: __first.closed, secondClosed: __second.closed, title: document.querySelector(".pane[data-pane=left] .pane-title").textContent })`);
  assert.equal(s.a, null, "the stale connect gives nothing");
  assert.equal(s.firstClosed, true, "its link was let go");
  assert.equal(s.secondClosed, false);
  assert.equal(s.title, "HP 49G on second");
  await c.ev("tx.panes[0].close()");
  await p.until("!tx.panes[0].place", 5_000, "disconnected");
  assert.deepEqual(c.errors, []);
});

test("the name dialog: Enter puts, a refused name asks again, Escape cancels", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t, 390);
  if (!f) return;
  const { c, p } = f;
  const key = async (k, code) => {
    await c.send("Input.dispatchKeyEvent", { type: "keyDown", key: k, code, windowsVirtualKeyCode: k === "Enter" ? 13 : 27, ...(k === "Enter" ? { text: "\r" } : {}) });
    await c.send("Input.dispatchKeyEvent", { type: "keyUp", key: k, code, windowsVirtualKeyCode: k === "Enter" ? 13 : 27 });
  };
  const type = (text) => c.ev(`(() => { const i = document.getElementById("name-input"); i.focus(); i.value = ${JSON.stringify(text)}; return true; })()`);
  await c.ev(`__calc(0)`);
  await p.idle(10_000);
  const putBad = `window.__put = false; tx.panes[0].drop([tx.fileItem(new File(["%%HP: T(3)A(R)F(.);\\r\\n1\\r\\n"], "1BAD.txt"))]).then(() => { __put = true; }); true`;
  await c.ev(putBad);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name dialog");
  await type("2X");
  await key("Enter", "Enter");
  await p.until(`document.getElementById("name-problem").textContent !== ""`, 5_000, "the refusal");
  assert.equal(await c.ev(`document.getElementById("name-dialog").open`), true, "a refused name asks again");
  await key("Escape", "Escape");
  await p.until("__put", 5_000, "Escape ends the put");
  assert.equal(await c.ev(`document.getElementById("name-dialog").open`), false);
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "put").length`), 0, "nothing put after Escape");
  await c.ev(putBad);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name dialog again");
  await c.ev(`document.getElementById("name-cancel").click(); true`);
  await p.until("__put", 5_000, "Cancel ends the put");
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "put").length`), 0, "nothing put after Cancel");
  await c.ev(putBad);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name dialog a third time");
  await type("GOOD");
  await key("Enter", "Enter");
  await p.until("__put", 10_000, "the put");
  assert.deepEqual(await c.ev(`__calcs[0].requests.filter((r) => r.type === "put").map((r) => r.name)`), ["GOOD"]);
  // Its toast at the window's bottom, clear of the pane's head.
  await p.until(`[...document.querySelectorAll(".toast")].some((e) => /copied/.test(e.textContent))`, 5_000, "the toast");
  const r = await c.ev(`(() => { const t = [...document.querySelectorAll(".toast")].at(-1).getBoundingClientRect(); const h = document.querySelector(".pane[data-pane=left] .pane-head").getBoundingClientRect(); return { top: t.top, bottom: t.bottom, head: h.bottom, inner: innerHeight }; })()`);
  assert.ok(r.top > r.head, `below the pane's head (${r.top} > ${r.head})`);
  assert.ok(r.bottom <= r.inner && r.bottom > r.inner - 80, `at the bottom of the window (${r.bottom} of ${r.inner})`);
  assert.deepEqual(c.errors, []);
});

test("previews: one waits at a time, a selection that moved on drops it, and a dropped one is not kept as an error", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`__calc(0, "48gx", { delay: (r) => (r.type === "get" ? 300 : 10) })`);
  await p.idle(10_000);
  await c.ev(`(async () => { for (const n of ["A", "B", "C", "D"]) await tx.panes[0].place.device.write(["HOME"], n, new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n\\\\<< " + n + " \\\\>>\\r\\n")); return true; })()`);
  await p.idle(10_000);
  await c.ev(`__calcs[0].requests.length = 0; for (const n of ["A", "B", "C", "D"]) tx.select(n); true`);
  await p.until(`/D/.test(document.querySelector(".pane[data-pane=left] .preview pre")?.textContent ?? "")`, 10_000, "D previewed");
  await p.idle(10_000);
  const got = await c.ev(`__calcs[0].requests.filter((r) => r.type === "get").map((r) => r.name)`);
  assert.deepEqual(got, ["A", "D"], "A was already running; B and C were dropped");
  assert.equal(await c.ev(`[...tx.panes[0].previews.values()].some((v) => v.status === "error")`), false, "no error kept for B or C");
  await c.ev(`tx.select("B")`);
  await p.until(`/B/.test(document.querySelector(".pane[data-pane=left] .preview pre")?.textContent ?? "")`, 10_000, "B previewed when selected again");
  assert.deepEqual(c.errors, []);
});

test("an unplug says the calculator is gone, once; a Worker that fails drops the device with a plain reason", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`(() => {
    const calc = __fake.fakeCalc({ model: "48gx" });
    window.__calcs[0] = calc;
    tx.app.deviceOptions = { worker: calc.worker };
    window.__link = __fake.stubLink();
    tx.panes[0].connectLink(__link, { model: "48gx" });
    return true;
  })()`);
  await p.idle(10_000);
  await c.ev(`__link.dispatchEvent(new CustomEvent("lost", { detail: "The cable or adapter was unplugged." })); true`);
  await p.until(`!tx.panes[0].place && !!document.querySelector(".toast")?.textContent`, 5_000, "dropped");
  await sleep(300);
  assert.deepEqual(await c.ev(`[...document.querySelectorAll(".toast")].map((e) => e.textContent)`), ["The calculator is gone: The cable or adapter was unplugged."]);
  assert.equal(await c.ev(title("left")), "No calculator");
  await c.ev(`__calc(0)`);
  await p.idle(10_000);
  await c.ev(`tx.panes[0].place.device.client.worker.onmessage({ data: { type: "failed", message: "unreachable executed" } }); true`);
  await p.until(`!tx.panes[0].place && [...document.querySelectorAll(".toast")].some((e) => /unreachable executed/.test(e.textContent))`, 5_000, "dropped");
  assert.match(await c.ev(`[...document.querySelectorAll(".toast")].map((e) => e.textContent).join("|")`), /The calculator is gone: the transfer part of the page stopped \(unreachable executed\)/);
  assert.deepEqual(c.errors, []);
});

// ---------------------------------------------------------------- the review's cases (PR 135)

test("review: jobs keep the directories they were asked for; drops never open a file in the tab; F5 never reloads; questions wait in turn", { timeout: 120_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`__calc(0)`);
  await c.ev(`__folder(1)`);
  await p.idle(10_000);
  await c.ev(SEED_CALC(0));
  await p.idle(10_000);
  await c.ev(`tx.panes[1].place.folder.mkdir(["hp-files"], "sub").then(() => true)`);
  await p.until(`!!tx.entry("sub", 1)`, 5_000, "sub listed");
  // A job that holds the calculator until released.
  const hold = `window.__release = null; tx.queue.add({ label: "hold", places: [tx.panes[0].place.id], run: () => new Promise((r) => { __release = r; }) }); true`;

  // A download asked in HOME and run after the pane moved into GAMES: still HOME's X.
  await c.ev(hold);
  await c.ev(`window.__dl = tx.panes[0].download([tx.entry("X")]); tx.panes[0].navigate(["HOME", "GAMES"]); __release(); true`);
  await c.ev(`__dl.then(() => true)`);
  assert.equal(await c.ev(`tx.app.lastDownload.name`), "X.hp");
  await c.ev(`tx.panes[0].navigate(["HOME"]); true`);
  await p.idle(10_000);

  // Copy everything, asked with the folder at its top, run after it moved into sub: lands at the top.
  await c.ev(`tx.app.now = () => new Date(2026, 9, 11, 9, 5); ${hold}`);
  await c.ev(`window.__all = tx.panes[0].copyAll(); tx.panes[1].navigate(["hp-files", "sub"]); __release(); true`);
  assert.equal(await c.ev(`__all`), "48GX-HOME-2026-10-11-0905");
  assert.deepEqual(await c.ev(`tx.panes[1].place.folder.list(["hp-files"]).then((l) => l.entries.map((e) => e.name).sort())`), ["48GX-HOME-2026-10-11-0905", "sub"]);
  assert.deepEqual(await c.ev(`tx.panes[1].place.folder.list(["hp-files", "sub"]).then((l) => l.entries.length)`), 0);
  await c.ev(`tx.panes[1].navigate(["hp-files"]); true`);

  // Files dropped on a pane that shows nothing usable: taken (no navigation), refused, nothing done.
  await c.ev(`tx.panes[1].close().then(() => true)`);
  const refused = await c.ev(`(() => {
    const dt = new DataTransfer();
    dt.items.add(new File(["x"], "a.txt"));
    const target = document.querySelector(".pane[data-pane=right] .pane-empty");
    const over = new DragEvent("dragover", { dataTransfer: dt, bubbles: true, cancelable: true });
    target.dispatchEvent(over);
    const drop = new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true });
    target.dispatchEvent(drop);
    const outside = new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true });
    document.querySelector(".tx-bar").dispatchEvent(outside);
    return { over: over.defaultPrevented, effect: dt.dropEffect, drop: drop.defaultPrevented, outside: outside.defaultPrevented, place: tx.panes[1].place };
  })()`);
  assert.deepEqual(refused, { over: true, effect: "none", drop: true, outside: true, place: null });
  await c.ev(`__folder(1)`);
  await p.idle(10_000, "the folder again", 1);

  // Rows dragged from another tab of the page (another token) are not copied from this tab's pane.
  await c.ev(`__calcs[0].requests.length = 0; (() => {
    const dt = new DataTransfer();
    dt.setData("application/x-saturnus-tx", JSON.stringify({ source: { page: "another-tab", pane: "left" }, path: ["HOME"], names: ["X"] }));
    tx.panes[1].ui.list.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
    return true;
  })()`);
  await p.until(`[...document.querySelectorAll(".toast")].some((e) => /another tab/.test(e.textContent))`, 5_000, "the foreign drop said");
  assert.equal(await c.ev(`__calcs[0].requests.filter((r) => r.type === "get").length`), 0);
  assert.equal(await c.ev(`!!tx.entry("X.hp", 1)`), false);

  // F5 outside the list (here on "Change"): no reload, a word instead.
  await c.ev(`document.querySelector(".pane[data-pane=left] .pane-change").focus(); window.__alive = 1; true`);
  await c.send("Input.dispatchKeyEvent", { type: "keyDown", key: "F5", code: "F5", windowsVirtualKeyCode: 116 });
  await c.send("Input.dispatchKeyEvent", { type: "keyUp", key: "F5", code: "F5", windowsVirtualKeyCode: 116 });
  await p.until(`[...document.querySelectorAll(".toast")].some((e) => /Select what to copy/.test(e.textContent))`, 5_000, "the F5 word");
  assert.equal(await c.ev(`window.__alive`), 1, "the page was not reloaded");

  // Two questions at once (two jobs on two places): the second waits; the first is not cancelled.
  await c.ev(`window.__a1 = null; window.__a2 = null; tx.app.ask({ title: "First?", body: "one", action: "Yes" }).then((a) => { __a1 = a; }); tx.app.ask({ title: "Second?", body: "two", action: "Yes" }).then((a) => { __a2 = a; }); true`);
  await p.until(`document.querySelectorAll("dialog.confirm[open]").length === 1`, 5_000, "one question open");
  assert.equal(await c.ev(`document.querySelector("dialog.confirm h2").textContent`), "First?");
  await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:last-child").click(); true`);
  await p.until(`__a1 === true && document.querySelector("dialog.confirm h2")?.textContent === "Second?"`, 5_000, "the second after the first");
  await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:first-child").click(); true`);
  await p.until(`__a2 === false`, 5_000, "the second answered");

  // A restore from a folder file that cannot be read: said, nothing asked.
  await c.ev(`tx.panes[1].place.folder.write(["hp-files"], "gone.hp", new Uint8Array([1])).then(() => true)`);
  await p.until(`!!tx.entry("gone.hp", 1)`, 5_000, "gone.hp listed");
  await c.ev(`(async () => { const e = tx.entry("gone.hp", 1); await tx.panes[1].place.folder.root.removeEntry("gone.hp"); await tx.panes[1].act("restoreFrom", [e]); return true; })().catch(() => true)`);
  await p.until(`[...document.querySelectorAll(".toast")].some((e) => /gone\\.hp could not be read/.test(e.textContent))`, 5_000, "the unreadable file said");
  assert.equal(await c.ev(`!!document.querySelector("dialog.confirm[open]")`), false);

  // Drag URLs: at most 20 kept, one replaced let go.
  const urls = await c.ev(`(() => {
    const pane = tx.panes[0];
    const revoked = [];
    const was = URL.revokeObjectURL;
    URL.revokeObjectURL = (u) => { revoked.push(u); was.call(URL, u); };
    for (let i = 0; i < 25; i++) pane.keepDragUrl("k" + i, new Uint8Array([i]));
    const first = pane.dragUrls.get("k24");
    pane.keepDragUrl("k24", new Uint8Array([9]));
    URL.revokeObjectURL = was;
    return { size: pane.dragUrls.size, revoked: revoked.length, replaced: revoked.includes(first) };
  })()`);
  assert.deepEqual(urls, { size: 20, revoked: 6, replaced: true });
  assert.deepEqual(c.errors, []);
});

test("review: the queue's progress updates its rows in place; a focused Cancel keeps the focus", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  await c.ev(`(() => {
    window.__report = null;
    tx.queue.add({ label: "Copy A to hp-files", places: ["x"], total: 1000, run: ({ report }) => { __report = report; return new Promise(() => {}); } }).catch(() => {});
    tx.queue.add({ label: "Copy B to hp-files", places: ["x"], run: () => new Promise(() => {}) }).catch(() => {});
    tx.queue.add({ label: "Copy C to hp-files", places: ["x"], run: () => new Promise(() => {}) }).catch(() => {});
    return true;
  })()`);
  await p.until(`document.querySelectorAll("#queue-list .queue-cancel").length === 2 && !!__report`, 5_000, "the queue");
  await c.ev(`window.__btn = document.querySelector('#queue-list .queue-job.queued .queue-cancel'); __btn.focus(); true`);
  for (const n of [100, 400, 700]) await c.ev(`__report(${n}, 1000); true`);
  const s = await c.ev(`({ same: document.activeElement === __btn, connected: __btn.isConnected, text: document.querySelector("#queue-list .queue-job.running .queue-state").textContent, bar: document.querySelector("#queue-list .queue-job.running progress").value })`);
  assert.deepEqual(s, { same: true, connected: true, text: "700 bytes of 1.0 KB", bar: 700 });
  // Cancel B by the keyboard: the focus goes to the next Cancel (C's).
  await c.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, text: "\r" });
  await c.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
  await p.until(`document.querySelectorAll("#queue-list .queue-job").length === 2`, 5_000, "B cancelled");
  assert.equal(await c.ev(`document.activeElement?.getAttribute("aria-label")`), "Cancel Copy C to hp-files");
  assert.deepEqual(c.errors, []);
});
