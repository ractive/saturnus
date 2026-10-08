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
import { test } from "node:test";

// worker.js imports the wasm bindings from ./pkg/; give it a fake instead.
const fake = `
export const log = [];
export const state = { deadline: undefined, out: [], refuse: null, now: null };
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
  timer() { log.push(["timer"]); state.out.push({ type: "frame", n: log.length }); }
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

test("one timer at the deadline the state machine asks for", async () => {
  pkg.state.deadline = performance.now() + 30;
  await send({ id: 1, cmd: "pause", paused: false });
  posted.length = 0;
  const timers = () => pkg.log.filter((c) => c[0] === "timer").length;
  const before = timers();
  await sleep(10);
  assert.equal(timers(), before, "not before its time");
  // Each call re-arms it; undefined means no timer.
  pkg.state.deadline = undefined;
  await sleep(40);
  assert.equal(timers(), before + 1);
  assert.equal(posted.at(-1).type, "frame", "what the timer gave is posted");
  await sleep(40);
  assert.equal(timers(), before + 1, "no deadline, no timer");
  assert.equal(pkg.state.now() > 0, true, "the clock is performance.now");
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
