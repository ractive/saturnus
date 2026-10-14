// One live calculator per model, used by the emulator and saturnus tx at
// once (iteration 46: the calculators' SharedWorker, web/hub.js), in two
// tabs of one headless Chrome profile; tx works it through the engine's
// memory commands (iteration 47, web/shared/hub-device.js), never its
// serial port. The emulator runs the 48GX (gxrom-r from SATURNUS_ROM_DIR)
// and stores a variable; saturnus tx adds the emulated 48GX of the kept
// ROM, which is that running calculator: nothing is typed on it (no
// SERVER, no serialOpen in tx's traffic), the variable is listed at once,
// one typed in the emulator later shows without a refresh, the emulator
// shows "Connected to saturnus tx · Show". Copies go both ways (a drop
// into the calculator, a copy to a folder and back over the name). ATTN
// or an open command line in the emulator: tx is refused at once in
// words, never left waiting. A serialOpen from either page is refused.
// tx disconnects, or its tab closes: the chip goes, the emulator runs on.
// Phones, background tabs and Safari are the owner's checks. Skipped
// without the ROM, Chrome or the wasm packages; SATURNUS_AUDIT=1 makes
// the skip a failure.
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

/** saturnus tx in a new tab of `emu`'s profile, its left pane on the kept 48GX, the commands its page sends to the hub logged (`__sent`); resolves to the tab, its helpers and the ms until it was listed. */
async function txOnKept(emu) {
  const tx = await emu.another();
  await tx.send("Page.enable");
  await tx.send("Runtime.enable");
  // Every command the page sends to the calculators' hub, from the first.
  await tx.send("Page.addScriptToEvaluateOnNewDocument", { source: `window.__sent = []; const post = MessagePort.prototype.postMessage; MessagePort.prototype.postMessage = function (m, ...rest) { if (m && typeof m.cmd === "string") window.__sent.push(m.cmd); return post.call(this, m, ...rest); };` });
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

/** The emulator page on the running 48GX with FROMEMU stored, or null when skipped. */
async function emulatorOn48gx(t) {
  const why = !dir || !existsSync(join(dir, ROM)) ? `no ${ROM} in SATURNUS_ROM_DIR` : txBuilt() ? null : "web/pkg or web/tx/pkg not built (just web tx-web)";
  if (why) {
    if (process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return null;
  }
  const emu = await session(t, { files: { "/__rom": join(dir, ROM) }, audit: true });
  if (!emu) return null;
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
  return emu;
}

/** Press and release `key` on the emulator page's calculator. */
const tap = (emu, key) => emu.ev(`window.saturnus.backend.keyDown('${key}'); new Promise((r) => setTimeout(r, 150)).then(() => { window.saturnus.backend.keyUp('${key}'); return true; })`);

/** The commands tx sent that type, press keys or use the serial port. */
const typing = (tx) => tx.ev(`window.__sent.filter((c) => /^(serial|run|insert|replace|typeText|typeKeys|typeLetter|keyDown|keyUp|probeServer|endServer|setSpeed)/.test(c))`);

const CHIP = `(() => { const c = document.getElementById("tx-chip"); return c && !c.hidden ? c.textContent.trim() : null; })()`;

test("the emulator and saturnus tx use the 48GX at once; nothing is typed on it", { timeout: 600_000 }, async (t) => {
  const emu = await emulatorOn48gx(t);
  if (!emu) return;
  const stackBefore = await emu.ev("window.saturnus.backend.stackTop().then((s) => s.depth)");
  assert.equal(await emu.ev(CHIP), null, "no chip before tx comes");

  // saturnus tx on the same running calculator: the variable is there at once.
  const first = await txOnKept(emu);
  t.diagnostic(`tx ready on the running 48GX (connected and HOME listed): ${first.readyMs} ms`);
  assert.ok((await first.p.names()).includes("FROMEMU"), "the emulator's variable is listed in tx");
  assert.deepEqual(await typing(first.tx), [], "tx typed nothing, never opened the serial port");
  assert.equal(await emu.ev("window.saturnus.store.state.booted"), "48gx", "the emulator still runs it");
  assert.equal(await emu.ev("window.saturnus.backend.commandLine().then((c) => !!c?.active)"), false, "no command line in the emulator");
  await until(emu.ev, `${CHIP} === "Connected to saturnus tx · Show"`, 10_000, "the emulator's chip");
  assert.equal(await emu.ev(`document.getElementById("tx-show").target`), "saturnus-tx");
  assert.equal(await emu.ev(`[...document.querySelectorAll("#tx-chip button, #tx-chip a")].length`), 1, "Show only: no Disconnect");

  // Typed in the emulator: in tx's list without a refresh.
  assert.equal((await emu.ev("window.saturnus.backend.run(\"7 'TYPED' STO\")")).error ?? null, null);
  const typed = Date.now();
  await first.p.until("tx.names(0).includes('TYPED')", 10_000, "the variable typed in the emulator listed in tx");
  t.diagnostic(`a variable typed in the emulator listed in tx after ${Date.now() - typed} ms`);

  // A drop into tx is in the emulator's memory at once.
  await first.p.drop({ "FROMTX.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 6 7 * \\>>\r\n" });
  await first.p.idle(STEP_MS, "FROMTX put");
  assert.ok((await first.p.names()).includes("FROMTX"));
  assert.ok((await emu.ev(EMU_HOME)).includes("FROMTX"), "tx's copy is in the emulator's memory");
  // To a folder and back over the name (the question, then the variable replaced).
  await first.tx.ev("tx.memoryFolder(1).then(() => true)");
  await first.p.until("tx.idle(1)", 10_000, "the folder shown");
  await first.tx.ev(`tx.panes[0].act("copy", [tx.entry("FROMEMU")]); true`);
  await first.p.until("tx.idle(0) && tx.idle(1) && tx.names(1).includes('FROMEMU.hp')", STEP_MS, "FROMEMU copied to the folder");
  await first.tx.ev(`window.__back = tx.panes[1].act("copy", [tx.entry("FROMEMU.hp", 1)]); true`);
  await first.p.answer("Replace FROMEMU?");
  await first.p.until("tx.idle(0) && tx.idle(1)", STEP_MS, "FROMEMU copied back");
  assert.equal((await emu.ev(EMU_HOME)).filter((n) => n.startsWith("FROMEMU")).join(), "FROMEMU", "replaced, no FROMEMU.1");
  assert.deepEqual(await typing(first.tx), [], "still nothing typed");
  assert.equal(await emu.ev("window.saturnus.backend.stackTop().then((s) => s.depth)"), stackBefore, "the emulator's stack as it was");
  assert.deepEqual(first.tx.errors, [], "no error in tx");

  // A serialOpen from either page on the shared calculator is refused in words.
  const SHARED = "This calculator is shared with another page, so its serial port cannot be opened here. Use a calculator of its own (a ROM file chosen in saturnus tx).";
  assert.equal(await emu.ev("window.saturnus.backend.request('serialOpen').then(() => 'opened', (e) => String(e.message ?? e))"), SHARED);
  assert.equal(await first.tx.ev("tx.panes[0].place.device.calc.request('serialOpen').then(() => 'opened', (e) => String(e.message ?? e))"), SHARED);

  // tx disconnects there: the chip goes; the emulator runs on.
  await first.tx.ev("tx.panes[0].close().then(() => true)");
  await until(emu.ev, `${CHIP} === null`, 10_000, "the chip gone after tx disconnected");
  assert.equal((await emu.ev("window.saturnus.backend.run('FROMEMU 1 +')")).error ?? null, null, "a typed line runs in the emulator");

  // tx again, its tab closed: the chip goes once the hub learns it.
  const second = await txOnKept(emu);
  await until(emu.ev, `${CHIP} !== null`, 10_000, "the chip again");
  await second.tx.send("Page.close");
  await until(emu.ev, `${CHIP} === null`, 30_000, "the chip gone after tx's tab closed");

  // tx again, then the emulator's tab closed: tx goes on copying.
  const third = await txOnKept(emu);
  await emu.send("Page.close");
  await sleep(500);
  await third.p.drop({ "AFTER.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 1 \\>>\r\n" });
  await third.p.idle(STEP_MS, "AFTER put with the emulator closed");
  const names = await third.p.names();
  assert.ok(names.includes("AFTER") && names.includes("FROMTX") && names.includes("FROMEMU"), `tx's calculator ran on: ${names}`);
  assert.deepEqual(third.tx.errors, [], "no error in tx");
});

// The user presses ATTN or opens a command line in the emulator while
// saturnus tx works the same 48GX: tx never waits. A read goes on (the
// memory is read from RAM); a write with a command line open is refused
// at once in tx's words, the line left as the user typed it; cancelled
// there, the write goes through.
test("ATTN or an open command line in the emulator: tx is refused at once in words, never left waiting", { timeout: 300_000 }, async (t) => {
  const emu = await emulatorOn48gx(t);
  if (!emu) return;
  const { tx, p } = await txOnKept(emu);
  assert.ok((await p.names()).includes("FROMEMU"));

  // ATTN on the emulator's keyboard: tx reads and writes on, at once.
  await tap(emu, "on");
  const start = Date.now();
  await tx.ev("tx.panes[0].ui.refresh.click(); true");
  await p.idle(10_000, "HOME read again after ATTN");
  await p.drop({ "AFTERON.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 2 \\>>\r\n" });
  await p.idle(10_000, "AFTERON put after ATTN");
  assert.ok((await p.names()).includes("AFTERON"));
  t.diagnostic(`read and put after ATTN in ${Date.now() - start} ms`);

  // A command line open in the emulator: the put is refused at once, in words; the line stays the user's.
  await tap(emu, "1");
  await until(emu.ev, "window.saturnus.backend.commandLine().then((c) => c?.active)", 10_000, "a command line open in the emulator");
  const refused = Date.now();
  await p.drop({ "REFUSED.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 3 \\>>\r\n" });
  await p.until(`[...document.querySelectorAll(".toast")].some((e) => /The calculator is busy in the emulator: finish or cancel the command line there\\./.test(e.textContent))`, 5_000, "the refusal said");
  assert.ok(Date.now() - refused < 5_000, "at once");
  await p.idle(5_000, "nothing left waiting");
  assert.equal((await p.names()).includes("REFUSED"), false);
  assert.equal(await emu.ev("window.saturnus.backend.commandLine().then((c) => c?.active && c.text)"), "1", "the command line is still the user's");
  assert.deepEqual(await typing(tx), [], "tx typed nothing");

  // Cancelled there (ATTN): the put goes through.
  await tap(emu, "on");
  await until(emu.ev, "window.saturnus.backend.commandLine().then((c) => !c?.active)", 10_000, "the command line cancelled");
  await p.drop({ "REFUSED.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 3 \\>>\r\n" });
  await p.idle(10_000, "REFUSED put once the line was cancelled");
  assert.ok((await p.names()).includes("REFUSED"));

  // A .hp file dropped on the drawn 48GX: stored in the current directory, said in a toast; tx lists it.
  const dropOnCalc = (name, bytesExpr) => emu.ev(`(async () => {
    const dt = new DataTransfer();
    dt.items.add(new File([${bytesExpr}], ${JSON.stringify(name)}));
    document.querySelector("sat-calculator").dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
    return true;
  })()`);
  await emu.ev("window.saturnus.backend.fetchFile(['HOME'], 'FROMEMU').then((r) => { window.__hp = r.data; return true; })");
  await dropOnCalc("COPIED.hp", "window.__hp");
  await until(emu.ev, `[...document.querySelectorAll(".toast")].some((e) => /COPIED\\.hp stored as COPIED in HOME/.test(e.textContent))`, 30_000, "the toast");
  assert.ok((await emu.ev(EMU_HOME)).includes("COPIED"));
  await p.until("tx.names(0).includes('COPIED')", 10_000, "listed in tx without a refresh");
  // A ROM dropped there is still a ROM: nothing stored, the 48GX still runs.
  const before = await emu.ev(EMU_HOME);
  await emu.ev(`fetch("/__rom").then((r) => r.arrayBuffer()).then((b) => { window.__rom = b; return true; })`);
  await dropOnCalc(ROM, "window.__rom");
  await until(emu.ev, "window.saturnus.store.state.booted === '48gx' && window.saturnus.backend.stats().then((s) => s.loop === 'sleep')", 60_000, "the 48GX runs");
  await sleep(1000);
  assert.deepEqual(await emu.ev(EMU_HOME), before, "the ROM was not stored as a variable");
});

// The emulator's "Files" link on the running 48GX:
// `?connect=48gx`, and saturnus tx's left pane on that calculator with no
// question; asked again (the request the link sends to tx's open tab,
// web/shared/site-links.js), nothing changes. The tab lookup itself:
// web/test/site-links.test.mjs.
test("the emulator's link connects saturnus tx's left pane to the running 48GX, once", { timeout: 300_000 }, async (t) => {
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
  await until(emu.ev, `document.getElementById("to-transfer").getAttribute("href") === "tx.html?connect=48gx"`, 30_000, "the link on the running 48GX");
  assert.equal(await emu.ev(`document.getElementById("to-transfer").target`), "saturnus-tx");
  await until(emu.ev, "window.saturnus.backend.stats().then((s) => s.emulatedMs > 3000 && s.loop === 'sleep')", 60_000, "the 48GX is up");

  const tx = await emu.another();
  await tx.send("Page.enable");
  await tx.send("Runtime.enable");
  const p = txPage(tx);
  await p.open(emu.port, "?connect=48gx");
  assert.equal(await tx.ev("location.search"), "", "?connect= taken off the address");
  await p.until("!!tx.panes[0].place?.hub && tx.panes[0].place.model === '48gx'", STEP_MS, "the left pane on the emulator's 48GX");
  await p.idle(STEP_MS, "HOME listed");
  assert.equal(await tx.ev("document.querySelector('dialog[open]')?.id ?? null"), null, "no question asked");

  // Asked again for the calculator it shows (the request the emulator's link sends to tx's open tab): nothing.
  await tx.ev("window.__place = tx.panes[0].place; tx.connectFromEmulator('48gx').then((r) => r === null)");
  assert.equal(await tx.ev("tx.panes[0].place === window.__place && !tx.panes[0].step && !document.querySelector('dialog[open]')"), true, "the same connection, nothing started, nothing asked");
  assert.deepEqual(tx.errors, [], "no error in tx");
});
