// "Serve a folder to a 38G, 39G or 40G" without a calculator: the words
// (serve-flows.js), the Worker's driver with the real wasm server and a
// fake clock (aplet-driver.js), the page's session over a fake link and
// a folder in memory (serve.js), replaying the 38G's recorded session
// (crates/saturnus-tx/traces/38g-aplets.trace). Needs web/tx/pkg (just
// tx-web) for the parts with the wasm server.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { eventLine, fileLabel, progressLine, serveStatus, stepsFor, transferFailedText, wireFailedText } from "../tx/serve-flows.js";
import { ApletDriver } from "../tx/aplet-driver.js";
import { HEAD_BYTES, ServeSession, folderFiles, isDirFile, rgbOf } from "../tx/serve.js";
import { Folder } from "../tx/folder.js";
import { MemoryDirectory } from "../tx/memory-handle.js";
import { Link } from "../tx/links.js";
import { parseTrace } from "../tx/trace.js";

const WEB = fileURLToPath(new URL("..", import.meta.url));
const ROOT = join(WEB, "..");
const PKG = join(WEB, "tx", "pkg", "saturnus_tx_web_bg.wasm");

async function wasm() {
  const mod = await import("../tx/pkg/saturnus_tx_web.js");
  mod.initSync({ module: readFileSync(PKG) });
  return mod;
}

test("the words: steps per model, lines, Transfer Failed explained", () => {
  assert.equal(stepsFor("38g").library, "LIB");
  assert.equal(stepsFor("40g").library, "APLET");
  assert.equal(stepsFor("38g").wire, null);
  assert.ok(stepsFor("39g").wire.join(" ").includes("HP39/40 (Wire)"));
  assert.equal(fileLabel("FUNCTION.000", "Function"), "Function (FUNCTION.000)");
  assert.equal(fileLabel("A.000", null), "A.000");
  assert.deepEqual(eventLine({ type: "received", name: "FUNCTION.000", title: "Function", size: 589 }), { kind: "in", text: "Received Function (FUNCTION.000), 589 bytes." });
  assert.equal(eventLine({ type: "sent", name: "HP38DIR.000", directory: true }), null);
  assert.deepEqual(eventLine({ type: "sent", name: "L1000000.000", directory: false }), { kind: "out", text: "Sent L1000000.000." });
  assert.match(eventLine({ type: "model", model: "38g" }).text, /HP 38G/);
  assert.match(eventLine({ type: "received", name: "HP39DIR.000", directory: true }).text, /list of files/);
  assert.match(transferFailedText("Transfer Failed", "NOPE.000"), /NOPE\.000 is not in the folder/);
  assert.match(transferFailedText("Transfer Failed"), /stopped the transfer/);
  assert.match(eventLine({ type: "failed", message: "timed out: too many retries" }).text, /stopped answering/);
  assert.match(wireFailedText("timed out: too many retries"), /did not start receiving/);
  assert.match(wireFailedText("cancelled by the peer"), /Transfer Failed/);
  assert.match(eventLine({ type: "wireDone" }).text, /not a binary aplet/);
  assert.equal(progressLine({ type: "sending", name: "A", done: 100, total: 2048 }), "Sending A… 100 bytes of 2.0 KB");
  assert.equal(progressLine({ type: "model" }), null);
  assert.equal(serveStatus({ model: "39g" }), "Waiting for the HP 39G or 40G…");
  assert.equal(serveStatus({ wire: true }), "Waiting for the calculator to receive by wire…");
  assert.equal(serveStatus({ current: "Receiving X… 1 KB" }), "Receiving X… 1 KB");
  assert.ok(isDirFile("hp38dir.cur") && isDirFile("HP39DIR.000") && !isDirFile("HP40DIR.CUR"));
  assert.deepEqual(rgbOf("rgb(1, 2, 3)"), [1, 2, 3]);
});

test("the folder's files: heads, directory files whole, folders and big files left out", async () => {
  const folder = new Folder(new MemoryDirectory("f"), { kind: "memory", title: "f" });
  const big = new Uint8Array(600);
  big.set([...new TextEncoder().encode("HP39BinB")]);
  await folder.write(folder.home, "BIG.000", big);
  const dir = new TextEncoder().encode(`HP38AscA ${"B 5 A.0001 A".repeat(40)}`);
  await folder.write(folder.home, "HP38DIR.000", dir);
  await folder.mkdir(folder.home, "sub");
  const files = await folderFiles(folder);
  assert.deepEqual(files.map((f) => f.name).sort(), ["BIG.000", "HP38DIR.000"]);
  assert.equal(files.find((f) => f.name === "BIG.000").head.length, HEAD_BYTES);
  assert.equal(files.find((f) => f.name === "HP38DIR.000").head.length, dir.length);
});

test("aplet_info reads a file's header", { skip: !existsSync(PKG) && "web/tx/pkg not built (just tx-web)" }, async () => {
  const api = await wasm();
  assert.deepEqual(api.aplet_info(new TextEncoder().encode("HP39BinB\x03XYZ")), { family: "39", binary: true, kind: "B", title: "XYZ", wire: true });
  assert.equal(api.aplet_info(new TextEncoder().encode("hello")), null);
});

/** A link that plays a recorded calculator: each `<` once the page wrote the `>`s before it. */
class TraceLink extends Link {
  constructor(steps) {
    super();
    this.steps = steps;
    this.at = 0;
    this.written = [];
    this.opened = null;
  }
  async open(settings) {
    this.opened = settings;
    queueMicrotask(() => this.feed());
  }
  feed() {
    while (this.at < this.steps.length && this.steps[this.at].dir === "<") {
      const s = this.steps[this.at++];
      this.emitData(s.bytes);
    }
  }
  async write(bytes) {
    const s = this.steps[this.at];
    assert.equal(s?.dir, ">", `step ${this.at}: a write the trace does not have`);
    assert.deepEqual([...bytes], [...s.bytes], `step ${this.at}: the bytes written`);
    this.written.push(bytes);
    this.at++;
    queueMicrotask(() => this.feed());
  }
  async close() {}
}

/** The trace's Kermit steps (before the wire part): `{dir, bytes}`. */
function steps(text) {
  const out = [];
  for (const s of parseTrace(text)) {
    if (s.kind === "note" && s.text.startsWith("wire")) break;
    if (s.kind === "write" || s.kind === "read") out.push({ dir: s.kind === "write" ? ">" : "<", bytes: s.bytes });
  }
  return out;
}

/** The aplet Worker run in this process: the driver over the real wasm server, timers real. */
function inProcessWorker(api) {
  return () => {
    const w = { onmessage: null, onerror: null };
    let driver = null;
    w.postMessage = (m) => queueMicrotask(() => {
      if (m.cmd === "init") {
        driver = new ApletDriver({
          TxApletServer: api.TxApletServer,
          now: () => performance.now(),
          setTimer: (fn, ms) => setTimeout(fn, ms),
          clearTimer: (t) => clearTimeout(t),
          post: (msg) => queueMicrotask(() => w.onmessage?.({ data: msg })),
          files: m.files,
        });
        w.onmessage?.({ data: { type: "ready" } });
      } else driver?.handle(m);
    });
    w.terminate = () => { driver = null; };
    return w;
  };
}

test("a session replays the 38G's recording: L1 into the folder and back", { skip: !existsSync(PKG) && "web/tx/pkg not built (just tx-web)" }, async () => {
  const api = await wasm();
  const text = readFileSync(join(ROOT, "crates/saturnus-tx/traces/38g-aplets.trace"), "utf8");
  const link = new TraceLink(steps(text));
  const folder = new Folder(new MemoryDirectory("aplets"), { kind: "memory", title: "aplets" });
  const session = new ServeSession({ link, folder, worker: inProcessWorker(api) });
  const events = [];
  session.addEventListener("event", (e) => events.push(e.detail));
  const lost = [];
  session.addEventListener("lost", (e) => lost.push(e.detail));
  await session.start();
  assert.deepEqual(link.opened, { baud: 9600, parity: "none", dataBits: 8 });
  for (let i = 0; i < 200 && link.at < link.steps.length; i++) await new Promise((r) => setTimeout(r, 10));
  await session.io;
  assert.deepEqual(lost, []);
  assert.equal(link.at, link.steps.length, "the whole recording played");
  const types = events.map((e) => e.type);
  assert.equal(types[0], "model");
  assert.ok(types.includes("needFile"));
  const names = (await folder.list(folder.home)).entries.map((e) => e.name).sort();
  assert.deepEqual(names, ["HP38DIR.000", "L1000000.000"]);
  const l1 = (await folder.read(folder.home, "L1000000.000")).data;
  assert.equal(String.fromCharCode(...l1.slice(0, 11)), "HP38BinE\x02L1");
  assert.equal(new TextDecoder().decode((await folder.read(folder.home, "HP38DIR.000")).data), "HP38AscA E 12 L1000000.0002 L1");
  await session.stop();
});

/** A Worker that is ready at once and keeps what it was sent. */
function recordingWorker(sent) {
  return () => {
    const w = { onmessage: null, onerror: null, terminated: false };
    w.postMessage = (m) => {
      sent.push(m);
      if (m.cmd === "init") queueMicrotask(() => w.onmessage?.({ data: { type: "ready" } }));
    };
    w.terminate = () => { w.terminated = true; };
    sent.worker = w;
    return w;
  };
}

test("a session whose link does not open stops what it started", async () => {
  const folder = new Folder(new MemoryDirectory("f"), { kind: "memory", title: "f" });
  class BadLink extends Link {
    closed = false;
    async open() { throw new Error("the port is in use"); }
    async close() { this.closed = true; }
  }
  const link = new BadLink();
  const sent = [];
  const session = new ServeSession({ link, folder, worker: recordingWorker(sent) });
  await assert.rejects(session.start(), /in use/);
  assert.equal(sent.worker.terminated, true);
  assert.equal(link.closed, true);
  assert.equal(session.worker, null);
  // Bytes arriving later go nowhere.
  link.emitData(new Uint8Array([1]));
  assert.deepEqual(sent.map((m) => m.cmd), ["init"]);
});

test("a received file the folder refuses is forgotten by the server; a file asked for is answered by name", async () => {
  const folder = new Folder(new MemoryDirectory("f"), { kind: "memory", title: "f" });
  await folder.write(folder.home, "GAME.000", new TextEncoder().encode("HP39BinB\x04Game"));
  const link = new Link();
  const sent = [];
  const session = new ServeSession({ link, folder, worker: recordingWorker(sent) });
  const events = [];
  session.addEventListener("event", (e) => events.push(e.detail));
  const files = await session.start();
  assert.equal(files[0].head.length, 13, "a short file's head is all of it");
  session.receive({ type: "received", name: "CON.000", data: new Uint8Array([1, 2]), directory: false });
  session.receive({ type: "needFile", name: "GAME.000" });
  await session.io;
  assert.deepEqual(sent.find((m) => m.cmd === "forget"), { cmd: "forget", name: "CON.000" });
  assert.equal(events.find((e) => e.type === "storeFailed")?.name, "CON.000");
  const provide = sent.find((m) => m.cmd === "provide");
  assert.equal(provide.name, "GAME.000");
  assert.equal(new TextDecoder().decode(provide.data), "HP39BinB\x04Game");
  await session.stop();
});
