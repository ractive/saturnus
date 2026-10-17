// The fast transfer (XModem) of the 48GX and 49G (web/tx/xmodem.js,
// pane.js): the words of its dialog and the menus that offer it, without
// a browser; then the page in headless Chrome with the fake calculator
// (web/test/tx-fake-calc.js) and a folder in memory: a get into the
// folder and a put from it, the dialog's steps (what to type, waiting,
// done, the reconnect), a name in use refused before the server ends,
// the card of a calculator out of server mode, and the dialog inside the
// window at five widths in both themes. The real XRECV and XSEND run in
// `just tx-e2e DIR` (tx-e2e.test.mjs). Chrome missing: the page tests
// skip (`SATURNUS_AUDIT=1` makes that a failure).

import assert from "node:assert/strict";
import { test } from "node:test";
import { paneActions, rowActions } from "../tx/actions.js";
import { fastDone, fastWords, hasXmodem } from "../tx/xmodem.js";
import { session, sleep } from "./chrome.mjs";
import { txBuilt, txPage } from "./tx-chrome.mjs";

// ---------------------------------------------------------------- without a browser

test("the dialog's words: before the plan, with it, and after", () => {
  const before = fastWords({ direction: "put", name: "PRG", model: "48gx" });
  assert.equal(before.title, "Fast transfer of PRG to the calculator");
  assert.equal(before.keys, null);
  assert.match(before.lead, /ends server mode on the HP 48G\/GX/);
  const plan = { keys: "'PRG' XRECV", switchedToRpn: false };
  const gx = fastWords({ direction: "put", name: "PRG", model: "48gx", plan });
  assert.equal(gx.keys, "'PRG' XRECV");
  assert.match(gx.lead, /type this and press ENTER/);
  assert.ok(gx.notes.some((n) => /empty string/.test(n)), "the 48GX's empty string after a failed XRECV");
  assert.match(gx.status, /up to 3 minutes/);
  const g49 = fastWords({ direction: "get", name: "PRG", model: "49g", plan: { keys: "'PRG' XSEND", switchedToRpn: true }, emulated: true });
  assert.equal(g49.title, "Fast transfer of PRG from the calculator");
  assert.match(g49.lead, /On the emulated calculator/);
  assert.ok(g49.notes.some((n) => /algebraic mode/.test(n)));
  assert.ok(!g49.notes.some((n) => /empty string/.test(n)));

  const done = fastDone({ direction: "get", name: "PRG", model: "49g", where: "PRG.hp in hp-files" });
  assert.equal(done.lead, "PRG came over: PRG.hp in hp-files.");
  assert.ok(done.ok);
  assert.match(done.restart, /right-shift and then → .* then reconnect/);
  assert.deepEqual(done.keys, ["↱", "→"]);
  const emu = fastDone({ direction: "put", name: "PRG", model: "48gx", emulated: true });
  assert.equal(emu.lead, "PRG is on the calculator.");
  assert.match(emu.restart, /Reconnect starts its server again/);
  assert.equal(emu.keys, null);
  const late = fastDone({ direction: "put", name: "PRG", model: "48gx", error: { kind: "xmodemTimeout", message: "x" } });
  assert.match(late.lead, /Was XRECV typed/);
  assert.ok(!late.ok);
  const before2 = fastDone({ direction: "put", name: "PRG", model: "48gx", error: { kind: "nameClash", varName: "PRG", message: "PRG already exists" }, left: false });
  assert.equal(before2.restart, null, "the server never ended: nothing to restart");
  assert.match(before2.lead, /Could not send PRG: PRG is already there/);
  assert.match(fastDone({ direction: "get", name: "A", model: "49g", error: { kind: "aborted" } }).lead, /^Stopped/);
});

test("the menus offer it on the 48GX and 49G only", () => {
  assert.ok(hasXmodem("48gx") && hasXmodem("49g") && !hasXmodem("48sx") && !hasXmodem("unknown"));
  const ids = (items) => items.filter((i) => i !== "-").map((i) => i.id);
  const v = { name: "PRG", directory: false };
  const row = (opts) => rowActions({ kind: "device", entries: [v], ...opts });
  assert.ok(ids(row({ other: "folder", xmodem: true })).includes("xmodemGet"));
  assert.equal(row({ other: "folder", xmodem: true }).find((i) => i.id === "xmodemGet").text, "Fast transfer to the other pane (XModem)…");
  assert.equal(row({ other: null, xmodem: true }).find((i) => i.id === "xmodemGet").text, "Fast transfer as a download (XModem)…");
  assert.ok(!ids(row({ other: "folder", xmodem: false })).includes("xmodemGet"));
  assert.ok(!ids(rowActions({ kind: "device", entries: [{ name: "D", directory: true }], other: "folder", xmodem: true })).includes("xmodemGet"));
  assert.ok(!ids(rowActions({ kind: "device", entries: [v, { name: "B" }], other: "folder", xmodem: true })).includes("xmodemGet"));
  const file = (name, opts) => ids(rowActions({ kind: "folder", entries: [{ name, directory: false }], ...opts }));
  assert.ok(file("PRG.hp", { other: "device", xmodem: true }).includes("xmodemPut"));
  assert.ok(!file("PRG.txt", { other: "device", xmodem: true }).includes("xmodemPut"));
  assert.ok(!file("PRG.hp", { other: "device", xmodem: false }).includes("xmodemPut"));
  assert.ok(!file("PRG.hp", { other: "folder", xmodem: true }).includes("xmodemPut"));
  const pane = (opts) => paneActions({ kind: "device", ...opts });
  assert.ok(ids(pane({ xmodem: true })).includes("xmodemPut"));
  assert.ok(pane({ xmodem: true, connected: false }).find((i) => i.id === "xmodemPut").disabled);
  assert.ok(!ids(pane({ xmodem: false })).includes("xmodemPut"));
});

// ---------------------------------------------------------------- the page

const WIDTHS = [360, 390, 430, 768, 1280];
const HEIGHTS = { 360: 780, 390: 844, 430: 932, 768: 1024, 1280: 900 };

/** The page with the fake calculator module, `__calc(i, model, opts)` and `__folder(i)` (as tx-page.test.mjs). */
async function fakePage(t) {
  if (!txBuilt()) {
    if (process.env.SATURNUS_AUDIT) assert.fail("web/pkg not built (just web)");
    t.skip("web/pkg not built (just web)");
    return null;
  }
  const c = await session(t, { audit: true });
  if (!c) return null;
  await c.send("Page.enable");
  await c.send("Runtime.enable");
  await c.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  const p = txPage(c);
  await p.open(c.port);
  await c.ev(`import("./test/tx-fake-calc.js").then((m) => {
    window.__calcs = [];
    window.__calc = async (i, model = "48gx", opts = {}) => {
      const calc = m.fakeCalc({ model, ...opts });
      __calcs[i] = calc;
      tx.app.deviceOptions = { worker: calc.worker };
      await tx.panes[i].connectLink(m.stubLink(opts.label ?? "stub link"), { model });
      return true;
    };
    window.__folder = (i, title = "hp-files") => tx.memoryFolder(i, title).then(() => true);
    return true;
  })`);
  return { c, p };
}

const dialogText = (cls) => `(document.querySelector("#xmodem-dialog .${cls}")?.textContent ?? "")`;
const shown = (act) => `(() => { const b = document.querySelector('#xmodem-dialog [data-act="${act}"]'); return !!b && !b.hidden; })()`;
const press = (act) => `(() => { document.querySelector('#xmodem-dialog [data-act="${act}"]').click(); return true; })()`;
const binary = (body) => `new TextEncoder().encode("HPHP48-E" + ${JSON.stringify(body)})`;

test("a fast get into the folder and a fast put from it: what to type, the wait, the reconnect", { timeout: 120_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  const { ev } = c;
  await ev(`__calc(0, "48gx", { holdXmodem: true })`);
  await ev(`__folder(1)`);
  await p.idle(10_000, "the calculator listed");
  await ev(`tx.panes[0].place.device.write(["HOME"], "PRG", new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n\\\\<< 1 2 + \\\\>>\\r\\n")).then(() => true)`);
  await p.idle(10_000, "PRG put");

  // The row's menu offers it on a 48GX.
  await ev(`document.querySelector('.pane[data-pane=left] device-list tr[data-name="PRG"] button.more').click(); true`);
  await p.until(`!!document.querySelector(".menu")`, 5_000, "the row's menu");
  assert.match(await ev(`document.querySelector(".menu").textContent`), /Fast transfer to the other pane \(XModem\)…/);
  await ev(`document.querySelector(".menu").remove(); true`);

  // Get: the dialog first says what happens, then what to type.
  await ev(`window.__get = tx.panes[0].act("xmodemGet", [tx.entry("PRG")]); true`);
  await p.until(`document.getElementById("xmodem-dialog")?.open`, 5_000, "the dialog");
  assert.equal(await ev(dialogText("xmodem-title")), "Fast transfer of PRG from the calculator");
  await p.until(`${dialogText("xmodem-command")} === "'PRG' XSEND"`, 5_000, "what to type");
  assert.match(await ev(dialogText("xmodem-lead")), /^Server mode has ended\. On the HP 48G\/GX, type this and press ENTER:/);
  assert.match(await ev(dialogText("xmodem-status")), /Waiting for the calculator/);
  assert.equal(await ev(shown("cancel")), true);
  assert.equal(await ev(shown("type")), false, "no typing for a calculator on a cable");
  assert.match(await ev(`document.querySelector("#queue-list").textContent`), /Fast transfer of PRG from/);
  // The user typed XSEND: it comes over, into the folder.
  await ev(`__calcs[0].release(); true`);
  await p.until(shown("reconnect"), 5_000, "done, with the reconnect");
  assert.equal(await ev(dialogText("xmodem-lead")), "PRG came over: PRG.hp in hp-files.");
  assert.match(await ev(dialogText("xmodem-restart-text")), /out of server mode\. Press right-shift, then →/);
  assert.equal(await ev(`document.querySelectorAll("#xmodem-dialog .xmodem-restart-keys kbd").length`), 2);
  const got = await ev(`tx.panes[1].place.folder.read(["hp-files"], "PRG.hp").then((f) => new TextDecoder().decode(f.data))`);
  assert.match(got, /^HPHP48-/);
  // Later: the pane says the server has ended, and other requests wait for a reconnect.
  await ev(press("later"));
  await ev("__get");
  await p.until(`document.querySelector(".pane[data-pane=left] .start h2")?.textContent === "The calculator left server mode"`, 5_000, "the card");
  assert.equal(await ev(`tx.panes[0].place.device.leftServer`), true);
  await ev(`document.querySelector('.pane[data-pane=left] [data-action="reconnect"]').click(); true`);
  await p.idle(10_000, "reconnected");

  // Put from the folder: a binary file, its name asked, then the same steps; Reconnect from the dialog.
  await ev(`tx.panes[1].place.folder.write(["hp-files"], "Q.hp", ${binary("String:fast")}).then(() => true)`);
  await p.until(`!!tx.entry("Q.hp", 1)`, 5_000, "Q.hp listed");
  await ev(`window.__put = tx.panes[1].act("xmodemPut", [tx.entry("Q.hp", 1)]); true`);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name");
  assert.equal(await ev(`document.getElementById("name-input").value`), "Q");
  await ev(`document.getElementById("name-ok").click(); true`);
  await p.until(`${dialogText("xmodem-command")} === "'Q' XRECV"`, 5_000, "what to type");
  assert.match(await ev(`[...document.querySelectorAll("#xmodem-dialog .xmodem-notes li")].map((l) => l.textContent).join(" ")`), /Q may be left on the calculator as an empty string/);
  await ev(`__calcs[0].release(); true`);
  await p.until(shown("reconnect"), 5_000, "done");
  assert.equal(await ev(dialogText("xmodem-lead")), "Q is on the calculator.");
  await ev(press("reconnect"));
  await ev("__put");
  await p.idle(10_000, "reconnected from the dialog");
  assert.equal((await ev(`tx.entry("Q")`))?.kind, "String");
  const kinds = await ev(`__calcs[0].requests.map((r) => r.type)`);
  assert.deepEqual(kinds.filter((k) => /^xmodem|^connect/.test(k)), ["connect", "xmodem-get", "connect", "xmodem-put", "connect"]);

  // A name in use: refused before the server ends, nothing to restart.
  await ev(`window.__put = tx.panes[1].act("xmodemPut", [tx.entry("Q.hp", 1)]); true`);
  await p.until(`document.getElementById("name-dialog").open`, 5_000, "the name");
  await ev(`document.getElementById("name-ok").click(); true`);
  await p.until(shown("close"), 5_000, "the refusal");
  assert.match(await ev(dialogText("xmodem-lead")), /Could not send Q: Q is already there: a fast transfer does not replace it/);
  assert.equal(await ev(`document.querySelector("#xmodem-dialog .xmodem-restart").hidden`), true);
  await ev(press("close"));
  await ev("__put");
  assert.equal(await ev(`tx.panes[0].place.device.leftServer`), false);
  assert.ok(await ev(`!!tx.panes[0].place.device.connection`));
  assert.deepEqual(c.errors, []);
});

test("Cancel while waiting stops the transfer; the 48SX and a text file are not offered", { timeout: 60_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  const { ev } = c;
  await ev(`__calc(0, "49g", { holdXmodem: true })`);
  await ev(`__folder(1)`);
  await p.idle(10_000);
  await ev(`tx.panes[0].place.device.write(["HOME"], "A", new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n1\\r\\n")).then(() => true)`);
  await p.idle(10_000);
  // Cancel while the job still waits in the page's queue (another job holds the calculator): dropped there, the server never ends.
  await ev(`tx.queue.add({ label: "busy", places: [tx.panes[0].place.id], run: () => new Promise((r) => { window.__free = r; }) }); true`);
  await ev(`__calcs[0].requests.length = 0; window.__queued = tx.panes[0].act("xmodemGet", [tx.entry("A")]); true`);
  await p.until(`document.getElementById("xmodem-dialog")?.open`, 5_000, "the dialog");
  await ev(press("cancel"));
  await p.until(shown("close"), 5_000, "cancelled");
  assert.equal(await ev(dialogText("xmodem-lead")), "Cancelled before it started: the calculator is still in server mode.");
  await ev(press("close"));
  await ev("__queued");
  await ev("__free(); true");
  await p.idle(10_000, "the busy job done");
  assert.equal(await ev(`__calcs[0].requests.filter((r) => r.type.startsWith("xmodem")).length`), 0, "nothing of the fast transfer sent");
  assert.equal(await ev(`tx.panes[0].place.device.leftServer`), false);
  // Running: Cancel stops the device's wait (the fake answers the abort as the driver does), the dialog says so.
  await ev(`window.__get = tx.panes[0].act("xmodemGet", [tx.entry("A")]); true`);
  await p.until(`${dialogText("xmodem-command")} === "'A' XSEND"`, 5_000, "what to type");
  await ev(press("cancel"));
  await p.until(shown("reconnect"), 5_000, "stopped");
  assert.match(await ev(dialogText("xmodem-lead")), /^Stopped\. If the calculator still waits, press ON on it\./);
  await ev(press("later"));
  await ev("__get");
  // A 48SX: no fast transfer in its menus.
  await ev(`__calc(0, "48sx")`);
  await p.idle(10_000);
  await ev(`tx.panes[0].place.device.write(["HOME"], "B", new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n1\\r\\n")).then(() => true)`);
  await p.idle(10_000);
  await ev(`document.querySelector('.pane[data-pane=left] .pane-more').click(); true`);
  await p.until(`!!document.querySelector(".menu")`, 5_000, "the pane's menu");
  assert.doesNotMatch(await ev(`document.querySelector(".menu").textContent`), /XModem/);
  await ev(`document.querySelector(".menu").remove(); true`);
  assert.equal(await ev(`tx.panes[0].xmodem`), false);
  assert.deepEqual(c.errors, []);
});

test("the dialog stays inside the window at five widths in both themes", { timeout: 200_000 }, async (t) => {
  const f = await fakePage(t);
  if (!f) return;
  const { c, p } = f;
  const { send, ev, settled } = c;
  await ev(`__calc(0, "49g", { holdXmodem: true, label: "USB serial port with a long name" })`);
  await ev(`__folder(1)`);
  await p.idle(10_000);
  await ev(`tx.panes[0].place.device.write(["HOME"], "AVERYLONGVARIABLENAMEINDEED", new TextEncoder().encode("%%HP: T(3)A(R)F(.);\\r\\n1\\r\\n")).then(() => true)`);
  await p.idle(10_000);
  await ev(`tx.panes[0].act("xmodemGet", [tx.entry("AVERYLONGVARIABLENAMEINDEED")]); true`);
  await p.until(`${dialogText("xmodem-command")} === "'AVERYLONGVARIABLENAMEINDEED' XSEND"`, 5_000, "what to type");
  const measure = `(() => {
    const d = document.getElementById("xmodem-dialog");
    const r = d.getBoundingClientRect();
    const wide = [...d.querySelectorAll("*")].filter((e) => e.scrollWidth > e.clientWidth + 1 && /auto|scroll/.test(getComputedStyle(e).overflowX)).length;
    return { page: document.documentElement.scrollWidth - document.documentElement.clientWidth, left: r.left, right: r.right, inner: innerWidth, wide };
  })()`;
  const failures = [];
  const check = async (state) => {
    for (const w of WIDTHS) {
      for (const scheme of ["light", "dark"]) {
        await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
        await send("Emulation.setDeviceMetricsOverride", { width: w, height: HEIGHTS[w], deviceScaleFactor: 2, mobile: w < 1000 });
        await sleep(100);
        const m = await settled(measure);
        if (m.page > 0) failures.push(`${state} at ${w}px ${scheme}: the page is ${m.page}px too wide`);
        if (m.left < -0.5 || m.right > m.inner + 0.5) failures.push(`${state} at ${w}px ${scheme}: the dialog is outside the window`);
        if (m.wide) failures.push(`${state} at ${w}px ${scheme}: ${m.wide} parts scroll sideways`);
      }
    }
  };
  await check("waiting");
  await ev(`__calcs[0].release(); true`);
  await p.until(shown("reconnect"), 5_000, "done");
  await check("done");
  assert.deepEqual(failures, []);
  assert.deepEqual(c.errors, []);
});
