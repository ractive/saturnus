// One calculator per model, shared by the emulator and saturnus tx
// (iteration 41), in two tabs of one headless Chrome profile: the
// emulator runs the 48GX (gxrom-r from SATURNUS_ROM_DIR) and stores a
// variable; saturnus tx adds the emulated 48GX from the kept ROM, which
// asks the emulator for it (the emulator saves, stops and shows "In use by
// saturnus tx"); tx sees the variable and puts a second one there; the
// emulator takes it back (tx leaves SERVER and lets go) and has both. The
// frozen tab, Android's tab discards and two builds side by side are the
// owner's checks (kb/iterations/iteration-41-shared-calculators.md).
// Skipped without the ROM, Chrome or the wasm packages; SATURNUS_AUDIT=1
// makes the skip a failure.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { session, sleep } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

const dir = process.env.SATURNUS_ROM_DIR;
const ROM = "gxrom-r";
/** How long a step on the emulated calculator may take at 1x. */
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

test("the emulator and saturnus tx share the 48GX, one at a time", { timeout: 600_000 }, async (t) => {
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
  await emu.ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], ${JSON.stringify(ROM)}));
    return true;
  })()`);
  await until(emu.ev, "window.saturnus.store.state.booted === '48gx'", 30_000, "the emulator runs the 48GX");
  // A variable made in the emulator: settled past the ROM's questions first.
  await until(emu.ev, "window.saturnus.backend.stats().then((s) => s.emulatedMs > 3000 && s.loop === 'sleep')", 60_000, "the 48GX is up");
  const stored = await emu.ev("window.saturnus.backend.run(\"41 'FROMEMU' STO\")");
  assert.equal(stored.error ?? null, null, "stored in the emulator");
  assert.ok((await emu.ev(EMU_HOME)).includes("FROMEMU"));

  // saturnus tx in a second tab of the same profile: the emulated 48GX of the kept ROM.
  const tx = await emu.another();
  await tx.send("Page.enable");
  await tx.send("Runtime.enable");
  const p = txPage(tx);
  await p.open(emu.port);
  await tx.ev(`document.querySelector('[data-action="add-emulated"]').click(); true`);
  await until(tx.ev, `document.getElementById("emulated-dialog").open && !!document.querySelector("#emulated-roms .rom-choice")`, 10_000, "the kept 48GX offered");
  await tx.ev(`document.querySelector("#emulated-roms .rom-choice").click(); true`);
  await p.until("!!tx.panes[0].place?.device?.connection", 240_000, "tx connected to the shared 48GX");
  await p.idle(STEP_MS, "HOME listed in tx");

  // The emulator let go of it, saved, and says where it is.
  await until(emu.ev, "window.saturnus.store.state.away?.by === 'tx' && !window.saturnus.store.state.booted", 10_000, "the emulator shows it as away");
  assert.equal(await emu.ev("document.querySelector('.no-rom-text').textContent"), "In use by saturnus tx.");
  assert.equal(await emu.ev("!document.querySelector('.no-rom-take').hidden"), true, "Take it back offered");
  assert.ok((await p.names()).includes("FROMEMU"), "the emulator's variable came across");

  // A variable copied there in tx.
  await p.drop({ "FROMTX.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 6 7 * \\>>\r\n" });
  await p.idle(STEP_MS, "FROMTX put");
  assert.ok((await p.names()).includes("FROMTX"));

  // Taken back in the emulator: tx leaves SERVER and lets go; the emulator has both.
  await emu.ev("document.querySelector('.no-rom-take').click(); true");
  await until(emu.ev, "window.saturnus.store.state.booted === '48gx' && !window.saturnus.store.state.away", 60_000, "the emulator took it back");
  await until(tx.ev, "!!tx.panes[0].away && !tx.panes[0].place", 10_000, "tx shows it as away");
  assert.deepEqual(
    await tx.ev(`[document.querySelector("#pane-title-left").textContent, document.querySelector('[data-pane="left"] .switch-name').textContent]`),
    ["Emulated 48GX", "Emulated 48GX"],
    "the pane and the switch still name the calculator that is away",
  );
  await until(emu.ev, "window.saturnus.backend.stats().then((s) => s.loop === 'sleep')", 30_000, "the 48GX settled");
  const names = await emu.ev(EMU_HOME);
  assert.ok(names.includes("FROMEMU") && names.includes("FROMTX"), `both variables in the emulator: ${names}`);
  // Out of server mode: a line typed now runs as on the stack.
  const r = await emu.ev("window.saturnus.backend.run('FROMEMU')");
  assert.equal(r.error ?? null, null);
  const top = await emu.ev("window.saturnus.backend.stackTop()");
  assert.equal(top.level1?.value, 41, "the calculator runs a typed line: not waiting in SERVER");
});
