// The transfer page's wasm bindings (crates/saturnus-tx-web, built into
// web/tx/pkg by `just tx-web`; skipped without it), driven directly from
// JavaScript on a fake clock: the ops session of saturnus-tx's recorded
// traces (crates/saturnus-tx/traces/MODEL-ops.trace, recorded from the
// emulated 48SX, 48GX and 49G; the session is crates/saturnus-tx/tests/
// scenario `ops`) through `TxDevice`, every byte the device writes the
// trace's, and the pure functions: the copy plan, the header check, the
// file names. No browser, no Worker, no page modules but the trace parser.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { parseTrace } from "../tx/trace.js";

const pkgFile = new URL("../tx/pkg/saturnus_tx_web_bg.wasm", import.meta.url);
const TRACES = new URL("../../crates/saturnus-tx/traces/", import.meta.url);
/** The traces' seed (crates/saturnus-tx/tests/scenario `SEED`). */
const SEED = 0x5a70037;
const enc = new TextEncoder();
const PROGRAM = enc.encode("%%HP: T(3)A(R)F(.);\r\n\\<< 1 2 + \\>>\r\n");
const NOTE = enc.encode("hello, calculator");
const ARROW = "A\u2192B";

let wasm = null;
async function load(t) {
  if (!existsSync(pkgFile)) {
    t.skip("web/tx/pkg not built (just tx-web)");
    return null;
  }
  wasm ??= import("../tx/pkg/saturnus_tx_web.js").then((pkg) => {
    pkg.initSync({ module: readFileSync(pkgFile) });
    return pkg;
  });
  return wasm;
}

const hex = (bytes) => Buffer.from(bytes).toString("hex");

/**
 * A trace's calculator on the caller's clock, as saturnus-tx's `Replay`:
 * each action must be the trace's next open or write; each read arrives
 * as long after the step before it as it did when recorded.
 */
class Replay {
  constructor(text) {
    this.steps = parseTrace(text).filter((s) => s.kind !== "note");
    this.next = 0;
    this.anchor = { at: 0, now: 0 };
  }
  get remaining() { return this.steps.length - this.next; }
  action(now, out) {
    const step = this.steps[this.next];
    const ok = step && (out.type === "open"
      ? step.kind === "open" && step.baud === out.baud && step.parity === out.parity
      : step.kind === "write" && hex(step.bytes) === hex(out.bytes));
    if (!ok) {
      const want = step ? `${step.kind} ${step.bytes ? Buffer.from(step.bytes).toString("latin1") : step.baud}` : "the end";
      const got = out.type === "open" ? `open ${out.baud}` : `write ${Buffer.from(out.bytes).toString("latin1")}`;
      throw new Error(`step ${this.next}: ${got}, expected ${want}`);
    }
    this.anchor = { at: step.at, now };
    this.next++;
  }
  nextInput() {
    const step = this.steps[this.next];
    return step?.kind === "read" ? this.anchor.now + (step.at - this.anchor.at) : undefined;
  }
  takeInput(now) {
    const due = this.nextInput();
    if (due === undefined || due > now) return null;
    return this.steps[this.next++].bytes;
  }
}

/** A TxDevice against a replay on a fake clock. */
class Host {
  constructor(pkg, replay) {
    this.pkg = pkg;
    this.replay = replay;
    this.now = 1000;
    this.device = new pkg.TxDevice(this.now, SEED);
    this.events = [];
  }
  fresh() {
    this.device.free();
    this.device = new this.pkg.TxDevice(this.now, SEED);
  }
  /** Run one request to its result. */
  run(request, data) {
    this.device.start(this.now, JSON.stringify(request), data);
    for (let i = 0; i < 1_000_000; i++) {
      for (const out of this.device.drain()) {
        if (out.type === "open" || out.type === "write") this.replay.action(this.now, out);
        else if (out.type === "result") return out;
        else this.events.push(out);
      }
      const times = [this.replay.nextInput(), this.device.deadline()].filter((t) => t !== undefined);
      assert.ok(times.length > 0, "the device waits for nothing");
      this.now = Math.max(this.now, Math.min(...times));
      const bytes = this.replay.takeInput(this.now);
      if (bytes) this.device.input(this.now, bytes);
      else this.device.timeout(this.now);
    }
    throw new Error(`${request.type}: no result (now ${this.now}, deadline ${this.device.deadline()}, next read ${this.replay.nextInput()}, step ${this.replay.next})`);
  }
  ok(request, data) {
    const r = this.run(request, data);
    assert.ok(r.ok, `${request.type}: ${JSON.stringify(r.error)}`);
    return r;
  }
  err(request, data) {
    const r = this.run(request, data);
    assert.ok(!r.ok, `${request.type}: ${JSON.stringify(r.outcome)}`);
    return r.error;
  }
}

/** A tree outcome's items as the next put-tree's items. */
const asItems = (outcome) => outcome.items.map((i) => (i.directory ? { path: i.fsPath } : { path: i.fsPath, data: i.data }));
const sameTree = (a, b) => {
  assert.deepEqual(a.items.map((i) => i.path), b.items.map((i) => i.path));
  assert.deepEqual(a.items.map((i) => (i.data ? hex(i.data) : null)), b.items.map((i) => (i.data ? hex(i.data) : null)));
};

/** The ops session (scenario `ops`) through the bindings. */
function opsSession(h, model) {
  const header = model === "49g" ? "HPHP49-" : "HPHP48-";
  const connected = h.ok({ type: "connect" });
  assert.equal(connected.outcome.connection.model, model);
  assert.deepEqual(h.ok({ type: "mkdir", dir: ["HOME"], name: "EMPTY" }).outcome, { type: "created", name: "EMPTY" });
  assert.equal(h.err({ type: "mkdir", dir: ["HOME"], name: "EMPTY" }).kind, "nameClash");

  // File-system names: a file's extension goes, a folder keeps its name.
  const first = [
    { path: ["PRG.txt"], data: PROGRAM },
    { path: ["NOTE.txt"], data: NOTE },
    { path: ["INNER"] },
    { path: ["INNER", `${ARROW}.txt`], data: NOTE },
  ];
  const stored = h.ok({ type: "put-tree", dir: ["HOME"], name: "D1", asString: true }, first);
  assert.deepEqual(stored.outcome, { type: "treeStored", directories: 2, variables: 3 });
  const got = h.ok({ type: "get-tree", dir: ["HOME", "D1"], format: "binary" }).outcome;
  assert.equal(got.type, "tree");
  assert.deepEqual(got.items.map((i) => i.path.join("/")), ["PRG", "NOTE", "INNER", `INNER/${ARROW}`]);
  assert.deepEqual(got.items.map((i) => i.fsPath.join("/")), ["PRG.hp", "NOTE.hp", "INNER", `INNER/${ARROW}.hp`]);
  for (const i of got.items.filter((i) => !i.directory)) {
    assert.ok(i.data instanceof Uint8Array);
    assert.equal(Buffer.from(i.data.subarray(0, 7)).toString("latin1"), header);
  }
  assert.ok(h.events.some((e) => e.type === "item" && e.path.join("/") === "HOME/D1/INNER"), "item events");

  // Purge: the directory's count first, then the purge with it.
  const notEmpty = h.err({ type: "purge", dir: ["HOME"], name: "D1" });
  assert.equal(notEmpty.kind, "notEmpty");
  assert.equal(notEmpty.count, 4);
  assert.deepEqual(h.ok({ type: "purge", dir: ["HOME"], name: "D1", count: notEmpty.count }).outcome, { type: "purged", name: "D1", count: 4 });
  h.ok({ type: "put-tree", dir: ["HOME"], name: "D1", asString: true }, asItems(got));
  sameTree(h.ok({ type: "get-tree", dir: ["HOME", "D1"], format: "binary" }).outcome, got);

  assert.deepEqual(h.ok({ type: "rename", dir: ["HOME"], from: "D1", to: "D2" }).outcome, { type: "renamed", from: "D1", to: "D2" });
  assert.equal(h.err({ type: "rename", dir: ["HOME"], from: "EMPTY", to: "D2" }).kind, "nameClash");
  assert.equal(h.err({ type: "rename", dir: ["HOME"], from: "NOPE", to: "X" }).kind, "notFound");
  assert.equal(h.ok({ type: "purge", dir: ["HOME"], name: "EMPTY" }).outcome.count, 0);

  const intoHome = [{ path: ["D2"] }, { path: ["D2", "PRG.txt"], data: PROGRAM }];
  const inUse = h.err({ type: "put-tree", dir: ["HOME"] }, intoHome);
  assert.equal(inUse.kind, "namesInUse");
  assert.deepEqual(inUse.names, ["D2/PRG"]);
  assert.deepEqual(h.ok({ type: "put-tree", dir: ["HOME"], replace: true }, intoHome).outcome, { type: "treeStored", directories: 0, variables: 1 });
  const d2 = h.ok({ type: "get-tree", dir: ["HOME", "D2"], format: "binary" }).outcome;
  sameTree(d2, got);

  const pict = h.ok({ type: "pict" });
  assert.equal(pict.outcome.type, "pict");
  assert.equal(Buffer.from(pict.data.subarray(0, 7)).toString("latin1"), header);
  // A fresh calculator's PICT is empty (0 x 0).
  assert.equal(h.pkg.describe_file(pict.data).kind, "Graphic");
  return d2;
}

/** The backup part: memory, backup, restore on a fresh calculator. */
function backupSession(h, model, d2) {
  const memory = h.ok({ type: "memory" }).outcome;
  assert.equal(memory.type, "memory");
  assert.ok(memory.fits && memory.free >= memory.needed && memory.needed === 2 * memory.home + 1024, JSON.stringify(memory));
  const before = h.ok({ type: "list", dir: ["HOME"] }).outcome.entries;
  const backup = h.ok({ type: "backup" });
  assert.equal(backup.outcome.model, model);
  assert.ok(backup.data instanceof Uint8Array && backup.data.length > 100);
  h.ok({ type: "disconnect" });

  h.fresh();
  h.ok({ type: "connect" });
  const fresh = h.ok({ type: "list", dir: ["HOME"] }).outcome.entries;
  assert.ok(!fresh.some((e) => e.name === "D2"));
  assert.throws(() => h.device.start(h.now, JSON.stringify({ type: "restore" }), undefined), /bytes/);
  const restored = h.ok({ type: "restore" }, backup.data);
  assert.equal(restored.outcome.type, "restored");
  assert.match(restored.outcome.message, /restarted.*reconnect/);
  assert.equal(h.device.restarted(), true);
  assert.equal(h.device.connection(), null);
  // Nothing goes out until a connect.
  assert.equal(h.err({ type: "list", dir: ["HOME"] }).kind, "restarted");
  h.ok({ type: "connect" });
  assert.equal(h.device.restarted(), false);
  const after = h.ok({ type: "list", dir: ["HOME"] }).outcome.entries;
  assert.deepEqual(after, before, "the same HOME: names, sizes, checksums");
  sameTree(h.ok({ type: "get-tree", dir: ["HOME", "D2"], format: "binary" }).outcome, d2);
  h.ok({ type: "disconnect" });
}

for (const model of ["48sx", "48gx", "49g"]) {
  test(`the ${model} ops trace through TxDevice`, { timeout: 120_000 }, async (t) => {
    const pkg = await load(t);
    if (!pkg) return;
    const replay = new Replay(readFileSync(new URL(`${model}-ops.trace`, TRACES), "utf8"));
    const h = new Host(pkg, replay);
    const d2 = opsSession(h, model);
    // The 49G's trace stops before its backup (it holds ROM bytes).
    if (model !== "49g") backupSession(h, model, d2);
    assert.equal(replay.remaining, 0, "trace left over");
  });
}

test("malformed requests throw before anything is sent", async (t) => {
  const pkg = await load(t);
  if (!pkg) return;
  const d = new pkg.TxDevice(0, SEED);
  const start = (req, data) => () => d.start(0, JSON.stringify(req), data);
  assert.throws(start({ type: "put-tree", dir: ["HOME"] }), /items/);
  assert.throws(start({ type: "put-tree" }, [{ path: "X" }]), /list of names/);
  assert.throws(start({ type: "put-tree" }, [{ path: ["X.hp"], data: [1, 2] }]), /Uint8Array/);
  assert.throws(start({ type: "put-tree" }, [{ path: ["%FF.hp"], data: new Uint8Array([1]) }]), /%FF/);
  assert.throws(start({ type: "restore" }), /bytes/);
  assert.throws(start({ type: "purge", name: "D", count: "all" }), /count/);
  assert.throws(start({ type: "rename", from: "A" }), /to is needed/);
  assert.throws(start({ type: "put", name: "A" }, 5), /Uint8Array/);
  assert.equal(d.isBusy(), false);
  assert.equal(d.restarted(), false);
  // Not connected: refused by the device, nothing written.
  d.start(0, JSON.stringify({ type: "backup" }), undefined);
  const out = d.drain();
  assert.deepEqual(out.map((o) => o.type), ["result"]);
  assert.equal(out[0].error.kind, "notConnected");
});

test("the copy plan warns about the other family and copies text", async (t) => {
  const pkg = await load(t);
  if (!pkg) return;
  const plan = pkg.copy_plan(JSON.stringify({
    from: "48gx",
    to: "49g",
    fromDir: ["HOME", "G"],
    selection: [{ name: "PRG", kind: "Program" }, { name: "D", kind: "Directory" }],
    target: [{ name: "PRG", kind: "String" }],
  }));
  assert.deepEqual(plan.inUse, ["PRG"]);
  assert.equal(plan.mismatch.from, "48");
  assert.equal(plan.mismatch.to, "49");
  assert.match(plan.mismatch.message, /HP 48 objects.*as text/);
  assert.deepEqual(plan.steps[0].get, { type: "get", dir: ["HOME", "G"], name: "PRG", format: "text" });
  const { skip, ...tree } = plan.steps[1].get;
  assert.deepEqual(tree, { type: "get-tree", dir: ["HOME", "G", "D"], format: "text" });
  assert.ok(skip.includes("Library") && !skip.includes("Integer"));
  const same = pkg.copy_plan(JSON.stringify({ from: "48sx", to: "48gx", selection: [{ name: "X", kind: "Real Number" }] }));
  assert.equal(same.mismatch, null);
  assert.equal(same.steps[0].format, "binary");
  // No text form: left out; a directory on either side: blocked.
  const down = pkg.copy_plan(JSON.stringify({
    from: "49g",
    to: "48gx",
    selection: [{ name: "L", kind: "Library" }, { name: "N", kind: "Integer" }, { name: "D", kind: "Directory" }],
    target: [{ name: "D", kind: "Directory" }],
  }));
  assert.deepEqual(down.untranslatable, ["L", "N"]);
  assert.deepEqual(down.blocked, ["D"]);
  assert.ok(down.steps[0].get.skip.includes("Integer"));
  const gx = pkg.copy_plan(JSON.stringify({ from: "48gx", to: "48sx", selection: [{ name: "P", kind: "Program" }] }));
  assert.equal(gx.steps[0].format, "text");
  assert.match(gx.warning, /48G\/GX/);
  const folder = pkg.copy_plan(JSON.stringify({ from: null, to: "49g", selection: [] }));
  assert.equal(folder.mismatch, null);
  assert.throws(() => pkg.copy_plan(JSON.stringify({ to: "42s" })), /42s/);

  // One file's header: a 48 file on a 49G is stored as a string.
  const f48 = enc.encode("HPHP48-R\x2c\x2a");
  const warn = pkg.header_check(f48, "49g");
  assert.equal(warn.from, "48");
  assert.match(warn.message, /string/);
  assert.equal(pkg.header_check(f48, "48sx"), null);
  assert.equal(pkg.header_check(PROGRAM, "49g"), null);
  assert.throws(() => pkg.header_check(f48, "38g"), /38g/);
});

test("folder, directory and backup names", async (t) => {
  const pkg = await load(t);
  if (!pkg) return;
  assert.equal(pkg.folder_name("A/B"), "A%2FB");
  assert.equal(pkg.dir_name("A%2FB.C"), "A/B.C");
  assert.throws(() => pkg.dir_name("%FF"));
  assert.equal(pkg.backup_file_name("48gx", 2026, 10, 10, 14, 32), "48GX-backup-2026-10-10-1432.hp");
  assert.equal(pkg.backup_file_name("49g", 2026, 1, 2, 3, 4), "49G-backup-2026-01-02-0304.hp");
  assert.equal(pkg.backup_file_name("48sx", 2026, 12, 31, 23, 59), "48SX-backup-2026-12-31-2359.hp");
  assert.throws(() => pkg.backup_file_name("hp", 2026, 1, 1, 0, 0));
});
