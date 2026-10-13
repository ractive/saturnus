// The calculators' hub (web/hub.js, iteration 46): its bookkeeping of the
// pages connected to one live calculator per model, against a fake of the
// wasm bindings' `Host` (one per engine), fake ports, fake Web Locks and
// an in-memory store of the auto-saved states: `node --test web/test/`
// (just web-test). Two pages on one model share its engine; replies go
// to the asker, frames to every page, the serial line's bytes to its
// holder, `memoryChanged` to the pages that watch; one holder of the
// line; the speed is the emulator page's but for saturnus tx's transfers;
// a machine is hidden only when every page is; a page that leaves (by
// message or by its lock) lets the line go with its server ended, and
// the last one has the machine saved and stopped; another build is
// refused. The pages and the real engine together:
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
  check() {}
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

const { Hub, LINE_TAKEN } = await import("../hub.js");
const { MIXED_BUILDS } = await import("../shared/calc-hub.js");
const { stateFormatOf, tooNew } = await import("../shared/calc-states.js");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Web Locks enough for the hub: exclusive, granted in order, released when the callback's promise settles. */
class FakeLocks {
  constructor() {
    this.held = new Map();
  }
  request(name, cb) {
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
function setup({ lineIdleMs = 40 } = {}) {
  const saved = new Map();
  const locks = new FakeLocks();
  const hub = new Hub({
    locks,
    build: "b1",
    lineIdleMs,
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
  return { hub, saved, page };
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

test("one holder of the serial line: the other page is refused, the bytes go to the holder, a write refused names saturnus tx", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  assert.equal((await tx.send("serialOpen")).ok, true);
  const refused = await emu.send("serialOpen");
  assert.deepEqual([refused.ok, refused.error], [false, LINE_TAKEN]);
  assert.equal((await emu.send("serialWrite", { bytes: new Uint8Array([1]) })).ok, false);
  const e = engineOf(hub, "48gx");
  e.host.out.push({ type: "serial", bytes: new Uint8Array([9]) });
  await tx.send("stats");
  assert.equal(tx.events("serial").length, 1);
  assert.equal(emu.events("serial").length, 0);
  const w = await emu.send("storeFile", { dir: ["HOME"], name: "A", data: new Uint8Array([1]) });
  assert.equal(w.error, LINE_TAKEN, "the emulator page hears who holds it");
  // The emulator's serialClose does not take tx's line.
  await emu.send("serialClose");
  assert.equal(e.holder, tx.conn);
});

test("the speed is the emulator page's; saturnus tx's only while it moves bytes, or with no emulator page", async () => {
  const { hub, page } = setup({ lineIdleMs: 40 });
  const emu = await page("emulator");
  await emu.send("setSpeed", { speed: "2" });
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  const e = engineOf(hub, "48gx");
  assert.equal(cmds(e, "setSpeed").at(-1).speed, "2", "the page's speed came along");
  const tx = await page("tx");
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("setSpeed", { speed: "max" });
  assert.equal(e.sentSpeed, "2", "not before it holds the line");
  await tx.send("serialOpen");
  await tx.send("serialWrite", { bytes: new Uint8Array([1]) });
  assert.equal(e.sentSpeed, "max", "a transfer at tx's speed");
  await sleep(80);
  assert.equal(e.sentSpeed, "2", "back to the emulator's once the line is quiet");
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

test("a page that leaves lets the line go with its server ended; the last one has it saved and stopped", async () => {
  const { hub, page, saved } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("serialOpen");
  const e = engineOf(hub, "48gx");
  const before = e.host.log.length;
  assert.equal((await tx.send("leave")).ok, true);
  assert.deepEqual(e.host.log.slice(before).map((m) => m.cmd).filter((c) => c !== "setSpeed"), ["serialClose", "endServer"]);
  assert.equal(e.holder, null);
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

test("hello from a page that shares its machine leaves the other page's line and watch alone", async () => {
  const { hub, page } = setup();
  const emu = await page("emulator");
  const tx = await page("tx");
  await emu.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("attach", { model: "48gx", rom: ROM, romName: "gx" });
  await tx.send("serialOpen");
  await emu.send("watchMemory", { on: true });
  const e = engineOf(hub, "48gx");
  const r = await emu.send("hello");
  assert.equal(r.result.protocol, 1);
  assert.equal(cmds(e, "hello").length, 0, "not the shared engine's hello");
  assert.equal(e.holder, tx.conn);
  assert.equal(emu.events("frame").length >= 1, true, "the frame again");
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
