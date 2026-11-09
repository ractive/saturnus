// The transfer page's protocol stack in Node, without a browser: the
// wasm device (web/pkg, `just web`; skipped without it) in the
// driver of the protocol Worker (web/tx/driver.js, run in-process here),
// the page's side (protocol.js), the Device (kermit-device.js) and a
// FakeLink playing the calculator of saturnus-tx's recorded traces
// (crates/saturnus-tx/traces, recorded from the emulated 48SX, 48GX and
// 49G). The session is the traces' own (crates/saturnus-tx/tests/
// scenario): connect, list, put text, string and binary, get binary and
// text, a put to a name in use refused, then replaced, list, disconnect.
// And the HP 100LX palmtop's recorded session (`100lx-session.trace`,
// saturnus-tx's palmtop_tests.rs against filer-proto's server) through
// the same driver over the wasm TxPalmtop. Every byte the device writes
// must be the trace's.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { DeviceDriver } from "../tx/driver.js";
import { KermitDevice } from "../tx/kermit-device.js";
import { FakeLink } from "../tx/links.js";
import { ProtocolClient } from "../tx/protocol.js";

const pkgFile = new URL("../pkg/saturnus_web_bg.wasm", import.meta.url);
const TRACES = new URL("../../crates/saturnus-tx/traces/", import.meta.url);
/** The traces' seed (crates/saturnus-tx/tests/scenario `SEED`). */
const SEED = 0x5a70037;
const PROGRAM = new TextEncoder().encode("%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \\>>\r\n");
const NOTE = new TextEncoder().encode("hello, calculator");
const ARROW = "A\u2192B";

let wasm = null;
async function load() {
  if (!existsSync(pkgFile)) return null;
  wasm ??= import("../pkg/saturnus_web.js").then((pkg) => {
    pkg.initSync({ module: readFileSync(pkgFile) });
    return pkg;
  });
  return wasm;
}

/** A Worker stand-in running the protocol driver in this process. */
export function inProcessWorker(TxDevice) {
  const w = {
    onmessage: null,
    terminated: false,
    driver: null,
    postMessage(m) {
      // As a Worker: later, in order.
      setTimeout(() => {
        if (w.terminated) return;
        if (m.cmd === "init") {
          w.driver = new DeviceDriver({
            TxDevice,
            now: () => performance.now(),
            setTimer: (fn, ms) => setTimeout(fn, ms),
            clearTimer: (t) => clearTimeout(t),
            post: (msg) => setTimeout(() => !w.terminated && w.onmessage?.({ data: msg }), 0),
            seed: m.seed,
            listenMs: m.listenMs,
            silenceMs: m.silenceMs,
          });
          w.onmessage?.({ data: { type: "ready" } });
          return;
        }
        w.driver.handle(m);
      }, 0);
    },
    terminate() {
      w.terminated = true;
      if (w.driver?.timer) clearTimeout(w.driver.timer);
    },
  };
  return w;
}

for (const model of ["48sx", "48gx", "49g"]) {
  test(`the ${model} trace's session through the page's protocol stack`, { timeout: 120_000 }, async (t) => {
    const pkg = await load();
    if (!pkg) {
      t.skip("web/pkg not built (just web)");
      return;
    }
    const link = new FakeLink(readFileSync(new URL(`${model}-session.trace`, TRACES), "utf8"));
    const client = new ProtocolClient(link, { worker: () => inProcessWorker(pkg.TxDevice), seed: SEED });
    const device = new KermitDevice(client);
    const busy = [];
    device.addEventListener("busy", (e) => busy.push(e.detail.label));
    const header = model === "49g" ? "HPHP49-" : "HPHP48-";
    try {
      const conn = await device.connect();
      assert.equal(conn.model, model);
      assert.deepEqual(conn.settings, { baud: 9600, parity: "none", dataBits: 8 });
      const listing = await device.list(["HOME"]);
      assert.deepEqual(listing.path, ["HOME"]);
      assert.ok(listing.entries.some((e) => e.name === "IOPAR"));
      // The same listing again comes from the device, not the calculator.
      assert.equal(await device.list(["HOME"]), listing);

      assert.equal(await device.write(["HOME"], "PRG", PROGRAM), "PRG");
      assert.equal(await device.write(["HOME"], "NOTE", NOTE, { asString: true }), "NOTE");
      assert.equal(await device.write(["HOME"], ARROW, NOTE, { asString: true }), ARROW);
      const prg = await device.read(["HOME"], "PRG");
      assert.equal(prg.fileName, "PRG.hp");
      assert.equal(new TextDecoder().decode(prg.data.slice(0, 7)), header);
      // Its preview, decoded here: a program has no text without the ROM.
      assert.equal(pkg.describe_file(prg.data).type, "program");
      assert.equal(await device.write(["HOME"], "PRG2", prg.data), "PRG2");
      const text = await device.read(["HOME"], "PRG2", { text: true });
      assert.equal(text.fileName, "PRG2.txt");
      assert.match(pkg.text_of(text.data), /«\s*1 2 \+\s*»/);
      const note = await device.read(["HOME"], "NOTE");
      const described = pkg.describe_file(note.data);
      assert.equal(described.type, "string");
      assert.equal(described.value, "hello, calculator");
      const arrow = await device.read(["HOME"], ARROW);
      assert.equal(arrow.fileName, "A\u2192B.hp");

      await assert.rejects(device.write(["HOME"], "PRG", PROGRAM), (err) => err.kind === "nameClash" && err.varName === "PRG");
      assert.equal(await device.write(["HOME"], "PRG", prg.data, { replace: true }), "PRG");
      const again = await device.list(["HOME"]);
      assert.notEqual(again, listing, "a put reads the directory again");
      for (const n of ["PRG", "PRG2", "NOTE", ARROW]) assert.ok(again.entries.some((e) => e.name === n), n);
      await device.disconnect();
      assert.equal(link.failure, null);
      assert.equal(link.next, link.steps.length, "every step of the trace was played");
      assert.ok(busy.includes("Looking for the calculator…") && busy.includes("Getting PRG…") && busy.includes("Putting PRG…"));
    } finally {
      await device.close();
    }
  });
}

test("a link that is lost fails the running request", { timeout: 30_000 }, async (t) => {
  const pkg = await load();
  if (!pkg) {
    t.skip("web/pkg not built (just web)");
    return;
  }
  // Only the first probe: no answer comes, then the cable goes.
  const link = new FakeLink("       0 = 9600 none\n");
  link.write = async () => {};
  const client = new ProtocolClient(link, { worker: () => inProcessWorker(pkg.TxDevice), seed: SEED });
  const device = new KermitDevice(client);
  const lost = [];
  device.addEventListener("lost", (e) => lost.push(e.detail));
  const connecting = device.connect();
  await new Promise((r) => setTimeout(r, 50));
  link.emitLost("unplugged");
  await assert.rejects(connecting, (err) => /unplugged|link/.test(err.message));
  assert.deepEqual(lost, ["unplugged"]);
  await device.close();
});

test("the search names every speed it listened at when nothing answers", { timeout: 30_000 }, async (t) => {
  const pkg = await load();
  if (!pkg) {
    t.skip("web/pkg not built (just web)");
    return;
  }
  // A link that takes every open and write and never answers.
  const link = new FakeLink([]);
  link.open = async () => {};
  link.write = async () => {};
  const client = new ProtocolClient(link, { worker: () => inProcessWorker(pkg.TxDevice), seed: SEED, listenMs: 40 });
  const device = new KermitDevice(client);
  const heard = [];
  device.addEventListener("search", (e) => heard.push(e.detail));
  await assert.rejects(device.connect(), (err) => err.kind === "noAnswer");
  assert.deepEqual(heard.filter((e) => e.type === "listening").map((e) => e.settings.baud), [9600, 4800, 2400, 1200]);
  await device.close();
});

test("a calculator that goes silent fails the request as notAnswering, and the device waits for a connect", { timeout: 60_000 }, async (t) => {
  const pkg = await load();
  if (!pkg) {
    t.skip("web/pkg not built (just web)");
    return;
  }
  // The 48SX's recorded session up to the connect and the listing, then a calculator that takes every write and says nothing.
  const link = new FakeLink(readFileSync(new URL("48sx-session.trace", TRACES), "utf8"));
  const client = new ProtocolClient(link, { worker: () => inProcessWorker(pkg.TxDevice), seed: SEED, silenceMs: 300 });
  const device = new KermitDevice(client);
  const said = [];
  device.addEventListener("notAnswering", () => said.push("notAnswering"));
  try {
    await device.connect();
    await device.list(["HOME"]);
    link.write = async () => {};
    const started = performance.now();
    await assert.rejects(device.read(["HOME"], "IOPAR"), (err) => err.kind === "notAnswering");
    assert.ok(performance.now() - started < 5_000, "within the silence, not after the Kermit retries");
    assert.deepEqual(said, ["notAnswering"]);
    assert.equal(device.notAnswering, true);
    assert.equal(client.notAnswering, true, "the device's state posted with the connection");
    assert.equal(device.connection, null);
    // Every request but a connect is refused at once.
    await assert.rejects(device.list(["HOME", "X"]), (err) => err.kind === "notAnswering");
  } finally {
    await device.close();
  }
});

test("the palmtop trace's session through the page's protocol stack", { timeout: 60_000 }, async (t) => {
  const pkg = await load();
  if (!pkg) {
    t.skip("web/pkg not built (just web)");
    return;
  }
  // crates/saturnus-tx/src/palmtop_tests.rs `session_requests`, in its order, against its recorded Filer server.
  const link = new FakeLink(readFileSync(new URL("100lx-session.trace", TRACES), "utf8"));
  const client = new ProtocolClient(link, { worker: () => inProcessWorker(pkg.TxPalmtop), kind: "palmtop" });
  const data = new Uint8Array([0x10, 0x16, 0x03, ...new TextEncoder().encode(" hello, palmtop\r\n")]);
  const C = ["C:"];
  const put = (replace) => client.request({ type: "put", dir: C, name: "NEW.TXT", replace }, data);
  try {
    const conn = await client.request({ type: "connect", baud: 19200 });
    assert.deepEqual(conn.connection, { model: "palmtop", settings: { baud: 19200, parity: "none", dataBits: 8 }, revision: 2 });
    assert.deepEqual(client.connection, conn.connection);
    const drives = await client.request({ type: "list", dir: [] });
    assert.deepEqual(drives.entries.map((e) => [e.name, e.drive, e.directory]), [["A:", true, true], ["C:", true, true], ["D:", true, true]]);
    const c = await client.request({ type: "list", dir: C });
    assert.deepEqual(c.entries.map((e) => [e.name, e.size, e.directory]), [["BIG.BIN", 5000, false], ["HELLO.TXT", 7, false], ["SUB", 0, true]]);
    assert.deepEqual(c.entries[1].modified, { year: 2026, month: 1, day: 1, hour: 12, minute: 0, second: 0 });
    const hello = await client.request({ type: "get", dir: C, name: "HELLO.TXT" });
    assert.equal(new TextDecoder().decode(hello.data), "Hello\r\n");
    assert.equal((await put(false)).name, "NEW.TXT");
    await assert.rejects(put(false), (err) => err.kind === "exists" && err.varName === "NEW.TXT");
    assert.equal((await put(true)).name, "NEW.TXT");
    assert.deepEqual(await client.request({ type: "rename", dir: C, from: "NEW.TXT", to: "NOTES.TXT" }), { type: "renamed", from: "NEW.TXT", to: "NOTES.TXT", data: undefined });
    assert.equal((await client.request({ type: "mkdir", dir: C, name: "DOCS" })).name, "DOCS");
    assert.equal((await client.request({ type: "remove", dir: C, name: "SUB", directory: true })).count, 3);
    const after = await client.request({ type: "list", dir: C });
    assert.deepEqual(after.entries.map((e) => e.name), ["BIG.BIN", "DOCS", "HELLO.TXT", "NOTES.TXT"]);
    await client.request({ type: "disconnect" });
    assert.equal(client.connection, null);
    assert.equal(link.failure, null);
    assert.equal(link.next, link.steps.length, "every step of the trace was played");
  } finally {
    await client.close();
  }
});
