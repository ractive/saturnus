// The calculators' hub (web/hub.js, iteration 46): its bookkeeping of the
// pages connected to one live calculator per model, against a fake of the
// wasm bindings' `Host` (one per engine), fake ports, fake Web Locks and
// an in-memory store of the auto-saved states: `node --test web/test/`
// (just web-test). Two pages on one model share its engine; replies go
// to the asker, frames to every page, `memoryChanged` to the pages that
// watch; the serial port of a shared calculator is refused; the emulator
// pages hear `peers` while a saturnus tx page is on it; the speed is the
// emulator page's; a machine is hidden only when every page is; the last
// page to leave (by message or by its lock) has the machine saved and
// stopped; another build is refused. The pages and the real engine together:
// web/test/tx-shared.test.mjs.

import assert from "node:assert/strict";
import { register } from "node:module";
import { test } from "node:test";

const fake = `
export const hosts = [];
export default async function init() {}
export const model_names = () => ["48gx", "49g"];
export const identify_rom = () => ({});
export const plan_roms = () => ({});
export const rom_download = () => null;
export const state_format = () => 2;
export class Host {
  constructor() { this.out = []; this.log = []; this.model = null; hosts.push(this); }
  setAutoSave(on) { this.autoSave = on; }
  command(json, bytes, tag) {
    const m = JSON.parse(json);
    this.log.push(m);
    if (this.panic) throw new Error("unreachable");
    if (m.cmd === "unload" && this.refuseUnload > 0) {
      this.refuseUnload--;
      if (tag !== undefined) this.out.push({ type: "reply", tag, ok: false, error: "typing is in progress (releaseAll stops it)" });
      return;
    }
    if (m.cmd === "unload") {
      if (this.owed) this.out.push({ type: "autoSave", model: this.model, romName: "r", cycles: 1, state: new Uint8Array([this.owed]) });
      this.model = null;
      this.out.push({ type: "status", model: null });
    }
    if (m.cmd === "storeFile" && this.serialHeld) {
      if (tag !== undefined) this.out.push({ type: "reply", tag, ok: false, error: "the serial port is in use (serialClose frees it)" });
      return;
    }
    if (m.cmd === "serialOpen") this.serialHeld = true;
    if (m.cmd === "serialClose") this.serialHeld = false;
    const result = m.cmd === "endServer" || m.cmd === "probeServer" ? { server: false } : m.cmd === "hello" ? { protocol: 1 } : m.cmd;
    if (tag !== undefined) this.out.push({ type: "reply", tag, ok: true, result });
  }
  check(json) { if (this.refuse) throw new Error(this.refuse); }
  saveNow() { this.saved = (this.saved ?? 0) + 1; }
  boot(model, rom, name, kept) {
    this.model = model;
    this.booted = (this.booted ?? 0) + 1;
    this.out.push({ type: "status", model }, { type: "frame", n: 1 });
    return { model, romName: name, ...(kept ? { restored: true } : {}) };
  }
  timer() {}
  deadline() { return undefined; }
  drain() { const o = this.out; this.out = []; return o; }
  free() { this.freed = true; }
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

const { Hub, SERIAL_SHARED } = await import("../hub.js");
const { CALC_STOPPED, Heartbeat, MIXED_BUILDS } = await import("../shared/calc-hub.js");
const { stateFormatOf, tooNew } = await import("../shared/calc-states.js");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Web Locks enough for the hub: exclusive, granted in order, released when the callback's promise settles; `ifAvailable` gets null when held. */
class FakeLocks {
  constructor() {
    this.held = new Map();
  }
  request(name, opts, cb) {
    if (typeof opts === "function") [cb, opts] = [opts, {}];
    if (opts.ifAvailable && this.held.get(name)?.length) return Promise.resolve(cb(null));
    return new Promise((resolve) => {
      const q = this.held.get(name) ?? [];
      this.held.set(name, q);
      const run = () => Promise.resolve(cb({ name })).then(resolve).finally(() => {
        q.shift();
        q[0]?.();
      });
      q.push(run);
      if (q.length === 1) run();
    });
  }
}

/** A hub with in-memory states; `page(who)` connects a page and joins. */
function setup({ locks = new FakeLocks(), stopTries } = {}) {
  const saved = new Map();
  const hub = new Hub({
    locks,
    build: "b1",
    stopTries,
    states: {
      get: async (model) => saved.get(model)?.state ?? null,
      put: async (model, rec) => saved.set(model, rec),
      delete: async (model) => saved.delete(model),
    },
  });
  let nextId = 0;
  const page = async (who, { build = "b1" } = {}) => {
    const got = [];
    const conn = hub.connect({ postMessage: (m) => got.push(m) });
    const send = async (cmd, args = {}) => {
      const id = ++nextId;
      await hub.receive(conn, { v: 1, id, cmd, ...args });
      await sleep(1);
      return got.find((m) => m.type === "reply" && m.id === id);
    };
    let dies = () => {};
    const lock = `page-${nextId + 1}`;
    locks.request(lock, () => new Promise((r) => {
      dies = r;
    }));
    const joined = await send("join", { who, build, lock });
    return { conn, got, send, joined, die: () => dies(), events: (type) => got.filter((m) => m.type === type) };
  };
  return { hub, saved, page, locks };
}

const ROM = new Uint8Array(4);
const engineOf = (hub, model) => hub.machines.get(model);
const cmds = (e, cmd) => e.host.log.filter((m) => m.cmd === cmd);

test("two pages on one model share its engine: booted once, replies to the asker, frames to both", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  assert.equal(emu.joined.ok, true);
  const a = await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(a.result.model, "48gx");
  const b = await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(b.result.attached, true, "running: shown, not booted again");
  const e = engineOf(hub, "48gx");
  assert.equal(e.host.booted, 1);
  assert.deepEqual(tx.events("frame"), [{ type: "frame", n: 1 }], "the frame it missed");
  assert.equal(tx.events("status").at(-1).model, "48gx");
  // A reply goes to the asker only; events to both.
  const r = await tx.send("stack");
  assert.equal(r.result, "stack");
  assert.equal(emu.got.filter((m) => m.type === "reply" && m.result === "stack").length, 0);
  e.host.out.push({ type: "frame", n: 2 });
  await emu.send("stats");
  assert.equal(emu.events("frame").at(-1).n, 2);
  assert.equal(tx.events("frame").at(-1).n, 2);
});

test("the serial port of a shared calculator is refused in plain words; a page alone on its engine opens it", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  assert.equal((await emu.send("serialOpen")).ok, true, "alone: passed to the engine");
  assert.equal(cmds(e, "serialOpen").length, 1);
  const tx = await page("tx");
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(e.host.serialHeld, false, "shared now: the port the lone page opened is let go");
  for (const p of [emu, tx]) {
    for (const [cmd, args] of [["serialOpen", {}], ["serialWrite", { bytes: new Uint8Array([1]) }]]) {
      const r = await p.send(cmd, args);
      assert.deepEqual([r.ok, r.error], [false, SERIAL_SHARED], cmd);
    }
    assert.equal((await p.send("serialClose")).ok, true, "serialClose is answered");
  }
  assert.equal(cmds(e, "serialOpen").length, 1, "nothing more reached the engine");
  assert.equal(cmds(e, "serialWrite").length, 0);
  // Its own ROM (`boot`): its home, alone.
  const own = await page("tx");
  await own.send("boot", { model: "48gx", rom: ROM, romName: "own" });
  assert.equal((await own.send("serialOpen")).ok, true);
});

test("the emulator pages hear peers while a saturnus tx page is on their calculator, and a page that comes later too", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(emu.events("peers").length, 0, "a page that never meets saturnus tx hears nothing");
  const tx = await page("tx");
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.deepEqual(emu.events("peers").at(-1), { type: "peers", tx: true });
  assert.equal(tx.events("peers").length, 0, "tx hears none");
  const later = await page("emulator");
  await later.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.deepEqual(later.events("peers").at(-1), { type: "peers", tx: true }, "a page that comes later");
  const before = emu.events("peers").length;
  await emu.send("hello");
  assert.deepEqual(emu.events("peers").slice(before), [{ type: "peers", tx: true }], "again after a reload's hello");
  // Another model shown: no tx there.
  await later.send("attach", { model: "49g", rom: ROM, romName: "g" });
  assert.deepEqual(later.events("peers").at(-1), { type: "peers", tx: false });
  await tx.send("leave");
  assert.deepEqual(emu.events("peers").at(-1), { type: "peers", tx: false });
  assert.deepEqual(later.events("peers").at(-1), { type: "peers", tx: false });
  // A tx page dropped by a fresh start: the emulator hears so.
  const tx2 = await page("tx");
  await tx2.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(emu.events("peers").at(-1).tx, true);
  await hub.bootKept(emu.conn, "48gx", ROM, "gx", true);
  assert.equal(tx2.events("gone").length, 1);
  assert.equal(emu.events("peers").at(-1).tx, false);
});

test("the speed is the emulator page's, whatever saturnus tx asks; tx's only with no emulator page", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  await emu.send("setSpeed", { speed: "2" });
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  assert.equal(cmds(e, "setSpeed").at(-1).speed, "2", "the page's speed came along");
  const tx = await page("tx");
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("setSpeed", { speed: "max" });
  assert.equal(e.sentSpeed, "2");
  // The emulator page closes: tx's speed alone.
  await emu.send("leave");
  assert.equal(e.sentSpeed, "max");
  assert.equal(cmds(e, "setSpeed").at(-1).speed, "max");
});

test("hidden only when every page is; memoryChanged to the pages that watch", async () => {
  const { hub, page } = setup();
  const a = await page("emulator");
  const b = await page("emulator");
  await a.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await b.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  await a.send("visibility", { hidden: true });
  assert.equal(cmds(e, "visibility").at(-1).hidden, false, "b still shows it");
  await b.send("visibility", { hidden: true });
  assert.equal(cmds(e, "visibility").at(-1).hidden, true);
  await a.send("watchMemory", { on: true });
  assert.equal(cmds(e, "watchMemory").at(-1).on, true);
  await b.send("watchMemory", { on: false });
  assert.equal(cmds(e, "watchMemory").at(-1).on, true, "a still watches");
  e.host.out.push({ type: "memoryChanged" });
  await a.send("stats");
  assert.equal(a.events("memoryChanged").length, 1);
  assert.equal(b.events("memoryChanged").length, 0);
});

test("a page that leaves sends nothing to the calculator; the last one has it saved and stopped", async () => {
  const { hub, page, saved } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  const before = e.host.log.length;
  assert.equal((await tx.send("leave")).ok, true);
  assert.deepEqual(e.host.log.slice(before).map((m) => m.cmd).filter((c) => c !== "setSpeed"), [], "no serialClose, no endServer");
  assert.equal(engineOf(hub, "48gx"), e, "the emulator page still shows it");
  // The emulator page's tab closes (its lock goes): saved, stopped.
  e.host.owed = 7;
  emu.die();
  await sleep(20);
  await hub.queue;
  assert.equal(cmds(e, "unload").length, 1);
  assert.deepEqual([...saved.get("48gx").state], [7], "saved on the way out");
  assert.equal(engineOf(hub, "48gx"), undefined);
  assert.equal(e.host.freed, true);
  // Booted again, it restores that state.
  const again = await page("tx");
  const r = await again.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(r.result.restored, true);
  assert.equal(cmds(engineOf(hub, "48gx"), "endServer").length, 1, "a restored state's server is ended");
});

test("unload with another page on it: this page lets go, the machine runs on", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  await emu.send("unload");
  assert.equal(cmds(e, "unload").length, 0);
  assert.equal(emu.events("status").at(-1).model, null, "the page shows no calculator");
  assert.equal(engineOf(hub, "48gx"), e);
  assert.equal((await tx.send("stack")).result, "stack");
});

test("a fresh start of a running calculator tells saturnus tx it is gone", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await hub.receive(emu.conn, { v: 1, cmd: "noop" });
  await hub.bootKept(emu.conn, "48gx", ROM, "gx", true);
  assert.equal(tx.events("gone").length, 1);
  assert.equal(engineOf(hub, "48gx").host.booted, 2);
  assert.notEqual(tx.conn.machine, engineOf(hub, "48gx"));
});

test("a page of another build is refused, and so is everything it sends after", async () => {
  const { page } = setup();
  const old = await page("tx", { build: "b0" });
  assert.deepEqual([old.joined.ok, old.joined.error], [false, MIXED_BUILDS]);
  const r = await old.send("hello");
  assert.deepEqual([r.ok, r.error], [false, MIXED_BUILDS]);
});

test("hello from a page that shares its machine leaves the other page's watch alone", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("watchMemory", { on: true });
  const e = engineOf(hub, "48gx");
  const r = await emu.send("hello");
  assert.equal(r.result.protocol, 1);
  assert.equal(cmds(e, "hello").length, 0, "not the shared engine's hello");
  assert.equal(cmds(e, "watchMemory").at(-1).on, true, "tx still watches");
  assert.equal(emu.events("frame").length >= 1, true, "the frame again");
});

test("another ROM of a running model boots it again for every page, the running machine kept first", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  const r = await emu.send("attach", { model: "48gx", rom: new Uint8Array([1, 2, 3, 4]), romName: "other" });
  assert.equal(r.result.attached, undefined, "booted, not shown");
  assert.equal(e.host.booted, 2);
  assert.equal(e.host.saved, 1, "saved before");
  assert.equal(tx.events("gone").length, 1);
});

test("a fresh start refused (a send types) drops nothing and deletes nothing", async () => {
  const { hub, page, saved } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  saved.set("48gx", { state: new Uint8Array([1]) });
  const e = engineOf(hub, "48gx");
  e.host.refuse = "typing is in progress (releaseAll stops it)";
  await assert.rejects(hub.bootKept(emu.conn, "48gx", ROM, "gx", true), /typing is in progress/);
  assert.equal(tx.events("gone").length, 0);
  assert.equal(saved.has("48gx"), true);
});

test("a page refused at join is not kept", async () => {
  const { hub, page } = setup();
  const before = hub.conns.size;
  await page("tx", { build: "b0" });
  assert.equal(hub.conns.size, before);
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

test("a page the hub took for gone is answered, never left waiting; a ping is answered at once", async () => {
  const { hub, page } = setup();
  const tx = await page("tx");
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal((await tx.send("ping")).ok, true);
  await tx.send("leave");
  const r = await tx.send("stack");
  assert.deepEqual([r.ok, r.error], [false, CALC_STOPPED]);
  const p = await tx.send("ping");
  assert.deepEqual([p.ok, p.error], [false, CALC_STOPPED]);
  // A ping does not wait behind the queue.
  const emu = await page("emulator");
  let release;
  hub.serial(() => new Promise((r) => { release = r; }));
  const got = [];
  hub.receive(emu.conn, { v: 1, id: "ping:1", cmd: "ping" });
  hub.receive(emu.conn, { v: 1, id: "q", cmd: "stats" });
  await sleep(5);
  got.push(...emu.got.filter((m) => m.type === "reply").map((m) => m.id));
  assert.deepEqual(got.filter((id) => id === "ping:1" || id === "q"), ["ping:1"], "the ping before the queued stats");
  release();
});

test("a failed engine (a wasm panic) is dropped unsaved and every page on it hears so", async () => {
  const { hub, page, saved } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  e.host.owed = 3;
  e.host.panic = true;
  const r = await emu.send("stack");
  assert.deepEqual([r.ok, r.error], [false, CALC_STOPPED]);
  assert.equal(engineOf(hub, "48gx"), undefined, "dropped");
  assert.equal(e.host.freed, true);
  assert.equal(saved.has("48gx"), false, "not saved");
  assert.equal(tx.events("gone").at(-1).message, CALC_STOPPED);
  assert.equal(emu.events("status").at(-1).model, null);
  assert.equal(emu.events("error").at(-1).message, CALC_STOPPED);
  // The pages go on: booted again, a new engine.
  e.host.panic = false;
  const again = await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(again.ok, true);
  assert.notEqual(engineOf(hub, "48gx"), e);
});

test("a page whose own home engine failed gets a new one at its next command", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  await emu.send("stats");
  const home = emu.conn.home;
  home.host.panic = true;
  const r = await emu.send("stats");
  assert.deepEqual([r.ok, r.error], [false, CALC_STOPPED]);
  assert.equal(emu.conn.home, null);
  assert.equal((await emu.send("stats")).ok, true);
  assert.notEqual(emu.conn.home, home);
  assert.equal(hub.conns.has(emu.conn), true);
});

test("a stop that waits for a send to end does not hold the other calculators' commands", async () => {
  const { hub, page, saved } = setup();
  const a = await page("emulator");
  const b = await page("emulator");
  await a.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await b.send("attach", { model: "49g", rom: ROM, romName: "49" });
  const e = engineOf(hub, "48gx");
  e.host.owed = 5;
  e.host.refuseUnload = 3;
  const t0 = Date.now();
  // The page leaves (its tab closed): the stop is the hub's.
  assert.equal((await a.send("leave")).ok, true);
  assert.equal((await b.send("stats")).ok, true);
  assert.ok(Date.now() - t0 < 200, "the 49G's command did not wait for the 48GX's stop");
  assert.equal(e.stopped, false, "still stopping");
  await sleep(1200);
  assert.equal(e.stopped, true, "stopped once the send ended");
  assert.deepEqual([...saved.get("48gx").state], [5], "and saved");
});

test("a stop waiting for a send is called off when a page shows the calculator again, which keeps its state again", async () => {
  const { hub, page } = setup();
  const a = await page("emulator");
  await a.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  e.host.refuseUnload = 100;
  await a.send("leave");
  assert.equal(e.stopping, true);
  hub.forgotten.add("48gx");
  const b = await page("emulator");
  const r = await b.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(hub.forgotten.has("48gx"), false, "auto-saved again");
  assert.equal(r.result.attached, true, "the running one, shown again");
  await sleep(400);
  assert.equal(e.stopped, false);
  assert.equal(e.stopping, false);
  e.host.refuseUnload = 0;
});

test("a boot refused (a send types) leaves the page on the calculator it showed", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "49g", rom: ROM, romName: "49" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const g = engineOf(hub, "49g");
  g.host.refuse = "typing is in progress (releaseAll stops it)";
  const r = await tx.send("attach", { model: "49g", rom: new Uint8Array([9]), romName: "other" });
  assert.equal(r.ok, false);
  assert.equal(tx.conn.machine, engineOf(hub, "48gx"), "still on the 48GX");
});

test("two hubs (two tabs' own Workers): the model's auto-save lock lets one keep its state, the other says so", async () => {
  const locks = new FakeLocks();
  const one = setup({ locks });
  const two = setup({ locks });
  const a = await one.page("emulator");
  const b = await two.page("emulator");
  await a.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await b.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal(a.events("autoSaveOff").length, 0);
  assert.match(b.events("autoSaveOff")[0].message, /HP 48GX also runs in another saturnus tab/);
  const eb = engineOf(two.hub, "48gx");
  assert.equal(eb.host.autoSave, false, "its engine saves nothing");
  eb.host.owed = 4;
  await b.send("visibility", { hidden: true });
  await two.hub.keeping;
  assert.equal(two.saved.has("48gx"), false);
  // The first tab's machine stops: its lock goes, and the waiting machine takes it.
  await a.send("unload");
  await sleep(5);
  assert.equal(b.events("autoSaveOn").length, 1);
  assert.equal(eb.host.autoSave, true);
  assert.equal(eb.autoSaveOff, null);
  await b.send("unload");
  await two.hub.keeping;
  assert.deepEqual([...two.saved.get("48gx").state], [4], "saved now");
});

test("a page's own unload while a send types is refused in the engine's words; nothing is stopped", async () => {
  const { hub, page } = setup();
  const a = await page("emulator");
  await a.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  e.host.refuseUnload = 1;
  const r = await a.send("unload");
  assert.deepEqual([r.ok, r.error], [false, "typing is in progress (releaseAll stops it)"]);
  assert.equal(e.stopped || e.stopping, false);
  assert.equal((await a.send("unload")).ok, true);
  assert.equal(e.stopped, true);
});

test("a stop given up (the send never ends) drops the machine and its pages show no calculator", async () => {
  const { hub, page } = setup({ stopTries: 2 });
  const a = await page("emulator");
  await a.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  e.host.refuseUnload = 1000;
  // Remove ROMs in another tab: its stop waits for the send, then gives up.
  await hub.serial(() => hub.forgetKept(null, "48gx"));
  assert.equal(e.stopping, true);
  await sleep(700);
  assert.equal(e.stopped, true);
  assert.equal(a.events("status").at(-1).model, null);
});
