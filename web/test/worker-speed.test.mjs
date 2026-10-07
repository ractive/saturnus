// The Web Worker's speed rule against a fake wasm core: `node --test
// web/test/` (just web-test). The speed applies only while the calculator
// computes; asleep, emulated time follows the wall clock at 1x.

import assert from "node:assert/strict";
import { register } from "node:module";
import { test } from "node:test";

// A fake core with an emulated clock: the CPU computes until `busyUntil`,
// otherwise sleeps in SHUTDN until its next timer event (every 500 ms,
// which wakes it for 2 ms, as a cursor blink); a key makes it compute
// \`work\` ms. Running costs no wall time.
const fake = `
export const cpu = { now: 0, busyUntil: 0, work: 0, queued: false };
const TICK = 500;
export default async function init() {}
export const layout = () => ({});
export const skin = () => ({});
export const model_for = (rom, model) => model;
export const model_names = () => ["48sx"];
export const identify_rom = () => "{}";
export const plan_roms = () => "{}";
class Core {
  constructor(model) { this.m = model; }
  model() { return this.m; }
  idle_ms() {
    if (cpu.now < cpu.busyUntil) return -1;
    return (Math.floor(cpu.now / TICK) + 1) * TICK - cpu.now;
  }
  keys_busy() { return cpu.queued; }
  type_letter() { cpu.queued = true; return true; }
  // The key reaches the ROM after the time that passed: it computes \`work\` ms.
  pump() {
    if (cpu.queued) cpu.busyUntil = Math.max(cpu.busyUntil, cpu.now + cpu.work);
    cpu.queued = false;
  }
  emulated_ms() { return cpu.now; }
  run_slice(left) {
    const idle = this.idle_ms();
    const step = Math.min(left, idle < 0 ? Math.min(cpu.busyUntil - cpu.now, 1) : idle);
    const tick = Math.floor(cpu.now / TICK);
    cpu.now += step;
    if (Math.floor(cpu.now / TICK) > tick) cpu.busyUntil = Math.max(cpu.busyUntil, cpu.now + 2);
    return step;
  }
  take_errors() { return []; }
  take_keys() { return undefined; }
  take_frame() { return undefined; }
}
export const Emulator = new Proxy(Core, {
  construct(target, args) {
    const core = new target(...args);
    return new Proxy(core, {
      get(t, name) {
        if (name in t) return t[name].bind(t);
        return () => 0;
      },
    });
  },
});
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

let id = 0;
/** Send `msg` with an id; the reply's promise. */
function send(msg) {
  const i = ++id;
  self.onmessage({ data: { v: 1, id: i, ...msg } });
  return new Promise((resolve) => {
    const look = () => {
      const r = posted.find((m) => m.type === "reply" && m.id === i);
      if (r) resolve(r);
      else setTimeout(look, 1);
    };
    look();
  });
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
/** A key wakes the sleeping machine (as on the calculator). */
const wake = () => send({ cmd: "typeLetter", letter: "A" });

/**
 * Emulated ms (paid and owed) and wall ms advanced over `ms` of wall time,
 * after `first`.
 */
async function measure(ms, first = async () => {}) {
  const a = (await send({ cmd: "stats" })).result;
  await first();
  await sleep(ms);
  const b = (await send({ cmd: "stats" })).result;
  return { emulated: b.emulatedMs + b.owedMs - (a.emulatedMs + a.owedMs), wall: b.nowMs - a.nowMs };
}

test("the speed applies to computing only", async (t) => {
  // Stop the run loop's timers so the process can end.
  t.after(() => send({ cmd: "pause", paused: true }));
  assert.ok((await send({ cmd: "boot", model: "48sx", rom: new Uint8Array(4) })).ok);
  await sleep(50);

  // Idle at 4x and at Max: emulated time keeps to the wall clock.
  for (const speed of ["4", "max"]) {
    await send({ cmd: "setSpeed", speed });
    const { emulated, wall } = await measure(1000);
    assert.ok(Math.abs(emulated / wall - 1) < 0.05, `idle at ${speed}: ${emulated} ms over ${wall} ms`);
  }

  // Computing at 4x runs four times as fast.
  await send({ cmd: "setSpeed", speed: "4" });
  pkg.cpu.busyUntil = Infinity;
  await wake();
  const busy = await measure(1000);
  const rate = busy.emulated / busy.wall;
  assert.ok(rate > 3.6 && rate < 4.4, `computing at 4x: rate ${rate}`);

  // At Max a computation runs as fast as it can, and the sleep after it
  // is at 1x again: 100 emulated ms of work, then a second of sleep (the
  // old rule skipped a second per pass and slept at 60x). How much of the
  // work shows depends on where the passes fall, so only the bounds count.
  await send({ cmd: "setSpeed", speed: "max" });
  pkg.cpu.busyUntil = pkg.cpu.now;
  await sleep(50);
  const after = await measure(1000, () => {
    pkg.cpu.work = 100;
    return wake();
  });
  const drift = after.emulated - after.wall;
  assert.ok(drift > 0 && drift < 300, `max then idle: ${after.emulated} ms over ${after.wall} ms`);

});
