// The fast transfer (XModem) against the emulated 48GX (`just tx-e2e
// DIR`, with tx-e2e.test.mjs): headless Chrome, the page's own "Add an
// emulated calculator" path (gxrom-r from SATURNUS_ROM_DIR, read only)
// and a folder in memory: a program put by Kermit and got back as the
// reference; got by the fast transfer into the folder (the page puts
// `« 'PRG' XSEND »` on the stack and ends the server, "Press the keys for
// me" presses EVAL), byte for byte the same; Reconnect starts the server
// again; the file put back by the fast transfer under another name
// (`« 'PRG2' XRECV »`), byte for byte the same again over Kermit; a third
// fast get cancelled before EVAL, and the reconnect takes the program off
// the stack again. Files offers no fast transfer on the 49G since
// iteration 70 (XSERV copies as fast); its recorded trace
// (crates/saturnus-tx/traces/49g-xmodem.trace) stays for the bindings'
// replay.
//
// With SATURNUS_TX_TRACES=DIR the link's bytes are written to
// DIR/MODEL-xmodem.trace (saturnus-tx's trace format, the device seed
// fixed, each request the page started as a note): the traces of
// crates/saturnus-web/tests/xmodem.rs. Skipped
// without the ROMs (SATURNUS_AUDIT=1 makes that a failure), Chrome or the
// wasm package.

import assert from "node:assert/strict";
import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { session } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";
import { formatTrace } from "../tx/trace.js";

const dir = process.env.SATURNUS_ROM_DIR;
const traces = process.env.SATURNUS_TX_TRACES;
/** The device seed of a recorded run (the sync markers): crates/saturnus-tx/tests/xmodem.rs replays with it. */
const XMODEM_SEED = 0x7e57_0039;
const PROGRAM = "%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \"fast\" \\>>\r\n";
const STEP_MS = 120_000;

const dialogText = (cls) => `(document.querySelector("#xmodem-dialog .${cls}")?.textContent ?? "")`;
const shown = (act) => `(() => { const b = document.querySelector('#xmodem-dialog [data-act="${act}"]'); return !!b && !b.hidden && !b.disabled; })()`;
const press = (act) => `(() => { document.querySelector('#xmodem-dialog [data-act="${act}"]').click(); return true; })()`;

for (const [model, file, header] of [["48gx", "gxrom-r", "HPHP48-"]]) {
  test(`the emulated ${model}: a fast get into the folder and a fast put back, byte for byte`, { timeout: 1_200_000 }, async (t) => {
    const why = !dir || !existsSync(join(dir, file)) ? `no ${file} in SATURNUS_ROM_DIR` : txBuilt() ? null : "web/pkg not built (just web)";
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
    // The cable console open the whole time (iteration 45): it sees the emulated link like a cable.
    await ev(`document.getElementById("show-cable").click(); true`);
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

    // Fast get: the command on the stack, the keys pressed on the emulated calculator, into the folder.
    await ev(`window.__get = tx.panes[0].act("xmodemGet", [tx.entry("PRG")]); true`);
    await p.until(`${dialogText("xmodem-command")} === "« 'PRG' XSEND »"`, STEP_MS, "the command for the get");
    const notes = await ev(`[...document.querySelectorAll("#xmodem-dialog .xmodem-notes li")].map((l) => l.textContent).join(" ")`);
    assert.doesNotMatch(notes, /algebraic mode/);
    assert.equal(await ev(shown("type")), true, "the emulated calculator can have the keys pressed");
    await ev(press("type"));
    await p.until(shown("reconnect"), STEP_MS * 2, "the fast get done");
    assert.equal(await ev(dialogText("xmodem-lead")), "PRG came over: PRG.hp in hp-files.");
    const fast = await ev(`tx.panes[1].place.folder.read(["hp-files"], "PRG.hp").then((f) => [...f.data])`);
    assert.deepEqual(fast, reference, "the fast get is the Kermit get, byte for byte");
    await ev(press("reconnect"));
    await ev("__get");
    await p.idle(STEP_MS * 2, "reconnected after the get");
    assert.equal(await ev("tx.panes[0].place.device.leftServer"), false);

    // Fast put of the folder's PRG.hp as PRG2.
    await ev(`window.__put = tx.panes[1].act("xmodemPut", [tx.entry("PRG.hp", 1)]); true`);
    await p.until(`document.getElementById("name-dialog").open`, 10_000, "the name");
    await ev(`(() => { document.getElementById("name-input").value = "PRG2"; document.getElementById("name-ok").click(); return true; })()`);
    await p.until(`${dialogText("xmodem-command")} === "« 'PRG2' XRECV »"`, STEP_MS, "the command for the put");
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

    // The console drew the Kermit packets and the XModem blocks; nothing outside a packet.
    await p.until(`document.querySelectorAll("#cable .cable-line").length === Math.min(tx.app.cable.size, 3000)`, 5_000, "the console drawn");
    const cable = await ev(`(() => {
      const rows = [...document.querySelectorAll("#cable .cable-line")];
      const t = (r, s) => r.querySelector(s)?.textContent ?? "";
      return { rows: rows.length, kept: tx.app.cable.size, blocks: rows.filter((r) => /^(SOH|STX) /.test(t(r, ".cl-type"))).length, init: rows.some((r) => t(r, ".cl-type") === "I 0"), loose: rows.filter((r) => / outside a packet$/.test(t(r, ".cl-desc"))).map((r) => r.textContent) };
    })()`);
    assert.equal(cable.rows, Math.min(cable.kept, 3000), "every kept line drawn");
    assert.ok(cable.init, "the init packet");
    assert.ok(cable.blocks >= 2, `the fast get's and put's blocks (${cable.blocks})`);
    assert.deepEqual(cable.loose, [], "nothing outside a packet");

    if (traces) {
      const text = await ev(`__rec.trace([
        "saturnus tx trace: the fast transfer of web/test/tx-e2e-xmodem.test.mjs on the",
        "emulated ${model.toUpperCase()} (ROM file ${file}), recorded with SATURNUS_TX_TRACES.",
        "Serial bytes only; times in ms since the link was made; seed ${XMODEM_SEED}.",
      ])`);
      writeFileSync(join(traces, `${model}-xmodem.trace`), text);
    }

    // Cancelled before EVAL (after the trace: its replay has no abort): the command waits on the stack, and the reconnect takes it off again.
    const stack = () => ev(`tx.panes[0].place.emulator.request("stack")`);
    const before = await stack();
    await ev(`window.__left = tx.panes[0].act("xmodemGet", [tx.entry("PRG")]); true`);
    await p.until(`${dialogText("xmodem-command")} === "« 'PRG' XSEND »"`, STEP_MS, "the command for the cancelled get");
    const waiting = await stack();
    assert.equal(waiting.length, before.length + 1, `the command on the stack (${JSON.stringify(waiting)})`);
    await ev(press("cancel"));
    await p.until(shown("reconnect"), STEP_MS, "cancelled");
    assert.match(await ev(dialogText("xmodem-lead")), /^Stopped/);
    await ev(press("reconnect"));
    await ev("__left");
    await p.idle(STEP_MS * 2, "reconnected after the cancel");
    assert.equal(await ev("tx.panes[0].place.device.leftServer"), false);
    assert.deepEqual(await stack(), before, "the stack as it was");
    await ev("tx.panes[0].close().then(() => true)");
    assert.deepEqual(c.errors, []);
  });
}
