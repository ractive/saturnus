// The fast transfer (XModem) against the emulated 48GX and 49G (`just
// tx-e2e DIR`, with tx-e2e.test.mjs): headless Chrome, the page's own
// "Add an emulated calculator" path (gxrom-r, rom.49g from
// SATURNUS_ROM_DIR, read only) and a folder in memory. Per model: a
// program put by Kermit and got back as the reference; got by the fast
// transfer into the folder (the page ends the server, the dialog says
// `'PRG' XSEND`, "Type it for me" types it on the emulated calculator),
// byte for byte the same; Reconnect starts the server again; the file put
// back by the fast transfer under another name (`'PRG2' XRECV`), byte for
// byte the same again over Kermit. The 49G, booted in algebraic mode, is
// switched to RPN for the command and back at the reconnect.
//
// With SATURNUS_TX_TRACES=DIR the link's bytes are written to
// DIR/MODEL-xmodem.trace (saturnus-tx's trace format, the device seed
// fixed, each request the page started as a note): the traces of
// crates/saturnus-tx-web/tests/xmodem.rs. Skipped
// without the ROMs (SATURNUS_AUDIT=1 makes that a failure), Chrome or the
// wasm packages.

import assert from "node:assert/strict";
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { session } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";
import { formatTrace, parseTrace } from "../tx/trace.js";

const dir = process.env.SATURNUS_ROM_DIR;
const traces = process.env.SATURNUS_TX_TRACES;
/** The device seed of a recorded run (the sync markers): crates/saturnus-tx/tests/xmodem.rs replays with it. */
const XMODEM_SEED = 0x7e57_0039;
const PROGRAM = "%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \"fast\" \\>>\r\n";
const STEP_MS = 120_000;

/** CRC-16/KERMIT, the 49G's `D` mode (wiki: protocols/xmodem-hp). */
function crc16(bytes) {
  let c = 0;
  for (const b of bytes) {
    c ^= b;
    for (let i = 0; i < 8; i++) c = c & 1 ? (c >>> 1) ^ 0x8408 : c >>> 1;
  }
  return c;
}

/**
 * The 49G's XSEND pads its last block with what follows the object in
 * memory, which may hold a run of the ROM: in the trace, every byte of
 * the received blocks after the file's `length` becomes zero and each
 * block's CRC is computed again (the reads keep their lengths and times).
 */
function zeroPadding(text, length) {
  const steps = parseTrace(text);
  const first = steps.findIndex((s) => s.kind === "read" && s.bytes[0] === 0x01 && s.bytes[1] === 0x01 && s.bytes[2] === 0xfe);
  assert.ok(first >= 0, "the first XModem block");
  // The blocks' reads, up to the end of transmission (the ACKs between them are writes).
  const reads = [];
  for (const s of steps.slice(first)) {
    if (s.kind !== "read") continue;
    if (s.bytes[0] === 0x04 && reads.length) break;
    reads.push(s);
  }
  const all = Uint8Array.from(reads.flatMap((s) => [...s.bytes]));
  let at = 0;
  let offset = 0;
  while (at < all.length && (all[at] === 0x01 || all[at] === 0x02)) {
    const size = all[at] === 0x01 ? 128 : 1024;
    const data = all.subarray(at + 3, at + 3 + size);
    for (let i = 0; i < size; i++) if (offset + i >= length) data[i] = 0;
    const c = crc16(data);
    all[at + 3 + size] = c >> 8;
    all[at + 4 + size] = c & 0xff;
    offset += size;
    at += size + 5;
  }
  assert.ok(offset >= length && at === all.length, "the file's blocks, and nothing else");
  let pos = 0;
  for (const s of reads) {
    s.bytes = all.slice(pos, pos + s.bytes.length);
    pos += s.bytes.length;
  }
  const notes = steps.filter((s) => s.kind === "note");
  const head = notes.findIndex((s) => /^Serial bytes only/.test(s.text));
  steps.splice(head + 1, 0, { kind: "note", text: "Edited: XSEND's padding of its last block (memory garbage, which held a run" }, { kind: "note", text: "of the ROM) is zeros, and the block's CRC recomputed for them." });
  return formatTrace(steps);
}

const dialogText = (cls) => `(document.querySelector("#xmodem-dialog .${cls}")?.textContent ?? "")`;
const shown = (act) => `(() => { const b = document.querySelector('#xmodem-dialog [data-act="${act}"]'); return !!b && !b.hidden && !b.disabled; })()`;
const press = (act) => `(() => { document.querySelector('#xmodem-dialog [data-act="${act}"]').click(); return true; })()`;

for (const [model, file, header] of [["48gx", "gxrom-r", "HPHP48-"], ["49g", "rom.49g", "HPHP49-"]]) {
  test(`the emulated ${model}: a fast get into the folder and a fast put back, byte for byte`, { timeout: 1_200_000 }, async (t) => {
    const why = !dir || !existsSync(join(dir, file)) ? `no ${file} in SATURNUS_ROM_DIR` : txBuilt() ? null : "web/pkg or web/tx/pkg not built (just web tx-web)";
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
    if (traces) {
      // Every request the page starts as a note before its bytes: the replay starts the same ones.
      await ev(`(() => {
        tx.app.wrapLink = (l) => (window.__rec = new tx.links.RecordingLink(l));
        const worker = () => {
          const w = new Worker(new URL("tx/protocol-worker.js", location.href), { type: "module" });
          const post = w.postMessage.bind(w);
          w.postMessage = (m) => {
            if (m?.cmd === "start") __rec.note("request " + JSON.stringify(m.request));
            post(m);
          };
          return w;
        };
        tx.app.deviceOptions = { seed: ${XMODEM_SEED}, worker };
        return true;
      })()`);
    }
    await ev(`tx.memoryFolder(1, "hp-files").then(() => true)`);
    await ev(`fetch("/${file}").then((r) => r.arrayBuffer()).then((b) => { tx.panes[0].addEmulated(new Uint8Array(b), ${JSON.stringify(file)}, ${JSON.stringify(model)}); return true; })`);
    await p.until("!!tx.panes[0].place?.device?.connection", 240_000, `${model}: connected`);
    await p.idle(STEP_MS, `${model}: HOME listed`);
    assert.equal(await ev("tx.panes[0].xmodem"), true);

    // The reference: PRG put by Kermit and got back as its binary file.
    await p.drop({ "PRG.txt": PROGRAM });
    await p.idle(STEP_MS, "PRG put");
    await ev(`tx.panes[0].download([tx.entry("PRG")])`);
    const reference = await ev(`[...tx.app.lastDownload.bytes]`);
    assert.equal(String.fromCharCode(...reference.slice(0, 7)), header);
    const checksum = (await ev(`tx.entry("PRG")`)).checksum;

    // Fast get: what to type, typed on the emulated calculator, into the folder.
    await ev(`window.__get = tx.panes[0].act("xmodemGet", [tx.entry("PRG")]); true`);
    await p.until(`${dialogText("xmodem-command")} === "'PRG' XSEND"`, STEP_MS, "what to type for the get");
    const notes = await ev(`[...document.querySelectorAll("#xmodem-dialog .xmodem-notes li")].map((l) => l.textContent).join(" ")`);
    if (model === "49g") assert.match(notes, /algebraic mode/, "the fresh 49G is in algebraic mode");
    assert.equal(await ev(shown("type")), true, "the emulated calculator can have it typed");
    await ev(press("type"));
    await p.until(shown("reconnect"), STEP_MS * 2, "the fast get done");
    assert.equal(await ev(dialogText("xmodem-lead")), "PRG came over: PRG.hp in hp-files.");
    const fast = await ev(`tx.panes[1].place.folder.read(["hp-files"], "PRG.hp").then((f) => [...f.data])`);
    assert.deepEqual(fast, reference, "the fast get is the Kermit get, byte for byte");
    await ev(press("reconnect"));
    await ev("__get");
    await p.idle(STEP_MS * 2, "reconnected after the get");
    assert.equal(await ev("tx.panes[0].place.device.leftServer"), false);
    if (model === "49g") {
      const flags = await ev(`tx.panes[0].place.emulator.request("flags")`);
      assert.ok(flags.set.includes(-95), `algebraic mode again after the reconnect (set: ${JSON.stringify(flags.set)})`);
    }

    // Fast put of the folder's PRG.hp as PRG2.
    await ev(`window.__put = tx.panes[1].act("xmodemPut", [tx.entry("PRG.hp", 1)]); true`);
    await p.until(`document.getElementById("name-dialog").open`, 10_000, "the name");
    await ev(`(() => { document.getElementById("name-input").value = "PRG2"; document.getElementById("name-ok").click(); return true; })()`);
    await p.until(`${dialogText("xmodem-command")} === "'PRG2' XRECV"`, STEP_MS, "what to type for the put");
    await ev(press("type"));
    await p.until(shown("reconnect"), STEP_MS * 2, "the fast put done");
    assert.equal(await ev(dialogText("xmodem-lead")), "PRG2 is on the calculator.");
    await ev(press("reconnect"));
    await ev("__put");
    await p.idle(STEP_MS * 2, "reconnected after the put");
    const prg2 = await ev(`tx.entry("PRG2")`);
    assert.equal(prg2?.kind, "Program");
    assert.equal(prg2.checksum, checksum, "the same object");
    await ev(`tx.panes[0].download([tx.entry("PRG2")])`);
    assert.deepEqual(await ev(`[...tx.app.lastDownload.bytes]`), reference, "the fast put stored the same object, byte for byte");

    if (traces) {
      const text = await ev(`__rec.trace([
        "saturnus tx trace: the fast transfer of web/test/tx-e2e-xmodem.test.mjs on the",
        "emulated ${model.toUpperCase()} (ROM file ${file}), recorded with SATURNUS_TX_TRACES.",
        "Serial bytes only; times in ms since the link was made; seed ${XMODEM_SEED}.",
      ])`);
      writeFileSync(join(traces, `${model}-xmodem.trace`), model === "49g" ? zeroPadding(text, reference.length) : text);
    }
    await ev("tx.panes[0].close().then(() => true)");
    assert.deepEqual(c.errors, []);
  });
}
