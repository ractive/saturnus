// The serial port in the browser's host, on the real ROMs: worker.js and
// the real wasm (web/pkg, `just web`) boot a 48GX and a 49G from
// `SATURNUS_ROM_DIR` (`gxrom-r`, `rom-2.10.49g`; skipped without them),
// the page takes the port (`serialOpen`), types SERVER, hears the
// server's NAKs as `serial` events (bytes as a Uint8Array), writes a
// Kermit Send-Init (`serialWrite` with a Uint8Array) and gets the
// server's ACK; a write (a hidden transaction) is refused meanwhile, and
// FINISH ends the server. The same through the state machine:
// crates/saturnus-host/tests/serial.rs.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const MODELS = [
  ["48gx", "gxrom-r"],
  ["49g", "rom-2.10.49g"],
];

const dir = process.env.SATURNUS_ROM_DIR;
const pkgFile = new URL("../pkg/saturnus_web_bg.wasm", import.meta.url);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** A Kermit packet with a type-1 check, ended by CR. */
function packet(seq, kind, data) {
  const tochar = (n) => n + 32;
  const body = [tochar(3 + data.length), tochar(seq), kind.charCodeAt(0), ...data];
  const s = body.reduce((a, b) => a + b, 0);
  return new Uint8Array([1, ...body, tochar((s + ((s & 192) >> 6)) & 63), 13]);
}

/** The packets in `bytes`: `{seq, kind, data}`. */
function packets(bytes) {
  const out = [];
  for (let i = bytes.indexOf(1); i >= 0 && i + 1 < bytes.length; i = bytes.indexOf(1, i + 1)) {
    const len = bytes[i + 1] - 32;
    if (len < 3 || i + 2 + len > bytes.length) continue;
    out.push({ seq: bytes[i + 2] - 32, kind: String.fromCharCode(bytes[i + 3]), data: bytes.slice(i + 4, i + 1 + len) });
  }
  return out;
}

test("Kermit through the page's serial port in the Worker", { timeout: 240_000 }, async (t) => {
  const roms = MODELS.filter(([, f]) => dir && existsSync(join(dir, f)));
  if (!roms.length) {
    t.skip("SATURNUS_ROM_DIR holds none of gxrom-r, rom-2.10.49g");
    return;
  }
  if (!existsSync(pkgFile)) {
    t.skip("no web/pkg (just web)");
    return;
  }
  const pkg = await import("../pkg/saturnus_web.js");
  pkg.initSync({ module: readFileSync(pkgFile) });
  const posted = [];
  // The Worker keeps its auto-saves in IndexedDB, which node has not: it says so and goes on.
  const warn = console.warn;
  console.warn = (...a) => String(a[0]).includes("was not kept") || warn(...a);
  globalThis.self = { postMessage: (m) => posted.push(m) };
  await import("../worker.js");

  let id = 0;
  const call = async (cmd, args = {}, ms = 60_000) => {
    const mine = ++id;
    self.onmessage({ data: { v: 1, id: mine, cmd, ...args } });
    const end = Date.now() + ms;
    for (;;) {
      const r = posted.find((m) => m.type === "reply" && m.id === mine);
      if (r) return r;
      if (Date.now() > end) assert.fail(`no reply to ${cmd}`);
      await sleep(5);
    }
  };
  /** The calculator's bytes since `from` (an index into `posted`). */
  const heard = (from) => {
    const all = posted.slice(from).filter((m) => m.type === "serial");
    for (const m of all) assert.ok(m.bytes instanceof Uint8Array && m.bytes.length, "bytes, as bytes");
    return new Uint8Array(all.flatMap((m) => [...m.bytes]));
  };
  const until = async (from, kind, ms) => {
    const end = Date.now() + ms;
    for (;;) {
      const p = packets(heard(from)).find((p) => p.kind === kind);
      if (p) return p;
      if (Date.now() > end) assert.fail(`no ${kind} packet in ${ms} ms: ${JSON.stringify(packets(heard(from)))}`);
      await sleep(20);
    }
  };

  for (const [model, file] of roms) {
    const rom = new Uint8Array(readFileSync(join(dir, file)));
    assert.equal((await call("boot", { model, rom, romName: file })).ok, true, model);
    // The host answers "Try To Recover Memory?" itself. Up: the memory
    // reads, and the calculator waits for a key, a second in a row.
    const ready = Date.now() + 90_000;
    for (let idle = 0; idle < 5; ) {
      const up = (await call("stack")).ok && (await call("stats")).result.loop === "sleep";
      idle = up ? idle + 1 : 0;
      assert.ok(Date.now() < ready, `${model}: the calculator never came up`);
      await sleep(200);
    }
    await call("setSpeed", { speed: "max" });

    const from = posted.length;
    assert.deepEqual(await call("serialOpen"), { type: "reply", id, ok: true, result: null }, model);
    const run = await call("run", { text: "SERVER" });
    assert.equal(run.ok, true, `${model}: ${run.error}`);
    const refused = await call("createDir", { dir: ["HOME"], name: "SERA" });
    assert.equal(refused.error, "the serial port is in use (serialClose frees it)", model);

    await until(from, "N", 60_000);
    // Send-Init with the defaults: MAXL 80, TIME 5, no padding, CR, #, no 8th-bit or repeat prefix, check 1.
    const init = [80, 5, 0].map((n) => n + 32).concat([64, 13 + 32, 35, 32, 49, 32]);
    const wrote = posted.length;
    assert.equal((await call("serialWrite", { bytes: packet(0, "I", init) })).ok, true, model);
    const ack = await until(wrote, "Y", 30_000);
    assert.equal(ack.seq, 0, model);
    assert.ok(ack.data[0] - 32 >= 30, `${model}: the server's MAXL`);

    const fin = posted.length;
    assert.equal((await call("serialWrite", { bytes: packet(0, "G", [70]) })).ok, true, model);
    assert.equal((await until(fin, "Y", 30_000)).seq, 0, `${model}: FINISH acknowledged`);
    assert.equal((await call("serialClose")).ok, true, model);
  }
  // Nothing left running (or saving), so the test's process ends.
  assert.equal((await call("unload")).ok, true);
});
