// One live calculator per model, used by the emulator and saturnus tx at
// once (iteration 46: the calculators' SharedWorker, web/hub.js), in two
// tabs of one headless Chrome profile. The emulator runs the 48GX
// (gxrom-r from SATURNUS_ROM_DIR) and stores a variable; saturnus tx adds
// the emulated 48GX of the kept ROM, which is that running calculator:
// the variable is listed at once, and the emulator goes on showing it
// (frames while tx works). A copy from tx is in the emulator's memory at
// once. tx's tab closed while it serves: the emulator's calculator runs
// on, out of SERVER. tx again, then the emulator's tab closed: tx goes on
// copying. Phones, background tabs and Safari are the owner's checks
// (kb/iterations/iteration-46-shared-worker.md). Skipped without the
// ROM, Chrome or the wasm packages; SATURNUS_AUDIT=1 makes the skip a
// failure.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { session, sleep } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

const dir = process.env.SATURNUS_ROM_DIR;
const ROM = "gxrom-r";
/** How long a step on the emulated calculator may take. */
const STEP_MS = 120_000;

async function until(ev, expression, ms, what) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await ev(expression).catch(() => null);
    if (v) return v;
    if (Date.now() > end) assert.fail(`timed out: ${what}`);
    await sleep(100);
  }
}

/** HOME's names in the emulator page's calculator. */
const EMU_HOME = "window.saturnus.backend.memoryTree().then((t) => t.variables.map((v) => v.name))";

/** saturnus tx in a new tab of `emu`'s profile, its left pane on the kept 48GX; resolves to the tab, its helpers and the ms until it was listed. */
async function txOnKept(emu) {
  const tx = await emu.another();
  await tx.send("Page.enable");
  await tx.send("Runtime.enable");
  const p = txPage(tx);
  await p.open(emu.port);
  await tx.ev(`document.querySelector('[data-action="add-emulated"]').click(); true`);
  await until(tx.ev, `document.getElementById("emulated-dialog").open && !!document.querySelector("#emulated-roms .rom-choice")`, 10_000, "the kept 48GX offered");
  const start = Date.now();
  await tx.ev(`document.querySelector("#emulated-roms .rom-choice").click(); true`);
  await p.until("!!tx.panes[0].place?.device?.connection", STEP_MS, "tx connected to the shared 48GX");
  await p.idle(STEP_MS, "HOME listed in tx");
  return { tx, p, readyMs: Date.now() - start };
}

test("the emulator and saturnus tx use the 48GX at once", { timeout: 600_000 }, async (t) => {
  const why = !dir || !existsSync(join(dir, ROM)) ? `no ${ROM} in SATURNUS_ROM_DIR` : txBuilt() ? null : "web/pkg or web/tx/pkg not built (just web tx-web)";
  if (why) {
    if (process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return;
  }
  const emu = await session(t, { files: { "/__rom": join(dir, ROM) }, audit: true });
  if (!emu) return;
  await emu.send("Page.enable");
  await emu.send("Runtime.enable");
  await emu.send("Page.navigate", { url: `http://127.0.0.1:${emu.port}/index.html` });
  await until(emu.ev, "!!window.saturnus", 30_000, "the emulator started");
  await emu.ev("window.saturnus.started");
  assert.equal(await emu.ev("window.saturnus.backend.shared"), true, "the emulator runs in the SharedWorker");
  await emu.ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], ${JSON.stringify(ROM)}));
    return true;
  })()`);
  await until(emu.ev, "window.saturnus.store.state.booted === '48gx'", 30_000, "the emulator runs the 48GX");
  await until(emu.ev, "window.saturnus.backend.stats().then((s) => s.emulatedMs > 3000 && s.loop === 'sleep')", 60_000, "the 48GX is up");
  const stored = await emu.ev("window.saturnus.backend.run(\"41 'FROMEMU' STO\")");
  assert.equal(stored.error ?? null, null, "stored in the emulator");
  // The frames the emulator page gets from here on.
  await emu.ev("window.__frames = 0; window.saturnus.backend.addEventListener('frame', () => window.__frames++); true");

  // saturnus tx on the same running calculator: the variable is there at once.
  const first = await txOnKept(emu);
  t.diagnostic(`tx ready on the running 48GX (connected and HOME listed): ${first.readyMs} ms`);
  assert.ok((await first.p.names()).includes("FROMEMU"), "the emulator's variable is listed in tx");
  // The emulator still shows it: no handover, and it saw what tx did (SERVER on its screen).
  assert.equal(await emu.ev("window.saturnus.store.state.booted"), "48gx", "the emulator still runs it");
  assert.ok((await emu.ev("window.__frames")) > 0, "the emulator's screen followed tx's work");
  assert.equal(await emu.ev("document.querySelector('.no-rom')?.hidden ?? true"), true, "no empty state in the emulator");

  // A copy from tx is in the emulator's memory at once.
  await first.p.drop({ "FROMTX.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 6 7 * \\>>\r\n" });
  await first.p.idle(STEP_MS, "FROMTX put");
  assert.ok((await first.p.names()).includes("FROMTX"));
  assert.ok((await emu.ev(EMU_HOME)).includes("FROMTX"), "tx's copy is in the emulator's memory");
  assert.deepEqual(first.tx.errors, [], "no error in tx");

  // tx's tab closed while it serves: the emulator's calculator runs on, out of SERVER.
  await first.tx.send("Page.close");
  let ran = null;
  for (let i = 0; i < 50; i++) {
    // The hub ends the server once it learns the tab is gone.
    await until(emu.ev, "window.saturnus.backend.stats().then((s) => s.loop === 'sleep')", 30_000, "the 48GX settled");
    ran = await emu.ev("window.saturnus.backend.run('FROMEMU 1 +')").catch((e) => ({ error: String(e) }));
    if (!ran?.error) break;
    await sleep(200);
  }
  assert.equal(ran?.error ?? null, null, "a typed line runs in the emulator");
  assert.equal((await emu.ev("window.saturnus.backend.stackTop()")).level1?.value, 42, "at its stack, not in SERVER");

  // tx again, then the emulator's tab closed: tx goes on.
  const second = await txOnKept(emu);
  t.diagnostic(`tx ready again on the running 48GX: ${second.readyMs} ms`);
  await emu.send("Page.close");
  await sleep(500);
  await second.p.drop({ "AFTER.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 1 \\>>\r\n" });
  await second.p.idle(STEP_MS, "AFTER put with the emulator closed");
  const names = await second.p.names();
  assert.ok(names.includes("AFTER") && names.includes("FROMTX") && names.includes("FROMEMU"), `tx's calculator ran on: ${names}`);
  assert.deepEqual(second.tx.errors, [], "no error in tx");
});
