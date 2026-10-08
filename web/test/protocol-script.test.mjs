// The cross-host test, Worker side: web/test/protocol-script.json through
// worker.js and the real wasm (web/pkg, `just web`), on a 48SX with a ROM
// of zeros; the transcript must be protocol-script.expected.json, which
// the state machine and the native runner also give
// (crates/saturnus-drive/tests/protocol_script.rs). `node --test
// web/test/` (just web-test).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const pkg = await import("../pkg/saturnus_web.js");
pkg.initSync({ module: readFileSync(new URL("../pkg/saturnus_web_bg.wasm", import.meta.url)) });

const posted = [];
globalThis.self = { postMessage: (m) => posted.push(m) };
await import("../worker.js");

const script = JSON.parse(readFileSync(new URL("./protocol-script.json", import.meta.url), "utf8"));
const expected = JSON.parse(readFileSync(new URL("./protocol-script.expected.json", import.meta.url), "utf8"));

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function until(ok, what) {
  for (let i = 0; i < 3000 && !ok(); i++) await sleep(2);
  assert.ok(ok(), `waited for ${what}`);
}

test("the command script gives the transcript every host gives", async () => {
  const steps = script.steps;
  let saved = null;
  const waiting = [];
  for (const [i, step] of steps.entries()) {
    const msg = { v: 1, ...step.msg };
    if (step.rom === "zeros") Object.assign(msg, { rom: new Uint8Array(256 * 1024), romName: "zeros" });
    if (step.state === "saved") msg.state = saved;
    if (!step.noId) msg.id = i;
    self.onmessage({ data: msg });
    if (step.noId) continue;
    waiting.push(i);
    if (step.pipe) continue;
    await until(() => waiting.every((j) => posted.some((m) => m.type === "reply" && m.id === j)), `the reply to step ${i}`);
    waiting.length = 0;
    const r = posted.findLast((m) => m.type === "reply" && m.id === i);
    if (r.ok && r.result?.state instanceof Uint8Array) saved = r.result.state;
  }
  const transcript = posted.map((m) => {
    if (m.type !== "reply") return m;
    const out = { reply: m.id, ok: m.ok };
    if (!m.ok) return { ...out, error: m.error };
    let result = m.result;
    if (result && typeof result === "object" && !Array.isArray(result)) {
      result = { ...result };
      for (const f of steps[m.id].any ?? []) if (f in result) result[f] = "*";
    }
    return { ...out, result };
  });
  assert.deepEqual(transcript, expected);
  assert.ok(saved instanceof Uint8Array && saved.length > 1000, "saveState's state is bytes");
});
