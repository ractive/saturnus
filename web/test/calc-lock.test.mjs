// The handover of a shared calculator (web/shared/calc-lock.js, iteration
// 41) over fake Web Locks and a fake BroadcastChannel, and the state
// format check (web/shared/calc-states.js). The real browser APIs run in
// web/test/tx-shared.test.mjs.
import assert from "node:assert/strict";
import { test } from "node:test";
import { CalcLock, IN_USE, inUseText, lockName } from "../shared/calc-lock.js";
import { stateFormatOf, tooNew } from "../shared/calc-states.js";

/** Web Locks for one origin: exclusive locks, `ifAvailable`, `signal` and `steal` as the spec has them. */
class FakeLocks {
  constructor() {
    this.holders = new Map();
    this.queues = new Map();
  }
  request(name, options, cb) {
    return new Promise((resolve, reject) => {
      const req = { cb, resolve, reject };
      const queue = this.queues.get(name) ?? [];
      this.queues.set(name, queue);
      if (options.steal) {
        const old = this.holders.get(name);
        this.holders.delete(name);
        if (old) {
          const e = new Error("stolen");
          e.name = "AbortError";
          old.reject(e);
        }
        this.grant(name, req);
        return;
      }
      if (this.holders.has(name) || queue.length) {
        if (options.ifAvailable) {
          Promise.resolve(cb(null)).then(resolve, reject);
          return;
        }
        queue.push(req);
        options.signal?.addEventListener("abort", () => {
          const i = queue.indexOf(req);
          if (i < 0) return;
          queue.splice(i, 1);
          const e = new Error("aborted");
          e.name = "AbortError";
          reject(e);
        });
        return;
      }
      this.grant(name, req);
    });
  }
  grant(name, req) {
    this.holders.set(name, req);
    Promise.resolve(req.cb({ name })).then(
      (v) => this.done(name, req, () => req.resolve(v)),
      (e) => this.done(name, req, () => req.reject(e)),
    );
  }
  done(name, req, settle) {
    if (this.holders.get(name) === req) {
      this.holders.delete(name);
      const next = this.queues.get(name)?.shift();
      if (next) this.grant(name, next);
    }
    settle();
  }
}

/** A BroadcastChannel bus: each endpoint hears the others, later; `deaf` ones (a frozen tab) hear nothing. */
function bus() {
  const ends = [];
  return () => {
    const end = {
      onmessage: null,
      deaf: false,
      postMessage(data) {
        for (const o of ends) if (o !== end && !o.deaf) setTimeout(() => o.onmessage?.({ data: structuredClone(data) }), 1);
      },
    };
    ends.push(end);
    return end;
  };
}

function pair({ wanted = async () => {}, askMs = 100, handoverMs = 400 } = {}) {
  const locks = new FakeLocks();
  const channel = bus();
  const log = [];
  const make = (who) => new CalcLock({
    who,
    locks,
    channel: channel(),
    askMs,
    handoverMs,
    onWanted: async (model, by) => {
      log.push(`${who} wanted ${model} by ${by}`);
      await wanted(who, model);
    },
    onLost: (model) => log.push(`${who} lost ${model}`),
  });
  return { locks, log, emulator: make("emulator"), tx: make("tx") };
}

test("a free calculator is taken at once; per model", async () => {
  const { emulator, tx, locks } = pair();
  assert.equal(await emulator.acquire("48gx"), true);
  assert.ok(emulator.holds("48gx"));
  assert.ok(locks.holders.has(lockName("48gx")));
  assert.equal(await tx.acquire("49g"), true, "another model is another lock");
  emulator.release("48gx");
  assert.ok(!emulator.holds("48gx"));
  await new Promise((r) => setTimeout(r, 5));
  assert.ok(!locks.holders.has(lockName("48gx")));
});

test("without asking, a held calculator is not taken", async () => {
  const { emulator, tx, log } = pair();
  await emulator.acquire("48gx");
  assert.equal(await tx.acquire("48gx", { ask: false }), false);
  assert.ok(emulator.holds("48gx"));
  assert.deepEqual(log, []);
});

test("asked, the holder wraps up first, then lets go", async () => {
  let wrapped = false;
  const { emulator, tx, log } = pair({
    wanted: async () => {
      await new Promise((r) => setTimeout(r, 200));
      wrapped = true;
    },
  });
  await emulator.acquire("48gx");
  assert.equal(await tx.acquire("48gx"), true);
  assert.ok(wrapped, "the holder's wrap-up ran before the asker got the lock");
  assert.ok(tx.holds("48gx"));
  assert.ok(!emulator.holds("48gx"));
  assert.deepEqual(log, ["emulator wanted 48gx by tx"], "handed over, not stolen");
  // And back.
  assert.equal(await emulator.acquire("48gx"), true);
  assert.ok(!tx.holds("48gx"));
  assert.deepEqual(log, ["emulator wanted 48gx by tx", "tx wanted 48gx by emulator"]);
});

test("a holder that does not answer (a frozen tab) loses the lock after the ask time", async () => {
  const { emulator, tx, log } = pair({ askMs: 100 });
  await emulator.acquire("48gx");
  emulator.channel.deaf = true;
  const t0 = Date.now();
  assert.equal(await tx.acquire("48gx"), true);
  assert.ok(Date.now() - t0 >= 100, "waited for an answer first");
  await new Promise((r) => setTimeout(r, 5));
  assert.ok(!emulator.holds("48gx"));
  assert.deepEqual(log, ["emulator lost 48gx"]);
});

test("a holder that answers but never lets go loses the lock after the handover time", async () => {
  const { emulator, tx, log } = pair({ askMs: 50, handoverMs: 300, wanted: () => new Promise(() => {}) });
  await emulator.acquire("48gx");
  const t0 = Date.now();
  assert.equal(await tx.acquire("48gx"), true);
  assert.ok(Date.now() - t0 >= 300, "an answer buys the holder the handover time");
  await new Promise((r) => setTimeout(r, 5));
  assert.ok(log.includes("emulator lost 48gx"));
  assert.ok(tx.holds("48gx"));
});

test("without Web Locks every calculator is the page's own", async () => {
  const solo = new CalcLock({ who: "tx", locks: null, channel: null });
  assert.ok(solo.solo);
  assert.equal(await solo.acquire("48gx"), true);
  assert.ok(solo.holds("49g"));
});

test("the words for who has a calculator", () => {
  assert.equal(inUseText("tx", "emulator"), "In use by saturnus tx.");
  assert.equal(inUseText("emulator", "tx"), "In use by the emulator.");
  assert.equal(inUseText("emulator", "emulator"), "In use in another emulator tab.");
  assert.equal(inUseText(null, "emulator"), "In use in another tab.");
  assert.equal(typeof IN_USE, "string");
});

test("a state's format version, and one newer than the page's", () => {
  const state = (v) => new Uint8Array([..."SATURNUS"].map((c) => c.charCodeAt(0)).concat([v & 0xff, v >> 8, 1, 2, 3]));
  assert.equal(stateFormatOf(state(2)), 2);
  assert.equal(stateFormatOf(state(0x0103)), 0x0103);
  assert.equal(stateFormatOf(new Uint8Array([1, 2, 3])), null);
  assert.equal(stateFormatOf(null), null);
  assert.equal(tooNew(state(3), 2), true);
  assert.equal(tooNew(state(2), 2), false);
  assert.equal(tooNew(state(1), 2), false, "an older one boots cold, as before");
  assert.equal(tooNew(new Uint8Array(20), 2), false, "not a state: not newer");
});
