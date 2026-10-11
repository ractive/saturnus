// The transfer page against the emulated calculators (`just tx-e2e DIR`):
// headless Chrome, the page's own "Add an emulated calculator" path
// (saturnus's core in the transfer page's Worker, booted from the ROM in
// SATURNUS_ROM_DIR, SERVER typed, its serial port the link) on the 48SX
// (sxrom-j), 48GX (gxrom-r) and 49G (rom.49g): connect and detect, list,
// put a text file, a plain file as a string and a binary file, get
// binary and as text, the round trip byte for byte, "Replace X?", the
// preview of a program, a string and a GROB, disconnect. Skipped without
// the ROMs (SATURNUS_AUDIT=1, as `just tx-e2e` sets it, makes the skip a
// failure), Chrome or the wasm packages. The ROMs are only read.

import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { session } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

const MODELS = [
  ["48sx", "sxrom-j", "HPHP48-"],
  ["48gx", "gxrom-r", "HPHP48-"],
  ["49g", "rom.49g", "HPHP49-"],
];
const dir = process.env.SATURNUS_ROM_DIR;
const PROGRAM = "%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \\>>\r\n";
const PROGRAM2 = "%%HP: T(3)A(R)F(.);\r\n\\<< 6 7 * \\>>\r\n";
const NOTE = "hello, calculator";
const GROB = "%%HP: T(3)A(R)F(.);\r\nGROB 8 4 F09090F0\r\n";

for (const [model, file, header] of MODELS) {
  test(`the emulated ${model}: connect, list, put, get, replace, preview, disconnect`, { timeout: 600_000 }, async (t) => {
    const rom = dir && join(dir, file);
    if (!rom || !existsSync(rom) || !txBuilt()) {
      const why = !rom || !existsSync(rom) ? `no ${file} in SATURNUS_ROM_DIR` : "web/pkg or web/tx/pkg not built (just web tx-web)";
      if (process.env.SATURNUS_AUDIT) assert.fail(why);
      t.skip(why);
      return;
    }
    const c = await session(t, { files: { "/rom.bin": rom }, audit: true });
    if (!c) return;
    const { ev } = c;
    await c.send("Page.enable");
    await c.send("Runtime.enable");
    await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
    const p = txPage(c);
    await p.open(c.port);
    const put = (text, name) => ev(`tx.putFiles([new File([${JSON.stringify(text)}], ${JSON.stringify(name)})]).then(() => true)`);
    const putBytes = (bytes, name) => ev(`tx.putFiles([new File([new Uint8Array(${JSON.stringify(bytes)})], ${JSON.stringify(name)})]).then(() => true)`);
    const get = async (name, text = false) => {
      await ev(`tx.getVariable(tx.ui.list.listing.entries.find((e) => e.name === ${JSON.stringify(name)}), { text: ${text} })`);
      return ev(`({ name: tx.state.lastDownload.name, bytes: [...tx.state.lastDownload.bytes] })`);
    };
    const entry = (name) => ev(`tx.ui.list.listing.entries.find((e) => e.name === ${JSON.stringify(name)}) ?? null`);

    await ev(`fetch("/rom.bin").then((r) => r.arrayBuffer()).then((b) => { tx.addEmulated(new Uint8Array(b), ${JSON.stringify(file)}, ${JSON.stringify(model)}); return true; })`);
    await p.until("!!tx.state.device?.connection", 240_000, `${model}: connected`);
    const conn = await ev("tx.state.device.connection");
    assert.equal(conn.model, model, "the model detected");
    assert.deepEqual(conn.settings, { baud: 9600, parity: "none", dataBits: 8 });
    assert.equal(await ev(`document.getElementById("pane-title").textContent`), `Emulated ${model.toUpperCase()}`);
    await p.idle(60_000, "HOME listed");

    // A text file: the calculator compiles it.
    await put(PROGRAM, "PRG.txt");
    await p.idle(60_000, "PRG put");
    assert.equal((await entry("PRG"))?.kind, "Program");
    const prg = await get("PRG");
    assert.equal(prg.name, "PRG.hp");
    assert.equal(String.fromCharCode(...prg.bytes.slice(0, 7)), header);
    // The binary file back under another name: the same object, byte for byte.
    await putBytes(prg.bytes, "PRG2.hp");
    await p.idle(60_000, "PRG2 put");
    const prg2 = await get("PRG2");
    assert.deepEqual(prg2.bytes, prg.bytes, "a binary round trip is exact");
    assert.equal((await entry("PRG2")).checksum, (await entry("PRG")).checksum);
    const text = await get("PRG2", true);
    assert.equal(text.name, "PRG2.txt");
    assert.match(String.fromCharCode(...text.bytes), /^%%HP:[^\r\n]*\r?\n[\s\S]*1 2 \+/);

    // A file that is no calculator object: a string, after asking.
    const note = put(NOTE, "NOTE.md");
    await p.answer("Store it as a string?");
    await note;
    await p.idle(60_000, "NOTE put");
    assert.equal((await entry("NOTE"))?.kind, "String");
    const noteFile = await get("NOTE");
    assert.equal(await ev(`tx.helpers().then((h) => h.describe_file(new Uint8Array(${JSON.stringify(noteFile.bytes)})).value)`), NOTE);

    // A name in use: "Replace PRG?", and PRG is the new program.
    const before = (await entry("PRG")).checksum;
    const again = put(PROGRAM2, "PRG.txt");
    await p.answer("Replace PRG?");
    await again;
    await p.idle(60_000, "PRG replaced");
    assert.notEqual((await entry("PRG")).checksum, before);
    assert.ok(!(await p.names()).some((n) => /^SATXPUT/.test(n)), "no temporary variable left");

    // The previews: a program as the calculator's text, a string, a GROB.
    await put(GROB, "PIC.txt");
    await p.idle(60_000, "PIC put");
    assert.equal((await entry("PIC"))?.kind, "Graphic");
    await ev(`tx.select("PRG")`);
    await p.until(`/6 7 \\*/.test(document.querySelector("#preview pre.obj-program")?.textContent ?? "")`, 60_000, "PRG previewed");
    await p.idle();
    await ev(`tx.select("NOTE")`);
    await p.until(`document.querySelector("#preview pre.obj-string")?.textContent === ${JSON.stringify(NOTE)}`, 60_000, "NOTE previewed");
    await p.idle();
    await ev(`tx.select("PIC")`);
    await p.until(`document.querySelector("#preview canvas.graphic")?.getAttribute("width") === "8"`, 60_000, "PIC previewed");
    await p.idle();

    await ev("tx.disconnect()");
    await p.until("!tx.state.device", 60_000, "disconnected");
    assert.equal(await ev(`document.getElementById("pane-title").textContent`), "No calculator");
    assert.deepEqual(c.errors, []);
  });
}
