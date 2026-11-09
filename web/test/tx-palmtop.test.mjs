// Files' HP 100LX/200LX palmtop in Node, without a browser: the page's
// PalmtopFolder (web/tx/palmtop.js) over the protocol stack (protocol.js,
// the driver of the protocol Worker run in-process, the wasm's TxPalmtop)
// against filer-proto's server over its in-memory files (the wasm's
// TxFilerServer on a link of its own, web/test/tx-fake-palmtop.js): the
// drives, listings with DOS dates and attributes, get and put byte for
// byte, 8.3 names (a longer one refused before anything is sent), the
// question before a replace, rename, new directory, a tree deleted with
// its count, a tree copied in from a folder, Stop part way, a palmtop
// that stops answering and the reconnect. Skipped without web/pkg (`just
// web`). The timers run on a quick clock: a wait the protocol arms (5.9 s
// for a reply) passes in a millisecond, the server's replies come first.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { DeviceDriver } from "../tx/driver.js";
import { MemoryDirectory } from "../tx/memory-handle.js";
import { Folder } from "../tx/folder.js";
import { PalmtopFolder, dosTime, entryKind, noAnswerText } from "../tx/palmtop.js";
import { ProtocolClient } from "../tx/protocol.js";
import { JobQueue } from "../tx/queue.js";
import { Transfers } from "../tx/transfers.js";
import { palmtopLink } from "./tx-fake-palmtop.js";

const pkgFile = new URL("../pkg/saturnus_web_bg.wasm", import.meta.url);
let wasm = null;
async function load() {
  if (!existsSync(pkgFile)) return null;
  wasm ??= import("../pkg/saturnus_web.js").then((pkg) => {
    pkg.initSync({ module: readFileSync(pkgFile) });
    return pkg;
  });
  return wasm;
}

/**
 * The protocol Worker in this process, on a clock that jumps over the
 * protocol's waits: one longer than 100 ms passes after 40 ms, long after
 * the server's replies (1 ms) have come.
 */
function quickWorker(TxPalmtop) {
  let skew = 0;
  const w = {
    onmessage: null,
    terminated: false,
    driver: null,
    postMessage(m) {
      setTimeout(() => {
        if (w.terminated) return;
        if (m.cmd === "init") {
          w.driver = new DeviceDriver({
            TxDevice: TxPalmtop,
            now: () => performance.now() + skew,
            setTimer: (fn, ms) => (ms <= 100 ? setTimeout(fn, ms) : setTimeout(() => {
              skew += ms - 40;
              fn();
            }, 40)),
            clearTimer: (t) => clearTimeout(t),
            post: (msg) => setTimeout(() => !w.terminated && w.onmessage?.({ data: msg }), 0),
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

/** A connected palmtop: `{pkg, link, client, palmtop}`, or null (skipped). */
async function connected(t, opts = {}) {
  const pkg = await load();
  if (!pkg) {
    t.skip("web/pkg not built (just web)");
    return null;
  }
  const link = palmtopLink(pkg.TxFilerServer, undefined, opts);
  const client = new ProtocolClient(link, { worker: () => quickWorker(pkg.TxPalmtop), kind: "palmtop" });
  const palmtop = new PalmtopFolder(client, { helpers: async () => pkg });
  t.after(() => palmtop.close());
  const conn = await palmtop.connect({ baud: opts.baud ?? 19200 });
  return { pkg, link, client, palmtop, conn };
}

const enc = new TextEncoder();
const dec = new TextDecoder();
const names = (listing) => listing.entries.map((e) => e.name);

test("the words: DOS times, the Type column, the speed to fall back to", () => {
  assert.equal(dosTime({ year: 2026, month: 11, day: 9, hour: 13, minute: 5, second: 30 }), new Date(2026, 10, 9, 13, 5, 30).getTime());
  assert.equal(dosTime(null), null);
  assert.equal(entryKind({ drive: true, directory: true }), "Drive");
  assert.equal(entryKind({ directory: true }), "Folder");
  assert.equal(entryKind({ directory: false, readOnly: true, hidden: true }), "File, read-only, hidden");
  assert.match(noAnswerText(19200), /^No palmtop answered at 19200 baud\. Is Filer open in front, with server mode on/);
  assert.doesNotMatch(noAnswerText(19200), /19200 is the safe one/);
  assert.match(noAnswerText(57600), /The palmtop's speed must be 57600 too; 19200 is the safe one/);
});

test("drives, listings, get and put byte for byte, 8.3 names, rename, new directory, delete with the count", { timeout: 60_000 }, async (t) => {
  const r = await connected(t);
  if (!r) return;
  const { link, palmtop, conn } = r;
  assert.deepEqual(link.opened, [19200]);
  assert.deepEqual(conn.settings, { baud: 19200, parity: "none", dataBits: 8 });
  const top = await palmtop.list(["Palmtop"]);
  assert.deepEqual(top.entries.map((e) => [e.name, e.kind, e.directory]), [["A:", "Drive", true], ["C:", "Drive", true], ["D:", "Drive", true]]);
  const c = await palmtop.list(["Palmtop", "C:"]);
  assert.deepEqual(names(c), ["SUB", "BIG.BIN", "HELLO.TXT"], "directories first");
  const big = c.entries.find((e) => e.name === "BIG.BIN");
  assert.equal(big.size, 5000);
  assert.equal(big.kind, "File");
  assert.equal(big.modified, new Date(2026, 0, 1, 12, 0, 0).getTime());
  // Kept until something changes it.
  assert.equal(await palmtop.list(["Palmtop", "C:"]), c);

  const progress = [];
  palmtop.addEventListener("progress", (e) => progress.push(e.detail.done));
  const got = await palmtop.read(["Palmtop", "C:"], "BIG.BIN");
  assert.deepEqual([...got.data], [...link.server.file("C:\\BIG.BIN")]);
  assert.deepEqual(progress, [2048, 4096, 5000]);

  // A put makes an upper-case DOS name; a long one is refused before anything is sent.
  const bytes = Uint8Array.from({ length: 3000 }, (_, i) => (i * 7) % 256);
  assert.equal(await palmtop.write(["Palmtop", "C:"], "new.bin", bytes), "NEW.BIN");
  assert.deepEqual([...link.server.file("C:\\NEW.BIN")], [...bytes]);
  assert.deepEqual(await palmtop.existing(["Palmtop", "C:"], "new.bin"), { name: "NEW.BIN", directory: false, exact: true });
  await assert.rejects(palmtop.write(["Palmtop", "C:"], "NEW.BIN", bytes), (err) => err.kind === "exists" && err.message === "NEW.BIN is already there.");
  assert.equal(await palmtop.write(["Palmtop", "C:"], "NEW.BIN", enc.encode("two"), { replace: true }), "NEW.BIN");
  assert.equal(dec.decode(link.server.file("C:\\NEW.BIN")), "two");
  await assert.rejects(palmtop.write(["Palmtop", "C:"], "LONGNAME1.TXT", bytes), (err) => err.kind === "bad-name" && err.message === "LONGNAME1.TXT: the palmtop takes DOS names: up to 8 letters or digits, a dot and up to 3 more (like NOTES.TXT).");
  assert.equal(link.server.file("C:\\LONGNAME1.TXT"), undefined);
  assert.equal(await palmtop.nameProblem("MY FILE"), "The palmtop takes no spaces in a name.");
  // The ROM disk says why.
  await assert.rejects(palmtop.write(["Palmtop", "D:"], "X.TXT", bytes), (err) => err.kind === "readOnly" && /write-protected \(D: is the palmtop's ROM disk\)/.test(err.message));

  assert.equal(await palmtop.rename(["Palmtop", "C:"], "NEW.BIN", "notes.txt"), "NOTES.TXT");
  await assert.rejects(palmtop.rename(["Palmtop", "C:"], "NOTES.TXT", "HELLO.TXT"), (err) => err.kind === "exists");
  assert.equal(await palmtop.mkdir(["Palmtop", "C:"], "docs"), "DOCS");
  await assert.rejects(palmtop.mkdir(["Palmtop"], "X"), (err) => err.kind === "refused");
  await assert.rejects(palmtop.write(["Palmtop"], "X.TXT", bytes), (err) => err.kind === "refused" && err.message === "Files go on a drive: open C: or A: first.");
  assert.deepEqual(names(await palmtop.list(["Palmtop", "C:"])), ["DOCS", "SUB", "BIG.BIN", "HELLO.TXT", "NOTES.TXT"]);

  // A directory: its count first, refused without `recursive`, then all of it.
  assert.equal(await palmtop.count(["Palmtop", "C:"], "SUB"), 3);
  await assert.rejects(palmtop.purge(["Palmtop", "C:"], "SUB"), (err) => err.kind === "not-empty" && err.count === 3);
  assert.equal(await palmtop.purge(["Palmtop", "C:"], "SUB", { recursive: true }), 3);
  assert.ok(!link.server.paths().some((p) => p.startsWith("C:\\SUB")));
  assert.equal(await palmtop.purge(["Palmtop", "C:"], "HELLO.TXT"), 0);
  assert.deepEqual(names(await palmtop.list(["Palmtop", "C:"])), ["DOCS", "BIG.BIN", "NOTES.TXT"]);
});

test("copies with a folder: a tree in, a long name left out with the reason, a file out; a .zip of a directory", { timeout: 60_000 }, async (t) => {
  const r = await connected(t);
  if (!r) return;
  const { pkg, link, palmtop } = r;
  const queue = new JobQueue();
  const said = [];
  const transfers = new Transfers({ queue, helpers: async () => pkg, ask: async () => true, askName: async () => null, say: (text) => said.push(text) });
  const root = new MemoryDirectory("hp-files");
  const folder = new Folder(root, { kind: "picked", title: "hp-files" });
  await folder.mkdir(["hp-files"], "TREE");
  await folder.write(["hp-files", "TREE"], "a.txt", enc.encode("A"));
  await folder.mkdir(["hp-files", "TREE"], "IN");
  await folder.write(["hp-files", "TREE", "IN"], "b.txt", enc.encode("B"));
  await folder.write(["hp-files"], "a long name.txt", enc.encode("x"));
  const from = { id: "f", kind: "folder", folder, title: "hp-files" };
  const to = { id: "p", kind: "folder", palmtop: true, folder: palmtop, device: palmtop, title: "the palmtop" };
  const items = (await folder.list(["hp-files"])).entries.map((e) => ({
    name: e.name, directory: e.directory, size: e.size,
    read: async () => (await folder.read(["hp-files"], e.name)).data,
    children: async () => {
      const walk = async (path) => (await folder.list(path)).entries.map((x) => ({
        name: x.name, directory: x.directory, size: x.size,
        read: async () => (await folder.read(path, x.name)).data,
        children: () => walk([...path, x.name]),
      }));
      return walk(["hp-files", e.name]);
    },
  }));
  const plan = await transfers.plan({ from, fromPath: ["hp-files"], items, to, toPath: ["Palmtop", "C:"] });
  assert.deepEqual(plan.steps.map((s) => s.name), ["TREE"]);
  assert.deepEqual(plan.sections.map((s) => s.items), [["a long name.txt"]]);
  assert.match(plan.sections[0].text, /^Left out: the palmtop takes DOS names only/);
  const result = await transfers.copy({ from, fromPath: ["hp-files"], items, to, toPath: ["Palmtop", "C:"] });
  assert.deepEqual(result, { copied: ["TREE"], failed: [], skipped: [] });
  assert.equal(dec.decode(link.server.file("C:\\TREE\\A.TXT")), "A");
  assert.equal(dec.decode(link.server.file("C:\\TREE\\IN\\B.TXT")), "B");
  // And out again: a file to the folder.
  const back = (await palmtop.list(["Palmtop", "C:"])).entries.filter((e) => e.name === "HELLO.TXT").map((e) => ({
    name: e.name, directory: false, size: e.size, read: async () => (await palmtop.read(["Palmtop", "C:"], e.name)).data, children: async () => [],
  }));
  await transfers.copy({ from: to, fromPath: ["Palmtop", "C:"], items: back, to: { ...from, id: "f2" }, toPath: ["hp-files"] });
  assert.equal(dec.decode((await folder.read(["hp-files"], "HELLO.TXT")).data), "Hello, palmtop\r\n");
  // A directory as a .zip: its files' bytes, no drive colon in the name.
  const zip = await palmtop.zip(["Palmtop", "C:"], ["TREE"]);
  assert.equal(zip.name, "TREE.zip");
  const all = await palmtop.files(["Palmtop", "C:"], ["TREE"]);
  assert.deepEqual(all.map((e) => e.name), ["TREE", "TREE/IN", "TREE/IN/B.TXT", "TREE/A.TXT"]);
  assert.equal((await palmtop.zip(["Palmtop", "C:"])).name, "C.zip");
});

test("Stop ends a copy at the palmtop's next reply: the file is closed, the part written deleted, the session goes on", { timeout: 60_000 }, async (t) => {
  const r = await connected(t);
  if (!r) return;
  const { link, palmtop } = r;
  const bytes = new Uint8Array(20_000).fill(7);
  const writing = palmtop.write(["Palmtop", "C:"], "PART.BIN", bytes);
  await new Promise((resolve) => palmtop.addEventListener("progress", resolve, { once: true }));
  palmtop.stop();
  await assert.rejects(writing, (err) => err.kind === "cancelled" && err.message === "Stopped.");
  assert.equal(link.server.file("C:\\PART.BIN"), undefined, "the part written is gone");
  const reading = palmtop.read(["Palmtop", "C:"], "BIG.BIN");
  await new Promise((resolve) => palmtop.addEventListener("progress", resolve, { once: true }));
  palmtop.stop();
  await assert.rejects(reading, (err) => err.kind === "cancelled");
  assert.ok(palmtop.connection, "still connected");
  assert.equal(dec.decode((await palmtop.read(["Palmtop", "C:"], "HELLO.TXT")).data), "Hello, palmtop\r\n");
});

test("a palmtop that stops answering: notAnswering, no connection, the queue dropped, then a reconnect", { timeout: 60_000 }, async (t) => {
  const r = await connected(t);
  if (!r) return;
  const { link, palmtop } = r;
  const said = [];
  palmtop.addEventListener("notAnswering", () => said.push("notAnswering"));
  link.silence();
  const first = palmtop.list(["Palmtop", "C:"]);
  const waiting = palmtop.read(["Palmtop", "C:"], "HELLO.TXT");
  await assert.rejects(first, (err) => err.kind === "notAnswering" && /stopped answering/.test(err.message));
  await assert.rejects(waiting, (err) => err.kind === "cancelled");
  assert.deepEqual(said, ["notAnswering"]);
  assert.equal(palmtop.notAnswering, true);
  assert.equal(palmtop.connection, null);
  link.answer();
  await palmtop.connect({ baud: 19200 });
  assert.equal(palmtop.notAnswering, false);
  assert.deepEqual(names(await palmtop.list(["Palmtop", "C:"])), ["SUB", "BIG.BIN", "HELLO.TXT"]);
});

test("nothing answering a connect says what to check, and 19200 after a faster speed", { timeout: 60_000 }, async (t) => {
  const pkg = await load();
  if (!pkg) {
    t.skip("web/pkg not built (just web)");
    return;
  }
  const link = palmtopLink(pkg.TxFilerServer);
  link.silence();
  const client = new ProtocolClient(link, { worker: () => quickWorker(pkg.TxPalmtop), kind: "palmtop" });
  const palmtop = new PalmtopFolder(client, { helpers: async () => pkg });
  t.after(() => palmtop.close());
  await assert.rejects(palmtop.connect({ baud: 57600 }), (err) => err.kind === "noAnswer" && err.message === noAnswerText(57600));
  assert.deepEqual(link.opened, [57600]);
  assert.equal(palmtop.notAnswering, false);
});
