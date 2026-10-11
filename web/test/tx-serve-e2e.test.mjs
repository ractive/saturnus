// "Serve a folder to a 38G, 39G or 40G" against the emulated calculators
// (part of `just tx-e2e DIR`): headless Chrome, the page's serve screen
// with a folder in memory and the emulated 38G (38G_A167.ROM) or 39G
// (rom.39g) booted in the page from SATURNUS_ROM_DIR, its keys pressed as
// a user would. Per model: the Function aplet sent to the folder (SEND,
// Disk drive, Send now; the calculator's directory file kept there too),
// received back (RECV, Disk drive, Function), then sent again over the
// old copy ("Replace existing aplet?"): the same bytes. On the 39G also
// the aplet by XModem (RECV, "HP39/40 (Wire)") and sent again: the same
// bytes. The built-in aplet's bytes stay in the test's memory; nothing is
// written to disk. Skipped without the ROMs (SATURNUS_AUDIT=1 makes the
// skip a failure), Chrome or the wasm packages.

import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { session, sleep } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

const dir = process.env.SATURNUS_ROM_DIR;
/** How long one transfer may take at 1x. */
const STEP_MS = 120_000;

const MODELS = [
  { model: "38g", file: "38G_A167.ROM", family: "HP38", library: "lib", disk: ["down"] },
  { model: "39g", file: "rom.39g", family: "HP39", library: "aplet", disk: ["down", "down"] },
];

function missing(file) {
  if (!dir || !existsSync(join(dir, file))) return `no ${file} in SATURNUS_ROM_DIR`;
  return txBuilt() ? null : "web/pkg or web/tx/pkg not built (just web tx-web)";
}

for (const { model, file, family, library, disk } of MODELS) {
  test(`serving a folder to the emulated ${model}: an aplet to the folder and back, byte for byte`, { timeout: 1_200_000 }, async (t) => {
    const why = missing(file);
    if (why) {
      if (process.env.SATURNUS_AUDIT) assert.fail(why);
      t.skip(why);
      return;
    }
    const c = await session(t, { files: { [`/${file}`]: join(dir, file) }, audit: true });
    if (!c) return;
    const { ev } = c;
    await c.send("Page.enable");
    await c.send("Runtime.enable");
    await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
    const p = txPage(c);
    await p.open(c.port);
    await ev(`tx.memoryFolder(1, "aplets").then(() => true)`);

    // The serve screen: the folder of the right pane is offered; an emulated calculator from a ROM file.
    await ev(`tx.serve.show().then(() => true)`);
    assert.equal(await ev(`document.querySelector("#serve .serve-choice").textContent`), "aplets");
    await ev(`fetch("/${file}").then((r) => r.arrayBuffer()).then((b) => { tx.serve.setCalc({ key: "file", bytes: new Uint8Array(b), name: ${JSON.stringify(file)} }); return true; })`);
    await ev(`(window.__serving = tx.serve.start().then(() => true), true)`);
    await p.until(`window.__serving && (!!tx.serve.session || !tx.serve.busy)`, 240_000, `${model}: booted`);
    await ev(`window.__serving`);
    if (!(await ev(`!!tx.serve.session`))) {
      assert.fail(`${model}: not serving: ${await ev(`[...document.querySelectorAll("[role=alert], [role=status], .toast")].map((e) => e.textContent).join(" | ")`)}`);
    }
    assert.match(await ev(`document.querySelector("#serve h2").textContent`), /^Serving aplets$/);
    assert.match(await ev(`document.querySelector("#serve .serve-to").textContent`), new RegExp(`emulated ${model.toUpperCase()}`));

    const keys = async (...names) => {
      for (const k of names) {
        await ev(`tx.serve.emulator.tap(${JSON.stringify(k)}).then(() => true)`);
        await sleep(700);
      }
    };
    const log = () => ev(`[...document.querySelectorAll("#serve .serve-log .serve-line span")].map((s) => s.textContent)`);
    const waitLog = (re, what) => p.until(`[...document.querySelectorAll("#serve .serve-log .serve-line span")].some((s) => ${re}.test(s.textContent))`, STEP_MS, `${model}: ${what}`);
    const folder = () => ev(`(async () => { const f = tx.serve.folder; const out = {}; for (const e of (await f.list(f.home)).entries) out[e.name] = [...(await f.read(f.home, e.name)).data]; return out; })()`);
    const lines = () => ev(`document.querySelectorAll("#serve .serve-log .serve-line:not(.muted)").length`);

    // SEND: Function to the folder.
    await keys(library, "d", ...disk, "f", "f");
    await waitLog(`/^Received Function \\(FUNCTION\\.000\\)/`, "Function received");
    await p.until(`[...document.querySelectorAll("#serve .serve-log span")].some((s) => /wrote its list/.test(s.textContent))`, STEP_MS, `${model}: the directory file written`);
    assert.match(await ev(`document.querySelector("#serve .serve-log").textContent`), new RegExp(`calculator is an HP ${model === "38g" ? "38G" : "39G or 40G"}`));
    const first = await folder();
    const aplet = first["FUNCTION.000"];
    assert.ok(aplet, "FUNCTION.000 in the folder");
    assert.equal(String.fromCharCode(...aplet.slice(0, 8)), `${family}BinB`);
    assert.equal(String.fromCharCode(...first[`${family}DIR.000`]), `${family}AscA B 12 FUNCTION.0008 Function`);
    assert.ok(!(`${family}DIR.CUR` in first), "the page answered HPxxDIR.CUR itself: no Initialize directory? question");

    // RECV: Function back from the folder.
    let before = await lines();
    await keys(library, "e", ...disk, "f");
    await sleep(3000);
    await keys("f");
    await waitLog(`/^Sent FUNCTION\\.000\\.$/`, "Function sent back");
    assert.ok((await lines()) > before);
    await sleep(2000);

    // SEND again over the folder's copy: the calculator asks to replace it; the same bytes.
    before = (await log()).filter((l) => l.startsWith("Received Function")).length;
    await keys(library, "d", ...disk, "f", "f");
    await sleep(4000);
    await keys("f");
    await p.until(`[...document.querySelectorAll("#serve .serve-log span")].filter((s) => /^Received Function/.test(s.textContent)).length > ${before}`, STEP_MS, `${model}: Function received again`);
    await sleep(3000);
    const again = await folder();
    assert.deepEqual(again["FUNCTION.000"], aplet, "the aplet came back byte for byte");
    assert.equal(String.fromCharCode(...again[`${family}DIR.000`]), `${family}AscA B 12 FUNCTION.0008 Function`);

    if (model === "39g") {
      // By wire: Function saved as XYZ, XYZ sent to the folder and deleted
      // on the calculator, then sent back by XModem: XYZ is there again,
      // and goes to the folder again byte for byte.
      await keys("aplet", "a", "alpha", "1", "alpha", "2", "alpha", "3", "f");
      await keys("d", ...disk, "f", "f");
      await waitLog(`/^Received XYZ \\(XYZ00000\\.000\\)/`, "XYZ received");
      await sleep(3000);
      const xyz = (await folder())["XYZ00000.000"];
      assert.equal(String.fromCharCode(...xyz.slice(0, 12)), "HP39BinB\x03XYZ");
      await keys("del", "f");
      await p.until(`[...document.querySelectorAll("#serve-wire-file option")].some((o) => o.value === "XYZ00000.000")`, 10_000, "XYZ offered by wire");
      await ev(`(() => { const s = document.getElementById("serve-wire-file"); s.value = "XYZ00000.000"; document.getElementById("serve-wire-send").click(); return true; })()`);
      await sleep(1000);
      await keys("aplet", "e", "down", "f");
      await waitLog(`/^Sent by wire/`, "sent by wire");
      await sleep(3000);
      await keys("d", ...disk, "f", "f");
      await sleep(4000);
      // "Replace existing aplet XYZ?": the folder has it.
      await keys("f");
      await p.until(`[...document.querySelectorAll("#serve .serve-log span")].filter((s) => /^Received XYZ/.test(s.textContent)).length > 1`, STEP_MS, "XYZ received after the wire");
      await sleep(3000);
      assert.deepEqual((await folder())["XYZ00000.000"], xyz, "the aplet taken by wire is the same");
    }
    assert.deepEqual((await log()).filter((l) => /broke off|not in the folder|could not be saved/.test(l)), []);

    await ev(`document.getElementById("serve-stop").click(); true`);
    await p.until(`!tx.serve.session`, 30_000, "stopped");
    assert.deepEqual(c.errors, []);
  });
}
