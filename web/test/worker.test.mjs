// The Web Worker as a driver of the protocol's state machine, against a
// fake of the wasm bindings' `Host`: `node --test web/test/` (just
// web-test). The rules the Worker used to implement itself (pacing,
// sends, refusals) are the state machine's, tested in Rust
// (crates/saturnus-host/src/protocol/tests.rs) and across hosts
// (protocol-script.test.mjs); here: one timer at the deadline it asks
// for, its output posted in order, replies matched to the page's ids,
// bytes passed apart, and the ROM slots' commands checked by it first.

import assert from "node:assert/strict";
import { register } from "node:module";
import { after, afterEach, test } from "node:test";

// worker.js imports the wasm bindings from ./pkg/; give it a fake instead.
const fake = `
export const log = [];
export const state = { deadline: undefined, next: undefined, out: [], refuse: null, now: null };
export default async function init() {}
export const model_names = () => ["48sx"];
export const identify_rom = () => ({});
export const plan_roms = () => ({});
export const rom_download = () => null;
export class Host {
  constructor(now) { state.now = now; }
  command(json, bytes, tag) {
    const m = JSON.parse(json);
    log.push(["command", m, bytes, tag]);
    if (tag !== undefined) state.out.push({ type: "status", cmd: m.cmd }, { type: "reply", tag, ok: true, result: m.cmd });
  }
  check(json) {
    log.push(["check", JSON.parse(json)]);
    if (state.refuse) throw state.refuse;
  }
  boot(model, rom, name) { log.push(["boot", model, rom.length, name]); return { model, romName: name }; }
  // A timer moves the deadline on to the next one (\`next\`, none unless a
  // test sets it), as the state machine's does: a deadline left in the
  // past would have the Worker re-arm and fire it forever, and the test
  // file would never end.
  timer() {
    log.push(["timer"]);
    state.out.push({ type: "frame", n: log.length });
    state.deadline = state.next;
    state.next = undefined;
  }
  deadline() { return state.deadline; }
  drain() { const o = state.out; state.out = []; return o; }
}
`;
const hooks = `
export async function resolve(spec, ctx, next) {
  if (spec.endsWith("/pkg/saturnus_web.js")) {
    return { url: "data:text/javascript," + encodeURIComponent(${JSON.stringify(fake)}), shortCircuit: true };
  }
  return next(spec, ctx);
}`;
register("data:text/javascript," + encodeURIComponent(hooks));

const posted = [];
globalThis.self = { postMessage: (m) => posted.push(m) };
await import("../worker.js");
const pkg = await import("../pkg/saturnus_web.js");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// A failed test must not leave a deadline armed for the next one.
afterEach(() => {
  pkg.state.deadline = undefined;
  pkg.state.next = undefined;
  pkg.state.refuse = null;
});

// The tests done, nothing may keep the file alive: a timer the Worker
// still re-arms (a broken re-arm, an interval) ends it with a message
// after 2 s instead of a run that never finishes. Unref'd, it does not
// keep a healthy file waiting.
after(() => {
  setTimeout(() => {
    console.error("worker.test.mjs: the Worker still runs a timer after the tests; ending the file");
    process.exit(1);
  }, 2000).unref();
});

/** How many timers the Worker has fired. */
const timers = () => pkg.log.filter((c) => c[0] === "timer").length;

async function send(msg) {
  self.onmessage({ data: { v: 1, ...msg } });
  await sleep(5);
}

test("replies carry the page's ids, bytes travel apart", async () => {
  await send({ id: "a", cmd: "boot", model: "48sx", rom: new Uint8Array([1, 2, 3]), romName: "x" });
  const [, msg, bytes, tag] = pkg.log.find((c) => c[0] === "command");
  assert.deepEqual(msg, { v: 1, id: "a", cmd: "boot", model: "48sx", romName: "x" });
  assert.deepEqual([...bytes], [1, 2, 3]);
  assert.deepEqual(posted.splice(0), [
    { type: "status", cmd: "boot" },
    { type: "reply", id: "a", ok: true, result: "boot" },
  ]);
  // Without an id there is no tag; a state is bytes too.
  await send({ cmd: "loadState", state: new Uint8Array([9]) });
  const last = pkg.log.at(-1);
  assert.equal(last[3], undefined);
  assert.deepEqual([...last[2]], [9]);
  assert.equal(last[1].state, undefined);
  // A file to store is bytes too.
  await send({ id: "f", cmd: "storeFile", dir: ["HOME"], name: "P", data: new Uint8Array([7, 8]) });
  const store = pkg.log.at(-1);
  assert.deepEqual(store[1], { v: 1, id: "f", cmd: "storeFile", dir: ["HOME"], name: "P" });
  assert.deepEqual([...store[2]], [7, 8]);
  posted.length = 0;
});

test("one timer at the deadline the state machine asks for, re-armed at the next", async () => {
  const before = timers();
  const at = performance.now() + 30;
  pkg.state.deadline = at;
  // After the timer, the engine hands back its next deadline.
  pkg.state.next = at + 20;
  await send({ id: 1, cmd: "pause", paused: false });
  posted.length = 0;
  await sleep(10);
  // On a starved machine the 10 ms may outlast the deadline: then the
  // timer has rightly fired, and only "once" is checked.
  if (performance.now() < at) assert.equal(timers(), before, "not before its time");
  for (let i = 0; i < 200 && timers() < before + 2; i++) await sleep(10);
  assert.equal(timers(), before + 2, "once at its time, once at the next deadline");
  assert.equal(posted.at(-1).type, "frame", "what the timer gave is posted");
  await sleep(60);
  assert.equal(timers(), before + 2, "no deadline, no timer");
  assert.equal(pkg.state.now() > 0, true, "the clock is performance.now");
});

test("a starved event loop: the timer fires late, once, and nothing loops", async () => {
  const before = timers();
  pkg.state.deadline = performance.now() + 5;
  // Delivered at once (no wait), then the machine too busy to run timers
  // for 50 ms: the deadline passes unseen and the timer fires late.
  self.onmessage({ data: { v: 1, id: 2, cmd: "pause", paused: false } });
  const end = performance.now() + 50;
  while (performance.now() < end);
  assert.equal(timers(), before, "no timer ran while the loop was starved");
  await sleep(30);
  assert.equal(timers(), before + 1);
  await sleep(30);
  assert.equal(timers(), before + 1, "a past deadline is not fired again and again");
  posted.length = 0;
});

test("ROM slot commands pass the state machine's check first", async () => {
  pkg.state.refuse = "typing is in progress (releaseAll stops it)";
  await send({ id: 7, cmd: "bootModel", model: "48sx" });
  assert.deepEqual(posted.at(-1), { type: "reply", id: 7, ok: false, error: "typing is in progress (releaseAll stops it)" });
  assert.deepEqual(pkg.log.findLast((c) => c[0] === "check")[1], { v: 1, cmd: "bootModel" });
  await send({ cmd: "chooseRom", model: "48sx" });
  assert.deepEqual(posted.at(-1), { type: "error", message: "typing is in progress (releaseAll stops it)" });
  pkg.state.refuse = null;
  assert.equal(pkg.log.filter((c) => c[0] === "boot").length, 0, "nothing booted");
});
