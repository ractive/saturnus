// One live calculator per model, used by Calculator and Files at once
// (iterations 46 and 50d: the calculators' hub, web/hub.js, in the one
// document's Worker, each view over a port of its own), in one headless
// Chrome tab; Files works it through the engine's memory commands
// (iteration 47, web/shared/hub-device.js), never its serial port.
// Calculator runs the 48GX (gxrom-r from SATURNUS_ROM_DIR) and stores a
// variable; Files adds the emulated 48GX of the kept ROM, which is that
// running calculator: nothing is typed on it (no SERVER, no serialOpen
// in Files' traffic), the variable is listed at once, one typed in
// Calculator later shows without a refresh, Calculator shows "Connected
// to Files · Show". Copies go both ways (a drop into the calculator, a
// copy to a folder and back over the name). ATTN or an open command line
// in Calculator: Files is refused at once in words, never left waiting.
// A serialOpen from either view is refused. Files disconnects: the chip
// goes, Calculator runs on; a hidden view never stops the calculator.
// Calculator's "Files" link connects Files' left pane to the running
// calculator, again after a disconnect, once. Phones, background tabs
// and Safari are the owner's checks. Skipped without the ROM, Chrome or
// the wasm packages; SATURNUS_AUDIT=1 makes the skip a failure.
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

/** HOME's names in Calculator's calculator. */
const EMU_HOME = "window.saturnus.backend.memoryTree().then((t) => t.variables.map((v) => v.name))";

/** Files in the same tab as Calculator (`emu`), its left pane on the kept 48GX, the commands it sends to the hub logged (`__sent`); resolves to the session, its helpers and the ms until it was listed. */
async function txOnKept(emu) {
  const tx = emu;
  const p = txPage(tx);
  await tx.ev(`location.hash = "#/files"; true`);
  await until(tx.ev, "!!window.tx", 30_000, "Files mounted");
  await tx.ev("tx.started.then(() => true)");
  // Every command Files sends to the calculators' hub (its HubClient's), from the first.
  await tx.ev(`import("./shared/calc-hub.js").then((m) => {
    window.__sent = [];
    if (!m.HubClient.prototype.__logged) {
      const request = m.HubClient.prototype.request;
      m.HubClient.prototype.request = function (cmd, args) { window.__sent.push(cmd); return request.call(this, cmd, args); };
      m.HubClient.prototype.__logged = true;
    }
    return true;
  })`);
  await tx.ev(`document.querySelector('.pane[data-pane=left] [data-action="add-emulated"]').click(); true`);
  await until(tx.ev, `document.getElementById("emulated-dialog").open && !!document.querySelector("#emulated-roms .rom-choice")`, 10_000, "the kept 48GX offered");
  const start = Date.now();
  await tx.ev(`document.querySelector("#emulated-roms .rom-choice").click(); true`);
  await p.until("!!tx.panes[0].place?.device?.connection", STEP_MS, "Files connected to the shared 48GX");
  await p.idle(STEP_MS, "HOME listed in Files");
  return { tx, p, readyMs: Date.now() - start };
}

/** Calculator on the running 48GX with FROMEMU stored, or null when skipped. */
async function emulatorOn48gx(t) {
  const why = !dir || !existsSync(join(dir, ROM)) ? `no ${ROM} in SATURNUS_ROM_DIR` : txBuilt() ? null : "web/pkg not built (just web)";
  if (why) {
    if (process.env.SATURNUS_AUDIT) assert.fail(why);
    t.skip(why);
    return null;
  }
  const emu = await session(t, { files: { "/__rom": join(dir, ROM) }, audit: true });
  if (!emu) return null;
  await emu.send("Page.enable");
  await emu.send("Runtime.enable");
  await emu.send("Page.navigate", { url: `http://127.0.0.1:${emu.port}/index.html#/calculator` });
  await until(emu.ev, "!!window.saturnus", 30_000, "Calculator started");
  await emu.ev("window.saturnus.started");
  assert.equal(await emu.ev("window.saturnus.backend.shared"), true, "Calculator runs in the hub");
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

/** Press and release `key` on Calculator's calculator. */
const tap = (emu, key) => emu.ev(`window.saturnus.backend.keyDown('${key}'); new Promise((r) => setTimeout(r, 150)).then(() => { window.saturnus.backend.keyUp('${key}'); return true; })`);

/** The commands Files sent that type, press keys or use the serial port. */
const typing = (tx) => tx.ev(`window.__sent.filter((c) => /^(serial|run|insert|replace|typeText|typeKeys|typeLetter|keyDown|keyUp|probeServer|endServer|setSpeed)/.test(c))`);

const CHIP = `(() => { const c = document.getElementById("tx-chip"); return c && !c.hidden ? c.textContent.trim() : null; })()`;

test("Calculator and Files use the 48GX at once; nothing is typed on it; a hidden view never stops it", { timeout: 600_000 }, async (t) => {
  const emu = await emulatorOn48gx(t);
  if (!emu) return;
  const stackBefore = await emu.ev("window.saturnus.backend.stackTop().then((s) => s.depth)");
  assert.equal(await emu.ev(CHIP), null, "no chip before Files comes");

  // Files on the same running calculator: the variable is there at once.
  const first = await txOnKept(emu);
  t.diagnostic(`Files ready on the running 48GX (connected and HOME listed): ${first.readyMs} ms`);
  assert.ok((await first.p.names()).includes("FROMEMU"), "Calculator's variable is listed in Files");
  assert.deepEqual(await typing(first.tx), [], "Files typed nothing, never opened the serial port");
  assert.equal(await emu.ev("window.saturnus.store.state.booted"), "48gx", "Calculator still runs it");
  assert.equal(await emu.ev(`document.getElementById("calculator-view").hidden`), true, "Calculator hidden, not removed");
  assert.notEqual(await emu.ev("window.saturnus.backend.stats().then((s) => s.loop)"), "stopped", "the hidden view's calculator runs on");
  assert.equal(await emu.ev("window.saturnus.backend.commandLine().then((c) => !!c?.active)"), false, "no command line in Calculator");
  await until(emu.ev, `${CHIP} === "Connected to Files · Show"`, 10_000, "Calculator's chip");
  assert.equal(await emu.ev(`document.getElementById("tx-show").getAttribute("href")`), "#/files/48gx");
  assert.equal(await emu.ev(`[...document.querySelectorAll("#tx-chip button, #tx-chip a")].length`), 1, "Show only: no Disconnect");

  // Typed in Calculator: in Files' list without a refresh.
  assert.equal((await emu.ev("window.saturnus.backend.run(\"7 'TYPED' STO\")")).error ?? null, null);
  const typed = Date.now();
  await first.p.until("tx.names(0).includes('TYPED')", 10_000, "the variable typed in Calculator listed in Files");
  t.diagnostic(`a variable typed in Calculator listed in Files after ${Date.now() - typed} ms`);

  // A drop into Files is in Calculator's memory at once.
  await first.p.drop({ "FROMTX.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 6 7 * \\>>\r\n" });
  await first.p.idle(STEP_MS, "FROMTX put");
  assert.ok((await first.p.names()).includes("FROMTX"));
  assert.ok((await emu.ev(EMU_HOME)).includes("FROMTX"), "Files' copy is in Calculator's memory");
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
  assert.equal(await emu.ev("window.saturnus.backend.stackTop().then((s) => s.depth)"), stackBefore, "Calculator's stack as it was");
  assert.deepEqual(emu.errors, [], "no error in the page");

  // A serialOpen from either view on the shared calculator is refused in words.
  const SHARED = "This calculator is shared with another view, so its serial port cannot be opened here. Use a calculator of its own (a ROM file chosen in Files).";
  assert.equal(await emu.ev("window.saturnus.backend.request('serialOpen').then(() => 'opened', (e) => String(e.message ?? e))"), SHARED);
  assert.equal(await first.tx.ev("tx.panes[0].place.device.calc.request('serialOpen').then(() => 'opened', (e) => String(e.message ?? e))"), SHARED);

  // Files disconnects there: the chip goes; Calculator runs on.
  await first.tx.ev("tx.panes[0].close().then(() => true)");
  await until(emu.ev, `${CHIP} === null`, 10_000, "the chip gone after Files disconnected");
  assert.equal((await emu.ev("window.saturnus.backend.run('FROMEMU 1 +')")).error ?? null, null, "a typed line runs in Calculator");

  // Files again, back and forth between the views: Files goes on copying, the calculator runs throughout.
  const again = await txOnKept(emu);
  await until(emu.ev, `${CHIP} !== null`, 10_000, "the chip again");
  await emu.ev(`location.hash = "#/calculator"; true`);
  await until(emu.ev, `saturnusApp.view === "calculator"`, 5_000, "Calculator shown");
  await again.p.drop({ "AFTER.txt": "%%HP: T(3)A(R)F(.);\r\n\\<< 1 \\>>\r\n" });
  await again.p.idle(STEP_MS, "AFTER put with Files hidden");
  assert.ok((await emu.ev(EMU_HOME)).includes("AFTER"), "Files' copy made while it was hidden");
  await emu.ev(`history.back(); true`);
  await until(emu.ev, `saturnusApp.view === "files"`, 5_000, "back: Files");
  const names = await again.p.names();
  assert.ok(names.includes("AFTER") && names.includes("FROMTX") && names.includes("FROMEMU"), `the calculator ran on: ${names}`);
  assert.deepEqual(emu.errors, [], "no error in the page");
});

// The user presses ATTN or opens a command line in Calculator while
// Files works the same 48GX: Files never waits. A read goes on (the
// memory is read from RAM); a write with a command line open is refused
// at once in tx's words, the line left as the user typed it; cancelled
// there, the write goes through.
test("ATTN or an open command line in Calculator: Files is refused at once in words, never left waiting", { timeout: 300_000 }, async (t) => {
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

  // A .hp file dropped on the drawn 48GX (Calculator shown): stored in the current directory, said in a toast; Files lists it.
  await emu.ev(`location.hash = "#/calculator"; true`);
  await until(emu.ev, `saturnusApp.view === "calculator"`, 5_000, "Calculator shown");
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
  await p.until("tx.names(0).includes('COPIED')", 10_000, "listed in Files without a refresh");
  // A ROM dropped there is still a ROM: nothing stored, the 48GX still runs.
  const before = await emu.ev(EMU_HOME);
  await emu.ev(`fetch("/__rom").then((r) => r.arrayBuffer()).then((b) => { window.__rom = b; return true; })`);
  await dropOnCalc(ROM, "window.__rom");
  await until(emu.ev, "window.saturnus.store.state.booted === '48gx' && window.saturnus.backend.stats().then((s) => s.loop === 'sleep')", 60_000, "the 48GX runs");
  await sleep(1000);
  assert.deepEqual(await emu.ev(EMU_HOME), before, "the ROM was not stored as a variable");
});

/** A real click (the mouse) on `selector` in page `c`. */
async function click(c, selector) {
  const r = await c.ev(`(() => { const e = document.querySelector(${JSON.stringify(selector)}); e.scrollIntoView({ block: "center" }); const b = e.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; })()`);
  for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
    await c.send("Input.dispatchMouseEvent", { type, x: r.x, y: r.y, button: "left", clickCount: 1 });
  }
}

// Calculator's "Files" link on the running 48GX (owner, 2026-10-16: it
// must connect, with no "Add an emulated calculator"), clicked with the
// mouse: `#/files/48gx`, Files shown in the same document with its left
// pane on that calculator, HOME listed, nothing asked. Files disconnects;
// the link again connects again; once more, while it shows that
// calculator, nothing changes. One tab throughout.
test("Calculator's Files link connects Files' left pane to the running 48GX, HOME listed, once", { timeout: 300_000 }, async (t) => {
  const emu = await emulatorOn48gx(t);
  if (!emu) return;
  await until(emu.ev, `document.getElementById("to-transfer").getAttribute("href") === "#/files/48gx"`, 30_000, "the link on the running 48GX");
  await emu.ev("window.__marker = 1; true");
  await click(emu, "#to-transfer");
  await until(emu.ev, `saturnusApp.view === "files" && !!window.tx`, 30_000, "Files shown");
  assert.equal(await emu.ev("location.hash"), "#/files/48gx");
  const p = txPage(emu);
  await p.until("!!tx.panes[0].place?.hub && tx.panes[0].place.model === '48gx'", STEP_MS, "the left pane on Calculator's 48GX");
  await p.idle(STEP_MS, "HOME listed");
  assert.ok((await p.names()).includes("FROMEMU"), "Calculator's variable listed");
  assert.equal(await emu.ev("document.querySelector('dialog[open]')?.id ?? null"), null, "nothing asked");
  assert.equal(await emu.ev(`document.getElementById("show-cable").hidden`), true, "no packets to show on Calculator's calculator");

  // Asked again for the calculator it shows: nothing.
  await emu.ev("window.__place = tx.panes[0].place; tx.connectFromEmulator('48gx').then((r) => r === null)");
  assert.equal(await emu.ev("tx.panes[0].place === window.__place && !tx.panes[0].step && !document.querySelector('dialog[open]')"), true, "the same connection, nothing started, nothing asked");

  // Settings opens while on it (owner, 2026-10-17: it did nothing, the shared calculator has no line settings).
  await emu.ev(`document.getElementById("open-settings").click()`);
  await p.until(`document.getElementById("settings-dialog").open && document.getElementById("set-now").hidden`, 5_000, "the settings, no line to name");
  await emu.ev(`document.getElementById("settings-dialog").close(); true`);

  // Disconnected in Files; back in Calculator, Files again: connected again.
  await emu.ev("tx.panes[0].close().then(() => true)");
  await p.until("!tx.panes[0].place", 10_000, "disconnected");
  await click(emu, "#to-emulator");
  await until(emu.ev, `saturnusApp.view === "calculator"`, 5_000, "Calculator shown");
  await click(emu, "#to-transfer");
  await p.until("!!tx.panes[0].place?.hub && tx.panes[0].place.model === '48gx'", STEP_MS, "connected again from Calculator");
  await p.idle(STEP_MS, "HOME listed again");

  // Files once more while it shows the 48GX: the same connection.
  await emu.ev("window.__place = tx.panes[0].place; true");
  await emu.ev(`location.hash = "#/calculator"; true`);
  await until(emu.ev, `saturnusApp.view === "calculator"`, 5_000, "Calculator shown");
  await click(emu, "#to-transfer");
  await sleep(2 * 1000);
  assert.equal(await emu.ev("tx.panes[0].place === window.__place && !tx.panes[0].step && !document.querySelector('dialog[open]')"), true, "the same connection, nothing started, nothing asked");
  assert.equal(await emu.ev("window.__marker"), 1, "never loaded again");
  assert.equal((await emu.tabs()).length, 1, "one tab");
  assert.deepEqual(emu.errors, [], "no error in the page");
});
