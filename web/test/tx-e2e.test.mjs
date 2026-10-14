// The transfer page against the emulated calculators (`just tx-e2e DIR`):
// headless Chrome, the page's own "Add an emulated calculator" path
// (saturnus's core in the transfer page's Worker, booted from the ROM in
// SATURNUS_ROM_DIR, SERVER typed, its serial port the link) on the 48SX
// (sxrom-j), 48GX (gxrom-r) and 49G (rom.49g), with a folder in memory in
// the other pane. Per model: connect and detect, files dropped (a text
// file, a plain file as a string, a binary file) and the round trip byte
// for byte, "Replace X?", the previews; a new directory, a copy into it
// from the folder, rename, the graphics screen; everything copied to a
// folder, a backup, the directory purged with its count; the folder copy
// of HOME put back and copied again, byte for byte the same; the backup
// restored onto a freshly booted calculator, the restart and the
// reconnect, the same HOME (names, sizes, checksums). Then an emulated
// 48GX to an emulated 49G, pane to pane, with the plan's warning.
// Skipped without the ROMs (SATURNUS_AUDIT=1, as `just tx-e2e` sets it,
// makes the skip a failure), Chrome or the wasm packages. The ROMs are
// only read.

import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, test as nodeTest } from "node:test";
import { session } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

const MODELS = [
  ["48sx", "sxrom-j", "HPHP48-"],
  ["48gx", "gxrom-r", "HPHP48-"],
  ["49g", "rom.49g", "HPHP49-"],
];
const ROM_FILES = Object.fromEntries(MODELS.map(([m, f]) => [m, f]));
const dir = process.env.SATURNUS_ROM_DIR;
const PROGRAM = "%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \\>>\r\n";
const PROGRAM2 = "%%HP: T(3)A(R)F(.);\r\n\\<< 6 7 * \\>>\r\n";
const NOTE = "hello, calculator";
const GROB = "%%HP: T(3)A(R)F(.);\r\nGROB 8 4 F09090F0\r\n";
/**
 * The tests, run side by side at the end of the file (each its own page
 * and calculators; the emulation is paced, so they mostly wait): about a
 * third of the time of one after the other (kb/docs/test-policy.md).
 */
const tests = [];
const test = (...args) => tests.push(args);
/** How long one step may take on the emulated calculator at 1x. */
const STEP_MS = 120_000;

/** Why the test cannot run (no ROM, no packages), or null. */
function missing(files) {
  for (const f of files) if (!dir || !existsSync(join(dir, f))) return `no ${f} in SATURNUS_ROM_DIR`;
  return txBuilt() ? null : "web/pkg or web/tx/pkg not built (just web tx-web)";
}

async function start(t, files) {
  const why = missing(files);
  if (why) {
    if (process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return null;
  }
  const c = await session(t, { files: Object.fromEntries(files.map((f) => [`/${f}`, join(dir, f)])), audit: true });
  if (!c) return null;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  const p = txPage(c);
  await p.open(c.port);
  return { c, p };
}

/** Boot the emulated `model` in pane `i` from its ROM file; resolves once connected and listed. */
async function boot({ c, p }, i, model) {
  const file = ROM_FILES[model];
  await c.ev(`fetch("/${file}").then((r) => r.arrayBuffer()).then((b) => { tx.panes[${i}].addEmulated(new Uint8Array(b), ${JSON.stringify(file)}, ${JSON.stringify(model)}); return true; })`);
  await p.until(`!!tx.panes[${i}].place?.device?.connection`, 240_000, `${model}: connected in pane ${i}`);
  await p.idle(STEP_MS, `${model}: HOME listed`, i);
}

/** Fill the name dialog with `value` and press its button. */
async function name({ c, p }, value) {
  await p.until(`document.getElementById("name-dialog").open`, 10_000, "the name dialog");
  await c.ev(`(() => { document.getElementById("name-input").value = ${JSON.stringify(value)}; document.getElementById("name-ok").click(); return true; })()`);
}

/** Every file below `path` of pane `i`'s folder: `{"D/A.hp": [bytes]}`. */
const folderTree = (path, i = 1) => `(async () => {
  const f = tx.panes[${i}].place.folder;
  const out = {};
  const walk = async (p, prefix) => {
    for (const e of (await f.list(p)).entries) {
      if (e.directory) await walk([...p, e.name], prefix + e.name + "/");
      else out[prefix + e.name] = [...(await f.read(p, e.name)).data];
    }
  };
  await walk(${JSON.stringify(path)}, "");
  return out;
})()`;

/** HOME of pane `i`'s calculator read again: `[[name, size, checksum]]`, sorted. */
const home = (i = 0) => `(tx.panes[${i}].place.device.refresh(null), tx.panes[${i}].place.device.list(["HOME"]).then((l) => l.entries.map((e) => [e.name, e.size, e.checksum]).sort()))`;

/** Answer every question a running copy (`window.__copy`, its end in `__copyDone`) asks with its action. */
async function answerAll({ c, p }, ms) {
  for (;;) {
    await p.until(`!!document.querySelector("dialog.confirm[open]") || window.__copyDone`, ms, "a question or the end of the copy");
    if (!(await c.ev(`!!document.querySelector("dialog.confirm[open]")`))) return c.ev("__copy");
    await c.ev(`document.querySelector("dialog.confirm .confirm-actions button:last-child").click(); true`);
  }
}

for (const [model, file, header] of MODELS) {
  test(`the emulated ${model}: files, directories, rename, purge, the graphics screen, the folder copy of HOME, backup and restore`, { timeout: 1_800_000 }, async (t) => {
    const s = await start(t, [file]);
    if (!s) return;
    const { c, p } = s;
    const { ev } = c;
    const entry = (n) => ev(`tx.entry(${JSON.stringify(n)})`);
    const download = async (n, text = false) => {
      await ev(text ? `tx.panes[0].downloadText(tx.entry(${JSON.stringify(n)}))` : `tx.panes[0].download([tx.entry(${JSON.stringify(n)})])`);
      return ev(`({ name: tx.app.lastDownload.name, bytes: [...tx.app.lastDownload.bytes] })`);
    };
    await ev(`tx.memoryFolder(1, "hp-files").then(() => true)`);
    await boot(s, 0, model);
    const conn = await ev("tx.panes[0].place.device.connection");
    assert.equal(conn.model, model, "the model detected");
    assert.deepEqual(conn.settings, { baud: 9600, parity: "none", dataBits: 8 });
    assert.equal(await ev(`document.querySelector(".pane[data-pane=left] .pane-title").textContent`), `Emulated HP ${model.toUpperCase()}`);

    // A text file: the calculator compiles it.
    await p.drop({ "PRG.txt": PROGRAM });
    await p.idle(STEP_MS, "PRG put");
    assert.equal((await entry("PRG"))?.kind, "Program");
    const prg = await download("PRG");
    assert.equal(prg.name, "PRG.hp");
    assert.equal(String.fromCharCode(...prg.bytes.slice(0, 7)), header);
    // The binary file back under another name: the same object, byte for byte.
    await p.drop({ "PRG2.hp": prg.bytes });
    await p.idle(STEP_MS, "PRG2 put");
    assert.deepEqual((await download("PRG2")).bytes, prg.bytes, "a binary round trip is exact");
    assert.equal((await entry("PRG2")).checksum, (await entry("PRG")).checksum);
    const text = await download("PRG2", true);
    assert.equal(text.name, "PRG2.txt");
    assert.match(String.fromCharCode(...text.bytes), /^%%HP:[^\r\n]*\r?\n[\s\S]*1 2 \+/);

    // A file that is no calculator object: a string, after asking.
    const note = p.drop({ "NOTE.md": NOTE });
    await p.answer("Store it as a string?");
    await note;
    await p.idle(STEP_MS, "NOTE put");
    assert.equal((await entry("NOTE"))?.kind, "String");
    const noteFile = await download("NOTE");
    assert.equal(await ev(`tx.helpers().then((h) => h.describe_file(new Uint8Array(${JSON.stringify(noteFile.bytes)})).value)`), NOTE);

    // A name in use: "Replace PRG?", and PRG is the new program.
    const before = (await entry("PRG")).checksum;
    const again = p.drop({ "PRG.txt": PROGRAM2 });
    await p.answer("Replace PRG?");
    await again;
    await p.idle(STEP_MS, "PRG replaced");
    assert.notEqual((await entry("PRG")).checksum, before);
    assert.ok(!(await p.names()).some((n) => /^SATXPUT/.test(n)), "no temporary variable left");

    // The previews: a program as the calculator's text, a string, a GROB.
    await p.drop({ "PIC.txt": GROB });
    await p.idle(STEP_MS, "PIC put");
    assert.equal((await entry("PIC"))?.kind, "Graphic");
    await ev(`tx.select("PRG")`);
    await p.until(`/6 7 \\*/.test(document.querySelector(".pane[data-pane=left] .preview pre.obj-program")?.textContent ?? "")`, STEP_MS, "PRG previewed");
    await p.idle();
    await ev(`tx.select("NOTE")`);
    await p.until(`document.querySelector(".pane[data-pane=left] .preview pre.obj-string")?.textContent === ${JSON.stringify(NOTE)}`, STEP_MS, "NOTE previewed");
    await p.idle();
    await ev(`tx.select("PIC")`);
    await p.until(`document.querySelector(".pane[data-pane=left] .preview canvas.graphic")?.getAttribute("width") === "8"`, STEP_MS, "PIC previewed");
    await p.idle();

    // A new directory; PRG and NOTE to the folder, and from there into the directory (dropped on its row).
    await ev(`tx.panes[0].mkdir(); true`);
    await name(s, "D1");
    await p.until(`tx.entry("D1")?.directory === true`, STEP_MS, "D1 made");
    await ev(`tx.panes[0].copySelection([tx.entry("PRG"), tx.entry("NOTE")]).then(() => true)`);
    await p.until(`!!tx.entry("PRG.hp", 1) && !!tx.entry("NOTE.hp", 1)`, STEP_MS, "PRG and NOTE in the folder");
    await ev(`(() => {
      const src = tx.panes[1].ui.list;
      const dt = new DataTransfer();
      src.select("NOTE.hp");
      src.select("PRG.hp", { add: true });
      src.querySelector('tr[data-name="PRG.hp"]').dispatchEvent(new DragEvent("dragstart", { dataTransfer: dt, bubbles: true }));
      tx.panes[0].ui.list.querySelector('tr[data-name="D1"]').dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
      return true;
    })()`);
    await p.until(`tx.panes[0].place.device.list(["HOME", "D1"]).then((l) => l.entries.length === 2)`, STEP_MS, "PRG and NOTE in D1");
    await p.idle(STEP_MS, "D1 filled");
    // Rename D1 to D2.
    await ev(`tx.panes[0].rename(tx.entry("D1")); true`);
    await name(s, "D2");
    await p.until(`!!tx.entry("D2") && !tx.entry("D1")`, STEP_MS, "D1 renamed");
    // The graphics screen, in the preview.
    await ev(`tx.panes[0].pict().then(() => true)`);
    await p.until(`document.querySelector(".pane[data-pane=left] .preview h2")?.textContent === "Graphics screen"`, STEP_MS, "the graphics screen shown");
    assert.match(await ev(`document.querySelector(".pane[data-pane=left] .preview .preview-meta").textContent`), /^Graphic, \d+ × \d+/);
    await p.idle();
    // Its button copies it to the folder on the other side, as PICT.hp.
    await ev(`document.querySelector(".pane[data-pane=left] .preview .preview-head button").click(); true`);
    await p.until(`!!tx.entry("PICT.hp", 1)`, STEP_MS, "PICT.hp in the folder");
    const pict = await ev(`tx.panes[1].place.folder.read(["hp-files"], "PICT.hp").then((f) => [...f.data])`);
    assert.equal(String.fromCharCode(...pict.slice(0, 7)), header);

    // An algebraic in a file from the calculator previews with its text:
    // the names come from the ROM of the emulated calculator of its model
    // (the owner's 48SX EQ: 'SIN(X)', 18.5 bytes, # 18FBh).
    await p.drop({ "EQ.txt": "%%HP: T(3)A(R)F(.);\r\n'SIN(X)'\r\n" });
    await p.idle(STEP_MS, "EQ put");
    const eq = await entry("EQ");
    assert.equal(eq?.kind, "Algebraic");
    if (model === "48sx") assert.deepEqual([eq.size, eq.checksum], [18.5, 0x18fb], "the owner's EQ");
    await ev(`tx.panes[0].copySelection([tx.entry("EQ")]).then(() => true)`);
    await p.until(`!!tx.entry("EQ.hp", 1)`, STEP_MS, "EQ.hp in the folder");
    await ev(`tx.select("EQ.hp", 1)`);
    if (model === "49g") {
      await p.until(`/copy it to a calculator/.test(document.querySelector(".pane[data-pane=right] .preview .unavailable")?.textContent ?? "")`, STEP_MS, "EQ.hp: why there is no text");
    } else {
      await p.until(`document.querySelector(".pane[data-pane=right] .preview pre.obj-program")?.textContent === "'SIN(X)'"`, STEP_MS, "EQ.hp previewed with its text");
    }
    await ev(`tx.select(null, 1)`);

    // Everything to a folder (the backup without free memory), then a backup.
    await ev(`tx.app.now = () => new Date(2026, 9, 11, 14, 30); true`);
    const copyRoot = `${model.toUpperCase()}-HOME-2026-10-11-1430`;
    await ev(`tx.panes[0].copyAll().then(() => true)`);
    await p.until(`!!tx.entry(${JSON.stringify(copyRoot)}, 1)`, STEP_MS, "HOME copied to a folder");
    const homeBefore = await ev(home());
    const backupName = `${model.toUpperCase()}-backup-2026-10-11-1430.hp`;
    await ev(`tx.panes[0].backup(); true`);
    await p.until(`document.getElementById("backup-dialog").open`, STEP_MS, "the backup dialog");
    assert.equal(await ev(`document.getElementById("backup-go").disabled`), false, "a fresh calculator has room");
    await ev(`document.getElementById("backup-go").click(); true`);
    await p.until(`!!tx.entry(${JSON.stringify(backupName)}, 1)`, STEP_MS * 2, "the backup in the folder");
    await p.idle(STEP_MS, "after the backup");
    assert.deepEqual(await ev(home()), homeBefore, "the backup left HOME as it was");

    // Purge D2: the question names what it holds.
    await ev(`tx.panes[0].act("remove", [tx.entry("D2")]); true`);
    await p.until(`!!document.querySelector("dialog.confirm[open]")`, STEP_MS, "the purge question");
    assert.match(await ev(`document.querySelector("dialog.confirm p").textContent`), /It holds 2 variables, which go too/);
    await p.answer("Purge D2?");
    await p.until(`!tx.entry("D2")`, STEP_MS, "D2 purged");

    // The folder copy of HOME put back into HOME (the names in use replaced after asking), then copied again: the same bytes.
    const copied = await ev(folderTree(["hp-files", copyRoot]));
    assert.ok(Object.keys(copied).includes("D2/PRG.hp"), JSON.stringify(Object.keys(copied)));
    await ev(`tx.panes[1].navigate(["hp-files", ${JSON.stringify(copyRoot)}]); true`);
    await p.idle(STEP_MS, "the copy listed", 1);
    await ev(`window.__copyDone = false; window.__copy = tx.panes[1].copySelection(tx.panes[1].ui.list.listing.entries).finally(() => { __copyDone = true; }); true`);
    const back = await answerAll(s, STEP_MS * 3);
    assert.deepEqual(back.failed, [], JSON.stringify(back));
    await p.idle(STEP_MS, "HOME put back");
    assert.deepEqual((await ev(home())).map((e) => e[0]), homeBefore.map((e) => e[0]), "HOME's names again");
    await ev(`tx.app.now = () => new Date(2026, 9, 11, 14, 31); tx.panes[1].navigate(["hp-files"]); true`);
    await ev(`tx.panes[0].copyAll().then(() => true)`);
    const copyRoot2 = `${model.toUpperCase()}-HOME-2026-10-11-1431`;
    await p.until(`!!tx.entry(${JSON.stringify(copyRoot2)}, 1)`, STEP_MS, "HOME copied again");
    assert.deepEqual(await ev(folderTree(["hp-files", copyRoot2])), copied, "the folder copy of HOME and back, byte for byte");

    // The backup restored onto a freshly booted calculator.
    await ev(`tx.panes[0].close().then(() => true)`);
    await boot(s, 0, model);
    assert.ok(!(await p.names()).includes("PRG"), "a fresh calculator");
    await ev(`tx.panes[1].act("restoreFrom", [tx.entry(${JSON.stringify(backupName)}, 1)]); true`);
    await p.answer("Replace everything on the calculator?");
    await p.until(`!!document.querySelector('.pane[data-pane=left] [data-action="reconnect"]')`, STEP_MS * 2, "the restart and the reconnect offer");
    assert.equal(await ev(`document.querySelector(".pane[data-pane=left] .start h2").textContent`), "The calculator restarted");
    await ev(`document.querySelector('.pane[data-pane=left] [data-action="reconnect"]').click(); true`);
    await p.idle(STEP_MS * 2, "reconnected after the restore");
    assert.deepEqual(await ev(home()), homeBefore, "the same HOME: names, sizes, checksums");

    await ev("tx.panes[0].close().then(() => true)");
    await p.until("!tx.panes[0].place", STEP_MS, "disconnected");
    assert.deepEqual(c.errors, []);
  });
}

test("an emulated 48GX to an emulated 49G, pane to pane, with the plan's warning first", { timeout: 1_200_000 }, async (t) => {
  const s = await start(t, [ROM_FILES["48gx"], ROM_FILES["49g"]]);
  if (!s) return;
  const { c, p } = s;
  const { ev } = c;
  await boot(s, 0, "48gx");
  await boot(s, 1, "49g");
  await p.drop({ "PRG.txt": PROGRAM, "X.txt": "%%HP: T(3)A(R)F(.);\r\n42.5\r\n" });
  await p.idle(STEP_MS, "PRG and X on the 48GX");
  const copy = ev(`tx.panes[0].copySelection([tx.entry("PRG"), tx.entry("X")]).then((r) => r)`);
  await p.until(`!!document.querySelector("dialog.confirm[open]")`, STEP_MS, "the plan's warning");
  assert.match(await ev(`document.querySelector("dialog.confirm .confirm-box").textContent`), /These are HP 48 objects\. They go to the HP 49G as text/);
  await p.answer("Copy 2 items to Emulated HP 49G?");
  const r = await copy;
  assert.deepEqual(r.copied.sort(), ["PRG", "X"]);
  await p.idle(STEP_MS, "the 49G listed", 1);
  assert.equal((await ev(`tx.entry("PRG", 1)`)).kind, "Program");
  assert.equal((await ev(`tx.entry("X", 1)`)).kind, "Real Number");
  await ev(`tx.panes[1].downloadText(tx.entry("PRG", 1))`);
  const got = await ev(`String.fromCharCode(...tx.app.lastDownload.bytes)`);
  assert.match(got, /1\.?\s+2\.?\s+\+/, "the 49G compiled the 48GX's text");
  await ev(`Promise.all([tx.panes[0].close(), tx.panes[1].close()]).then(() => true)`);
  assert.deepEqual(c.errors, []);
});

describe("the transfer page on the emulated calculators", { concurrency: true }, () => {
  for (const args of tests) nodeTest(...args);
});
