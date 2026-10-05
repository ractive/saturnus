// Tests of web/memory.js with a fake backend: `node --test web/test/` (just web-test).

import assert from "node:assert/strict";
import { test } from "node:test";
import { MemoryView, ObjectLoader } from "../memory.js";
import { Store } from "../store.js";

const tick = () => new Promise((r) => setTimeout(r, 0));

/** A backend whose reads answer when the test lets them. */
class FakeBackend extends EventTarget {
  constructor() {
    super();
    this.machine = "one";
    /** Reads waiting for `release()`: `[machine, resolve]`. */
    this.held = [];
    this.hold = false;
    this.reads = 0;
  }
  async watchMemory() { return { supported: true, reason: null }; }
  read(make) {
    const machine = this.machine;
    this.reads++;
    if (!this.hold) return Promise.resolve(make(machine));
    return new Promise((resolve) => this.held.push(() => resolve(make(machine))));
  }
  release() {
    for (const r of this.held.splice(0)) r();
  }
  memoryTree() { return this.read((m) => ({ path: ["HOME"], variables: [{ name: m, type: "Real Number" }] })); }
  stack() { return this.read((m) => [{ type: "string", value: m }]); }
  flags() { return this.read(() => ({ system: ["0"], user: ["0"], set: [] })); }
}

test("a ROM booted while a read is in flight is read for itself", async () => {
  const backend = new FakeBackend();
  const store = new Store();
  const memory = new MemoryView(backend, store);
  store.set({ booted: "48sx", romName: "one" });
  await memory.setOpen(true);
  await tick();
  assert.equal(store.state.memoryTree.variables[0].name, "one");

  // A change arrives; its reads are slow.
  backend.hold = true;
  backend.dispatchEvent(new CustomEvent("memoryChanged"));
  await tick();
  assert.equal(backend.held.length, 3);
  // Meanwhile another ROM boots: the panes are emptied for it.
  backend.machine = "two";
  store.set({ booted: "49g", romName: "two" });
  await tick();
  assert.equal(store.state.memoryTree, null);
  // The old machine's answers arrive: dropped, and the new machine is
  // read without waiting for its memory to change.
  backend.hold = false;
  backend.release();
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(store.state.memoryTree?.variables[0].name, "two");
  assert.deepEqual(store.state.memoryStack, [{ type: "string", value: "two" }]);
  assert.equal(store.state.memorySupport.supported, true);
});

test("a failed object read is not kept", async () => {
  let fail = true;
  let reads = 0;
  const seen = [];
  const loader = new ObjectLoader(
    async () => {
      reads++;
      if (fail) throw new Error("the calculator is busy");
      return { type: "real", value: 1, text: "1" };
    },
    (state) => seen.push(state),
  );
  assert.deepEqual(loader.get("k", 100), { key: "k" });
  await tick();
  assert.deepEqual(seen, [{ key: "k", error: "the calculator is busy" }]);
  // Drawing the failure itself does not read again ...
  assert.equal(loader.get("k", 100, false).error, "the calculator is busy");
  assert.equal(reads, 1);
  // ... the next look does (the memory changed, the variable was selected again).
  fail = false;
  assert.deepEqual(loader.get("k", 100), { key: "k" });
  await tick();
  assert.equal(reads, 2);
  assert.deepEqual(loader.get("k", 100).object, { type: "real", value: 1, text: "1" });
  // A good answer is kept while the variable is unchanged, and read again when it changes.
  loader.get("k", 100);
  assert.equal(reads, 2);
  loader.get("k2", 100);
  assert.equal(reads, 3);
});
