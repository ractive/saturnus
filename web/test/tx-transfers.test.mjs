// The copies between the panes (web/tx/transfers.js) in Node: the real
// KermitDevice over the fake calculator (web/test/tx-fake-calc.js, in
// place of the protocol Worker), the real Folder over a folder in memory,
// the real wasm bindings for names, file kinds and the copy plan
// (web/tx/pkg; skipped without it, `just tx-web`). What is asked before a
// copy (replace, strings, the other family, what is left out), what each
// answer does, and what lands where.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { Folder } from "../tx/folder.js";
import { KermitDevice } from "../tx/kermit-device.js";
import { MemoryDirectory } from "../tx/memory-handle.js";
import { ProtocolClient } from "../tx/protocol.js";
import { JobQueue } from "../tx/queue.js";
import { Transfers, fileItem, flatten, folderItem, namesText, removeQuestion, replaceManyQuestion, restoreQuestion, suggestName } from "../tx/transfers.js";
import { fakeCalc, stubLink } from "./tx-fake-calc.js";

const pkgFile = new URL("../tx/pkg/saturnus_tx_web_bg.wasm", import.meta.url);
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

const enc = (t) => new TextEncoder().encode(`%%HP: T(3)A(R)F(.);\r\n${t}\r\n`);
const text = (bytes) => new TextDecoder().decode(bytes);

let serial = 0;
/** A connected fake calculator as a place. */
async function calcPlace(model = "48gx", opts = {}) {
  const calc = fakeCalc({ model, delay: () => 0, ...opts });
  const device = new KermitDevice(new ProtocolClient(stubLink(), { worker: calc.worker }));
  await device.connect();
  const place = { id: `calc${++serial}`, kind: "device", device, model, title: `the ${model}` };
  return { place, calc, device };
}

/** A folder in memory as a place. */
function folderPlace(title = "hp-files") {
  const folder = new Folder(new MemoryDirectory(title), { kind: "memory", title });
  return { place: { id: `folder${++serial}`, kind: "folder", folder, title }, folder };
}

/** Transfers whose questions are answered by `answers` (in order: true, false, or a name); every question kept. */
function transfers(h, answers = []) {
  const asked = [];
  const said = [];
  const t = new Transfers({
    queue: new JobQueue(),
    helpers: async () => h,
    ask: async (q) => {
      asked.push(q);
      return Boolean(answers.shift());
    },
    askName: async (q) => {
      asked.push(q);
      return answers.shift() ?? null;
    },
    say: (s, o) => said.push([s, Boolean(o?.error)]),
  });
  return { t, asked, said };
}

async function seed(device) {
  await device.write(["HOME"], "PRG", enc("\\<< 1 2 + \\>>"));
  await device.write(["HOME"], "X", enc("42"));
  await device.mkdir(["HOME"], "GAMES");
  await device.write(["HOME", "GAMES"], "TETRIS", enc("\\<< 4 \\>>"));
  await device.mkdir(["HOME", "GAMES"], "OLD");
  await device.write(["HOME", "GAMES", "OLD"], "PONG", enc("\\<< 6 \\>>"));
}

test("a calculator to a folder: .hp files and folders; Replace X? first, Cancel gets nothing", async (t) => {
  const h = await load(t);
  if (!h) return;
  const { place: calc, device, calc: fake } = await calcPlace();
  await seed(device);
  const { place: dir, folder } = folderPlace();
  const items = (await device.list(["HOME"])).entries;
  const a = transfers(h);
  const r = await a.t.copy({ from: calc, fromPath: ["HOME"], items, to: dir, toPath: ["hp-files"] });
  assert.deepEqual(r.copied.sort(), ["GAMES", "PRG", "X"]);
  assert.deepEqual(a.asked, [], "nothing to ask into an empty folder");
  assert.deepEqual((await folder.list(["hp-files"])).entries.map((e) => e.name), ["GAMES", "PRG.hp", "X.hp"]);
  assert.deepEqual((await folder.list(["hp-files", "GAMES"])).entries.map((e) => e.name), ["OLD", "TETRIS.hp"]);
  assert.match(text((await folder.read(["hp-files", "GAMES", "OLD"], "PONG.hp")).data), /^HPHP48-/);

  // Again: PRG.hp and X.hp in use, asked once; Cancel gets nothing.
  fake.requests.length = 0;
  const b = transfers(h, [false]);
  assert.equal(await b.t.copy({ from: calc, fromPath: ["HOME"], items: items.filter((e) => !e.directory), to: dir, toPath: ["hp-files"] }), null);
  assert.equal(b.asked.length, 1);
  assert.equal(b.asked[0].title, "Replace 2 files?");
  assert.deepEqual(b.asked[0].sections[0].items, ["PRG.hp", "X.hp"]);
  assert.equal(fake.requests.filter((q) => q.type === "get").length, 0);
  // The directory again: merged, its files in use asked about.
  await folder.write(["hp-files", "GAMES"], "TETRIS.hp", new Uint8Array([1]), { replace: true });
  const c = transfers(h, [false]);
  const kept = await c.t.copy({ from: calc, fromPath: ["HOME"], items: items.filter((e) => e.directory), to: dir, toPath: ["hp-files"] });
  assert.deepEqual(kept.failed, ["GAMES"]);
  assert.equal(c.asked[0].title, "Replace 2 files?");
  assert.deepEqual(c.asked[0].sections[0].items, ["GAMES/TETRIS.hp", "GAMES/OLD/PONG.hp"]);
  assert.deepEqual([...(await folder.read(["hp-files", "GAMES"], "TETRIS.hp")).data], [1], "kept after Cancel");
  const d = transfers(h, [true]);
  await d.t.copy({ from: calc, fromPath: ["HOME"], items: items.filter((e) => e.directory), to: dir, toPath: ["hp-files"] });
  assert.match(text((await folder.read(["hp-files", "GAMES"], "TETRIS.hp")).data), /^HPHP48-/, "replaced after Replace");
  // A directory as one file ("Get as one file").
  const e = transfers(h);
  await e.t.copy({ from: calc, fromPath: ["HOME"], items: items.filter((x) => x.directory), to: dir, toPath: ["hp-files"], single: true });
  assert.match(text((await folder.read(["hp-files"], "GAMES.hp")).data), /^HPHP48-.Directory/);
  // A file where a folder would go (or back) is left out, said.
  await folder.mkdir(["hp-files"], "PRG2.hp");
  await device.write(["HOME"], "PRG2", enc("1"));
  const f = transfers(h);
  assert.equal(await f.t.copy({ from: calc, fromPath: ["HOME"], items: [(await device.list(["HOME"])).entries.find((x) => x.name === "PRG2")], to: dir, toPath: ["hp-files"] }), null);
  assert.match(f.said[0][0], /Nothing was copied: PRG2\.hp \(a folder of that name is there\)/);
});

test("a folder to a calculator: files and folders; strings, names, Replace X? and the merge", async (t) => {
  const h = await load(t);
  if (!h) return;
  const { place: calc, device, calc: fake } = await calcPlace();
  await device.write(["HOME"], "X", enc("1"));
  const { place: dir, folder } = folderPlace();
  await folder.write(["hp-files"], "X.txt", enc("2"));
  await folder.write(["hp-files"], "notes.md", new TextEncoder().encode("hello"));
  await folder.write(["hp-files"], "1BAD.txt", enc("3"));
  await folder.mkdir(["hp-files"], "SUB");
  await folder.write(["hp-files", "SUB"], "A.txt", enc("4"));
  await folder.write(["hp-files", "SUB"], "B.md", new TextEncoder().encode("plain"));
  const entries = (await folder.list(["hp-files"])).entries;
  const items = entries.map((e) => folderItem(folder, ["hp-files"], e));
  // 1BAD asks a name (given GOOD), then the plan asks about the strings, then "Replace X?".
  const a = transfers(h, ["GOOD", true, true]);
  const r = await a.t.copy({ from: dir, fromPath: ["hp-files"], items, to: calc, toPath: ["HOME"] });
  assert.deepEqual(a.asked.map((q) => q.title), ["A name for it", "Copy 4 items to the 48gx?", "Replace X?"]);
  assert.deepEqual(a.asked[1].sections[0].items, ["SUB/B.md", "notes.md"]);
  assert.deepEqual(r.copied.sort(), ["1BAD.txt", "SUB", "X.txt", "notes.md"]);
  const home = (await device.list(["HOME"])).entries;
  assert.deepEqual(home.map((e) => [e.name, e.kind]).sort(), [["GOOD", "Real Number"], ["SUB", "Directory"], ["X", "Real Number"], ["notes", "String"]]);
  assert.deepEqual((await device.list(["HOME", "SUB"])).entries.map((e) => e.name).sort(), ["A", "B"]);
  const put = fake.requests.find((q) => q.type === "put-tree");
  assert.deepEqual([put.name, put.asString], ["SUB", true]);
  // SUB again: the directory is there, so the copy goes into it; its names in use ask once.
  const b = transfers(h, [true, false]);
  const again = await b.t.copy({ from: dir, fromPath: ["hp-files"], items: items.filter((i) => i.name === "SUB"), to: calc, toPath: ["HOME"] });
  assert.deepEqual(b.asked.map((q) => q.title), ["Store it as a string?", "Replace 2 variables?"]);
  assert.deepEqual(again.failed, ["SUB"], "Cancel: not replaced");
  const c = transfers(h, [true, true]);
  assert.deepEqual((await c.t.copy({ from: dir, fromPath: ["hp-files"], items: items.filter((i) => i.name === "SUB"), to: calc, toPath: ["HOME"] })).copied, ["SUB"]);
  const merge = fake.requests.filter((q) => q.type === "put-tree").at(-1);
  assert.deepEqual([merge.dir, merge.name, merge.replace], [["HOME", "SUB"], null, true]);
  // A file of the other family is warned about before it goes.
  const { place: c49 } = await calcPlace("49g");
  const binary = (await device.read(["HOME"], "X")).data;
  await folder.write(["hp-files"], "X48.hp", binary);
  const d = transfers(h, [false]);
  assert.equal(await d.t.copy({ from: dir, fromPath: ["hp-files"], items: [folderItem(folder, ["hp-files"], { name: "X48.hp", directory: false })], to: c49, toPath: ["HOME"] }), null);
  assert.match(d.asked[0].sections[0].text, /HP 48 file: the HP 49G stores it as a string/);
  // A variable name taken by a directory is left out.
  await folder.write(["hp-files"], "SUB.hp", binary);
  const e = transfers(h);
  assert.equal(await e.t.copy({ from: dir, fromPath: ["hp-files"], items: [folderItem(folder, ["hp-files"], { name: "SUB.hp", directory: false })], to: calc, toPath: ["HOME"] }), null);
  assert.match(e.said[0][0], /SUB \(a directory of that name is there\)/);
  // Dropped files (from the file manager) go the same way.
  const f = transfers(h);
  const dropped = await f.t.copy({ items: [fileItem(new File([enc("5")], "Y.txt"))], to: calc, toPath: ["HOME"] });
  assert.deepEqual(dropped.copied, ["Y.txt"]);
});

test("between calculators: the plan's warning, what is left out, Replace X?, text across families", async (t) => {
  const h = await load(t);
  if (!h) return;
  const { place: gx, device: gxd, calc: gxFake } = await calcPlace("48gx");
  await seed(gxd);
  await gxd.write(["HOME"], "LIB", h ? new TextEncoder().encode("HPHP48-ELibrary:x") : null);
  const { place: g49, device: d49, calc: fake49 } = await calcPlace("49g");
  await d49.write(["HOME"], "X", enc("7"));
  await d49.mkdir(["HOME"], "PRG");
  const items = (await gxd.list(["HOME"])).entries;
  const a = transfers(h, [false]);
  assert.equal(await a.t.copy({ from: gx, fromPath: ["HOME"], items, to: g49, toPath: ["HOME"] }), null);
  const q = a.asked[0];
  assert.equal(q.title, "Copy 2 items to the 49g?");
  assert.equal(q.body, "Before anything is copied:");
  assert.match(q.sections[0].text, /^These are HP 48 objects\. They go to the HP 49G as text, which it compiles;/);
  assert.deepEqual(q.sections[0].items.sort(), ["GAMES (a directory, with all it holds)", "X"], "the warning names what it covers");
  assert.deepEqual(q.sections[1].items, ["LIB"], "a library has no text form");
  assert.deepEqual(q.sections[2].items, ["PRG"], "a directory there with the name");
  assert.equal(gxFake.requests.filter((r) => r.type === "get" || r.type === "get-tree").length, 0, "nothing got before the answer");
  const b = transfers(h, [true, true]);
  const r = await b.t.copy({ from: gx, fromPath: ["HOME"], items, to: g49, toPath: ["HOME"] });
  assert.deepEqual(b.asked.map((x) => x.title), ["Copy 2 items to the 49g?", "Replace X?"]);
  assert.deepEqual(r.copied.sort(), ["GAMES", "X"]);
  assert.deepEqual(gxFake.requests.filter((x) => x.type === "get" || x.type === "get-tree").map((x) => [x.type, x.format]), [["get", "text"], ["get-tree", "text"]]);
  assert.ok(fake49.requests.some((x) => x.type === "put" && x.name === "X" && x.replace));
  assert.deepEqual((await d49.list(["HOME", "GAMES"])).entries.map((e) => e.name).sort(), ["OLD", "TETRIS"]);
  // Within a family, binary and nothing to say.
  const { place: gx2, device: gx2d } = await calcPlace("48gx");
  const c = transfers(h);
  await c.t.copy({ from: gx, fromPath: ["HOME"], items: items.filter((e) => e.name === "PRG"), to: gx2, toPath: ["HOME"] });
  assert.deepEqual(c.asked, []);
  assert.deepEqual([...(await gx2d.read(["HOME"], "PRG")).data], [...(await gxd.read(["HOME"], "PRG")).data], "byte for byte");
});

test("a folder to a folder: files and folders, merged, Replace X? first", async () => {
  const { place: src, folder: a } = folderPlace("a");
  const { place: dst, folder: b } = folderPlace("b");
  await a.write(["a"], "one.hp", new Uint8Array([1]));
  await a.mkdir(["a"], "d");
  await a.write(["a", "d"], "two.hp", new Uint8Array([2]));
  await b.mkdir(["b"], "d");
  await b.write(["b", "d"], "two.hp", new Uint8Array([9]));
  const h = { file_kind: () => "binary" };
  const items = (await a.list(["a"])).entries.map((e) => folderItem(a, ["a"], e));
  const no = transfers(h, [false]);
  const r = await no.t.copy({ from: src, fromPath: ["a"], items, to: dst, toPath: ["b"] });
  assert.deepEqual(r.copied, ["one.hp"]);
  assert.deepEqual(r.failed, ["d"]);
  assert.equal(no.asked[0].title, "Replace d/two.hp?");
  assert.deepEqual([...(await b.read(["b", "d"], "two.hp")).data], [9]);
  const yes = transfers(h, [true, true]);
  await yes.t.copy({ from: src, fromPath: ["a"], items, to: dst, toPath: ["b"] });
  assert.deepEqual(yes.asked.map((q) => q.title), ["Replace one.hp?", "Replace d/two.hp?"]);
  assert.deepEqual([...(await b.read(["b", "d"], "two.hp")).data], [2]);
  // Onto itself: refused.
  const self = transfers(h);
  assert.equal(await self.t.copy({ from: src, fromPath: ["a"], items, to: src, toPath: ["a"] }), null);
});

test("the questions' words and the small helpers", async () => {
  assert.deepEqual(replaceManyQuestion(["PRG"], "The calculator"), {
    title: "Replace PRG?",
    body: "The calculator already has a variable named PRG here. The copy takes its place; the old contents are lost.",
    action: "Replace",
    danger: true,
  });
  assert.equal(replaceManyQuestion(["a.hp", "b.hp"], "hp-files", { files: true }).title, "Replace 2 files?");
  assert.deepEqual(removeQuestion(["GAMES"], { device: true, count: 3, where: "the calculator" }), {
    title: "Purge GAMES?",
    body: "GAMES is removed from the calculator for good. It holds 3 variables, which go too.",
    action: "Purge",
    danger: true,
    sections: [],
  });
  const many = removeQuestion(["a", "b"], { device: false, count: 1, where: "hp-files" });
  assert.equal(many.title, "Delete 2 items?");
  assert.equal(many.body, "They are removed from hp-files for good. They hold 1 item, which go too.");
  assert.deepEqual(many.sections, [{ text: "Deleted:", items: ["a", "b"] }]);
  assert.equal(restoreQuestion("48GX-backup.hp", "Emulated 48GX").title, "Replace everything on the calculator?");
  assert.equal(namesText(["A"]), "A");
  assert.equal(namesText(["A", "B"]), "A and B");
  assert.equal(namesText(["A", "B", "C", "D"]), "A, B and 2 more");
  assert.equal(suggestName("1 my file.txt"), "myfile");
  assert.equal(suggestName("---"), "NAME");
  const { folder } = folderPlace("z");
  await folder.mkdir(["z"], "d");
  await folder.write(["z", "d"], "x.hp", new Uint8Array([5]));
  await folder.mkdir(["z", "d"], "e");
  const tree = await flatten(folderItem(folder, ["z"], { name: "d", directory: true }));
  assert.deepEqual(tree.map((i) => [i.path.join("/"), i.data ? [...i.data] : null]), [["e", null], ["x.hp", [5]]]);
});

test("review: one storage in both panes is not copied onto itself, nor a folder into itself", async () => {
  const root = new MemoryDirectory("Files");
  // Two panes on the page's files: two Folder objects, two places, one storage.
  const a = { id: "left:1", kind: "folder", folder: new Folder(root, { kind: "memory", title: "Files" }), title: "Files" };
  const b = { id: "right:1", kind: "folder", folder: new Folder(root, { kind: "memory", title: "Files" }), title: "Files" };
  await a.folder.write(["Files"], "one.hp", new Uint8Array([1]));
  await a.folder.mkdir(["Files"], "d");
  const items = (await a.folder.list(["Files"])).entries.map((e) => folderItem(a.folder, ["Files"], e));
  const same = transfers({ file_kind: () => "binary" });
  assert.equal(await same.t.copy({ from: a, fromPath: ["Files"], items, to: b, toPath: ["Files"] }), null);
  assert.match(same.said[0][0], /one\.hp \(it is already here\)/);
  assert.deepEqual(same.asked, [], "no Replace question about itself");
  const into = transfers({ file_kind: () => "binary" });
  assert.equal(await into.t.copy({ from: a, fromPath: ["Files"], items: items.filter((i) => i.name === "d"), to: b, toPath: ["Files", "d"] }), null);
  assert.match(into.said[0][0], /d \(a folder cannot go into itself\)/);
  // Another storage with the same title is another folder.
  const c = folderPlace("Files");
  const other = transfers({ file_kind: () => "binary" });
  assert.deepEqual((await other.t.copy({ from: a, fromPath: ["Files"], items, to: c.place, toPath: ["Files"] })).copied.sort(), ["d", "one.hp"]);
});

test("review: a copy whose planning fails says so and sends nothing", async () => {
  const { place: dir, folder } = folderPlace();
  await folder.write(["hp-files"], "A.hp", new Uint8Array([1]));
  const lost = { id: "lost", kind: "device", model: "48gx", title: "the 48gx", device: { list: async () => { throw Object.assign(new Error("The cable or adapter was unplugged."), { kind: "calculator" }); } } };
  const a = transfers({ file_kind: () => "binary" });
  const r = await a.t.copy({ from: dir, fromPath: ["hp-files"], items: [folderItem(folder, ["hp-files"], { name: "A.hp", directory: false })], to: lost, toPath: ["HOME"] });
  assert.equal(r, null);
  assert.deepEqual(a.said, [["Nothing was copied: The cable or adapter was unplugged.", true]]);
});

test("review: the progress of a copy between two calculators only grows", async () => {
  const src = new EventTarget();
  const dst = new EventTarget();
  const fire = (d, done, total) => d.dispatchEvent(new CustomEvent("progress", { detail: { done, total, label: "" } }));
  const seen = [];
  const t = new Transfers({ queue: new JobQueue(), helpers: async () => ({}), ask: async () => true, askName: async () => null, say: () => {} });
  await t.watch([{ kind: "device", device: src }, { kind: "device", device: dst }], (done, total) => seen.push([done, total]), async () => {
    fire(src, 50, 100);
    fire(src, 100, 100);
    fire(dst, 40, 120);
    fire(dst, 120, 120);
  });
  assert.deepEqual(seen, [[50, 200], [100, 200], [140, 220], [220, 220]]);
  for (let i = 1; i < seen.length; i++) assert.ok(seen[i][0] >= seen[i - 1][0] && seen[i][0] / seen[i][1] >= seen[i - 1][0] / seen[i - 1][1], `never backwards: ${JSON.stringify(seen)}`);
});
