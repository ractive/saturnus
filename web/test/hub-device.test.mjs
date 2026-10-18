// HubDevice (web/shared/hub-device.js, iteration 47): the emulator's
// calculator as saturnus tx's device, against a fake hub that answers the
// memory commands from an in-memory tree as the engine would: the list
// from the tree (one memoryTree per memoryChanged), the change event, the
// mapping of the device calls to the commands, nameClash, the notEmpty
// count, the engine's refusals in tx's words, getTree and putTree with
// their progress, backup and its noRoom. The pages and the real engine
// together: web/test/tx-shared.test.mjs.

import assert from "node:assert/strict";
import { test } from "node:test";

import { HubDevice, MAX_STORE_BYTES, countAll, refusalText } from "../shared/hub-device.js";

const enc = (s) => new TextEncoder().encode(s);
const bin = (s) => enc(`HPHP48-R${s}`);

/** The page's name helpers as the transfer bindings give them, enough for the tests. */
const helpers = async () => ({
  file_name: (name, format) => `${name}.${format === "text" ? "txt" : "hp"}`,
  folder_name: (name) => name,
  var_name: (file) => {
    if (file.startsWith("!")) throw new Error("bad");
    return file.replace(/\.(hp|txt)$/i, "");
  },
  dir_name: (folder) => folder,
  file_kind: (data) => {
    const t = new TextDecoder().decode(data.slice(0, 8));
    if (t.startsWith("HPHP")) return "binary";
    if (t.startsWith("%%HP")) return "text";
    return "other";
  },
  name_problem: (name) => (/\s/.test(name) || !name ? "no spaces" : null),
});

/** A fake hub: the memory tree, the files' bytes by path, the commands seen; `refuse` makes the next write fail with the engine's words. */
class FakeHub extends EventTarget {
  constructor(tree = []) {
    super();
    this.tree = tree;
    this.path = ["HOME"];
    this.log = [];
    this.refuse = null;
    this.files = new Map();
  }
  dir(path) {
    let vars = this.tree;
    for (const n of path.slice(1)) vars = vars.find((v) => v.name === n).variables;
    return vars;
  }
  changed() {
    this.dispatchEvent(new CustomEvent("memoryChanged", { detail: { type: "memoryChanged" } }));
  }
  async request(cmd, args = {}) {
    this.log.push({ cmd, ...args });
    const write = ["storeFile", "fetchFile", "purge", "rename", "createDir", "backup"].includes(cmd);
    if (write && this.refuse) {
      const why = this.refuse;
      this.refuse = null;
      throw new Error(why);
    }
    switch (cmd) {
      case "hello": return { protocol: 1 };
      case "attach": return { model: args.model, attached: true };
      case "watchMemory": return { supported: true, reason: null };
      case "memoryTree": return { path: this.path, variables: structuredClone(this.tree) };
      case "fetchFile": {
        const data = this.files.get([...args.dir, args.name].join("/")) ?? bin(args.name);
        return { name: args.name, size: data.length, data: args.text ? enc(`%%HP: T(3); ${args.name}`) : data };
      }
      case "storeFile": {
        const vars = this.dir(args.dir);
        const i = vars.findIndex((v) => v.name === args.name);
        if (i >= 0 && args.replace !== true) throw new Error(`${args.name} already exists in { HOME }`);
        if (i >= 0) vars.splice(i, 1);
        vars.unshift({ name: args.name, type: "Program", size: args.data.length, checksum: 1 });
        this.files.set([...args.dir, args.name].join("/"), args.data);
        return { name: args.name };
      }
      case "createDir": this.dir(args.dir).unshift({ name: args.name, type: "Directory", size: 5, checksum: 0, variables: [] }); return {};
      case "rename": this.dir(args.dir).find((v) => v.name === args.name).name = args.to; return {};
      case "purge": {
        const vars = this.dir(args.dir);
        vars.splice(vars.findIndex((v) => v.name === args.name), 1);
        return {};
      }
      case "backup": return { size: 10, data: bin("backup") };
      default: throw new Error(`unknown command "${cmd}"`);
    }
  }
  async close() { this.closed = true; }
}

const TREE = () => [
  { name: "X", type: "Real Number", size: 10.5, checksum: 0x1a2b },
  { name: "D", type: "Directory", size: 40, checksum: 3, variables: [
    { name: "P", type: "Program", size: 20, checksum: 4 },
    { name: "E", type: "Directory", size: 5, checksum: 5, variables: [{ name: "S", type: "String", size: 8, checksum: 6 }] },
  ] },
];

async function connected(tree = TREE()) {
  const hub = new FakeHub(tree);
  const d = new HubDevice(hub, { helpers });
  await d.connect({ model: "48gx", rom: new Uint8Array(4), romName: "gx" });
  return { hub, d };
}

const count = (hub, cmd) => hub.log.filter((m) => m.cmd === cmd).length;

test("connect: hello, attach, the memory watched; nothing typed, no serial port", async () => {
  const { hub, d } = await connected();
  assert.deepEqual(hub.log.map((m) => m.cmd), ["hello", "attach", "watchMemory"]);
  assert.equal(hub.log[2].on, true);
  assert.deepEqual(d.connection, { model: "48gx", settings: null, hub: true });
  assert.equal(hub.log.some((m) => /^serial|run|probeServer|setSpeed/.test(m.cmd)), false);
});

test("a calculator without a user memory, or a memory that cannot be read, is refused", async () => {
  const hub = new FakeHub();
  hub.request = async (cmd, args) => (cmd === "attach" ? { model: "38g" } : FakeHub.prototype.request.call(hub, cmd, args));
  await assert.rejects(new HubDevice(hub, { helpers }).connect({ model: "38g", rom: new Uint8Array(1) }), /no user memory/);
  const off = new FakeHub();
  off.request = async (cmd, args) => (cmd === "watchMemory" ? { supported: false, reason: "the 49G's memory is in flash" } : FakeHub.prototype.request.call(off, cmd, args));
  await assert.rejects(new HubDevice(off, { helpers }).connect({ model: "49g", rom: new Uint8Array(1) }), /in flash/);
});

test("the list comes from the tree, read once per memoryChanged, which fires change", async () => {
  const { hub, d } = await connected();
  const home = await d.list(["HOME"]);
  assert.deepEqual(home.entries, [
    { name: "X", kind: "Real Number", size: 10.5, checksum: 0x1a2b, directory: false },
    { name: "D", kind: "Directory", size: 40, checksum: 3, directory: true },
  ]);
  assert.equal(home.free, null, "no free figure");
  assert.deepEqual((await d.list(["HOME", "D", "E"])).entries.map((e) => e.name), ["S"]);
  assert.equal(count(hub, "memoryTree"), 1, "one tree for both listings");
  const changes = [];
  d.addEventListener("change", (e) => changes.push(e.detail.path));
  hub.tree.unshift({ name: "NEW", type: "Real Number", size: 10.5, checksum: 9 });
  hub.changed();
  assert.deepEqual(changes, [null]);
  assert.equal((await d.list(["HOME"])).entries[0].name, "NEW", "typed in the emulator, shown without a refresh");
  assert.equal(count(hub, "memoryTree"), 2);
  await assert.rejects(d.list(["HOME", "NOPE"]), (err) => err.kind === "notFound");
});

test("the device calls map to the memory commands", async () => {
  const { hub, d } = await connected();
  const got = await d.read(["HOME", "D"], "P");
  assert.deepEqual([got.name, got.fileName, got.format], ["P", "P.hp", "binary"]);
  assert.deepEqual(hub.log.at(-1), { cmd: "fetchFile", dir: ["HOME", "D"], name: "P" });
  const text = await d.read(["HOME"], "X", { text: true });
  assert.deepEqual([text.fileName, text.format, new TextDecoder().decode(text.data).slice(0, 4)], ["X.txt", "text", "%%HP"]);
  assert.equal(hub.log.at(-1).text, true, "fetchFile with text: true");
  assert.equal(await d.write(["HOME"], "Y", bin("y")), "Y");
  assert.deepEqual({ ...hub.log.at(-1), data: undefined }, { cmd: "storeFile", dir: ["HOME"], name: "Y", data: undefined, replace: false });
  assert.equal(await d.mkdir(["HOME", "D"], "F"), "F");
  assert.deepEqual(hub.log.at(-1), { cmd: "createDir", dir: ["HOME", "D"], name: "F" });
  assert.equal(await d.rename(["HOME"], "Y", "Z"), "Z");
  assert.deepEqual(hub.log.at(-1), { cmd: "rename", dir: ["HOME"], name: "Y", to: "Z" });
  assert.equal(await d.purge(["HOME"], "Z"), 0);
  assert.deepEqual(hub.log.at(-1), { cmd: "purge", dir: ["HOME"], name: "Z" });
});

test("a name in use: nameClash for a put, a new directory and a rename; a put with replace replaces", async () => {
  const { hub, d } = await connected();
  await assert.rejects(d.write(["HOME"], "X", bin("x")), (err) => err.kind === "nameClash" && err.varName === "X");
  await assert.rejects(d.write(["HOME"], "D", bin("x"), { replace: true }), (err) => err.kind === "isDirectory");
  await assert.rejects(d.mkdir(["HOME"], "X"), (err) => err.kind === "nameClash");
  await assert.rejects(d.rename(["HOME"], "X", "D"), (err) => err.kind === "nameClash" && err.varName === "D");
  assert.equal(count(hub, "storeFile") + count(hub, "createDir") + count(hub, "rename"), 0, "refused before anything is sent");
  await d.write(["HOME"], "X", bin("new"), { replace: true });
  assert.equal(hub.log.at(-1).replace, true);
  await assert.rejects(d.write(["HOME"], "bad name", bin("x")), (err) => err.kind === "badName");
  await assert.rejects(d.write(["HOME"], "N", enc("plain")), (err) => err.kind === "notAnObject");
  assert.equal(await d.write(["HOME"], "N", enc("plain"), { asString: true }), "N");
  await assert.rejects(d.write(["HOME"], "BIG", bin("x".repeat(MAX_STORE_BYTES))), /512 KB/);
});

test("a directory that holds variables is purged only with its count", async () => {
  const { hub, d } = await connected();
  assert.equal(countAll(TREE()[1].variables), 3);
  await assert.rejects(d.purge(["HOME"], "D"), (err) => err.kind === "notEmpty" && err.count === 3);
  await assert.rejects(d.purge(["HOME"], "D", { count: 2 }), (err) => err.kind === "notEmpty");
  assert.equal(count(hub, "purge"), 0);
  assert.equal(await d.purge(["HOME"], "D", { count: 3 }), 3);
  assert.equal(count(hub, "purge"), 1, "one purge: the engine's is PGDIR for a directory");
});

test("the engine's refusals come back in tx's words, at once", async () => {
  const { hub, d } = await connected();
  for (const [engine, words] of [
    ["the command line is open: finish it (ENTER) or cancel it (ON) first", "The calculator is busy: finish or cancel its command line in Calculator."],
    ["typing is in progress (releaseAll stops it)", /busy typing in Calculator/],
    ["a transfer is in progress (releaseAll stops it)", /busy with another transfer/],
    ["flag -33 is set (I/O over infrared): clear it for transfers over the wire", /Flag -33 is set/],
  ]) {
    hub.refuse = engine;
    await assert.rejects(d.read(["HOME"], "X"), (err) => err.kind === "calculator" && (typeof words === "string" ? err.message === words : words.test(err.message)), engine);
  }
  assert.equal(refusalText("something else"), "something else");
  assert.equal(d.running, null, "nothing left waiting");
});

test("getTree: a directory's variables before its directories, file-system names, skip, item labels and progress", async () => {
  const { d } = await connected();
  const labels = [];
  const progress = [];
  d.addEventListener("busy", (e) => e.detail.label && labels.push(e.detail.label));
  d.addEventListener("progress", (e) => progress.push(e.detail.done));
  const t = await d.getTree(["HOME", "D"], { skip: ["String"] });
  assert.deepEqual(t.dir, ["HOME", "D"]);
  assert.deepEqual(t.items.map((i) => [i.path.join("/"), i.fsPath.join("/"), i.directory]), [["P", "P.hp", false], ["E", "E", true]]);
  assert.deepEqual(t.skipped, [{ path: ["E", "S"], kind: "String" }]);
  assert.ok(labels.includes("Getting D/P…") && labels.includes("Getting D/E/S…"), labels.join(", "));
  assert.ok(progress.length >= 1 && progress.at(-1) > 0);
  const text = await d.getTree(["HOME", "D", "E"], { format: "text" });
  assert.deepEqual(text.items.map((i) => i.fsPath.join("/")), ["S.txt"]);
});

test("putTree: a new directory, or into one with namesInUse until replace; progress per variable", async () => {
  const { hub, d } = await connected();
  const items = [{ path: ["A.hp"], data: bin("a") }, { path: ["SUB"] }, { path: ["SUB", "B.hp"], data: bin("b") }];
  const progress = [];
  d.addEventListener("progress", (e) => progress.push([e.detail.done, e.detail.total]));
  assert.deepEqual(await d.putTree(["HOME"], "NEW", items), { directories: 2, variables: 2 });
  const made = hub.dir(["HOME", "NEW"]);
  assert.deepEqual(made.map((v) => v.name), ["A", "SUB"], "the tree's order kept");
  assert.deepEqual(hub.dir(["HOME", "NEW", "SUB"]).map((v) => v.name), ["B"]);
  assert.deepEqual(progress.at(-1), [bin("a").length + bin("b").length, bin("a").length + bin("b").length]);
  await assert.rejects(d.putTree(["HOME"], "NEW", items), (err) => err.kind === "nameClash");
  await assert.rejects(d.putTree(["HOME", "NEW"], null, items), (err) => err.kind === "namesInUse" && err.names.join() === "A,SUB/B");
  const before = count(hub, "storeFile");
  assert.deepEqual(await d.putTree(["HOME", "NEW"], null, items, { replace: true }), { directories: 0, variables: 2 });
  assert.equal(count(hub, "storeFile"), before + 2);
  assert.equal(hub.log.filter((m) => m.cmd === "storeFile").at(-1).replace, true);
  await assert.rejects(d.putTree(["HOME"], "Q", [{ path: ["N.txt"], data: enc("plain") }]), (err) => err.kind === "notAnObject");
});

test("backup: one command, the file back; noRoom with the numbers", async () => {
  const { hub, d } = await connected();
  const b = await d.backup();
  assert.equal(b.model, "48gx");
  assert.equal(new TextDecoder().decode(b.data).slice(0, 4), "HPHP");
  assert.deepEqual(hub.log.at(-1), { cmd: "backup" });
  hub.refuse = "a backup needs 9024 bytes free and the calculator has 812.5";
  await assert.rejects(d.backup(), (err) => err.kind === "noRoom" && err.needed === 9024 && err.free === 812.5);
});

test("queued operations run one at a time; the waiting ones can be cancelled; gone is lost", async () => {
  const { hub, d } = await connected();
  const a = d.read(["HOME"], "X", { tag: "preview" });
  const b = d.read(["HOME"], "X", { tag: "preview" });
  assert.equal(d.queued >= 1, true);
  assert.equal(d.cancelQueued("preview") >= 1, true);
  await a;
  await assert.rejects(b, (err) => err.kind === "cancelled");
  const lost = [];
  d.addEventListener("lost", (e) => lost.push(e.detail));
  hub.dispatchEvent(new CustomEvent("gone", { detail: "This calculator's ROM was removed in Calculator." }));
  assert.deepEqual(lost, ["This calculator's ROM was removed in Calculator."]);
  assert.equal(d.connection, null);
  await d.close();
  assert.equal(hub.closed, true);
});

test("a memoryChanged during this page's put reads nothing until the put is done; a read refused by the emulator's write is tried again", async () => {
  const { hub, d } = await connected();
  await d.list(["HOME"]);
  let release;
  const slow = new Promise((r) => {
    release = r;
  });
  const store = hub.request.bind(hub);
  hub.request = async (cmd, args) => {
    if (cmd === "storeFile") await slow;
    if (cmd === "memoryTree" && hub.storing) throw new Error("a transfer is in progress (releaseAll stops it)");
    return store(cmd, args);
  };
  const changes = [];
  d.addEventListener("change", () => changes.push(Date.now()));
  const put = d.write(["HOME"], "Y", bin("y"));
  await new Promise((r) => setTimeout(r, 5));
  const trees = count(hub, "memoryTree");
  hub.changed();
  const listed = d.list(["HOME"]);
  await new Promise((r) => setTimeout(r, 5));
  assert.equal(count(hub, "memoryTree"), trees, "no read while the put runs");
  assert.equal(changes.length, 0, "the change told after the put");
  release();
  assert.equal(await put, "Y");
  assert.ok((await listed).entries.some((e) => e.name === "Y"), "the list after the put has it");
  assert.equal(changes.length, 1);
  // The emulator writes: its refusal is not said, the read comes once it is done.
  hub.storing = true;
  hub.changed();
  const later = d.list(["HOME"]);
  setTimeout(() => {
    hub.storing = false;
  }, 300);
  assert.ok((await later).entries.length > 0);
});
