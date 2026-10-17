// Set the clock, in the real page in headless Chrome: the button and the
// "every start" checkbox show for every model with a clock and not for
// the 42S; the checkbox is kept as a preference; a busy calculator or an
// open command line is refused in plain words, nothing sent; a set sends
// this computer's local time and says so. With `SATURNUS_ROM_DIR`
// holding `gxrom-r`: on a running 48GX the clock is set and DATE reads
// today, an open command line is refused, and with the checkbox on a
// loaded state gets the clock set by itself. Skipped without Chrome or
// web/pkg.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { session, sleep } from "./chrome.mjs";

async function open(t, files = undefined) {
  const c = await session(t, files ? { files } : undefined);
  if (!c) return null;
  const { send, ev, port } = c;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 860, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html#/calculator` });
  for (let i = 0; i < 150 && !(await ev("!!window.saturnus").catch(() => false)); i++) await sleep(100);
  await ev("window.saturnus.started.then(() => true)");
  return c;
}

const shown = (sel) => `!document.querySelector(${JSON.stringify(sel)}).closest("[hidden]")`;

test("the button and the checkbox show for the models with a clock", { timeout: 120_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  const { ev } = c;
  for (const model of ["48sx", "48gx", "49g", "38g", "39g", "40g", "42s", "27s", "17bii", "17b", "28s", "19bii"]) {
    await ev(`window.saturnus.store.set({ model: ${JSON.stringify(model)} }); true`);
    const clock = model !== "42s" && model !== "28s";
    assert.equal(await ev(shown("#set-clock")), clock, `${model}: the button`);
    assert.equal(await ev(shown("#clock-start")), clock, `${model}: the checkbox`);
  }
  // Off until a calculator runs, saying why.
  await ev(`window.saturnus.store.set({ model: "48gx" }); true`);
  assert.equal(await ev(`document.querySelector("#set-clock").disabled`), true);
  assert.equal(await ev(`document.querySelector("#set-clock").title`), "Start the calculator first");
  // The checkbox is a preference, off at first.
  assert.equal(await ev(`document.querySelector("#clock-start").checked`), false);
  await ev(`document.querySelector("#clock-start").click(); true`);
  assert.equal(await ev(`localStorage.getItem("saturnus.clockOnStart")`), "on");
  await ev(`document.querySelector("#clock-start").click(); true`);
  assert.equal(await ev(`localStorage.getItem("saturnus.clockOnStart")`), "off");
});

test("busy or typing is refused in plain words; a set sends the local time", { timeout: 120_000 }, async (t) => {
  const c = await open(t);
  if (!c) return;
  const { ev } = c;
  // A running 48GX as far as the controls know, the host's setClock
  // replaced by a recorder.
  await ev(`(() => {
    window.__sent = [];
    window.saturnus.backend.setClock = async (fields) => { window.__sent.push(fields); return null; };
    window.saturnus.store.set({ model: "48gx", booted: "48gx" });
    return true;
  })()`);
  // The state and the click go in one step: a status update from the
  // running page between the two would undo the state under test.
  const click = async (state = {}) => {
    await ev(`window.saturnus.store.set({ message: "", ...${JSON.stringify(state)} }); document.querySelector("#set-clock").click(); true`);
    for (let i = 0; i < 20 && !(await ev("window.saturnus.store.state.message")); i++) await sleep(50);
    return ev("window.saturnus.store.state.message");
  };
  assert.equal(await click({ busy: true }), "The calculator is busy. Try again when it has finished.");
  assert.equal(await click({ busy: false, cmdlineOpen: true }), "Finish what you're typing first.");
  assert.equal(await ev("window.__sent.length"), 0, "nothing sent");
  const before = await ev("Date.now()");
  const said = await click({ cmdlineOpen: false });
  assert.match(said, /^Clock set to /);
  const [f] = await ev("window.__sent");
  const sent = new Date(f.year, f.month - 1, f.day, f.hour, f.minute, f.second, f.millisecond).getTime();
  assert.ok(Math.abs(sent - before) < 5000, `${JSON.stringify(f)} is the local time`);
  // The host's own refusal is said as it is.
  await ev(`window.saturnus.backend.setClock = async () => { throw new Error("The calculator is off. Turn it on first."); }; true`);
  assert.equal(await click(), "The calculator is off. Turn it on first.");
});

test("48GX ROM: the clock is set, DATE reads today; an open line is refused; every start sets it", { timeout: 180_000 }, async (t) => {
  const dir = process.env.SATURNUS_ROM_DIR;
  const rom = dir ? join(dir, "gxrom-r") : null;
  if (!rom || !existsSync(rom)) {
    t.skip("SATURNUS_ROM_DIR/gxrom-r not found");
    return;
  }
  const c = await open(t, { "/__rom": rom });
  if (!c) return;
  const { ev } = c;
  await ev(`(async () => {
    const bytes = await (await fetch("/__rom")).arrayBuffer();
    await window.saturnus.startWithRom(new File([bytes], "gxrom-r"));
    return true;
  })()`);
  for (let i = 0; i < 150 && (await ev("window.saturnus.store.state.booted")) !== "48gx"; i++) await sleep(100);
  // Idle: past the boot's question, which the host answers.
  await ev(`window.__idle = async (ms = 1500) => {
    const b = window.saturnus.backend;
    const t0 = (await b.stats()).emulatedMs;
    for (;;) {
      const s = await b.stats();
      if (s.emulatedMs - t0 >= ms && s.loop === "sleep") return true;
      await new Promise((r) => setTimeout(r, 50));
    }
  }; true`);
  await ev("window.__idle(4000)");
  const today = await ev(`(() => { const d = new Date(); return d.getMonth() + 1 + "." + String(d.getDate()).padStart(2, "0") + d.getFullYear(); })()`);
  const message = async () => {
    for (let i = 0; i < 100 && !(await ev("window.saturnus.store.state.message")); i++) await sleep(100);
    return ev("window.saturnus.store.state.message");
  };
  await ev(`window.saturnus.store.set({ message: "" }); document.querySelector("#set-clock").click(); true`);
  assert.match(await message(), /^Clock set to /);
  const date = async () => {
    const r = await ev(`window.saturnus.backend.run("DATE").then(() => window.saturnus.backend.stack()).then((s) => s[0].text)`);
    await ev(`window.saturnus.backend.run("DROP").then(() => true)`);
    return r;
  };
  assert.equal(await date(), today);
  // A command line open: refused, the line kept.
  await ev(`window.saturnus.backend.insert("12").then(() => true)`);
  for (let i = 0; i < 40 && !(await ev("window.saturnus.store.state.cmdlineOpen")); i++) await sleep(100);
  await ev(`window.saturnus.store.set({ message: "" }); document.querySelector("#set-clock").click(); true`);
  assert.equal(await message(), "Finish what you're typing first.");
  assert.equal(await ev(`window.saturnus.backend.commandLine().then((l) => l.text)`), "12");
  await ev(`window.saturnus.backend.replace("").then(() => { window.saturnus.backend.typeKeys(["on"]); return true; })`);
  await ev("window.__idle()");
  // Every start: a state saved with a wrong date, loaded with the box on.
  await ev(`window.saturnus.backend.run("1.022001 →DATE").then(() => true)`);
  assert.equal(await date(), "1.022001");
  await ev(`window.saturnus.backend.request("saveState").then((r) => { window.__state = r.state; return true; })`);
  await ev(`document.querySelector("#clock-start").click(); true`);
  await ev(`window.saturnus.backend.loadStateBytes(window.__state).then(() => { document.querySelector("sat-controls").started(); return true; })`);
  // The set waits for 1.5 s of sleep: looks far enough apart not to keep it from that.
  let got = null;
  for (let i = 0; i < 12 && got !== today; i++) {
    await sleep(2500);
    got = await date().catch(() => null);
  }
  assert.equal(got, today, "the clock set after the state was loaded");
});
