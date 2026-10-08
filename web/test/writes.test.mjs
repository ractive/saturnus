// Tests of web/writes.js with a fake backend: `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { MAX_FILE_BYTES, MemoryWrites, fileNameFor, pathText, variableName } from "../writes.js";
import { Store } from "../store.js";

test("files and variables name each other", () => {
  assert.equal(variableName("prog.hp"), "prog");
  assert.equal(variableName("C:\\calc\\GAME.txt"), "GAME");
  assert.equal(variableName("dir/a.b.c"), "a.b");
  assert.equal(variableName(".hidden"), ".hidden");
  assert.equal(variableName("NOEXT"), "NOEXT");
  assert.equal(fileNameFor("PRG"), "PRG.hp");
  assert.equal(fileNameFor("A/B:C"), "A_B_C.hp");
  assert.equal(fileNameFor(""), "variable.hp");
  assert.equal(pathText(["HOME", "D"]), "HOME › D");
});

/** A backend whose writes answer when the test says. */
class FakeBackend {
  constructor() {
    this.calls = [];
    this.pending = [];
  }
  answer(cmd, args) {
    this.calls.push([cmd, ...args]);
    return new Promise((resolve, reject) => this.pending.push({ resolve, reject }));
  }
  storeFile(...a) { return this.answer("storeFile", a); }
  fetchFile(...a) { return this.answer("fetchFile", a); }
  purge(...a) { return this.answer("purge", a); }
  rename(...a) { return this.answer("rename", a); }
  changeDir(...a) { return this.answer("changeDir", a); }
  setFlag(...a) { return this.answer("setFlag", a); }
  next() { return this.pending.shift(); }
}

const tick = () => new Promise((r) => setTimeout(r, 0));

function setup() {
  const backend = new FakeBackend();
  const store = new Store();
  const saved = [];
  let t = 0;
  const writes = new MemoryWrites(backend, store, { save: (b, n) => saved.push([n, [...b]]), now: () => (t += 50) });
  return { backend, store, saved, writes };
}

test("one write at a time, behind the overlay, with its message", async () => {
  const { backend, store, writes } = setup();
  const p = writes.purge(["HOME"], "X");
  assert.equal(store.state.writing, "Purging X…");
  // A second one is refused while the first runs.
  assert.equal(await writes.setFlag(5, true), null);
  assert.deepEqual(store.state.writeMessage, { text: "Another write is still running.", error: true });
  assert.equal(backend.calls.length, 1);
  backend.next().resolve({ emulatedMs: 1 });
  await p;
  assert.equal(store.state.writing, null);
  assert.deepEqual(store.state.writeMessage, { text: "X purged from HOME, in 0.05 s.", error: false });
  // A failure says what failed and why.
  const q = writes.rename(["HOME", "D"], "A", "B");
  backend.next().reject(new Error("B already exists in { HOME D }"));
  await q;
  assert.deepEqual(store.state.writeMessage, { text: "Renaming A failed: B already exists in { HOME D }", error: true });
  // The 49G in algebraic mode sets flags by keys, and says so.
  const f = writes.setFlag(-95, false);
  backend.next().resolve({ keys: true });
  await f;
  assert.match(store.state.writeMessage.text, /^Flag -95 cleared \(by keys/);
});

test("stored files are named after them; too large ones are not sent", async () => {
  const { backend, store, writes } = setup();
  const small = { name: "prog.hp", size: 10 };
  const big = { name: "big.bin", size: MAX_FILE_BYTES + 1 };
  const p = writes.storeFiles(["HOME", "D"], [big, small]);
  await tick();
  assert.deepEqual(backend.calls, [["storeFile", ["HOME", "D"], "prog", small]]);
  backend.next().resolve({ name: "prog" });
  await p;
  assert.match(store.state.writeMessage.text, /^prog\.hp stored as prog in HOME › D, in 0\.05 s\. Larger than 512 KiB, not stored: big\.bin\.$/);
});

test("a fetched file is saved by the page, or by the app (a cancelled dialog says nothing)", async () => {
  const { backend, store, saved, writes } = setup();
  const p = writes.fetch(["HOME"], "PRG");
  backend.next().resolve({ name: "PRG", size: 3, data: new Uint8Array([1, 2, 3]) });
  await p;
  assert.deepEqual(saved, [["PRG.hp", [1, 2, 3]]]);
  assert.match(store.state.writeMessage.text, /^PRG saved as PRG\.hp \(3 bytes\)/);
  // The app wrote it itself.
  const q = writes.fetch(["HOME"], "Q");
  backend.next().resolve({ name: "Q", size: 9, file: "Q.hp" });
  await q;
  assert.equal(saved.length, 1);
  assert.match(store.state.writeMessage.text, /^Q saved as Q\.hp \(9 bytes\)/);
  // Cancelled in the app's dialog: no message, the overlay gone.
  const r = writes.storeAsked(["HOME"]);
  backend.next().resolve(null);
  assert.equal(await r, null);
  assert.equal(store.state.writeMessage, null);
  assert.equal(store.state.writing, null);
  assert.deepEqual(backend.calls.at(-1), ["storeFile", ["HOME"], null, null]);
});
