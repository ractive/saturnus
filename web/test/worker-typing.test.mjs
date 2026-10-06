// The Web Worker's typing commands against a fake wasm core: `node --test
// web/test/` (just web-test). A send in progress refuses key commands and
// a boot, and text must be a string.

import assert from "node:assert/strict";
import { register } from "node:module";
import { test } from "node:test";

// worker.js imports the wasm bindings from ./pkg/; give it a fake instead.
const fake = `
export const calls = [];
export const job = { done: false };
export default async function init() {}
export const layout = () => ({});
export const skin = () => ({});
export const model_for = (rom, model) => model;
export const model_names = () => ["48sx"];
class Core {
  constructor(model) { this.m = model; }
  model() { return this.m; }
  idle_ms() { return 1000; }
  keys_busy() { return false; }
  run_slice(ms) { return ms; }
  take_errors() { return []; }
  take_keys() { return undefined; }
  take_frame() { return undefined; }
  has_key() { return true; }
  start_typing(verb, text) { calls.push(["start_typing", verb, text]); job.done = false; return true; }
  typing_step() { return job.done; }
  typing_result() { return { typed: 1 }; }
  stop_typing() { calls.push(["stop_typing"]); }
}
export const Emulator = new Proxy(Core, {
  construct(target, args) {
    const core = new target(...args);
    return new Proxy(core, {
      get(t, name) {
        if (name in t) return t[name].bind(t);
        return (...a) => { calls.push([name, ...a]); return 0; };
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

test("a send in progress refuses keys and a boot; text must be a string", async () => {
  assert.ok((await send({ cmd: "boot", model: "48sx", rom: new Uint8Array(4) })).ok);

  const bad = await send({ cmd: "insert" });
  assert.equal(bad.ok, false);
  assert.match(bad.error, /"text"/);
  assert.equal(pkg.calls.filter((c) => c[0] === "start_typing").length, 0, "nothing typed");

  const typed = send({ cmd: "insert", text: "« 1 2 + » EVAL" });
  await new Promise((r) => setTimeout(r, 5));
  assert.deepEqual(pkg.calls.find((c) => c[0] === "start_typing"), ["start_typing", "insert", "« 1 2 + » EVAL"]);

  const letter = await send({ cmd: "typeLetter", letter: "A" });
  assert.equal(letter.ok, false);
  assert.match(letter.error, /typing is in progress/);
  const boot = await send({ cmd: "boot", model: "48sx", rom: new Uint8Array(4) });
  assert.equal(boot.ok, false);
  assert.match(boot.error, /typing is in progress/);
  assert.ok((await send({ cmd: "keyUpAll" })).ok);
  const keyCalls = pkg.calls.filter((c) => ["type_letter", "release_held", "pump"].includes(c[0]));
  assert.deepEqual(keyCalls, [], "no key reached the machine during the send");

  pkg.job.done = true;
  const r = await typed;
  assert.equal(r.ok, true);
  assert.deepEqual(r.result, { typed: 1 });
  // Afterwards the keys work again.
  assert.ok((await send({ cmd: "typeLetter", letter: "A" })).ok);
  // Stop the run loop's timers so the process can end.
  await send({ cmd: "pause", paused: true });
});
