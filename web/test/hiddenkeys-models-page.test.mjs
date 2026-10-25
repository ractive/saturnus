// The unprinted key functions and the ON key's menu of the models beyond
// the 48GX (iteration 65, web/hiddenkeys.js, web/onkeys.js), in the real
// page in headless Chrome. No ROM: the page believes the model runs and
// records the presses (web/test/hiddenkeys-helpers.mjs). Checked: on the
// 48SX ⌥ shows SERVER over PRG (I/O hidden meanwhile) and lights the
// left-printed MODES green, where → opens its other menu; on the 49G the
// held-shift lines in the tooltips and no ghosts in its cursor pad, and
// its tests menu asks first; on the 38G (one shift) both
// modifiers light the one shift's labels, the SETUP bracket's functions
// in the tooltips, its ON menu, and Clear memory asks first; the 19BII's
// OFF ghost over ON; the 42S's menu named after its EXIT key. With
// `SATURNUS_ROM_DIR`, one ON-menu action per family on its ROM: the
// 48SX's Restart and its held Reset memory, the 49G's Restart, the 28S's
// Restart, the 38G's Clear memory, and the Pioneers' Clear memory, which
// needs the keys going down together (42S, 17BII, 19BII). Screenshots
// with a modifier held go to `SATURNUS_SHOTS` when it names a folder.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { sleep } from "./chrome.mjs";
import { fake, page, shot } from "./hiddenkeys-helpers.mjs";

const title = (p, k) => p.ev(`document.querySelector("sat-calculator").skinKey(${JSON.stringify(k)}).querySelector("title").textContent`);
/** The printed label `text` in the skin, as a selector expression. */
const printed = (text) => `[...document.querySelectorAll(".skin .print text")].find((n) => n.textContent === ${JSON.stringify(text)} && !n.classList.contains("ghost"))`;

test("48SX: ⌥ shows SERVER over PRG and lights MODES green; the ON menu has ON+MTH", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("48sx")), "48sx");
  assert.equal(await p.visible('[data-ghost="prg"]'), false, "no ghost at rest");
  await p.modifier("alt", true);
  await p.lit("glow-right");
  assert.equal(await p.visible('[data-ghost="prg"]'), true, "⌥: SERVER over PRG");
  assert.equal(await p.ev(`document.querySelector('.skin [data-ghost="prg"]').textContent`), "SERVER");
  assert.equal(await p.ev(`getComputedStyle(${printed("I/O")}).visibility`), "hidden", "I/O hidden while ⌥ is held");
  assert.equal(await p.visible('[data-ghost="9"]'), true, "EQ CAT over 9");
  assert.equal(await p.ev(`document.querySelectorAll('.skin [data-ghost="left"]').length`), 0, "a command-line function has no ghost");
  await sleep(600);
  assert.ok(await p.ev(`getComputedStyle(${printed("MODES")}).filter.includes("glow-right")`), "⌥: MODES lit in the right shift's colour");
  assert.equal(await p.ev(`getComputedStyle(${printed("MODES")}).opacity`), "1", "and not faded");
  assert.equal(await p.ev(`getComputedStyle(${printed("PRINT")}).opacity`), "0.4", "PRINT (→ MTH does nothing) fades");
  await shot(p, "hidden-keys-48sx-alt-1280.png");
  await p.modifier("alt", false);
  await p.lit(null);
  assert.equal(await p.ev(`getComputedStyle(${printed("I/O")}).visibility`), "visible", "I/O back on release");

  assert.equal(await title(p, "prg"), "PRG (alpha H)\n⌃-click: I/O\n⌥-click: Kermit server (not printed)");
  assert.equal(await title(p, "7"), "7\n⌃-click: SOLVE\n⌥-click: SOLVR menu of the current equation");
  assert.equal(await title(p, "2"), "2\n⌃-click: STACK (LAST STACK, the stack before the last command line)\n⌥-click: ARG (LAST ARG, the last command's arguments)");
  assert.equal(await title(p, "on"), "ON\n⌃-click: CONT\n⌥-click: OFF\nLong press or right-click: Restart, Self-test, Print display, Reset memory");

  await p.click("on", { button: "right" });
  assert.deepEqual(await p.menu(), ["RestartON+C", "Self-testON+D", "Print displayON+MTH", "Reset memory…ON+A+F"]);
  assert.deepEqual(await p.sent(), []);
  await p.pick("Print display");
  for (let i = 0; i < 50 && (await p.sent()).length < 4; i++) await sleep(50);
  assert.deepEqual(await p.sent(), ["down on", "down mth", "up mth", "up on"]);
});

test("49G: held-shift functions in the tooltips; no ghosts in the cursor pad, which has no label slot; ON+F keys", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("49g")), "49g");
  assert.equal(await title(p, "mode"), "MODE (alpha H)\n⌃-click: CUSTOM\n⌥-click: END\nWith the left shift held down: MODES menu (not printed)");
  assert.equal(await title(p, "right"), "▶ (RIGHT)\n⌃-click: SWAP, as ▶; in a command line, the cursor to its end (not printed)\n⌥-click: XModem server; in a command line, the cursor to its end; in algebraic mode it types XSERV() (not printed)\nWith the right shift held down: Kermit server (not printed)");
  assert.equal(await title(p, "a"), "F1 (A, alpha A)\n⌃-click: Y= (algebraic mode; in RPN mode hold the left shift down)");
  await p.modifier("alt", true);
  await p.lit("glow-right");
  assert.equal(await p.ev(`document.querySelectorAll(".skin .ghost").length`), 0, "no ghost: the cursor pad has no label slot, held functions never have one");
  await shot(p, "hidden-keys-49g-alt-1280.png");
  await p.modifier("alt", false);
  await p.click("on", { button: "right" });
  assert.deepEqual(await p.menu(), ["RestartON+F3", "Self-test…ON+F4", "Reset memory…ON+F1+F6"]);
  await p.pick("Restart");
  for (let i = 0; i < 50 && (await p.sent()).length < 4; i++) await sleep(50);
  assert.deepEqual(await p.sent(), ["down on", "down c", "up c", "up on"], "F3 is the key c");
  // The tests menu asks first: its test 9 formats the flash.
  await p.ev("window.sent = []");
  await p.click("on", { button: "right" });
  await p.pick("Self-test");
  for (let i = 0; i < 50 && !(await p.ev(`!!document.querySelector("dialog.confirm[open]")`)); i++) await sleep(50);
  assert.match(await p.ev(`document.querySelector("dialog.confirm").textContent`), /formats the flash/);
  await p.ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === "Cancel").click(), true`);
  await sleep(200);
  assert.deepEqual(await p.sent(), [], "Cancel: nothing pressed");
});

test("38G: one shift for both modifiers, the SETUP bracket's functions, its ON menu; Clear memory asks", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("38g")), "38g");
  assert.equal(await title(p, "plot"), "PLOT\n⌃- or ⌥-click: SETUP-PLOT, the plot setup view (the SETUP bracket)");
  assert.equal(await title(p, "up"), "▲ (UP)\n⌃- or ⌥-click: the beginning of a list; in HOME, the oldest entry (not printed)");
  assert.equal(await p.ev(`document.querySelectorAll(".skin .ghost").length`), 0, "no ghosts on the 38G");
  for (const mod of ["alt", "ctrl"]) {
    await p.modifier(mod, true);
    await p.lit("glow-left");
    await p.modifier(mod, false);
    await p.lit(null);
  }
  await p.modifier("ctrl", true);
  await p.lit("glow-left");
  await sleep(600);
  await shot(p, "hidden-keys-38g-ctrl-1280.png");
  await p.modifier("ctrl", false);
  assert.match(await title(p, "on"), /\nLong press or right-click: Restart, Self-test, Continuous test, Clear memory$/);
  await p.click("on", { button: "right" });
  assert.deepEqual(await p.menu(), ["RestartON+menu key 3", "Self-testON+menu key 4", "Continuous testON+menu key 5", "Clear memory…ON+menu keys 1 and 6"]);
  await p.pick("Clear memory");
  for (let i = 0; i < 50 && !(await p.ev(`!!document.querySelector("dialog.confirm[open]")`)); i++) await sleep(50);
  assert.equal(await p.ev(`document.querySelector("dialog.confirm h2").textContent`), "Clear the calculator's memory?");
  await p.ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent === "Cancel").click(), true`);
  await sleep(200);
  assert.deepEqual(await p.sent(), [], "Cancel: nothing pressed");
});

test("19BII: either modifier shows OFF over ON; the 42S's menu is named after EXIT", { timeout: 120_000 }, async (t) => {
  const p = await page(t);
  if (!p) return;
  assert.equal(await p.ev(fake("19bii")), "19bii");
  assert.equal(await p.visible('[data-ghost="on"]'), false);
  await p.modifier("alt", true);
  await p.lit("glow-left");
  assert.equal(await p.visible('[data-ghost="on"]'), true, "⌥: OFF over ON");
  await shot(p, "hidden-keys-19bii-alt-1280.png");
  await p.modifier("alt", false);
  await p.lit(null);
  assert.equal(await title(p, "on"), "ON\n⌃- or ⌥-click: OFF (not printed)\nLong press or right-click: Restart, Self-test, Print display, Clear memory");

  assert.equal(await p.ev(fake("42s")), "42s");
  await p.click("on", { button: "right" });
  assert.equal(await p.ev(`document.querySelector('.menu[aria-label$=" combinations"]').getAttribute("aria-label")`), "EXIT combinations");
  assert.deepEqual(await p.menu(), ["RestartEXIT+√x", "Self-testEXIT+LN", "Clear memory…EXIT+Σ++XEQ"]);
});

// ---------------------------------------------------------------- ROMs

const HELPERS = `
  window.__idle = async (ms = 800) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (;;) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep" && !window.saturnus.store.state.keysDown.length) return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  };
  window.__screen = () => document.querySelector("sat-calculator").screenText();
  true`;

/** The page with `file` from SATURNUS_ROM_DIR booted as `model` (null: skipped). */
async function boot(t, file, model) {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, file) : null;
  if (!rom || !existsSync(rom)) {
    t.skip(`SATURNUS_ROM_DIR/${file} not found`);
    return null;
  }
  const p = await page(t, { rom });
  if (!p) return null;
  await p.ev(HELPERS);
  await p.ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], ${JSON.stringify(file)}));
    return true;
  })()`);
  for (let i = 0; i < 200 && (await p.ev("window.saturnus.store.state.booted")) !== model; i++) await sleep(100);
  assert.equal(await p.ev("window.saturnus.store.state.booted"), model);
  await p.ev("window.__idle(3000)");
  const tap = async (...keys) => {
    for (const k of keys) {
      await p.click(k);
      await p.ev("window.__idle()");
    }
  };
  /** The ON menu's entry `text`, confirmed when it asks. */
  const combo = async (text) => {
    await p.click("on", { button: "right" });
    await p.pick(text);
    await sleep(300);
    if (await p.ev(`!!document.querySelector("dialog.confirm[open]")`)) {
      await p.ev(`[...document.querySelectorAll("dialog.confirm button")].find((b) => b.textContent !== "Cancel").click(), true`);
    }
    await sleep(1500);
    await p.ev("window.__idle(2000)");
  };
  return { ...p, tap, combo, screen: () => p.ev("window.__screen()") };
}

test("48SX ROM: Restart keeps the variables; Reset memory, held a second, asks the ROM's question", { timeout: 240_000 }, async (t) => {
  const p = await boot(t, "sxrom-j", "48sx");
  if (!p) return;
  const stack = () => p.ev("window.saturnus.backend.stack().then((s) => s.map((l) => l.text))");
  const vars = () => p.ev("window.saturnus.backend.memoryTree().then((t) => t.variables.map((v) => v.name))");
  await p.ev(`window.saturnus.backend.run("42 'SATX' STO 1 2")`);
  await p.ev("window.__idle(500)");
  assert.deepEqual(await stack(), ["2", "1"]);
  await p.combo("Restart");
  assert.deepEqual(await stack(), [], "ON+C cleared the stack");
  assert.ok((await vars()).includes("SATX"), "and kept SATX");
  await p.combo("Reset memory");
  await p.tap("f");
  await p.ev("window.__idle(3000)");
  assert.ok(!(await vars()).includes("SATX"), "SATX gone after NO");
});

test("49G ROM: Restart (ON+F3) clears the stack and keeps the variables", { timeout: 240_000 }, async (t) => {
  const p = await boot(t, "rom.49g", "49g");
  if (!p) return;
  const stack = () => p.ev("window.saturnus.backend.stack().then((s) => s.map((l) => l.text))");
  const vars = () => p.ev("window.saturnus.backend.memoryTree().then((t) => t.variables.map((v) => v.name))");
  // RPN mode (flag -95 clear; the cold-booted 49G is in algebraic mode).
  await p.ev("window.saturnus.backend.setFlag(-95, false)");
  await p.ev(`window.saturnus.backend.run("42 'SATX' STO 1 2")`);
  await p.ev("window.__idle(800)");
  assert.deepEqual(await stack(), ["2", "1"]);
  await p.combo("Restart");
  assert.deepEqual(await stack(), [], "ON+F3 cleared the stack");
  assert.ok((await vars()).includes("SATX"), "and kept SATX");
});

test("28S ROM: Restart (ON+▲) clears the stack", { timeout: 240_000 }, async (t) => {
  const p = await boot(t, "hp28s.rom", "28s");
  if (!p) return;
  // ⬅ with no command line takes the cold start's "Memory Lost" away.
  await p.tap("backspace");
  const empty = await p.screen();
  await p.tap("1", "enter", "2", "enter");
  const two = await p.screen();
  assert.notEqual(two, empty, "two numbers on the stack");
  await p.combo("Restart");
  assert.notEqual(await p.screen(), two, "the stack screen changed");
  // The cleared stack shows as the empty one did, after any message is gone.
  await p.tap("backspace");
  assert.equal(await p.screen(), empty, "an empty stack, as after the boot");
});

/**
 * A Clear memory on a key-driven model: the screen after it is the one a
 * cold start shows (memory lost; after `start`, the Pioneers' language
 * prompt answered), not the one `setup` left.
 */
async function clearsMemory(t, file, model, setup, start = []) {
  const p = await boot(t, file, model);
  if (!p) return;
  await p.tap(...start);
  const cold = await p.screen();
  await setup(p);
  const before = await p.screen();
  assert.notEqual(before, cold, "the setup changed the screen");
  await p.combo("Clear memory");
  assert.equal(await p.screen(), cold, "the screen of a cold start: memory cleared");
}

test("38G ROM: Clear memory (ON+menu keys 1 and 6)", { timeout: 240_000 }, async (t) => {
  await clearsMemory(t, "38G_A167.ROM", "38g", (p) => p.tap("1", "plus", "2", "enter"));
});

test("42S ROM: Clear memory (EXIT+Σ++XEQ, the keys going down together)", { timeout: 240_000 }, async (t) => {
  await clearsMemory(t, "hp42s.rom", "42s", (p) => p.tap("5", "enter"));
});

test("17BII ROM: Clear memory (CLR+menu keys 1 and 6, together)", { timeout: 240_000 }, async (t) => {
  // The sixth menu key answers the language prompt (English).
  await clearsMemory(t, "hp17bii.rom", "17bii", (p) => p.tap("5", "plus"), ["f"]);
});

test("19BII ROM: Clear memory (ON+menu keys 1 and 6)", { timeout: 240_000 }, async (t) => {
  await clearsMemory(t, "hp19bii.rom", "19bii", (p) => p.tap("5", "plus"), ["f"]);
});

/**
 * A self-test from the ON menu, left with its Restart (wiki:
 * hardware/*-keyboard, "Leaving the tests"): the test changes the
 * screen, and Restart brings back the screen a Restart gives outside it.
 */
async function leavesTest(t, file, model, test, start = []) {
  const p = await boot(t, file, model);
  if (!p) return;
  await p.tap(...start);
  await p.combo("Restart");
  const restarted = await p.screen();
  // A running test never lets the machine sleep: no wait for idle here.
  await p.click("on", { button: "right" });
  await p.pick(test);
  await sleep(4000);
  assert.notEqual(await p.screen(), restarted, `${test} runs`);
  await p.combo("Restart");
  assert.equal(await p.screen(), restarted, `Restart left ${test}`);
}

test("39G ROM: the continuous test, left with Restart held 2 s", { timeout: 240_000 }, async (t) => {
  await leavesTest(t, "rom.39g", "39g", "Continuous test");
});

test("19BII ROM: the self-test, left with Restart", { timeout: 240_000 }, async (t) => {
  await leavesTest(t, "hp19bii.rom", "19bii", "Self-test", ["f"]);
});

test("17BII ROM: the self-test, left with Restart", { timeout: 240_000 }, async (t) => {
  await leavesTest(t, "hp17bii.rom", "17bii", "Self-test", ["f"]);
});
