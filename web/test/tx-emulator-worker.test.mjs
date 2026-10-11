// The transfer page's emulated-calculator Worker (web/tx/emulator-worker.js)
// with the real wasm package (web/pkg, `just web`; skipped without it):
// it answers the protocol, keeps nothing (no autoSave messages), and a
// command that throws inside it is that command's error and a `fatal`
// message, not silence.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

const pkgFile = new URL("../pkg/saturnus_web_bg.wasm", import.meta.url);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

test("a command that throws in the Worker answers with an error and a fatal message", { timeout: 30_000 }, async (t) => {
  if (!existsSync(pkgFile)) {
    t.skip("no web/pkg (just web)");
    return;
  }
  const pkg = await import("../pkg/saturnus_web.js");
  pkg.initSync({ module: readFileSync(pkgFile) });
  const posted = [];
  globalThis.self = { postMessage: (m) => posted.push(m) };
  await import("../tx/emulator-worker.js");
  const reply = async (id) => {
    for (let i = 0; i < 200; i++) {
      const r = posted.find((m) => m.type === "reply" && m.id === id);
      if (r) return r;
      await sleep(10);
    }
    assert.fail(`no reply to ${id}`);
  };
  self.onmessage({ data: { v: 1, id: 1, cmd: "hello" } });
  assert.equal((await reply(1)).ok, true);
  // Bytes that cannot be bytes: the Worker's own step throws.
  self.onmessage({ data: { v: 1, id: 2, cmd: "serialWrite", bytes: 10n } });
  const r = await reply(2);
  assert.equal(r.ok, false);
  assert.match(r.error, /the emulated calculator failed/);
  assert.ok(posted.some((m) => m.type === "fatal" && /failed/.test(m.message)), "fatal posted");
  // And the next command is still answered.
  self.onmessage({ data: { v: 1, id: 3, cmd: "hello" } });
  assert.equal((await reply(3)).ok, true);
  assert.ok(!posted.some((m) => m.type === "autoSave"));
});
